# Lesson 3.5 — Transactions and Isolation

> **Concept first:** a transaction bundles multiple statements into an atomic unit. Either every change is applied, or none of them is. Isolation levels decide what concurrent transactions can see of each other.
> **Time:** 30 minutes.

## ACID — the four-letter acronym you have to know

- **Atomicity** — all or nothing.
- **Consistency** — every transaction takes the DB from one valid state to another (constraints honored).
- **Isolation** — concurrent transactions don't trip over each other (see below).
- **Durability** — once committed, the change survives a power loss.

Postgres gives you all four out of the box. Many newer "scalable" databases trade one or more away. Read their fine print before adopting.

## The mechanics

```sql
BEGIN;                                  -- start
INSERT INTO orders   (user_id, amount_cents) VALUES (1, 1000);
INSERT INTO payments (order_id, status, amount_cents) VALUES (LASTVAL(), 'pending', 1000);
COMMIT;                                 -- both rows visible to others, or…
ROLLBACK;                               -- neither row exists, ever
```

Three rules:

1. **Every statement is implicitly its own transaction** if you don't start one. Most clients also auto-commit.
2. **Errors inside an explicit `BEGIN` block force a `ROLLBACK`** — Postgres marks the transaction as "aborted" and refuses further statements until you `ROLLBACK`.
3. **Keep transactions short.** Long-running transactions hold locks and block other writes. Open it, do the work, commit.

## In sqlx code

```rust
let mut tx = pool.begin().await?;          // start
sqlx::query!("INSERT INTO orders ...").execute(&mut *tx).await?;
sqlx::query!("INSERT INTO payments ...").execute(&mut *tx).await?;
tx.commit().await?;                        // commit
// If the function returns Err before commit, Drop runs ROLLBACK for you.
```

The `&mut *tx` pattern reborrows the transaction; sqlx requires this syntax to fit the borrow checker.

## Isolation levels — what concurrent transactions see

Postgres supports four:

| Level | What can happen | Default? |
|---|---|---|
| **Read uncommitted** | "Dirty reads": you can see another transaction's uncommitted rows. *Postgres never actually does this — it's treated as Read Committed.* | — |
| **Read committed** | You only see committed rows. But the same query inside one transaction may return different rows at different times (non-repeatable read). | ✅ Postgres default |
| **Repeatable read** | A snapshot is taken at the start of the transaction; every read sees that snapshot. Concurrent writes may cause a *serialization failure* you must retry. | — |
| **Serializable** | As if all transactions ran one at a time. Strongest. May abort more often; retry on failure. | — |

Two practical guidelines:

- **`READ COMMITTED` (the default) is fine for 95% of business logic.** It's the trade-off most apps want.
- **Switch to `SERIALIZABLE` when you have business logic that *requires* "no concurrent transaction interfered."** Money transfers, inventory reservations, anything where a race condition is observable.

```sql
BEGIN ISOLATION LEVEL SERIALIZABLE;
-- ... statements that must be atomic w.r.t. concurrency ...
COMMIT;
```

In sqlx, set the isolation level on the `BEGIN`:

```rust
sqlx::query("BEGIN ISOLATION LEVEL SERIALIZABLE").execute(&pool).await?;
```

(Helper extensions exist; we'll see them in Phase 4.)

## The lost-update problem

Two clients both want to add 100 to a balance currently at 1000:

```
client A:  read balance → 1000
client B:  read balance → 1000
client A:  write balance = 1000 + 100 = 1100
client B:  write balance = 1000 + 100 = 1100      -- ☠️ should be 1200
```

Three fixes, ordered by strength:

1. **Compute in SQL:**
   ```sql
   UPDATE accounts SET balance = balance + 100 WHERE id = $1;
   ```
   The DB does the add atomically. Always prefer this.

2. **Row-level lock:**
   ```sql
   SELECT balance FROM accounts WHERE id = $1 FOR UPDATE;
   -- ...do work in client...
   UPDATE accounts SET balance = $2 WHERE id = $1;
   ```
   `FOR UPDATE` holds an exclusive lock until commit. The other client blocks.

3. **`SERIALIZABLE` isolation** plus retry on conflict. Heaviest.

## Advisory locks — application-level critical sections

When you need a mutex *across processes*, Postgres advisory locks are the cleanest tool:

```sql
SELECT pg_advisory_xact_lock(hashtext('cron-job:nightly-recon'));
-- ... do work ...
-- automatically released on COMMIT or ROLLBACK
```

`pg_advisory_xact_lock` blocks until the lock is yours, then releases at end-of-transaction. Use it for: "only one worker should run this cron at a time."

## Idempotency keys — when retries are unavoidable

When the client retries a request (network blip), the second call must not double-charge. The pattern:

```sql
INSERT INTO charges (idempotency_key, amount_cents, user_id, stripe_id)
VALUES ($1, $2, $3, $4)
ON CONFLICT (idempotency_key) DO NOTHING
RETURNING *;
```

If `RETURNING *` produces a row: you inserted, charge Stripe. If it produces zero rows: someone else already inserted; do nothing. We use this pattern *everywhere* in Phase 8.

## A worked example: transfer money between accounts

```sql
BEGIN ISOLATION LEVEL SERIALIZABLE;

UPDATE accounts SET balance = balance - 1000 WHERE id = $from;
UPDATE accounts SET balance = balance + 1000 WHERE id = $to;

INSERT INTO ledger_entries (from_id, to_id, amount_cents, idempotency_key)
VALUES ($from, $to, 1000, $key)
ON CONFLICT (idempotency_key) DO NOTHING;

-- A check constraint or trigger could also enforce balance >= 0 to catch overdrafts.
COMMIT;
```

If two concurrent transfers race, `SERIALIZABLE` aborts one and we retry. The math is *guaranteed* to be consistent.

## Why this matters

- **A transaction is the unit of "and nothing bad happened in between."** It's how you reason about correctness in the face of concurrency.
- **Most "weird production bug" stories are race conditions inside a missing transaction.**
- **Idempotency keys + `ON CONFLICT DO NOTHING` are how you survive at-least-once delivery.** This is the foundation of webhook reliability (Phase 8) and message-queue consumers (Phase 11).

## Green-bar checkpoint

- You can wrap two `INSERT`s in a `BEGIN`/`COMMIT` in raw SQL.
- You can pick between `UPDATE ... SET balance = balance + ?`, `FOR UPDATE`, and `SERIALIZABLE` for a given problem.
- You can sketch an idempotency-key insert in SQL.

Next: `lessons/06-stepA-drizzle-sqlite-sveltekit.md`.
