# Phase 7 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **RBAC modeling** | Hard-coded role checks in handlers | Normalized roles/permissions/role_permissions tables | Scoped role grants (multi-tenancy); permission-name conventions; fast lookup with indexes |
| **ABAC patterns** | None — relies only on roles | Ownership, tenancy, time-bounded handled per case | Combines patterns explicitly; uses RLS in DB as belt-and-braces |
| **Policy function shape** | Inline if/else in handlers | Pure `can_X` predicates outside handlers | Reusable building blocks (`require_*`); admin-bypass first; named denial reasons |
| **Deny by default** | Allows on missing match | Returns `Forbidden::Unsupported` on unknown | Default-deny enforced by linter / type system; new resources require explicit policies |
| **`require!` macro** | Long match expressions | Macro used uniformly across handlers | Visible signal in every privileged handler; greppable; auditable |
| **Audit logging** | Not present | Insert per privileged action | Inside the same transaction; append-only; snapshot of the resource; retention policy |
| **Testing** | "Happy path" admin can do everything | Per-policy allow + deny tests | Coverage of every `Forbidden` variant; property tests for invariants; spec-like test names |
| **Admin bootstrap** | "INSERT INTO user_roles" in production | CLI tool + first-user fallback | Last-admin guardrail; audit-logged grants; step-up required for sensitive promotions |
| **Multi-tenancy** | None (single-tenant app) | `org_id` filter in every query | RLS + application-layer checks; explicit cross-tenant privileged endpoints |

## Self-check before moving to Phase 8

- [ ] `make verify` passes locally.
- [ ] You can read `projects/05-rbac-policy-lab/tests/policy.rs` and explain every test's intent.
- [ ] You completed Exercises E7.1 – E7.4.
- [ ] You can articulate "admin bypass first, specific allow next, deny last" without prompting.
- [ ] You can name three reasons to use both RBAC and ABAC instead of just one.
- [ ] CI is green on your branch.

Phase 8 — **Stripe + Money** — sits squarely on top of the policy layer (only authorized actors can charge; only admins can refund). It's also where the curriculum's most rigorous attention to correctness lives — money is unforgiving.
