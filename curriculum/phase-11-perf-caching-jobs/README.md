# Phase 11 — Performance, Caching, Background Jobs

> **Audience:** you finished Phase 10. You can *see* the system.
> **Outcome:** you can find a bottleneck, decide where to spend the
> optimization budget, and ship a fix supported by load test numbers.
> **Time:** 2–3 weeks.

## The mental model

> *Measure twice, optimize once.* Profiling tells you where to spend
> effort. Guessing produces clean code that didn't need optimizing.

Three categories of work in Phase 11:

1. **Profiling** — find the bottleneck (CPU? IO? lock contention?).
2. **Caching + concurrency** — the standard fixes (Redis, semaphores, batch).
3. **Background jobs + multi-tenancy** — patterns that need their own design (the outbox, RLS).

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-mental-model.md` | Find before fix; benchmark methodology |
| `lessons/02-profiling-rust.md` | `cargo flamegraph`, tokio-console, `pprof-rs` |
| `lessons/03-db-perf.md` | `EXPLAIN ANALYZE`, indexes, N+1, pool tuning |
| `lessons/04-caching-with-redis.md` | Cache-aside, single-flight, TTL strategies |
| `lessons/05-background-jobs.md` | Outbox pattern, `apalis`/`tokio-cron-scheduler` |
| `lessons/06-multi-tenancy.md` | Row-level isolation, Postgres RLS, audit |
| `lessons/07-load-testing.md` | `oha`/`k6` against MemberClub; capture before/after |
| `lessons/08-perf-budget.md` | Latency budgets per route; deploy gates |

## The capstone

This phase has no separate capstone project. Instead, you:

1. **Pick one MemberClub route** with measurable latency (`/v1/notes` is a
   fine candidate).
2. **Load-test it** with `oha`: capture p50/p95/p99 + throughput.
3. **Find the bottleneck** via traces + profile.
4. **Ship a fix** (add an index, add caching, batch a query).
5. **Re-test** and commit the comparison report to `docs/perf/`.

The report is the deliverable; the work behind it is the lesson.

## Green-bar checkpoint

- `docs/perf/notes-api-2026-05-26.md` exists with before/after numbers.
- `docs/perf/methodology.md` documents how you measured (warmup,
  concurrency, duration).
- Your fix is in a PR with the report referenced from the description.

## What's next

Phase 12 — **Principal Engineer Skills** — the non-code part of the job.
