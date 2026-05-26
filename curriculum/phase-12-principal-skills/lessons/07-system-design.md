# Lesson 12.7 — System Design

> **Concept first:** large systems are decomposed into services that
> communicate. Picking the boundaries and protocols is the L7 skill.
> **Time:** 25 minutes.

## The conversational arc of a system-design discussion

1. **Clarify requirements.** What scale? Read/write ratio? Latency
   targets? Consistency requirements?
2. **Sketch the components.** What services? What data stores? What
   message queues?
3. **Pick the data model first.** Most design decisions follow.
4. **Pick the consistency model.** Strong, eventual, causal,
   read-your-writes — pick deliberately, not by default.
5. **Handle failure.** What happens when X is down? Retries? Fallbacks?
6. **Capacity math.** RPS × payload bytes × N services = bandwidth +
   storage + CPU.
7. **Operability.** Deploy, observability, on-call.

A typical 45-minute design discussion spends 25 minutes on steps 1–3,
15 on 4–5, 5 on 6–7. Most engineers reverse it. Don't.

## Decomposition heuristics

When to split a service:

- **Different data ownership.** Auth owns users; billing owns
  subscriptions; content owns posts. Different schemas, different
  caches, different SLOs.
- **Different scaling characteristics.** A billing service does ~10
  RPS; a recommendation engine does ~10000 RPS. Different fleets.
- **Different deploy cadence.** A safety-critical service deploys
  weekly; a feature service deploys hourly. Different risk profiles.

When *not* to split:

- "It feels modular." Modularity is achieved with modules, not services.
- "Microservices are best practice." They're not, until you have the
  ops maturity.
- "Different team." If two teams need to coordinate on every change,
  you've split your service wrong, not split it well.

## Consistency: the spectrum

| Model | Example | Use |
|---|---|---|
| **Strong** | "After I write, every read sees my write" | Money, auth, anything where a stale read is wrong |
| **Read-your-writes** | "*Your* reads see *your* writes; others may lag" | UI freshness for the actor |
| **Eventual** | "Reads catch up over seconds" | Caches, feeds, analytics |
| **Causal** | "If A caused B, every observer sees A before B" | Chat, comments, ordering-sensitive UX |

The trade-off: stronger consistency means more coordination, which means
more latency or less availability under partition (CAP).

For MemberClub:

- Strong: billing, sessions, subscription state.
- Read-your-writes: a user's own notes after they create one.
- Eventual: tier projection updated by webhook (5-min staleness OK).

## Back-pressure

When a downstream is slow, an upstream that doesn't slow down accumulates
work until it falls over. Three patterns:

- **Bounded queues.** mpsc with a fixed buffer; producers wait when
  full. Phase 2.4.
- **Circuit breakers.** After N failures to call X, stop trying for M
  seconds; return a cached/default response.
- **Load shedding.** When latency or queue depth exceeds threshold,
  refuse *new* work with `503 Service Unavailable` + `Retry-After`.

The "no back-pressure" failure mode: a slow DB causes the app to queue
requests indefinitely, until the request queue fills memory and the app
OOMs. *Way* worse than refusing requests cleanly at the edge.

## Capacity math

For a target:

> "Handle 10000 RPS of `GET /v1/notes/{id}` at p99 < 100ms."

Sketch:

```
10000 RPS × 1 KB payload = 10 MB/s egress
10000 RPS × 0.1 ms CPU = 1 vCPU minimum (×3 for headroom)
Cache hit ratio target: 90% (1000 RPS to DB)
DB QPS budget: 1000; conn pool 50 × 8 pods = 400 conns; each at ~25ms = 16 QPS each
```

If the math doesn't close, the design doesn't work. Find the constraint
*before* you ship.

## Failure modes

Walk every external dependency and ask: what if it's down?

- **DB primary unreachable.** Read replica? Fail-fast 503? Both.
- **Stripe API down.** Mirror reads from our DB; queue writes via
  outbox.
- **Email provider down.** Queue messages in outbox; retry with backoff.
- **OS layer down (datacenter).** Multi-region? Active-passive?
  Active-active?

Pick one or two failure modes you *do* handle gracefully and document
them. Three is ambitious; ten is fantasy.

## A worked example: MemberClub's "usage-based billing"

Following the conversational arc:

1. **Requirements:** 1000 customers × 10000 usage events/day average,
   peak 1000 events/sec for any single customer.
2. **Components:** new `usage-recorder` service receiving writes;
   `usage-aggregator` cron; existing `notes-api` for read APIs.
3. **Data model:** `usage_records (id, customer_id, kind, amount,
   created_at)` partitioned by month.
4. **Consistency:** writes are eventual (1-minute lag OK); read-your-writes
   for the customer's own dashboard via reading the record table
   directly.
5. **Failure:** `usage-recorder` queues to local disk if DB unreachable;
   aggregator re-runs idempotently.
6. **Capacity:** 1000 RPS × 200 B = 200 KB/s. Negligible.
7. **Operability:** existing observability stack; one new dashboard;
   one new alert.

This entire conversation fits on one whiteboard. The next step is the
RFC (Lesson 12.3).

## Why this matters

- **The big-picture conversation is the Principal's job.** Get good at
  it.
- **Capacity math saves you from "it's slow under load" surprises.**
- **Designing for failure is non-negotiable** past a certain scale. Most
  outages are *unhandled* failure modes.

## Green-bar checkpoint

- You can quote the seven steps of a design conversation in order.
- You can pick a consistency model for three sample requirements.
- You can sketch the back-pressure pattern.

Next: `lessons/08-capstone-deliverables.md`.
