# Hiring and Interview Guide — Rust + Postgres + Stripe stack

This document is the playbook for hiring backend engineers into a
team running the stack in this repository: Rust + Axum, Postgres +
sqlx, Stripe, Svelte 5 frontends, dual-mode auth, RLS multi-tenancy.

It is opinionated. Every question, every rubric, every behavioral
prompt is grounded in something this codebase actually contains. The
implicit thesis is: an interview loop that's disconnected from the
work the candidate would actually do is an interview loop that
produces no signal worth the candidate's time.

## Table of contents

- [What this guide is and isn't](#what-this-guide-is-and-isnt)
- [Leveling bands](#leveling-bands)
  - [L4 — Software Engineer](#l4--software-engineer)
  - [L5 — Senior Software Engineer](#l5--senior-software-engineer)
  - [L6 — Staff Software Engineer](#l6--staff-software-engineer)
  - [L7 — Principal Software Engineer](#l7--principal-software-engineer)
- [The loop structure](#the-loop-structure)
- [Question bank](#question-bank)
  - [Phone screen](#phone-screen)
  - [On-site coding](#on-site-coding)
  - [System design](#system-design)
  - [Behavioral](#behavioral)
- [Grading rubrics](#grading-rubrics)
- [Calibration meeting playbook](#calibration-meeting-playbook)
- [The diversity-of-thought rule](#the-diversity-of-thought-rule)
- [Anti-patterns and traps](#anti-patterns-and-traps)

## What this guide is and isn't

**It is:**

- The interviewer-side playbook for our loop.
- A leveling rubric so that two interviewers leveling the same person
  at the same answer reach the same conclusion.
- A question bank an interviewer can pull from cold, with the answer
  key, the grading rubric, and the trap to avoid.

**It is not:**

- A career ladder. Levels here describe what we hire for; they do not
  describe how internal employees get promoted.
- A coaching document for candidates. Candidate-facing material lives
  on the careers page, not in `docs/`.
- A diversity hiring policy. Those policies are owned by People; this
  guide contains *one* operational rule (the diversity-of-thought
  rule on panel composition) that protects loop-quality signal but
  does not substitute for the broader policy.

## Leveling bands

Each band is defined on four axes:

- **(a) Code quality** — what their PRs look like.
- **(b) System design** — what they sketch on a whiteboard.
- **(c) Cross-functional** — how they work with PM, design, sales,
  support, SRE, security.
- **(d) Mentorship** — what their effect on the people around them
  looks like.

The bands are *inclusive upward*: an L6 still does the L5 things,
just not as the primary work.

### L4 — Software Engineer

**Years of experience:** 1–3 typical. Not a hard requirement.

**(a) Code quality.** Writes correct, idiomatic code in the
languages of the team after a 4–6 week ramp. Reads existing
patterns and copies them rather than inventing new ones. Asks for
review feedback and applies it without re-litigating. Their PRs
need 1–2 rounds of review for non-trivial work. Reading
[`projects/01-hello-cli/src/lib.rs`](../../projects/01-hello-cli/)
and producing a similar-shaped contribution is fair to ask.

**(b) System design.** Can read a system diagram and explain it. Can
extend a service by one endpoint without help. Cannot yet design a
new service end-to-end. Asked "design a notes API," they would
sketch something resembling our
[`projects/03-notes-api`](../../projects/03-notes-api/) with help
from the interviewer.

**(c) Cross-functional.** Talks to their immediate teammates daily.
Hands off to PM and design when there are questions. Does not yet
write requirements; reads them.

**(d) Mentorship.** Is mentored. Asks good questions in standups.
Sometimes pairs with a new intern.

### L5 — Senior Software Engineer

**Years of experience:** 4–7 typical, with the caveat that mid-30s
career-changers can hit L5 in 2 years if they have prior senior
experience elsewhere.

**(a) Code quality.** Owns features end-to-end. Writes idiomatic
Rust with the right error types, the right trait boundaries, the
right tests. Their PRs typically land with 1 round of review. Reads
the codebase before reaching for a new dependency. Notices when
they are duplicating an existing primitive (e.g. would not
re-implement `Money` if they read
[`projects/06-stripe-money-lab`](../../projects/06-stripe-money-lab/)).

**(b) System design.** Can design a single service end-to-end. Picks
sensible primitives (an outbox for reliable side-effects, RLS for
tenant isolation) without being told to. Asked "design a webhook
receiver," they would design something resembling
[`projects/07-webhook-receiver`](../../projects/07-webhook-receiver/)
with the outbox correctly applied. Cannot yet design across
multiple services with shared state.

**(c) Cross-functional.** Writes design notes. Joins customer calls
when called upon and represents engineering credibly. Negotiates
scope with PM rather than just accepting it.

**(d) Mentorship.** Pairs with L4s on hard bugs. Reviews their PRs
with substantive feedback. Does not yet design career growth for
the people they mentor.

### L6 — Staff Software Engineer

**Years of experience:** 7–12 typical, but heavily depends on prior
exposure to projects of equivalent complexity.

**(a) Code quality.** Owns a *system* — not just a feature.
Equivalent to owning all of
[`projects/04-auth-demo`](../../projects/04-auth-demo/) and being
the person the rest of the company asks before changing how auth
works. Their PRs typically land in one round; their reviews of
others' PRs catch subtle bugs (e.g. would catch a missing
`set_local_tenant_sql` call on a code path that bypasses the
middleware).

**(b) System design.** Designs across multiple services. Writes
RFCs that align teams. Authored or co-authored multiple ADRs in
our `../01-architecture-decisions/` set. Can defend a design choice
against a senior peer who disagrees, and can change their mind
gracefully when shown new evidence.

**(c) Cross-functional.** Spends real time with PM on roadmap, with
SRE on operational concerns, with support on customer signal. Drives
postmortems and the action items that come out of them. Could
credibly run an incident command.

**(d) Mentorship.** Mentors L4–L5 on a multi-quarter horizon. Owns
the technical career growth conversations for at least 2 engineers
on the team. Identifies when someone is ready to be promoted and
sponsors them.

### L7 — Principal Software Engineer

**Years of experience:** 10+ typical, but L7 is a behaviors band
more than a years band — we have seen L7-capable engineers at 8
years and L6-stuck engineers at 18.

**(a) Code quality.** Their hands-on code is now ~30% of their time
and is mostly load-bearing primitives: a shared library, an
incident-prevention refactor, a new auth flow. The other 70% is
direction-setting (see (b)–(d)).

**(b) System design.** Authors vision documents (see
[`01-vision-doc-template.md`](./01-vision-doc-template.md)). Spots
the architectural decision that, if missed, will block the company
2 years from now. Influences peer-team designs without owning their
roadmap. Comfortable saying "we are not doing X" and defending it.

**(c) Cross-functional.** Trusted by VP+ to translate business
strategy into technical strategy. Comfortable in customer
conversations with VPs of Engineering on the other side. Mediates
between product, sales, and engineering when their priorities
conflict.

**(d) Mentorship.** Multipliers the people around them. Multiple
engineers on the team would say their growth in the last 12 months
was directly attributable to this person. Identifies and grows the
next generation of L6s.

**The L6 vs L7 line:**

The single most useful distinction we've found is:

- An **L6** asks "is this the right way to do *this project*?"
- An **L7** asks "is this the right *project*?"

If your L6 candidate keeps escalating beyond the project framing
without prompting, lean upward in calibration. If your L7 candidate
keeps relitigating tactical decisions instead of stepping back to the
strategy, lean downward.

## The loop structure

A complete on-site loop is:

| Stage | Length | Interviewer count | Purpose |
|------|-------:|------------------:|---------|
| Phone screen | 60 min | 1 | Coding + SQL + trace-reading; gate for on-site |
| On-site coding | 60–75 min | 1–2 | Algorithmic + codebase navigation |
| System design | 60 min | 1–2 | Sized to target level |
| Behavioral | 45–60 min | 1 (often hiring manager) | Past behavior, motivation, growth |
| Bar-raiser | 45–60 min | 1 | Calibration across loops |

For L4: 4 stages.
For L5/L6: 5 stages (add a second on-site coding focused on debugging).
For L7+: 5 stages with the system design weighted heavier and the
coding stage replaced with a code-review exercise on real PRs.

Total interviewer time per loop: 4–7 hours. Total interviewer time
on the candidate's side: ~5 hours. Both numbers matter; both are
budgets to spend wisely.

## Question bank

Format for each question:

- **Stage:** which loop stage this fits.
- **Target level:** which level we'd default to running it at.
- **Time:** how long it should take.
- **The question:** how you'd state it.
- **What strong looks like:** the rubric for "strong hire."
- **What weak looks like:** the rubric for "no hire."
- **Trap:** the failure mode that makes this question score poorly
  even when the candidate is good.

### Phone screen

The phone screen has three questions, one each of coding, SQL, and
trace-reading. The candidate should leave knowing what they got
right and where they were weak — withholding feedback at the phone
screen burns goodwill for no gain.

#### PS-1 (coding) — "Implement a small CLI feature"

- **Stage:** Phone screen.
- **Target level:** L4–L5.
- **Time:** 25 min.
- **The question.** "I'll share a single file: a small CLI parser
  written in your language of choice. Add a `--config <path>` flag
  that loads defaults from a TOML file; CLI flags should win over
  config file values. Tests are provided; make them pass."

  Reference shape: the `clap`-based CLI in
  [`projects/01-hello-cli/src/lib.rs`](../../projects/01-hello-cli/)
  or the credential-file handling in
  [`projects/10-memberclub-cli/src/config.rs`](../../projects/10-memberclub-cli/).

- **What strong looks like.**
  - Reads the failing tests *first* to understand the contract.
  - Writes a tiny structured type for the config file rather than a
    raw `HashMap`.
  - Handles "file does not exist" as a non-error (use defaults).
  - Asks one good clarifying question, e.g. "should partial config
    files merge or replace?"
  - Their final code would pass `cargo clippy -- -D warnings` if we
    ran it.

- **What weak looks like.**
  - Jumps to coding without reading the tests.
  - Uses `unwrap()` on the file I/O.
  - Doesn't merge — overwrites everything with config-file values,
    losing CLI overrides.
  - Doesn't notice the existing patterns in the file.

- **Trap.** Candidates who are out of practice on the syntax of their
  preferred language will fumble even though their design instincts
  are right. Distinguish "rusty on Rust" from "shaky on design" —
  the first is a coachable lean-toward-hire; the second isn't.

#### PS-2 (SQL) — "Find the data leak"

- **Stage:** Phone screen.
- **Target level:** L4–L7. The same question scales.
- **Time:** 15 min.
- **The question.** "Here are two tables: `users(id, email)` and
  `orders(id, user_id, tenant_id, amount_cents)`. Here is a query
  that returns one tenant's orders. The query is wrong — under
  certain conditions, it returns another tenant's orders too. Find
  the bug and fix it. Then tell me how you'd prevent this *class*
  of bug at the database layer."

  The seed bug: `SELECT o.* FROM orders o JOIN users u ON o.user_id
  = u.id WHERE o.tenant_id = $1 OR o.amount_cents > 100000` — an
  `OR` precedence trap that breaks tenant isolation.

  Reference: the RLS pattern in
  [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/)
  and ADR
  [`0007`](../01-architecture-decisions/0007-postgres-rls-tenant-isolation.md).

- **What strong looks like.**
  - Spots the precedence bug in under 5 minutes.
  - The L5+ candidate then *names* Row-Level Security (or an
    equivalent enforcement-at-the-database-layer mechanism) without
    prompting.
  - The L6+ candidate explains *why* application-layer checks alone
    are insufficient: a single missing `tenant_id` in a `WHERE`
    clause anywhere in the codebase leaks data.
  - The L7+ candidate also notes the migration cost of retrofitting
    RLS onto an existing schema and how to gate the rollout
    safely (one table at a time, with a feature-flag).

- **What weak looks like.**
  - Doesn't see the bug in the query (it's three lines).
  - Sees the bug but suggests "I'd just be more careful" as the
    prevention — at L5+ this is a no-hire signal.
  - Suggests adding a stored procedure but cannot say what would
    prevent a developer from bypassing the stored procedure.

- **Trap.** Candidates who learned SQL on MySQL may not have
  encountered RLS. They should still reach for "enforce at the DB
  layer, not the app layer." The mechanism is less important than
  the principle. Don't penalize for not naming RLS specifically; do
  penalize for "be more careful in code review."

#### PS-3 (trace-reading) — "Diagnose the slow request"

- **Stage:** Phone screen.
- **Target level:** L5+. (At L4, replace with a pair-debug exercise.)
- **Time:** 20 min.
- **The question.** "Here is a tracing output (span tree, JSON, or
  a flamegraph image) from a single HTTP request that took 3.2
  seconds. The expected latency is under 200ms. Talk me through how
  you'd diagnose it, and tell me what's wrong."

  We pre-build three traces — pick one matching the role:

  1. **Database trace.** One slow query that's missing an index.
     The span tree shows it taking 2.9s of the 3.2s. Easy to spot if
     they read the tree top-down.
  2. **N+1 query trace.** 200 sequential 10ms database queries.
     Total: 2s. The tell is the *count*, not the duration of any
     one query.
  3. **External-API trace.** A synchronous Stripe call inside the
     request handler — exactly the antipattern that motivated ADR
     [`0006-outbox-over-broker`](../01-architecture-decisions/0006-outbox-over-broker.md).

- **What strong looks like.**
  - Asks "what's the request flow?" before diagnosing.
  - Reads the span tree top-down, identifying the dominating span.
  - Names the root cause specifically ("missing index on
    `orders.tenant_id`," "N+1 on the loop at the API layer," "this
    Stripe call shouldn't be in the request path; it should be in
    the outbox").
  - Proposes a fix and a *test* that would have caught the bug.
  - L6+ candidates also propose the *monitoring* that would catch
    the next instance.

- **What weak looks like.**
  - Doesn't know what a span tree is. (Acceptable at L4; not at L5+.)
  - Identifies "the database is slow" but can't say which query.
  - Proposes "we should cache it" without identifying the underlying
    cause.

- **Trap.** Some candidates have only seen logs, never traces. If
  they ask "can I see the logs instead?" — say yes, and switch to a
  log-based version of the same scenario. The skill we're testing
  is structured debugging, not familiarity with a tracing tool.

### On-site coding

Two flavors. We pick based on the role and the level.

#### Flavor A — Algorithmic ("could you have built this primitive?")

- **Stage:** On-site coding.
- **Target level:** L4–L5.
- **Time:** 60 min.
- **The question.** "Implement a small `Money` type for a payments
  system. Hold cents as a 64-bit integer and a 3-letter ISO currency
  string. Implement `checked_add` and `checked_sub` returning a
  `Result`; reject currency mismatches; reject overflow. Write
  property tests proving that any sequence of `checked_add`
  operations either succeeds with the expected total or returns an
  error — never silently overflows."

  Reference: [`projects/06-stripe-money-lab`](../../projects/06-stripe-money-lab/)
  and ADR
  [`0003-money-i64-cents`](../01-architecture-decisions/0003-money-i64-cents.md).

- **What strong looks like.**
  - Reaches for `i64` cents *immediately* and articulates why (no
    floats near money).
  - Doesn't implement `Add` / `Sub` operator traits — recognizes
    that operator overloads make it too easy to silently ignore the
    error case.
  - Writes the property test correctly: the "no silent overflow"
    invariant.
  - L5: also suggests the $21B ceiling pattern.

- **What weak looks like.**
  - Uses `f64`. Refuses to update when prompted.
  - Implements `Add` returning `Money` rather than `Result<Money>`.
  - Confuses `wrapping_add` with `checked_add`.

- **Trap.** Don't penalize candidates who use `i128` instead of
  `i64`. It's a defensible choice; explore why they made it.

#### Flavor B — Codebase navigation ("could you have shipped against our codebase?")

- **Stage:** On-site coding.
- **Target level:** L5–L7.
- **Time:** 75 min.
- **The question.** "Here's a clone of our public outbox demo
  project (`projects/08-outbox-demo`). The `mark_failed` function
  has a bug: when a task hits `max_attempts`, it still gets retried
  one more time. Find it and fix it. Then add a new test that would
  have caught this bug."

  We pre-introduce a one-line bug: `attempts < max_attempts` vs
  `attempts <= max_attempts`, or similar. The candidate gets the
  full repo, a working `cargo test`, and our README.

- **What strong looks like.**
  - Runs the tests first to see what passes and what fails.
  - Reads `src/lib.rs` top-down, locates `mark_failed`, finds the
    off-by-one.
  - Writes a focused test: "given a task at `attempts =
    max_attempts - 1`, mark_failed should set status to `failed`,
    not bump for another retry."
  - L6+ candidates also note: "I'd add a property test over (attempts,
    max_attempts) pairs; the current tests don't cover the boundary
    densely enough."
  - L7+ candidates also note the broader pattern: "off-by-one on a
    retry count is the kind of bug that needs a CHECK constraint or
    a property test. I'd file it as a CONTRIBUTING.md note."

- **What weak looks like.**
  - Doesn't read the existing tests — re-derives the contract from
    scratch and gets it wrong.
  - Fixes the bug but doesn't write a test.
  - Writes a test that asserts the old (buggy) behavior because they
    didn't think about what *correct* is.

- **Trap.** A nervous candidate may spend 30 minutes orienting in
  the codebase. Reassure: "treat me as a senior engineer pairing
  with you; ask whatever you'd ask in real life." A real engineer
  would ask "what's the existing test coverage?" before changing
  code, and we should reward that.

#### Flavor C — Code review (L7+ only)

- **Stage:** On-site coding (replaces algorithmic for L7+).
- **Target level:** L7.
- **Time:** 60 min.
- **The question.** "Here are three PRs against our codebase. They
  are real-shaped: one is a routine feature, one introduces a subtle
  bug, one is a refactor that conflicts with our ADR
  [`0002-orm-stance`](../01-architecture-decisions/0002-orm-stance.md).
  Review them as if you owned the codebase. We'll watch you do it
  and discuss your findings."

- **What strong looks like.**
  - Reads the ADRs and PLAYBOOK *first* to ground their review in
    our team's stated preferences.
  - Catches the bug.
  - Catches the ADR-violating refactor and pushes back kindly but
    firmly.
  - Leaves comments at the level of "I'd ask for a change because
    X" — not nitpicks, not approval theater.
  - Surfaces 1–2 *systemic* observations (e.g. "all three of these
    PRs touch the same module; consider whether this module needs
    refactoring").

- **What weak looks like.**
  - Skims; misses the bug.
  - Approves the ADR-violating refactor because it "looks clean."
  - Leaves only nits.
  - Cannot articulate which feedback is blocking vs non-blocking.

- **Trap.** A candidate who is excellent at writing code but has
  never done much PR review at scale will under-perform here.
  Distinguish "doesn't know our codebase yet" from "doesn't know
  how to review code at all."

### System design

Four prompts at escalating scope. We choose based on level.

#### SD-1 — Service-level (L4–L5)

- **Time:** 60 min.
- **Prompt.** "Design a notes API. Users can sign up, log in, create
  notes (title + body), list their notes, and delete them. Defend
  the database schema, the auth, and the API shape."

  Reference: [`projects/03-notes-api`](../../projects/03-notes-api/)
  and [`projects/04-auth-demo`](../../projects/04-auth-demo/).

- **Strong.** Reaches for HTTPS+JWT or session cookies (knows
  there's a choice; can explain it). Designs the schema with a
  composite index on `(user_id, created_at)` for the list query.
  Asks about pagination — and chooses cursor over offset for a list
  that could grow large. Recognizes the auth layer is the
  enforcement boundary for ownership.

- **Weak.** Designs an unauthenticated API. Lists notes by
  `SELECT *` with no pagination. Doesn't ask about scale.

#### SD-2 — Subsystem (L5–L6)

- **Time:** 60 min.
- **Prompt.** "Design the side-effects layer for a billing system.
  When a customer pays, we need to: send a receipt email, notify
  Slack, update the analytics warehouse, and call a partner's
  webhook. Some of these can fail. The customer's invoice must be
  marked paid in our DB, and *all four* side-effects must
  eventually happen, exactly once. Defend the design."

  Reference: the outbox pattern in
  [`projects/08-outbox-demo`](../../projects/08-outbox-demo/) and
  ADR
  [`0006-outbox-over-broker`](../01-architecture-decisions/0006-outbox-over-broker.md).

- **Strong.** Reaches for the outbox pattern — writes the
  business mutation and the four side-effect intents in the same
  database transaction. Discusses worker design (claim, dispatch,
  mark done/failed), retries with backoff, max-attempts ceiling.
  Asks about idempotency on the consumer side (Slack, partner
  webhook). L6 candidates also discuss observability — how would
  you know if the queue is backing up?

- **Weak.** "We'll just use Kafka" — without addressing the dual-
  write problem (write to DB + publish to Kafka is not atomic).
  Synchronous calls inside the request handler. No retries. No
  failure ceiling.

#### SD-3 — Cross-service (L6–L7)

- **Time:** 60 min.
- **Prompt.** "We're going multi-tenant. Today every customer has
  their own Postgres database. We want a single shared database
  where data is logically isolated by `tenant_id`. Walk me through
  the design, the migration, and the failure modes."

  Reference:
  [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/)
  and ADR
  [`0007-postgres-rls-tenant-isolation`](../01-architecture-decisions/0007-postgres-rls-tenant-isolation.md).

- **Strong.** Defines the `tenant_id` discipline (every tenant-scoped
  table gets one, NOT NULL). Reaches for enforcement at the database
  layer (RLS, or row-filtering proxy, or stored procedures). Talks
  about the session variable / GUC pattern. Discusses the migration:
  one table at a time, with shadow reads and dual writes. Names the
  subtle failure mode (the migrator role bypasses RLS because it's
  a superuser — so it must be a *separate* role). Discusses the
  monitoring needed during migration ("compare row counts per tenant
  before and after").

- **Weak.** "We'll add `tenant_id` to every query in the app code"
  with no discussion of how to enforce. No migration plan. Calls
  the multi-tenant problem solved by "we'll just be careful."

#### SD-4 — Platform-scale (L7)

- **Time:** 60 min.
- **Prompt.** "We're an EU-data-residency-required customer's dream
  vendor: we want to launch a fully EU shard within 12 months.
  Walk me through the architecture, the operations, and the legal
  surface."

  Reference: Bet 2 of
  [`01-vision-doc-template.md`](./01-vision-doc-template.md) in this
  directory.

- **Strong.** Distinguishes *control plane* (global, holds metadata
  about which tenant lives in which region) from *data plane*
  (regional, holds customer data). Discusses how routing decides
  which region to serve a request. Names the operational pain: dual
  on-call rotations, CI parity across regions, what "deploy to
  prod" means in a multi-region world. Discusses the legal surface:
  DPA, sub-processor list, GDPR data-subject requests. Talks about
  what we'd *not* do (e.g. don't cross-shard joins; don't try to
  share encryption keys). At L7 we *expect* the non-goals.

- **Weak.** "We'll just spin up a copy of prod in EU." No
  discussion of the control plane, the routing, or the legal
  surface. Underestimates the operational cost.

### Behavioral

STAR format (Situation, Task, Action, Result) is the structure we
expect candidates to follow. The interviewer should explicitly cue
the format if the candidate isn't using it.

The behavioral interview is *the* opportunity to assess (c)
cross-functional and (d) mentorship — neither shows up well in
coding or design.

#### B-1 — Disagreement (all levels)

- **Prompt.** "Tell me about a time you disagreed with a peer or a
  manager about a technical decision. Walk me through what
  happened and what you did."

- **Strong.** Names the disagreement specifically. Names the
  other person's argument fairly. Describes how they sought
  evidence rather than escalating. Describes the resolution — and
  importantly, describes a case where *they were wrong* somewhere
  in their career, or at least describes that they updated their
  view when shown evidence. L6+ candidates describe a case where
  they "disagreed and committed" — they didn't get the decision
  they wanted, but they backed the team's chosen path fully.

- **Weak.** Frames the disagreement as the other person being
  wrong throughout. Cannot describe a time they updated their view.
  Describes a "win" where they got their way by working around the
  other person.

- **Trap.** Candidates with thin track records will repeat the same
  story across multiple behavioral questions. Push for a *different*
  story if the first reuses material from B-3.

#### B-2 — A bug you caused (L4–L7)

- **Prompt.** "Tell me about the worst bug you've shipped to
  production. What happened, what did you do, what did you learn?"

- **Strong.** Names the bug specifically and accurately. Owns it
  without deflecting. Walks through the response: detection,
  diagnosis, mitigation, root-cause analysis. Names *what they
  changed about how they work* afterward. L6+ candidates name a
  systemic change (e.g. "I introduced a postmortem template after
  this; we now do them for every Sev2+").

- **Weak.** "I haven't really shipped a bad bug" — implausible at
  L5+; suggests either dishonesty or not enough scope. Deflects to
  "the other team's fault." Cannot describe what they learned.

- **Trap.** Honest candidates are sometimes hesitant to share a bad
  bug story; reassure them that the answer is for evaluating their
  learning, not their failure rate.

#### B-3 — Mentorship (L5–L7)

- **Prompt.** "Tell me about someone you've mentored or grown.
  What did they need, what did you do, where are they now?"

- **Strong.** Names the person (or describes them clearly) and
  what they needed. Describes a *plan*, not just ad-hoc help. Can
  point to the outcome ("they got promoted to L5," or "they
  recovered from a rough quarter," or "they moved into a tech-lead
  role"). Talks about what *they* (the candidate) learned from the
  mentoring relationship — the best mentors don't see it as one-way.

- **Weak.** "I'm always happy to help my teammates" — vague,
  unattributed. Or: lists their mentorship as a synonym for "I
  answer questions in Slack."

#### B-4 — Saying no (L6–L7)

- **Prompt.** "Tell me about a time you said no to a request from
  your manager, a PM, or a senior leader. How did you do it?"

- **Strong.** Names the request, the reason for saying no, and the
  conversation. Describes how they made the no actionable — i.e.,
  offered an alternative path or a clear bar that would let them
  say yes. L7 candidates have multiple instances and can talk about
  the *credibility cost* of saying no and how they spend it
  intentionally.

- **Weak.** Cannot recall a time. Said no badly (no alternative;
  scorched-earth). Said no to a peer but never to leadership.

#### B-5 — Cross-functional collaboration (all levels)

- **Prompt.** "Tell me about a project where engineering, product,
  and another function had different priorities. How did you work
  through it?"

- **Strong.** Names the functions, the tensions, and the resolution.
  Talks about *how they understood the other function's framing*,
  not just "I explained ours." L6+ candidates describe a case where
  they changed engineering's plan because they understood what the
  other function actually needed.

- **Weak.** Treats the other function as an antagonist. Cannot
  describe what the other side wanted in the other side's language.

#### B-6 — Failure mode of the L7 candidate

- **Prompt (L7 only).** "Tell me about a strategic call you made
  that turned out to be wrong. Not a tactical bug — a strategic
  miscall."

- **Strong.** Has one. Can name it. Can describe how it manifested,
  when it became clear it was wrong, and what they did. Bonus:
  describes the *cost* of correcting it — strategic mistakes are
  expensive to unwind and the candidate should know it.

- **Weak.** Cannot name one. At L7 this is a red flag — every L7
  has made a strategic miscall, and the ability to name it is a
  marker of self-awareness.

## Grading rubrics

After each interview, the interviewer writes up their feedback in
the form:

```
RECOMMENDATION: strong hire | hire | no hire | strong no hire
LEVEL: L4 | L5 | L6 | L7

WHAT THEY DID WELL:
- [bullet]
- [bullet]

WHAT THEY DID POORLY:
- [bullet]
- [bullet]

EVIDENCE FOR LEVEL (a) (b) (c) (d):
- [bullet per axis]

QUESTIONS I HAVE FOR THE NEXT INTERVIEWER:
- [bullet]
```

The four levels of recommendation map to the calibration meeting
as follows:

- **Strong hire** — the interviewer would be excited to work with
  this person; the bar should generally be met.
- **Hire** — the interviewer would work with this person without
  reservation but did not see exceptional signal.
- **No hire** — there are specific concerns the interviewer can
  name; the candidate should not be hired into this loop. (May be
  worth re-loading at a different level.)
- **Strong no hire** — there is a specific behavioral, ethical, or
  competence concern severe enough that the interviewer would not
  work with this person; the candidate is not coming back.

Three rubric rules:

1. **Recommendation must be specific.** "Hire — they seemed
   thoughtful" is not enough. Every recommendation has at least
   one piece of evidence per axis.

2. **Level must be specific.** Do not write "L5 or L6." Pick one;
   the calibration meeting is where the band gets debated. If you
   genuinely can't pick, write "L5 with a note: extend at L6 if
   the system design lands."

3. **No comparing candidates.** Do not write "stronger than
   Candidate X." Candidates are evaluated against the rubric, not
   against each other. (Yes, the calibration meeting *does*
   compare candidates — that's the meeting's job, not the
   interviewer's.)

## Calibration meeting playbook

The calibration meeting happens within 48 hours of the last
interview. Everyone who interviewed the candidate is there. The
hiring manager runs it.

### Agenda (45 min)

1. **Hiring manager states the loaded level and the role.** (1 min)
2. **Each interviewer reads their summary aloud, in order of
   loop sequence.** (3 min × interviewer count)
3. **Open discussion.** (15–20 min)
4. **Decision.** (5 min) Hire (and at what level), or no hire.

### How to disagree productively

This is the hardest part of the hiring process to get right. Three
disagreement modes show up frequently:

**Mode 1: "I saw X, you saw not-X."**

Two interviewers came away with different reads of the same axis.
Resolve by going to the *evidence* — quote what the candidate
actually said or did. The interviewer with the stronger evidence
wins the read. If both have equally strong evidence pointing
opposite ways, that *is* the signal — the candidate's behavior is
inconsistent, and inconsistency at this level is a no-hire by
itself.

**Mode 2: "We saw the same thing, but we weight it differently."**

One interviewer thinks a flaw is fatal; another thinks it's
coachable. Resolve by going to the *job*. Will the candidate
encounter this situation in their first 6 months? If yes, the
weighting interviewer who treats it as fatal is right. If no, the
weighting interviewer who treats it as coachable is right. The
hiring manager's job is to know the answer to "what will they
encounter in their first 6 months."

**Mode 3: "I want to hire them, you don't."**

The dangerous mode. Default to the "no" — the cost of a bad hire
is much higher than the cost of a missed good hire. *But* — if the
"yes" can name specific evidence and the "no" can only name a
vibe, push the "no" interviewer to articulate. A "vibe-based no"
is where bias hides; it must be made concrete before it becomes a
decision.

### Veto rules

- One **strong no hire** with specific evidence is a veto. No
  negotiating.
- Two **no hire** results with overlapping evidence is a veto.
- A single **no hire** can be overridden if every other interviewer
  saw the opposite signal at the same axis — but the override must
  be documented, and the override interviewer's recommendation goes
  to the file so we can recalibrate later.

### The leveling debate

If the loop is split between L5 and L6 (or L6 and L7):

- Default to the lower band. It is much easier to promote someone
  who is exceeding their level than to manage out someone who was
  miscalibrated up. Internal promotion is also better-aligned with
  the team's incentives than external mis-leveling.

- Exception: if the candidate explicitly asked to be considered at
  the higher band and the loop saw strong signal at it, default to
  the higher band. The candidate's self-assessment is signal.

- Always make the offer explicit about the level. "We'd like to
  offer at L5; based on your loop, here are the L5 expectations we
  saw you meet, and here are the L6 areas we'd want to see grow."

## The diversity-of-thought rule

Every loop has at least one panelist whose primary discipline is one
of:

- **Debugging / production engineering.** Someone who has worked
  on-call in production at the relevant scale. They run the
  trace-reading question and one behavioral.
- **SRE / reliability.** Someone whose default question on every
  design is "what's the blast radius?" They run a system design
  and bring this lens to calibration.
- **Security mindset.** Someone whose default question is "what
  happens when the input is hostile?" They run an authorization or
  trust-boundary question.

The point is not quotas. The point is that *signal quality* improves
when at least one panelist is wired to look at the candidate from a
not-default angle. Most loops over-index on coding and design and
under-index on operational and adversarial thinking. This rule is a
forcing function against that drift.

If you cannot staff a panel with one of these three perspectives,
delay the loop. Two-thirds of a panel is not better than waiting a
week.

## Anti-patterns and traps

### "I'd just be more careful" answers

A candidate who, when shown a class-of-bug question (the PS-2 SQL
injection / RLS bypass, the SD-2 dual-write problem, the SD-3
multi-tenant migration), answers "I'd be more careful in code
review" is a no-hire at L5+. The L5+ bar is *understanding why
discipline doesn't scale and reaching for enforcement at the right
layer*.

### Solo bar-raisers

The bar-raiser is a calibration tool, not a veto authority. A loop
where the bar-raiser is the only "no hire" needs scrutiny — either
the rest of the panel is missing a signal the bar-raiser saw (in
which case the bar-raiser should be able to *name* it), or the
bar-raiser is over-indexing on a personal preference. Resolve by
going to evidence.

### Pattern matching from the resume

Resumes carry information; they also carry noise. A candidate from
a famous company is not automatically L7-eligible. A candidate from
no company you've heard of is not automatically L4. The leveling
band is established by the loop, not the resume.

### Take-home assignments

We do not give take-home assignments. They burn the candidate's
unpaid time, they advantage candidates without families or second
jobs, and they correlate poorly with the work they would actually do
once hired. The on-site coding interview is the right venue for
that signal.

### Asking trivia

"What's the difference between `Rc` and `Arc`?" is a trivia
question masquerading as a knowledge question. Replace with: "Walk
me through a time you had to share state between two threads. Why
did you pick the primitive you picked?" — same knowledge surface,
much better signal.

### Time-pressured coding for senior roles

L6+ candidates rarely write greenfield code under time pressure in
their day jobs. Test what they actually do: read code, find bugs,
review PRs, design systems. The Flavor B and Flavor C coding
interviews are designed for this.

### The bias against introverts in behavioral

Some excellent candidates are slow to start, especially on
behavioral questions. Reassure: "take a minute to think about the
example you want to walk through; I'm not going to penalize you for
the pause." The signal is in the substance of the answer, not the
speed.

### Hiring against a strict job description

A job description is a starting point, not a checklist. We hire
engineers who could *do* the job, not engineers whose resume *matches*
the bullet points. If the candidate has done something equivalent
to what we want, that's enough. If the candidate has done *exactly*
what we want, that's a bonus.

### The "culture fit" trap

"Culture fit" without a defined culture becomes a hand-wave for
"reminds me of me." If a candidate is going to be a problematic
hire, name *specifically* what behavior would be a problem. If you
cannot, the concern is not actionable.

The reverse trap: dismissing real concerns as "culture fit theater."
If a candidate showed up to the interview being rude to the
recruiter, or condescending to a junior interviewer, or unable to
discuss their work without naming bad-faith antagonists, those are
real signals. Name them.
