# Lesson 3.3 — SQL by Doing

> **Concept first:** SQL is declarative — you tell the DB *what* you want, not *how* to get it. Six statements cover 95% of daily work: SELECT, INSERT, UPDATE, DELETE, JOIN, GROUP BY.
> **Time:** 60 minutes.

## SELECT — read

```sql
-- All columns, all rows
SELECT * FROM users;

-- Specific columns
SELECT id, email FROM users;

-- Filter with WHERE
SELECT id, email FROM users WHERE is_email_verified = TRUE;

-- Sort
SELECT id, email FROM users ORDER BY created_at DESC;

-- Limit
SELECT id, email FROM users ORDER BY created_at DESC LIMIT 20;

-- Paging via OFFSET (avoid past page 100 — use keyset paging instead)
SELECT * FROM users ORDER BY id LIMIT 20 OFFSET 40;
```

**Avoid `SELECT *` in application queries.** It returns columns you don't need (network bytes + memory) and breaks silently if a column is added. Be explicit.

## INSERT — create

```sql
INSERT INTO users (email, password_hash)
VALUES ('alice@example.com', '$argon2id$…');

-- Multiple rows
INSERT INTO users (email, password_hash) VALUES
    ('alice@example.com', '$argon2id$…'),
    ('bob@example.com',   '$argon2id$…');

-- Returning the ID after insert (Postgres / SQLite)
INSERT INTO users (email, password_hash)
VALUES ('alice@example.com', '$argon2id$…')
RETURNING id, public_id, created_at;
```

The `RETURNING` clause is gold — one round-trip, no `SELECT … WHERE id = lastval()`.

## UPDATE — modify

```sql
UPDATE users SET is_email_verified = TRUE WHERE id = 42;

-- ALWAYS include a WHERE. Forgetting it updates every row.
UPDATE users SET email_verified_at = NOW() WHERE id = 42 RETURNING *;
```

A senior habit: **type the `WHERE` clause first**, then go back and write the `SET`. Reduces "I just nuked production" stories.

## DELETE — remove

```sql
DELETE FROM sessions WHERE expires_at < NOW();

-- Soft delete pattern
UPDATE posts SET deleted_at = NOW() WHERE id = 17;
```

Same advice: type `WHERE` first.

## JOIN — combine

```sql
-- Get every order with the customer's email
SELECT o.id, o.amount_cents, u.email
FROM orders o
JOIN users  u ON u.id = o.user_id;

-- LEFT JOIN keeps rows from the left even when no match exists on the right
SELECT u.id, u.email, COUNT(o.id) AS order_count
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
GROUP BY u.id;
```

Four flavours:

- **`INNER JOIN`** (or just `JOIN`) — only rows that match in both tables.
- **`LEFT JOIN`** — every row from the left, even if no match on the right (right columns are `NULL`).
- **`RIGHT JOIN`** — mirror of left. Rarely seen; just swap operands.
- **`FULL OUTER JOIN`** — every row from both sides.

> Mental shortcut: **`INNER`** = intersection, **`LEFT`** = "every left row plus its match if any."

## GROUP BY — aggregate

```sql
SELECT DATE(created_at) AS day, COUNT(*) AS signups
FROM users
WHERE created_at >= NOW() - INTERVAL '30 days'
GROUP BY DATE(created_at)
ORDER BY day;
```

`COUNT`, `SUM`, `AVG`, `MIN`, `MAX` are the everyday aggregators.

## DISTINCT — deduplicate

```sql
SELECT DISTINCT country FROM users;
```

If `DISTINCT` is slow on a big table, you usually want a `GROUP BY` or an index.

## UPSERT — insert-or-update

```sql
INSERT INTO subscriptions (user_id, stripe_id, status)
VALUES (42, 'sub_abc', 'active')
ON CONFLICT (user_id) DO UPDATE
    SET status = EXCLUDED.status,
        updated_at = NOW();
```

The standard pattern for "make sure this row exists with these values." We use it heavily for Stripe webhook handlers.

## CTEs and subqueries — readable queries for hard problems

A CTE (`WITH`) names an intermediate result so the main query stays readable:

```sql
WITH recent_users AS (
    SELECT id FROM users WHERE created_at > NOW() - INTERVAL '7 days'
)
SELECT o.id, o.amount_cents
FROM orders o
JOIN recent_users r ON r.id = o.user_id
WHERE o.amount_cents > 1000;
```

Reaching for a CTE is almost always cleaner than nesting `SELECT … FROM (SELECT …)`.

## Aliases and case

- Column aliases: `AS day` (or just `day`).
- Table aliases: `FROM users u`.
- SQL is case-insensitive for keywords (`SELECT` = `select`) but column/table names are case-sensitive *if* they were created with quotes (`CREATE TABLE "Users"`). Don't do that.

## A worked example: dashboard query

"For the last 30 days, show each day's signups, paid signups, and conversion %."

```sql
WITH days AS (
    SELECT generate_series(CURRENT_DATE - INTERVAL '29 days', CURRENT_DATE, INTERVAL '1 day')::date AS day
),
totals AS (
    SELECT DATE(u.created_at) AS day,
           COUNT(*) AS signups,
           COUNT(*) FILTER (WHERE s.status = 'active') AS paid
    FROM users u
    LEFT JOIN subscriptions s ON s.user_id = u.id
    WHERE u.created_at >= CURRENT_DATE - INTERVAL '29 days'
    GROUP BY DATE(u.created_at)
)
SELECT d.day,
       COALESCE(t.signups, 0) AS signups,
       COALESCE(t.paid, 0)    AS paid,
       CASE WHEN COALESCE(t.signups,0) = 0 THEN 0
            ELSE ROUND(100.0 * t.paid / t.signups, 1) END AS conversion_pct
FROM days d
LEFT JOIN totals t ON t.day = d.day
ORDER BY d.day;
```

That single query — readable thanks to the CTEs — replaces a hundred lines of application code.

## Try it

If you have Postgres running locally, fire up `psql`:

```bash
docker compose up -d db
docker compose exec db psql -U app -d app
```

Inside `psql`:

```
\l           -- list databases
\dt          -- list tables
\d users     -- describe the users table
\?           -- help
```

Type any of the queries above. Don't worry about breaking anything; this is a dev DB.

## Why this matters

- **SQL is the most durable backend skill.** Languages and frameworks come and go; SQL has been the *lingua franca* of data for 50 years and counting.
- **The query you can read in a year is the query you should write today.** Favor CTEs and explicit `AS` aliases.
- **Aggregate queries are how product decisions get made.** Every "what's our conversion rate?" question is a `GROUP BY`.

## Green-bar checkpoint

- You can write a SELECT that joins three tables and filters on a date range.
- You can rewrite a nested subquery as a CTE.
- You can write an UPSERT for "ensure this Stripe customer record exists with the latest data."

Next: `lessons/04-indexes-and-explain.md`.
