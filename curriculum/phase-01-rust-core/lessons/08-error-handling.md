# Lesson 1.8 — Error Handling

> **Concept first:** Rust has no exceptions. Errors are *values* you return from functions. The `?` operator is your superpower. Two crates — `thiserror` and `anyhow` — make error types nearly painless.
> **Time:** 45 minutes.

## Two kinds of errors

Rust draws a sharp line:

- **Recoverable** errors: the file you tried to open didn't exist, the JSON you tried to parse was malformed. Returned as `Result<T, E>`. The caller decides what to do.
- **Unrecoverable** errors: an invariant you depend on is broken, a `Vec` index is out of bounds, a `match` you thought was exhaustive isn't. Use `panic!`. The program crashes (or unwinds and prints a backtrace).

> **Panic is for bugs.** If you're panicking on user input or a missing file, you're using the wrong tool. Use `Result`.

## `Result<T, E>` recap

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

You return `Result<T, E>` to say "either I produced a `T`, or I produced an `E`." The caller must handle both arms — the compiler will warn if they ignore the `Result`.

```rust
use std::fs;

fn read_file(path: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

fn main() {
    match read_file("Cargo.toml") {
        Ok(content) => println!("{} bytes", content.len()),
        Err(e)      => eprintln!("error: {e}"),
    }
}
```

## The `?` operator — early-return on error

Writing `match` on every fallible call is noisy. The `?` operator desugars to: "if `Err`, return early; if `Ok`, unwrap the value."

```rust
fn read_two(a: &str, b: &str) -> Result<(String, String), std::io::Error> {
    let x = fs::read_to_string(a)?;       // returns Err on failure
    let y = fs::read_to_string(b)?;       // same
    Ok((x, y))
}
```

`?` only works in functions that return `Result<…, E>` or `Option<…>` (with compatible types).

## Custom error types with `thiserror`

For *libraries* and *modules*, you want a typed error enum so callers can match on the cause:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("invalid amount: {0} cents (max is 21B)")]
    InvalidAmount(i64),

    #[error("currency mismatch: {expected} vs {got}")]
    CurrencyMismatch { expected: String, got: String },

    #[error("database error")]
    Db(#[from] sqlx::Error),                // auto-converts sqlx::Error via `?`

    #[error("stripe error")]
    Stripe(#[from] stripe::Error),
}
```

`#[derive(Error)]` generates the `Display` and `Error::source` implementations for you. `#[from]` generates an `impl From<sqlx::Error> for BillingError` so the `?` operator just works on `sqlx` calls:

```rust
pub async fn charge(amount: i64, currency: &str) -> Result<(), BillingError> {
    if amount > 2_100_000_000_000 { return Err(BillingError::InvalidAmount(amount)); }  // $21B ceiling, in cents
    let _row = sqlx::query!("…").execute(&pool).await?;  // sqlx::Error → BillingError
    Ok(())
}
```

> **Rule of thumb:** every *crate* (library) defines its own `Error` enum with `thiserror`. The variants name domain failure modes; the `#[from]` wraps lower-level errors.

## Application-level errors with `anyhow`

When you're at the top of the stack — a CLI's `main`, an Axum handler before mapping to HTTP — you don't care about typed variants, you just want "anything that went wrong, with context." That's `anyhow`:

```rust
use anyhow::{Context, Result};

fn ship(order_id: i64) -> Result<()> {
    let order = db::fetch_order(order_id).context(format!("loading order {order_id}"))?;
    payments::charge(&order).context("charging the customer")?;
    fulfillment::dispatch(&order).context("dispatching to warehouse")?;
    Ok(())
}
```

If `dispatch` fails, the error chain reads:

```
Error: dispatching to warehouse

Caused by:
    0: warehouse unreachable: timeout
```

Crisp, layered, debuggable.

> **The split that works in practice:**
> - In *libraries*: `Result<T, MyTypedError>` (uses `thiserror`).
> - In *binaries / application code*: `anyhow::Result<T>` for top-level functions and `main`.

## Mapping errors at the boundary

In Axum (Phase 4), handlers convert your typed errors into HTTP responses:

```rust
impl IntoResponse for BillingError {
    fn into_response(self) -> Response {
        let status = match self {
            BillingError::InvalidAmount(_) | BillingError::CurrencyMismatch { .. } =>
                StatusCode::BAD_REQUEST,
            BillingError::Db(_) | BillingError::Stripe(_) =>
                StatusCode::INTERNAL_SERVER_ERROR,
        };
        // ... build a problem-details JSON body ...
        (status, Json(body)).into_response()
    }
}
```

That's the **error-mapping boundary**: typed errors flow up; HTTP responses flow out. Two distinct vocabularies.

## When `unwrap` is OK

- In tests.
- In example code (like this lesson).
- After you've *proven* something can't fail (e.g. parsing a string constant you control).

If you find yourself writing `.unwrap()` and feeling nervous, you've found a bug-in-waiting. Replace with `?` or `unwrap_or_else`.

## `unwrap_or`, `unwrap_or_default`, `unwrap_or_else`

When `Option`/`Result` is empty and you have a sensible fallback:

```rust
let port: u16 = std::env::var("PORT").ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(3000);
```

`unwrap_or` returns the alternative eagerly; `unwrap_or_else` calls a closure (use it if the alternative is expensive to compute).

## A worked example: `hello-cli`'s error handling

Recall `projects/01-hello-cli/src/main.rs`:

```rust
#[derive(Debug)]
enum ReadError {
    NotFound(PathBuf),
    Io(io::Error),
    NotUtf8,
}

fn read_input(path: Option<&Path>) -> Result<String, ReadError> {
    let Some(p) = path else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).map_err(ReadError::Io)?;
        return Ok(buf);
    };
    match fs::read(p) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ReadError::NotFound(p.to_path_buf())),
        Err(e) => Err(ReadError::Io(e)),
    }
}
```

Three things to notice:

1. **The error enum names the *cases the caller cares about*.** "I/O failed in general" vs "file not found" matters for the exit code we return.
2. **We use `?` even inside the `Result::Err` branch.** The `let … else` returns early on the `None` path *cleanly*.
3. **In `main`, we `match` on the error and pick an exit code.** That's the boundary — error type → user-facing behaviour.

We don't use `thiserror` here because the error stays local to one file. As soon as it crosses a module boundary, we'd promote it.

## Why this matters

- **No silent failures.** Every error is in the type signature. Callers cannot ignore them by accident.
- **Composability.** Functions that return `Result` compose via `?`. Functions that throw exceptions compose via try/catch chains and fragile docstrings.
- **Recoverable vs panic separation.** When something panics in production, you treat it as a bug. When something returns `Err`, you treat it as a known failure mode. They get different alerting.

## Green-bar checkpoint

- You can define a `thiserror` enum with two variants — one wrapping a lower-level error via `#[from]`, one carrying a custom payload.
- You can rewrite a function full of `.unwrap()` calls into one that returns `anyhow::Result<T>` and uses `.context(...)`.
- You can articulate when `panic!` is appropriate (bugs / invariant violations) vs `Result` (recoverable failures).

Next: `lessons/09-modules-and-crates.md`.
