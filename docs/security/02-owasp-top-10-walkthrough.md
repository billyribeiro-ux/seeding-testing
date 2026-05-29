# OWASP Top 10 (2021) walkthrough — MemberClub

The OWASP Top 10 isn't a checklist; it's a catalog of the categories
that account for most production breaches. We use it as a calibration
tool: walk every category against the actual code, name the concrete
defense (with a file:line reference where possible), and flag the
categories where MemberClub has nothing yet so the gap is on the
roadmap.

This document walks the 2021 edition. The 2026 revision is likely to
fold A05 and A09; we'll update when that lands.

---

## A01 — Broken access control

**What it covers.** Failing to enforce that a user can only access
data and actions appropriate to their identity. Includes IDOR (insecure
direct object reference), missing function-level authz, and force-browsing.

**Defenses in repo.**

- **Explicit policy layer.** Every authorized action goes through
  `projects/05-rbac-policy-lab/src/lib.rs`'s `can_*` functions, which
  combine RBAC (role membership) and ABAC (resource attributes). The
  `require!` macro converts a `false` to an early `403`. The test file
  (`tests/policy.rs`) reads like a permission matrix — easy to review.
- **Row-Level Security in Postgres.** Tenant-scoped tables can't
  return cross-tenant rows even if the handler forgets a `WHERE`
  clause — the RLS policy does the filtering. See
  `projects/12-multi-tenant-rls/src/lib.rs:72` (`set_local_tenant_sql`)
  and ADR-0007.
- **IDOR-resistant ids.** Sessions and tokens are 32-byte random
  base64url strings (`projects/04-auth-demo/src/lib.rs:455`,
  `random_token_b64url(32)`) — not sequential integers exposed in URLs.

**Audit.** Grep for handlers under `/admin` that don't call any
`require!` or `policy::can_*`. There should be zero hits.

**Gap.** No automated test that asserts a non-admin can't hit every
admin route — only the manual tests in `tests/policy.rs`. **Action:**
add a property-based test in the auth demo that iterates every route
in the router and asserts `403` for a non-admin subject.

---

## A02 — Cryptographic failures

**What it covers.** Plaintext storage of sensitive data, weak algorithms,
weak keys, hardcoded secrets, transport without TLS.

**Defenses in repo.**

- **Argon2id for passwords.** `projects/04-auth-demo/src/password.rs`
  uses `argon2::Argon2::default()` with per-password salts via
  `SaltString::generate(&mut OsRng)` and stores PHC strings so parameter
  changes are forward-compatible.
- **RS256 for JWTs.** `projects/04-auth-demo/src/jwt_rs256.rs` uses
  asymmetric signing — the private key never touches a verifying
  service. Algorithm pinned (see STRIDE S-3).
- **HMAC-SHA-256 for webhooks.**
  `projects/07-webhook-receiver/src/lib.rs:152` (`compute_signature`)
  uses `hmac::Hmac<Sha256>`.
- **No hardcoded secrets.** `.env.example` is the only env-var
  catalog; every entry has a placeholder, never a real value.
- **Constant-time compares.** Webhook signature equality uses
  `timing_safe_eq` (`projects/07-webhook-receiver/src/lib.rs:198`).

**Audit.** `cargo deny check` rejects unmaintained crypto crates (see
`deny.toml`). The duplicate-version ban will catch `openssl` + `ring`
mixing.

**Gap.** No envelope encryption for at-rest sensitive columns (e.g.
TOTP secrets in `projects/04-auth-demo/src/totp.rs`). **Action:**
introduce per-row encryption with a column-level KMS data key.

---

## A03 — Injection

**What it covers.** SQL injection, command injection, LDAP injection,
template injection. Untrusted input being interpreted as code.

**Defenses in repo.**

- **Parameterized SQL everywhere.** All sqlx call sites use `?`
  placeholders with `.bind()`. Examples already cited in STRIDE T-2.
- **No `eval`-equivalents in Rust.** No `Command::new` calls with
  shell concatenation; no `tera` or `handlebars` templates rendering
  user input as code.
- **JSON parsing rejects malformed input.**
  `projects/07-webhook-receiver/src/lib.rs:217` (`parse_event_meta`)
  uses serde_json and returns `WebhookError::BadJson` on failure
  rather than panicking.

**Audit.** `rg "format!.*sqlx::query"` should return zero hits in app
code. The one acceptable use is `projects/12-multi-tenant-rls/src/lib.rs:72`
where the interpolated value is a UUID newtype (see STRIDE T-2).

**Gap.** No fuzz testing on the JSON parser surface. **Action:** add
`cargo-fuzz` target for the webhook parser; run nightly in CI.

---

## A04 — Insecure design

**What it covers.** Missing security controls baked into the
architecture itself — not "we forgot to validate this input" but "we
chose a design that has no place to put the validation."

**Defenses in repo.**

- **Outbox pattern over direct broker writes.** ADR-0006 picks the
  outbox so failures can't dual-write inconsistently. See
  `projects/08-outbox-demo/`.
- **Idempotent webhooks by design.**
  `projects/07-webhook-receiver/src/lib.rs:242` (`store_event` +
  `pending_event_id`) makes duplicate delivery a no-op while still
  resuming an event whose handler crashed before completing.
- **Money as `i64` cents with a `Money` newtype.** Documented in
  `docs/00-mental-models/money.md` and ADR-0003. Floating-point money
  bugs are a category error here.
- **Tenant id as a newtype.** `projects/12-multi-tenant-rls/src/lib.rs`
  uses `TenantId(Uuid)` so a handler that wants a tenant id can't
  accidentally receive a user id.

**Gap.** No formal threat model review checkpoint in the SDLC.
**Action:** require an STRIDE walk in every RFC under `docs/02-rfcs/`
for any feature touching auth, money, or PII.

---

## A05 — Security misconfiguration

**What it covers.** Default credentials, verbose error messages,
unnecessary features enabled, missing headers, unhardened cloud
buckets.

**Defenses in repo.**

- **Debug/release feature differences.** Cookies are `secure(true)`
  only in release: `.secure(cfg!(not(debug_assertions)))`
  (`projects/04-auth-demo/src/lib.rs:459`). This is the right default
  because local dev runs over http.
- **Tracing filter excludes query logs.** `RUST_LOG=
  info,sqlx::query=warn,tower_http=info` (`.env.example:32`).
- **Problem-details error format.** Errors return RFC 7807 documents
  (`projects/07-webhook-receiver/src/lib.rs:99`) with stable URLs —
  not stack traces.

**Audit.** Search for `unwrap()` or `expect()` in handler code; any
hit is a potential leak of internals into a 500 response.

**Gap.** No transport-headers middleware ships browser hardening
headers today. **Action:** see `04-transport-headers.md`; add a
`SetResponseHeaderLayer` chain to `apps/memberclub/api`.

---

## A06 — Vulnerable and outdated components

**What it covers.** Using a dependency with a known CVE; running an
EOL runtime; missing patches.

**Defenses in repo.**

- **`cargo audit` in `make verify`.** Hits the RustSec advisory DB on
  every build and CI run. See `Makefile`.
- **`cargo deny check` in `make verify`.** License + advisory + ban
  + source rules in the root `deny.toml`. The duplicate-version ban
  catches the famous `openssl` + `ring` mixing.
- **`rust-toolchain.toml` pins the toolchain.** No "works on my
  machine" drift.
- **`pnpm audit` for the JS side.** Documented in
  `03-supply-chain.md`.

**Gap.** No `cargo vet` yet (this PR seeds it). **Action:** roll out
the `vet` workflow over the next sprint; see `03-supply-chain.md`.

---

## A07 — Identification and authentication failures

**What it covers.** Weak password storage, missing MFA, predictable
session ids, credential-stuffing tolerance, missing rate limits.

**Defenses in repo.**

- **Argon2id** — see A02.
- **Random 32-byte session tokens** — see A01.
- **TOTP / WebAuthn support.** `projects/04-auth-demo/src/totp.rs` —
  enroll, verify, and step-up. Verify uses constant-time string
  comparison via the `totp-lite`-style code path at `:193`.
- **Refresh token families.** Replay of any used refresh token
  revokes the family (`projects/04-auth-demo/src/refresh_tokens.rs`).
- **Login rate limit.** `tower_governor` by IP
  (`projects/04-auth-demo/src/lib.rs:29`).
- **Email enumeration mitigations.** Login always runs Argon2 even on
  unknown users (sentinel hash, `password.rs:11`). The password-reset
  flow returns 200 whether or not the email exists
  (`projects/04-auth-demo/src/password_reset.rs`).

**Gap.** No account-level lockout — distributed credential stuffing
could still grind through accounts. **Action:** add a per-account
failed-login counter with backoff.

---

## A08 — Software and data integrity failures

**What it covers.** Trusting code or data without verifying integrity
— unsigned updates, deserialization of untrusted data, CI/CD
poisoning.

**Defenses in repo.**

- **Webhook HMAC** — every event is signed; we never trust the body.
- **Outbox + idempotent handlers** — a corrupted broker delivery
  can't double-spend.
- **`Cargo.lock` committed.** Reproducible builds; no "patch-level
  drift" surprises.
- **Audit log of state changes.** `audit()` calls in
  `projects/04-auth-demo/src/lib.rs:477` and elsewhere.

**Audit.** Check that `Cargo.lock` is tracked, not gitignored. (It
is.)

**Gap.** No SLSA provenance attestation on release binaries.
**Action:** add `cargo-dist` with provenance; sign release artifacts
with cosign.

---

## A09 — Security logging and monitoring failures

**What it covers.** No logs of security-relevant events, logs you can
tamper with, no alerting on attack indicators.

**Defenses in repo.**

- **Tracing across handlers.** Every request gets a request id (see
  `tower_http::request_id::SetRequestIdLayer` in
  `projects/03-notes-api/src/lib.rs:34`).
- **Audit log table.** `audit()` writes `(user_id, action, metadata)`
  rows for state changes.
- **Webhook events stored raw.** `stripe_events` table keeps the body
  so we can replay an investigation.

**Gap.** Audit table lives in the same DB as the data — a DB
compromise can rewrite both. **Action:** ship audit rows to a
write-only sink (separate role, separate DB, or warehouse), and
alert on missing-row gaps.

---

## A10 — Server-side request forgery (SSRF)

**What it covers.** The server makes a request to a URL the attacker
controls — internal cloud metadata services (169.254.169.254), private
IPs, file://, gopher://.

**Defenses in repo.**

- **No URL-fetching handler today.** The MemberClub API doesn't
  accept "fetch this URL on my behalf" requests. The only outbound
  HTTP is to Stripe and is to a hard-coded base URL — no user input
  in the host portion.
- **Stripe client base URL fixed.** See
  `projects/10-memberclub-cli/src/client.rs` — base URL constants
  only.

**Gap.** When we add webhook re-delivery to customer URLs (planned),
SSRF becomes a real risk. **Action:** when that lands, ship an
allowlist of destination hosts; reject private IPs; resolve DNS once
and use the IP (no TOCTOU).

---

## Cross-cutting: a one-page audit recipe

When reviewing a change, run this quick five-minute sweep:

1. **`rg "format!" PR-files | rg "sqlx"`** — injection check (A03).
2. **`rg "unwrap\\(\\)|expect\\(" PR-files | rg -v test`** — leakage
   check (A05).
3. **`rg "tracing::.*(token|secret|password|key)"`** — log-secret check
   (A02 / A09).
4. **`rg -L "require!|policy::" PR-files/admin*`** — authz check (A01).
5. **`cargo audit && cargo deny check`** — supply-chain check (A06).

If any sweep returns a hit, justify it in the PR description before
merging. The reviewer's job is to push back when the justification is
"I forgot."
