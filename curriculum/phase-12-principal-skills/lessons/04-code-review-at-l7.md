# Lesson 12.4 — Code Review at L7

> **Concept first:** a senior reviewer's job is to *teach without
> crushing* and to *protect the system without policing the engineer*.
> Differentiate style from substance.
> **Time:** 20 minutes.

## What to look for, in order

```
1. Does this match an ADR? Should it be discussed in one?
2. Does it create the right abstractions for the next change?
3. Does it leak abstractions across boundaries?
4. Will it survive concurrency / scale / failure?
5. Are the tests right? Right means "fail when the bug returns."
6. Does the public API feel idiomatic to the rest of the codebase?
7. Documentation / changelog / migration plan?
8. (Last) Is it stylistically consistent?
```

Style is at the bottom. If you start there, you've already lost the room.

## The three flavors of comments

| Flavor | When | How |
|---|---|---|
| **Blocker** | "This breaks the system" | "This races with X. Add a transaction." |
| **Suggestion** | "Consider this" | "nit: rename `foo` to `bar` for consistency with our pattern" |
| **Question** | "I don't understand" | "Why did you choose this over the existing util?" |

Tag explicitly. `BLOCKING:`, `nit:`, `q:` — the author knows what to
act on without guessing.

## Praise the work, critique the code

```
"Nice work on the migration safety — the transaction shape is exactly right.
A concern on the index choice: ..."
```

vs

```
"This is wrong."
```

The second one feels demoralizing even when it's true. The first opens
the conversation.

Author bias matters: a comment from a senior on a junior's PR weighs 3×
what the same comment would from another junior. Adjust for that —
soften, explain, link.

## Don't review what tools should review

If `clippy` catches it, don't comment on it. If `prettier`/`rustfmt`
catches it, don't comment. If `cargo deny` catches it, don't comment.

Spend reviewer attention on what tools can't see.

## The "should have been an ADR" pattern

```
"This significantly changes how we handle background jobs. Could you
extract the design rationale into an ADR (or pointer to an existing
one)? I want to make sure we agree on the direction before merging the
implementation."
```

This is a kind, productive "blocker." It signals: "your code is fine; we
need to think bigger first."

## Reviewing the diff vs reviewing the state

A 200-line PR diff might look reasonable; the *resulting* file might be
a mess. Always check out the branch and read the file — not just the
diff.

In GitHub: "View diff with whitespace" + "Hide whitespace" toggles
between the two views. Use both.

## Approving vs requesting changes

- **Approve with comments** — small suggestions; author can land it
  with or without addressing.
- **Comment only** — questions; not blocking; expect a reply.
- **Request changes** — blocking; explain why; offer a path forward.

Don't approve with major suggestions. The author won't act on them.

## Latency matters

A PR that sits 3 days for review is a *cultural* signal — "your work
isn't urgent." Senior engineers respond to PRs within hours, even if
just to say "I'll look tomorrow."

Set a personal SLA. 24 hours is reasonable.

## When to pair instead of review

If the PR has:

- > 500 lines of changes,
- multiple competing patterns,
- significant new abstractions,
- the author seems frustrated in earlier comments,

...stop reviewing in writing. Schedule 30 minutes; pair through it
together. Faster, less abrasive, more learning.

## Why this matters

- **Reviewers shape culture more than any team meeting.** The signals you
  send in comments become the team's defaults.
- **Teaching is the multiplier.** A reviewer who explains "why" raises
  the floor for the whole team.
- **Quick reviews unblock the team.** Slow ones become resentment.

## Green-bar checkpoint

- You can name the eight review-order items.
- You can write a comment that *blocks* without crushing.
- You can pick "review" vs "pair" for a given PR.

Next: `lessons/05-incident-response.md`.
