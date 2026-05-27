# Compliance

> Your architecture has a compliance shape whether or not you wrote it down.

This directory exists for two kinds of readers:

1. The engineer who just got told their startup is selling to an EU customer
   and they need to "be GDPR compliant by next quarter," who has never seen
   the regulation and doesn't know where it lives in their codebase.
2. The engineer about to enter an SOC 2 audit who has been told to gather
   "evidence" and doesn't know what that means in practice.

The thesis of this directory: **compliance is not a layer you bolt on at
the end.** It is a vocabulary for talking about properties your system
already has — or doesn't have. Once you can name the property
("authentication is centralized," "PII is encrypted at rest," "every
admin action is logged with the actor's identity"), you can decide if
you have it, and then either point to where you have it or build it.

The MemberClub schema and the projects in [`projects/`](../../projects/)
are the running example. Many of the controls auditors will ask about
are already in this repo — the docs below say where.

## Files

- [`01-gdpr.md`](./01-gdpr.md) — General Data Protection Regulation (EU).
  Lawful basis, data minimization, rights of access/erasure/portability,
  breach notification. How each maps to the MemberClub schema and the
  patterns used in [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/).
  The audit-log-vs-erasure tension and crypto-shredding.
- [`02-soc2.md`](./02-soc2.md) — SOC 2 (US, generic SaaS). Type I vs
  Type II, the five Trust Service Criteria, the evidence-collection
  workflow, what an auditor will ask for and where to point them in
  this repository.
- [`03-hipaa.md`](./03-hipaa.md) — HIPAA (US, healthcare). When this
  applies (you handle PHI), what it requires, and the specific gaps
  MemberClub would need to close to be HIPAA-compliant.
- [`04-pii-data-map.md`](./04-pii-data-map.md) — the foundational
  document GDPR and SOC 2 auditors both ask for: a table of every
  column containing PII, with classification, lawful basis, retention,
  and propagation paths. Includes the worked MemberClub example.

## How to use this directory

If you've never thought about compliance:
- Read `04-pii-data-map.md` first. The act of building the data map will
  surface most of the things the regulations care about.
- Then read whichever regulation applies to your customer base
  (`01-gdpr.md` if EU users, `02-soc2.md` if enterprise SaaS customers,
  `03-hipaa.md` if any healthcare data).

If you've been here before:
- The cross-reference table at the bottom of each file points to the
  concrete code, schema, and runbook locations that satisfy each control.
- Use these docs as audit-prep checklists: walk down the table, confirm
  each row, gather screenshots / git links / log samples.

## What this directory is NOT

- Not legal advice. Compliance ultimately involves lawyers and qualified
  auditors; these docs are an engineer's working understanding, not a
  certification path.
- Not exhaustive. PCI-DSS, FedRAMP, ISO 27001, CCPA, state-level US
  privacy laws (TX, CA, CO, VA, ...) all exist and all have engineering
  consequences. The four areas covered here are the most common starting
  points for an early-stage SaaS.
- Not a substitute for working with the security and legal functions of
  your company. They exist for a reason; bring them in early.

## Cross-reference: where compliance touches the rest of this repo

| Compliance concern | Lives in |
|---|---|
| Authentication & RBAC | [`projects/04-auth-demo`](../../projects/04-auth-demo/), [`projects/05-rbac-policy-lab`](../../projects/05-rbac-policy-lab/) |
| Row-level isolation | [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/) |
| Audit log | MemberClub `audit_log` table — see [`projects/10-memberclub-cli`](../../projects/10-memberclub-cli/) |
| Transport security | [`docs/security/04-transport-headers.md`](../security/04-transport-headers.md) |
| Threat modeling | [`docs/security/01-stride-threat-model.md`](../security/01-stride-threat-model.md) |
| Supply chain | [`docs/security/03-supply-chain.md`](../security/03-supply-chain.md) |
| Change management | GitHub PR template + CI gates |
| Incident response | [`docs/runbooks/`](../runbooks/), [`docs/03-postmortems/`](../03-postmortems/) |
| Disaster recovery (RPO/RTO) | [`docs/runbooks/disaster-recovery.md`](../runbooks/disaster-recovery.md) |
