# ADR 0006 — Outbox pattern over external message broker

- Status: Accepted
- Date: 2026-05-26
- Deciders: api-team, infra-team
- Tags: jobs, queues, infra, reliability

## Context and Problem Statement

We need to perform side-effects asynchronously: emailing receipts after
a successful Stripe webhook, recomputing analytics after a subscription
event, sending welcome messages after signup. Each side-effect must be
*atomic* with the business write that triggered it — no orphaned
mailings, no missed business writes.

## Decision Drivers

- Atomic write of "business row + side-effect intent."
- No new infrastructure to operate.
- Idempotent dispatch (retries are normal).
- Scales to thousands of jobs per minute.

## Considered Options

1. **Direct broker push (RabbitMQ / Kafka / SQS) inside the handler.**
   Risk of "wrote to DB, failed to push to broker" or vice versa.
2. **Outbox table in Postgres** + a worker that polls
   `FOR UPDATE SKIP LOCKED`. No new infra; atomic with the business
   transaction.
3. **Database triggers + `LISTEN/NOTIFY`.** Fast, but failures harder
   to trace; can't backoff.

## Decision Outcome

Chose **option 2**.

- `outbox` table with `(id, kind, payload, status, attempts,
  next_attempt_at, created_at, completed_at)`.
- Writers `INSERT` the outbox row *inside the same transaction* as the
  business write. Both commit or neither does.
- Workers `UPDATE … SET status='processing' … RETURNING * WHERE id IN
  (SELECT id FROM outbox WHERE status='pending' AND next_attempt_at
  <= NOW() ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1)`.
- Failed dispatches set `status='pending'` with exponential
  `next_attempt_at` backoff.
- Dispatch must be idempotent (uniqueness on Stripe API idempotency
  keys, email message ids, etc.).

## Consequences

- **Positive:** atomic with the business transaction; replay-friendly;
  uses Postgres we already operate; clear failure-mode story.
- **Negative:** Postgres carries the queue load; sustained > 1000
  jobs/sec would need an external broker.
- **Mitigations:** monitor `outbox_pending_count`; if it sustains > 10k
  for an hour, scale workers; if Postgres CPU dominates, revisit.

## Notes

Related: Phase 11.5; project ideas in EXERCISES E11.4.

The decision to migrate to an external broker is itself an ADR
(future). Until then, the outbox handles MemberClub's load.
