# HIPAA

The Health Insurance Portability and Accountability Act of 1996, as
amended by HITECH (2009) and the Omnibus Rule (2013), governs the
handling of **PHI — Protected Health Information** in the United States.

If MemberClub does not handle PHI, HIPAA does not apply. If MemberClub
ever handles PHI — for example by offering a "wellness" subscription
that tracks health metrics, or by integrating with a healthcare provider
as a payment platform — HIPAA suddenly applies in full force.

This file describes (a) what triggers HIPAA, (b) what the regulation
requires, and (c) the specific gaps MemberClub would have to close to
be compliant. **MemberClub is NOT HIPAA-compliant today.** It does not
need to be. The point of this document is to make the gap explicit so
that if the question ever comes up, the answer can be informed.

## What counts as PHI

PHI is any information that:

1. Is created, received, maintained, or transmitted by a **covered
   entity** or **business associate**, AND
2. Relates to the past, present, or future physical or mental health
   of an individual, the provision of healthcare to an individual, or
   payment for such care, AND
3. Identifies the individual or can reasonably be used to identify
   them.

The third clause is broad. The 18 HIPAA Safe Harbor identifiers
include names, geographic subdivisions smaller than a state, dates more
granular than year, phone numbers, email addresses, SSNs, medical
record numbers, account numbers, biometric identifiers, full-face
photographs, IP addresses, device identifiers, and "any other unique
identifying number, characteristic, or code." De-identification means
removing all 18 (or applying an "expert determination" methodology with
documented analysis).

Examples in a MemberClub-like context:

- "User 4242's heart rate is 72" — PHI if the user is identifiable
  (which they are, by `user_id`).
- "An anonymous user (we don't know who) has a heart rate of 72" — not
  PHI if the de-identification is solid.
- "User 4242 paid $30 for their gym membership" — not PHI on its face
  (gym membership isn't healthcare). But if the gym is part of a
  prescribed physical-therapy program, the same fact becomes payment
  information for healthcare and is PHI.

The line is fuzzy. Err on the side of treating ambiguous data as PHI.

## Covered entity vs business associate

- **Covered entity (CE)**: health plans, healthcare clearinghouses, and
  healthcare providers who transmit health information electronically
  for transactions HIPAA governs.
- **Business associate (BA)**: a person or entity that performs
  functions on behalf of a CE involving PHI. Cloud hosting providers,
  email senders, analytics platforms, anything that touches PHI on the
  CE's behalf.

MemberClub today is neither — it doesn't handle PHI. If MemberClub
partnered with a healthcare provider to bill on their behalf, it would
become a BA and need a **Business Associate Agreement (BAA)** with the
provider.

A BA's subprocessors also become BAs ("subcontractor BAAs"). AWS, GCP,
Azure, Stripe, Datadog, and most modern SaaS vendors will sign BAAs on
request, but you have to request and execute them — they don't apply
by default.

## What HIPAA requires (the Security Rule)

The Security Rule organizes safeguards into three categories.

### Administrative safeguards

- **Security management process**: formal risk analysis, risk
  management plan, sanction policy, information system activity review.
- **Workforce security**: authorization/supervision, workforce
  clearance procedures, termination procedures. (Mostly the same hires
  and offboarding hygiene SOC 2 requires.)
- **Information access management**: access authorization, access
  establishment and modification. Minimum-necessary principle: each
  workforce member has access to only the PHI required for their role.
- **Security awareness and training**: HIPAA-specific training,
  annual. Logged.
- **Security incident procedures**: similar to GDPR's breach
  procedures but with different timelines.
- **Contingency plan**: data backup plan, disaster recovery plan,
  emergency mode operation plan, testing and revision procedures,
  applications and data criticality analysis. The DR runbook
  ([`../runbooks/disaster-recovery.md`](../runbooks/disaster-recovery.md))
  is the primary artifact.
- **Evaluation**: periodic technical and non-technical evaluation
  showing the safeguards continue to meet the rule.

### Physical safeguards

- **Facility access controls**: locks on the data center doors.
  Mostly handled by your cloud provider's own SOC 2 / ISO controls
  (which is why the BAA matters).
- **Workstation use and security**: lock screens, encrypted laptops,
  full-disk encryption.
- **Device and media controls**: how PHI on portable media is
  protected, disposed of, reused.

### Technical safeguards

This is where most of the engineering work concentrates.

- **Access control**: unique user IDs, emergency access procedures,
  automatic logoff, encryption and decryption.
- **Audit controls**: hardware, software, and procedural mechanisms
  that record and examine activity in systems containing PHI. The
  `audit_log` table again — but expanded to record every *read* of
  PHI, not just every write.
- **Integrity**: PHI must not be improperly altered or destroyed.
  Cryptographic checksums, version history.
- **Person or entity authentication**: verify identity of accessor.
  MFA, SSO.
- **Transmission security**: PHI must be encrypted in transit.
  TLS 1.2+, no plaintext PHI in URLs/query strings (because those end
  up in access logs).

### Encryption: "addressable" vs "required"

HIPAA's Security Rule has a quirk: most safeguards are labeled either
"required" or "addressable." "Addressable" sounds optional but isn't —
it means "if you don't do this, document why an alternative is at
least as protective." Encryption of PHI at rest and in transit is
"addressable" by the letter of the rule. In practice, **the only
defensible interpretation is to encrypt everything**, because the
"alternative" you'd have to document would be implausible.

For breach notification, the Safe Harbor is encryption: if PHI is
encrypted using a NIST-approved algorithm and the keys are not also
compromised, the loss is not a reportable breach. This is a massive
practical difference. Encrypt everything.

## Breach notification

HIPAA defines a breach as "acquisition, access, use, or disclosure of
PHI in a manner not permitted under the Privacy Rule that compromises
the security or privacy of the PHI."

Timelines:

- Individuals: notify "without unreasonable delay," no later than 60
  calendar days from discovery.
- HHS: report annually if fewer than 500 individuals; immediately
  (within 60 days) if 500 or more.
- Media: if a breach affects more than 500 residents of a state or
  jurisdiction, notify prominent media outlets in that area.

60 days is generous compared to GDPR's 72 hours. But the operational
process is the same: detect quickly, decide quickly, document the
decision, notify per the matrix.

## What MemberClub would have to add to be HIPAA-compliant

Assume MemberClub adds a "wellness" feature that stores user-reported
exercise minutes, body-weight, blood-pressure, and step counts. This
is PHI. The gaps:

### Encryption at rest, everywhere

Today MemberClub relies on cloud-provider-managed disk encryption
("EBS encrypted volumes" or equivalent). That's necessary but not
sufficient. We would need:

- Per-tenant or per-field application-level encryption for PHI columns,
  with keys held in a KMS.
- The crypto-shredding pattern described in
  [`01-gdpr.md`](./01-gdpr.md) — same mechanism, different driver.
- Encrypted backups (already done by AWS RDS); explicitly verified.

**Gap:** MemberClub currently relies only on disk-level encryption.
PHI fields would need application-level encryption.

### PHI-aware audit logging

Today the `audit_log` table records state-changing actions. Under
HIPAA we'd also need to log every **read** of PHI: which user accessed
whose PHI, when, from where.

This is operationally expensive — a "list users" page might generate
hundreds of audit-log rows per call. Most implementations log at the
"event of access" level (the admin opened user 4242's profile page)
rather than "every byte of PHI read."

**Gap:** MemberClub does not log reads of any data today.

### Minimum necessary in code

Every API endpoint that returns PHI must return only the minimum
necessary for the requesting role. Today MemberClub returns whole
user objects from `GET /users/:id`. Under HIPAA, the support agent
calling that endpoint should see only the fields they need.

**Gap:** No field-level access controls. RBAC is endpoint-granular.

### BAAs with subprocessors

Today MemberClub's subprocessor list includes Stripe, AWS, Datadog,
SendGrid. To handle PHI, each would need to sign a BAA. Most will;
some (Datadog only on certain plans, SendGrid only via specific products)
require contract upgrades.

**Gap:** No BAAs in place. Some subprocessors are not HIPAA-eligible
on their current plans.

### Designated security and privacy officers

HIPAA requires named officers. Today MemberClub has informal owners
but no documented role assignments at this level.

**Gap:** Formal role designation required.

### Workforce HIPAA training

Required annually, logged. Most companies use a vendor (KnowBe4,
HushHush) for this.

**Gap:** No HIPAA-specific training program.

### Risk analysis, documented

A formal, periodic risk analysis specifically targeting PHI
confidentiality, integrity, and availability.
[`docs/security/01-stride-threat-model.md`](../security/01-stride-threat-model.md)
is the structural foundation but would need a HIPAA-targeted appendix.

**Gap:** Threat model exists but not HIPAA-scoped.

### Sanction policy

Documented disciplinary actions for workforce members who violate
PHI handling policy.

**Gap:** No documented sanction policy.

## The "we're not in healthcare" misconception

Some founders assume "we're a SaaS company, HIPAA doesn't apply to us."
This is correct *until* you onboard a single healthcare customer. The
moment a hospital signs up and stores PHI in your system, you are a
business associate. Saying "we didn't know" is not a defense.

Practical safeguard: have terms of service that prohibit storing PHI
in MemberClub. Enforce this with a content filter that pattern-matches
obvious PHI in input fields. Make sales aware that healthcare leads
need a sales-engineering conversation before any commitment.

## When HIPAA and GDPR collide

If you handle PHI of an EU resident, both regulations apply. The
intersection is operationally manageable:

- Both demand encryption.
- Both demand breach notification (GDPR's 72-hour clock is stricter;
  the HIPAA timeline is the long pole *after* you've met the GDPR one).
- GDPR's right of erasure conflicts with HIPAA's record-retention
  rules — providers must retain medical records for years. The
  retention obligation generally takes precedence via the erasure
  exception in GDPR Article 17(3)(b) (processing necessary for
  compliance with a legal obligation), but document the decision.
- Cross-border transfer: PHI of an EU resident cannot be transferred
  to the US without GDPR-grade safeguards (SCCs, adequacy). HIPAA
  doesn't speak to cross-border at all.

## Recap

| Concern | HIPAA status for MemberClub today |
|---|---|
| Does the regulation currently apply? | No (no PHI). |
| Would it apply if we added health features? | Yes. |
| Encryption at rest | Partial (disk-level only); would need application-level for PHI. |
| Encryption in transit | Yes (TLS everywhere; see [`docs/security/04-transport-headers.md`](../security/04-transport-headers.md)). |
| Audit logging | Writes yes, reads no. |
| Access control | Endpoint-granular; would need field-granular for PHI. |
| BAAs with subprocessors | None executed. |
| Workforce training | General security training; no HIPAA module. |
| Breach response | General runbooks exist; no HIPAA-specific timeline matrix. |
| Designated security/privacy officers | Informal owners only. |

The gap is meaningful but not insurmountable. The point is: don't
discover the gap by signing a healthcare customer and *then* finding
out.
