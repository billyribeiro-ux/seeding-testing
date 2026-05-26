# RFC 0000 — Template

> Copy this file to `NNNN-your-slug.md` and fill it in.

- Author: @you
- Status: Draft | In Review | Adopted | Rejected | Superseded
- Reviewers: @people who will give feedback (assign explicitly)
- Date: YYYY-MM-DD
- Related: ADR-NNNN, prior RFCs, issues

## Summary

One paragraph. What do we want to do? In plain English. A senior who
hasn't seen the project should understand the proposal from this
paragraph alone.

## Motivation

Why now? What evidence justifies it?

- Customer requests (link to support tickets).
- Data (graphs, query results).
- Competitive pressure.
- Engineering pain (a maintenance burden, a frequent incident).

## Goals

- **G1.** A specific, measurable outcome.
- **G2.** Another one.
- **G3.** (Usually three is the right number.)

## Non-goals

- **N1.** What we're *not* doing, even though someone will ask.
- **N2.** Scope hygiene.

## Proposed Design

### Data model
The schema changes; the new tables. SQL DDL acceptable here.

### Flow
Sequence diagrams, ASCII or otherwise. The end-to-end happy path.

### Failure modes
What goes wrong, and what we do about it. (Each downstream dependency:
"if X is down, then …")

### Alternatives considered
For each: pros, cons, why we didn't choose it.

## Decision Matrix

If the choice between alternatives isn't obvious, build the matrix:

| Option | Implementation cost | Latency | Customer fit | Risk |
|---|---|---|---|---|
| A | 2 weeks | medium | medium | low |
| B (chosen) | 5 weeks | high | high | medium |
| C | 3 weeks | high | low | low |

## Implementation plan

By week, with owners:

- **Week 1:** schema + tests. @alice.
- **Week 2:** the write path. @bob.
- **Week 3:** the read path. @bob.
- **Week 4:** background aggregation. @alice.
- **Week 5:** UI and dashboards. @carol.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Customers confused by the new pricing model | Real-time dashboard + soft alerts at 80% |
| Migration to existing customers | Default to "off"; opt-in only |

## Migration

How existing customers / data move to the new state. Pre-conditions,
back-out plan, communication.

## Open questions

Enumerate. *Do not* pretend they're answered.

1. How do refunds interact with the new feature?
2. Does Salesforce CPQ need a quote-only flow?

## Decision

(Filled when the review concludes.)

- **Outcome:** Adopted / Rejected / Adopted with modifications.
- **Decider:** @senior.
- **Date:** YYYY-MM-DD.
- **Follow-up ADR:** docs/01-architecture-decisions/NNNN-slug.md
