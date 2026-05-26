# Lesson 4.1 — The Axum Mental Model

> **Concept first:** a web framework is a function from `Request` to `Response`, plus a router. Axum makes that literal.
> **Time:** 25 minutes.

## The four pieces

1. **Router** — a tree of `(method, path) → handler` entries. Cheap to build, cheap to clone.
2. **Handler** — an `async fn` that takes *extractors* and returns a value that implements `IntoResponse`.
3. **Extractor** — a type whose `FromRequest`/`FromRequestParts` impl pulls a value out of the request. Built-in: `Path<T>`, `Query<T>`, `Json<T>`, `State<S>`, `Extension<E>`, `Bytes`, `String`, headers, cookies. You can write your own.
4. **Layer** — middleware. A function that wraps a `Service` and returns a new `Service`. Stacked with `.layer(...)`.

That's it. Everything else builds on these four primitives.

## The "hello, world"

```rust
use axum::{Router, routing::get};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| async { "hello" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

Six lines of code give you a working HTTP server with method dispatch, async, and graceful connection handling.

## Handlers are *just functions*

Axum infers what to inject by looking at the function signature:

```rust
async fn greet(Path(name): Path<String>) -> String {
    format!("hello, {name}")
}
```

`Path<String>` extracts the path parameter; `String` is `IntoResponse`, so the framework knows to return it with `Content-Type: text/plain` and status 200.

You can mix extractors freely. The only rule: only **one** extractor consumes the body (`Json<T>`, `Bytes`, `String`, `Form<T>`), and it must come *last*.

## `IntoResponse` — what counts as a response

| Type | What it produces |
|---|---|
| `()` | `204 No Content` |
| `&'static str`, `String` | `200 text/plain` |
| `Vec<u8>`, `Bytes` | `200 application/octet-stream` |
| `Json<T>` (T: Serialize) | `200 application/json` |
| `StatusCode` | empty body with that status |
| `(StatusCode, T)` | `T`'s body with that status |
| `Result<T, E>` (both `IntoResponse`) | `T` on `Ok`, `E` on `Err` |
| Your own type | implement `IntoResponse` |

The `Result<T, E>` pattern is how we map errors to HTTP — see Lesson 4.5.

## The capstone's router (preview)

```rust
let v1 = Router::new()
    .route("/notes",       get(list_notes).post(create_note))
    .route("/notes/{id}",  get(get_note).patch(update_note).delete(delete_note));

let app = Router::new()
    .route("/healthz", get(health))
    .nest("/v1", v1)
    .with_state(state)
    .layer(/* middleware */);
```

Each `.route()` registers one path with one or more methods. `.nest("/v1", v1)` mounts a sub-router under a prefix.

## Why this matters

- **Handlers are unit-testable.** They're just functions. You can call them in tests with hand-built extractors.
- **Routers are composable.** Mount a `v1`/`v2` API side-by-side. Mount third-party routers (e.g. `tower-http`'s `ServeDir`) without ceremony.
- **Extractors document the contract.** A handler signature like `fn(State<AppState>, Path<i64>, Json<Body>) -> Result<Json<Note>, ApiError>` is its own README.

## Green-bar checkpoint

- You can write a 6-line "hello, world" Axum service.
- You can write a handler that takes `Path<i64>` and returns a `Result<Json<Foo>, ApiError>`.
- You can explain how Axum decides where to inject each argument.

Next: `lessons/02-state-and-extension.md`.
