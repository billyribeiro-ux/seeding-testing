# Lesson 6.2 — Signed HttpOnly Session Cookies

> **Concept first:** the browser is a hostile environment. Cookies are how we authenticate without exposing tokens to JavaScript. Signed + HttpOnly + SameSite = the safe default.
> **Time:** 30 minutes.

## The shape

When a user logs in successfully:

1. Generate a 256-bit random session id.
2. Store a *hash* of the session id in the `sessions` table along with the user_id, IP, UA, issued_at, expires_at.
3. Sign the session id with a server secret using HMAC-SHA-256 and set a cookie:
   ```
   Set-Cookie: session=<signed-id>; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=2592000
   ```

When the user makes a subsequent request:

1. Read the cookie.
2. Verify the signature (rejects tampering).
3. Hash the id, look up the row in `sessions`.
4. If `revoked_at IS NULL` and `expires_at > now()`, attach the user to the request.

Logout = `UPDATE sessions SET revoked_at = NOW() WHERE id = $1`. One row, instant invalidation, no JWT-revocation pain.

## The flags, in detail

| Flag | What it does | When to set |
|---|---|---|
| `HttpOnly` | JavaScript can't read the cookie | **Always** for session cookies |
| `Secure` | Only sent over HTTPS | **Always** in production; set conditionally in dev to allow `http://localhost` |
| `SameSite=Lax` | Sent on same-site requests + top-level navigations; *not* on cross-site `<img src=…>` etc. | **Default** for session cookies |
| `SameSite=Strict` | Sent only on same-site requests; breaks "click a link from email" flows | Rare; use for super-sensitive cookies |
| `SameSite=None; Secure` | Sent everywhere | Cross-site embeds (rare; e.g. iframe widgets) |
| `Path` | Limits the cookie's scope to a URL prefix | Usually `/` |
| `Domain` | Limits the cookie's scope to a domain | Leave unset to use the request's exact host |
| `Max-Age` / `Expires` | Lifetime | Match the session row's `expires_at` |

## Storing what in the DB

```sql
CREATE TABLE sessions (
    id            BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    token_hash    BYTEA       NOT NULL UNIQUE,             -- SHA-256 of the random token
    user_id       BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_agent    TEXT,
    ip_inet       INET,
    issued_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at    TIMESTAMPTZ NOT NULL,
    revoked_at    TIMESTAMPTZ,
    rotated_at    TIMESTAMPTZ                                  -- last refresh
);
CREATE INDEX sessions_user_id_active_idx ON sessions (user_id) WHERE revoked_at IS NULL;
```

Two strong opinions:

- **Hash the token in the DB**, not the plaintext. Same logic as passwords: a leaked DB shouldn't grant an attacker stolen sessions.
- **Track `user_agent` and `ip_inet`** for the user-visible "your active sessions" page.

## CSRF mitigation

`SameSite=Lax` blocks the most common CSRF vectors: a malicious site cannot POST a form to yours and have the cookie sent.

For extra defense, especially on `SameSite=None` or when SameSite isn't enough, use **double-submit tokens**:

1. On login, set a non-HttpOnly cookie `csrf=<random>` alongside the session cookie.
2. The SvelteKit client reads `csrf` (it's not HttpOnly) and echoes it in a header on every state-changing request: `X-CSRF-Token: <random>`.
3. Server compares cookie value and header value. They must match.

An attacker can ride the session cookie via CSRF, but they cannot read the `csrf` cookie cross-site, so they can't set the header.

For *pure GET* endpoints (idempotent reads), CSRF doesn't apply. SameSite=Lax + correct verb usage covers most cases.

## Session rotation

After a privileged action (password change, 2FA enroll, payment-method-add), *rotate the session id*: invalidate the current row, issue a new id, set a fresh cookie. Even if an attacker stole the previous cookie, it's now useless.

```rust
let new_id = generate_session_token();
sqlx::query!(
    "UPDATE sessions SET revoked_at = NOW(), rotated_at = NOW() WHERE id = $1",
    current_session_id
).execute(&pool).await?;
insert_new_session(new_id, user_id).await?;
set_cookie(jar, new_id);
```

## In Axum

We use `axum-extra::extract::cookie::{SignedCookieJar, Cookie, Key}`:

```rust
let key = Key::derive_from(env::var("SESSION_SECRET")?.as_bytes());
let app = Router::new().route(...).with_state(AppState { key, ... });

async fn login(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(body): Json<LoginBody>,
) -> Result<(SignedCookieJar, Json<UserDto>), ApiError> {
    let user = authenticate(&s.pool, &body.email, &body.password).await?;
    let token = generate_session_token();
    insert_session(&s.pool, user.id, &token).await?;

    let cookie = Cookie::build(("session", token))
        .http_only(true)
        .secure(true)              // false in dev to allow http://localhost
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(30))
        .build();

    Ok((jar.add(cookie), Json(UserDto::from(user))))
}
```

The `SignedCookieJar` automatically signs outbound cookies and verifies inbound ones. Tamper with the cookie value and it's silently dropped.

## Why this matters

- **HttpOnly cookies can't be *exfiltrated* by XSS.** An attacker who injects JavaScript into your page cannot read the session token out of the cookie, so they can't steal it and replay it elsewhere — whereas a token in `localStorage` is trivially readable and gone. Be precise about the limit, though: HttpOnly does **not** make XSS harmless. While the script is running in the victim's origin, the browser still attaches the cookie to any request the script makes, so the attacker can ride the live session in-page. HttpOnly shrinks the blast radius (no durable token theft); it does not replace fixing the XSS.
- **Server-side sessions = instant logout.** Compare with JWT-in-cookie: revoking a JWT before its expiry requires a denylist or short lifetimes plus refresh tokens. With server-side sessions, you `UPDATE one row`.
- **Rotation on privileged actions** is a senior-engineer reflex. If an attacker stole a session through some other vector, that stolen token expires the moment the user updates their password.

## Green-bar checkpoint

- You can name the four cookie flags every session cookie should have and explain each.
- You can articulate why hashing the session token in the DB matters.
- You can sketch session-rotation-on-privileged-action.

Next: `lessons/03-jwt-access-and-refresh.md`.
