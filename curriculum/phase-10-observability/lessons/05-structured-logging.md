# Lesson 10.5 — Structured Logging

> **Concept first:** logs are JSON objects, not sentences. Levels are
> deliberate. PII is redacted. The boring discipline scales.
> **Time:** 15 minutes.

## JSON, not text

```json
{
  "timestamp": "2026-05-26T16:50:00.123Z",
  "level": "INFO",
  "service": "notes-api",
  "trace_id": "01HXY...",
  "span_id": "abc123",
  "req_id": "01HXY...",
  "user_id": 42,
  "route": "/v1/notes",
  "method": "POST",
  "message": "note created",
  "note_id": 7
}
```

Every field is queryable. Every level is filterable. Every record is
machine-readable.

Compare with:

```
2026-05-26 16:50:00 INFO notes-api: user 42 created note 7
```

You can grep, but you can't *aggregate*. "How many notes were created in
the last hour by users on the Pro tier?" — the first format answers it
with a single query; the second requires parsing magic.

In Rust:

```rust
tracing_subscriber::fmt()
    .json()
    .with_current_span(true)
    .with_span_list(true)
    .flatten_event(true)
    .init();
```

That's the prod config. For dev, swap `.json()` for `.compact()`.

## Levels: what goes where

| Level | Purpose | Examples |
|---|---|---|
| `ERROR` | Something is broken; humans should know. Alerts fire. | DB connection lost, panic recovered, Stripe error |
| `WARN` | Unusual; investigate during business hours. | Retry succeeded after 3 attempts, deprecation warning |
| `INFO` | Business events worth recording. | Login, signup, payment, subscription change |
| `DEBUG` | Developer-grade detail; off in prod. | "Cache hit for key X", "Decided to use index Y" |
| `TRACE` | Truly granular; rarely used. | Per-byte network log |

Default prod: `INFO`. Cranked up via env per-module for incident response:
`RUST_LOG=info,notes_api::billing=debug`.

## Sampling — when not to log

For very high-rate events (cache hits, health checks), sample:

- Use `tracing` directly with a custom layer that samples.
- Or use `metrics::counter!` instead of a log line.

The rule: **if you're emitting more than ~10 events per second per
service from the same code path, sample or convert to a metric.**

## PII redaction

We **never log**:

- Passwords (even hashed — leak the hash, lose every account).
- Credit card numbers, CVVs, account numbers.
- API keys, JWTs, session tokens.
- Personal identifiers (SSN, license, etc.) unless you're a healthcare
  provider with HIPAA approvals.

We *sometimes* log:

- **Email addresses** — useful but high-risk. Hash with a service salt
  when shipping to long-term storage.
- **IP addresses** — necessary for security; may be PII under GDPR.
- **User IDs (internal)** — fine, as long as the IDs are opaque.

For the careful ones, add a `redact` field to `tracing` events:

```rust
tracing::info!(email = %redact_email(&email), "user registered");
```

Better: don't log it at all. Most "we need to log the email" cases are
solved by logging the user id.

## The log boundary

In our services, logs are emitted at exactly three places:

1. **The HTTP layer** (the request span, automatic).
2. **The error boundary** (`IntoResponse for ApiError` logs ERROR/WARN).
3. **Explicit business events** (`tracing::info!("subscription.upgraded",
   ...)`).

Helpers don't log. Repositories don't log. If something interesting
happens, return it as a value or attach it to the current span.

## Why this matters

- **Logs are how you debug after the fact.** Future-you (and on-call
  colleagues) read them at 2 AM.
- **Structured beats grep.** Always.
- **PII discipline is a one-mistake-away thing.** Build the habit early.

## Green-bar checkpoint

- You can configure JSON logging in 5 lines.
- You can pick the right level for a given event.
- You can name three things you must never log.

Next: `lessons/06-dashboards-as-code.md`.
