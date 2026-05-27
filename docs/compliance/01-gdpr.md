# GDPR

The General Data Protection Regulation (EU 2016/679) applies to any
organization that processes personal data of EU residents, regardless of
where the organization is. If MemberClub sells to even one user in the
EU — and we do — the regulation applies to MemberClub.

This file is the engineer's working subset. It is not a legal opinion.

## What the regulation actually requires

The text of GDPR is ~88 pages. The engineering-relevant obligations boil
down to:

### Lawful basis

Every act of processing personal data must rest on at least one of six
lawful bases, named in Article 6:

1. **Consent** — the data subject said yes, freely, specifically, and
   informedly, with a clear affirmative act. Pre-ticked boxes don't count.
2. **Contract** — the processing is necessary to fulfill a contract with
   the data subject (e.g. shipping the user the product they bought).
3. **Legal obligation** — you have to keep this data because some other
   law says so (e.g. tax records).
4. **Vital interests** — life-or-death emergency.
5. **Public task** — applies to government, mostly.
6. **Legitimate interests** — your operations require it and they don't
   override the user's rights. This is the catch-all and the one that
   gets you sued. Document the balancing test.

For MemberClub:

- `users.email`, `users.password_hash`: **contract** (we can't deliver
  the service without knowing who you are).
- `users.marketing_opt_in`, `users.newsletter`: **consent**, recorded
  with timestamp and version of the consent banner shown.
- `audit_log`: **legitimate interest** (security and fraud), plus
  **legal obligation** for the subset that satisfies SOX/SOC controls.
- `payments.*`: **contract** + **legal obligation** (we have to retain
  financial records for ~7 years in most jurisdictions).

### Data minimization

Article 5(1)(c): collect only what you need for the stated purpose, and
no more.

Engineering consequences:

- Resist the product manager who says "let's also collect their phone
  number, in case we ever want it." If you can't name a current use,
  don't collect.
- Logs that capture full request bodies will capture PII that you have
  no business retaining. Either filter at the logging layer or redact
  before write. See [`docs/security/04-transport-headers.md`](../security/04-transport-headers.md)
  for the redaction layer pattern.
- Backups inherit whatever was in the source database. If you collected
  too much, your backups now hold it too. Deleting from production
  doesn't delete from a 90-day-old backup tape.

### Purpose limitation

You stated *why* you collected the data; you can only use it for that
purpose unless you obtain a new lawful basis. "We collected the email
for transactional notifications" does not extend to "and now we mail
them the quarterly newsletter."

### Right of access (Article 15)

A user can ask for a copy of all the personal data you hold about them.
You have one month to respond (extensible to three for "complex"
requests).

In this repo this is the `GET /me/export` endpoint that MemberClub
exposes — it walks the `users`, `subscriptions`, `payments` (metadata
only), and `audit_log` tables and produces a JSON blob.

What's tricky: **derived data also counts**. The recommendation scores
the system computed about the user are personal data. The cohort the
user was placed in is personal data. The risk score that decided their
credit limit is personal data. Modeling-pipeline outputs often slip
through because they don't feel like "the user's data" but they are
"data about the user."

### Right of erasure / "right to be forgotten" (Article 17)

A user can ask you to delete their personal data, subject to exceptions
(legal obligation to retain, freedom of expression, etc.).

The engineering reality:

- Deleting from the primary database is easy.
- Deleting from read replicas happens automatically as replication
  catches up.
- Deleting from search indices, caches, and pre-aggregated tables
  requires extra plumbing.
- Deleting from backups is impossible in any practical sense — you
  cannot rewrite a 6-month-old tape. You document the retention window
  of backups, and you commit that the deleted user's data will be gone
  from backups by `now + retention_window`.
- Deleting from logs is impossible without breaking the audit trail.

### The "can't delete the audit log" tension

The `audit_log` table is append-only by design. Every state-changing
admin action writes a row: `(actor_id, action, target_id, before, after,
timestamp)`. This is load-bearing for SOC 2, for fraud investigation,
for legal discovery.

If an audit-log row says "admin 7 reset the password for user 4242 at
2026-03-15T14:23Z," and user 4242 invokes their right of erasure, can
we delete that row?

- Deleting it breaks the audit trail. The row that says "admin reset
  user 4242's password" disappears; the actions that depended on that
  reset now reference a ghost.
- Keeping it preserves an identifier (`target_id = 4242`) that links to
  the deleted user.

The standard mitigation is **crypto-shredding**:

1. Every user has a per-user **data encryption key (DEK)**.
2. PII fields (and the resolvable parts of audit-log rows referencing
   that user) are encrypted with the user's DEK before being written.
3. The DEKs themselves are encrypted with a **key encryption key (KEK)**
   held in a KMS and stored alongside the user record.
4. When a user invokes erasure, you delete (or zero out, or scramble)
   their DEK row.
5. The encrypted data still exists everywhere — primary DB, backups,
   logs — but is now unreadable forever.

This satisfies the regulation: "the personal data has been rendered
inaccessible." It also preserves the structural integrity of the audit
log (the row still exists; you just can't decrypt the parts that named
the user). It is not a free pass — you have to actually implement the
encryption, key management, and rotation. But it's the only practical
answer to the append-log-vs-erasure tension at scale.

MemberClub does NOT yet implement crypto-shredding. Its erasure
implementation does the "delete from primary, replace identifier in
audit_log with `<redacted>`, document backup retention" version. This is
defensible for an early-stage product and clearly the gap to close as
the company grows.

### Right to portability (Article 20)

A user can ask for their data in a "structured, commonly used, machine-
readable format." JSON is fine. CSV is fine. PDF is not.

The export endpoint above satisfies this. The portability right is
slightly narrower than the access right — it covers data the user
provided or that derives from their activity, not (for example) the
risk scores you computed.

### Breach notification (Articles 33–34)

If a breach is "likely to result in a risk to the rights and freedoms of
natural persons," you must notify the supervisory authority within 72
hours of becoming aware of it. If the risk is "high," you must also
notify the affected users without undue delay.

The 72-hour clock is the operational hard part. You need:

- A way to detect breaches quickly (intrusion detection, anomaly
  monitoring, the customer who tweeted that their email is in a paste
  bin).
- A documented incident-response process that gets the right humans on
  the call within hours, not days. See
  [`docs/runbooks/`](../runbooks/) — the 5xx-spike and webhook-lag
  playbooks both feed into the broader incident process.
- A pre-written decision matrix for "is this a notifiable breach?"
  (made before the incident, when you have time to think).
- Contacts and templates for the supervisory authority and for users.

### Data Protection Impact Assessment (DPIA)

For processing that is "likely to result in a high risk" to subjects —
profiling at scale, large-scale processing of special categories,
systematic monitoring of public areas — you must run a DPIA before
starting. The DPIA documents what you're doing, the necessity, the
risks, the mitigations.

For MemberClub today, no DPIA-triggering activity is in scope. If
MemberClub adds AI-driven content moderation or behavior scoring, DPIA
becomes relevant.

## Controller vs. processor

GDPR distinguishes:

- **Controller**: decides why and how data is processed. MemberClub is
  the controller for its users.
- **Processor**: processes data on behalf of a controller, under
  contract. Stripe (for payment data we hand to it), AWS (for the
  database itself), Datadog (for the logs we ship), are processors.

The controller owes most of the obligations. The controller must have a
**Data Processing Agreement (DPA)** with each processor that flows down
relevant obligations.

Engineering consequences:

- You can't ship PII to a SaaS vendor without a DPA. The vendor's
  standard terms usually include one; check.
- The list of subprocessors is part of your privacy notice. When you
  add a new vendor that handles PII, you have to update it.
- Cross-border transfer rules (data leaving the EU) require additional
  contractual mechanisms (Standard Contractual Clauses, adequacy
  decisions). If your DB is in AWS `eu-west-1` and your logs go to
  Datadog `us1`, you have a cross-border transfer.

## EU-resident-only deployment patterns

The strongest answer to cross-border transfer concerns is: don't
transfer. Run an EU instance for EU users, a US instance for everyone
else, and never let the two share data.

Architecturally this is multi-tenant with a region-scoped tenancy. The
primitive is the same as
[`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/):
every row carries a `tenant_id`, every connection sets a session
`current_setting('app.tenant_id')`, and a Postgres row-level-security
policy filters automatically. The region scope is an additional level:
every tenant has a `region`, and traffic for a tenant routes to the
region-local API and database.

Trade-offs:

- Operational cost roughly doubles per region.
- Cross-region features (a global leaderboard, a global search) become
  hard. You have to decide if they're worth opting in to data transfer
  for.
- Disaster recovery is region-local: a region failure means EU users
  see downtime, not a US-region failover that would itself be a data
  transfer event.

## Concrete schema and code touchpoints

| Obligation | Where it lives in MemberClub |
|---|---|
| Lawful basis catalog | [`04-pii-data-map.md`](./04-pii-data-map.md) "lawful basis" column |
| Consent record | `users.marketing_opt_in_ts`, `users.consent_version` |
| Data minimization | review at PR time; CI lint for new columns |
| Right of access | `GET /me/export` |
| Right of erasure | `DELETE /me` (cascades, redacts audit_log) |
| Right of portability | `GET /me/export?format=json` |
| Right to object (to marketing) | `users.marketing_opt_in = false` |
| Breach notification process | [`docs/runbooks/`](../runbooks/) + the post-mortem flow in [`docs/03-postmortems/`](../03-postmortems/) |
| Subprocessor list | privacy notice page (external) + `docs/security/03-supply-chain.md` |
| EU-resident isolation | [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/) primitive |
| Retention windows | [`04-pii-data-map.md`](./04-pii-data-map.md) "retention" column |
| Crypto-shredding | NOT YET IMPLEMENTED; planned |

## What "GDPR compliant" actually feels like in day-to-day engineering

- Every new column added in a migration triggers a checklist: is this
  PII? what's the lawful basis? retention? does it propagate to logs?
- New vendor integrations require a DPA review and a privacy-notice update.
- Marketing requests "let me email all users who clicked X in the last
  90 days" become conversations about whether the consent record covers
  this exact use.
- The export endpoint becomes a tax on every feature: when you add a
  new table containing personal data, you have to add it to the export.
- The erasure endpoint becomes a similar tax: you have to delete from
  the new table too, or document why it's exempt.

It is not a one-time project. It is a way of working.
