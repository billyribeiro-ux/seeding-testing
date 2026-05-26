# Lesson 6.6 — 2FA via TOTP

> **Concept first:** Two-Factor Authentication adds *something you have* (a phone running an authenticator app) on top of *something you know* (the password). TOTP is the open standard for it.
> **Time:** 20 minutes.

## What TOTP is

TOTP (Time-based One-Time Password, RFC 6238) is a 6-digit code derived from:

- A shared secret (32 bytes random, base32-encoded).
- The current Unix time, divided into 30-second steps.

The authenticator app (Google Authenticator, 1Password, Authy, …) and the server both compute the same code at the same time. They never communicate; the secret was exchanged once at enrollment.

A code is valid for ~30 s. The server allows a small window (±1 step) for clock skew between phone and server.

## Enrollment flow

```
1. Server generates a random 32-byte secret.
2. Server stores it (encrypted at rest) under user.totp_secret.
3. Server returns a `provisioning_uri`:
     otpauth://totp/MemberClub:alice@b.com?secret=BASE32SECRET&issuer=MemberClub
4. Client renders that URI as a QR code; user scans it with their authenticator app.
5. User types the 6-digit code the app shows.
6. Server verifies the code; on success, flips user.totp_enabled = TRUE.
```

If step 6 fails, the secret is *not* committed — leave `totp_enabled = FALSE` and let the user retry.

## Login with 2FA enabled

Two steps:

1. Email + password → server returns `{ "next": "totp_required", "session_pending_id": "..." }`.
2. User submits 6-digit code → server verifies and *now* issues the session cookie / JWT.

Cookies / tokens are issued only after both factors check out.

## Recovery codes

Phones get lost. Without recovery codes, a lost device = locked-out account = customer support ticket. The standard mitigation: at enrollment, generate 8–12 recovery codes (each 10-char random), show them once, store *hashed* copies.

```
Save these in a password manager — each can be used once if you lose your phone:
  3a7f-2b1c-8e94
  ...
```

A recovery code is a one-time bypass for the TOTP step. After use, it's marked consumed.

## Step-up authentication

For *very* sensitive actions (changing email, viewing/regenerating recovery codes, deleting account, viewing payment methods), require the user to re-prove TOTP within the same session:

- Set `users.totp_last_verified_at` on every successful TOTP check.
- Sensitive endpoints require `totp_last_verified_at >= NOW() - INTERVAL '15 minutes'`; otherwise return `403 Step-Up Required` and the UI prompts for the code again.

OAuth 2.0 calls this Step-Up Authentication; it's an RFC (8176) in 2026.

## The `totp-rs` crate

```toml
[dependencies]
totp-rs = "5"
```

```rust
use totp_rs::{Algorithm, TOTP, Secret};

let secret = Secret::Raw(secret_bytes.to_vec());
let totp = TOTP::new(
    Algorithm::SHA1, 6, 1, 30,
    secret.to_bytes().unwrap(),
    Some("MemberClub".into()),
    "alice@b.com".into(),
).unwrap();

let uri = totp.get_url();                 // for the QR code
let valid = totp.check_current(&"123456").unwrap();
```

The 1 in the third argument is the *skew window* — accept codes from the previous step too, to account for the user typing during a clock tick.

## Why SHA-1?

The TOTP RFC defaults to SHA-1; every authenticator app on Earth supports it. SHA-256 and SHA-512 are allowed but support is patchy. Stick with SHA-1 for TOTP — the algorithm is the secret-derivation, not a password hash. SHA-1 is fine here.

## What to *not* do

- **Don't store the secret unencrypted.** If your DB leaks, an attacker has both factors. Encrypt with a service-level key (`SecretBox` / `XChaCha20-Poly1305`).
- **Don't send TOTP codes via SMS.** SIM-swap attacks are real. SMS is a fallback at best, never primary.
- **Don't allow disabling 2FA without a fresh TOTP code.** Otherwise an attacker who briefly compromises a session disables 2FA and walks off with the account.

## Why this matters

- **2FA is the single highest-impact security control after argon2.** Credential stuffing (the most common attack on logins) fails completely.
- **Recovery codes turn 2FA from a UX disaster into a feature.**
- **Step-up gates the most damaging actions even after the session is compromised.**

## Green-bar checkpoint

- You can sketch the TOTP enrollment flow.
- You can articulate why we don't issue a session cookie after only the password check when TOTP is enabled.
- You can describe step-up auth and one concrete action that should require it.

Next: `lessons/07-rate-limiting-and-lockout.md`.
