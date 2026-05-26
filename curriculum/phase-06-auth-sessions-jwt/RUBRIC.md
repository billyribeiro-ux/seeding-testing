# Phase 6 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Password hashing** | Plain text or MD5/SHA-1 | argon2id with sane defaults | Parameter-agile (re-hash on login), measures hash time under load |
| **Session model** | Cookie holds the user id | Signed HttpOnly cookie carrying a random token; hash stored server-side | Rotation on privileged actions; per-session metadata; tested revocation |
| **JWT** | HS256 or no JWT | Access + refresh with standard claims | RS256 + JWKS + key rotation via `kid`; reuse-detection on refresh |
| **CSRF defense** | None | SameSite=Lax | Double-submit token for state-changing routes; tested |
| **2FA** | None | TOTP enrollment + verify | Recovery codes (hashed), step-up auth on sensitive actions |
| **Email verification + reset** | Tokens in URL params with no expiry | Signed time-bounded single-use tokens | Constant-time response to forgot-password; session-revocation on reset |
| **Enumeration resistance** | "User not found" on login or reset | Same status code for unknown user | Constant-time response; logged but not surfaced to wire |
| **Rate limiting / lockout** | None | Per-IP rate limit on auth routes | Per-account counters in Redis; exponential backoff; tested 429s |
| **Extractor design** | Re-parse cookie/jwt in every handler | Single `AuthenticatedUser` extractor | Variants (`VerifiedUser`, `StepUpUser`); extractor tests are the auth contract |
| **Audit logging** | None | One row per privileged action | Inside the same txn as the side effect; queryable, never edited |

## Self-check before moving to Phase 7

- [ ] `make verify` passes locally.
- [ ] You can register, log in, hit `/me` with *both* cookie and bearer transports.
- [ ] You can articulate the dual-mode pattern out loud.
- [ ] You completed exercises E6.1 – E6.4 (E6.5–E6.7 are stretch).
- [ ] You understand argon2id parameters and can re-tune them.
- [ ] You can read the `AuthenticatedUser` extractor and explain every branch.
- [ ] CI is green on your branch.

Phase 7 — **RBAC + ABAC** — uses everything in Phase 6 as the *identity* layer. Now we decide what each identity can do.
