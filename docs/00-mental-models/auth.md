# Mental Model: AuthN ≠ AuthZ

> *Authentication answers "who are you?". Authorization answers "what
> can you do?". Conflating them is the most common security bug class.*

## The two questions

| Question | What it produces | Where it lives |
|---|---|---|
| **Who are you?** (AuthN) | A user identity | Cookie / JWT / SAML / OAuth callback |
| **What can you do?** (AuthZ) | An allow/deny decision | A policy function on `(subject, action, resource, context)` |

Most failures happen because someone answers AuthN ("they're logged in")
and assumes AuthZ ("therefore they can do this"). The two are
independent.

## The dual-mode AuthN pattern

Browsers and API clients have different threat models. Use different
credentials, one extractor:

| Audience | Credential | Why |
|---|---|---|
| Browser (SvelteKit web) | Signed HttpOnly cookie + server-side session row | XSS can't read it; logout is instant (one `UPDATE`) |
| API client (mobile, CLI, partner) | RS256 JWT (15-min access) + refresh token | Stateless reads at scale; the client can store secrets safely |

A single `AuthenticatedUser` extractor (Phase 6.8) tries the bearer
token, then falls back to the signed cookie. Handlers don't fork on
transport. The contract is *one* — `user: AuthenticatedUser` in the
signature — and the security perimeter is *one* file.

## Things AuthN must never do

- **Tell you whether an email exists.** `POST /auth/forgot-password`
  returns 204 whether the email is registered or not. Otherwise you've
  built an enumeration vector.
- **Tell you whether the password was wrong vs the email was wrong.**
  Both fail as `401 invalid credentials`. Don't be helpful here.
- **Trust the client about *who* it is.** Validate every claim
  server-side. JWT `sub` is just a number; a user record lookup confirms
  it.

## Things AuthZ must always do

- **Deny by default.** If no policy matches, the answer is no.
- **Apply admin override first.** The remaining logic stays simple.
- **Name the denial reason.** `Forbidden::NotOwner`, not a string.
- **Audit every privileged success.** A row in `audit_logs` in the same
  transaction as the state change.

## Step-up authentication

Some actions are too sensitive for "you're logged in" to be enough:

- Changing email.
- Viewing recovery codes.
- Deleting the account.
- Impersonating another user (admin only).
- Viewing PII fields.

For these, require *recent* TOTP — within 15 minutes of the request.
The user's session is fine; we just want a fresh second factor before
the damaging action.

```rust
require_step_up(s, ctx, Duration::minutes(15))?;
```

Step-up gates the worst-case damage even after a stolen session.

## "We have SSO" doesn't excuse anything

SAML / OIDC / "Login with Google" are *AuthN providers*. They tell you
who the user is. They do not tell you *what they can do*. The AuthZ
layer remains your job — RBAC + ABAC against your own resources.

A common SSO mistake: trusting a group claim from the IdP to grant
admin. Today that's fine; tomorrow IT changes a group definition and
suddenly half the company is your admin. Mirror the IdP's groups into
your own `user_roles` table and grant explicitly.

## The session vs token decision matrix

| Property | Session cookie | JWT |
|---|---|---|
| Revocation latency | Immediate (one `UPDATE`) | Up to access-token TTL |
| Stateless reads | No (DB lookup per request) | Yes |
| XSS exposure | Zero (HttpOnly) | High (if stored in JS) |
| Works on mobile | Awkward (cookie jar) | Natural |
| Works on CLI | Awkward | Natural |
| Cross-origin cost | CORS pain | One header |

For MemberClub web: cookie. For the mobile app and CLI: JWT. For
partner integrations: API key (a special kind of long-lived JWT with
restricted scope).

## The audit channel

Every privileged AuthN event writes a row in `audit_logs` *inside the
same transaction* as the state change:

- Login success / failure (failure with the IP, not the user).
- Logout (which sessions were revoked).
- Password change.
- 2FA enroll / disable / step-up.
- Email change.
- Role grant / revoke.
- User impersonation start / end.

Six months later when someone asks "did anyone log into Alice's account
on June 14?", the answer is one SQL query away. Without an audit
channel, the answer is a long, awkward incident.

## Why this matters

- **Conflating AuthN and AuthZ is the #1 SaaS bug.** A user who is
  signed in is *not* automatically allowed to do a thing.
- **The session vs JWT choice is per-audience, not per-product.** Use
  both at the same time when you serve both.
- **Step-up + audit make the worst-case damage *contained*.** Stolen
  sessions exist; minimize what they can do unnoticed.

## Related

- ADR 0004 — Dual-mode auth: cookies for web, JWT for API
- Phase 6 lessons (9 of them)
- Phase 7 lessons (the AuthZ layer)
- `docs/00-mental-models/rbac-vs-abac.md`
