# Lesson 4.5 — Errors as Responses (RFC 7807 Problem-Details)

> **Concept first:** at the HTTP boundary, an `Err(_)` becomes a `Response`. The wire format we use is **Problem-Details for HTTP APIs** (RFC 7807) — a JSON object that's machine-readable and human-friendly.
> **Time:** 30 minutes.

## The shape

```json
{
  "type":   "https://memberclub.test/problems/not-found",
  "title":  "Not Found",
  "status": 404,
  "detail": "note 999 not found"
}
```

Four fields:

- **`type`** — a stable URI that *identifies* the problem class. Same for every "note not found" no matter what id.
- **`title`** — a short, human-readable summary.
- **`status`** — the HTTP status code, redundantly so it survives logs and transports.
- **`detail`** — a specific, human-friendly description of this instance.

Extensions are allowed (e.g. `instance` for a per-request URI, `errors: [...]` for validation arrays). Be conservative; clients usually only consume the four core fields.

The Content-Type is **`application/problem+json`**, not `application/json`. Clients that understand the format can switch behaviour.

## In `notes-api`

```rust
#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)] Notes(#[from] NotesError),
    #[error("invalid request body")] BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title, kind, detail) = match &self { /* per-variant mapping */ };
        let body = ProblemDetails { kind, title, status: status.as_u16(), detail };
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(CONTENT_TYPE, "application/problem+json".into());
        response
    }
}
```

Three responsibilities:

1. **Pick the HTTP status.** `Empty`/`TooLong` → 400, `NotFound` → 404, `Db` → 500.
2. **Pick the `type` URI.** Use your own domain (`memberclub.test/problems/*` here). The URI doesn't have to resolve, but it should be stable.
3. **Log noisy errors on the server side.** 5xx errors get `tracing::error!`; 4xx get `tracing::warn!`. We keep the wire `detail` short and the log noisy.

## Don't leak internals

A `5xx` problem-details body must *never* contain a SQL error string, a stack trace, or a file path. Two reasons:

1. **Security** — leaks DB schema and code paths to attackers.
2. **Stability** — a typed error log is more useful than a string in 50 places.

In `notes-api`, the `Db(_)` arm returns `"database error"` to the wire; the actual `sqlx::Error` lands in the structured log via `tracing::error!`.

## The five status codes you'll use 95% of the time

| Status | When |
|---|---|
| `200` | Read succeeded |
| `201` | Created (POST returning a new resource) — include `Location: /resource/{id}` |
| `204` | Deleted / accepted, no body |
| `400` | Bad input — client's fault, no retry |
| `401` | Not authenticated (login first) |
| `403` | Authenticated but not allowed (do not retry) |
| `404` | Resource doesn't exist (or you don't want to confirm it does) |
| `409` | Conflict (UNIQUE constraint, version mismatch) |
| `422` | Validation failed (some shops use 400 here; pick one) |
| `429` | Rate-limited (include `Retry-After`) |
| `500` | Server bug |
| `503` | Service down / overloaded |

`401` vs `403`: 401 means "I don't know who you are." 403 means "I know who you are but you can't do that."

## Errors that *aren't* errors

Some HTTP "errors" are normal client flow:

- **`401`** is normal when the session expired — client should redirect to login.
- **`404`** is normal when a user navigates to a deleted bookmark.
- **`409`** is normal in optimistic-locking flows ("someone else updated this; reload").

Don't alert on these. Alert on **5xx rate**, **p99 latency**, **error-budget burn**, not "this user got a 404 once."

## Why this matters

- **Problem-Details is a *contract*.** Mobile teams, SDK generators, and developer-portal docs all read it the same way.
- **The status-code menu above is small.** You will pick from it 95% of the time. Memorize.
- **Server-side logs vs wire bodies are two different audiences.** Be generous with the logs, frugal with the bodies.

## Green-bar checkpoint

- You can implement `IntoResponse` for a typed error enum producing problem-details JSON.
- You can pick the right status code for "user double-clicked the submit button and the second click hit a UNIQUE constraint" (409).
- You can articulate why we strip `sqlx::Error` details from the wire body.

Next: `lessons/06-middleware-layers.md`.
