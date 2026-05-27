# 13-load-test

Phase 11 perf harness. A tiny `loadgen` binary + reusable metric
aggregator the perf retrospective template
(`docs/perf/2026-Q2-retrospective.md`) hooks into.

## The binary

```bash
cargo run -p load-test --release -- \
    --url http://127.0.0.1:3001/v1/notes \
    --concurrency 32 \
    --requests 5000 \
    --output run.json
```

Writes a JSON report to stdout AND `--output` (if given):

```json
{
  "total_requests": 5000,
  "status_counts": {
    "class_2xx": 4998,
    "class_3xx": 0,
    "class_4xx": 0,
    "class_5xx": 2,
    "network_error": 0
  },
  "p50_ms": 12,
  "p95_ms": 31,
  "p99_ms": 84,
  "mean_ms": 14,
  "max_ms": 412,
  "failed": 2
}
```

Plus a one-line summary to stdout: how many requests, how long, RPS,
how many failed. The shape matches the `notes-api p99` row in the
perf retrospective table so reports paste straight in.

## Why not `oha` / `k6`?

Because the curriculum pairs the harness with a very specific report
shape (per-status-class counts + p50/p95/p99, no other noise). A
100-line in-tree implementation is easier to teach + version + diff
than configuring an external tool.

## Tests (3)

`cargo test -p load-test`:

  - `empty_run_yields_all_zeros` — the degenerate case (no
    samples → all-zero report, doesn't panic).
  - `counts_by_status_class` — 2xx/3xx/4xx/5xx + network-error bucket
    + `failed = 5xx + network` invariant.
  - `percentiles_are_sorted` — for 100 samples of 1..=100ms, p50 ≈ 50,
    p95 ≈ 95, p99 ≈ 99 (off-by-one tolerance per the rounding rule
    in `percentile_ms`).
