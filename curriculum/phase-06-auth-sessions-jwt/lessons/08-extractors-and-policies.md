# Lesson 6.8 — The `AuthenticatedUser` Extractor

> **Concept first:** every protected endpoint asks the same question: "who is calling me?" An Axum extractor turns that question into a handler signature. One extractor handles *both* cookie and JWT transports.
> **Time:** 25 minutes.

## The shape

```rust
async fn me(user: AuthenticatedUser) -> Json<UserDto> {
    Json(UserDto::from(user.0))
}
```

The handler doesn't know whether the user authenticated via cookie or JWT. The extractor handles both. If neither succeeds, the request gets a `401 Unauthorized` *before* the handler runs.

## The dual-mode extractor

```rust
pub struct AuthenticatedUser(pub User);

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: AppStateGetter + Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Try Authorization: Bearer <jwt>
        if let Some(token) = bearer_token(&parts.headers) {
            let claims = state.app_state().jwt.verify(token)?;
            let user = state.app_state().users.find(claims.sub.parse()?).await?;
            return Ok(Self(user));
        }

        // 2. Try the session cookie
        let jar = SignedCookieJar::from_headers(&parts.headers, state.app_state().cookie_key.clone());
        if let Some(session_cookie) = jar.get("session") {
            let session = state.app_state().sessions.find_by_token(session_cookie.value()).await?;
            if session.is_active() {
                let user = state.app_state().users.find(session.user_id).await?;
                return Ok(Self(user));
            }
        }

        Err(ApiError::Unauthorized)
    }
}
```

Three habits to copy:

1. **Try bearer first, cookie second.** Bearer is explicit; cookie is implicit. If a client sets both, the explicit choice wins.
2. **Validate freshness on cookie path.** `session.is_active()` checks `revoked_at IS NULL AND expires_at > NOW()`.
3. **Always return the same `ApiError::Unauthorized` for any miss.** Don't leak whether it was "no token," "expired token," or "revoked session" — same response.

## Variants for stricter requirements

```rust
pub struct VerifiedUser(pub User);    // requires is_email_verified
pub struct StepUpUser(pub User);      // requires recent TOTP

impl FromRequestParts for VerifiedUser {
    /* same as AuthenticatedUser, but reject if !user.is_email_verified */
}
```

Each is a layer of strictness on top of the previous. Handlers pick the strictest extractor that matches their need.

## Optional auth — when the endpoint behaves differently for anonymous users

A public profile page might want to show "Follow" or "Unfollow" depending on whether the viewer follows the target. Use `Option<AuthenticatedUser>`:

```rust
async fn profile(viewer: Option<AuthenticatedUser>, Path(handle): Path<String>) -> Result<Json<Profile>, ApiError> {
    let target = users.find_by_handle(&handle).await?;
    let following = match viewer {
        Some(v) => follows.exists(v.0.id, target.id).await?,
        None    => false,
    };
    Ok(Json(Profile { user: target.into(), following }))
}
```

The `Option<T>` wrapper makes the extractor non-fatal: failure to extract = `None`, not `401`.

## What goes *into* the extracted user

A common mistake: extract too much. Two approaches:

- **Lightweight:** the extractor returns `UserId(i64)`. The handler fetches the user from the pool if it needs the row. Cheap (one DB call instead of two), but every handler that needs the user object does an extra query.
- **Heavyweight:** the extractor fetches the full row. Every protected endpoint pays one DB call up front, but the handler is straightforward.

We do *heavyweight* in the capstone — clarity over micro-optimization. Watch for it under load (see Phase 11 — we add caching).

## Tests

The extractor is *the* thing tests target — it's the security perimeter.

```rust
#[tokio::test]
async fn requires_authentication() {
    let app = app().await;
    let res = app.oneshot(Request::get("/me").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accepts_jwt() {
    let (app, token, _user) = setup_with_jwt().await;
    let res = app.oneshot(
        Request::get("/me").header("authorization", format!("Bearer {token}")).body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn accepts_session_cookie() {
    let (app, cookie, _user) = setup_with_cookie().await;
    let res = app.oneshot(
        Request::get("/me").header("cookie", cookie).body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn rejects_revoked_session() {
    let (app, cookie, user) = setup_with_cookie().await;
    revoke_all_sessions(user.id).await;
    let res = app.oneshot(/* ... */).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
```

The extractor's tests double as the *authentication contract*. Anyone reading the test file learns what auth allows, refuses, and how each method behaves.

## Why this matters

- **A single extractor across cookie + JWT** is what makes the dual-mode pattern usable. Handlers don't fork.
- **Variants (`VerifiedUser`, `StepUpUser`)** push enforcement into types. You can't accidentally call a step-up-required handler from a non-step-up context.
- **The extractor is your security perimeter.** Test the hell out of it.

## Green-bar checkpoint

- You can sketch the dual-mode extractor (bearer → cookie → 401).
- You can articulate the trade-off between lightweight (id-only) and heavyweight (full user) extraction.
- You can write a test that asserts a revoked session is rejected.

Next: `lessons/09-build-auth-demo.md`.
