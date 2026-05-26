# Lesson 6.4 — Email Verification

> **Concept first:** new accounts are *unverified* until the user proves they own the email. Verification is a signed time-bounded single-use token sent over the only side-channel you trust (their inbox).
> **Time:** 20 minutes.

## Why verify?

Three reasons:

1. **Anti-abuse.** Bots can't read email; verification slows automated signups.
2. **Recovery channel.** Password reset only works if we know the email is reachable.
3. **Notification trust.** Receipt emails, security alerts, login-from-new-device — all require a verified channel.

Until verified, the account is in a *partial* state: it exists, the user can log in, but features are gated (see Phase 7 — RBAC will enforce "verified-only" rules).

## The token

A verification token is a small signed payload:

```rust
struct VerifyClaims {
    sub: i64,        // user id
    purpose: &'static str,   // "email-verify"
    iat: i64,
    exp: i64,        // 24 h
    nonce: String,   // unique per token, stored server-side for single-use
}
```

Sign it with the same JWT signing key (or a dedicated HMAC secret for email-only). Two patterns are common:

- **Stateless** — the token *is* the entire claim; server-side has no row. Pros: cheap. Cons: revoking before expiry is impossible.
- **Stateful** — server inserts `(user_id, nonce_hash, expires_at, used_at)`; the token only carries the nonce. Pros: single-use, revocable.

We use **stateful**. Verification clicks must be single-use; if the link leaks (forwarded email), the second click should fail.

## The flow

```
register:
  1. INSERT users (..., is_email_verified=false)
  2. nonce = random_bytes(32)
  3. INSERT email_verifications (user_id, nonce_hash, expires_at = now + 24h, used_at = NULL)
  4. email user a link: https://memberclub.test/verify?token=<base64url(nonce)>

verify:
  1. nonce = base64url_decode(token)
  2. SELECT * FROM email_verifications WHERE nonce_hash = sha256(nonce) AND used_at IS NULL AND expires_at > NOW()
  3. If found:
     a. UPDATE email_verifications SET used_at = NOW() WHERE id = $1
     b. UPDATE users SET is_email_verified = TRUE WHERE id = $1
     c. audit-log the event
  4. Else: render "link expired or already used"
```

Three habits:

- **Hash the nonce before storing.** Same logic as session tokens and passwords — leaked DB shouldn't grant verification.
- **Single transaction for steps 3a/3b/3c.** Verifying without flipping the flag is bad; flipping the flag without consuming the nonce is worse.
- **Idempotent client-side.** If the user clicks twice, the second click sees `used_at IS NOT NULL` and the page still shows "verified" — don't surface a scary error.

## Sending the email

In dev: MailHog catches everything at `localhost:8025`. In tests: `tracing::info!` the link instead of sending. In production: a transactional email provider (Postmark, SES, SendGrid).

The library to use varies; for a minimal Rust setup the `lettre` crate sends SMTP. We'll wire that in MemberClub's `infra/mail` crate.

## "Resend verification"

Add a `POST /auth/verify-email/resend` endpoint. Rate-limit aggressively (1 per minute per account). Invalidate previous unused tokens for the same user before issuing the new one — otherwise an attacker who briefly saw the first link can still use it.

## What if the user changes their email?

Don't immediately update `users.email`. Issue a verification token for the *new* address; on click, swap. Bonus: also email the old address ("your email was changed to X — if this wasn't you, click here to revert").

## Why this matters

- **Verification is a low-effort, high-leverage anti-abuse measure.** Every consumer SaaS does it.
- **Stateful tokens with single-use enforcement** prevent forwarded-link replay.
- **The "atomically flip the flag and consume the token" pattern** is the same shape we'll use for password reset (next lesson) and for accepting team invites.

## Green-bar checkpoint

- You can sketch the `email_verifications` table.
- You can write the verify-and-flip transaction.
- You can articulate when to invalidate previous tokens on resend.

Next: `lessons/05-password-reset.md`.
