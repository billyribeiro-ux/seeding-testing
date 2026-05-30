# projects/03-notes-api

Phase 4 capstone — a real Axum service over the `sqlx-notes` data layer.

## What it teaches

- `axum::Router` with `nest("/v1", ...)` for versioned routes.
- `State<Arc<AppState>>` for shared resources (DB pool).
- `Path`, `Query`, `Json` extractors.
- Custom error → response mapping (RFC 7807 problem-details).
- `tower-http` layers: `TraceLayer`, `CompressionLayer`, `CorsLayer`, request-id propagation.
- Graceful shutdown on ctrl-C and SIGTERM.
- Integration tests with `tower::ServiceExt::oneshot` — no real port binding.

## Run it

```bash
cargo run -p notes-api
# listens on 127.0.0.1:3000

curl http://localhost:3000/healthz
curl http://localhost:3000/v1/notes
curl -X POST http://localhost:3000/v1/notes \
     -H 'content-type: application/json' \
     -d '{"body":"hello"}'
curl http://localhost:3000/v1/notes/1
curl -X PATCH http://localhost:3000/v1/notes/1 \
     -H 'content-type: application/json' \
     -d '{"body":"updated"}'
curl -X DELETE http://localhost:3000/v1/notes/1 -i
```

## Test it

```bash
cargo test -p notes-api
cargo clippy -p notes-api -- -D warnings
```

Or `make verify` from the repo root.

## Endpoints

| Method | Path | Response |
|---|---|---|
| GET | `/healthz` | `200 {"status":"ok","version":"..."}` |
| GET | `/v1/notes?limit=N&cursor=...` | `200 {"items":[Note...],"next":"cursor-or-null"}` |
| POST | `/v1/notes` | `201 Note` or `400` problem-details |
| GET | `/v1/notes/:id` | `200 Note` or `404` problem-details |
| PATCH | `/v1/notes/:id` | `200 Note` or `400`/`404` |
| DELETE | `/v1/notes/:id` | `204` or `404` |

All errors return `application/problem+json` (RFC 7807):

```json
{
  "type":   "https://memberclub.test/problems/not-found",
  "title":  "Not Found",
  "status": 404,
  "detail": "note 999 not found"
}
```

Request IDs (`X-Request-Id`) are echoed back; if not provided, a UUID v4 is generated.

## Config

| Env var | Default | Notes |
|---|---|---|
| `DATABASE_URL` | `sqlite::memory:` | Use `sqlite://./dev.sqlite` for persistence |
| `APP_BIND` | `127.0.0.1:3000` | `host:port` |
| `RUST_LOG` | `info,notes_api=debug,tower_http=info` | tracing-subscriber filter |
