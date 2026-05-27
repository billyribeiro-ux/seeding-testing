# Phase 3 — Exercises

Nine drills across SQL fundamentals, schema design, Drizzle, and sqlx.

---

## E3.1 — Spot the NULL pitfall (Easy) — shipped

Drill exercise: the deliverable is the predicted-then-verified result table in the `<details>` block below. Internalizing `NULL = NULL → NULL`, the difference between `COUNT(*)` and `COUNT(col)`, and that `SUM` ignores NULLs is the whole point — no project code attached.

Predict the result of each, then verify in `psql`:

```sql
SELECT NULL = NULL;
SELECT NULL = 0;
SELECT NULL IS NULL;
SELECT COUNT(*)        FROM (VALUES (1), (NULL), (2)) v(x);
SELECT COUNT(v.x)      FROM (VALUES (1), (NULL), (2)) v(x);
SELECT SUM(v.x)        FROM (VALUES (1), (NULL), (2)) v(x);
```

<details><summary>Answer</summary>

`NULL`, `NULL`, `true`, `3`, `2`, `3`.

`COUNT(*)` counts rows. `COUNT(col)` counts non-NULL values in `col`. `SUM` ignores NULLs.
</details>

---

## E3.2 — Pick the index (Easy) — shipped

Drill exercise: the deliverable is the index-choice answer in the `<details>` block. The lesson — partial indexes win when the predicate is selective, low-cardinality boolean indexes rarely pay rent — is the takeaway, not any code change.

You frequently run:

```sql
SELECT id, email FROM users WHERE created_at > NOW() - INTERVAL '7 days' AND is_email_verified = TRUE;
```

Which index helps most?

a. `CREATE INDEX ON users (created_at);`
b. `CREATE INDEX ON users (is_email_verified);`
c. `CREATE INDEX ON users (created_at) WHERE is_email_verified = TRUE;` (partial)
d. `CREATE INDEX ON users (is_email_verified, created_at);` (composite)

<details><summary>Answer</summary>

**(c)** is best if "verified" is a small fraction of users — smaller index, fewer pages, faster reads, faster writes. **(d)** is a strong second.

(a) is too coarse (you still scan unverified rows). (b) helps almost nothing because most boolean indexes match too many rows.
</details>

---

## E3.3 — Write the upsert (Easy) — shipped

Drill exercise: the deliverable is the `INSERT … ON CONFLICT … DO UPDATE … RETURNING *` statement in the `<details>` block. Same idempotency pattern is reused in Phase 8 for Stripe customer rows.

Write a single statement that ensures a `stripe_customers` row exists for `(user_id = $1, stripe_id = $2)`, updating `updated_at` and `email` if it already exists.

<details><summary>Answer</summary>

```sql
INSERT INTO stripe_customers (user_id, stripe_id, email, updated_at)
VALUES ($1, $2, $3, NOW())
ON CONFLICT (user_id) DO UPDATE
    SET email      = EXCLUDED.email,
        stripe_id  = EXCLUDED.stripe_id,
        updated_at = NOW()
RETURNING *;
```

(Requires a UNIQUE constraint on `user_id`.)
</details>

---

## E3.4 — Lost-update fix (Medium) — shipped

Drill exercise: the deliverable is the three ranked strategies in the `<details>` block — compute-in-SQL (cheapest), `SELECT … FOR UPDATE` row lock (medium), and `SERIALIZABLE` + retry (most general). Money-handling code in Phase 8 leans on strategy 1 by default.

Two clients both want to debit $10 from the same account. Sketch *three* different SQL strategies that prevent the lost-update problem. Rank them by overhead.

<details><summary>Answer</summary>

1. **Compute in SQL** (lowest overhead):
   ```sql
   UPDATE accounts SET balance = balance - 1000 WHERE id = $1 AND balance >= 1000;
   ```
   The `WHERE balance >= 1000` doubles as the "don't overdraft" check.
2. **Row-level lock** (medium):
   ```sql
   BEGIN;
   SELECT balance FROM accounts WHERE id = $1 FOR UPDATE;
   -- compute in code …
   UPDATE accounts SET balance = $new WHERE id = $1;
   COMMIT;
   ```
3. **`SERIALIZABLE` isolation + retry** (highest, most general):
   ```sql
   BEGIN ISOLATION LEVEL SERIALIZABLE;
   SELECT balance FROM accounts WHERE id = $1;
   UPDATE accounts SET balance = $new WHERE id = $1;
   COMMIT;  -- may abort with 40001; retry
   ```
</details>

---

## E3.5 — Add a column to the Drizzle schema (Medium) — shipped

Reference implementation lives in `projects/02b-sqlite-notes-svelte/` — the Drizzle schema, queries, and the SvelteKit `+page.server.ts` load shape are all in place. The `<details>` block below sketches the exact change pattern (nullable `tag` column, conditional `where` clause, `?tag=foo` query-param plumbing) for anyone re-deriving it from scratch.

In `projects/02b-sqlite-notes-svelte/src/lib/server/db/schema.ts`, add a nullable `tag` text column (max 32 chars). Update the queries to:

1. Allow setting a tag when creating a note (form field `tag`).
2. Filter the list by tag if a `?tag=foo` query param is present.

Apply the migration with `pnpm db:push`, update vitest tests to cover the filter.

<details><summary>Answer (sketch)</summary>

```ts
// schema.ts
export const notes = sqliteTable('notes', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  body: text('body').notNull(),
  tag: text('tag'),
  createdAt: text('created_at').notNull().default(sql`(strftime('%Y-%m-%dT%H:%M:%fZ','now'))`)
});

// queries.ts
export async function listNotes(db, tag?: string) {
  let q = db.select().from(notes);
  if (tag) q = q.where(eq(notes.tag, tag));
  return q.orderBy(desc(notes.id)).all();
}

// +page.server.ts load
export const load = async ({ url }) => ({ notes: await listNotes(db, url.searchParams.get('tag') ?? undefined) });
```

Test that filtering by tag returns only matching rows.
</details>

---

## E3.6 — Add `update` to sqlx-notes (Medium) — shipped

Reference implementation: `projects/02c-sqlx-notes/src/lib.rs` exposes `pub async fn update(pool, id, body) -> NotesResult<Note>` (around line 121) that validates the body with the same trim/empty/too-long rules as `add`, runs an `UPDATE … RETURNING` and converts a missing row into `NotesError::NotFound(id)`. The same file also ships `list_keyset(...)` for the keyset-pagination drill referenced from Phase 4.

Add a `pub async fn update(pool, id, body) -> NotesResult<Note>` to `projects/02c-sqlx-notes/src/lib.rs` that:

- Validates body (same rules as `add`).
- Returns `NotFound(id)` if the row doesn't exist.
- Otherwise updates and returns the updated row.

Add two integration tests: update happy path + not-found.

<details><summary>Answer (sketch)</summary>

```rust
pub async fn update(pool: &SqlitePool, id: i64, body: &str) -> NotesResult<Note> {
    let trimmed = body.trim();
    if trimmed.is_empty() { return Err(NotesError::Empty); }
    if trimmed.chars().count() > 4096 { return Err(NotesError::TooLong(trimmed.chars().count())); }

    let row = sqlx::query_as::<_, Note>(
        "UPDATE notes SET body = ? WHERE id = ? RETURNING id, body, created_at",
    )
    .bind(trimmed).bind(id)
    .fetch_optional(pool).await?;
    row.ok_or(NotesError::NotFound(id))
}
```
</details>

---

## E3.7 — Read an `EXPLAIN ANALYZE` (Medium) — shipped

Drill exercise: the deliverable is the diagnosis (Seq Scan over 1M rows + in-memory top-N sort → 1.5s) and the composite-index fix in the `<details>` block. Reading plans is a tool-using-the-tool skill; the answer is the artefact.

Given this output, explain *what's slow* and *how to fix it*:

```
Sort  (cost=37250..37501 rows=100 width=64) (actual time=1542..1545 rows=100 loops=1)
  Sort Key: created_at DESC
  Sort Method: top-N heapsort  Memory: 41kB
  ->  Seq Scan on orders  (cost=0.00..29501 rows=1000000 width=64)
                          (actual time=0.03..1410 rows=1000000 loops=1)
        Filter: (user_id = 42)
        Rows Removed by Filter: 999900
Planning Time: 0.07 ms
Execution Time: 1545 ms
```

<details><summary>Answer</summary>

The planner did a **sequential scan** of 1M rows, filtered down to 100 in memory, then **sorted** for `ORDER BY created_at DESC`.

Fix: a composite index on `(user_id, created_at DESC)`. The planner can then walk the matching `user_id` block in the right order — no seq scan, no sort.

```sql
CREATE INDEX orders_user_id_created_at_idx ON orders (user_id, created_at DESC);
```
</details>

---

## E3.8 — Migration discipline (Medium) — shipped

Drill exercise: the deliverable is the two-step migration sketch in the `<details>` block — first add `deleted_at TIMESTAMPTZ` plus a partial "alive" index, then enforce soft-delete at the application layer (replace `delete()` with `soft_delete()` and filter the list query) rather than revoking DELETE at the DB. The pattern shows up again in Phase 5's seed CLI cleanup paths.

The `notes` table needs a `deleted_at TIMESTAMPTZ` column for soft-delete support. Sketch the migration file. Then sketch the *next* migration that retires hard-delete behavior (i.e. callers should not be able to issue `DELETE FROM notes` directly).

<details><summary>Answer</summary>

```sql
-- migrations/20260601120000_add_deleted_at_to_notes.sql
ALTER TABLE notes ADD COLUMN deleted_at TIMESTAMPTZ;
CREATE INDEX notes_alive_idx ON notes (created_at DESC) WHERE deleted_at IS NULL;
```

For the second migration: rather than blocking DELETE at the DB layer (which would also block your test cleanup), enforce it at the application layer — replace `delete(...)` with `soft_delete(...)` and update the listing query:

```sql
SELECT ... FROM notes WHERE deleted_at IS NULL ORDER BY id DESC;
```

A more aggressive approach is a `REVOKE DELETE ON notes FROM app;` and a `BEFORE DELETE` trigger that raises. Match your needs.
</details>

---

## E3.9 — Idempotency key (Stretch) — shipped

Drill exercise: the deliverable is the schema + `INSERT … ON CONFLICT (idempotency_key) DO NOTHING RETURNING *` pattern in the `<details>` block, with the Rust glue that distinguishes first-write (Some → call Stripe) from replay (None → re-fetch). Phase 8's webhook receiver builds the production version on top of exactly this skeleton.

Write the SQL + Rust glue for an idempotent "create charge" endpoint. Requirements:

- A request includes a client-generated `idempotency_key` (UUID).
- The first call inserts a `charges` row and (in your imagination) calls Stripe.
- Subsequent calls with the same key return the *original* row, no double-charge.
- Two concurrent calls with the same key produce one row.

<details><summary>Answer (sketch)</summary>

Schema:

```sql
CREATE TABLE charges (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    idempotency_key UUID NOT NULL UNIQUE,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    amount_cents    BIGINT NOT NULL CHECK (amount_cents > 0 AND amount_cents < 2_100_000_000_00),
    stripe_id       TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Rust (pseudo-sqlx):

```rust
let inserted = sqlx::query_as::<_, Charge>(
    r#"
    INSERT INTO charges (idempotency_key, user_id, amount_cents)
    VALUES ($1, $2, $3)
    ON CONFLICT (idempotency_key) DO NOTHING
    RETURNING *
    "#,
).bind(key).bind(user_id).bind(amount).fetch_optional(&pool).await?;

let charge = match inserted {
    Some(row) => {
        let stripe_id = stripe.create_charge(row.amount_cents, …).await?;
        sqlx::query!("UPDATE charges SET stripe_id = $1 WHERE id = $2", stripe_id, row.id)
            .execute(&pool).await?;
        Charge { stripe_id: Some(stripe_id), ..row }
    }
    None => sqlx::query_as::<_, Charge>("SELECT * FROM charges WHERE idempotency_key = $1")
        .bind(key).fetch_one(&pool).await?,
};
```

This is the pattern Phase 8 builds out for real Stripe webhooks.
</details>
