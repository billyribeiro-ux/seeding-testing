# Lesson 6.9 — Build `auth-demo` Step by Step

> **The capstone of Phase 6.** Walk through `projects/04-auth-demo/` and understand each module.
> **Time:** 60 minutes.

## What's actually in the project

```
projects/04-auth-demo/
├── Cargo.toml
├── README.md
├── migrations/20260526130000_init.sql      ← users, sessions, audit_logs
└── src/
    ├── main.rs            ← binary, listens on 127.0.0.1:3001
    ├── lib.rs             ← Router, handlers, ApiError, AuthenticatedUser
    ├── password.rs        ← argon2id hash/verify
    ├── sessions.rs        ← session insert/find/revoke, token_hash
    └── jwt.rs             ← HS256 access + refresh tokens
└── tests/
    └── auth.rs            ← 11 integration tests
```

The split mirrors the responsibilities. Each module is independently testable.

## Endpoints

| Method | Path | Function |
|---|---|---|
| GET | `/healthz` | Liveness |
| POST | `/auth/register` | argon2 hash, validates, inserts |
| POST | `/auth/login` | Issues *both* a cookie *and* a JWT pair |
| POST | `/auth/logout` | Revokes the session row, clears the cookie |
| POST | `/auth/refresh` | Issues new access + refresh (rotation, no reuse-detect yet) |
| GET | `/me` | Requires auth (cookie *or* bearer) |

## How the dual-mode extractor works

```rust
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        // 1. Authorization: Bearer
        if let Some(token) = bearer_from(&parts.headers) {
            let claims = state.jwt.verify_access(&token)?;
            let user_id: i64 = claims.sub.parse().map_err(|_| ApiError::Unauthorized)?;
            return Ok(Self(fetch_user(&state.pool, user_id).await?.ok_or(ApiError::Unauthorized)?));
        }
        // 2. Signed session cookie
        let jar = SignedCookieJar::from_headers(&parts.headers, state.cookie_key.clone());
        if let Some(cookie) = jar.get("session") {
            if let Some(session) = sessions::find_active(&state.pool, cookie.value()).await? {
                return Ok(Self(fetch_user(&state.pool, session.user_id).await?.ok_or(ApiError::Unauthorized)?));
            }
        }
        Err(ApiError::Unauthorized)
    }
}
```

Bearer first, cookie second; everything else is `401`.

## Where argon2 is used

```rust
let hash = password::hash(&body.password)?;              // on register
let ok   = password::verify(&body.password, &user.password_hash);  // on login
```

The PHC-formatted string includes the parameters, so future-Argon2-defaults transparently verify older hashes (see Lesson 6.1 on re-hashing-on-login as an upgrade).

## Where the cookie is set

```rust
Cookie::build(("session", token))
    .http_only(true)
    .secure(cfg!(not(debug_assertions)))
    .same_site(SameSite::Lax)
    .path("/")
    .max_age(time::Duration::days(30))
    .build()
```

Four flags every session cookie needs. `secure` is gated behind `cfg!` so tests against `http://localhost` work in dev.

## How `sessions::token_hash` defends a DB leak

```rust
pub fn token_hash(token: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().to_vec()
}
```

The cookie carries the *plaintext* token. The DB stores SHA-256. An attacker with the DB cannot forge a cookie because they only have the hash; an attacker with the cookie cannot find the user's session id without the DB. Both pieces have to leak to compromise a session.

## Why HS256 instead of RS256 in the demo

The lessons describe RS256 (asymmetric keys, JWKS, key rotation). The capstone uses HS256 because:

1. **Boot time.** No RSA key generation; no PEM parsing; no JWKS endpoint to mock.
2. **Pedagogical clarity.** All the *behavior* — claims, `iss`/`aud`/`exp`/`nbf`/`jti`, `purpose`, leeway — is identical. RS256 is just a different signer.
3. **One exercise** (E6.5) bumps the demo to RS256 + JWKS. Same code shape, swapped algorithm.

Production MemberClub uses RS256 from Phase 8 onward.

## Tests as the contract

Take a slow read of `tests/auth.rs`. Every guarantee the service makes is asserted there:

- `register_rejects_weak_password` → password validation runs.
- `register_then_register_same_email_conflicts` → UNIQUE constraint surfaces as 409.
- `login_for_unknown_email_is_401_not_404` → no enumeration.
- `login_and_me_via_bearer` → the JWT path works.
- `login_and_me_via_cookie` → the cookie path works.
- `logout_invalidates_cookie` → revocation actually revokes.
- `me_without_any_auth_is_401` → guarded routes are guarded.

If you change the service and any of these fails, you broke an invariant — *intentionally or not*.

## Why this matters

- **The dual-mode pattern collapses cookie + JWT into one mental model.** Handlers are the same; only the front door differs.
- **A unique constraint maps cleanly to 409 Conflict.** Don't open-code "does email exist?" — let the DB tell you.
- **`401` for both wrong-password and unknown-email** is the difference between "easy to enumerate" and "frustrating to attack."

## Green-bar checkpoint

- `cargo test -p auth-demo` shows 18 passed.
- You can curl every endpoint and round-trip a login → `/me` with both transports.
- You can articulate three reasons the demo uses HS256 and the production path uses RS256.

Phase 6 is complete. Phase 7 — **RBAC + ABAC** — now that we know *who*, we decide *what they can do*.
