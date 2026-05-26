# Lesson 12.3 — RFCs and Design Docs

> **Concept first:** an RFC (Request for Comments) or design doc surfaces a
> non-trivial change for *debate* before code is written. Cheap iteration
> in prose; expensive iteration in code.
> **Time:** 20 minutes.

## RFC vs ADR

| | ADR | RFC |
|---|---|---|
| Audience | Future engineers | Current team for review |
| Purpose | Capture the decision *after* it's made | Surface a proposal *before* it's decided |
| Status flow | Proposed → Accepted/Rejected (immutable after) | Draft → Reviewed → Adopted/Rejected (then archived) |
| Length | 1 page | 3–10 pages |
| Frequency | Per architectural decision | Per non-trivial change |

An RFC often becomes an ADR — you adopt the RFC, then write an ADR
capturing the final decision.

## The template

```md
# RFC: Usage-Based Billing for MemberClub

- Author: @alice
- Status: Draft
- Reviewers: @bob (billing), @carol (product), @dave (data)
- Date: 2026-05-26
- Related: ADR 0008 (Stripe is the rail)

## Summary
One paragraph: what we want to do, in plain English.

## Motivation
Why now? What evidence (data, customer requests, competitive pressure)?

## Goals
- Customers can purchase additional storage as a usage-based add-on.
- Billing happens at the end of the month (post-paid).
- p99 latency on the add-on counter < 50ms.

## Non-goals
- We are not implementing per-second metering.
- We are not switching the base subscription model.

## Proposed Design

### Data model
Tables: `usage_records`, `usage_invoices`, `usage_tiers`.

### Flow
1. Application writes a `usage_records` row on each unit consumed.
2. Nightly job aggregates per customer.
3. Monthly job creates an invoice with the metered line item.

### Alternatives considered
- **Stripe Metered Billing.** Pros: native. Cons: per-event API calls
  hit Stripe rate limits at our volume; granular invoice rendering is
  inflexible.
- **Pre-paid credits.** Pros: cash up front; no surprise invoices. Cons:
  customers strongly prefer post-paid (5 customer interviews).

## Decision Matrix
| Option                 | Implementation | Latency | Customer fit | Risk |
|---                     |---             |---      |---           |---   |
| Stripe Metered         | 2 weeks        | medium  | medium       | low  |
| Custom + Stripe invoice| 5 weeks        | high    | high         | medium |
| Pre-paid credits       | 3 weeks        | high    | low          | low  |

## Implementation plan
- Week 1: data model + tests
- Week 2: usage write API
- Week 3: nightly aggregation
- Week 4: monthly invoice
- Week 5: customer dashboard

## Risks and mitigations
- *Risk*: tiered overage limit confuses customers.
  - *Mitigation*: real-time dashboard + soft alerts at 80%.

## Migration
- Existing customers default to "no metering" — no behavior change.

## Open questions
1. How do refunds interact with usage? (See E12.2)
2. Does Salesforce CPQ need a quote-only flow?

## Decision (filled when approved)
TBD.
```

Eleven sections. Three (Summary, Motivation, Proposed Design) are
non-negotiable.

## How to run a review

1. **Distribute early.** Send the link with two days' notice for
   reviewers.
2. **Set a review meeting** for tough items — async comments + one
   synchronous discussion.
3. **Capture every disagreement in the doc.** Even if not resolved.
4. **Authority makes the call.** A senior or principal decides. If
   the decision is contested, escalate to staff/director.
5. **Convert to ADR.** Once decided, write the ADR. The RFC stays as
   the historical artifact.

## When to *not* write an RFC

- The change is < 1 week of work.
- Reversible (e.g. internal-only API changes).
- The team has already agreed in a meeting.

If you're not sure, write the *outline only* and ask the team if they
want the rest.

## Anti-patterns

- **RFCs that justify a decision already made.** That's marketing, not
  RFC.
- **RFCs without explicit alternatives.** Same as ADRs.
- **RFCs that ignore data.** "I think X" is weaker than "Y customers
  reported Z."
- **Endless drafts.** Set a "decision by" date. After that date, default
  to the simplest option.

## Why this matters

- **Iterating in prose is 100× cheaper than iterating in code.**
- **Reviewers catch issues your IDE can't.** Concurrency, security,
  product-fit.
- **RFCs build alignment.** The team that reads the RFC won't be
  surprised by the PR.

## Green-bar checkpoint

- You can sketch the 11-section template from memory.
- You can name three things that *don't* need an RFC.
- You can run a review meeting that ends in a decision, not in
  "let's keep talking."

Next: `lessons/04-code-review-at-l7.md`.
