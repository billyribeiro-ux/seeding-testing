# MVCC and Isolation Levels

> "How can two transactions see different values for the same row at the same
> wall-clock instant?" — every engineer at some point

The answer is that they aren't reading the same row. They are reading different
*versions* of it. That is MVCC.

## What MVCC actually is

MVCC stands for **Multi-Version Concurrency Control**. In a non-MVCC system
(the textbook locking model), readers block writers and writers block readers.
A `SELECT` would have to wait for an in-flight `UPDATE` on the same row to
commit or abort. This is correct, simple, and catastrophically slow under any
realistic mixed workload.

Postgres (and Oracle, and modern SQL Server with snapshot isolation enabled,
and MySQL with InnoDB) instead does this:

1. Every row stored on disk is a **tuple**. A logical row can have multiple
   physical tuples — one per version that some still-running transaction might
   need to see.
2. Every tuple has two hidden columns: `xmin` (the transaction ID that
   inserted it) and `xmax` (the transaction ID that deleted or updated-and-
   superseded it, or 0 if it is still live).
3. Every transaction gets a **snapshot** at the moment it starts (or at the
   moment of its first statement, depending on isolation level). The snapshot
   is essentially "the set of transaction IDs that had committed at this point
   in time."
4. When the transaction reads a tuple, it consults `xmin` and `xmax` against
   its snapshot:
   - `xmin` must be in the snapshot (the inserting transaction must have
     committed *before* my snapshot was taken).
   - `xmax` must NOT be in the snapshot (the deleter, if any, committed
     *after* my snapshot — so from my point of view the row still exists).
   This is called a **visibility check**.
5. An `UPDATE` does not overwrite the old tuple. It inserts a new tuple with
   the new values and sets the old tuple's `xmax` to the current transaction
   ID. A `DELETE` just sets `xmax`.

The consequence: **reads never block writes, and writes never block reads.**
A `SELECT` running for an hour can coexist with thousands of `UPDATE`s
because each `UPDATE` is appending a new tuple, not overwriting the one the
long `SELECT` is looking at.

## Inspecting xmin/xmax yourself

```sql
SELECT xmin, xmax, * FROM subscriptions WHERE id = 42;
```

These are real columns (system columns; they don't show in `SELECT *` by
default). After an update, you'll see `xmin` jump to a higher number. Open
two `psql` sessions, run `BEGIN` in both, `UPDATE` in one but don't commit,
then `SELECT xmin, xmax` in the other — you can watch MVCC happen.

## Dead tuples and why VACUUM exists

Because `UPDATE` and `DELETE` leave the old tuple in place (with `xmax` set),
the table file grows monotonically until something cleans it up. A tuple is
"dead" once no still-running transaction could possibly need to see it — i.e.,
once every active snapshot's lower bound has moved past its `xmax`.

Dead tuples are not just wasted disk. They are:

- Wasted I/O: every sequential scan has to skip over them.
- Wasted cache: they sit in `shared_buffers` taking up RAM.
- Sources of "the table is 40GB but `SELECT COUNT(*)` returns 200 rows"
  bloat war stories.

`VACUUM` is the housekeeping process that reclaims dead-tuple space and
updates the **visibility map** (a per-page bitmap that lets future scans
skip pages where every tuple is visible to every transaction — this is what
makes index-only scans possible). `VACUUM FULL` rewrites the entire table
and takes an `ACCESS EXCLUSIVE` lock; you almost never want to run it in
production. The autovacuum daemon runs regular `VACUUM` (no `FULL`) based on
thresholds set in `postgresql.conf`. See
[`02-btree-wal-and-vacuum.md`](./02-btree-wal-and-vacuum.md) for the full
mechanism.

## The four SQL standard isolation levels

The SQL standard defines four levels, each named after the strongest anomaly
they *prevent*. Postgres implements three of them with snapshot semantics;
the fourth (`READ UNCOMMITTED`) doesn't exist in Postgres at all (asking for
it silently gives you `READ COMMITTED`).

### The anomalies

- **Dirty read**: T1 reads a row that T2 has modified but not committed.
  If T2 rolls back, T1 has seen a value that never existed.
- **Non-repeatable read**: T1 reads row R, then T2 commits an `UPDATE` to R,
  then T1 reads R again in the same transaction and sees a different value.
- **Phantom read**: T1 runs `SELECT ... WHERE filter`, T2 commits an
  `INSERT` matching `filter`, T1 reruns the same query and sees a row that
  wasn't there before.
- **Write skew** (not in the standard, but very real): T1 and T2 each read a
  set of rows, each decides independently to write based on what it read,
  and both commit. Neither saw the other's write, and the combined result
  violates an invariant. Example below.

### The levels

| Level | Dirty read | Non-repeatable read | Phantom read | Write skew |
|---|---|---|---|---|
| READ UNCOMMITTED | possible (in standard) | possible | possible | possible |
| READ COMMITTED   | prevented | possible | possible | possible |
| REPEATABLE READ  | prevented | prevented | possible (in standard) | possible |
| SERIALIZABLE     | prevented | prevented | prevented | prevented |

Postgres-specific notes:

- `READ COMMITTED` is the default. Each *statement* gets a fresh snapshot.
  Two `SELECT`s in the same transaction can see different data.
- `REPEATABLE READ` in Postgres is actually **snapshot isolation**: one
  snapshot for the entire transaction. This is stronger than the standard's
  `REPEATABLE READ` — Postgres's version already prevents phantoms. It does
  NOT prevent write skew.
- `SERIALIZABLE` uses **SSI** (Serializable Snapshot Isolation). It runs at
  snapshot-isolation speed but tracks read/write dependency graphs at commit
  time and aborts one transaction in any cycle that could violate
  serializability. This is the only level that prevents write skew, and it
  is the only level that gives you the textbook guarantee "the result is as
  if the transactions ran one at a time in some order."

## Postgres defaults

- `default_transaction_isolation = read committed`
- `default_transaction_read_only = off`
- `default_transaction_deferrable = off`

You override per-transaction with:

```sql
BEGIN ISOLATION LEVEL SERIALIZABLE;
-- ...
COMMIT;
```

or per-session with `SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL ...`.

## Worked example: MemberClub `subscriptions`

The MemberClub schema (see [`projects/10-memberclub-cli`](../../projects/10-memberclub-cli/))
has a `subscriptions` table roughly like:

```sql
CREATE TABLE subscriptions (
    id          BIGSERIAL PRIMARY KEY,
    member_id   BIGINT NOT NULL,
    status      TEXT NOT NULL,        -- 'active' | 'paused' | 'canceled'
    plan        TEXT NOT NULL,        -- 'free' | 'pro' | 'team'
    seats_used  INT NOT NULL DEFAULT 1,
    seats_max   INT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Two concurrent transactions:

- **T1** (a billing job): "If this subscription is `active`, charge it for
  next month."
- **T2** (the user's web request): "Cancel my subscription."

Both target subscription `id = 42`, which starts `status = 'active'`.

### Under READ COMMITTED

```
T1: BEGIN;
T1: SELECT status FROM subscriptions WHERE id = 42;  -- sees 'active'
T2:                                                  BEGIN;
T2:                                                  UPDATE subscriptions SET status = 'canceled' WHERE id = 42;
T2:                                                  COMMIT;
T1: -- (T1 now decides to charge)
T1: INSERT INTO charges (subscription_id, amount) VALUES (42, 1999);
T1: COMMIT;
```

T1 charged a canceled subscription. The user is on the phone with support.
This is **not a bug in Postgres** — `READ COMMITTED` makes no promise about
the row not changing between statements. It is a bug in T1's logic. The fix
is either to re-check status in the `INSERT` (`WHERE status = 'active'`) or
to use a higher isolation level.

### Under REPEATABLE READ

Same interleaving:

```
T1: BEGIN ISOLATION LEVEL REPEATABLE READ;
T1: SELECT status FROM subscriptions WHERE id = 42;  -- snapshot taken; 'active'
T2:                                                  UPDATE ...; COMMIT;
T1: INSERT INTO charges ...;
T1: COMMIT;
```

T1 still commits and still creates the bad charge. `REPEATABLE READ` ensures
that if T1 *re-read* row 42 it would still see `'active'` — its snapshot is
frozen. But it does nothing to prevent the *write* of `charges` based on the
stale read. This is write skew, and `REPEATABLE READ` does not prevent it.

If instead T1 had done a writing modification of row 42 itself:

```
T1: UPDATE subscriptions SET seats_used = seats_used + 1 WHERE id = 42 AND status = 'active';
```

then under `REPEATABLE READ` Postgres would detect the conflict (T2 already
updated row 42 in a way that's not in T1's snapshot) and abort T1 with
`ERROR: could not serialize access due to concurrent update` (SQLSTATE 40001).
T1 must retry. This is the classic serialization-failure pattern.

### Under SERIALIZABLE

```
T1: BEGIN ISOLATION LEVEL SERIALIZABLE;
T1: SELECT status FROM subscriptions WHERE id = 42;
T2:                                                  BEGIN ISOLATION LEVEL SERIALIZABLE;
T2:                                                  UPDATE subscriptions SET status = 'canceled' WHERE id = 42;
T2:                                                  COMMIT;
T1: INSERT INTO charges (subscription_id, amount) VALUES (42, 1999);
T1: COMMIT;
-- ERROR: could not serialize access due to read/write dependencies among transactions
```

T1's commit fails. SSI noticed that T1 *read* row 42, T2 *wrote* row 42, and
T1's subsequent write depends on its read. There is no serial order
(T1-then-T2 or T2-then-T1) that produces this outcome. So one must abort.
The application **must be prepared to retry** any `SERIALIZABLE` transaction
that fails with SQLSTATE 40001.

### Trade-offs

- `READ COMMITTED` is the right default. Use `SELECT ... FOR UPDATE` or
  `... WHERE status = 'active'` predicates in writes to prevent the
  read-then-stale-write class of bugs.
- `REPEATABLE READ` is right when you have a long-running report that
  needs a consistent snapshot but doesn't itself need to write conditionally
  on what it read.
- `SERIALIZABLE` is right when correctness matters more than throughput and
  you can implement retry loops. The throughput penalty in modern Postgres
  is much smaller than the lore suggests, but the retry handling is real
  application complexity.

## The `FOR UPDATE SKIP LOCKED` pattern

`projects/08-outbox-demo` runs on SQLite (whose serialized writes make
its `UPDATE ... WHERE id = (SELECT ... LIMIT 1) RETURNING` claim
naturally single-flight), but its docstring and the README call out the
Postgres production form — the one you reach for the moment you want
*parallel* workers
([`projects/08-outbox-demo/src/lib.rs`](../../projects/08-outbox-demo/src/lib.rs)):

```sql
SELECT id, payload
FROM outbox
WHERE status = 'pending'
ORDER BY id
LIMIT 100
FOR UPDATE SKIP LOCKED;
```

This is the manual escape hatch from MVCC. We are saying:

- `FOR UPDATE`: place a row-level lock on every row we return. Subsequent
  attempts by other transactions to update or delete these rows will block.
- `SKIP LOCKED`: if another worker has already taken a `FOR UPDATE` lock on
  a candidate row, skip it instead of waiting.

Net effect: N workers can each pull a disjoint batch of pending outbox
rows with zero coordination beyond what the database itself provides. This
is essentially "we opted into serializability for these specific rows
without paying for it on the rest of the table." It works because each
worker takes its lock, does its work (publish the event, update the row to
`status = 'done'`), and commits — the lock is held only for the duration
of the transaction.

The non-MVCC alternative — `SELECT WHERE status = 'pending' LIMIT 100`
without locking — would have multiple workers grab the same batch and
double-publish. The `READ COMMITTED` default doesn't save you here because
all the workers' `SELECT`s see the same committed state.

This is also exactly the kind of place where, if you turned the whole
worker loop into a `SERIALIZABLE` transaction instead, you would spend
your life writing retry logic. `FOR UPDATE SKIP LOCKED` is the pragmatic
answer for "queue-shaped workloads on a relational database."

## Common operational symptoms tied to MVCC

- **"idle in transaction" sessions are killing my database.** A client
  opened a transaction, ran one query, and went to lunch. As long as that
  transaction is open, Postgres cannot vacuum any tuples newer than its
  snapshot — across the entire database. Bloat grows linearly with how long
  the transaction stays open. Fix: set `idle_in_transaction_session_timeout`.
- **Bloat after a big `UPDATE`.** An `UPDATE foo SET x = x + 1` on a 10M-row
  table creates 10M dead tuples. `VACUUM` will reclaim them, but the table
  file itself doesn't shrink (only `VACUUM FULL` or `pg_repack` does that).
- **`SELECT COUNT(*)` is slow.** Counting requires visiting every live tuple
  because MVCC means there's no maintained row count — different transactions
  would see different counts. Use approximate counts from `pg_stat_user_tables`
  if you don't need exact numbers.

## Further reading

- The visibility-check algorithm is laid out in detail in PostgreSQL's
  `src/backend/access/heap/heapam_visibility.c`. Reading it is unreasonable
  but possible; the Egor Rogov book linked from the README walks through it.
- The classic paper for SSI is Cahill, Röhm, and Fekete, "Serializable
  Isolation for Snapshot Databases" (SIGMOD 2008). The technique used by
  Postgres is a refinement of that.
