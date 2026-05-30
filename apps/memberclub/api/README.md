# memberclub-api

The capstone Rust service that knits together every backend pattern in the
curriculum into one coherent, self-contained Axum application:

* **HTTP & observability** from `projects/03-notes-api` — Axum routing,
  problem-details errors, the request-id middleware, and a Prometheus
  `/metrics` endpoint with `route` / `method` / `status_class` labels.
* **Auth** from `projects/04-auth-demo` — argon2id password hashing, signed
  HttpOnly session cookies, HS256 JWT, and an `AuthenticatedUser` extractor
  that accepts *either* transport.
* **Authorization** from `projects/05-rbac-policy-lab` — pure-function
  policy predicates (`can_read_doc`, `can_write_doc`, `can_delete_doc`)
  over a multi-tenant `Note` resource, a `Forbidden` enum naming every
  denial reason, and the `require!` macro for early-return.

There are *no* path dependencies on the standalone project crates. Each
pattern is inlined under `src/` so this service is readable as a single
end-to-end integration example.

## Layout

```
apps/memberclub/api/
├── Cargo.toml
├── README.md
├── migrations/
│   ├── 20260526170000_init.sql        users + sessions + notes + audit_logs
│   └── 20260527000000_billing.sql     stripe_customers + subscriptions + invoices + events
└── src/
    ├── main.rs                        binary: tracing + bind + graceful shutdown
    ├── lib.rs                         Router, AppState, ApiError, /healthz, /metrics
    ├── auth.rs                        password / sessions / JWT / extractor
    ├── notes.rs                       DB ops + policy-gated handlers
    ├── billing.rs                     Stripe checkout/portal + signed webhook receiver
    └── policy.rs                      Forbidden + can_* predicates + require!
```

## Endpoints

| Method | Path                  | Auth | Notes |
|---|---|---|---|
| GET    | `/healthz`            | none | liveness probe |
| GET    | `/metrics`            | none | Prometheus exposition |
| POST   | `/v1/auth/register`   | none | argon2id hash, sets cookie, returns JWT |
| POST   | `/v1/auth/login`      | none | sets cookie, returns JWT |
| POST   | `/v1/auth/logout`     | cookie | revokes session, clears cookie (204) |
| GET    | `/v1/me`              | yes  | returns `UserDto` |
| GET    | `/v1/notes`           | yes  | the authenticated user's notes |
| POST   | `/v1/notes`           | yes  | create owned by current user |
| GET    | `/v1/notes/{id}`      | yes  | gated by `can_read_doc` |
| PATCH  | `/v1/notes/{id}`      | yes  | gated by `can_write_doc` |
| DELETE | `/v1/notes/{id}`      | yes  | gated by `can_delete_doc` |
| POST   | `/v1/billing/checkout`| yes  | validates price_id, mints Stripe customer, returns Checkout URL |
| POST   | `/v1/billing/portal`  | yes  | returns Customer Portal URL |
| POST   | `/webhooks/stripe`    | sig  | HMAC-SHA-256 signature + idempotent event mirror |

## Scope cuts vs. the source projects

* **No TOTP / 2FA.** `auth-demo` covers TOTP exhaustively; the integration
  here keeps the auth surface narrow.
* **No refresh-token rotation.** The login response returns an HS256 access
  token. Refreshing is out of scope for this integration example.
* **One combined migration.** Easier to read top-to-bottom than the chained
  init+totp migrations in `auth-demo`.

## Running

```bash
# In-memory SQLite (default), bound to 127.0.0.1:3002
cargo run -p memberclub-api

# Against a file-backed SQLite
DATABASE_URL=sqlite://./memberclub.db?mode=rwc cargo run -p memberclub-api
```

## Testing

```bash
cargo test -p memberclub-api
```

Tests are hermetic: each spins up a fresh in-memory SQLite pool and drives
the router via `tower::ServiceExt::oneshot` — no real listener, no port,
no shared state between tests.
