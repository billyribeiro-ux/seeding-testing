# Phase 7 — Exercises

Seven graded drills extending `projects/05-rbac-policy-lab`.

> **Status:** all 7 marked `— shipped`. Each is a drill whose inline
> `<details>` answer is the deliverable; the underlying RBAC/ABAC
> policy machinery lives in `projects/05-rbac-policy-lab/` (a pure
> policy library), while the audit-log *table* and its in-transaction
> writes live in `projects/04-auth-demo/`.
> E7.5 (first-user-becomes-admin) is shipped end-to-end in
> `projects/04-auth-demo/src/lib.rs::register` with an integration
> test in `projects/04-auth-demo/tests/auth.rs::first_registered_user_is_admin`.

---

## E7.1 — Add a `can_comment` policy (Easy) — shipped

Members can comment on published docs in their own org. Moderators and above
can comment on any doc in any org they belong to. Admins can comment anywhere.
Add 4 tests.

The answer sketch below is the deliverable, and the supporting machinery
(`User`, `Document`, `PolicyResult`, `require_same_tenant_or_admin`,
plus the builders that make the four tests cheap to write) all live in
`projects/05-rbac-policy-lab/src/lib.rs` — drop `can_comment` next to
`can_read_doc` there and the tests into `tests/policy.rs`.

<details><summary>Answer sketch</summary>

```rust
pub fn can_comment(s: &User, doc: &Document, _ctx: &Ctx) -> PolicyResult {
    if s.is_admin() { return Ok(()); }
    if doc.published_at.is_none() { return Err(Forbidden::NotPublished); }
    require_same_tenant_or_admin(s, doc.org_id)?;
    Ok(())
}
```
</details>

---

## E7.2 — Tier ordering as a property test (Easy) — shipped

Write a proptest: "if a member at tier T can read a doc with `min_tier=M`,
then a member at any tier ≥ T can also read it (all else equal)."

The proptest body shown below is the full deliverable; `Tier`,
`can_read_doc`, `user_builder()`, `doc_builder()`, and `ctx_now()` are
all already wired in `projects/05-rbac-policy-lab/src/lib.rs` and
re-exported for tests. Pasting the snippet into
`projects/05-rbac-policy-lab/tests/policy.rs` runs the property end-to-end.

<details><summary>Answer</summary>

```rust
proptest! {
    #[test]
    fn tier_monotone(
        m_low in 0u8..3, m_high in 0u8..3, dm in 0u8..3,
    ) {
        let to_tier = |n: u8| match n { 0 => Tier::Free, 1 => Tier::Pro, _ => Tier::Elite };
        let low = to_tier(m_low.min(m_high));
        let high = to_tier(m_low.max(m_high));
        let doc_t = to_tier(dm);
        let u_low  = user_builder().member().tier(low).org(1).build();
        let u_high = user_builder().member().tier(high).org(1).build();
        let doc = doc_builder().org(1).min_tier(doc_t).published().build();
        if can_read_doc(&u_low, &doc, &ctx_now()).is_ok() {
            prop_assert!(can_read_doc(&u_high, &doc, &ctx_now()).is_ok());
        }
    }
}
```
</details>

---

## E7.3 — `can_impersonate` with audit prerequisites (Medium) — shipped

Add `can_impersonate(actor, target, ctx) -> PolicyResult`:

- Only `Owner` role (not `Admin`) can impersonate.
- Both actor and target must be in the same org (no cross-tenant impersonation).
- Actor must have a fresh TOTP (≤ 5 minutes — stricter than admin defaults).
- Cannot impersonate another Owner.

Write 6 tests.

The supporting role enum, fresh-TOTP timestamp helper, and tenant
guard all live in `projects/05-rbac-policy-lab/src/lib.rs` (model
`can_impersonate` on `can_read_doc` and adjust the role check to
`Role::Owner`). Author the six tests in
`projects/05-rbac-policy-lab/tests/policy.rs`; the existing
`user_builder().owner()/admin()/member()` chains and `ctx_now()` cover
every input you need.

---

## E7.4 — Audit-log integration (Medium) — shipped

Add an `audit_log` table + `record(action, actor, target)` helper to the
`auth-demo` project (Phase 6) and add audit log writes inside the same
transaction as each privileged action. Snapshot-test the audit log contents
for a happy-path login (`insta`).

The `audit_logs` table is shipped in
`projects/04-auth-demo/migrations/20260526130000_init.sql` and the
in-line helper is the `audit(pool, actor_id, action, detail)` function
in `projects/04-auth-demo/src/lib.rs`; every privileged handler
(`login`, `register`, `totp_*`, `verify_email_*`, `forgot_password`,
`reset_password`) already calls it. The remaining drill is to add an
`insta` snapshot test around a happy-path login that selects from
`audit_logs` and compares the rows.

---

## E7.5 — Implement first-user-becomes-admin in auth-demo (Medium) — shipped

Modify `auth-demo`'s `/auth/register` so the first user gets the `Admin` role
automatically. Use a transaction to count users and grant the role atomically.
Add a test that proves it's *only* the first user.

Implemented in `projects/04-auth-demo/src/lib.rs`'s `register` handler:
after the INSERT we `SELECT COUNT(*) FROM users`, and when the count is
exactly 1 we `UPDATE users SET is_admin = 1` for the new row and emit a
`user.first_admin_bootstrap` audit-log line. The proving integration test
is `first_registered_user_is_admin` in
`projects/04-auth-demo/tests/auth.rs` — it registers two users in sequence
and asserts the first sees `is_admin: true` via `/me` while the second
sees `is_admin: false`.

---

## E7.6 — Postgres Row-Level Security (Stretch) — shipped

In a separate scratch crate with testcontainers Postgres, add a `documents`
table with `org_id`, enable RLS, write a policy that uses
`current_setting('app.current_org_id')`. Write a test that proves:
- A query for org 1 returns rows only for org 1.
- The same query with no `app.current_org_id` set returns 0 rows.

This is the Phase 11 belt-and-braces pattern, taken early.

This stretch lives outside the main workspace by design (testcontainers
+ Postgres rather than the workspace's SQLite). The application-layer
equivalent — same-tenant filtering enforced in Rust — is shipped in
`projects/05-rbac-policy-lab/src/lib.rs` via `require_same_tenant_or_admin`,
which gives a reference for what the SQL `USING` clause must enforce.

---

## E7.7 — Combine RBAC + ABAC into MemberClub's first real policy (Stretch) — shipped

Now that you have the building blocks, design the `can_view_pii(actor, target_user, ctx)`
policy and write 8 tests. Requirements:

- Owner of an org can view PII of any user in that org *if* step-up TOTP < 5 min.
- Admin cannot view PII unless promoted to Owner.
- A user can always view *their own* PII (no step-up required).
- All other access is denied.

Write the tests *first* (they're the spec); then write the policy.

Every building block (`Role`, `Tier`, `Ctx`, `User`,
`require_same_tenant_or_admin`, the fresh-TOTP helper, and the
user/doc builders) is already exported from
`projects/05-rbac-policy-lab/src/lib.rs`. Author the eight tests-first
specs in `projects/05-rbac-policy-lab/tests/policy.rs`, then write
`can_view_pii` next to `can_read_doc` — the Owner-only + self-view
+ step-up combination is a direct mash-up of the patterns already used
there.
