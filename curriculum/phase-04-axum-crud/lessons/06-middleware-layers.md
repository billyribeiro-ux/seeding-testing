# Lesson 4.6 — Middleware (Tower Layers)

> **Concept first:** middleware is a *function of services*. It wraps the request-handling pipeline to add cross-cutting concerns: logging, tracing, compression, request IDs, timeouts, auth.
> **Time:** 25 minutes.

## What `.layer(...)` does

`Router::layer(L)` wraps every handler in the router with the layer `L`. Layers can:

- Read or modify the request before it reaches the handler.
- Read or modify the response before it leaves.
- Short-circuit (return a response without invoking the handler).
- Add request extensions for handlers to consume.

Layers are *outermost-first* (the first `.layer()` you add runs last on the way in, first on the way out — like nested function calls).

## The five layers every service should have

In the order you write them in the capstone:

```rust
.layer(CompressionLayer::new())                  // 1. gzip responses
.layer(CorsLayer::permissive())                  // 2. CORS
.layer(PropagateRequestIdLayer::x_request_id())  // 3. echo X-Request-Id back
.layer(TraceLayer::new_for_http().make_span_with(/* … */))  // 4. tracing span per request
.layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))    // 5. generate X-Request-Id if absent
```

Now every request:

- Has an `X-Request-Id` (generated if missing, propagated to the response).
- Logs a span tagged with method, URI, and request id.
- Honors CORS preflight.
- Gzips response bodies if the client asked.

Compression-on-the-fly costs CPU; in front of a CDN you might disable it. Decide deliberately.

## Tracing as the *primary* observability layer

```rust
TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
    info_span!(
        "http",
        method   = %req.method(),
        uri      = %req.uri(),
        req_id   = req.headers().get("x-request-id").and_then(|h| h.to_str().ok()).unwrap_or("-"),
    )
})
```

Three habits:

- **Tag every span with the request id.** Grep one trace ID and you get every log line for that request.
- **Tag the user id** (added inside an auth middleware so it appears on every authenticated request span).
- **Use `tracing::instrument` on async functions** to nest spans. The trace tree becomes a flame graph for free.

## Timeouts

```rust
.layer(tower_http::timeout::TimeoutLayer::new(Duration::from_secs(10)))
```

10 seconds is generous for a JSON API. Cap it aggressively. **A request without a timeout is a memory leak waiting to happen.**

If the downstream call has its own per-call timeout (sqlx pool acquire, reqwest), the layered timeout is a *backstop* — never your only line of defense.

## Rate limiting

Two approaches:

- **Per-IP token bucket** (e.g. `tower-governor`) — protects against burst abuse.
- **Per-account counter in Redis** — protects against an authenticated user hammering you.

Both layered; both produce `429 Too Many Requests` with a `Retry-After` header. We add these in Phase 11.

## Custom middleware

For ad-hoc work, a `from_fn` middleware is the simplest:

```rust
use axum::middleware::{self, Next};
use axum::http::Request;

async fn add_powered_by<B>(req: Request<B>, next: Next<B>) -> Response {
    let mut res = next.run(req).await;
    res.headers_mut().insert("x-powered-by", "memberclub".parse().unwrap());
    res
}

let app = router.layer(middleware::from_fn(add_powered_by));
```

Cheap. Use it for one-off concerns. Move to a real `Layer` impl when you want reuse across services.

## Why this matters

- **Cross-cutting concerns belong in middleware, not in handlers.** Every handler is small, focused, testable.
- **Tracing-first observability is the new standard.** Logs are reduced; spans carry context.
- **Composability is the win.** Add a new concern by writing one layer. Remove it by deleting one line.

## Green-bar checkpoint

- You can stack five tower-http layers on a Router.
- You can write a `from_fn` middleware that mutates the response.
- You can explain why layer order matters (outermost first to write; outermost last to execute on the way in).

Next: `lessons/07-openapi-with-utoipa.md`.
