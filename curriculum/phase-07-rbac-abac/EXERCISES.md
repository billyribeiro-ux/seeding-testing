# Phase 7 — Exercises

Seven graded drills extending `projects/05-rbac-policy-lab`.

---

## E7.1 — Add a `can_comment` policy (Easy)

Members can comment on published docs in their own org. Moderators and above
can comment on any doc in any org they belong to. Admins can comment anywhere.
Add 4 tests.

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

## E7.2 — Tier ordering as a property test (Easy)

Write a proptest: "if a member at tier T can read a doc with `min_tier=M`,
then a member at any tier ≥ T can also read it (all else equal)."

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

## E7.3 — `can_impersonate` with audit prerequisites (Medium)

Add `can_impersonate(actor, target, ctx) -> PolicyResult`:

- Only `Owner` role (not `Admin`) can impersonate.
- Both actor and target must be in the same org (no cross-tenant impersonation).
- Actor must have a fresh TOTP (≤ 5 minutes — stricter than admin defaults).
- Cannot impersonate another Owner.

Write 6 tests.

---

## E7.4 — Audit-log integration (Medium)

Add an `audit_log` table + `record(action, actor, target)` helper to the
`auth-demo` project (Phase 6) and add audit log writes inside the same
transaction as each privileged action. Snapshot-test the audit log contents
for a happy-path login (`insta`).

---

## E7.5 — Implement first-user-becomes-admin in auth-demo (Medium)

Modify `auth-demo`'s `/auth/register` so the first user gets the `Admin` role
automatically. Use a transaction to count users and grant the role atomically.
Add a test that proves it's *only* the first user.

---

## E7.6 — Postgres Row-Level Security (Stretch)

In a separate scratch crate with testcontainers Postgres, add a `documents`
table with `org_id`, enable RLS, write a policy that uses
`current_setting('app.current_org_id')`. Write a test that proves:
- A query for org 1 returns rows only for org 1.
- The same query with no `app.current_org_id` set returns 0 rows.

This is the Phase 11 belt-and-braces pattern, taken early.

---

## E7.7 — Combine RBAC + ABAC into MemberClub's first real policy (Stretch)

Now that you have the building blocks, design the `can_view_pii(actor, target_user, ctx)`
policy and write 8 tests. Requirements:

- Owner of an org can view PII of any user in that org *if* step-up TOTP < 5 min.
- Admin cannot view PII unless promoted to Owner.
- A user can always view *their own* PII (no step-up required).
- All other access is denied.

Write the tests *first* (they're the spec); then write the policy.
