# docs/security/

Defense-in-depth notes for MemberClub. The curriculum points here from
Phase 9 onward, and on-call engineers reach for these during incident
triage.

The premise is paranoid by design: assume any single control will fail,
and write down the next layer that catches the blast.

## Layout

```
docs/security/
├── README.md                          ← you are here
├── 01-stride-threat-model.md          ← STRIDE applied to MemberClub
├── 02-owasp-top-10-walkthrough.md     ← OWASP 2021 mapped to repo code
├── 03-supply-chain.md                 ← cargo audit / deny / vet, pnpm
└── 04-transport-headers.md            ← CSP, HSTS, frame-options, SRI
```

## What's covered

### 01 — STRIDE threat model

For each STRIDE category (**S**poofing, **T**ampering, **R**epudiation,
**I**nfo disclosure, **D**oS, **E**levation), two-to-three concrete
threats keyed to real code paths in this repo. Every threat lists the
mitigation that's already in place (file:line) and what residual risk
remains so the next engineer knows where to look when something goes
sideways.

### 02 — OWASP Top-10 walkthrough

A walk through OWASP 2021's ten categories against MemberClub's actual
code. Each category cites the concrete defense (e.g. *A03 Injection →
`sqlx::query` parameterized everywhere, audit by grepping for
`format!("…")` adjacent to `sqlx::query`*). Where a defense is missing
it's flagged as a **Gap** with a one-line action item.

### 03 — Supply chain hygiene

The vendoring playbook:

- `cargo audit` (wired into `make verify`) — RustSec advisory matching
- `cargo deny` (wired into `make verify`) — licenses, bans, sources
- `cargo vet` (new; this repo seeds `.cargo-vet/`) — human review of
  third-party Rust crates at the version level
- `pnpm audit`, `pnpm dedupe`, `--ignore-scripts` — the JS posture
- Case studies: `left-pad` and `xz-utils` — what each control would or
  would not have caught

### 04 — Transport headers

Per-header explanations for every browser-facing header MemberClub
should ship: Content-Security-Policy, Strict-Transport-Security,
X-Content-Type-Options, X-Frame-Options, Referrer-Policy,
Permissions-Policy, Subresource-Integrity. Each entry says what the
header does, the exact value to ship, and how to test that it's set.

## Secrets policy (the one-paragraph version)

Secrets never enter git. The repository's `.gitignore` blocks `.env`,
`*.pem`, `dev-keys/`, `*.key`. The `.env.example` shipped at the root is
the only source of truth for "which secrets exist." Production secrets
live in the deployment platform's secret store and are mounted as env
vars at process start. Rotation is **policy-driven**, not
incident-driven: webhook signing secrets rotate quarterly; JWT signing
keys rotate every six months (with overlapping JWK kids so verifications
don't break mid-rotation); database credentials rotate yearly.

In code:

- `STRIPE_WEBHOOK_SECRET` is read in `apps/memberclub/api` and threaded
  through `projects/07-webhook-receiver/src/lib.rs:173` (`verify_signature`).
- `SESSION_SECRET` (≥ 64 bytes) is used to sign session cookies in
  `projects/04-auth-demo/src/main.rs:40`.
- `JWT_PRIVATE_KEY_PATH` / `JWT_PUBLIC_KEY_PATH` point at RS256 PEM
  files outside the source tree (see `projects/04-auth-demo/src/jwt_rs256.rs`).

If a secret leaks: rotate first, investigate second. The clock starts
the moment leakage is suspected, not the moment it's confirmed.

## Cross-references

- ADR-0004 (dual-mode auth) — why session cookies AND JWTs
- ADR-0005 (explicit policies, no Casbin) — RBAC/ABAC posture
- ADR-0007 (Postgres RLS) — tenant isolation at the row level
- `docs/runbooks/stripe-webhook-lag.md` — webhook signature failures
- `Makefile` (`make verify`) — runs `cargo audit` + `cargo deny check`

## Adding to this directory

If you discover a new threat class, add it to **01**. If you find a
defense in the codebase that isn't documented here, add it to **02**.
If you adopt a new supply-chain tool, document the workflow in **03**.
If you ship a new browser-facing header, document its value and test in
**04**.

When in doubt, write the doc before merging the change.
