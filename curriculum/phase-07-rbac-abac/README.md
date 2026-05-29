# Phase 7 — RBAC + ABAC: Authorization Without Tears

> **Audience:** you finished Phase 6. You know *who* the user is.
> **Outcome:** you decide *what they can do* — by role, by attribute, by ownership — with an explicit, testable, auditable policy layer.
> **Time:** 2 weeks.

## The mental model

> *AuthN answers "who are you?". AuthZ answers "can you do this?"*.

Two complementary techniques:

- **RBAC (Role-Based Access Control)** — coarse-grained. "Admins can delete; moderators can hide; members can post." A small fixed set of roles maps to a fixed set of permissions.
- **ABAC (Attribute-Based Access Control)** — fine-grained. "A user can edit their own post; they can read posts whose `tier_required <= their_tier`; they can't edit posts older than 24 h."

Real systems use *both*. RBAC for the broad strokes; ABAC for the contextual edges.

## The decision rule

Every privileged action passes through a single function:

```rust
fn check(subject: &User, action: Action, resource: &Resource, ctx: &Ctx) -> Result<(), ForbiddenReason>;
```

Four inputs: who, what, on what, in what context. One output: ok or a specific reason. We compile policies into one file you can read top-to-bottom.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-mental-model.md` | RBAC vs ABAC, when to use each |
| `lessons/02-rbac-schema.md` | `roles`, `permissions`, `role_permissions`, `user_roles` |
| `lessons/03-abac-policies.md` | Pure functions of `(subject, action, resource, ctx)` |
| `lessons/04-policy-trait-and-macro.md` | The `Policy` trait, `require!(...)`, deny-by-default |
| `lessons/05-ownership-and-tenancy.md` | "Owner can," "tenancy must match," time-bounded perms |
| `lessons/06-admin-bootstrap.md` | First-user-becomes-admin, grant via CLI |
| `lessons/07-audit-log.md` | Append-only, queryable, written in the same txn |
| `lessons/08-test-policies-as-spec.md` | Every policy has a passing test that *is* its contract |
| `lessons/09-build-rbac-policy-lab.md` | Capstone walkthrough |

## The capstone — `projects/05-rbac-policy-lab`

A **pure policy library** (no HTTP, no DB, no Axum) for a "Documents" domain. The
plumbing — routing, extractors, the audit-log *table* — was already taught in
Phases 4 and 6; Phase 7 isolates the policy logic so it reads like a textbook of
patterns:

- **Roles:** `member`, `moderator`, `admin`, `owner`.
- **Resources:** documents carry an `owner_id`, an `org_id`, a `min_tier`, and a publish state.
- **Ownership policy:** a member can read/write their own docs; a moderator can write/publish within their org; an admin can do anything (subject to step-up on deletes/grants).
- **Tier policy:** docs have a `min_tier` (`free`, `pro`, `elite`); readers must meet or exceed it.
- **Tenancy + step-up:** cross-tenant access is denied unless admin; sensitive admin actions require a fresh TOTP check.
- **Tests:** every policy has at least one allow test and one deny test per distinct branch — the test file *is* the spec.

The decision functions return `Forbidden` directly; a real service wraps that in
its own `ApiError` at the handler boundary (exactly the shape `auth-demo` already
demonstrates). No SQLite — the library is pure logic, tested as plain Rust plus a
few property tests.

## Green-bar checkpoint

```bash
cargo test  -p rbac-policy-lab     # all green
cargo clippy -p rbac-policy-lab -- -D warnings
```

…and `make verify` is green.

## What's next

Phase 8 — **Stripe + Money**. The most consequential phase. Money in `i64` cents, the $21B ceiling, idempotency keys, webhook reliability, subscription lifecycle.
