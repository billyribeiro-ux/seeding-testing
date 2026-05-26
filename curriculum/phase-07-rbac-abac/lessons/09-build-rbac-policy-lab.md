# Lesson 7.9 — Build `rbac-policy-lab` Step by Step

> **The capstone of Phase 7.** Walk through `projects/05-rbac-policy-lab/`.
> **Time:** 45 minutes.

## What's in the project

```
projects/05-rbac-policy-lab/
├── Cargo.toml
├── README.md
└── src/lib.rs            ← domain + policies + macro + test builders
└── tests/policy.rs       ← 31 tests (the policy spec)
```

A pure library — no HTTP, no DB, no Axum. The patterns are universal; the
plumbing was already taught in Phases 4 and 6.

## Why no plumbing?

This is the design decision that makes the project memorable:

- **Phase 4** covered Axum routing and `IntoResponse`.
- **Phase 6** covered the `AuthenticatedUser` extractor.
- **Phase 7** focuses on policy logic.

Mixing the three layers in one project would dilute the lesson. Instead, the
capstone reads like a textbook of policy patterns — 11 predicate functions,
each one small and named for what it does.

## The five reusable building blocks

```rust
pub fn require_role(s: &User, min: Role) -> PolicyResult;
pub fn require_email_verified(s: &User) -> PolicyResult;
pub fn require_step_up(s: &User, ctx: &Ctx, max_age: Duration) -> PolicyResult;
pub fn require_same_tenant_or_admin(s: &User, resource_org_id: OrgId) -> PolicyResult;
```

Plus the macro:

```rust
require!(can_X(...));      // pass-through of a PolicyResult
require!(cond, reason);    // inline assertion
```

That's the entire vocabulary. Every domain policy composes from these.

## The six domain policies

```rust
pub fn can_read_doc(s: &User, doc: &Document, _ctx: &Ctx)  -> PolicyResult { ... }
pub fn can_write_doc(s: &User, doc: &Document, _ctx: &Ctx) -> PolicyResult { ... }
pub fn can_delete_doc(s: &User, doc: &Document, ctx: &Ctx) -> PolicyResult { ... }
pub fn can_publish_doc(s: &User, doc: &Document, _ctx: &Ctx) -> PolicyResult { ... }
pub fn can_grant_role(actor: &User, role: Role, target_org: OrgId, ctx: &Ctx) -> PolicyResult { ... }
pub fn can_revoke_admin(actor: &User, target_org: OrgId, remaining: usize, ctx: &Ctx) -> PolicyResult { ... }
```

Six policies, each ~10 lines. Admin override → specific allow → deny.

## The test file is the security spec

`tests/policy.rs` reads like a permission matrix. Test names alone tell the
story:

```
admin_can_read_any_doc                              ← allow
member_can_read_published_doc_in_own_org_at_their_tier
member_cannot_read_doc_in_other_org                  ← WrongTenant
owner_can_read_their_own_draft                       ← owner-as-implicit-permission
non_owner_cannot_read_unpublished_doc                ← NotPublished
free_member_cannot_read_pro_doc                      ← InsufficientTier
elite_member_can_read_pro_doc

admin_can_write_any_doc
member_cannot_write_others_doc                       ← NotOwner
owner_can_write_own_doc_when_verified
unverified_owner_cannot_write_own_doc                ← EmailNotVerified
moderator_can_write_doc_in_own_org
moderator_cannot_write_doc_in_other_org              ← NotOwner

admin_can_delete_with_fresh_step_up                  ← step-up required
admin_cannot_delete_with_stale_step_up               ← StepUpRequired
owner_can_delete_own_doc
member_cannot_delete_others_doc

moderator_can_publish_in_own_org
member_cannot_publish                                ← NotModerator
unverified_moderator_cannot_publish                  ← EmailNotVerified

admin_can_grant_moderator_in_own_org
admin_cannot_grant_owner_role                        ← only owner can mint owner
owner_can_grant_owner_role
member_cannot_grant_role
admin_cannot_grant_role_in_other_org   (admin-bypass note)
admin_cannot_grant_with_stale_step_up

admin_can_revoke_when_others_remain
admin_cannot_revoke_last_admin                       ← LastAdmin
```

Three property tests catch invariants:
- `admin_can_read_any_published` (admin is omnipotent over published docs).
- `owner_can_write_own_when_verified` (ownership beats tenancy).
- `member_blocked_from_other_org_reads` (tenancy is hard).

## The test builders that make this work

```rust
let admin = user_builder().admin().build();
let m     = user_builder().id(7).member().tier(Tier::Pro).org(1).build();
let stale = user_builder().admin().step_up_stale().build();

let doc   = doc_builder().owner(99).org(2).min_tier(Tier::Elite).draft().build();
```

One chained call. The interesting attributes are in the builder; everything
else gets sane defaults. A test that says nothing about TOTP gets a fresh
TOTP; a test that says nothing about email gets verified.

When a test cares about an attribute, the builder makes it loud. When it
doesn't, the builder makes it invisible. That's the magic.

## How a real service uses this library

```rust
// In an Axum handler somewhere in MemberClub:
async fn delete_doc(
    user: AuthenticatedUser, Path(id): Path<i64>, State(s): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let doc = repo::doc_by_id(&s.pool, id).await?;
    rbac_policy_lab::require!(rbac_policy_lab::can_delete_doc(&user.0.into(), &doc.into(), &ctx_from_request())?);
    /* ... actual delete + audit ... */
}
```

The library returns `Forbidden`; the application wraps it in
`ApiError::Forbidden(reason)` and the existing `IntoResponse` impl emits a
generic `403 application/problem+json`. The reason lives in the audit log,
not the wire body.

## Why this matters

- **Pure-function policies are the highest-leverage way to reason about
  security.** They're trivial to test, trivial to refactor, trivial to
  review.
- **The test file is the spec.** A new engineer reads `tests/policy.rs` and
  knows what the system permits, before reading any handler code.
- **Building blocks compose.** `require_role` + `require_step_up` +
  `require_same_tenant_or_admin` — every policy is a recipe with three to
  five ingredients.

## Green-bar checkpoint

- `cargo test -p rbac-policy-lab` shows 31 passed.
- You can write a new policy from the building blocks without looking at
  existing ones.
- You can read three test names from `tests/policy.rs` and predict the
  outcome of running them.

Phase 7 is complete. Phase 8 — **Stripe + Money** — is the most consequential
phase. We obsess over `i64` cents, the $21B ceiling, idempotency keys, and
webhook reliability.
