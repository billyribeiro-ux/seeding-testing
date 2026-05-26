# Lesson 7.7 — The Audit Log: Append-Only, Queryable, Inside the Txn

> **Concept first:** an audit log is a single append-only table that records *every* privileged action. Three properties: append-only, structured, transactional with the state change. Get all three or you don't have an audit log.
> **Time:** 20 minutes.

## What it answers

- "Who deleted that document?"
- "When was Alice promoted to admin? By whom?"
- "Did anyone impersonate user 42 in the last 30 days?"
- "Show every action by user 17 across the last quarter."

If the answer is "we have to grep nginx logs and hope," you don't have an audit log.

## The shape

```sql
CREATE TABLE audit_logs (
    id          BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_id    BIGINT      REFERENCES users(id),                -- NULL = system / cron / seed
    action      TEXT        NOT NULL,                            -- 'doc.deleted', 'role.granted'
    target      JSONB       NOT NULL,                            -- the resource snapshot
    metadata    JSONB       NOT NULL DEFAULT '{}'::jsonb,        -- request_id, ip, ua, etc.
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX audit_logs_actor_id_created_idx ON audit_logs (actor_id, created_at DESC);
CREATE INDEX audit_logs_action_created_idx ON audit_logs (action, created_at DESC);
```

Three columns do the work: `actor_id` (who), `action` (what), `target` (on what — a JSONB snapshot).

`metadata` carries optional extras: request id, IP, user-agent, source (web/CLI/API), affected_count, etc. Use it for forensics.

## Three rules

### 1. Append-only

You never `UPDATE` or `DELETE` from `audit_logs`. To enforce it:

```sql
REVOKE UPDATE, DELETE ON audit_logs FROM app;
GRANT INSERT, SELECT ON audit_logs TO app;
```

A trigger that raises on `UPDATE`/`DELETE` works too. The app role *cannot* rewrite history.

### 2. Inside the same transaction

This is the rule juniors miss:

```rust
let mut tx = pool.begin().await?;
sqlx::query("DELETE FROM documents WHERE id = ?").bind(id).execute(&mut *tx).await?;
audit(&mut tx, Some(actor.id), "doc.deleted", json!({"doc_id": id})).await?;
tx.commit().await?;
```

The audit row and the state change are *atomic*. If the audit insert fails, the delete rolls back. If the delete fails, the audit row is never written. **No phantom audits, no untracked changes.**

### 3. Snapshot the state, don't reference it

Store the *current* state of the resource in `target`, not a reference to it:

```rust
audit(
    &mut tx, Some(actor.id), "doc.deleted",
    json!({
        "doc_id": doc.id,
        "title": doc.title,
        "owner_id": doc.owner_id,
        "deleted_size_bytes": doc.size,
    }),
).await?;
```

Six months later when someone asks "what was in that document?", the audit row remembers. If you stored only `doc_id`, the document is gone and so is the answer.

This is a *deliberate denormalization*. Audit rows are append-only and never need to be consistent with the live data — they need to remember what *was*.

## What to audit

Always:

- **AuthN events:** login success/failure, logout, password change, 2FA enroll/disable.
- **AuthZ changes:** role grants/revokes, permission changes.
- **Privileged data access:** PII viewing, impersonation start/end.
- **Destructive operations:** deletes, "purge user," "wipe org."
- **Billing changes:** subscription upgrades/cancels, refunds.

Never:

- Ordinary reads (GET /v1/notes) — too noisy. Use access logs for those.
- Background-job ticks — those go in a different log.

## Querying the audit log

Common queries you'll want to be fast:

```sql
-- "What did Alice do this week?"
SELECT created_at, action, target FROM audit_logs
WHERE actor_id = $1 AND created_at >= NOW() - INTERVAL '7 days'
ORDER BY created_at DESC LIMIT 100;

-- "Who deleted document 42?"
SELECT actor_id, created_at, metadata FROM audit_logs
WHERE action = 'doc.deleted' AND target->>'doc_id' = '42'
ORDER BY created_at DESC LIMIT 5;

-- "Every privileged action by service accounts (actor_id IS NULL) today"
SELECT action, COUNT(*) FROM audit_logs
WHERE actor_id IS NULL AND created_at >= DATE_TRUNC('day', NOW())
GROUP BY action ORDER BY 2 DESC;
```

The two indexes (`(actor_id, created_at DESC)` and `(action, created_at DESC)`) make these instant.

## Retention

Audit logs grow forever. Plan retention up front:

- **Hot retention** (in Postgres): 90 days.
- **Cold retention** (in object storage as compressed JSON): 7 years (typical compliance horizon).

A nightly job dumps rows older than 90 days to S3 and `DELETE`s them from Postgres. (Yes, the *system* can delete — `app` cannot.) The exported files are immutable, archived, and have integrity checksums.

## Audit search UI

An admin dashboard with a search box pointed at the audit log is one of the most useful things you can build. Every incident retro asks the same question — "who did what, when?" — and the audit log answers it before you've finished pouring coffee.

## Why this matters

- **Audit logs are how you survive compliance audits, security incidents, and your own legitimate forgetfulness.**
- **Inside-the-txn writes prevent phantom records and untracked changes.**
- **Snapshotting the state** makes a six-month-old row still useful.

## Green-bar checkpoint

- You can sketch the `audit_logs` table from memory.
- You can articulate the three rules (append-only, in-txn, snapshot).
- You can name two queries an admin will run against this table.

Next: `lessons/08-test-policies-as-spec.md`.
