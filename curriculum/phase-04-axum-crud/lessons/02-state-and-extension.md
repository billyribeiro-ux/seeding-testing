# Lesson 4.2 — `State` and Shared Resources

> **Concept first:** every handler needs access to the same DB pool, config, and metrics. Axum's `State<T>` is how you share them cheaply and type-safely.
> **Time:** 20 minutes.

## The shape

```rust
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
}

let state = AppState { pool, config: Arc::new(cfg), metrics: Arc::new(m) };
let app = Router::new().route("/users", get(list_users)).with_state(state);

async fn list_users(State(s): State<AppState>) -> Json<Vec<User>> { /* … */ }
```

Two rules:

1. **`AppState` must be `Clone`.** Each request gets a clone (cheap when fields are `Arc`/`Pool`).
2. **Use `Arc<Inner>` for any field that is itself `Clone`-expensive** — e.g. wrap a big `Config` once at startup.

## Why `Arc<AppState>` shows up in the capstone

We wrap `AppState` in `Arc` because:

- The router is built *before* the layers know what `S` is. Putting state behind `Arc` keeps the clone trivial regardless of how `AppState` grows.
- Tests can clone the `Arc` per `oneshot` invocation cheaply.

```rust
.with_state(Arc::new(state))
async fn list_notes(State(s): State<Arc<AppState>>) -> ...
```

## `Extension<T>` vs `State<T>`

Axum has two ways to attach shared data:

| | `State<T>` | `Extension<T>` |
|---|---|---|
| Type-checked | ✓ (must match the router's `S`) | ✗ (looked up at runtime; panics if missing) |
| Set with | `.with_state(...)` | `.layer(Extension(...))` |
| Use when | Your app has *one* canonical state struct | You're injecting per-route or per-layer extras |

Prefer `State`. Reach for `Extension` when a middleware needs to attach per-request data (e.g. an authenticated user pulled out of the session cookie — Phase 6).

## Per-request data from middleware

Middlewares can put values into the request's `extensions` map. Downstream handlers read them via `Extension<T>`:

```rust
// middleware
let mut req = req;
req.extensions_mut().insert(AuthenticatedUser { id: 42 });

// handler
async fn whoami(Extension(user): Extension<AuthenticatedUser>) -> String {
    format!("hello user {}", user.id)
}
```

The auth pattern in Phase 6 uses this shape.

## Splitting sub-routers with different state

If `v1` needs a slightly different state than `/healthz`, give it its own:

```rust
let v1: Router<Arc<V1State>> = Router::new().route(...).with_state(v1_state);
let root = Router::new().route("/healthz", get(health)).merge(v1);
```

Axum reconciles the state types at compile time. If you try to nest a router with state `A` inside one expecting `B`, the compiler refuses.

## A subtle pitfall: holding references across `.await`

```rust
let pool = &s.pool;
let users = sqlx::query("SELECT ...").fetch_all(pool).await?; // ok

let conn = pool.acquire().await?;                          // exclusive lock on a conn
expensive_other_query(pool).await?;                        // ⚠️ may starve waiting for a conn
```

When the connection guard is held across an `.await`, that connection is unavailable to everyone else. Keep the critical section tight.

## Why this matters

- **One `AppState` keeps handlers honest.** Every dependency is explicit in the signature.
- **`Arc` lets `AppState` grow without breaking callers.** Add a `redis: Arc<Redis>` field — handlers that don't use it don't notice.
- **`Extension` is the right tool for *per-request* injection** — auth, request IDs, tenant IDs.

## Green-bar checkpoint

- You can declare an `AppState` and inject it into a handler.
- You can explain when to choose `State` vs `Extension`.
- You can spot the connection-held-across-`.await` anti-pattern.

Next: `lessons/03-routes-and-versioning.md`.
