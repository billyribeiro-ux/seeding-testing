# ADR 0007 — Postgres Row-Level Security as belt-and-braces tenant isolation

- Status: Accepted
- Date: 2026-05-26
- Deciders: api-team, security-team
- Tags: security, multi-tenancy, postgres

## Context and Problem Statement

MemberClub is a multi-tenant SaaS. Cross-tenant data leakage is the
worst-case scenario — both reputationally and legally (GDPR Article 32).
A missing `WHERE org_id = $1` is one line of code, and humans miss it.

## Decision Drivers

- Single-line `WHERE` clauses cannot be the *only* defense.
- Tests can prove application correctness *today* but not for code
  changes that haven't been written.
- A DB-level enforcement adds a structural guarantee.
- Performance impact must be measurable and acceptable.

## Considered Options

1. **Application-layer checks only.** Cheap; one missed query == leak.
2. **Postgres Row-Level Security policies** + application checks.
   Defense in depth.
3. **Schema-per-tenant.** Strong isolation; operationally heavy; doesn't
   scale past ~1000 tenants.
4. **Database-per-tenant.** Strongest isolation; only for Enterprise tier.

## Decision Outcome

Chose **option 2** (RLS + app checks) for MemberClub's shared-schema
tenancy.

- Every tenant-scoped table gets `ALTER TABLE … ENABLE ROW LEVEL SECURITY`.
- A `USING / WITH CHECK` policy filters by
  `current_setting('app.current_org_id')`.
- Application middleware sets `SET LOCAL app.current_org_id = $1` at
  the start of every transaction.
- A separate DB role (`memberclub_admin BYPASSRLS`) is used by cron
  jobs and admin tools, and every use is audit-logged.
- Application-layer policy checks (Phase 7) remain in place. RLS is
  *additional*, not replacement.

## Consequences

- **Positive:** the DB *physically refuses* to return rows from other
  tenants. A bug in handler code is contained; the leak doesn't ship.
- **Negative:** ~5% query overhead. New `EXPLAIN` plans include the
  RLS predicate.
- **Mitigations:** measure before/after with the Phase 11.7 perf
  methodology. The bypass role is for *audited* cross-tenant work only.

## Notes

For Enterprise customers requiring isolation beyond row-level, we
graduate to per-DB. Future ADR will document that path when it's needed.

Related: Phase 11.6; PLAYBOOK "Constraints belong in the DB."
