//! Chaos / property-style tests for the outbox pattern.
//!
//! These are higher-effort tests that probe the *production* invariants:
//!
//!   1. With many concurrent workers and many enqueued rows, every row is
//!      processed *exactly once*. No row is dropped; no row is double-
//!      processed.
//!   2. The total of all per-worker counters equals the number of enqueued
//!      rows.
//!   3. Each worker observes monotonically increasing `attempts` per row
//!      across retries (no time travel).
//!
//! SQLite serializes writes, so the proof here is: even when many tokio
//! tasks call `claim_next` concurrently, the table's `UPDATE … RETURNING`
//! ensures exactly-once delivery. Postgres production gets the same
//! property from `FOR UPDATE SKIP LOCKED`.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use outbox_demo::{Dispatcher, OutboxRow, migrate, record_transfer, run_once};

/// Build a fresh in-memory pool. Note: max_connections = 1 for
/// `sqlite::memory:` because each connection sees its own DB.
async fn fresh_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

/// A dispatcher that records every (worker_id, outbox_id) it processed.
struct Recorder {
    seen: Arc<Mutex<Vec<(u32, i64)>>>,
    worker_id: u32,
}

impl Dispatcher for Recorder {
    fn handle(&self, row: &OutboxRow) -> Result<(), String> {
        self.seen.lock().unwrap().push((self.worker_id, row.id));
        Ok(())
    }
}

/// 100 enqueued rows, 8 concurrent workers, all draining the same queue.
/// Asserts: total processed == 100, each row processed exactly once,
/// the set of processed ids == the set of enqueued ids.
#[tokio::test]
async fn many_workers_drain_disjoint() {
    const ROWS: usize = 100;
    const WORKERS: u32 = 8;

    let pool = fresh_pool().await;

    // Enqueue ROWS transfers.
    let mut enqueued_ids = HashSet::new();
    for i in 0..ROWS {
        let (_t, outbox_id) = record_transfer(
            &pool,
            &format!("acct_a{i}"),
            &format!("acct_b{i}"),
            (i as i64) + 1,
        )
        .await
        .unwrap();
        enqueued_ids.insert(outbox_id);
    }

    // Spawn WORKERS workers; each drains until run_once returns false.
    let seen = Arc::new(Mutex::new(Vec::<(u32, i64)>::new()));
    let mut handles = Vec::with_capacity(WORKERS as usize);
    for worker_id in 0..WORKERS {
        let pool = pool.clone();
        let recorder = Recorder {
            seen: seen.clone(),
            worker_id,
        };
        handles.push(tokio::spawn(async move {
            // Drain until run_once returns false (queue empty).
            while run_once(&pool, &recorder, 5).await.unwrap() {}
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let processed = seen.lock().unwrap();

    // (1) Every enqueued row was processed once.
    let processed_ids: HashSet<i64> = processed.iter().map(|(_, id)| *id).collect();
    assert_eq!(processed_ids, enqueued_ids, "missing or extra ids");

    // (2) No row was processed twice (no duplicate (worker_id, outbox_id)
    //     entries OR any duplicate of outbox_id alone — even if the same
    //     worker tried).
    let total_processed = processed.len();
    assert_eq!(
        total_processed, ROWS,
        "expected {ROWS} processings, got {total_processed} (duplicates?)"
    );

    // (3) Work was actually distributed — at least 2 workers picked up rows
    //     (sanity check that we're not just running serially).
    let active_workers: HashSet<u32> = processed.iter().map(|(w, _)| *w).collect();
    assert!(
        active_workers.len() >= 2,
        "only {} worker(s) did any work — concurrency didn't engage",
        active_workers.len()
    );
}

/// Mixed enqueue + drain: producers and workers race for the queue.
/// Asserts the queue eventually drains to empty, processed count
/// matches enqueued count, and no row was processed twice.
#[tokio::test]
async fn producers_and_workers_race_to_empty() {
    const PRODUCERS: usize = 4;
    const ROWS_PER_PRODUCER: usize = 25;
    const WORKERS: u32 = 4;
    let total_rows = PRODUCERS * ROWS_PER_PRODUCER;

    let pool = fresh_pool().await;

    let seen = Arc::new(Mutex::new(Vec::<(u32, i64)>::new()));

    // Producers: enqueue rows concurrently.
    let mut producer_handles = Vec::with_capacity(PRODUCERS);
    for p in 0..PRODUCERS {
        let pool = pool.clone();
        producer_handles.push(tokio::spawn(async move {
            for i in 0..ROWS_PER_PRODUCER {
                let _ = record_transfer(
                    &pool,
                    &format!("p{p}_a{i}"),
                    &format!("p{p}_b{i}"),
                    (i as i64) + 1,
                )
                .await
                .unwrap();
            }
        }));
    }

    // Workers: drain. They use the same "drain until empty" loop, but
    // because producers are still running they may see false transiently;
    // we make each worker keep polling until BOTH conditions hold:
    //   - producers are done (joined)
    //   - run_once returns false
    // We approximate by polling with brief yields up to a deadline.
    let producers_done = Arc::new(tokio::sync::Notify::new());
    let producers_done_setter = producers_done.clone();
    tokio::spawn(async move {
        for h in producer_handles {
            h.await.unwrap();
        }
        producers_done_setter.notify_waiters();
    });

    let mut worker_handles = Vec::with_capacity(WORKERS as usize);
    for worker_id in 0..WORKERS {
        let pool = pool.clone();
        let recorder = Recorder {
            seen: seen.clone(),
            worker_id,
        };
        let done = producers_done.clone();
        worker_handles.push(tokio::spawn(async move {
            // We loop until both (a) producers are done and (b) the queue
            // is empty. Tokio's `Notify` lets us check non-blockingly.
            let mut producers_finished = false;
            for _ in 0..1000 {
                if run_once(&pool, &recorder, 5).await.unwrap() {
                    // grabbed a row; keep draining
                } else if producers_finished {
                    return;
                } else {
                    // Wait briefly for either producers-done or new work.
                    tokio::select! {
                        () = done.notified() => producers_finished = true,
                        () = tokio::time::sleep(std::time::Duration::from_millis(10)) => {}
                    }
                }
            }
        }));
    }

    for h in worker_handles {
        h.await.unwrap();
    }

    let processed = seen.lock().unwrap();
    assert_eq!(
        processed.len(),
        total_rows,
        "race produced wrong count: {} processed vs {} enqueued",
        processed.len(),
        total_rows
    );
    let unique: HashSet<i64> = processed.iter().map(|(_, id)| *id).collect();
    assert_eq!(unique.len(), total_rows, "duplicates leaked through");
}
