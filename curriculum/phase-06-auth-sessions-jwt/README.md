# Phase 6 — Auth: Sessions and JWT (the Dual-Mode Pattern)

> **Audience:** you finished Phase 5.
> **Outcome:** you can ship password hashing, signed session cookies for web browsers, JWT access + refresh tokens for API clients, 2FA via TOTP, email verification, password reset, and rate limiting — the *enterprise* dual-mode pattern.
> **Time:** 3 weeks.

## The mental model

> **AuthN answers "who are you?". AuthZ answers "what can you do?".**
>
> Phase 6 is *only* about AuthN. AuthZ is Phase 7.

Two transports for AuthN, deliberately chosen per audience:

- **Web browsers** (the SvelteKit app) use **signed HttpOnly session cookies**. XSS can't read them; CSRF is mitigated via SameSite=Lax + double-submit token; revocation is a single row update.
- **API clients** (mobile, CLI, partner integrations) use **JWT access tokens** (15-minute lifetime, RS256-signed) + **refresh tokens** (30 days, single-use, rotation-with-reuse-detection).

Why dual-mode and not just JWT-in-localStorage? Because XSS pwns localStorage. Modern OWASP guidance for browser apps is *signed HttpOnly cookies*; JWTs are for clients that can store secrets safely.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-password-hashing-argon2.md` | Argon2id, salts, params, re-hashing on login |
| `lessons/02-session-cookies.md` | Signed HttpOnly cookies, SameSite, CSRF |
| `lessons/03-jwt-access-and-refresh.md` | RS256, claims, refresh rotation, key rotation by `kid` |
| `lessons/04-email-verification.md` | Signed time-bounded tokens, single-use, audit |
| `lessons/05-password-reset.md` | Same flow, different intent |
| `lessons/06-2fa-totp.md` | TOTP, recovery codes, step-up auth |
| `lessons/07-rate-limiting-and-lockout.md` | tower-governor, per-account counters, exponential backoff |
| `lessons/08-extractors-and-policies.md` | The `AuthenticatedUser` extractor, dual-mode bridging |
| `lessons/09-build-auth-demo.md` | Capstone walkthrough |

## The capstone — `projects/04-auth-demo`

A real Axum service that demonstrates every primitive in the phase plan:

- **`POST /auth/register`** — argon2-hashed signup, sends verification email (logged in tests).
- **`POST /auth/verify-email`** — single-use token completes verification.
- **`POST /auth/login`** — issues a cookie *and* a JWT access+refresh pair.
- **`POST /auth/logout`** — invalidates the session row.
- **`POST /auth/refresh`** — single-use refresh, with reuse detection.
- **`POST /auth/totp/enroll`** — start 2FA enrollment (returns provisioning URI + recovery codes).
- **`POST /auth/totp/confirm`** — confirm enrollment.
- **`GET  /me`** — protected; works via either cookie *or* `Authorization: Bearer`.

Backed by SQLite (so the project builds without Docker). Argon2id, HS256-signed JWTs by default (RS256 + JWKS is the E6.5 stretch in `jwt_rs256.rs`), signed cookies via `axum-extra::SignedCookieJar`, integration tests covering happy + sad paths.

## Green-bar checkpoint

```bash
cargo test -p auth-demo                           # all green
cargo run  -p auth-demo                           # listens on :3001

# Register, login, hit a protected endpoint via JWT
TOKEN=$(curl -s -X POST localhost:3001/auth/register \
            -H 'content-type: application/json' \
            -d '{"email":"a@b.com","password":"correct horse battery staple"}' \
        | jq -r .access_token)
curl -H "authorization: Bearer $TOKEN" localhost:3001/me

# And via cookie
curl -c /tmp/c.txt -s -X POST localhost:3001/auth/login \
     -H 'content-type: application/json' \
     -d '{"email":"a@b.com","password":"correct horse battery staple"}'
curl -b /tmp/c.txt localhost:3001/me
```

…and `make verify` is green.

## What's next

Phase 7 — **RBAC + ABAC**. Now that we know *who* the user is, we decide *what they can do*. Roles, permissions, ownership-based policies, audit logs.
