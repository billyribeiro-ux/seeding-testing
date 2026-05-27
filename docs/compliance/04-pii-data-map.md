# PII Data Map

This is the single most useful compliance artifact you can produce.
It is also the one auditors will ask for first, before anything else.

A PII data map is a table that lists, for every place personal data
lives in your system, the answers to five questions:

1. **What is it?** (column / field / log key)
2. **What classification?** (identifier / sensitive / pseudonymized / derived)
3. **What's the lawful basis?** (consent / contract / legal / legitimate)
4. **What's the retention?** (deletion or anonymization schedule)
5. **Where does it propagate?** (logs, backups, OTel spans, vendor APIs)

The act of producing the map will surface:

- Columns that exist because someone added them three years ago and
  nobody remembers why ("we don't have a lawful basis for this" =
  delete the column).
- Logs that capture data you never meant to retain ("OTel span attribute
  `user.email` is sent to a vendor whose DPA doesn't cover EU data" =
  redact the attribute).
- Backups that hold data well past your stated retention window
  ("backups retain 365 days, but our privacy notice says we delete
  marketing data after 90 days" = either fix the policy or fix the
  backup retention).

## The template

The map is usually a table in a Notion doc, a Confluence page, or a
spreadsheet that lives in a secured drive. It does NOT need to live in
this repo — and arguably shouldn't, because changing it on every schema
change is a different cadence from code changes. But the **structure**
should be documented here.

A row in the map has at least these columns:

| Column name | What it means |
|---|---|
| Location | `<table>.<column>`, or `logs.field`, or `otel.attr`, etc. |
| Description | One sentence: what is this? |
| Classification | identifier / contact / sensitive / pseudonymized / derived / non-PII |
| Lawful basis (GDPR) | consent / contract / legal obligation / legitimate interest |
| Purpose | one-sentence reason this data is collected/used |
| Retention | "indefinite (subject to deletion request)" / "90 days" / etc. |
| Propagates to | comma-separated: read replicas, logs, backups, OTel, Stripe, ... |
| Owner | name of the team/person responsible |
| Last reviewed | date |
| Notes | edge cases, deprecation plans, etc. |

### Classification definitions

- **Identifier**: a value that uniquely identifies a natural person
  on its own. Email, full name, government ID, account ID where the
  account is for one person.
- **Contact**: a way to reach the person. Email, phone, address.
  (Overlaps with identifier; classify by primary purpose.)
- **Sensitive**: special-category data under GDPR Article 9 (health,
  race, sexual orientation, biometrics, religion, political opinion,
  union membership) plus financial account data. Sensitive data
  triggers stricter handling.
- **Pseudonymized**: data that has been processed in such a way that
  it can no longer be attributed to a specific person without the use
  of additional information held separately. A hash of an email is
  pseudonymized; the email itself is not.
- **Derived**: data computed about the person (risk scores, cohort
  assignment, model predictions). Often forgotten — treat as PII.
- **Non-PII**: data that is genuinely about the system, not the
  person. Server hostname, request latency, error code. Note these in
  the map only if they're at risk of being misclassified.

### Lawful basis: pick the strongest applicable

If both "contract" and "legitimate interest" would justify a use,
choose "contract" — it's more defensible and easier to explain.
"Consent" is the weakest because it's revocable; if you can rely on
another basis, do.

### Retention: be specific

"As long as necessary" is not an answer. Concrete acceptable values:

- "Indefinite while account is active; 90 days after account deletion."
- "365 days from creation."
- "Until consent is revoked, then 30 days for queue flush."

### Propagation: trace the data, not just the source

Every PII row in production has a fan-out. The fan-out for, say,
`users.email`:

- **Primary DB** (`memberclub.users.email`): the source.
- **Read replicas**: same value, automatically.
- **Backups**: pg_dump nightly + WAL archive. Retention 30 days.
- **Logs**: `tracing` fields include `user.email` on login events.
  Retention: 90 days. Shipped to Datadog (a subprocessor).
- **OTel spans**: spans tagged with `user.email` on authenticated
  requests. Sent to the OTel collector → exporter → Honeycomb
  (subprocessor).
- **Email service**: Postmark (subprocessor) receives the email when
  sending transactional notifications.
- **Stripe**: customer object includes email. Subprocessor.
- **Local devs' databases**: synthetic data only — confirmed no PII
  in dev/staging.

Each "propagates to" entry implies a control. If Datadog receives
`user.email` in logs, then (a) the Datadog DPA must cover that, (b)
the data residency must match, (c) the retention in Datadog must be
compatible, (d) if the user is deleted, the log entries will age out
naturally within the log retention window — document this.

## Worked example: the MemberClub map

This is illustrative — the real map is updated continuously and lives
elsewhere. The format below is what each row in the real map should
look like.

### `users` table

| Location | Class | Lawful basis | Purpose | Retention | Propagates to | Notes |
|---|---|---|---|---|---|---|
| `users.id` | identifier (pseudonymous) | contract | join key | indefinite while active; redacted in audit_log on erasure | DB, replicas, backups, logs, OTel, Stripe customer metadata | numeric; not directly identifying without join |
| `users.email` | identifier + contact | contract | login, notifications | indefinite while active; deleted on erasure (modulo backup window) | DB, replicas, backups, logs (login events), OTel, Postmark, Stripe | redact in logs above DEBUG level |
| `users.password_hash` | sensitive (auth secret) | contract | authentication | indefinite while active; deleted on erasure | DB, replicas, backups | NEVER logged; argon2id hash, not password |
| `users.full_name` | identifier | contract | personalization | indefinite while active; deleted on erasure | DB, replicas, backups, Stripe | |
| `users.country` | non-PII (alone) | contract | tax, region routing | indefinite | DB, replicas, backups, logs | becomes identifying combined with other fields |
| `users.created_at` | metadata | contract | analytics, support | indefinite | DB, replicas, backups, logs | |
| `users.marketing_opt_in` | metadata | consent | marketing decision | indefinite while active | DB, replicas, backups | drives whether marketing emails are sent |
| `users.marketing_opt_in_ts` | metadata | consent | proof-of-consent timestamp | indefinite (legal hold) | DB, replicas, backups | |
| `users.consent_version` | metadata | consent | proof of which notice was shown | indefinite (legal hold) | DB, replicas, backups | |

### `subscriptions` table

| Location | Class | Lawful basis | Purpose | Retention | Propagates to | Notes |
|---|---|---|---|---|---|---|
| `subscriptions.id` | identifier (pseudonymous) | contract | join key | 7 years post-cancellation (financial) | DB, replicas, backups | |
| `subscriptions.user_id` | identifier (FK) | contract | ownership | 7 years post-cancellation | DB, replicas, backups | |
| `subscriptions.plan` | non-PII | contract | billing | 7 years post-cancellation | DB, replicas, backups | |
| `subscriptions.status` | metadata | contract | service delivery | 7 years post-cancellation | DB, replicas, backups | |
| `subscriptions.stripe_subscription_id` | identifier (cross-system) | contract | Stripe sync | 7 years post-cancellation | DB, replicas, backups, Stripe | links our record to Stripe's |

### `payments` table

| Location | Class | Lawful basis | Purpose | Retention | Propagates to | Notes |
|---|---|---|---|---|---|---|
| `payments.id` | identifier (pseudonymous) | contract + legal | financial records | 7 years (US tax) / longer per jurisdiction | DB, replicas, backups | |
| `payments.user_id` | identifier (FK) | contract + legal | attribution | 7 years | DB, replicas, backups | |
| `payments.amount_cents` | financial (non-PII alone) | contract + legal | accounting | 7 years | DB, replicas, backups, Stripe | |
| `payments.stripe_charge_id` | identifier (cross-system) | contract + legal | reconciliation | 7 years | DB, replicas, backups, Stripe | |
| `payments.last4` | identifier (truncated) | contract | dispute support | 7 years | DB, replicas, backups | last 4 digits of card; permissible to retain per PCI |

**Note:** MemberClub does NOT store full card numbers, expiry, or CVV.
That data lives only with Stripe; we receive `last4` for support
purposes. This is the standard "Stripe is the card data environment"
delegation and is what keeps MemberClub out of PCI scope.

### `audit_log` table

| Location | Class | Lawful basis | Purpose | Retention | Propagates to | Notes |
|---|---|---|---|---|---|---|
| `audit_log.id` | identifier (pseudonymous) | legal + legitimate | audit trail | 7 years | DB, replicas, backups | |
| `audit_log.actor_id` | identifier (FK to users) | legal + legitimate | accountability | 7 years | DB, replicas, backups | redacted on actor erasure, not deleted |
| `audit_log.target_id` | identifier (FK to users) | legal + legitimate | what was acted on | 7 years | DB, replicas, backups | redacted on target erasure, not deleted |
| `audit_log.before` / `audit_log.after` | varies (may contain PII) | legal + legitimate | reconstruction | 7 years | DB, replicas, backups | this is where crypto-shredding matters; today we redact specific fields on erasure |

### Logs (Datadog)

| Location | Class | Lawful basis | Purpose | Retention | Propagates to | Notes |
|---|---|---|---|---|---|---|
| `logs.user_id` | identifier (pseudonymous) | legitimate (operations) | debugging | 90 days hot, 365 days cold | Datadog | |
| `logs.user_email` (login events only) | identifier + contact | legitimate (security) | failed-login forensics | 90 days hot, 365 days cold | Datadog | not logged on routine requests |
| `logs.request_id` | non-PII | legitimate (operations) | request correlation | 90 days hot, 365 days cold | Datadog | |
| `logs.ip_address` | identifier (per GDPR) | legitimate (security) | abuse detection | 90 days hot, 365 days cold | Datadog | truncate last octet for non-security paths |

### OTel spans (Honeycomb)

| Location | Class | Lawful basis | Purpose | Retention | Propagates to | Notes |
|---|---|---|---|---|---|---|
| `span.user.id` | identifier (pseudonymous) | legitimate (operations) | latency attribution | 60 days | Honeycomb | |
| `span.tenant.id` | identifier (org) | legitimate (operations) | latency attribution | 60 days | Honeycomb | |
| `span.http.url` | varies | legitimate (operations) | URL pattern analysis | 60 days | Honeycomb | scrub query params before export |
| `span.user.email` | identifier + contact | legitimate (operations) | NOT EXPORTED | n/a | n/a | stripped by OTel processor before export |

### Backups (S3)

Backups are not their own data type — they are a snapshot of the
above. Treat each row in the map's "propagates to: backups" column
as implicitly inheriting the same classification, with the addition
that the retention is `max(source retention, backup retention)`.

| Backup location | Contents | Retention | Access controls |
|---|---|---|---|
| `s3://memberclub-backups-primary/postgres/` | nightly pg_dump | 30 days | IAM: only backup role + on-call group |
| `s3://memberclub-backups-primary/wal/` | continuous WAL archive | 30 days | IAM: only backup role + on-call group |
| `s3://memberclub-backups-secondary/...` (other region) | replicated copy | 30 days | IAM: tighter — only DR-test role |

## Maintaining the map

The map goes stale fast. The discipline that keeps it fresh:

1. **Migration template**: every database migration PR has a checklist
   that includes "did you add a new column? if it contains PII, has the
   data map been updated?" Catch at PR review.
2. **Quarterly review**: the privacy owner walks the map quarterly, asks
   the data owner of each row "still accurate? still needed? still the
   right retention?" Removes obsolete rows.
3. **Schema-diff CI check**: a CI job dumps the current schema, diffs
   against a known-good baseline, and fails the build if there are new
   columns not annotated in the map. This is the strongest enforcement
   — humans forget, CI doesn't.
4. **Log-key linter**: similar idea for structured logs. Every log call
   uses a known key vocabulary; new keys not in the vocabulary fail CI
   until added to the map.
5. **OTel attribute allowlist**: the OTel processor has an explicit
   list of allowed attribute names. Anything else is dropped. This
   prevents accidental PII export through tracing.

## What auditors do with the map

GDPR auditors:

- Walk the map. For each row, verify the lawful basis matches what the
  privacy notice describes. Mismatches are findings.
- Pick a sample of rows and trace propagation: "you say `users.email`
  goes to Postmark. Show me the Postmark DPA and the data residency
  setting." Anything claimed in the map must be evidenced.
- Pick a sample of erasure requests and verify every row marked "deleted
  on erasure" was actually deleted (modulo backup windows).

SOC 2 auditors:

- Use the map to validate the **Confidentiality** and **Privacy** TSCs.
- Cross-check against the access control list: who has read access to
  the tables that contain "identifier" or "sensitive" rows?
- Verify the retention policies are enforced by a process (a scheduled
  job, a manual checklist) — having the row in the map isn't enough;
  the deletion has to actually happen.

HIPAA auditors:

- Demand the equivalent of the map but specifically for PHI. The 18
  Safe Harbor identifiers are the check matrix.

If the map is honest and current, you walk into the audit room with the
hardest question already answered.
