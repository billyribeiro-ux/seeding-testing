# Lesson 12.8 — Capstone Deliverables

> **Concept first:** you don't "finish" Phase 12 by reading. You finish
> it by *shipping* three artifacts that demonstrate the L7 mindset.
> **Time:** 2–5 days of focused work.

## Artifact 1 — An ADR

Pick **one** decision (from MemberClub or from your day job) and write
the ADR. Use the MADR template from Lesson 12.2.

Suggestions if you're stuck:

- **`ADR 0010: Single Money type for all monetary values.`** Captures
  the Phase 8 stance.
- **`ADR 0011: Outbox over external broker for background jobs.`**
  Captures Phase 11.5.
- **`ADR 0012: RLS as belt-and-braces for tenancy.`** Captures
  Phase 11.6.

Path: `docs/01-architecture-decisions/NNNN-slug.md`.

Quality bar:

- 7 sections per MADR.
- "Considered Options" has ≥ 2 real alternatives.
- "Consequences" lists at least one *negative* you accept.

## Artifact 2 — A design doc

Pick **one** non-trivial feature and write the RFC. Use the template from
Lesson 12.3.

Suggestions:

- **Usage-based billing for MemberClub.** Most realistic.
- **A community comments feature with moderation queues.**
- **A SAML SSO integration for Enterprise customers.**
- **A weekly engagement-report email.**

Path: `docs/02-rfcs/NNNN-slug.md`.

Quality bar:

- 11 sections per template.
- An honest decision matrix with at least 3 options.
- Open questions enumerated, not pretended-away.
- Implementation plan in weeks, not days.

## Artifact 3 — A postmortem

Pick **one** scenario from `docs/runbooks/` (or invent one realistic
incident) and write the full postmortem.

Suggestions:

- **"Stripe webhook lag caused tier projection to be 30 min stale."**
- **"A migration accidentally locked the `users` table for 8 minutes
  during a deploy."**
- **"A new endpoint without an index caused DB pool exhaustion."**
- **"A cookie path change logged out every user across our subdomain."**

Path: `docs/03-postmortems/YYYY-MM-DD-slug.md`.

Quality bar:

- Five-Whys reaches a *systemic* cause, not a person.
- Follow-ups have owners and dates.
- "What didn't" is at least as long as "What worked."

## Optional: peer review

If you can, have a real engineer (a colleague, a mentor) review one of
the three. A line-by-line review is the best teacher.

If you can't, do a *self-review* with a one-day cool-down: write the
doc, sleep on it, re-read it tomorrow with fresh eyes. You'll catch
things.

## What "complete" looks like

```
docs/
├── 00-mental-models/                   (skim — these were in PLAYBOOK.md)
├── 01-architecture-decisions/
│   └── NNNN-your-adr.md                 ← required
├── 02-rfcs/
│   └── NNNN-your-rfc.md                 ← required
└── 03-postmortems/
    └── YYYY-MM-DD-your-postmortem.md    ← required
```

Three files. Committed. Pushed. PR opened (even if you immediately
merge it).

That PR is the artifact. You can hand it to a hiring committee or to your
manager and say: "this is the work I do."

## Why this matters

- **You don't show L7 by *talking* — you show it by *writing*.**
- **The three artifacts demonstrate three muscles**: architectural
  reasoning, structured proposals, blameless post-incident analysis.
- **They live in the repo.** Future engineers read them; future you
  reads them.

## Green-bar checkpoint

- All three files exist in `docs/`.
- A PR is open (or merged).
- You can defend each one in a 10-minute conversation.

You have completed the curriculum. You are now ready to be considered for
Principal Engineer L7+. The remainder of the work is practice, judgment,
and continuing to apply the patterns of this curriculum.

Welcome to the role.
