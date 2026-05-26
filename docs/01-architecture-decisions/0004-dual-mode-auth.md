# ADR 0004 — Dual-mode auth: signed cookies for web, JWT for API clients

- Status: Accepted
- Date: 2026-05-26
- Deciders: api-team, security-team
- Tags: auth, security, web, api

## Context and Problem Statement

Two audiences with very different threat models share the same backend:

- **Browsers** (SvelteKit web app) — vulnerable to XSS reading any token
  in `localStorage`/JS-visible state. Session revocation must be
  immediate (logout from any device).
- **API clients** (mobile, CLI, partner integrations) — can store
  secrets safely, expect bearer-token semantics, need offline-friendly
  short-lived credentials.

We need an auth mechanism that serves both without compromising either.

## Decision Drivers

- XSS-resistant credentials for browsers.
- Stateless, scalable credentials for API clients.
- Immediate revocation when needed (password reset, logout).
- One mental model for handler code — handlers shouldn't fork on
  transport.

## Considered Options

1. **JWT for everything (localStorage in the browser).** XSS pwns it.
2. **Sessions for everything.** API clients hate cookies and must
   round-trip the session DB on every request.
3. **Dual-mode: signed HttpOnly cookies for web + JWT (access+refresh)
   for API clients.** One extractor merges both.

## Decision Outcome

Chose **option 3**.

- Web flow: HMAC-signed HttpOnly cookie carries a random session token;
  hashed token in `sessions` table; revocation is a single `UPDATE`.
- API flow: RS256 (or HS256 in the demo) JWT, 15-minute access token,
  30-day refresh token, refresh-rotation with reuse-detection (planned
  via E6.7).
- A single `AuthenticatedUser` extractor (Phase 6.8) tries Bearer first,
  then signed cookie. Handlers don't branch.

## Consequences

- **Positive:** XSS cannot exfiltrate browser sessions; logout is
  instant; API clients use the standard bearer pattern; one handler
  signature for both transports.
- **Negative:** two code paths in the extractor; learners must understand
  both; cookie + bearer mix can confuse Postman users.
- **Mitigations:** the extractor is *the* security perimeter — well-tested
  (Phase 6 capstone has 18 tests). Documentation is explicit. Postman
  pre-request scripts can populate either transport.

## Notes

We do not use third-party identity providers (Auth0, Okta) — direct
ownership of the auth surface is part of the team's competency by
design. Future ADRs may revisit for enterprise SSO (SAML/OIDC).

Related: Phase 6 lessons; project `04-auth-demo`.
