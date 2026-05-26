# Lesson 6.3 — JWT Access + Refresh Tokens

> **Concept first:** JWTs are bearer tokens for clients that can store secrets safely (mobile apps, CLIs, server-to-server). Short-lived access tokens + single-use refresh tokens give you authentication without server-side state on the hot path.
> **Time:** 35 minutes.

## What a JWT is, in three sentences

A JWT (JSON Web Token) is a base64url-encoded JSON header + payload, *signed* with either a shared secret (HMAC) or a private key (RSA / EdDSA). Anyone with the matching key can *verify* the signature. The payload contains claims like `sub` (subject = user id) and `exp` (expiry).

A JWT is not encrypted (unless you use JWE, which we don't). The payload is readable; the signature only proves authenticity.

## Why two tokens

If a JWT is valid for 30 days, an attacker who steals it gets 30 days of access. If it's valid for 5 minutes, the attacker has 5 minutes — *but* the user would have to log in every 5 minutes. The solution is two tokens:

- **Access token** — short-lived (15 minutes), used on every API call.
- **Refresh token** — long-lived (30 days), used *only* to get a new access token.

Stealing an access token grants 15 minutes of API access. Stealing a refresh token is more damaging — so we also rotate refresh tokens *on every use* and detect reuse.

## Signing algorithm: RS256 (or EdDSA)

| Algorithm | When to use |
|---|---|
| **HS256** (HMAC) | Single service or trusted set; symmetric secret. Simple. |
| **RS256** (RSA) | Multi-service; verifier has only the public key. |
| **EdDSA** (Ed25519) | Same idea as RS256 but faster and smaller keys. |

We use **RS256** for MemberClub:

- The auth service signs with the private key.
- Every other service verifies with the public key (which we publish at `/.well-known/jwks.json`).
- Compromising one service doesn't compromise auth.

## Claims

Standard claims (registered by RFC 7519):

| Claim | Meaning |
|---|---|
| `iss` | Issuer (your service's identifier) |
| `aud` | Audience (which service is allowed to consume this) |
| `sub` | Subject (the user id, as a string) |
| `iat` | Issued-at (Unix seconds) |
| `nbf` | Not-before (Unix seconds; reject before this time) |
| `exp` | Expiry (Unix seconds) |
| `jti` | JWT ID — unique per token, used for revocation / reuse detection |

We add custom claims for our domain:

```json
{
  "iss":  "memberclub-api",
  "aud":  "memberclub-api",
  "sub":  "42",
  "iat":  1748275200,
  "nbf":  1748275200,
  "exp":  1748276100,
  "jti":  "01HVZ0Q3R6XJYW0K8X2D9YQ1T1",
  "scope": "read:notes write:notes",
  "user": { "email": "alice@b.com", "is_admin": false }
}
```

Three rules:

1. **`exp` is non-negotiable.** Every JWT must expire.
2. **Validate `iss` and `aud`.** Don't accept a token issued by another service.
3. **Allow a small `leeway`** (e.g. 60 s) for clock skew between issuer and verifier.

## Refresh rotation + reuse detection

The most important security property: if a refresh token is *ever used twice*, **revoke the entire refresh chain** and force a fresh login. Here's how:

1. Each refresh token has a unique `jti`.
2. On issue, store `(jti, user_id, family_id, issued_at, expires_at, used_at = NULL)` in `refresh_tokens`.
3. When a refresh token is presented:
   - Look it up by `jti`.
   - If `used_at IS NOT NULL`, *this token was already used* — that's reuse, meaning either a network retry or a thief.
     - Revoke the entire `family_id`. The legitimate user will be logged out; better than the attacker continuing.
     - Audit-log the event.
   - Otherwise, mark `used_at = NOW()`, issue a *new* refresh token in the same family, return it.

```
familyA:  refreshA1 -> refreshA2 -> refreshA3 -> ...
```

If an attacker uses `refreshA2` while the user already used it to get `refreshA3`, the system detects the reuse and revokes the whole family. The user gets logged out; the attacker is locked out.

This pattern is the IETF OAuth 2.0 best practice as of 2026.

## Key rotation via `kid`

Eventually you'll want to rotate keys. The trick: include a `kid` (key id) in the JWT header:

```json
{ "alg": "RS256", "typ": "JWT", "kid": "2026-05" }
```

Publish multiple keys at `/.well-known/jwks.json`:

```json
{ "keys": [
    { "kid": "2026-04", "alg": "RS256", "kty": "RSA", "use": "sig", "n": "...", "e": "AQAB" },
    { "kid": "2026-05", "alg": "RS256", "kty": "RSA", "use": "sig", "n": "...", "e": "AQAB" }
]}
```

Verifiers look up the correct key by `kid`. Rolling out a new key is: publish JWKS → wait one max-token-lifetime → start signing with the new key. Old tokens still verify against the old key.

## In Rust: `jsonwebtoken`

```toml
[dependencies]
jsonwebtoken = "9"
```

```rust
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, encode, decode};

#[derive(Serialize, Deserialize)]
struct Claims {
    iss: String, aud: String, sub: String,
    iat: i64, nbf: i64, exp: i64,
    jti: String,
    scope: String,
}

fn issue(user_id: i64, signing_key: &EncodingKey) -> Result<String, AuthError> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        iss:   "memberclub-api".into(),
        aud:   "memberclub-api".into(),
        sub:   user_id.to_string(),
        iat:   now, nbf: now, exp: now + 15 * 60,
        jti:   ulid::Ulid::new().to_string(),
        scope: "read:notes write:notes".into(),
    };
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("2026-05".into());
    encode(&header, &claims, signing_key).map_err(Into::into)
}

fn verify(token: &str, verifying_key: &DecodingKey) -> Result<Claims, AuthError> {
    let mut v = Validation::new(Algorithm::RS256);
    v.set_audience(&["memberclub-api"]);
    v.set_issuer(&["memberclub-api"]);
    v.leeway = 60;
    let data = decode::<Claims>(token, verifying_key, &v)?;
    Ok(data.claims)
}
```

## Why this matters

- **Short-lived JWT + rotating refresh = the modern API auth standard.** OAuth 2.0, OpenID Connect, every major identity provider in 2026.
- **Reuse detection turns refresh tokens from a liability into a tripwire.** Stolen tokens trigger lockouts instead of silent compromise.
- **Key rotation via `kid` is graceful.** No "rotate at midnight" downtime.

## Green-bar checkpoint

- You can name the claims a JWT should always have and explain each.
- You can sketch the refresh-rotation-with-reuse-detection algorithm.
- You can articulate why we use RS256 over HS256 in a multi-service architecture.

Next: `lessons/04-email-verification.md`.
