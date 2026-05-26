# projects/02-quote-generator

The Phase 2 capstone drill: a concurrent HTTP client that fetches many URLs in parallel with
bounded concurrency, per-request timeouts, and an optional overall deadline.

## What it teaches

- `tokio::main` to boot an async runtime.
- `tokio::sync::Semaphore` to cap in-flight requests.
- `tokio::time::timeout` to bound any single I/O call.
- `futures::stream::FuturesUnordered` to surface results in completion order.
- Splitting business logic (`src/lib.rs`) from CLI plumbing (`src/main.rs`) — the same enterprise habit from Phase 1.
- Integration tests against `wiremock` (a real local HTTP server). **Zero internet dependency.**

## Run it

```bash
cargo run -p quote-generator -- --help

# Fetch three URLs with 4 in-flight max, 2 s per request, 30 s overall
cargo run -p quote-generator -- \
    --concurrency 4 --timeout 2s --deadline 30s \
    https://example.com https://example.org https://example.net

# Or read URLs from a file (# comments and blank lines ignored)
cat > urls.txt <<'EOF'
# Free quote APIs
https://api.quotable.io/random
https://zenquotes.io/api/random
EOF
cargo run -p quote-generator -- --file urls.txt --timeout 5s
```

## Test it

```bash
cargo test    -p quote-generator               # unit + integration (wiremock)
cargo clippy  -p quote-generator -- -D warnings
cargo fmt     -p quote-generator -- --check
```

Or the workspace gate (what CI runs):

```bash
make verify
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | All URLs returned 2xx |
| 1 | At least one URL failed (timeout, network error, non-2xx) |
| 2 | Bad input (no URLs, file not found, invalid duration) |
| 3 | Overall deadline exceeded |

## Output format

```
OK    200    173.2ms  https://api.quotable.io/random
TIME           2s    https://slow.example.com/
ERR            --    https://nope.example.com/  (dns error: ...)
```
