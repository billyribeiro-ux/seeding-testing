# docs/leadership — Artifacts staff/principal engineers actually produce

> "By the time you reach L7, the code is the smallest part of the job."
> — paraphrasing every promo committee, ever.

This directory holds the **leadership artifacts** an L7+ engineer is
expected to *produce* — not consume, not skim, not co-sign. The work
that lives here is the work a promo committee will read in your packet.

## The framing

Most of this repository is craft: Rust, SQL, tests, observability. That
craft is the price of admission. At L6 you ship features that move
business metrics. At L7 you shape the technical direction that decides
which features are worth shipping in the first place. At L8 you do that
across multiple teams that don't all report to you.

The shift looks like this:

| Level | Where the time goes | Primary output |
|------:|--------------------|----------------|
| L4    | Tickets             | PRs |
| L5    | Features            | PRs + design notes |
| L6    | Projects            | RFCs, ADRs, postmortems |
| L7    | **Direction**       | **Vision docs, leveling rubrics, OSS work, talks** |
| L8    | **Organizations**   | Strategy memos, org design, narrative |

The five artifacts in this directory are the L7 column. They are
*templates* — you can clone them for your own situation — and *worked
examples* against this codebase. Both halves matter. A template
without an example feels abstract. An example without a template feels
like a one-off. Together they let a reader trace from "what should I
write" to "what does done look like."

## The artifacts

1. **[01-vision-doc-template.md](./01-vision-doc-template.md)** — A
   3-year technical vision document. The template is generic; the
   worked example commits to specific bets for the MemberClub product
   built in this repo (usage-based billing on the existing outbox,
   EU shard on the multi-tenant-rls primitives, an Auth-as-a-Service
   spin-out, an in-house experimentation platform, and a deprecation
   of the dual-mode auth code path).

2. **[02-hiring-and-interview-guide.md](./02-hiring-and-interview-guide.md)**
   — Leveling rubrics for L4–L7, a question bank organized by loop
   stage (phone screen, on-site coding, system design, behavioral),
   and the calibration meeting playbook. The questions are grounded
   in this repo's stack: a candidate could legitimately be asked to
   debug the outbox worker in `projects/08-outbox-demo/` or design the
   next iteration of `projects/12-multi-tenant-rls/` on the whiteboard.

3. **[03-oss-maintainership-playbook.md](./03-oss-maintainership-playbook.md)**
   — What it takes to maintain a popular crate. CHANGELOG discipline,
   triage cadence, release automation, the "first PR experience,"
   funding and sustainability (including the lesson from xz-utils),
   the maintainer's bill of rights, and when to deprecate.

4. **[04-conference-talk-template.md](./04-conference-talk-template.md)**
   — Three talk shapes (case study, deep-dive, opinion piece), slide
   design rules, the "no live demo unless you can survive failure"
   rule, the 20-minute structure, Q&A handling, and a worked outline
   for *Outbox without RabbitMQ: shipping reliable side-effects on
   Postgres alone* using `projects/08-outbox-demo` as the case study.

## How to use this directory

Two reading paths:

**Path A — "I have a real situation."**
Find the file whose template matches what you need to write. Read the
template section (the headings + the prose under each). Skim the
worked example for tone. Fork a copy. Fill it in.

**Path B — "I want to understand what L7 looks like."**
Read all five worked examples in order. They are deliberately
opinionated, deliberately specific, and deliberately committed to
concrete bets the existing repo could support. They are the *kind of
thing* a promo packet sample writeup would contain.

## What you will NOT find here

- A career ladder. (Every company has its own; the leveling rubric
  inside `02-hiring-and-interview-guide.md` is for *hiring against this
  stack*, not for tracking your own promo.)
- Sales pitches for staff engineering. The Will Larson and Tanya
  Reilly books cover that ground; we link to them where relevant
  rather than restating.
- A reading list. The four artifacts above are the work.

## Conventions used in this directory

- Templates and worked examples are interleaved within the same file,
  with the worked example separated by a `---` rule. The TOC at the
  top of each file points to both halves.
- Every claim about this codebase that a reader can verify is linked
  to the concrete file path (e.g. `../../projects/08-outbox-demo/src/lib.rs`).
- Internal links are relative so the directory can be vendored into
  another repo without breaking.
- No emojis. No vendor logos. No motivational quotes after this page.

## Cross-references inside this repo

The artifacts here lean on, and occasionally extend, work in:

- [`../README.md`](../README.md) — the overall `docs/` index
- [`../01-architecture-decisions/`](../01-architecture-decisions/) — the
  ADRs that the worked vision-doc example assumes are already merged
- [`../02-rfcs/`](../02-rfcs/) — RFCs are an L5/L6 deliverable; vision
  docs sit one level up and reference RFCs the way an RFC references
  ADRs
- [`../03-postmortems/`](../03-postmortems/) — postmortems feed back
  into the vision doc's "Current state assessment" section
- [`../../PLAYBOOK.md`](../../PLAYBOOK.md) — mental models the L7 work
  presupposes

If a future reader is wondering where to put their own artifact:

- Single decision, narrow scope, locks the team in → ADR
- Multi-week project proposal seeking team buy-in → RFC
- "What broke and what we'll change" → postmortem
- 3-year direction or org-wide change → vision doc (this directory)
- Hiring or interview process change → update the guide here
- Crate or repo maintenance philosophy → update the OSS playbook here
- A talk you're giving externally → start from the conference template

## Maintenance

These artifacts decay if they are not maintained. Suggested cadence:

- Vision doc — re-read every quarter, rewrite every 12 months
- Hiring guide — review after every loop where a calibration went
  sideways, plus a quarterly full pass
- OSS playbook — update when a maintainer onboards or off-boards
- Talk template — update after every talk you give that bombs

If you change a worked example, leave the template alone unless the
template is *also* wrong. The two are coupled but should evolve at
different rates.
