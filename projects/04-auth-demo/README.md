# projects/04-auth-demo

Phase 6 capstone — argon2 password hashing + signed HttpOnly session cookies +
HS256 JWT access/refresh tokens, behind one Axum service. The
`AuthenticatedUser` extractor accepts either transport, so handlers don't fork.

## What it teaches

- `argon2id` PHC-formatted hashes with sane defaults.
- `axum-extra::SignedCookieJar` for HMAC-signed HttpOnly session cookies.
- `jsonwebtoken` with access + refresh, separated by a `purpose` claim.
- A dual-mode `AuthenticatedUser` extractor that tries `Authorization: Bearer`
  first, then falls back to the signed cookie.
- Idempotent migration with `sqlx::migrate!`.
- Audit-log row written inside the same transaction as the state change.

## Run it

```bash
cargo run -p auth-demo
# listens on 127.0.0.1:3001

# Register
TOKEN=$(curl -s -X POST localhost:3001/auth/register \
            -H 'content-type: application/json' \
            -d '{"email":"alice@example.com","password":"correct horse battery staple"}' \
        | jq -r .id)

# Log in (returns access_token + refresh_token + Set-Cookie)
curl -i -X POST localhost:3001/auth/login \
     -H 'content-type: application/json' \
     -d '{"email":"alice@example.com","password":"correct horse battery staple"}'

# /me via JWT
TOKEN=$(curl -s -X POST localhost:3001/auth/login \
        -H 'content-type: application/json' \
        -d '{"email":"alice@example.com","password":"correct horse battery staple"}' \
     | jq -r .access_token)
curl -H "authorization: Bearer $TOKEN" localhost:3001/me

# /me via cookie
curl -c /tmp/c.txt -s -X POST localhost:3001/auth/login \
     -H 'content-type: application/json' \
     -d '{"email":"alice@example.com","password":"correct horse battery staple"}'
curl -b /tmp/c.txt localhost:3001/me
```

## Test it

```bash
cargo test -p auth-demo
cargo clippy -p auth-demo -- -D warnings
```

Or `make verify` from the repo root.

## What's deliberately *not* here (and lives in the lessons + EXERCISES)

- **TOTP / 2FA** — Lesson 6.6 documents the flow; E6.4 implements it.
- **Email verification + password reset** — Lessons 6.4 + 6.5; E6.2 / E6.3.
- **Rate limiting + lockout** — Lesson 6.7; E6.6.
- **RS256 + JWKS** — this demo uses HS256 for simplicity. Lesson 6.3 documents
  the RS256 production pattern; E6.5 swaps it in.

Each is a focused exercise — the capstone establishes the *bones* of the auth
service; the exercises layer on production-grade controls.

## Endpoints

| Method | Path | Notes |
|---|---|---|
| GET | `/healthz` | Liveness |
| POST | `/auth/register` | argon2 hash, validates email and password |
| POST | `/auth/login` | Issues both a Set-Cookie and a `LoginResponse { access_token, refresh_token }` |
| POST | `/auth/logout` | Revokes the cookie session row; clears the cookie |
| POST | `/auth/refresh` | Issues a new access + refresh pair (rotation); future exercise: reuse detection |
| GET | `/me` | Returns the authenticated user (cookie or bearer) |
