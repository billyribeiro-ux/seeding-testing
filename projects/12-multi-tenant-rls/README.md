# 12-multi-tenant-rls

Phase 11 lab — multi-tenant Postgres with row-level security.

The pattern (documented in ADR
`docs/01-architecture-decisions/0007-postgres-row-level-security.md`):

  1. Every tenant-scoped table has `tenant_id UUID NOT NULL`.
  2. `ALTER TABLE x ENABLE ROW LEVEL SECURITY;`
  3. A policy on the table filters rows by a session GUC:
     `current_setting('app.tenant_id')::uuid`.
  4. The app calls `SELECT set_config('app.tenant_id', $1, true)` at
     the top of each request transaction.
  5. The app's Postgres role is NOT a superuser — superusers bypass
     RLS, so migrations run as a separate `app_migrator` role.

## What this crate provides

  * `TenantId` — newtype around `uuid::Uuid` so a function expecting
    a tenant id can't accidentally receive (say) a user id.
  * `PolicyBuilder` — emits the migration SQL for "make this table
    tenant-isolated." Saves a migration author from re-typing the
    same six statements per table.
  * `set_local_tenant_sql(t)` — the exact `set_config(...)` statement
    the app's per-request middleware issues.

## Tests (5)

`cargo test -p multi-tenant-rls`:

  - `set_local_tenant_sql_is_a_set_config_with_local_true` — shape
    + content.
  - `policy_builder_emits_enable_drop_create` — the three-statement
    body (ENABLE, DROP IF EXISTS, CREATE POLICY).
  - `policy_builder_with_custom_column_name` — supports tables that
    use a different column (e.g. `org_id`).
  - `tenant_id_round_trips_through_display_and_parse` — newtype
    plumbing.
  - `builder_includes_table_and_column_in_output` — proptest over
    every legal SQL identifier shape: the output ALWAYS includes the
    requested table + column names verbatim.

A real-DB integration test that proves RLS actually blocks cross-
tenant reads is gated on `$DATABASE_URL` and lives in `tests/rls.rs`
(stub for now — the curriculum guides the learner through writing it
once Postgres is up).
