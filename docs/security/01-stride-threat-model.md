# STRIDE threat model for MemberClub

STRIDE is Microsoft's threat-classification taxonomy from the early
2000s. It still earns its keep two decades later because the six
categories cover almost every interesting attack you'll see at the app
layer:

- **S**poofing — pretending to be someone (or something) else
- **T**ampering — modifying data in flight or at rest
- **R**epudiation — denying that an action was taken
- **I**nformation disclosure — reading data you shouldn't
- **D**enial of service — making the system unavailable
- **E**levation of privilege — gaining capabilities you weren't given

For each category below: a list of two-or-three concrete threats keyed
to MemberClub's code paths, the mitigation already in the repo (with a
file:line reference), and the residual risk so the next engineer knows
where the next layer of defense should land.

This is a living document. Add a row when you find a new threat. Edit
the residual-risk column when the underlying mitigation changes.

---

## Spoofing

### S-1 — Forged Stripe webhook

**Threat.** An attacker discovers the public webhook URL
(`POST /webhooks/stripe`) and posts a hand-crafted JSON body claiming a
`payment_intent.succeeded` event for a known customer. If the receiver
accepts it, the customer gets upgraded without paying.

**Mitigation in repo.** The receiver verifies the HMAC-SHA-256
signature in the `Stripe-Signature` header before doing anything else
with the body. See `projects/07-webhook-receiver/src/lib.rs:173`
(`verify_signature`). The handler at `:273` (`webhook`) calls verify
**first**, then parses, then stores. The signature is compared with a
constant-time equality check (`:198`, `timing_safe_eq`).

**Residual risk.**

- Webhook signing secret leak (operator compromise, env-var dump in a
  log). Detection: any 200 response to a webhook where our system did
  not initiate a corresponding API call. Add a Sigma rule on the
  webhook audit log.
- Replay within the freshness window. The signature header carries a
  timestamp and the receiver rejects anything outside `max_skew` (5
  min). An attacker who steals a valid request *and* its signature can
  replay within that window if our idempotency check fails.

### S-2 — Forged session cookie

**Threat.** An attacker crafts a cookie named `session` with arbitrary
content and presents it to the API to be treated as a logged-in user.

**Mitigation in repo.** The cookie value is a random 32-byte
URL-safe-base64 token (`projects/04-auth-demo/src/lib.rs:455`,
`random_token_b64url`). The cookie itself is signed by axum-extra's
`SignedCookieJar` using a ≥ 64-byte key from `SESSION_SECRET`
(`projects/04-auth-demo/src/main.rs:40`). The DB stores only the
SHA-256 of the token, so even read-only DB access doesn't yield a
session token usable against the API (see
`projects/04-auth-demo/src/sessions.rs:1`).

**Residual risk.** Cookie key compromise. Rotate `SESSION_SECRET` on
suspicion and every six months on policy. Existing sessions invalidate
immediately on rotation — by design.

### S-3 — Forged JWT (algorithm confusion)

**Threat.** Classic JWT attack: send a token with `alg: none` or with
`alg: HS256` while the verifier expects RS256, tricking the verifier
into using the public key as an HMAC secret.

**Mitigation in repo.** `projects/04-auth-demo/src/jwt_rs256.rs:216`
(`verify_access`) and `:232` (`verify`) pin the algorithm to RS256 via
`jsonwebtoken::Validation::new(Algorithm::RS256)`, so the `alg` field
in the token is constrained.

**Residual risk.** Private-key compromise (the `JWT_PRIVATE_KEY_PATH`
PEM file). Detection: log the `kid` of every issued token; alert when a
`kid` not in the active set appears.

---

## Tampering

### T-1 — Webhook body mutation in flight

**Threat.** A TLS-terminating proxy (or a compromised ingress) mutates
the JSON body of a Stripe webhook between Stripe and our receiver.

**Mitigation in repo.** The HMAC is computed over the raw body
(`projects/07-webhook-receiver/src/lib.rs:152`, `compute_signature`),
and the verify step (`:197`) recomputes against the exact bytes
received. Any single-byte mutation flips the signature.

**Residual risk.** A proxy that re-serializes JSON (re-orders fields,
normalizes whitespace) breaks verification — by design — and is caught
as a `SignatureMismatch` (`:73`). Operationally, make sure no
intermediary buffers and re-marshals.

### T-2 — SQL injection via interpolated query

**Threat.** Concatenating untrusted input into a SQL string lets an
attacker break out of the query and run arbitrary SQL.

**Mitigation in repo.** All app SQL uses sqlx's parameterized queries
(`sqlx::query`, `sqlx::query_as`, `sqlx::query!`). Examples:

- `projects/04-auth-demo/src/lib.rs:371` — `INSERT INTO users (email,
  password_hash) VALUES (?, ?) ...` with `bind`
- `projects/08-outbox-demo/src/lib.rs:144` — outbox row update
- `projects/03-notes-api/src/seed.rs:64` — `SELECT id FROM notes WHERE
  body = ?` with `bind`

Sweep with `rg "format!.*sqlx::query"` — any hits warrant review.

**Residual risk.** ORM-bypass debug code. `projects/12-multi-tenant-rls/src/lib.rs:72`
contains `format!("SELECT set_config('app.tenant_id', '{}', true)", t.0)` —
this is acceptable only because `t.0` is a `Uuid` newtype that can't
serialize to anything that breaks SQL syntax. If you ever change the
type of `t.0` to `String`, this becomes a bug.

### T-3 — Outbox tampering

**Threat.** A second writer mutates an outbox row between our read and
our handler's processing, causing a side-effect with the wrong payload.

**Mitigation in repo.** Outbox rows are claimed with row-level locks
and an `attempts` counter; concurrent processors can't double-process.
See `projects/08-outbox-demo/src/lib.rs:163` for the claim logic.

**Residual risk.** A privileged DB user could mutate rows directly.
Detection: triggers that audit changes to `outbox.payload` after
`created_at`.

---

## Repudiation

### R-1 — User denies an account action

**Threat.** A user claims "I never changed my email / never enabled
2FA / never deleted my data." Without an audit trail we can't
distinguish lie from compromise.

**Mitigation in repo.** Every state change calls `audit()` to write a
row into `audit_log` with `user_id`, `action`, and `metadata`. Example:
`projects/04-auth-demo/src/lib.rs:477`
(`audit(&s.pool, Some(user.id), "user.logged_in", None)`). The login
flow also issues a refresh-token family rooted at the login event
(`:472`), so we can correlate token use back to a specific login.

**Residual risk.** The audit table is in the same database as the
state it audits. A DB-level compromise can rewrite both. Production
should ship audit events to an append-only sink (e.g. a separate
write-only role or a downstream warehouse).

### R-2 — Stripe disputes a charge we never processed

**Threat.** Stripe says we charged a customer; the customer says no.

**Mitigation in repo.** `stripe_events` is the authoritative log of
every webhook we accepted (`projects/07-webhook-receiver/src/lib.rs:242`,
`store_event`). The row includes the raw body and the stripe event id;
the (event id, received-at) tuple gives us a defensible record.

**Residual risk.** A dropped webhook the cron poller picks up later
gets a different `received_at`. Document this in the runbook
(`docs/runbooks/stripe-webhook-lag.md`).

---

## Information disclosure

### I-1 — Cross-tenant read

**Threat.** Tenant A queries the API and the response contains tenant
B's data — either due to a buggy `WHERE` clause or a missing one.

**Mitigation in repo.** Postgres Row-Level Security. Every
tenant-scoped table has `ENABLE ROW LEVEL SECURITY` plus a policy
filtering on the `app.tenant_id` GUC, which the request handler sets
via `SET LOCAL app.tenant_id = '...'` at the start of each transaction.
See `projects/12-multi-tenant-rls/src/lib.rs:72` (`set_local_tenant_sql`).
The app role is **not** a Postgres superuser, so it can't bypass RLS.

**Residual risk.** A handler that forgets to set the GUC silently
returns zero rows (safe, but creates a bug). Detect with an integration
test that hits each tenant-scoped table without first calling
`set_local_tenant_sql` and asserts 0 rows.

### I-2 — Password hash leak via timing

**Threat.** Login latency reveals whether a user exists ("user not
found" returns in 1 ms; "wrong password" takes 200 ms because Argon2
ran).

**Mitigation in repo.** The login path *always* runs Argon2 verify,
even when the user doesn't exist, against a sentinel hash. See
`projects/04-auth-demo/src/password.rs:11` (`SENTINEL_HASH`). The
constant-time path keeps response time uniform.

**Residual risk.** Argon2 parameters are tuned for the production
machine; a slower runner can make login take seconds. Monitor p99
login latency; alert if it diverges from the baseline.

### I-3 — Secrets in logs

**Threat.** A panic prints the connection string (with password) or a
debug log dumps a JWT.

**Mitigation in repo.** `RUST_LOG=info,sqlx::query=warn,tower_http=info`
(`.env.example:32`) is the production filter — sqlx query logs are
suppressed at info level. JWTs and webhook signatures never appear in
structured fields named in the code (search `tracing::info!.*token` —
no hits).

**Residual risk.** A new contributor adds `tracing::info!("body =
{body:?}")`. Mitigate with a clippy lint or a pre-commit grep that
fails on `{secret`, `{token`, `{password` in tracing macros.

---

## Denial of service

### D-1 — Login brute-force

**Threat.** An attacker hammers `/auth/login` with credential-stuffing
attempts, exhausting our Argon2 CPU budget and locking real users out.

**Mitigation in repo.** `tower_governor` rate-limits login by IP. See
`projects/04-auth-demo/src/lib.rs:29` (`use tower_governor::GovernorLayer`)
and `:53` (`pub struct RateLimit`). Default in production:
`RateLimit::production_defaults().login_per_minute`. The state is
in-process today — adequate behind a single instance, replace with
Redis-backed `rate_limit` (`projects/11-redis-cache/src/lib.rs:27`)
when horizontally scaling.

**Residual risk.** Distributed brute-force from many IPs (botnet)
defeats per-IP limits. Layer: account-level lockout after N failures,
plus a CAPTCHA after the first rate-limit hit.

### D-2 — Slow-loris / long-body abuse

**Threat.** A client opens a connection and sends one byte per minute,
tying up workers.

**Mitigation in repo.** `tower_http::timeout::TimeoutLayer` is wired
into the router stack: `projects/03-notes-api/src/lib.rs:110` and
`projects/03-notes-api/tests/router_extras.rs:151`. Default request
timeout 10s.

**Residual risk.** Body-size DoS — a 10 GB POST takes more than 10s
just to read. Add an explicit `RequestBodyLimitLayer` and confirm at
the ingress layer (nginx `client_max_body_size`).

### D-3 — Webhook storm

**Threat.** Stripe retries a backlog after an outage and floods us
with 10k events in a minute.

**Mitigation in repo.** Idempotent insert
(`projects/07-webhook-receiver/src/lib.rs:242`, `store_event`)
short-circuits duplicates without doing real work. The handler returns
200 fast on duplicates (`:288`).

**Residual risk.** A new event type with an expensive handler can
still pile up. Worker pool sizing and queue depth alerts cover this —
see capacity-planner docs.

---

## Elevation of privilege

### E-1 — Regular user accesses admin endpoint

**Threat.** A user with a valid session hits `/admin/users` and gets
back the user list.

**Mitigation in repo.** Explicit policy check: every admin handler
calls `policy::can_*` from `projects/05-rbac-policy-lab/src/lib.rs`
which gates on `Subject::is_admin()` (`:111`). The `require!` macro at
the call site turns the policy result into an early `403`. The pattern
is also documented in ADR-0005 (explicit policies, no Casbin).

**Residual risk.** A handler that forgets the `require!` call. Audit:
grep for handlers that mention "admin" without a `require!` line.

### E-2 — RLS bypass via DB role

**Threat.** A handler accidentally uses a superuser DB connection
(e.g. from a migration helper) for tenant-scoped reads.

**Mitigation in repo.** Two roles: `app` (NOLOGIN BYPASSRLS = false) for
the app, `migrator` for migrations. Migration helper functions accept
the migrator pool, not the app pool. Search for
`PgPool` in handler signatures — they all take `state.pool` which is
the app pool.

**Residual risk.** Test code that connects with superuser credentials
gets used as the basis for a new handler. Forbid superuser connections
outside `#[cfg(test)]` with a CI check.

### E-3 — Refresh-token replay

**Threat.** An attacker steals a refresh token (e.g. from a logging
sink) and uses it to mint new access tokens forever.

**Mitigation in repo.** Refresh tokens are families: every refresh
returns a new (refresh, access) pair and invalidates the previous
refresh's `jti`. Replay of a used token revokes the entire family
(`projects/04-auth-demo/src/refresh_tokens.rs`,
`projects/04-auth-demo/src/lib.rs:467`).

**Residual risk.** Detection latency. The family is only revoked when
the legitimate user *or* the attacker tries to refresh again. Mitigate
by short access-token TTL (default 15 min, see
`projects/04-auth-demo/src/jwt.rs`).

---

## Open threats (no mitigation yet)

These don't have a defense in the repo today. Treat them as gaps for
the next security cycle.

- **CSRF on cookie endpoints.** `SameSite::Lax` is set on the session
  cookie (`projects/04-auth-demo/src/lib.rs:460`) which covers most
  cases, but state-changing GETs would still be vulnerable. Action:
  audit handlers; no state-changing GETs, or upgrade `SameSite::Strict`.
- **Subdomain takeover on `*.memberclub.test`.** No DNS audit. Action:
  add to the on-call quarterly checklist.
- **Dependency confusion on internal crate names.** No internal
  registry yet. Action: when one is added, reserve internal names on
  crates.io defensively.
