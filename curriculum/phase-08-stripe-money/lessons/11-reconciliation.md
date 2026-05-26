# Lesson 8.11 — Reconciliation: Our Ledger vs Stripe's

> **Concept first:** mirror Stripe state via webhooks. Then *prove* the mirror is correct via a nightly reconciliation job.
> **Time:** 15 minutes.

## Why reconcile

Webhooks are reliable, but reality includes:

- Webhooks that never arrived (DNS hiccup; you 5xx'd them past the retry budget).
- Bugs in your handler that silently dropped events.
- Manual edits via the Stripe dashboard (an ops person voided an invoice
  without going through your code).

Reconciliation is the *catch-net*. Once a day, we ask Stripe "what
happened yesterday?" and compare to our DB. Discrepancies are alerts, not
mysteries.

## The job, in pseudocode

```rust
async fn nightly_reconciliation(pool: &PgPool, stripe: &stripe::Client) -> anyhow::Result<()> {
    let yesterday_start = today_in_utc() - chrono::Duration::days(1);
    let yesterday_end   = today_in_utc();

    // 1. Pull all charges Stripe recorded yesterday.
    let stripe_charges = stripe.list_charges(&ListCharges {
        created: Some(RangeQuery::between(yesterday_start.timestamp(), yesterday_end.timestamp())),
        ..Default::default()
    }).await?;

    // 2. Pull our mirror.
    let ours: Vec<Payment> = sqlx::query_as("SELECT * FROM payments WHERE succeeded_at >= $1 AND succeeded_at < $2")
        .bind(yesterday_start).bind(yesterday_end).fetch_all(pool).await?;

    // 3. Diff by stripe_charge_id.
    let stripe_ids: HashSet<&str> = stripe_charges.iter().map(|c| c.id.as_str()).collect();
    let our_ids:    HashSet<&str> = ours.iter().map(|p| p.stripe_charge_id.as_str()).collect();

    let missing_from_us: Vec<_> = stripe_ids.difference(&our_ids).collect();
    let extra_in_us:     Vec<_> = our_ids.difference(&stripe_ids).collect();

    if !missing_from_us.is_empty() || !extra_in_us.is_empty() {
        alert_ops(&format!(
            "reconciliation: {} missing from our DB, {} extras",
            missing_from_us.len(), extra_in_us.len()
        )).await?;
    }

    // 4. Sum-check.
    let stripe_total: i64 = stripe_charges.iter().map(|c| c.amount).sum();
    let our_total:    i64 = ours.iter().map(|p| p.amount_cents).sum();
    if stripe_total != our_total {
        alert_ops(&format!(
            "reconciliation: stripe ${:.2} vs our ${:.2}",
            stripe_total as f64 / 100.0, our_total as f64 / 100.0
        )).await?;
    }

    // 5. Record the run.
    sqlx::query("INSERT INTO reconciliation_runs (date, stripe_total_cents, our_total_cents, missing, extras) VALUES (...)")
        .execute(pool).await?;

    Ok(())
}
```

Five steps; one job; runs every day at 02:00 UTC.

## What "alert ops" means

In MemberClub:

- A Slack message to `#billing-alerts` with the diff summary.
- An incident in our on-call system if the dollar discrepancy > $100.
- A page if the missing-charge count > 0 (data integrity).

A 1-row drift is usually a webhook that was retried after the day boundary;
self-resolves the next day.

## The `reconciliation_runs` table

```sql
CREATE TABLE reconciliation_runs (
    id                  BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    date                DATE NOT NULL UNIQUE,
    stripe_total_cents  BIGINT NOT NULL,
    our_total_cents     BIGINT NOT NULL,
    missing_from_us     JSONB NOT NULL DEFAULT '[]'::jsonb,
    extras_in_us        JSONB NOT NULL DEFAULT '[]'::jsonb,
    completed_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Auditable record. Compliance loves this table.

## Stripe Sigma for the heavy lifts

For dashboards (MRR, churn cohort, ARPU), we don't query our DB — we use
**Stripe Sigma**, Stripe's SQL warehouse over their data. Reasons:

1. Stripe Sigma has every event Stripe ever observed, not just what our
   webhooks captured.
2. The schema is documented and stable.
3. Reports are auditable artifacts.

Our nightly reconciliation can also pull from Sigma for the comparison
side, which is sometimes more reliable than `list_charges` paging.

## The locking trick

Multiple reconciliation runs at once would be a mess. Use a Postgres
advisory lock so only one runs at a time across the fleet:

```sql
SELECT pg_advisory_xact_lock(hashtext('nightly-recon'));
```

Held for the transaction; released on commit. Other workers block.

## Why this matters

- **Reconciliation is the *evidence* that your webhooks worked.**
  Without it, you have hope, not knowledge.
- **A daily run with a clear pass/fail signal** gives you confidence to
  refactor billing code.
- **The Sigma cross-reference** catches webhook gaps no synthetic test
  could.

## Green-bar checkpoint

- You can sketch the five-step nightly job from memory.
- You can articulate why we trust Sigma over our DB for analytics.
- You can use a Postgres advisory lock to single-instance a cron job.

Next: `lessons/12-build-stripe-money-and-webhook.md`.
