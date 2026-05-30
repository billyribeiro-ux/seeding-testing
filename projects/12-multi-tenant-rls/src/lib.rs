//! multi-tenant-rls — the policy layer that builds the SQL for
//! tenant-isolated, row-level-secured Postgres tables.
//!
//! The mechanism is the one from
//! `docs/01-architecture-decisions/0007-postgres-rls-tenant-isolation.md`:
//!
//!   1. Every tenant-scoped table carries a tenant key column.
//!   2. The table has RLS enabled: `ALTER TABLE x ENABLE ROW LEVEL
//!      SECURITY`.
//!   3. A policy filters reads + writes by a session GUC the app sets
//!      at the start of each request transaction.
//!   4. The app role is NOT a Postgres superuser — RLS bypass for
//!      superusers is the whole reason migrations run as a separate
//!      `app_migrator` role.
//!
//! Note on the key type: the MemberClub capstone (and ADR 0007) use an
//! `org_id BIGINT` key with the `app.current_org_id` GUC. This lab uses a
//! `tenant_id UUID` key with `app.tenant_id` on purpose — RLS is entirely
//! key-type-agnostic, and showing both makes that explicit. The six SQL
//! statements are identical; only the column name and the `::uuid` /
//! `::bigint` cast differ.
//!
//! What this crate provides:
//!
//!   * [`TenantId`] — newtype around `uuid::Uuid` so a function that
//!     wants a tenant id can't accidentally receive a user id.
//!   * [`PolicyBuilder`] — composes the migration SQL for "make this
//!     table tenant-isolated" so a migration author writes one line
//!     per table instead of repeating the 6-statement boilerplate.
//!   * [`set_local_tenant_sql`] — the exact `SET LOCAL` statement the
//!     app issues at the top of every request transaction.
//!
//! A real-DB integration test (`tests/rls.rs`, gated on a `DATABASE_URL`
//! and skipped when it is absent so `make verify` stays hermetic) is the
//! curriculum exercise: once Postgres is up, the learner writes it to
//! prove the policy actually blocks cross-tenant reads.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub Uuid);

impl TenantId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TenantId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

/// The exact session-level statement the app issues at the start of
/// each request transaction so the RLS policy can see the tenant.
///
/// We use parameterized `set_config(..., 'true')` rather than `SET
/// LOCAL` because sqlx + connection pools can't always rely on a
/// `SET LOCAL` lasting through the txn — `set_config(..., true)` is
/// the explicit local form.
#[must_use]
pub fn set_local_tenant_sql(t: TenantId) -> String {
    format!("SELECT set_config('app.tenant_id', '{}', true)", t.0)
}

/// Builds the migration body for "make this table tenant-isolated."
#[derive(Debug)]
pub struct PolicyBuilder<'a> {
    table: &'a str,
    column: &'a str,
}

impl<'a> PolicyBuilder<'a> {
    #[must_use]
    pub fn new(table: &'a str) -> Self {
        Self {
            table,
            column: "tenant_id",
        }
    }

    /// Override the tenant column name. Default `tenant_id`.
    #[must_use]
    pub fn column(mut self, name: &'a str) -> Self {
        self.column = name;
        self
    }

    /// Emit the SQL to:
    ///   1. enable row-level security on the table,
    ///   2. drop any pre-existing policy with the same name (idempotent),
    ///   3. add a policy that admits a row iff its `tenant_id` matches
    ///      `current_setting('app.tenant_id')`.
    ///
    /// Callers paste the output into a `sqlx migrate add ...` SQL file.
    #[must_use]
    pub fn build(&self) -> String {
        let mut sql = String::new();
        writeln!(sql, "ALTER TABLE {} ENABLE ROW LEVEL SECURITY;", self.table).unwrap();
        writeln!(
            sql,
            "DROP POLICY IF EXISTS tenant_isolation ON {};",
            self.table
        )
        .unwrap();
        writeln!(sql, "CREATE POLICY tenant_isolation ON {}", self.table).unwrap();
        writeln!(
            sql,
            "  USING ({} = current_setting('app.tenant_id', true)::uuid)",
            self.column
        )
        .unwrap();
        writeln!(
            sql,
            "  WITH CHECK ({} = current_setting('app.tenant_id', true)::uuid);",
            self.column
        )
        .unwrap();
        sql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_local_tenant_sql_is_a_set_config_with_local_true() {
        let t = TenantId(Uuid::nil());
        let sql = set_local_tenant_sql(t);
        assert!(sql.contains("set_config('app.tenant_id'"));
        assert!(sql.contains("'true'") || sql.contains(", true)"));
        assert!(sql.contains(&t.to_string()));
    }

    #[test]
    fn policy_builder_emits_enable_drop_create() {
        let sql = PolicyBuilder::new("documents").build();
        assert!(sql.contains("ALTER TABLE documents ENABLE ROW LEVEL SECURITY"));
        assert!(sql.contains("DROP POLICY IF EXISTS tenant_isolation ON documents"));
        assert!(sql.contains("CREATE POLICY tenant_isolation ON documents"));
        assert!(sql.contains("USING (tenant_id = current_setting('app.tenant_id'"));
        assert!(sql.contains("WITH CHECK (tenant_id = current_setting('app.tenant_id'"));
    }

    #[test]
    fn policy_builder_with_custom_column_name() {
        let sql = PolicyBuilder::new("invoices").column("org_id").build();
        assert!(sql.contains("USING (org_id = current_setting('app.tenant_id'"));
    }

    #[test]
    fn tenant_id_round_trips_through_display_and_parse() {
        let t = TenantId::new();
        let s = t.to_string();
        let back: TenantId = s.parse().unwrap();
        assert_eq!(t, back);
    }
}

#[cfg(test)]
mod prop_tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Every random table name + every random column name must
        /// produce SQL that contains both. We're proving the builder
        /// doesn't accidentally normalize the names away.
        #[test]
        fn builder_includes_table_and_column_in_output(
            table in "[a-z][a-z0-9_]{0,30}",
            column in "[a-z][a-z0-9_]{0,30}",
        ) {
            let sql = PolicyBuilder::new(&table).column(&column).build();
            prop_assert!(sql.contains(&table), "SQL must include the table name: {sql}");
            prop_assert!(sql.contains(&column), "SQL must include the column name: {sql}");
        }
    }
}
