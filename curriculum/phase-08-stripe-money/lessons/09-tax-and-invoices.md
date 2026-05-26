# Lesson 8.9 — Tax and Invoices

> **Concept first:** tax is jurisdiction-specific, time-varying, and audited.
> Outsource the rate logic to Stripe Tax (or a vendor). Store enough to reconstruct.
> **Time:** 15 minutes.

## Two ways to handle tax

| Approach | When |
|---|---|
| **Stripe Tax** | You sell in many jurisdictions. Stripe computes the rate, files the returns. |
| **Manual `tax_rates`** | You sell in one jurisdiction with a simple flat rate, or you have your own tax engine. |

For MemberClub we enable **Stripe Tax**. We get correct rates per
customer location, automatic registration tracking, and quarterly export
of the data we file with tax authorities.

## Tax-inclusive vs tax-exclusive pricing

- **Inclusive** (typical for EU consumer pricing): the listed price *includes*
  tax. $10 USD → customer pays $10; the merchant keeps $9.30, tax authority
  gets $0.70.
- **Exclusive** (typical for B2B and US pricing): the listed price *excludes*
  tax. $10 USD → customer pays $10.70.

Pick one per product. Stripe lets you set it on the Price.

## The data we store

```sql
CREATE TABLE invoices (
    id                    BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    stripe_id             TEXT NOT NULL UNIQUE,
    user_id               BIGINT NOT NULL REFERENCES users(id),
    subtotal_cents        BIGINT NOT NULL CHECK (subtotal_cents >= 0 AND subtotal_cents < 2_100_000_000_00),
    tax_cents             BIGINT NOT NULL CHECK (tax_cents >= 0 AND tax_cents < 2_100_000_000_00),
    total_cents           BIGINT NOT NULL CHECK (total_cents >= 0 AND total_cents < 2_100_000_000_00),
    currency              CHAR(3) NOT NULL,
    status                TEXT NOT NULL,                  -- 'draft'|'open'|'paid'|'void'|'uncollectible'
    invoice_pdf_url       TEXT,                           -- Stripe-hosted PDF
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Three rules:

1. **Store amounts as cents,** not the rendered dollar string.
2. **Store `tax_cents` separately from `subtotal_cents`.** You'll need both
   for analytics and for tax filings.
3. **Mirror the Stripe-hosted PDF URL** rather than rendering invoices
   yourself. Stripe handles localization, legal-required fields, the
   "INVOICE" header. Don't reinvent.

## Invoice items

When proration happens mid-cycle, Stripe creates *invoice items* — small
positive or negative line items that adjust the next invoice's total. We
mirror them in `invoice_items` if we need per-line analytics; otherwise
the aggregate `subtotal_cents` is enough.

## The math invariant

```
total_cents == subtotal_cents + tax_cents
```

Always. If they disagree, something is wrong in your mirror logic. Add a
DB-level `CHECK` constraint:

```sql
ALTER TABLE invoices ADD CONSTRAINT invoice_total_matches_parts
    CHECK (total_cents = subtotal_cents + tax_cents);
```

## Reconciliation

A nightly job sums `invoices.total_cents` for the previous day in test mode
and compares against Stripe's Balance Reports. Differences > 0.01% are
alerted. We'll cover the reconciliation job in Lesson 8.11.

## Why this matters

- **Tax law changes constantly.** Outsourcing the engine keeps you out of
  legal trouble.
- **Storing the parts (subtotal + tax)** means we can produce historical
  reports correctly even if the rates change later.
- **The PDF is the audit artifact.** Don't render invoices yourself; let
  Stripe.

## Green-bar checkpoint

- You can articulate the difference between tax-inclusive and tax-exclusive.
- You can write the `CHECK` constraint that enforces `total = subtotal + tax`.
- You can decide between Stripe Tax and manual rates for a given product.

Next: `lessons/10-refunds-and-disputes.md`.
