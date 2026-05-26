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

## What ships

- **Argon2id** password hashing with a sentinel-hash timing defense on
  unknown-email logins (no enumeration).
- **Signed HttpOnly session cookies** (`axum-extra` `SignedCookieJar`)
  + server-side `sessions` row keyed by SHA-256 of the token.
- **HS256 JWT access (15 min) + refresh (30 d)** tokens with a
  `purpose` claim discriminating the two.
- **Dual-mode `AuthenticatedUser` extractor** — Bearer first, then
  signed cookie. Same handler signature for both.
- **TOTP 2FA** (Phase 6.6) — enrollment, confirmation, login
  second-factor enforcement, 8 single-use recovery codes, disable
  with step-up. Backed by `totp-rs` (RFC 6238 SHA-1 / 6 digits /
  30 s step / ±1 skew).

## What's deliberately *not* here (and lives in the lessons + EXERCISES)

- **Email verification + password reset** — Lessons 6.4 + 6.5; E6.2 / E6.3.
- **Rate limiting + lockout** — Lesson 6.7; E6.6.
- **RS256 + JWKS** — this demo uses HS256 for simplicity. Lesson 6.3 documents
  the RS256 production pattern; E6.5 swaps it in.

Each is a focused exercise — the capstone establishes the *bones* of the auth
service; the exercises layer on additional production-grade controls.

## Endpoints

| Method | Path | Notes |
|---|---|---|
| GET | `/healthz` | Liveness |
| POST | `/auth/register` | argon2 hash, validates email and password |
| POST | `/auth/login` | Issues both a Set-Cookie and a `LoginResponse { access_token, refresh_token }`. If TOTP is enabled, also requires `totp` or `recovery_code` in the body |
| POST | `/auth/logout` | Revokes the cookie session row; clears the cookie |
| POST | `/auth/refresh` | Issues a new access + refresh pair (rotation); future exercise: reuse detection |
| POST | `/auth/totp/enroll` | Authenticated; returns `secret`, `provisioning_uri`, and 8 single-use `recovery_codes` (shown once) |
| POST | `/auth/totp/confirm` | Authenticated; submit the 6-digit code displayed in the authenticator app to finalize enrollment |
| POST | `/auth/totp/disable` | Authenticated + step-up; requires a fresh 6-digit code or one recovery code |
| GET | `/me` | Returns the authenticated user (cookie or bearer) |

### Sample TOTP flow

```bash
# Register + log in once
TOKEN=$(curl -s -X POST localhost:3001/auth/login -H 'content-type: application/json' \
        -d '{"email":"alice@example.com","password":"correct horse battery staple"}' | jq -r .access_token)

# Enroll → renders provisioning_uri as a QR code; user scans with Authy / 1Password / etc.
curl -s -X POST localhost:3001/auth/totp/enroll -H "authorization: Bearer $TOKEN" | jq

# Confirm with the 6-digit code shown in the app
curl -i -X POST localhost:3001/auth/totp/confirm \
    -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
    -d '{"code":"123456"}'                                   # → 204

# Now every login requires the second factor:
curl -i -X POST localhost:3001/auth/login -H 'content-type: application/json' \
    -d '{"email":"alice@example.com","password":"correct horse battery staple"}'  # → 401 (TotpRequired)

curl -i -X POST localhost:3001/auth/login -H 'content-type: application/json' \
    -d '{"email":"alice@example.com","password":"correct horse battery staple","totp":"123456"}'  # → 200
```
