# Lesson 11.7 — Load Testing with `oha` and `k6`

> **Concept first:** synthetic traffic with controlled concurrency,
> recorded in a markdown report. Reproducible numbers, comparable across
> changes.
> **Time:** 20 minutes.

## Tools

| Tool | What |
|---|---|
| **`oha`** | Lightweight CLI HTTP load generator. Rust. One binary. |
| **`k6`** | Scriptable load tester from Grafana Labs. JS scripts; complex scenarios. |
| **`wrk`** | The OG. Lua scripts. Still fine for simple GETs. |

For first-pass perf work in this curriculum: **`oha`**. It's the one-liner
"how fast is this endpoint?" tool. For complex multi-step scenarios (login
→ navigate → purchase), use `k6`.

## `oha` basics

```bash
cargo install oha
oha -z 30s -c 50 http://localhost:3000/v1/notes
```

Reads as: "run for 30 seconds, with 50 concurrent connections."

Output (abbreviated):

```
Summary:
  Total time:   30.005s
  Requests:     45000
  Success rate: 100.00%

Latency:
  Average:      33.3ms
  p50:          28.1ms
  p95:          82.6ms
  p99:          145.3ms

Status code distribution:
  [200] 45000 responses
```

The numbers you record: success rate, total requests, p50/p95/p99.

## A real test plan

Document the methodology *every time*:

```md
# notes-api list-notes 2026-05-26

## Methodology
- Hardware: dev laptop (Apple M3 Max, 32 GB RAM)
- Service: notes-api commit abc1234, release build
- DB: Postgres 17 in compose, dataset seeded with `--profile load --count 100000`
- Warmup: 10 s before measurement
- Window: 60 s
- Concurrency: 50

## Command
$ oha -z 60s -c 50 --no-tui http://localhost:3000/v1/notes

## Results — before
Success: 100%
Throughput: ~1500 RPS
p50: 28ms; p95: 83ms; p99: 162ms

## Hypothesis
With no `limit` the endpoint serializes all 100 000 notes, even though
most callers only want the first page. Capping the default to 20 should
cut serialization cost dramatically.

## Change
PR #4242: default `limit` from "no cap" to 20.

## Results — after
Success: 100%
Throughput: ~2600 RPS (+75%)
p50: 15ms; p95: 51ms; p99: 113ms
```

Commit it to `docs/perf/notes-api-2026-05-26.md`. Future engineers
*will* read this when they want to understand why the default is 20.

## What to measure

| Metric | Why |
|---|---|
| **Success rate** | If errors spike at high load, you have a bug, not a perf issue |
| **Throughput** (RPS) | Capacity, business-relevant |
| **p50** | What the median user feels |
| **p95** | What 5% of users feel (your tail) |
| **p99** | The talkative-on-social-media users |
| **Memory** during run | Watch for leaks |
| **CPU** during run | Are you CPU-bound? |

`tokio-console` open in another tab during the test catches task
starvation.

## `k6` for scenarios

```js
// k6 scripts/login-and-browse.js
import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  stages: [
    { duration: '30s', target: 50 },
    { duration: '2m',  target: 50 },
    { duration: '30s', target: 0  },
  ],
};

export default function () {
  const login = http.post('http://localhost:3000/auth/login', JSON.stringify({
    email: 'test@example.com', password: 'correct horse battery staple'
  }), { headers: { 'content-type': 'application/json' } });
  check(login, { 'login 200': r => r.status === 200 });

  const me = http.get('http://localhost:3000/me', {
    headers: { authorization: `Bearer ${login.json('access_token')}` },
  });
  check(me, { 'me 200': r => r.status === 200 });
  sleep(1);
}
```

Run with `k6 run scripts/login-and-browse.js`. The stages let you ramp
up; the `check`s let you assert each step's status.

## The latency budget table

Build a one-page table in `docs/perf/budgets.md` that gives each route a
budget. Update it when you ship a performance fix.

```
Route                  p99 budget   p99 actual (2026-05-26)
/v1/notes              200ms        145ms ✓
/v1/notes/{id}         150ms        51ms  ✓
POST /v1/notes         300ms        87ms  ✓
GET /me                100ms        42ms  ✓
POST /auth/login       400ms        320ms ✓  (argon2 dominates)
POST /webhooks/stripe  100ms        18ms  ✓
```

Green ticks mean "within budget." When a row turns red, you have a
release-blocking ticket.

## CI for perf?

Tempting; usually not worth it. The variance between CI runners is too
high to catch real regressions reliably. Run perf tests on staging or
a dedicated benchmark host, manually, with the same methodology each
time.

(If you do CI perf tests, focus on *macro* signals — total throughput,
not p99 — and require statistical confidence over multiple runs.)

## Why this matters

- **A documented before/after** is the difference between a
  performance *fact* and a vibe.
- **The methodology section** lets you re-measure consistently and
  catches drift.
- **A budget table makes regressions visible** — green→red is the
  alert.

## Green-bar checkpoint

- You can run `oha` against an endpoint and read the results.
- You can sketch a load test report with methodology + results.
- You can pick `oha` vs `k6` for a given scenario.

Next: `lessons/08-perf-budget.md`.
