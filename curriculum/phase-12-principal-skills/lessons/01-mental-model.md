# Lesson 12.1 — What "Principal Engineer" Actually Means

> **Concept first:** Principal Engineers aren't promoted senior engineers.
> The job is different.
> **Time:** 15 minutes.

## What you stop doing

- Closing tickets at the same rate as a senior engineer. You'll close
  fewer; that's correct.
- Writing every line of new code. Others do; you review, advise, unblock.
- Spending most of the day in your IDE. Most of the day is in conversation
  — synchronous and asynchronous.

## What you start doing

- **Setting direction.** "We should bet on X." Make the bet visible; own
  the outcome.
- **Writing.** ADRs, RFCs, postmortems, growth-conversation notes,
  newcomer onboarding docs.
- **Code review with intent.** Not "is this code correct"; "is this code
  the *right* code, will the next engineer maintain it, does it match
  the system we want."
- **Unblocking.** Before someone realizes they're blocked.
- **Killing projects.** This is harder than starting them.
- **Sponsoring** (not just mentoring). Putting your reputation behind
  someone for the room they're not in.
- **Asking the boring question.** "What does success look like?" "How
  will we measure it?" "What happens if we don't do this?"

## The four levers

A Principal moves four levers, in roughly this order:

1. **Standards.** What "good" looks like; the ADRs that capture it.
2. **Architecture.** The shape of the system; service boundaries; data
   ownership.
3. **People.** Pairing, mentoring, sponsoring, hiring.
4. **Direction.** Which bets the team makes.

The bigger the lever, the slower the feedback. Adjusting "what good
looks like" pays for itself over years. Adjusting direction pays for
itself over months.

## The most important phrase

> *"What problem are we actually solving?"*

You'll say this 20 times a week. The job is largely to repeat it until
the room can answer it without flinching. If they can't, you go back to
talking; you don't go to code.

## Three traits that signal "principal-ready"

| Trait | What it looks like |
|---|---|
| **Writes clearly under stress.** | Their incident postmortem reads like a calm essay, not a frantic timeline. |
| **Disagrees and commits.** | After arguing the alternative, they go all-in on the chosen path. No sandbagging. |
| **Names the elephant.** | If the project is in trouble, they say so — early, in writing, with proposed remedies. |

If you don't yet do all three, that's normal. Practice.

## The most common failure modes

- **The "lead architect" who writes code in private** and emerges with a
  beautiful design no one will maintain. *Useless.*
- **The "purist" who blocks every PR over style.** *Annoying.*
- **The "hero" who fixes every incident personally.** *Doesn't scale.*
- **The "philosopher" who writes ADRs about hypothetical problems.**
  *Wastes everyone's time.*

The job is to be *useful*, not impressive. Useful at scale, but *useful*
above all.

## Why this matters

- **Title escalation without role change is unsatisfying** — for you and
  your team. Understand the role differently from the start.
- **The leverage is real.** A great Principal multiplies their team's
  throughput; a poor one drains it.
- **The role is *taught*, not inferred.** Write ADRs, do design reviews,
  lead postmortems — the muscle grows.

## Green-bar checkpoint

- You can articulate three things you'll *stop* doing and three you'll
  *start*.
- You can name the four levers in order.
- You can say the most important phrase out loud without irony.

Next: `lessons/02-adrs.md`.
