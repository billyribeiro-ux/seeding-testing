# Lesson 11.6 — Multi-tenancy and Row-Level Security

> **Concept first:** if your product has organizations, you have
> multi-tenancy. Get tenant isolation wrong and you become a news story.
> Two layers of defense; both required.
> **Time:** 20 minutes.

## The three patterns

| Pattern | Pros | Cons |
|---|---|---|
| **Shared schema, tenant-key column** (here: `org_id`) | Simple; cheap; works at any scale | Easy to forget the filter |
| **Schema-per-tenant** | DB-level isolation; per-tenant backups | Migration headache; doesn't scale past ~1000 tenants |
| **DB-per-tenant** | Strongest isolation; per-tenant DRP | Operationally expensive; usually only for enterprise tier |

90% of SaaS uses **shared schema**. We use shared schema with **two layers
of defense**:

1. Application-layer policy check (Phase 7).
2. DB-layer Row-Level Security (RLS).

## Row-Level Security

Postgres lets you attach policies to a table. The DB *physically refuses*
to return rows outside the policy.

```sql
ALTER TABLE documents ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_iso_select ON documents
    FOR SELECT
    USING (org_id = current_setting('app.current_org_id')::bigint);

CREATE POLICY tenant_iso_modify ON documents
    FOR ALL
    USING (org_id = current_setting('app.current_org_id')::bigint)
    WITH CHECK (org_id = current_setting('app.current_org_id')::bigint);
```

Two policies — one for `SELECT`, one for inserts/updates/deletes. The
`USING` clause filters what rows are visible; `WITH CHECK` enforces what
inserts are allowed.

## Setting the context

The application sets the context at the start of every request, before
any query runs:

```rust
let mut tx = pool.begin().await?;
sqlx::query("SELECT set_config('app.current_org_id', $1, true)")
    .bind(org_id.to_string())
    .execute(&mut *tx).await?;
// ... every query on THIS transaction is now tenant-isolated ...
tx.commit().await?;
```

The third arg `true` makes it **transaction-local** — which is why the
`set_config` and the tenant-scoped queries must share one transaction:
the setting is gone the moment that transaction ends. That is exactly
what you want with a connection pool — a connection handed back can't
leak a stale `current_org_id` into the next request.

## The escape hatch — `BYPASSRLS`

Some operations (cron jobs, admin tools) need to read across tenants. The
DB role for those bypasses RLS:

```sql
CREATE ROLE memberclub_admin BYPASSRLS;
GRANT ALL ON ALL TABLES IN SCHEMA public TO memberclub_admin;
```

The application uses `memberclub_app` (RLS-enforced) for user-facing
requests, and `memberclub_admin` (no RLS) for internal jobs.

## When RLS is overkill

For an app with a single tenant per process / database / shard, RLS
adds complexity for no gain. Skip it.

For an app with shared schema, every customer in one DB, *enable it*.
The cost is one migration; the upside is a structural defense against
the worst SaaS bug.

## Application-layer checks still matter

RLS is the floor, not the ceiling. The application layer still checks:

- **Ownership** (Phase 7 — `can_edit_doc` for owner-vs-not).
- **Tier gating** (Pro can read, Free can't).
- **Time-bounded permissions** (step-up TOTP).

RLS only enforces "you can only see rows of *your* org." All other
permissions live in Phase 7's policy functions.

## Cross-tenant operations — auditing them

The few admin endpoints that *deliberately* read across tenants must:

1. Use the `memberclub_admin` role.
2. Be gated by `can_X` policies (requiring step-up TOTP, etc.).
3. Be **audit-logged** with the cross-tenant detail.

This is the "user impersonation" pattern — the privilege exists but
every use is recorded.

## Multi-tenant data lifecycle

When a tenant signs up:

```
1. Insert a row in `organizations`.
2. Insert the founding user with `org_id = $new`.
3. Optionally: seed welcome data (`INSERT INTO documents (...)`).
```

When a tenant churns:

```
1. Soft-delete: `UPDATE organizations SET deleted_at = NOW()`.
2. Disable: prevent the founding user from logging in until reactivated.
3. After retention window: hard-delete (cascade).
```

GDPR: hard-delete on user request, even if the tenant is paying.

## Why this matters

- **Multi-tenant data leakage is the worst SaaS news story.** RLS
  prevents the entire category of "I missed the WHERE clause" bugs.
- **Two layers of defense compound.** App + DB; either alone is too
  fragile.
- **Admin escape hatches must be auditable.** Privileged access is fine;
  *unobservable* privileged access is not.

## Green-bar checkpoint

- You can write a Postgres RLS policy for a `documents` table.
- You can articulate why we set `app.current_org_id` *transaction-local*.
- You can name three rules for cross-tenant admin endpoints.

Next: `lessons/07-load-testing.md`.
