# ADR 0005 — Explicit policy functions, not Casbin

- Status: Accepted
- Date: 2026-05-26
- Deciders: api-team, security-team
- Tags: authz, rbac, abac

## Context and Problem Statement

MemberClub combines RBAC (role-based) and ABAC (attribute-based) decisions:
"a moderator can hide a post in their own org if they're email-verified
and TOTP-step-up was within 15 minutes." We need a way to express,
review, and test these policies.

## Decision Drivers

- Auditable: a security review must read the policies in one place.
- Testable: every policy gets per-branch tests (Phase 7.8).
- Type-safe: don't allow comparing a `UserId` to an `OrgId` by accident.
- Familiar: most engineers can debug Rust faster than a Casbin DSL.
- No fancy runtime requirements.

## Considered Options

1. **Casbin** (general-purpose policy engine) — separate DSL, casbin
   models + policy CSV. Powerful, lots of moving parts.
2. **Open Policy Agent (OPA)** — externalize policies entirely. Network
   call per check. Massive for our scope.
3. **Plain Rust functions** (`pub fn can_X(...) -> Result<(),
   Forbidden>`). Compiler-checked, testable, familiar.

## Decision Outcome

Chose **option 3**.

- All policies live in one module per service (`policy.rs`).
- One function per (action, resource) pair. Admin override first,
  specific allow next, deny last.
- Reusable predicates: `require_role`, `require_email_verified`,
  `require_step_up`, `require_same_tenant_or_admin`.
- A `require!` macro for early-return on denial in handlers.
- A typed `Forbidden` enum names every denial reason; the wire response
  is a generic 403 problem-details with the reason in audit logs only.

## Consequences

- **Positive:** policies are normal Rust — type-safe, testable, fast.
  The test suite *is* the security spec.
- **Negative:** non-engineers can't author policies without engineering
  involvement. Casbin would in theory let a security analyst write
  rules.
- **Mitigations:** for our scale (< 50 policy functions), engineering
  involvement is fine. If the policy count ever exceeds ~100 or non-
  engineers need to author rules, revisit.

## Notes

We re-evaluate at 50 policy functions or when a security team is
established. If we adopt OPA/Casbin later, the migration path is to
delegate from `can_X` functions to the engine, keeping the macro
surface identical.

Related: Phase 7 lessons; project `05-rbac-policy-lab`.
