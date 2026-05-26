# Lesson 11.1 — Measure Twice, Optimize Once

> **Concept first:** every performance "improvement" without a
> before/after measurement is technical debt with extra steps.
> **Time:** 15 minutes.

## The discipline

```
1. State the goal.        "p99 of /v1/notes under 200ms at 100 RPS."
2. Measure today.          "Current p99 is 480ms at 100 RPS."
3. Hypothesize.            "The N+1 in fetching note metadata dominates."
4. Profile to confirm.     trace says 6 sequential DB queries per request.
5. Fix.                    Batch with a JOIN.
6. Re-measure.             p99 now 95ms at 100 RPS.
7. Ship + document.        docs/perf/notes-list-2026-05-26.md
```

Step 6 is the one juniors skip. Without it, "the fix" might have made things
worse — and you'd never know.

## The "should I optimize" decision tree

```
Is the metric breaching the SLO?
├── Yes → optimize
└── No → does optimizing free engineering time?
    ├── Yes → optimize (e.g. faster CI, faster dev loop)
    └── No → don't. Spend the effort on features.
```

A 10× speed-up on a code path nobody hits is wasted engineering time.

## Latency budget

A single user-facing request often spans multiple services. Each has a
slice of the total budget:

```
"User clicks Subscribe → confirmation visible"   target: 1500ms

  Browser navigation:           50ms
  SvelteKit SSR:               150ms
  Auth check (cookie + DB):    100ms
  Stripe Checkout Session API: 400ms (third-party; outside our control)
  Redirect + Stripe page load: 800ms (Stripe's UX; outside our control)
  ───────────────────────────────────
                  total:      1500ms ✓
```

When the total budget breaches, you negotiate which slice shrinks first.
A budget makes the conversation concrete.

## What "p99" actually means

p99 is "the 99th percentile latency over some window." 1% of requests are
slower; 99% are faster.

p50 (median) is what your *average* user feels.
p99 is what your *unlucky* user feels — and the one who tweets about it.

Optimize p99 — not the average. The mean is misleading; outliers are what
customers remember.

## Methodology checklist

When you measure, document:

- **Hardware** — local dev vs prod-like staging vs production.
- **Concurrency** — RPS, parallel connections.
- **Warmup** — 30 seconds of traffic before recording.
- **Window** — 5 minutes minimum for stable percentiles.
- **Data scale** — empty DB vs realistic dataset.
- **Cold vs warm cache** — both matter.

A "performance test" missing any of these is a vibe.

## Why this matters

- **Without a number, you can't tell whether you helped.**
- **Latency budgets force conversations** about where to cut.
- **p99 is the customer-visible quality bar.** Mean is for marketing
  slides.

## Green-bar checkpoint

- You can write a methodology checklist for a load test.
- You can explain why p99 > mean is normal and what to do about it.
- You can name three reasons *not* to optimize even when something is
  slow.

Next: `lessons/02-profiling-rust.md`.
