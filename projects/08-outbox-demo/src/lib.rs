//! outbox-demo — the production pattern for atomic business writes + async
//! side effects, in ~200 lines.
//!
//! Two tables:
//!   * `transfers` — the business state.
//!   * `outbox` — the queue of side effects to publish.
//!
//! Two operations:
//!   * `record_transfer` — inserts the business row AND an outbox row in one
//!     transaction. Both commit or neither does.
//!   * `claim_next` / `mark_done` / `mark_failed` — the worker's per-row
//!     lifecycle, with exponential backoff on failure.
//!
//! `SQLite` serializes writes so `claim_next` is naturally single-flight.
//! Postgres production uses `FOR UPDATE SKIP LOCKED` for parallel workers
//! (see Phase 11.5 lesson) — the API shape is identical.

use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Domain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct Transfer {
    pub id: i64,
    pub from_account: String,
    pub to_account: String,
    pub amount_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct OutboxRow {
    pub id: i64,
    pub kind: String,
    pub payload: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub next_attempt_at: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("transfer amount must be positive (got {0})")]
    NonPositiveAmount(i64),
    #[error("transfer accounts must differ")]
    SameAccount,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

pub type OutboxResult<T> = Result<T, OutboxError>;

// ---------------------------------------------------------------------------
// Producer side — write the business row AND the outbox row atomically.
// ---------------------------------------------------------------------------

/// Record a transfer and emit `transfer.recorded` to the outbox. Atomic.
pub async fn record_transfer(
    pool: &SqlitePool,
    from_account: &str,
    to_account: &str,
    amount_cents: i64,
) -> OutboxResult<(Transfer, i64)> {
    if amount_cents <= 0 {
        return Err(OutboxError::NonPositiveAmount(amount_cents));
    }
    if from_account == to_account {
        return Err(OutboxError::SameAccount);
    }

    let mut tx = pool.begin().await?;

    let transfer: Transfer = sqlx::query_as::<_, Transfer>(
        "INSERT INTO transfers (from_account, to_account, amount_cents)
         VALUES (?, ?, ?)
         RETURNING id, from_account, to_account, amount_cents, created_at",
    )
    .bind(from_account)
    .bind(to_account)
    .bind(amount_cents)
    .fetch_one(&mut *tx)
    .await?;

    let payload = serde_json::json!({
        "transfer_id": transfer.id,
        "from":        transfer.from_account,
        "to":          transfer.to_account,
        "amount_cents": transfer.amount_cents,
    })
    .to_string();

    let outbox_id: i64 = sqlx::query_scalar(
        "INSERT INTO outbox (kind, payload) VALUES ('transfer.recorded', ?) RETURNING id",
    )
    .bind(&payload)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((transfer, outbox_id))
}

// ---------------------------------------------------------------------------
// Worker side — claim, dispatch, mark.
// ---------------------------------------------------------------------------

/// Claim the next pending row. Returns `Some(row)` if work was claimed,
/// `None` if the queue is empty or no row is due. Concurrent callers see
/// disjoint rows.
pub async fn claim_next(pool: &SqlitePool) -> OutboxResult<Option<OutboxRow>> {
    // UPDATE ... RETURNING with a single-row subselect atomically transitions
    // *exactly one* eligible row from pending → processing. SQLite serializes
    // writes; the next caller can only see whatever is left.
    let row: Option<OutboxRow> = sqlx::query_as::<_, OutboxRow>(
        "UPDATE outbox SET status = 'processing', attempts = attempts + 1
         WHERE id = (
             SELECT id FROM outbox
             WHERE status = 'pending'
               AND next_attempt_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now')
             ORDER BY next_attempt_at, id
             LIMIT 1
         )
         RETURNING id, kind, payload, status, attempts, last_error,
                   next_attempt_at, created_at, completed_at",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Mark a claimed row as completed successfully.
pub async fn mark_done(pool: &SqlitePool, id: i64) -> OutboxResult<()> {
    sqlx::query(
        "UPDATE outbox SET status = 'done', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark a row failed and schedule a retry with exponential backoff
/// (`2^attempts` seconds, capped at 256 s). After `max_attempts` failures,
/// the row is moved to `status='failed'` and will not be retried.
pub async fn mark_failed(
    pool: &SqlitePool,
    id: i64,
    error_msg: &str,
    max_attempts: i64,
) -> OutboxResult<()> {
    let row: (i64,) = sqlx::query_as("SELECT attempts FROM outbox WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let attempts = row.0;

    if attempts >= max_attempts {
        sqlx::query(
            "UPDATE outbox SET status = 'failed', last_error = ?,
                                completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(error_msg)
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        // Backoff: 2^attempts seconds, capped at 256 s.
        let backoff = backoff_for_attempt(attempts);
        let next_at = (Utc::now() + chrono::Duration::seconds(backoff as i64)).to_rfc3339();
        sqlx::query(
            "UPDATE outbox SET status = 'pending', last_error = ?, next_attempt_at = ?
             WHERE id = ?",
        )
        .bind(error_msg)
        .bind(next_at)
        .bind(id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[must_use]
pub fn backoff_for_attempt(attempts: i64) -> u64 {
    let exp = attempts.clamp(0, 8) as u32;
    1u64 << exp
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OutboxStats {
    pub pending: i64,
    pub processing: i64,
    pub done: i64,
    pub failed: i64,
}

pub async fn stats(pool: &SqlitePool) -> OutboxResult<OutboxStats> {
    let row: OutboxStats = sqlx::query_as::<_, OutboxStats>(
        "SELECT
            COUNT(*) FILTER (WHERE status='pending')    AS pending,
            COUNT(*) FILTER (WHERE status='processing') AS processing,
            COUNT(*) FILTER (WHERE status='done')       AS done,
            COUNT(*) FILTER (WHERE status='failed')     AS failed
         FROM outbox",
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// A "dispatcher" trait so tests can plug in their own side-effect handler.
// ---------------------------------------------------------------------------

pub trait Dispatcher: Send + Sync {
    fn handle(&self, row: &OutboxRow) -> Result<(), String>;
}

/// One worker iteration: claim one row, dispatch, mark.
///
/// Returns `Ok(true)` if a row was processed; `Ok(false)` if the queue was
/// empty.
pub async fn run_once<D: Dispatcher>(
    pool: &SqlitePool,
    dispatcher: &D,
    max_attempts: i64,
) -> OutboxResult<bool> {
    let Some(row) = claim_next(pool).await? else {
        return Ok(false);
    };

    match dispatcher.handle(&row) {
        Ok(()) => {
            mark_done(pool, row.id).await?;
            tracing::info!(outbox_id = row.id, kind = %row.kind, "outbox row processed");
            Ok(true)
        }
        Err(e) => {
            mark_failed(pool, row.id, &e, max_attempts).await?;
            tracing::warn!(outbox_id = row.id, kind = %row.kind, error = %e, "outbox row failed");
            Ok(true)
        }
    }
}

/// Continuous worker loop. Sleeps `idle_poll_interval` when the queue is
/// empty. Returns when `cancel.await` fires.
pub async fn run_loop<D: Dispatcher>(
    pool: SqlitePool,
    dispatcher: D,
    idle_poll_interval: Duration,
    max_attempts: i64,
    cancel: impl std::future::Future<Output = ()>,
) -> OutboxResult<()> {
    tokio::pin!(cancel);
    loop {
        tokio::select! {
            biased;
            () = &mut cancel => return Ok(()),
            res = run_once(&pool, &dispatcher, max_attempts) => {
                match res {
                    Ok(true) => {}
                    Ok(false) => tokio::time::sleep(idle_poll_interval).await,
                    Err(e) => {
                        tracing::error!(error = %e, "worker error; sleeping");
                        tokio::time::sleep(idle_poll_interval).await;
                    }
                }
            }
        }
    }
}
