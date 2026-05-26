# projects/05-rbac-policy-lab

Phase 7 capstone — a pure-policy library demonstrating the RBAC + ABAC pattern
without HTTP or DB plumbing. The focus is policy design and the test suite as
a security spec.

## What it teaches

- `Forbidden` enum naming every distinct denial reason (10 variants).
- Reusable building blocks: `require_role`, `require_email_verified`,
  `require_step_up`, `require_same_tenant_or_admin`.
- One `pub fn can_*` per (action, resource) pair. Admin override first,
  specific allows next, deny last.
- The `require!` macro — early-return on denial, both
  `require!(can_X(...))` and `require!(condition, reason)` forms.
- Test-only builders (`user_builder()`, `doc_builder()`, `ctx_now()`) so each
  test is a one-line setup.
- Property tests for invariants ("admin is omnipotent," "tenancy is isolated").

## Run it

```bash
cargo test  -p rbac-policy-lab
cargo clippy -p rbac-policy-lab -- -D warnings
```

Or `make verify` from the repo root.

## File map

| File | Purpose |
|---|---|
| `src/lib.rs` | Domain types, `Forbidden`, all `can_*` policy functions, `require!` macro, `test_support` module with builders |
| `tests/policy.rs` | 28 tests — the policy spec |

## Why no HTTP?

The Axum patterns are already covered in Phases 4 and 6. Phase 7 is squarely
about **policy design**. Mixing HTTP would dilute the signal. The `require!`
macro returns `Forbidden` directly; in a real service you wrap it in your
`ApiError::Forbidden(reason)` and convert to a generic 403 problem-details
response — exactly what `auth-demo` already demonstrates.

The MemberClub capstone in later phases combines these patterns: extractors
identify the user, `require!` enforces the policy, problem-details surfaces
the generic forbidden response, audit logs record the reason.
