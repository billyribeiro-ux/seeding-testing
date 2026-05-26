# Phase 12 — Principal Engineer Skills

> **Audience:** you finished Phase 11. You can write the code.
> **Outcome:** you can do the *other 70%* of being a principal engineer:
> architectural decisions, design docs, code review, incident response,
> mentoring, system design.
> **Time:** 2 weeks (and the rest of your career).

## The mental model

> *Code is the smallest part of being a Principal Engineer. The rest is
> judgment, communication, and force-multiplication.*

Engineers at every level write code. Principals shape *which* code gets
written, by whom, and why. Five practices cover most of the role:

1. **Architecture Decision Records (ADRs).** Captures decisions so the next
   engineer doesn't relitigate them.
2. **RFCs / Design Docs.** Surfaces a non-trivial change for debate before
   the code is written.
3. **Code Review at Scale.** Differentiates style from substance; teaches
   without crushing.
4. **Incident Response.** Runbooks; blameless postmortems; trend analysis.
5. **Mentoring.** Pairing, code katas, growth ladders.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-mental-model.md` | What the role actually is |
| `lessons/02-adrs.md` | Architecture decisions in MADR format |
| `lessons/03-rfcs-and-design-docs.md` | When to write one; the template; review etiquette |
| `lessons/04-code-review-at-l7.md` | Differentiating style from substance; teaching, not policing |
| `lessons/05-incident-response.md` | Runbooks, postmortems, the 5-Whys, repeat-incident analysis |
| `lessons/06-mentoring-and-pairing.md` | Pairing, katas, growth ladders, sponsorship |
| `lessons/07-system-design.md` | The whiteboard skill: capacity, consistency, back-pressure |
| `lessons/08-capstone-deliverables.md` | What you must ship to finish this phase |

## The capstone — three artifacts

To complete Phase 12, ship:

1. **An ADR** for one *real* decision (drawn from the curriculum or your
   work). Use the MADR template.
2. **A design doc** for a hypothetical feature (e.g. usage-based billing
   for MemberClub).
3. **A postmortem** for a *simulated* incident — pick a scenario from
   `docs/runbooks/` and document the 5-Whys.

Each lives under `docs/` and is reviewed by a human (a colleague, a
mentor, or a senior on this list of engineers we trust).

## Why this matters

- **The code is replaceable.** The decisions about *which* code, in *which*
  shape, made by *which* people, scale your career.
- **Writing well is the senior multiplier.** A clear ADR saves your team
  ten meetings. A clear design doc prevents three failed implementations.
- **Mentoring scales you beyond your hands.** Five engineers you coached
  ship more than your hands ever could.

## Green-bar checkpoint

- The three capstone artifacts exist in `docs/`.
- You can quote the MADR template from memory.
- You can lead a postmortem without blaming a person.

You are now ready to be considered for Principal Engineer L7+. The rest is
practice, judgment, and the willingness to be the most useful person in
the room.
