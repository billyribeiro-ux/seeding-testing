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

## The "compliance shape" claim, expanded

The introduction says your architecture has a compliance shape whether
you wrote it down or not. What that means in practice:

- **Authentication** is either centralized or scattered. A centralized
  auth subsystem with one place to revoke a credential is the SOC 2,
  GDPR, and HIPAA happy path. Scattered auth — separate password DBs
  per service, ad-hoc API keys in env vars — is the same regulations'
  nightmare.
- **Logging** either captures who-did-what or it doesn't. A repo that
  logs "user updated" without recording the actor is failing the audit
  trail requirement, regardless of whether you've heard of audit trails.
- **Data flow** either has explicit boundaries or it doesn't. If any
  service can read any table, you have no field-level access control,
  no minimum-necessary discipline, and a finding waiting to be made.
- **Secrets** are either rotatable or they aren't. Hardcoded secrets in
  config files are GDPR breach risk and SOC 2 finding fuel.
- **Deletion** is either implemented or it isn't. If you cannot
  enumerate every place a user's data lives, you cannot honor an
  erasure request.

The good news: each of those shapes can be measured against without a
regulator. You can ask "can I revoke a credential in one place?" or
"can I find every column touched by user X?" before any auditor
arrives. The compliance documents in this directory are checklists for
asking those questions in regulation-friendly language.

## What you do NOT find here

- **Marketing claims about being "compliant."** No-one can call
  themselves "GDPR compliant" or "HIPAA compliant" in the abstract.
  You can have a SOC 2 report (an artifact). You can have evidence of
  GDPR compliance practices (artifacts and processes). You cannot have
  a binary "yes."
- **A certificate.** This directory does not produce one. The audit
  firm does, after they review the evidence.
- **Permission to skip privacy/security review.** Documenting a control
  is not the same as having implemented it. Engineers who read these
  documents and conclude "we're fine" without checking the artifacts
  exist are missing the point.

## How to suggest changes to these docs

Compliance documentation gets stale fast. If you spot drift:

- The map ([`04-pii-data-map.md`](./04-pii-data-map.md)) is the most
  volatile. A new column in any migration touches it. Update via the
  same PR as the migration.
- The regulation docs (`01-gdpr.md`, `02-soc2.md`, `03-hipaa.md`) are
  stable until the law changes — but the cross-reference tables at the
  bottom of each go stale as the repo grows. Update them when you add
  a project, runbook, or significant security feature.
- The README (this file) should be updated only when a new file is
  added to this directory or a major restructure happens.

PRs touching these docs should tag the security/privacy owner for
review. The "I just fixed a typo" exception applies, but anything
substantive needs the owner's eyes.

## Reading order

| You are | Read this order |
|---|---|
| New engineer, never seen any of this | README → `04-pii-data-map.md` → the regulation that applies to your customers |
| Preparing for first SOC 2 audit | `02-soc2.md` → walk the evidence table in this README → fill gaps |
| Preparing for GDPR DPA review | `01-gdpr.md` → `04-pii-data-map.md` → controller/processor mapping |
| Considering healthcare partnership | `03-hipaa.md` → gap analysis → discuss with security owner before any commitment |
| Auditor arrived on Monday | All four files plus the cross-reference table |

## What kind of artifact each regulation produces

- **GDPR**: not an artifact per se — a posture maintained continuously,
  evidenced by the privacy notice, DPAs, data map, internal procedures
  for rights requests, and breach-notification readiness.
- **SOC 2**: a written report from a qualified auditor, valid for the
  audit window it covers. Customers ask for the report; you sign an
  NDA, you send the PDF.
- **HIPAA**: not a certification at all. HHS does not issue HIPAA
  certificates. Vendors who claim to be "HIPAA certified" are using
  the term loosely; what they mean is that they have undergone a
  third-party HIPAA audit and have a report. Some accept HITRUST as
  a more formal alternative.

## The "shift left" rhythm

The right time to think about each compliance concern:

| When | Concern |
|---|---|
| Designing a new feature | Threat model. Data-minimization review. Lawful-basis selection if new PII is collected. |
| Writing the migration | PII data map update. Retention policy. |
| Writing the code | Logging plan (what's logged, what's redacted). Audit-log writes for state changes. |
| Code review | Reviewer checks: any new PII? any new external data flow? any new secret? |
| CI | Static scans (secret leakage, dependency vulnerabilities). |
| Pre-launch | Privacy review if PII is new. Subprocessor DPA if a new vendor is added. |
| Post-launch | Monitor the new logs for unexpected PII. Verify retention is running. |

Shifting these activities right of launch (i.e. deferring them) is how
compliance becomes a quarterly fire drill rather than an everyday
rhythm. Aim for left.

## A small confession

This curriculum's MemberClub project is **not** a real compliance
showcase. It is a teaching example for the concepts. Specific gaps
called out explicitly in the docs:

- Crypto-shredding is not implemented (called out in
  [`01-gdpr.md`](./01-gdpr.md)).
- Read-side audit logging is not implemented (called out in
  [`03-hipaa.md`](./03-hipaa.md)).
- Field-level RBAC is not implemented (called out in
  [`03-hipaa.md`](./03-hipaa.md)).
- Some entries in the data map are illustrative rather than
  schema-derived ([`04-pii-data-map.md`](./04-pii-data-map.md)).

The point is the *vocabulary and the shape*. A real product taking
compliance seriously would close each of those gaps before going to
audit.
