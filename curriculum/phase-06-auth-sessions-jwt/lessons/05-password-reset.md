# Lesson 6.5 — Password Reset

> **Concept first:** password reset is the same machinery as email verification — a signed, time-bounded, single-use token — with two extra rules: don't leak whether the email exists, and invalidate every active session on success.
> **Time:** 20 minutes.

## The flow

```
request reset:
  POST /auth/forgot-password { email }
  ────────────────────────────────────
  Always respond 204 No Content (or 200 with a generic "if that email exists, we sent a link")
  ────────────────────────────────────
  Internally:
    if user exists:
      issue a reset token, INSERT into password_resets, email it
    if not:
      do nothing — but take roughly the same time

reset:
  POST /auth/reset-password { token, new_password }
  ────────────────────────────────────
  - Validate token (same pattern as email verification: not used, not expired, matches stored hash).
  - Hash new_password (argon2id).
  - Single transaction:
      UPDATE users SET password_hash = $1 WHERE id = $2
      UPDATE password_resets SET used_at = NOW() WHERE id = $3
      UPDATE sessions SET revoked_at = NOW() WHERE user_id = $2     -- kill every active session
      INSERT INTO audit_logs (...)
    COMMIT
  - Email a notification to the user: "your password was just changed. If this wasn't you, contact support."
```

## Why we always 204

If your endpoint returns "user not found" when the email isn't registered, an attacker can enumerate your user base:

```
$ curl -X POST /auth/forgot-password -d '{"email":"a@b.com"}'  # 404
$ curl -X POST /auth/forgot-password -d '{"email":"x@y.com"}'  # 204
# → a@b.com isn't registered; x@y.com is
```

**Always respond with the same status and body**, regardless of whether the email exists. Send the email only when it does.

## Constant-time response

Even with the same status code, the response *time* leaks information: hashing argon2 takes 100 ms; not hashing is instantaneous. Fix:

```rust
async fn forgot_password(...) -> impl IntoResponse {
    let started = Instant::now();
    let result = handle_internally().await;
    // Pad to a fixed budget so timing leaks nothing.
    let target = Duration::from_millis(250);
    if let Some(rest) = target.checked_sub(started.elapsed()) {
        tokio::time::sleep(rest).await;
    }
    let _ = result; // ignore for response
    StatusCode::NO_CONTENT
}
```

Subtle; worth the few lines.

## Revoke sessions on success

This is the rule juniors forget. When a password is reset, *every* active session for that user must be invalidated. Otherwise: an attacker who phished the user's old password (and was active in a session) keeps the session alive even though the password changed.

```sql
UPDATE sessions SET revoked_at = NOW() WHERE user_id = $1;
```

One row update, all sessions dead.

## Notification email

After a successful reset, email the user (at their verified address):

> Your MemberClub password was just changed on May 26, 2026, 16:50 UTC from IP 198.51.100.42. If this wasn't you, [contact support immediately].

A wrong reset is recoverable if the user notices fast; a silent reset is account theft.

## Throttle the reset endpoint

A reset endpoint that can be hit 1000x/sec is a spam vector (we'd be sending mail per call). Rate-limit by IP and by email:

- 5 attempts per IP per hour.
- 3 attempts per email per day.

Returns 429 with `Retry-After` past the limit. Discussed in detail in Lesson 6.7.

## "Reset works without an email" — magic links

For users who never set a password (OAuth-only) or who want a passwordless flow, a *magic link* is a reset token that on click *logs the user in directly* without changing the password. Same machinery, different completion. Some users prefer it; many security-paranoid auditors don't. We support both behind a feature flag.

## Why this matters

- **Enumeration is a real vector.** Big consumer sites have been caught leaking subscriber lists this way.
- **Session revocation on reset** closes a loop juniors miss and seniors check for first.
- **The audit + notification email** turn "silent compromise" into "user knows immediately."

## Green-bar checkpoint

- You can articulate why `/auth/forgot-password` should be `204` for both cases.
- You can write the four-statement transaction that completes a password reset.
- You can describe the constant-time padding trick.

Next: `lessons/06-2fa-totp.md`.
