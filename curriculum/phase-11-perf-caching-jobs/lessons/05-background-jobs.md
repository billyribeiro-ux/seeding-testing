# Lesson 11.5 — Background Jobs and the Outbox Pattern

> **Concept first:** webhooks need to be 200'd fast; some work is heavy.
> Move heavy work to a background queue. The queue itself is a row in
> your database (the outbox), not an external broker.
> **Time:** 25 minutes.

## Why background jobs

Three reasons:

1. **Webhook acks must be fast.** Stripe gives you 30 seconds. Sending an
   email + writing rows + computing analytics may exceed that.
2. **Crons need a home.** Nightly reconciliation (Phase 8.11), weekly
   churn report, hourly health check.
3. **Heavy on-demand work.** Generating a PDF, computing an embedding,
   re-rendering thumbnails.

## The outbox pattern

Don't write directly to a message broker; write to a *table* in your
database, in the same transaction as the work. A separate process polls
the table and fires the side-effect.

```sql
CREATE TABLE outbox (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    kind            TEXT NOT NULL,                     -- 'email.welcome', 'webhook.stripe', 'metric.signup'
    payload         JSONB NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',   -- 'pending'|'processing'|'done'|'failed'
    attempts        INT NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ
);

CREATE INDEX outbox_due_idx ON outbox (next_attempt_at) WHERE status = 'pending';
```

Webhook handler writes:

```rust
let mut tx = pool.begin().await?;
sqlx::query("INSERT INTO stripe_events (...)").execute(&mut *tx).await?;
sqlx::query("INSERT INTO outbox (kind, payload) VALUES ('email.upgrade', $1)")
    .bind(serde_json::to_value(&payload)?).execute(&mut *tx).await?;
tx.commit().await?;
return Ok(StatusCode::OK);   // Stripe gets 200 < 100ms later
```

Both rows commit together. If either fails, neither happens.

A worker:

```rust
loop {
    let row: Option<OutboxRow> = sqlx::query_as::<_, OutboxRow>(
        "UPDATE outbox SET status = 'processing', attempts = attempts + 1
         WHERE id = (
             SELECT id FROM outbox
             WHERE status = 'pending' AND next_attempt_at <= NOW()
             ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1
         )
         RETURNING *"
    ).fetch_optional(&pool).await?;

    let Some(row) = row else {
        tokio::time::sleep(Duration::from_secs(1)).await;
        continue;
    };

    match dispatch(&row).await {
        Ok(()) => {
            sqlx::query("UPDATE outbox SET status = 'done', completed_at = NOW() WHERE id = $1")
                .bind(row.id).execute(&pool).await?;
        }
        Err(e) => {
            let backoff = Duration::from_secs(2u64.pow(row.attempts.min(8) as u32));
            sqlx::query(
                "UPDATE outbox SET status = 'pending', next_attempt_at = NOW() + $2::interval
                 WHERE id = $1"
            ).bind(row.id).bind(format!("{}s", backoff.as_secs())).execute(&pool).await?;
            tracing::warn!(?row.id, ?e, "outbox dispatch failed; retrying");
        }
    }
}
```

Two critical SQL idioms:

- **`FOR UPDATE SKIP LOCKED`** — many workers can poll concurrently; each
  one grabs a unique row.
- **Exponential backoff via `next_attempt_at`** — failed jobs retry with
  increasing delay.

## Why "outbox" not "broker first"

You *could* push directly to RabbitMQ / SQS / Kafka. But:

- **Transaction-safety.** With outbox, the side effect either happens or
  doesn't — atomic with the business write. With a broker, you can write
  to the DB *and* fail to publish (or vice versa).
- **No new infra.** Postgres is already there.
- **Replay-friendly.** A failed job is a row; reset `status='pending'` and
  it tries again.

When you outgrow this (>1000 jobs/sec sustained), graduate to a broker.
Most apps never get there.

## Cron-style jobs

Three ways:

| Tool | When |
|---|---|
| **`tokio-cron-scheduler`** | In-process; one node only. Fine for low-stakes daily/hourly jobs. |
| **`apalis`** | Full job framework with retries, scheduling, monitoring. Worth it past a few dozen jobs. |
| **External cron (systemd timer / K8s CronJob)** | The most reliable; runs a one-shot binary. Use for nightly reconciliation. |

For MemberClub:
- Nightly reconciliation → external cron (K8s CronJob).
- Hourly subscription-status sync from Stripe → outbox + scheduled
  enqueue.
- "Welcome email after signup" → outbox (immediate after signup).

## At-least-once vs exactly-once

The outbox gives you **at-least-once**. If a worker crashes after the
side effect but before `UPDATE outbox SET status='done'`, the row will
retry. The dispatch must be *idempotent*.

For emails: include a unique message id; the SMTP provider dedupes.
For Stripe API writes: use idempotency keys (Phase 8).
For internal database writes: `ON CONFLICT DO NOTHING` + uniqueness on
the relevant key.

**Exactly-once across distributed systems is impossible**; aim for
idempotent at-least-once.

## Why this matters

- **The outbox makes async work safe.** Both the business write and the
  job-row write commit atomically.
- **`FOR UPDATE SKIP LOCKED` is the Postgres queue primitive.** It's
  built in; use it.
- **Postgres scales further than you think for queues.** Don't pre-adopt
  Kafka.

## Green-bar checkpoint

- You can sketch the outbox table.
- You can write the worker's `UPDATE ... RETURNING` with
  `FOR UPDATE SKIP LOCKED`.
- You can articulate why the outbox row is written *inside* the business
  transaction.

Next: `lessons/06-multi-tenancy.md`.
