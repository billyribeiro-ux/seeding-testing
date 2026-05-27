//! event-sourcing — a teachable event-sourcing kernel.
//!
//! ## The split this crate teaches
//!
//! Event sourcing replaces "save current state" with "save the events
//! that *caused* the current state." The current state is then a
//! left-fold over those events. Three artifacts fall out of that:
//!
//! 1. **Events.** Immutable past-tense facts (`Deposited`, `Withdrew`).
//!    They carry data, not behavior.
//! 2. **Aggregate.** A consistency boundary that knows how to *apply*
//!    events (mutate in-memory state) and how to *handle* a command
//!    (return the events the command would produce, without mutating).
//!    The split between `apply` and `handle` is the heart of the
//!    pattern: `handle` is pure ("what would happen"), `apply` is the
//!    only mutator, and replay just calls `apply` over the persisted
//!    log.
//! 3. **EventStore.** An append-only log keyed by `(stream_id, version)`
//!    with a `UNIQUE` constraint that gives optimistic concurrency
//!    control for free. Two writers racing at the same expected version
//!    → one INSERT wins, the other gets a unique-violation we surface
//!    as [`EsError::ConcurrencyConflict`].
//!
//! The "CQRS" half of the pattern is a *consequence*, not a separate
//! decision: once events are the source of truth, the read model is
//! whatever shape your queries want. We ship a [`BalanceProjection`]
//! that polls the event log and maintains an `account_balances` table.
//!
//! ## Worked domain: `BankAccount`
//!
//! The classic. The invariants — "can't withdraw past the balance,"
//! "can't operate on a closed account" — are exactly the kind of
//! business rule that *should* be enforced inside the aggregate and
//! *must not* leak into the read model.
//!
//! ```text
//! Commands → handle (pure)   → Vec<Event>
//!                                  │
//!                                  ▼
//!                            EventStore.append(stream_id, expected_version, …)
//!                                  │
//!                                  ▼
//! State      ← apply (mut)   ← replay loop
//!                                  │
//!                                  ▼
//!                            BalanceProjection (poll loop)
//! ```
//!
//! ## What this crate is NOT
//!
//! - **Not a message broker.** `EventStore::subscribe` returns a poll
//!   stream against the event log, not a Kafka client.
//! - **Not a production engine.** No snapshotting, no upcasters, no
//!   schema-version column. The teaching doc
//!   (`docs/distributed-systems/05-event-sourcing-and-cqrs.md`) calls
//!   out the gotchas.
//! - **Not a "free upgrade" over CRUD.** Use it when the audit story or
//!   the "replay history into a new read model" story pays for the
//!   extra cognitive load. The doc has the full when-it-wins /
//!   when-it-loses table.

use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that escape the event store and aggregate layer.
#[derive(Debug, Error)]
pub enum EsError {
    /// Optimistic concurrency lost. Two writers raced; one of them must
    /// reload the stream and retry.
    #[error(
        "concurrency conflict on stream `{stream_id}`: expected version {expected}, actual {actual}"
    )]
    ConcurrencyConflict {
        stream_id: String,
        expected: i64,
        actual: i64,
    },

    /// A command violated a business invariant. The wrapped string names
    /// the specific rule that fired.
    #[error("domain invariant violated: {0}")]
    Domain(String),

    /// SQLx surfaced a database error we did not recognise as a
    /// concurrency conflict.
    #[error(transparent)]
    Db(#[from] sqlx::Error),

    /// We could not (de)serialize a payload.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type EsResult<T> = Result<T, EsError>;

// ---------------------------------------------------------------------------
// Event + Aggregate traits
// ---------------------------------------------------------------------------

/// A domain event — a past-tense fact about something that happened.
///
/// Implementors are typically data-only `serde` enums. The trait itself
/// is intentionally tiny; the orchestrator only needs `Send + Sync` and
/// the ability to (de)serialize.
pub trait Event: Send + Sync + Serialize + for<'de> Deserialize<'de> {
    /// A short, stable name for this event variant (e.g. `"Deposited"`).
    /// Stored in the `event_type` column so projections can route
    /// without parsing the payload.
    fn event_type(&self) -> &'static str;
}

/// An aggregate root — the consistency boundary for a stream.
///
/// The two methods enforce the event-sourcing split:
///
/// - [`Aggregate::apply`] is the ONLY mutator. It is called once per
///   event during replay. It must be deterministic and total: given the
///   same starting state and the same event, it always produces the
///   same next state, and it never fails.
/// - [`Aggregate::handle`] is pure. It inspects the current state plus
///   the incoming command and returns the events the command *would*
///   produce, or a domain error. It does NOT mutate. The orchestrator
///   persists the returned events first, *then* loops `apply` over
///   them.
///
/// Keeping these split is what makes replay safe: rebuilding state from
/// the log is just `events.iter().for_each(|e| state.apply(e))`. No
/// command logic runs during replay; no "did the user have permission
/// back then?" check fires; the log IS the truth.
pub trait Aggregate: Default + Sized {
    type Event: Event;
    type Command;
    type Error: std::error::Error;

    /// Mutate state from a single event. Must be deterministic. Never
    /// fails — if you find yourself wanting to return an error here,
    /// you have a bug in [`Aggregate::handle`].
    fn apply(&mut self, event: &Self::Event);

    /// Decide what events a command would produce. Pure — does not
    /// mutate `self`. The caller persists the returned events
    /// (atomically) before calling `apply` on them.
    fn handle(&self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error>;

    /// Replay a slice of events into a fresh aggregate. Convenience
    /// helper around `Default + apply` used by [`EventStore::load_aggregate`].
    fn replay(events: &[Self::Event]) -> Self {
        let mut state = Self::default();
        for e in events {
            state.apply(e);
        }
        state
    }
}

// ---------------------------------------------------------------------------
// Stored event envelope
// ---------------------------------------------------------------------------

/// One row in the event log, with the metadata the store maintains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    /// Global monotonically-increasing position across all streams.
    /// Projections checkpoint on this.
    pub position: i64,
    /// Which aggregate instance this event belongs to (e.g. an account
    /// id rendered as a string).
    pub stream_id: String,
    /// Per-stream version, starting at 1. Used for optimistic
    /// concurrency: `expected_version` in `append` must equal the
    /// current max version for the stream.
    pub version: i64,
    /// The event type name (e.g. `"Deposited"`).
    pub event_type: String,
    /// JSON-serialized event payload.
    pub payload_json: String,
    /// When the event was appended (wall clock, for human inspection
    /// only — never for ordering).
    pub occurred_at: DateTime<Utc>,
}

impl StoredEvent {
    /// Decode the payload into a concrete event type.
    pub fn decode<E: Event>(&self) -> EsResult<E> {
        Ok(serde_json::from_str(&self.payload_json)?)
    }
}

// ---------------------------------------------------------------------------
// EventStore
// ---------------------------------------------------------------------------

/// Append-only event store backed by a single SQLite table.
///
/// The table schema:
///
/// ```sql
/// CREATE TABLE events (
///     position     INTEGER PRIMARY KEY AUTOINCREMENT,
///     stream_id    TEXT    NOT NULL,
///     version      INTEGER NOT NULL,
///     event_type   TEXT    NOT NULL,
///     payload_json TEXT    NOT NULL,
///     occurred_at  TEXT    NOT NULL,
///     UNIQUE(stream_id, version)
/// );
/// ```
///
/// The `UNIQUE(stream_id, version)` index is the load-bearing piece.
/// Two writers each loading version N from a stream, computing
/// different next events, and trying to insert at version N+1 → exactly
/// one INSERT wins; the other fails with a unique-violation that we
/// surface as [`EsError::ConcurrencyConflict`]. The caller's job is to
/// reload the stream and retry.
#[derive(Clone)]
pub struct EventStore {
    pool: SqlitePool,
}

impl EventStore {
    /// Open an in-memory SQLite database, run the migration, and return
    /// the store. Each call creates a *new* database; for tests, share
    /// the returned store across tasks via [`Clone`] (SQLx pools are
    /// `Arc` internally).
    pub async fn in_memory() -> EsResult<Self> {
        // `?cache=shared&mode=memory` keeps the DB alive while any
        // connection in the pool is open; without it, an in-memory DB
        // would be per-connection and the pool would see different
        // databases on each connection.
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await?;
        Self::with_pool(pool).await
    }

    /// Wrap an existing pool. Useful when the caller has their own
    /// migration story.
    pub async fn with_pool(pool: SqlitePool) -> EsResult<Self> {
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Expose the underlying pool (lets projections add their own
    /// tables to the same DB so checkpoint + read-model live in one
    /// transaction-able place).
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn migrate(&self) -> EsResult<()> {
        // Plain `CREATE TABLE IF NOT EXISTS` — no separate migrations
        // dir to keep the crate self-contained for the lesson.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS events (
                position     INTEGER PRIMARY KEY AUTOINCREMENT,
                stream_id    TEXT    NOT NULL,
                version      INTEGER NOT NULL,
                event_type   TEXT    NOT NULL,
                payload_json TEXT    NOT NULL,
                occurred_at  TEXT    NOT NULL,
                UNIQUE(stream_id, version)
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Append events to a stream atomically at `expected_version`.
    ///
    /// `expected_version` is the version the caller believes the stream
    /// is at; the first appended event lands at `expected_version + 1`.
    /// An empty `events` slice is a no-op (returns `Ok(())`).
    ///
    /// # Errors
    ///
    /// - [`EsError::ConcurrencyConflict`] if another writer beat us to
    ///   `expected_version + 1`. The actual current version is reported
    ///   in the error so the caller's retry loop can pick up where it
    ///   left off.
    /// - [`EsError::Db`] for any other SQLx error.
    /// - [`EsError::Json`] if serialization fails.
    pub async fn append<E: Event>(
        &self,
        stream_id: &str,
        expected_version: i64,
        events: &[E],
    ) -> EsResult<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        let now = Utc::now().to_rfc3339();

        for (offset, event) in events.iter().enumerate() {
            let version = expected_version + 1 + offset as i64;
            let payload = serde_json::to_string(event)?;
            let event_type = event.event_type();

            let res = sqlx::query(
                "INSERT INTO events (stream_id, version, event_type, payload_json, occurred_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(stream_id)
            .bind(version)
            .bind(event_type)
            .bind(&payload)
            .bind(&now)
            .execute(&mut *tx)
            .await;

            if let Err(e) = res {
                // SQLite reports the UNIQUE(stream_id, version) collision
                // as a SQLITE_CONSTRAINT (code 2067 for unique). We don't
                // pattern-match the integer — we ask sqlx for the
                // categorization via `Error::Database` + its `is_unique_violation`.
                if let sqlx::Error::Database(db_err) = &e {
                    if db_err.is_unique_violation() {
                        // Roll back and report the actual current version.
                        drop(tx);
                        let actual = self.current_version(stream_id).await?;
                        return Err(EsError::ConcurrencyConflict {
                            stream_id: stream_id.to_string(),
                            expected: expected_version,
                            actual,
                        });
                    }
                }
                return Err(EsError::Db(e));
            }
        }

        tx.commit().await?;
        Ok(())
    }

    /// The current max `version` for a stream, or 0 if the stream is
    /// empty.
    pub async fn current_version(&self, stream_id: &str) -> EsResult<i64> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT MAX(version) FROM events WHERE stream_id = ?")
                .bind(stream_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map_or(0, |r| r.0))
    }

    /// Load every event for a stream, in version order.
    pub async fn load(&self, stream_id: &str) -> EsResult<Vec<StoredEvent>> {
        let rows = sqlx::query(
            "SELECT position, stream_id, version, event_type, payload_json, occurred_at
             FROM events WHERE stream_id = ? ORDER BY version ASC",
        )
        .bind(stream_id)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(stored_from_row(&row)?);
        }
        Ok(out)
    }

    /// Replay a stream into a fresh aggregate. Convenience: load +
    /// decode + fold.
    pub async fn load_aggregate<A: Aggregate>(&self, stream_id: &str) -> EsResult<A> {
        let stored = self.load(stream_id).await?;
        let mut events = Vec::with_capacity(stored.len());
        for s in &stored {
            events.push(s.decode::<A::Event>()?);
        }
        Ok(A::replay(&events))
    }

    /// Fetch all events with position strictly greater than `after`,
    /// across every stream. Used by [`EventStore::subscribe`] and
    /// directly by projections that prefer to drive their own polling.
    pub async fn read_from(&self, after: i64, limit: i64) -> EsResult<Vec<StoredEvent>> {
        let rows = sqlx::query(
            "SELECT position, stream_id, version, event_type, payload_json, occurred_at
             FROM events WHERE position > ? ORDER BY position ASC LIMIT ?",
        )
        .bind(after)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(stored_from_row(&row)?);
        }
        Ok(out)
    }

    /// Subscribe to the global event log starting after `from_position`.
    ///
    /// This is a *poll-based* subscription, not a real message broker.
    /// It returns a `Stream<StoredEvent>` backed by a tokio task that
    /// loops `read_from(checkpoint, batch_size)` every
    /// `poll_interval` and emits each new row. Drop the stream to stop
    /// the task.
    ///
    /// `batch_size` caps the rows pulled per poll so a quiet system
    /// does not block on a huge SELECT after a long sleep.
    pub fn subscribe(
        &self,
        from_position: i64,
        poll_interval: Duration,
        batch_size: i64,
    ) -> Pin<Box<dyn Stream<Item = StoredEvent> + Send>> {
        let pool = self.pool.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<StoredEvent>(64);

        tokio::spawn(async move {
            let mut checkpoint = from_position;
            loop {
                let res = sqlx::query(
                    "SELECT position, stream_id, version, event_type, payload_json, occurred_at
                     FROM events WHERE position > ? ORDER BY position ASC LIMIT ?",
                )
                .bind(checkpoint)
                .bind(batch_size)
                .fetch_all(&pool)
                .await;

                let rows = match res {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(error = %e, "subscribe poll failed");
                        tokio::time::sleep(poll_interval).await;
                        continue;
                    }
                };

                if rows.is_empty() {
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                for row in rows {
                    let stored = match stored_from_row(&row) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(error = %e, "subscribe decode failed");
                            continue;
                        }
                    };
                    checkpoint = stored.position;
                    if tx.send(stored).await.is_err() {
                        return; // receiver dropped — stop polling.
                    }
                }
            }
        });

        Box::pin(ReceiverStream::new(rx))
    }
}

fn stored_from_row(row: &sqlx::sqlite::SqliteRow) -> EsResult<StoredEvent> {
    let occurred_at_str: String = row.try_get("occurred_at")?;
    let occurred_at = DateTime::parse_from_rfc3339(&occurred_at_str)
        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
    Ok(StoredEvent {
        position: row.try_get("position")?,
        stream_id: row.try_get("stream_id")?,
        version: row.try_get("version")?,
        event_type: row.try_get("event_type")?,
        payload_json: row.try_get("payload_json")?,
        occurred_at,
    })
}

// ===========================================================================
// Worked domain: BankAccount
// ===========================================================================

/// Commands the outside world can submit against a bank account.
#[derive(Debug, Clone)]
pub enum BankCommand {
    OpenAccount { owner_id: u64 },
    Deposit { cents: i64 },
    Withdraw { cents: i64 },
    Close,
}

/// Events that result from those commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum BankEvent {
    Opened { owner_id: u64 },
    Deposited { cents: i64 },
    Withdrew { cents: i64 },
    Closed,
}

impl Event for BankEvent {
    fn event_type(&self) -> &'static str {
        match self {
            Self::Opened { .. } => "Opened",
            Self::Deposited { .. } => "Deposited",
            Self::Withdrew { .. } => "Withdrew",
            Self::Closed => "Closed",
        }
    }
}

/// Domain errors specific to the bank account.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankError {
    #[error("account is not open")]
    NotOpen,
    #[error("account is already open")]
    AlreadyOpen,
    #[error("account is closed")]
    Closed,
    #[error("amount must be positive (got {0})")]
    NonPositiveAmount(i64),
    #[error("insufficient funds: balance {balance}, attempted withdraw {attempted}")]
    InsufficientFunds { balance: i64, attempted: i64 },
}

/// Lifecycle of a bank account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccountStatus {
    /// Stream is empty — the account has never been opened. The
    /// `Default` impl chooses this; it is the only state where
    /// `OpenAccount` is legal.
    #[default]
    Uninitialized,
    Open,
    Closed,
}

/// The aggregate. The `apply`/`handle` split is the whole lesson — read
/// the trait docstrings on [`Aggregate`].
#[derive(Debug, Clone, Default)]
pub struct BankAccount {
    pub status: AccountStatus,
    pub owner_id: Option<u64>,
    pub balance_cents: i64,
}

impl Aggregate for BankAccount {
    type Event = BankEvent;
    type Command = BankCommand;
    type Error = BankError;

    fn apply(&mut self, event: &Self::Event) {
        match event {
            BankEvent::Opened { owner_id } => {
                self.status = AccountStatus::Open;
                self.owner_id = Some(*owner_id);
                self.balance_cents = 0;
            }
            BankEvent::Deposited { cents } => {
                self.balance_cents = self.balance_cents.saturating_add(*cents);
            }
            BankEvent::Withdrew { cents } => {
                self.balance_cents = self.balance_cents.saturating_sub(*cents);
            }
            BankEvent::Closed => {
                self.status = AccountStatus::Closed;
            }
        }
    }

    fn handle(&self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error> {
        // Each branch is an invariant check + the events the command
        // would produce. NOTHING mutates `self` — the caller persists
        // first, then loops `apply`.
        match command {
            BankCommand::OpenAccount { owner_id } => match self.status {
                AccountStatus::Uninitialized => Ok(vec![BankEvent::Opened { owner_id }]),
                AccountStatus::Open => Err(BankError::AlreadyOpen),
                AccountStatus::Closed => Err(BankError::Closed),
            },
            BankCommand::Deposit { cents } => {
                if cents <= 0 {
                    return Err(BankError::NonPositiveAmount(cents));
                }
                match self.status {
                    AccountStatus::Open => Ok(vec![BankEvent::Deposited { cents }]),
                    AccountStatus::Uninitialized => Err(BankError::NotOpen),
                    AccountStatus::Closed => Err(BankError::Closed),
                }
            }
            BankCommand::Withdraw { cents } => {
                if cents <= 0 {
                    return Err(BankError::NonPositiveAmount(cents));
                }
                match self.status {
                    AccountStatus::Open => {
                        if cents > self.balance_cents {
                            Err(BankError::InsufficientFunds {
                                balance: self.balance_cents,
                                attempted: cents,
                            })
                        } else {
                            Ok(vec![BankEvent::Withdrew { cents }])
                        }
                    }
                    AccountStatus::Uninitialized => Err(BankError::NotOpen),
                    AccountStatus::Closed => Err(BankError::Closed),
                }
            }
            BankCommand::Close => match self.status {
                AccountStatus::Open => Ok(vec![BankEvent::Closed]),
                AccountStatus::Uninitialized => Err(BankError::NotOpen),
                AccountStatus::Closed => Err(BankError::Closed),
            },
        }
    }
}

// ===========================================================================
// CQRS read model: BalanceProjection
// ===========================================================================

/// A projection consumes events and maintains a read model. The trait
/// is intentionally tiny — the heavy lifting lives in the concrete
/// implementation (which can be a SQL view, an in-memory map, a Redis
/// hash, …).
#[async_trait]
pub trait Projection: Send + Sync {
    /// Apply one stored event to the read model. Must be idempotent
    /// keyed by `position` so a restart from an old checkpoint does not
    /// double-apply.
    async fn handle(&self, event: &StoredEvent) -> EsResult<()>;

    /// The highest `position` this projection has already applied.
    /// Returned by the projection so the runner can resume after a
    /// restart.
    async fn checkpoint(&self) -> EsResult<i64>;
}

/// A read model that maintains the current balance of every account.
///
/// Backed by an `account_balances` table in the same SQLite database as
/// the event store. Updating the read model + the checkpoint happens in
/// a single transaction so a crash mid-update cannot leave the
/// checkpoint ahead of the data.
#[derive(Clone)]
pub struct BalanceProjection {
    pool: SqlitePool,
    /// Read-side cache so tests can assert without round-tripping to
    /// SQLite. Kept consistent with the table.
    cache: std::sync::Arc<Mutex<HashMap<String, i64>>>,
}

impl BalanceProjection {
    pub async fn new(store: &EventStore) -> EsResult<Self> {
        let pool = store.pool.clone();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS account_balances (
                stream_id     TEXT PRIMARY KEY,
                balance_cents INTEGER NOT NULL,
                status        TEXT    NOT NULL
            )",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS projection_checkpoints (
                name        TEXT PRIMARY KEY,
                position    INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await?;
        // Initialize our checkpoint row at 0 if not already present.
        sqlx::query(
            "INSERT INTO projection_checkpoints (name, position)
             VALUES ('balance', 0)
             ON CONFLICT(name) DO NOTHING",
        )
        .execute(&pool)
        .await?;
        Ok(Self {
            pool,
            cache: std::sync::Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Get the current balance for an account, if any. `None` means
    /// the projection has never seen an event for that stream.
    pub async fn balance(&self, stream_id: &str) -> EsResult<Option<i64>> {
        let cache = self.cache.lock().await;
        if let Some(v) = cache.get(stream_id) {
            return Ok(Some(*v));
        }
        drop(cache);
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT balance_cents FROM account_balances WHERE stream_id = ?")
                .bind(stream_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }
}

#[async_trait]
impl Projection for BalanceProjection {
    async fn handle(&self, event: &StoredEvent) -> EsResult<()> {
        // Each projection update is one transaction: write the read
        // model AND bump the checkpoint together. Crash recovery is
        // then trivially "resume from checkpoint."
        let mut tx = self.pool.begin().await?;

        // We only care about events we know how to interpret. Unknown
        // event_types are silently skipped (after still advancing the
        // checkpoint) — this is exactly the schema-evolution loophole
        // the lesson doc warns about.
        let decoded: Option<BankEvent> = serde_json::from_str(&event.payload_json).ok();

        if let Some(ev) = decoded {
            let (delta, status_update): (i64, Option<&str>) = match &ev {
                BankEvent::Opened { .. } => (0, Some("open")),
                BankEvent::Deposited { cents } => (*cents, None),
                BankEvent::Withdrew { cents } => (-*cents, None),
                BankEvent::Closed => (0, Some("closed")),
            };

            let prior: Option<(i64, String)> = sqlx::query_as(
                "SELECT balance_cents, status FROM account_balances WHERE stream_id = ?",
            )
            .bind(&event.stream_id)
            .fetch_optional(&mut *tx)
            .await?;

            let (new_balance, new_status) = match prior {
                Some((bal, st)) => (
                    bal + delta,
                    status_update.map_or(st, std::string::ToString::to_string),
                ),
                None => (delta, status_update.unwrap_or("open").to_string()),
            };

            sqlx::query(
                "INSERT INTO account_balances (stream_id, balance_cents, status)
                 VALUES (?, ?, ?)
                 ON CONFLICT(stream_id) DO UPDATE SET
                     balance_cents = excluded.balance_cents,
                     status        = excluded.status",
            )
            .bind(&event.stream_id)
            .bind(new_balance)
            .bind(&new_status)
            .execute(&mut *tx)
            .await?;

            // Keep the in-memory cache in sync.
            let mut cache = self.cache.lock().await;
            cache.insert(event.stream_id.clone(), new_balance);
        }

        sqlx::query(
            "UPDATE projection_checkpoints SET position = ? WHERE name = 'balance' AND position < ?",
        )
        .bind(event.position)
        .bind(event.position)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn checkpoint(&self) -> EsResult<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT position FROM projection_checkpoints WHERE name = 'balance'")
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }
}

/// Drive a projection by polling the event store. Returns when the
/// `cancel` future resolves. One iteration claims at most `batch_size`
/// rows; an empty poll sleeps `poll_interval`.
pub async fn run_projection<P: Projection>(
    store: EventStore,
    projection: P,
    poll_interval: Duration,
    batch_size: i64,
    cancel: impl std::future::Future<Output = ()>,
) -> EsResult<()> {
    tokio::pin!(cancel);
    loop {
        let checkpoint = projection.checkpoint().await?;
        let batch = store.read_from(checkpoint, batch_size).await?;
        if batch.is_empty() {
            tokio::select! {
                biased;
                () = &mut cancel => return Ok(()),
                () = tokio::time::sleep(poll_interval) => continue,
            }
        }
        for ev in batch {
            projection.handle(&ev).await?;
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn open_command() -> BankCommand {
        BankCommand::OpenAccount { owner_id: 42 }
    }

    // ---------- Aggregate-only tests (no DB) -------------------------------

    /// Apply-replay round trip: handling a sequence of commands and
    /// applying their events incrementally produces the same state as
    /// replaying the resulting events into a fresh aggregate.
    #[test]
    fn apply_replay_round_trip() {
        let mut acc = BankAccount::default();
        let mut log = Vec::new();

        for cmd in [
            open_command(),
            BankCommand::Deposit { cents: 1_000 },
            BankCommand::Deposit { cents: 500 },
            BankCommand::Withdraw { cents: 300 },
        ] {
            let evs = acc.handle(cmd).unwrap();
            for e in &evs {
                acc.apply(e);
            }
            log.extend(evs);
        }

        let replayed = BankAccount::replay(&log);
        assert_eq!(replayed.status, AccountStatus::Open);
        assert_eq!(replayed.balance_cents, 1_200);
        assert_eq!(replayed.owner_id, Some(42));
        assert_eq!(replayed.balance_cents, acc.balance_cents);
    }

    #[test]
    fn cant_deposit_before_opening() {
        let acc = BankAccount::default();
        let err = acc.handle(BankCommand::Deposit { cents: 100 }).unwrap_err();
        assert_eq!(err, BankError::NotOpen);
    }

    #[test]
    fn cant_withdraw_past_balance() {
        let mut acc = BankAccount::default();
        for e in acc.handle(open_command()).unwrap() {
            acc.apply(&e);
        }
        for e in acc.handle(BankCommand::Deposit { cents: 50 }).unwrap() {
            acc.apply(&e);
        }
        let err = acc
            .handle(BankCommand::Withdraw { cents: 200 })
            .unwrap_err();
        assert_eq!(
            err,
            BankError::InsufficientFunds {
                balance: 50,
                attempted: 200
            }
        );
    }

    #[test]
    fn cant_do_anything_after_close() {
        let mut acc = BankAccount::default();
        for e in acc.handle(open_command()).unwrap() {
            acc.apply(&e);
        }
        for e in acc.handle(BankCommand::Close).unwrap() {
            acc.apply(&e);
        }
        assert_eq!(
            acc.handle(BankCommand::Deposit { cents: 10 }).unwrap_err(),
            BankError::Closed
        );
        assert_eq!(
            acc.handle(BankCommand::Withdraw { cents: 10 }).unwrap_err(),
            BankError::Closed
        );
        assert_eq!(
            acc.handle(BankCommand::Close).unwrap_err(),
            BankError::Closed
        );
        assert_eq!(acc.handle(open_command()).unwrap_err(), BankError::Closed);
    }

    #[test]
    fn rejects_non_positive_amounts() {
        let mut acc = BankAccount::default();
        for e in acc.handle(open_command()).unwrap() {
            acc.apply(&e);
        }
        assert_eq!(
            acc.handle(BankCommand::Deposit { cents: 0 }).unwrap_err(),
            BankError::NonPositiveAmount(0)
        );
        assert_eq!(
            acc.handle(BankCommand::Withdraw { cents: -5 }).unwrap_err(),
            BankError::NonPositiveAmount(-5)
        );
    }

    #[test]
    fn empty_stream_replays_to_default_state() {
        let acc = BankAccount::replay(&[]);
        assert_eq!(acc.status, AccountStatus::Uninitialized);
        assert_eq!(acc.balance_cents, 0);
        assert!(acc.owner_id.is_none());
    }

    // ---------- EventStore tests -------------------------------------------

    #[tokio::test]
    async fn append_and_load_round_trip_preserves_version_order() {
        let store = EventStore::in_memory().await.unwrap();
        let events = vec![
            BankEvent::Opened { owner_id: 1 },
            BankEvent::Deposited { cents: 100 },
            BankEvent::Deposited { cents: 50 },
            BankEvent::Withdrew { cents: 30 },
        ];
        store.append("acct-1", 0, &events).await.unwrap();

        let loaded = store.load("acct-1").await.unwrap();
        assert_eq!(loaded.len(), 4);
        let versions: Vec<i64> = loaded.iter().map(|e| e.version).collect();
        assert_eq!(versions, vec![1, 2, 3, 4]);
        // Payload survives the round trip.
        let decoded: Vec<BankEvent> = loaded.iter().map(|e| e.decode().unwrap()).collect();
        assert_eq!(decoded, events);
    }

    #[tokio::test]
    async fn load_aggregate_replays_the_stream() {
        let store = EventStore::in_memory().await.unwrap();
        let events = vec![
            BankEvent::Opened { owner_id: 7 },
            BankEvent::Deposited { cents: 1_000 },
            BankEvent::Withdrew { cents: 250 },
        ];
        store.append("acct-7", 0, &events).await.unwrap();

        let acc: BankAccount = store.load_aggregate("acct-7").await.unwrap();
        assert_eq!(acc.status, AccountStatus::Open);
        assert_eq!(acc.owner_id, Some(7));
        assert_eq!(acc.balance_cents, 750);
    }

    /// Two writers each load the stream at version N, each compute a
    /// different next event, each try to append at expected_version=N.
    /// Exactly one succeeds; the other gets a `ConcurrencyConflict`
    /// with the actual current version.
    #[tokio::test]
    async fn optimistic_concurrency_rejects_the_loser() {
        let store = EventStore::in_memory().await.unwrap();
        store
            .append("acct-c", 0, &[BankEvent::Opened { owner_id: 1 }])
            .await
            .unwrap();
        assert_eq!(store.current_version("acct-c").await.unwrap(), 1);

        // Both writers think the stream is at version 1.
        let a = store.clone();
        let b = store.clone();

        // We don't strictly need concurrency to demonstrate the
        // mechanism — sequential calls with the same expected_version
        // exercise the exact same code path. We still spawn both to
        // confirm the lock works under tokio.
        let h1 = tokio::spawn(async move {
            a.append("acct-c", 1, &[BankEvent::Deposited { cents: 100 }])
                .await
        });
        let h2 = tokio::spawn(async move {
            b.append("acct-c", 1, &[BankEvent::Deposited { cents: 200 }])
                .await
        });

        let r1 = h1.await.unwrap();
        let r2 = h2.await.unwrap();
        let oks = [r1.is_ok(), r2.is_ok()].iter().filter(|x| **x).count();
        let conflicts = [&r1, &r2]
            .iter()
            .filter(|r| matches!(r, Err(EsError::ConcurrencyConflict { .. })))
            .count();
        assert_eq!(oks, 1, "exactly one writer succeeds");
        assert_eq!(conflicts, 1, "the other gets a ConcurrencyConflict");

        if let Some(Err(EsError::ConcurrencyConflict {
            expected, actual, ..
        })) = [r1, r2].into_iter().find(Result::is_err).as_ref()
        {
            assert_eq!(*expected, 1);
            assert_eq!(*actual, 2);
        }

        // Stream is at version 2 with exactly one new event appended.
        assert_eq!(store.current_version("acct-c").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn append_empty_is_a_no_op() {
        let store = EventStore::in_memory().await.unwrap();
        store
            .append::<BankEvent>("acct-empty", 0, &[])
            .await
            .unwrap();
        assert_eq!(store.current_version("acct-empty").await.unwrap(), 0);
        assert!(store.load("acct-empty").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn read_from_returns_global_order() {
        let store = EventStore::in_memory().await.unwrap();
        store
            .append("a", 0, &[BankEvent::Opened { owner_id: 1 }])
            .await
            .unwrap();
        store
            .append("b", 0, &[BankEvent::Opened { owner_id: 2 }])
            .await
            .unwrap();
        store
            .append("a", 1, &[BankEvent::Deposited { cents: 10 }])
            .await
            .unwrap();

        let all = store.read_from(0, 100).await.unwrap();
        assert_eq!(all.len(), 3);
        assert!(all[0].position < all[1].position);
        assert!(all[1].position < all[2].position);
        assert_eq!(all[0].stream_id, "a");
        assert_eq!(all[1].stream_id, "b");
        assert_eq!(all[2].stream_id, "a");
    }

    // ---------- Projection / CQRS test -------------------------------------

    /// Append a sequence of events, then run the projection on a poll
    /// loop. Within a bounded time it must converge on the correct
    /// per-account balance. This is the eventual-consistency story in
    /// 30 lines.
    #[tokio::test]
    async fn balance_projection_eventually_converges() {
        let store = EventStore::in_memory().await.unwrap();
        let projection = BalanceProjection::new(&store).await.unwrap();

        store
            .append(
                "acct-p",
                0,
                &[
                    BankEvent::Opened { owner_id: 1 },
                    BankEvent::Deposited { cents: 1_000 },
                    BankEvent::Deposited { cents: 250 },
                    BankEvent::Withdrew { cents: 600 },
                ],
            )
            .await
            .unwrap();

        // Run the projection on a poll loop with a short interval.
        let proj_handle = tokio::spawn({
            let store = store.clone();
            let projection = projection.clone();
            async move {
                let _ = tokio::time::timeout(
                    Duration::from_millis(500),
                    run_projection(
                        store,
                        projection,
                        Duration::from_millis(10),
                        100,
                        std::future::pending::<()>(),
                    ),
                )
                .await;
            }
        });

        // Poll until convergence (or timeout fires the assert below).
        let deadline = std::time::Instant::now() + Duration::from_millis(400);
        loop {
            if let Some(balance) = projection.balance("acct-p").await.unwrap() {
                if balance == 650 {
                    break;
                }
            }
            assert!(
                std::time::Instant::now() <= deadline,
                "projection did not converge; saw {:?}",
                projection.balance("acct-p").await.unwrap()
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(projection.balance("acct-p").await.unwrap(), Some(650));
        // Checkpoint advanced to the position of the last event.
        assert!(projection.checkpoint().await.unwrap() >= 4);

        proj_handle.abort();
        let _ = proj_handle.await;
    }

    // ---------- proptest: append-batching associativity --------------------

    use proptest::prelude::*;

    // Generate a random sequence of (already-valid) bank events
    // starting with Opened, plus a random batching of those events
    // into chunks. The invariant: the final aggregate state is
    // identical no matter how the events were batched into
    // `append()` calls. This is the "events are associative" property
    // that makes event sourcing safe to retry.
    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 16,
            .. ProptestConfig::default()
        })]
        #[test]
        fn batching_does_not_change_final_state(
            deposits in proptest::collection::vec(1i64..1_000, 1..8),
            chunk_size in 1usize..5
        ) {
            // Build a valid event sequence: open, then a series of deposits.
            let mut events = vec![BankEvent::Opened { owner_id: 1 }];
            for d in &deposits {
                events.push(BankEvent::Deposited { cents: *d });
            }
            let expected_balance: i64 = deposits.iter().sum();

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let store = EventStore::in_memory().await.unwrap();
                let stream_id = "p-test";
                // Append in chunks of `chunk_size`.
                let mut expected_version = 0_i64;
                for chunk in events.chunks(chunk_size) {
                    store.append(stream_id, expected_version, chunk).await.unwrap();
                    expected_version += chunk.len() as i64;
                }
                let acc: BankAccount = store.load_aggregate(stream_id).await.unwrap();
                prop_assert_eq!(acc.balance_cents, expected_balance);
                prop_assert_eq!(acc.status, AccountStatus::Open);
                Ok::<(), TestCaseError>(())
            }).unwrap();
        }
    }
}
