# Lesson 4.4 — Extractors and Validation

> **Concept first:** an extractor is a *parser*. It either gives you a well-typed value or fails with a clean error. Validation is where you say "...and also the value must satisfy these business rules."
> **Time:** 25 minutes.

## Built-in extractors

| Extractor | What it does |
|---|---|
| `Path<T>` | Parses path parameters into `T` (e.g. `i64`, tuple, struct) |
| `Query<T>` | Parses query string into `T` (`T: Deserialize`) |
| `Json<T>` | Parses `application/json` body into `T` |
| `Form<T>` | Parses `application/x-www-form-urlencoded` body into `T` |
| `Bytes` / `String` | Raw body (for custom parsing) |
| `HeaderMap` | All request headers |
| `Extension<T>` | Looks up a value injected by middleware |
| `State<S>` | Application state |
| `axum_extra::TypedHeader<T>` | Parsed typed header (Cookie, Authorization, …) |

All extractors are *try-extractors*: when extraction fails (bad JSON, missing path arg, wrong content type), Axum returns a clean error response *before* your handler runs.

## Custom extractor pattern

When a value is computed from multiple inputs and reused across handlers, write a custom extractor:

```rust
pub struct AuthenticatedUser(pub UserId);

impl<S> FromRequestParts<S> for AuthenticatedUser
where S: Send + Sync,
{
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let cookie = parts.headers.get("cookie").ok_or(ApiError::Unauthorized)?;
        let user_id = decode_session(cookie)?;
        Ok(Self(user_id))
    }
}

// Now every handler can take `AuthenticatedUser` and just *have it*.
async fn me(AuthenticatedUser(uid): AuthenticatedUser) -> Json<Profile> { /* … */ }
```

We use exactly this pattern in Phase 6 for the dual-mode auth.

## Validation

Two layers, each worth doing:

### 1. Type-level validation — happens for free

```rust
#[derive(Deserialize)]
struct CreateOrder {
    quantity: u32,     // can never be negative
    user_id: UserId,   // can never be a tax_rate by accident
}
```

The Rust type system rejects half the bad input you'd otherwise have to check by hand.

### 2. Semantic validation — explicit

Two styles:

**Inline (what `notes-api` uses):**

```rust
let trimmed = body.body.trim();
if trimmed.is_empty() { return Err(ApiError::from(NotesError::Empty)); }
if trimmed.chars().count() > 4096 { return Err(...); }
```

Cheap, clear, lives next to the call.

**`validator` crate (for big payloads):**

```rust
use validator::Validate;

#[derive(Deserialize, Validate)]
struct CreateUser {
    #[validate(email)]                          email: String,
    #[validate(length(min = 1, max = 100))]      name: String,
    #[validate(range(min = 18, max = 120))]      age: u8,
}

async fn create(Json(body): Json<CreateUser>) -> Result<...> {
    body.validate().map_err(ApiError::from)?;
    ...
}
```

Declarative, dry, machine-readable error messages.

Pick *one* style per crate. Don't mix.

## Where to validate

Both the application layer *and* the database layer:

- **Application validation** runs first, returns a clean problem-details response (`400`).
- **Database CHECK constraints** are the safety net for when application code has a bug. A `CHECK (length(body) > 0 AND length(body) <= 4096)` makes the schema enforce the same rule.

> **Belt-and-braces.** Two independent enforcements of one invariant.

## Validation errors → problem-details

Map validator errors into your `ApiError` so the wire format is consistent:

```rust
impl From<validator::ValidationErrors> for ApiError {
    fn from(e: validator::ValidationErrors) -> Self {
        ApiError::BadRequest(e.to_string())
    }
}
```

Or render them as a structured `errors: [{ field, message }]` array in the problem-details body — clients can show inline form errors.

## Why this matters

- **Extractors push parsing failure to the boundary.** Inside the handler, you have valid typed data.
- **Validation in two layers means you can refactor application code freely.** The DB still defends correctness.
- **Custom extractors collapse repetitive plumbing.** "Pull the authenticated user out of the cookie" should be one line of handler signature, not 30 lines of handler body.

## Green-bar checkpoint

- You can write a `Json<CreateUser>` handler that validates with the `validator` crate.
- You can write a custom extractor that reads a header.
- You can articulate why CHECK constraints + application validation are both worth doing.

Next: `lessons/05-errors-as-responses.md`.
