# Performance Test Methodology

> The protocol every perf measurement follows. Copy this section into
> every load-test report so reviewers can reproduce.

## Why a written methodology

Two people running "the same" load test on the same service can get
30%-different numbers if they don't agree on:

- Warmup duration.
- Dataset scale.
- Concurrency.
- Cold vs warm cache.
- Where the load generator runs (same host? cross-region?).

Document the methodology *once*; copy it into every report.

## The minimum reproducible recipe

```yaml
Hardware:
  - load_generator: AWS c7i.large (2 vCPU, 4 GiB) us-east-1
  - target_service: 4 × Fly.io Hetzner machines, shared-cpu-1x (1 vCPU, 256 MB)
  - database: Fly.io postgres-shared (1 GB RAM)
  - network: cross-region (load generator → service) ~25 ms RTT

Software:
  - target_version: notes-api commit abc1234, release build
  - load_tool: oha 1.5.0
  - protocol: HTTPS/1.1 (no h2)

Dataset:
  - seeded with `notes-seed --profile load --count 100000`
  - 1000 users; each with ~100 notes; bodies 200–2000 chars

Procedure:
  1. Restart service pods (cold start).
  2. Warmup: 30 s at 10 RPS.
  3. Measurement window: 60 s at target concurrency.
  4. Cool down: 10 s.
  5. Capture: success rate, throughput, p50, p95, p99, max,
     server CPU + memory + DB pool in-use during the window.
```

## What to record

Every test report contains, at minimum:

```md
## Methodology
{paste the recipe above, edited for any differences}

## Command
$ oha -z 60s -c <concurrency> --no-tui <URL>

## Results
- Success rate
- Throughput
- p50, p95, p99
- Server-side metrics (CPU, RSS, DB pool in-use)
- Anomalies observed (alerts that fired, log spikes)

## Hypothesis
{one paragraph}

## Change
{PR link, summary}

## After
{re-measured numbers}

## Conclusion
{decision: ship / iterate / reject}
```

## What to *not* do

- **Run on your laptop while screen-sharing.** Notebook power throttling
  is unreliable.
- **Test against an empty DB.** Realistic dataset only.
- **Skip warmup.** First few seconds always include JIT/cache warm
  effects.
- **Use averages.** p50, p95, p99. Average lies.
- **Compare against an unrelated run.** Always paired (same methodology,
  same day, same hardware).

## Tooling specifics

### `oha`

```bash
cargo install oha
oha -z 60s -c 50 --no-tui --insecure https://api.test/v1/notes
```

- `-z` time-bound run (alternative: `-n` for fixed request count).
- `-c` concurrent connections.
- `--no-tui` is required when piping output to a file.

### `k6`

For multi-step flows. See `scripts/login-and-browse.js` template.

### Server-side metric capture

From the load generator's perspective:

```bash
oha -z 60s -c 50 https://api.test/v1/notes > load.txt &
LOAD_PID=$!

# During the run, snapshot server metrics every 5 s
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do
    date -u +%H:%M:%S
    curl -s https://api.test/metrics | rg '^(http_requests_total|db_pool_in_use)' | head -10
    sleep 5
done

wait $LOAD_PID
```

Capture both client-side (oha) and server-side (`/metrics`) views.

## Quarterly perf review

Every quarter:

1. Re-run the standard tests; record numbers in
   `docs/perf/<service>-<YYYY>-Q<N>.md`.
2. Update `budgets.md`'s *last verified* column.
3. Note any trend (worse than last quarter? better?).
4. Pick *one* route to optimize next quarter.

90 minutes per service; the discipline pays for itself.

## Related

- Phase 11 lesson 7 (Load testing)
- Phase 11 lesson 8 (Perf budgets and deploy gates)
- `budgets.md` — the live table
