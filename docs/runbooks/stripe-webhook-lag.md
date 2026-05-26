# Runbook: Stripe webhook lag

## Detection

Alerts that fire on this:

- **`webhook_lag`** — events with `received_at - created_at_stripe > 5min` for 10 min. *Warning.*
- **`webhook_unprocessed`** — `processed_at IS NULL AND received_at < NOW() - INTERVAL '10 min'` row count > 100. *Paging.*

## Immediate triage

```sql
-- What's the lag distribution right now?
SELECT
    NOW() - received_at AS receipt_age,
    NOW() - to_timestamp(created_at_stripe) AS event_age,
    event_type,
    processed_at IS NOT NULL AS done
FROM stripe_events
WHERE received_at > NOW() - INTERVAL '1 hour'
ORDER BY received_at DESC
LIMIT 20;
```

Three patterns to recognize:

| Pattern | Cause |
|---|---|
| Lots of rows, `done=false`, `receipt_age > 5 min` | The receiver is up but the processor is stuck. |
| `event_age` huge, `receipt_age` small | Stripe is delivering old events (replays or backlog). Our processing is fine. |
| Few rows, mostly done, lag growing | Webhook *delivery* is slow on Stripe's side. |

## Mitigation

### A — Processor stuck

```bash
# Check the processor's status
flyctl ps --app webhook-processor
flyctl logs --app webhook-processor | rg '"level":"ERROR"' | head -20
```

If the processor is crashed or stuck on a single bad event:

```sql
-- Find the row blocking everything
SELECT * FROM stripe_events
WHERE status = 'processing' AND received_at < NOW() - INTERVAL '5 min'
ORDER BY received_at LIMIT 5;
```

If a single event is stuck (e.g. the handler panics on it), mark it
`failed` and continue:

```sql
UPDATE stripe_events SET status = 'failed', completed_at = NOW()
WHERE id = <stuck_id>;
```

Restart the processor pods:

```bash
flyctl restart --app webhook-processor
```

### B — Stripe replay storm

Stripe replays a batch of historical events (e.g. after their incident
resolves). The receiver returns 200 to each because they're duplicates;
the processor has nothing to do. No action needed; let it drain.

Verify by checking:

```sql
SELECT event_type, COUNT(*) FROM stripe_events
WHERE received_at > NOW() - INTERVAL '1 hour'
GROUP BY event_type ORDER BY 2 DESC LIMIT 10;
```

If many of the events are `checkout.session.completed` from yesterday's
date, it's a replay storm.

### C — Stripe delivery slow

Check https://status.stripe.com. If Stripe reports an incident, there's
nothing we can do but wait. Notify customers if user-facing impact:

> "We're seeing delays from our payment provider. Subscriptions may take
> longer to activate. Engineering is monitoring."

## What customers will see

- **New signups paying with Stripe:** the "Welcome to Pro" email may
  arrive late, and `users.tier` updates may lag by minutes.
- **Failed payments:** the dunning email is delayed.
- **Refunds:** the receipt email is delayed, but the money already left
  the customer.

None of these are *broken* — just slow. Communicate accordingly.

## Recovery

After the lag returns to normal:

```sql
-- Are there any events we never processed?
SELECT COUNT(*) FROM stripe_events WHERE processed_at IS NULL;

-- For events older than 24 hours that are still pending, run a backfill.
-- This is a manual operation — coordinate with billing.
```

## Postmortem trigger

If lag exceeded 30 minutes or any customer was affected:

- Open a postmortem within 48 hours.
- Use `docs/03-postmortems/0000-TEMPLATE.md`.
- Include the lag chart from Grafana.

## Related

- ADR 0008 — Stripe is the rail, our DB is the source of truth
- ADR 0006 — Outbox pattern
- Phase 8.7 — Webhook signature + idempotency + replay safety
- `projects/07-webhook-receiver`
