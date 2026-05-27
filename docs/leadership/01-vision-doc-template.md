# 3-Year Technical Vision — Template + Worked Example

A vision document is the artifact a staff/principal engineer writes
*before* the RFCs. RFCs propose multi-week projects. A vision doc
proposes which projects are worth proposing.

The template below is intentionally short. The worked example that
follows it is intentionally long, because reading one good vision doc
teaches more than reading three template skeletons.

## Table of contents

- [Why a vision doc, and not an RFC](#why-a-vision-doc-and-not-an-rfc)
- [The template](#the-template)
  - [1. The next 3 years](#1-the-next-3-years)
  - [2. Current state assessment](#2-current-state-assessment)
  - [3. The big bets](#3-the-big-bets)
  - [4. Sequencing and dependencies](#4-sequencing-and-dependencies)
  - [5. Risks and mitigations](#5-risks-and-mitigations)
  - [6. What we are NOT doing](#6-what-we-are-not-doing)
  - [7. Open questions](#7-open-questions)
- [Worked example: MemberClub 2026 → 2029](#worked-example-memberclub-2026--2029)

---

## Why a vision doc, and not an RFC

An RFC is a *proposal*. It asks "should we build X?" and exits with a
yes/no.

A vision doc is a *thesis*. It asks "where is the business going, and
what does that mean for our architecture?" and exits with a
prioritized list of bets, each of which will spawn one or more RFCs
later.

Three tells that you need a vision doc and not an RFC:

1. You can't write a single RFC because the work spans 4+ quarters.
2. You can't write a single RFC because it would force decisions on
   adjacent teams you don't own.
3. You keep getting "but what about X?" pushback in design reviews and
   you suspect the real question is "what is the strategy?"

Vision docs are read by VPs and CTOs. RFCs are read by the team.
That difference dictates everything about how the document is written:
short, no jargon-without-glossary, explicit about trade-offs, and
ruthless about saying what you are *not* doing.

## The template

Copy from here. Replace bracketed placeholders. The headers below
are mandatory. Skip none.

```
# [Product / Surface] Technical Vision — [START YEAR] → [END YEAR]

**Author:** [name, title]
**Status:** Draft | Reviewed | Approved
**Last updated:** [YYYY-MM-DD]
**Reviewers:** [VPE], [CTO], [Head of Product], [tech-lead names]
```

### 1. The next 3 years

One paragraph. No more, no less. This is the "why I am writing this"
section, and it has to land in 90 seconds because that's all the VP
will give you on first read.

Answer three questions:

- Where is the **business** going? (Revenue mix shift, geo expansion,
  new product surface, regulatory change.)
- Why does that imply a **technical** shift? (If the answer is "it
  doesn't" then either the vision doc is unnecessary or you are
  hiding the implication.)
- What's the **shape** of the answer? (One sentence that someone could
  quote at a board meeting.)

If you cannot say in one paragraph where the business is going, you
do not have a vision yet. Go back and talk to product, sales, and
finance until you do.

### 2. Current state assessment

Honest, sometimes uncomfortable, read of where the code and the team
sit *today*. The audience is the VPE; assume zero rose-coloring.

Recommended sub-sections:

- **What's load-bearing.** The systems that, if broken, take the
  product down. List by name and link to the code. The first time
  someone reads the doc they should learn what these are.
- **What's tech-debt.** Specifically: what would slow us down if we
  tried to ship the bets in section 3? Don't list debt for its own
  sake — list debt that *gates* the bets.
- **What we got wrong.** Past bets that did not pay off. Naming them
  builds credibility for the new bets.
- **What's working.** Systems that punch above their weight. Identify
  why so you can keep doing that.

This section should be readable as a standalone "state of the
codebase" memo. If you cannot say what's load-bearing without
referring to a JIRA ticket, you are not senior enough to write this
doc — go pair with the person who owns it.

### 3. The big bets

Three to five bets. Not two, not seven. Each bet gets its own H3.

For each bet, four sub-fields:

- **The bet.** One sentence. "We bet that ___ will become ___, and
  that ___ is the right primitive."
- **The evidence we'd see if it's right.** Quantitative where
  possible. "If by Q3 2027 we have N customers using this and
  retention on the cohort is X% higher, the bet is paying off."
- **Kill-criteria if it's wrong.** What would cause you to walk away?
  Specific, observable, and ideally measurable inside one year. A bet
  with no kill-criteria is faith, not strategy.
- **Cost in team-quarters.** Headcount × quarters. Be honest about
  it. If you're underestimating, the reviewer will catch you; if you
  pad, the reviewer will catch you. Show your work.

The bets must be *ordered* by priority — bet #1 is the one you would
defend hardest if the budget were cut in half.

### 4. Sequencing and dependencies

A Gantt-style ASCII chart or a numbered dependency list. The point is
to make explicit what must happen *before* what, so a reader can
quickly see which bets are blocked by which other bets, and which
teams are on the critical path.

Use real quarters (`2026 Q1`, not "soon"). If you can't put real
quarters on it, you don't have a sequencing plan, you have a wishlist.

### 5. Risks and mitigations

Top five risks. Not the full list — a vision doc with 27 risks
listed is one whose author is hedging. Pick the five that, if any one
materialized, would cause you to rewrite the doc.

For each risk:

- **Description.** What could happen.
- **Likelihood × impact.** Low / Med / High on each.
- **Mitigation.** What you'd do *now* to reduce likelihood or impact.
- **Contingency.** What you'd do *then* if it happened anyway.

Risks come from four buckets, and a good list spans all four:

- **Technical** — a primitive doesn't scale, a vendor deprecates.
- **People** — key person quits, can't hire the niche role.
- **Business** — product strategy shifts, a deal falls through.
- **External** — regulator changes the rules, a peer ships first.

### 6. What we are NOT doing

This section is what separates L7s from L6s.

An L6 vision doc lists what we *will* do. An L7 vision doc lists
what we *will not* do and explains why — because saying no costs
credibility, and the senior engineer is the one who spends credibility
on focus.

Each non-goal gets one sentence on *what* and one sentence on *why
not*. Be specific. "We are not building a service mesh" is a
non-goal. "We will improve performance" is not.

Examples of good non-goals:

- "We are not moving off Postgres for the foreseeable future."
- "We are not building an internal feature-flag system; we use
  LaunchDarkly."
- "We are not adopting Kubernetes for the application tier."
- "We are not supporting on-prem deployments."

Each of these is a decision that the company will live with for years.
Naming them in the vision doc means everyone can plan around them
instead of relitigating in every design review.

### 7. Open questions

The questions you cannot answer alone. List them with the person whose
input you need and the deadline by which you need an answer.

A vision doc that has *no* open questions is one whose author has
either thought about the problem so long that they no longer notice
the assumptions, or one whose author is presenting fait accompli. In
either case the reviewer will be suspicious.

A reasonable count: 3–8 open questions. Each is a sentence; the
discussion belongs in the comment thread, not the doc body.

---

## Worked example: MemberClub 2026 → 2029

This worked example uses the codebase in this repo as its substrate.
All bets are constrained to things the existing code could legitimately
evolve into within three years.

```
# MemberClub Technical Vision — 2026 → 2029

Author:        E. Engineer, Principal Engineer, Platform
Status:        Draft v0.4
Last updated:  2026-05-27
Reviewers:     VPE (J. Park), CTO (R. Liu), Head of Product (K. Sato),
               TLs: Auth (M. Diaz), Payments (S. Khan),
               Frontend (P. Nguyen), Data (A. Iyer)
```

### 1. The next 3 years

Over the next three years MemberClub shifts from a single-tenant
US-only subscription product to a multi-tenant, multi-region,
usage-billed *platform* that other businesses build their member
programs on top of. The technical implication is that the primitives
we currently use to *run* the product — the dual-mode auth, the
Stripe slice, the outbox, the multi-tenant RLS — must become the
*product*: stable, documented, versioned, and consumable by external
developers. **The shape of the answer is "every internal capability
becomes an external surface by 2029, or we delete it."**

### 2. Current state assessment

#### What's load-bearing

- **Auth.** The dual-mode session-cookie + JWT pattern in
  [`projects/04-auth-demo`](../../projects/04-auth-demo/) is the
  enforcement point for every API and every web page. If it goes
  down, nothing else works. ADR
  [`0004-dual-mode-auth`](../01-architecture-decisions/0004-dual-mode-auth.md)
  documents the design; the implementation is ~1200 lines of Rust
  with 18 integration tests.

- **Money.** The `Money(i64 cents, Currency)` primitive in
  [`projects/06-stripe-money-lab`](../../projects/06-stripe-money-lab/)
  is the only money type in the repo. Every dollar amount the company
  ever quotes, charges, or refunds flows through it. ADR
  [`0003-money-i64-cents`](../01-architecture-decisions/0003-money-i64-cents.md)
  documents the $21B ceiling and the no-floats rule. 25 tests
  including proptest coverage of the proportional split.

- **The outbox.** [`projects/08-outbox-demo`](../../projects/08-outbox-demo/)
  is the reference implementation for every side-effect that must
  happen "after" a business transaction commits — email, webhook,
  third-party API call, analytics. The SQLite version is
  illustrative; the Postgres version using `FOR UPDATE SKIP LOCKED`
  is what runs in production. ADR
  [`0006-outbox-over-broker`](../01-architecture-decisions/0006-outbox-over-broker.md).

- **Stripe.** All recurring billing flows through the webhook
  receiver in
  [`projects/07-webhook-receiver`](../../projects/07-webhook-receiver/),
  which writes to the outbox before responding 2xx. ADR
  [`0008-stripe-is-the-rail`](../01-architecture-decisions/0008-stripe-is-the-rail.md).

- **Tenant isolation.** The RLS primitives in
  [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/)
  are how every tenant-scoped query knows which tenant it's for. ADR
  [`0007-postgres-rls-tenant-isolation`](../01-architecture-decisions/0007-postgres-rls-tenant-isolation.md).
  Currently used by ~6 of our 11 tenant-scoped tables; the migration
  to cover all 11 is in flight.

#### What's tech-debt

- **The notes-api / memberclub split.** Historical accident: notes
  was the throwaway tutorial project, MemberClub is the product, and
  yet 30% of the auth and observability code lives in notes-api and
  is duplicated in memberclub. We should pick one as the canonical
  axum-app crate and extract a shared `mc-http` library. Gates
  Bet #3 (Auth-as-a-Service) because that bet ships the auth code as
  a published crate and we can't publish 30% duplication.

- **The dual-mode auth code path.** Has been useful but the carry
  cost is real: every endpoint authentication has two code paths,
  every test must cover both, every postmortem starts with "which
  mode was the request?" 80% of API traffic is JWT, 100% of web
  traffic is cookie; there's no traffic in the overlap. Gates
  Bet #5 (deprecate dual-mode).

- **The outbox dispatcher is a trait but only has one impl.** The
  `Dispatcher` trait in `projects/08-outbox-demo/src/lib.rs` is wired
  up to support pluggable side-effects but in practice we only ever
  use the in-process HTTP dispatcher. Either delete the trait or
  ship the second impl (a Kafka-style external dispatch). Either is
  fine; the current state is the worst of both.

- **The 02b-sqlite-notes-svelte / 02c-sqlx-notes pair.** Two
  implementations of the same "notes" feature, one SQLite + Drizzle,
  one Postgres + sqlx. Useful as a teaching ladder; load-bearing for
  zero of our customers. Should be moved out of `projects/` into a
  `curriculum/` subtree so it stops contributing to mental overhead
  during code review.

- **No EU presence.** We have no shard, no DPA template, no GDPR
  data-subject-request handler. The RLS primitives let us isolate
  tenants logically; they do not (yet) let us isolate *data
  residency*. Gates Bet #2 (EU shard).

#### What we got wrong

- **The `02-quote-generator` async-stream demo** was supposed to
  evolve into a streaming API. It never did. We should stop linking
  to it from product-facing docs and move it to `curriculum/` along
  with the 02b/02c pair.

- **The Casbin evaluation.** Burned a week proving Casbin was not the
  right authorization engine for us before deciding to write the
  explicit policy code that became
  [`projects/05-rbac-policy-lab`](../../projects/05-rbac-policy-lab/).
  Result was good; the cost was a week of distraction. ADR
  [`0005-explicit-policies-no-casbin`](../01-architecture-decisions/0005-explicit-policies-no-casbin.md)
  captures the lesson. Cost: 1 engineer-week.

- **First webhook receiver attempt.** The original
  `07-webhook-receiver` did not write to the outbox; it called Stripe
  back synchronously. We had two incidents in three months before
  rewriting it. Cost: 2 incidents, 8 engineer-days, one customer
  apology email.

#### What's working

- **Rust + Postgres as the load-bearing pair.** Zero memory-safety
  incidents in 18 months. Two database incidents, both human (a
  missing index and a forgotten `WHERE` in a manual UPDATE), both
  caught by replicas. The choice from ADR
  [`0001-rust-axum`](../01-architecture-decisions/0001-rust-axum.md)
  has paid off; we should *not* relitigate it.

- **The "no ORM" stance.** ADR
  [`0002-orm-stance`](../01-architecture-decisions/0002-orm-stance.md):
  sqlx with hand-written SQL. Engineers ramp up faster than the team
  expected and bugs are easier to triage because the SQL is right
  there. Keep going.

- **`make verify`** as the single command. CI parity with local has
  saved roughly 1 hour per engineer per week (informal estimate from
  the last team survey). Whatever the next bet costs, it must not
  break `make verify`.

- **The curriculum.** The phase-00 → phase-12 onboarding has cut
  ramp-up for new hires from 6 weeks to ~3. Worth more than any
  single feature we've shipped this year.

### 3. The big bets

#### Bet 1 — Usage-based billing on the existing outbox + Stripe slice

- **The bet.** We bet that within 3 years more than half of our
  ARR will come from usage-based pricing (per-active-member,
  per-engagement-event, per-stored-record) rather than the flat
  seat-tier pricing we ship today, and that the right primitive
  to build this on is the existing outbox in
  [`projects/08-outbox-demo`](../../projects/08-outbox-demo/) — each
  billable event is just another row in the outbox table, dispatched
  by a new `MeterDispatcher` impl of the `Dispatcher` trait. The
  Stripe slice ([`projects/06-stripe-money-lab`](../../projects/06-stripe-money-lab/)
  + [`projects/07-webhook-receiver`](../../projects/07-webhook-receiver/))
  already proves we can move from `i64 cents` to a Stripe charge
  reliably; usage billing adds an aggregation step but not a new
  primitive.

- **The evidence we'd see if it's right.**
  - By 2027 Q2: at least 3 customers on usage-based pricing,
    generating $50k+ ARR each.
  - By 2027 Q4: usage-based MRR > 25% of total MRR.
  - By 2028 Q4: usage-based MRR > 50% of total MRR.
  - Aggregation latency P99 < 5 minutes from event to meter.
  - Zero reconciliation discrepancies > $1.00 between our meter and
    Stripe's billing.

- **Kill-criteria.** If by 2027 Q4 we have fewer than 5 paying
  customers on usage-based pricing, or our aggregation pipeline has
  produced more than two customer-visible billing errors > $100, we
  kill the bet and return to flat tiers. We do NOT iterate further;
  the cost of "almost usage billing" is worse than "no usage
  billing."

- **Cost.** 1 backend engineer + 0.5 data engineer + 0.25 designer ×
  4 quarters = 2.75 team-quarters phase 1. Add 1 engineer × 2
  quarters for Stripe Billing integration in phase 2 = 2.0
  team-quarters. **Total: ~4.75 team-quarters.**

#### Bet 2 — EU shard built on the multi-tenant-rls primitives

- **The bet.** We bet that EU revenue will be > 20% of total revenue
  by 2028 and that GDPR-compliant data residency is non-negotiable
  for the largest EU customers we want to land. The primitive we'll
  build it on is the existing tenant-isolation work in
  [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/),
  extended from row-level isolation to shard-level isolation. A
  tenant's `tenant_id` already implies an `app.tenant_id` GUC; we'll
  layer an `app.region` GUC that the router enforces, and run a
  physically separate Postgres cluster in eu-west-1.

- **The evidence we'd see if it's right.**
  - By 2027 Q2: 5+ EU pilot customers on the shard.
  - By 2027 Q4: 1+ Fortune-500 EU enterprise customer signed.
  - DPA template approved by 2 EU enterprise customers' legal teams
    without redlines on the residency clause.
  - Cross-region read latency P99 < 50ms for the global control
    plane (the per-tenant data plane is region-local).

- **Kill-criteria.** If by 2027 Q4 EU revenue is below $200k ARR or
  we have lost two deals where data residency was *not* the cited
  blocker, we kill the bet. We do NOT keep the shard running for
  prestige; the operational cost of a multi-region setup is real.

- **Cost.** 2 backend + 1 SRE + 0.5 legal × 3 quarters = 10.5
  team-quarters. Note: legal counts as 0.5 of an engineering quarter
  for cost-modeling; their actual hours are smaller but their
  scheduling friction is high.

#### Bet 3 — `mc-auth` as a published crate (Auth-as-a-Service for ourselves first, then the world)

- **The bet.** We bet that the dual-mode auth pattern we've stabilized
  in [`projects/04-auth-demo`](../../projects/04-auth-demo/) is
  reusable enough that we should extract it into a `mc-auth` crate
  consumed by every internal Rust service first, and then — only if
  we love using it ourselves for 6+ months — published to crates.io
  as an open-source library. The internal extraction de-duplicates
  the ~30% drift between notes-api and memberclub. The public release
  is the open-source flywheel: contributions, name recognition,
  pipeline of hires who've used the crate before.

- **The evidence we'd see if it's right.**
  - By 2026 Q4: `mc-auth` is the single auth dependency for both
    notes-api and memberclub; drift is zero.
  - By 2027 Q2: a third internal service (the upcoming admin
    console) consumes `mc-auth` with zero modifications.
  - By 2027 Q4: `mc-auth` is published to crates.io with 500+ weekly
    downloads.
  - By 2028 Q2: at least one external company has filed a
    non-trivial PR against the public repo.

- **Kill-criteria.** If after the internal extraction we find we need
  to fork `mc-auth` for any of our three services in 2027, the
  abstraction is wrong and we do *not* publish. Better to have an
  internal library than a public one we can't keep our promises on.

- **Cost.** 1 engineer × 2 quarters for the internal extraction =
  2.0 team-quarters. Add 0.25 engineer × 4 quarters of maintenance
  burden once published = 1.0 team-quarter (recurring). The OSS
  maintenance cost is the line item the
  [`03-oss-maintainership-playbook.md`](./03-oss-maintainership-playbook.md)
  in this directory is built to keep honest.

#### Bet 4 — In-house experimentation platform on the outbox

- **The bet.** We bet that the right primitive for experimentation
  (A/B tests, gradual rollouts, holdback measurement) is, again, the
  outbox + a small `experiments` schema, and not a third-party
  feature-flag vendor. The case: every exposure is just an outbox
  row tagged with the experiment id and the variant. The variant
  decision is computed by a hash on `(experiment_id, subject_id)` —
  no network call, no flake risk, no per-MAU pricing.

- **The evidence we'd see if it's right.**
  - By 2027 Q2: 10+ live experiments per quarter, vs. our current 0.
  - By 2027 Q4: at least one major product decision (e.g.
    onboarding redesign) attributed to an experiment.
  - Per-MAU cost of experimentation: $0 (vs. ~$0.05–$0.20 if we
    bought a vendor).

- **Kill-criteria.** If by 2027 Q4 we have run fewer than 5
  experiments OR the platform has produced one invalid p-value that
  shipped a bad decision to production, we buy the vendor. The cost
  of half-built statistics tooling is higher than the per-MAU price.

- **Cost.** 1 backend + 1 data × 3 quarters = 6.0 team-quarters.
  This is the bet with the highest variance — could be a 2-quarter
  effort, could be 8.

#### Bet 5 — Deprecate dual-mode auth in favor of JWT-only

- **The bet.** We bet that within 3 years the session-cookie
  authentication mode of
  [`projects/04-auth-demo`](../../projects/04-auth-demo/) will be
  unused by all of our active customers, and that the right
  end-state is a single-mode JWT-bearer auth (still HttpOnly when
  the client is a browser; the cookie *carrying* the JWT is the
  delivery mechanism, not a separate session). The win is the
  reduction in code-path multiplicity — every endpoint goes from two
  auth checks to one, every test halves, every postmortem skips a
  branch.

- **The evidence we'd see if it's right.**
  - By 2027 Q1: cookie-mode traffic < 5% of total auth traffic.
  - By 2027 Q4: cookie-mode is deprecated (warned in logs); no
    customer has filed an issue.
  - By 2028 Q2: cookie-mode code removed; ADR 0004 is updated with a
    "superseded" note.

- **Kill-criteria.** If at any point an enterprise customer cites
  cookie-mode as a critical requirement (e.g. their compliance team
  insists on opaque session cookies, not bearer tokens), we keep
  dual-mode and update this vision doc.

- **Cost.** 0.5 engineer × 2 quarters for the deprecation engineering
  + 0.25 engineer × 2 quarters for customer migration support = 1.5
  team-quarters.

### 4. Sequencing and dependencies

```
                 2026          2027          2028          2029
                 Q1 Q2 Q3 Q4   Q1 Q2 Q3 Q4   Q1 Q2 Q3 Q4   Q1 Q2
Bet 3 internal    ██ ██                                                  (gates Bet 5)
Bet 3 public                ██ ██ ██ ██   ██ ██
Bet 1 phase 1           ██ ██ ██ ██
Bet 1 phase 2                       ██ ██
Bet 2 EU shard          ██ ██ ██   ██ ██ ██                              (depends on RLS hardening)
Bet 4 experiments              ██ ██ ██   ██ ██
Bet 5 deprecate                         ██ ██ ██                         (depends on Bet 3 internal)
RLS-hardening          ██ ██                                             (pre-req for Bet 2)
                  └── prerequisite phase ──┘
```

Critical path: **RLS-hardening → Bet 2** is the longest chain. If
RLS-hardening slips a quarter, Bet 2 slips a quarter, and Bet 2 is
the largest revenue lever. Treat RLS-hardening as P0.

Bet 5 is *blocked* by Bet 3 (internal): we cannot delete dual-mode
auth code until the `mc-auth` extraction is the canonical
implementation, otherwise we'd have to rewrite the deprecation across
two code paths.

Bet 4 is independent and can be slipped without dragging anything
else.

### 5. Risks and mitigations

| # | Risk | L × I | Mitigation | Contingency |
|---|------|-------|------------|-------------|
| 1 | The outbox can't handle Bet 1's event volume (10k–100k events/sec) | M × H | Run a load test in 2026 Q3 on Postgres outbox with the production indexes; baseline at `projects/13-load-test`. Decide partition strategy before commiting Bet 1 phase 2. | Move usage events to a dedicated outbox shard or, if Postgres is genuinely insufficient, evaluate Kafka — *only then*, not as the default. |
| 2 | We can't hire a senior SRE for the EU shard | M × H | Start the search now (2026 Q2); offer relocation; identify a 0.5-FTE contractor as a backstop. | Delay Bet 2 by a quarter; reuse our existing US SRE as 50%-EU until we hire. Note: this is the most personnel-fragile bet. |
| 3 | Stripe deprecates an API we depend on (especially around metered billing) | L × H | Subscribe to Stripe's API changelog as an action item, not as a vibe; have the Stripe TL audit our integration every 6 months. | Pin the API version; budget 2 engineer-weeks for the migration; document in ADR 0008's revision history. |
| 4 | An EU regulator changes the rules mid-build (e.g. Schrems III) | L × H | Track regulatory news monthly; consult outside counsel before commiting Bet 2 phase 2. | Pause Bet 2, return to the open-question list, re-plan. The shard infrastructure has value even if the legal terrain shifts. |
| 5 | `mc-auth` becomes a maintenance albatross post-publication | M × M | Apply the OSS maintainership playbook from day 1: triage cadence, the 18-month deprecation rule, the maintainer's bill of rights. See [`03-oss-maintainership-playbook.md`](./03-oss-maintainership-playbook.md). | Archive the public repo; keep the internal crate. Failure to maintain OSS gracefully is worse than never publishing. |

### 6. What we are NOT doing

- **We are not moving off Postgres.** Not for the data plane, not for
  the analytics plane, not for the event stream. ADR
  [`0001-rust-axum`](../01-architecture-decisions/0001-rust-axum.md)
  and ADR
  [`0006-outbox-over-broker`](../01-architecture-decisions/0006-outbox-over-broker.md)
  both depend on this. Postgres at our scale (estimated 100k QPS
  read, 10k QPS write by 2029) is a solved problem. We will revisit
  *only if* a quantitative load test shows we cannot meet a
  latency SLO; we will not revisit for vendor pitches.

- **We are not adopting Kubernetes for the application tier.** We
  run on Fly.io plus managed Postgres today. Kubernetes is a
  perfectly fine tool that adds a full-time platform-engineering
  headcount the moment it lands. We have not seen the need yet, and
  the migration cost is bigger than the operational pain that
  motivates it. Storage and CI may still run on k8s where the
  vendor-managed offering is k8s under the hood; the *application*
  tier is not.

- **We are not building an internal feature-flag system separate from
  the experimentation platform.** If Bet 4 ships, it is also our
  feature-flag system. If Bet 4 is killed, we buy a vendor for both.

- **We are not adopting GraphQL.** Our API is REST + JWT bearer
  ([`projects/04-auth-demo`](../../projects/04-auth-demo/) and
  [`projects/10-memberclub-cli`](../../projects/10-memberclub-cli/)).
  Customers have not asked for GraphQL; the typical "one query
  fetches everything" advantage matters less when our largest
  resource graphs are 3 levels deep. We will revisit only if a
  customer signs a contract clause requiring it.

- **We are not supporting on-prem deployments.** SaaS only. The
  EU shard is the maximum geographic concession. On-prem would
  invalidate Bets 1, 3, and 4 simultaneously because each assumes
  we control the runtime.

- **We are not rewriting the SvelteKit web app in Next.js or any
  other framework.** Svelte 5 runes-first is working; the team
  knows it; vitest coverage in `apps/memberclub/web` is green.
  Re-platforming the frontend is a $1M+ project with no business
  driver. We revisit only if Svelte itself becomes unmaintained.

- **We are not splitting the monorepo.** The Cargo workspace at the
  root, plus the `apps/memberclub/web` SvelteKit project, is the
  unit of release. Splitting buys us nothing in 2026 and costs CI
  parity, shared tooling, and `make verify`.

- **We are not building our own observability backend.** We use
  OpenTelemetry → vendor. We will not chase per-MAU pricing into a
  self-hosted Tempo/Loki/Prometheus stack unless our spend exceeds
  $300k/year on the vendor, which our 3-year forecast does not
  reach.

### 7. Open questions

1. **Bet 1 — Stripe Billing vs custom metered:** do we lean on
   Stripe's metered billing primitives or compute the invoices
   ourselves and only call Stripe for the charge? Need: a Stripe
   pricing comparison from S. Khan by 2026 Q3.

2. **Bet 2 — what is the EU control-plane story?** A separate
   control plane per region, or a global control plane that holds
   metadata and a per-region data plane? Need: the Bet 2 RFC author
   (TBD) to write a design note by 2026 Q3.

3. **Bet 3 — what's the OSS license?** Apache-2.0 mirrors the rest
   of the repo (see `LICENSE-APACHE`); MIT keeps the dual-license
   parity (see `LICENSE-MIT`). Need: a legal sign-off — but on the
   timeline, the license question only blocks the public release,
   not the internal extraction.

4. **Bet 4 — do we hire a stats-specialist?** Either a data
   scientist who can own the statistics correctness layer or a
   senior engineer with strong stats. Need: VPE input on headcount.

5. **Bet 5 — what's the customer comms plan for cookie-mode
   deprecation?** Two enterprise customers explicitly use cookie
   mode; we need a 12-month deprecation notice and a migration
   runbook before Bet 5 begins. Need: PM input by 2026 Q4.

6. **Cross-cutting — what's the team shape?** This vision implies
   ~5 net-new headcount over 3 years (heaviest in 2027). Need: VPE
   sign-off on the hiring plan before any bet starts.

7. **Cross-cutting — what's our SLO posture?** We have informal
   "best effort" SLAs today. By the time Bet 2 lands we need
   contractual SLOs for EU enterprise. Need: an error-budget RFC
   from the SRE team by 2026 Q4.

8. **Cross-cutting — when do we hire a security lead?** Bets 1, 2,
   and 3 each independently increase our attack surface (more
   billing, more regions, more public code). Today the senior
   backend engineers split security work; that does not scale.
   Need: a security-hire conversation with the CTO by 2026 Q3.

---

## Appendix: how to review a vision doc

For the reviewer (VPE, CTO, peer L7+):

- **First pass (10 min).** Read sections 1, 3 (headlines only), 6.
  If section 1 doesn't convince you the business is going where the
  author claims, ask for evidence. If you can't agree with the bets
  in section 3 from the headline alone, ask the author what they'd
  cut first. Section 6 tells you whether the author has actually
  made trade-offs.

- **Second pass (30 min).** Read section 2 (current state) and
  the full text of each bet in section 3. Section 2 must be honest;
  if it isn't, the bets will be miscalibrated. Each bet must have
  kill-criteria; a bet without kill-criteria is a wish.

- **Third pass (60 min).** Walk the dependency graph in section 4
  and stress-test it. What's the critical path? What's the
  single-point-of-failure (usually a key hire). Read section 5 and
  the open questions; the doc is approved when both are
  *resolvable*, not when both are *resolved*.

For the author: a vision doc is not approved when everyone agrees.
It is approved when the disagreements are *named* in section 7 and
the path to resolution is known. If you remove a real disagreement
from the doc to make it land more easily, you've authored a press
release, not a vision document.
