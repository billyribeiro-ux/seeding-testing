# Phase 12 — Exercises

The three artifacts in Lesson 12.8 *are* the exercises. Plus:

---

## E12.4 — Run a code review on a real PR (Stretch)

Find any open-source Rust project. Pick a recent PR. Read it the way the
lesson taught:

1. Match against an ADR? (Open issues/decisions/RFCs in the repo.)
2. Right abstractions for the next change?
3. Concurrency / scale / failure?
4. Tests are right?
5. Public API idiomatic?
6. Documentation?
7. (Last) Style?

Write your review as you would post it. Save it to
`docs/04-review-practice/NNNN.md`. Don't actually submit it.

---

## E12.5 — Run a postmortem drill (Stretch)

Recruit two friends. Walk through this scenario in 30 minutes:

> "On May 26, at 14:32 UTC, the auth-demo service started returning 500
> on every login. The on-call engineer rolled back the latest deploy at
> 14:46. Investigation showed the rollback didn't fully resolve the issue
> until a session-table migration ran at 15:01. Three more users logged
> errors during the rolling restart."

Roles: DRI, scribe, observer. After the simulation, write the postmortem.

---

## E12.6 — Mock system-design (Stretch)

Pick one of the RFC topics from Lesson 12.8. Pair with a senior engineer
(or a mentor) for a 45-minute whiteboard session. They ask questions; you
sketch.

Goal: practice the *conversational arc* (Lesson 12.7). End with a one-page
summary you'd convert into the RFC.

---

## E12.7 — Find a mentee (Stretch — for the next 6 months)

Identify one engineer 1–2 levels below you whose growth interests you.
Offer a recurring 30-minute weekly chat. Follow the structure in Lesson
12.6.

This isn't graded; it's the practice that makes the role real.
