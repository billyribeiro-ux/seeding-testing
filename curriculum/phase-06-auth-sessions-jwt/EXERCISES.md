# Phase 6 — Exercises

Seven graded drills extending `projects/04-auth-demo`.

---

## E6.1 — Hash a password (Easy)

Outside the project, in a small scratch binary, take a plaintext password from
stdin, hash it with argon2id, print the PHC string. Then verify a second input
against it.

<details><summary>Answer</summary>

```rust
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use argon2::Argon2;
use std::io::{self, BufRead};

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let p1 = lines.next().unwrap().unwrap();
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(p1.as_bytes(), &salt).unwrap().to_string();
    println!("hash: {hash}");
    let p2 = lines.next().unwrap().unwrap();
    let parsed = PasswordHash::new(&hash).unwrap();
    let ok = Argon2::default().verify_password(p2.as_bytes(), &parsed).is_ok();
    println!("match: {ok}");
}
```
</details>

---

## E6.2 — Email verification (Medium)

Add an `email_verifications` table, a `POST /auth/verify-email/request` endpoint
that issues a token (stored hashed), and `POST /auth/verify-email/confirm` that
single-uses it. On success, `UPDATE users SET is_email_verified = 1`.

Add tests covering: happy path, double-use is rejected, expired token is rejected.

<details><summary>Answer (sketch)</summary>

```sql
CREATE TABLE email_verifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash BLOB NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    used_at TEXT
);
```

Handler:

```rust
async fn confirm(State(s): State<AppState>, Json(body): Json<ConfirmBody>) -> Result<StatusCode, ApiError> {
    let token_hash = sessions::token_hash(&body.token);
    let mut tx = s.pool.begin().await?;
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM email_verifications
         WHERE token_hash = ? AND used_at IS NULL AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')"
    ).bind(&token_hash).fetch_optional(&mut *tx).await?;
    let (id, user_id) = row.ok_or(ApiError::Unauthorized)?;
    sqlx::query("UPDATE email_verifications SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE users SET is_email_verified = 1 WHERE id = ?")
        .bind(user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
```
</details>

---

## E6.3 — Password reset (Medium) — shipped

Same machinery as verification, plus: on successful reset, *revoke every active
session for that user* in the same transaction. Always 204 for the request
endpoint regardless of whether the email exists.

A reference implementation lives in `projects/04-auth-demo/src/password_reset.rs`
and `projects/04-auth-demo/tests/password_reset.rs` (10 hermetic tests covering
the constant-time pad, the four-statement transaction, expired/replayed/unknown
tokens, weak-password rejection, and resend invalidation).

<details><summary>Answer (sketch)</summary>

Same `password_resets` table. Reset handler:

```rust
let mut tx = pool.begin().await?;
let row: Option<(i64, i64)> = sqlx::query_as(
    "SELECT id, user_id FROM password_resets
     WHERE token_hash = ? AND used_at IS NULL AND expires_at > ?"
).bind(...).fetch_optional(&mut *tx).await?;
let (id, user_id) = row.ok_or(ApiError::Unauthorized)?;
let new_hash = password::hash(&body.new_password)?;
sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?").bind(new_hash).bind(user_id).execute(&mut *tx).await?;
sqlx::query("UPDATE password_resets SET used_at = ? WHERE id = ?").bind(now).bind(id).execute(&mut *tx).await?;
sqlx::query("UPDATE sessions SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL").bind(now).bind(user_id).execute(&mut *tx).await?;
tx.commit().await?;
```
</details>

---

## E6.4 — Add TOTP 2FA (Medium)

Add `totp-rs` dep. Endpoints: `POST /auth/totp/enroll` (returns provisioning URI
and 10 recovery codes), `POST /auth/totp/verify` (confirms enrollment), and
modify `POST /auth/login` to require a second step when 2FA is enabled.

---

## E6.5 — Bump JWT from HS256 to RS256 (Stretch)

Generate an RSA keypair at startup (`rsa` crate or `ring`). Change `Algorithm`
and `EncodingKey`/`DecodingKey`. Add a `/.well-known/jwks.json` endpoint that
serves the public key as a JWK. Add a `kid` to the JWT header.

<details><summary>Hints</summary>

- `rsa::RsaPrivateKey::new(&mut rng, 2048)?` for the keypair.
- `EncodingKey::from_rsa_pem(...)` and `DecodingKey::from_rsa_pem(...)`.
- For JWK: convert modulus and exponent to base64url, build the JSON.

Real production uses pre-generated keys mounted from secrets, not per-boot generation.
</details>

---

## E6.6 — Rate-limit `/auth/login` (Medium) — shipped

Add the `tower_governor` crate. Apply a stricter limit to `/auth/login` than
the rest of the API: 5 requests per IP per minute. Add a test that asserts the
6th request gets `429` with a `Retry-After` header.

A reference implementation lives in `projects/04-auth-demo/src/lib.rs`
(`login_governor_layer`) and `projects/04-auth-demo/tests/login_rate_limit.rs`
(4 hermetic tests covering the 6th-request-is-429 assertion, per-IP isolation,
that the layer is scoped to `/auth/login`, and that disabling the limit truly
disables it). The limit is configured via `AppState::with_rate_limit` so the
default `AppState::new` constructor leaves it off — production wires
`RateLimit::production_defaults()` (5/min/IP) via `main.rs`.

---

## E6.7 — Refresh-token reuse detection (Stretch)

Persist refresh tokens in a `refresh_tokens` table (`jti, user_id, family_id,
issued_at, used_at, parent_jti`). On every refresh call: mark the presented
token used, link the new token as a child in the same family. If a token is
presented while already used → revoke the entire family, audit-log, return 401.

Add tests that simulate the legitimate flow and the attack flow (an attacker
replays a previously-used refresh).
