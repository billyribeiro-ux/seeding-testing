# Conference Talk Template

A staff/principal engineer is regularly asked to give external talks:
at conferences, meetups, customer events, internal-all-hands events
that include other companies. This document is the template + the
worked example for those talks.

The implicit thesis is that a *good* technical talk has the same
structure as a good RFC: a clear thesis, ordered claims, evidence
for each claim, an opinion stated bluntly, and a strong close. The
difference is that a talk has a clock and a hostile audience-attention
budget.

## Table of contents

- [The three talk shapes](#the-three-talk-shapes)
- [The structure](#the-structure)
- [Slide design rules](#slide-design-rules)
- [The live demo question](#the-live-demo-question)
- [Q&A handling](#qa-handling)
- [Speaker mechanics](#speaker-mechanics)
- [Worked outline: "Outbox without RabbitMQ"](#worked-outline-outbox-without-rabbitmq-shipping-reliable-side-effects-on-postgres-alone)
- [Preparation timeline](#preparation-timeline)
- [Post-talk hygiene](#post-talk-hygiene)

## The three talk shapes

The first decision is which *kind* of talk you're giving. Most
talks fail because the speaker picked the wrong shape for their
material.

### Shape 1: The case study

**"Here's what we did, what worked, what didn't, and what we'd do
differently."**

When it lands: when the audience is your peer group (other senior
engineers at companies your size) and your story has a hard pivot
or a non-obvious lesson. The audience leaves with a model they can
apply to a similar problem in their own work.

When it doesn't: when you don't yet have a "result." A case study
without an outcome is a status update, and status updates do not
deserve conference slots.

Tell from a case-study talk:

- The speaker can describe the *initial* design and the *final*
  design and explain why they're different.
- The speaker can name at least one thing that didn't work.
- The speaker doesn't hide the failure mode.

### Shape 2: The deep-dive

**"Here is one system, in detail, to a depth most blog posts
won't go."**

When it lands: when the audience is a self-selecting group of
people who already understand the basics and want the
implementation-level reality. SREcon, Strange Loop, Rust Nation —
the rooms where 30 minutes of well-explained source code is
genuinely the best use of the audience's time.

When it doesn't: at a general-audience venue. A deep-dive at a
generalist conference will half-empty the room by minute 7.

Tell from a deep-dive talk:

- The speaker can show code that *runs*, not pseudo-code.
- The speaker has internalized the problem space well enough to
  preempt the 3–5 questions an attendee in the room will be
  silently asking.
- The speaker isn't trying to convince anyone of anything; they're
  *teaching*.

### Shape 3: The opinion piece

**"Here's what I think the industry has wrong, and here's the case
for the alternative."**

When it lands: when you've spent enough time on the topic that
your opinion is hard-won, when you can name what you're arguing
*against* and steelman it, and when the venue's audience is the
right size to take the punch. Opinion pieces work best at venues
with a tradition of strong takes (RustConf, P99 CONF, KubeCon main
stage occasionally).

When it doesn't: when the opinion is weakly held, when the speaker
can't steelman the opposition, or when the venue treats opinions as
attacks.

Tell from an opinion-piece talk:

- The speaker can recite, in good faith, the strongest argument
  against their position before destroying it.
- The speaker takes the audience somewhere they didn't expect.
- The talk would generate strong replies (blog posts, mailing-list
  threads) even from people who disagree.

### Picking a shape

Material → shape matrix:

| You have... | Best shape |
|---|---|
| A 2-year project, shipped, with measurable outcomes | Case study |
| Source code you wish more people understood | Deep-dive |
| A take you'd defend at a debate, with evidence | Opinion piece |
| A demo that mostly works | Case study (do *not* make it the demo show) |
| A library you maintain | Deep-dive |
| A new pattern you noticed | Opinion piece |

If two shapes both fit, the audience matters more than the material.
A case study at a deep-dive conference is a let-down; a deep-dive
at a case-study conference is a snooze.

## The structure

A 20-minute talk has four sections. The clock here is hard. The
audience's attention budget is harder.

- **Hook — 30 seconds.** One sentence that captures the entire
  talk in tweet form. Not a thesis statement; a *claim* the
  audience will want to challenge. "We deleted RabbitMQ and
  everything got better." "Your retry logic is dishonest." "The
  ORM you love is the bottleneck of your career."
- **Context — 2 minutes.** Who you are, what your system is, why
  the audience should care. The minimum required for the rest of
  the talk to land. Resist the urge to use more time; the
  audience will ask later if they need more.
- **Meat — 15 minutes.** The 3–5 ideas you came to deliver. One
  idea per ~4 minutes. Each idea: the claim, the evidence, and the
  takeaway-in-one-sentence.
- **Callbacks + close — 3 minutes.** Reuse a phrase or two from the
  hook. Restate the 3–5 ideas as a list. End on a single sentence
  the audience will remember and quote.

The 3-minute close is the part most speakers shortchange. It's
also the part that determines what the audience tells their
colleagues at lunch.

### Why 3–5 ideas, not 7

The audience can remember *3* ideas reliably, *5* with effort, and
*7 or more* not at all. A 20-minute talk with 7 ideas is a talk
with 3 retained ideas and 4 forgotten ones. Cut to 3–5 before you
walk on stage.

### Variable time budgets

- **Lightning (5 min):** Hook 15s, Context 30s, Meat 3 min (one
  idea), Close 75s. Skip the 3–5 ideas; pick one.
- **Long (45 min):** Hook 30s, Context 4 min, Meat 35 min (5
  ideas), Close 5 min. The Q&A budget grows proportionally.
- **Keynote (60 min):** Different beast. Different document.

## Slide design rules

Slides are not the talk. The talk is the talk. Slides exist to do
two things: anchor the audience visually when their attention drifts,
and provide a fallback when the audience can't quite parse what you
just said.

The rules:

### Rule 1: One idea per slide

If your slide has two ideas, it has two slides. The audience can
read or listen, not both at once; if both ideas are on the screen,
they read both and miss what you said. Split.

### Rule 2: Maximum 15 words per slide

A title and a short subtitle is fine. A full sentence is the
ceiling. A paragraph is forbidden.

The point of words on a slide is *cue*, not *content*. The content
is in your mouth. The slide cues the audience back to the room when
their attention drifts.

### Rule 3: Code samples must be readable at 24pt

If the audience in the back row cannot read the code, the code is
not on the slide. This means:

- ~10 lines maximum per slide.
- Strip imports.
- Highlight the *relevant* 1–3 lines (different color, or a
  callout box). The rest is context, and de-emphasized.
- If the code has changed across the talk, only show the *diff*.

If your code legitimately needs more than 10 lines to demonstrate
the point, you're showing the wrong code. Find the smallest example
that captures the idea.

### Rule 4: No bullets-on-bullets

Nested bullet lists are a crutch for unprepared talks. If you find
yourself writing a slide with three top-level bullets each containing
sub-bullets, you have multiple slides masquerading as one.

### Rule 5: One color of "Look here"

Pick one highlight color (yellow, orange, red — your choice). Use
it for one purpose: pointing the audience at the part of the slide
that matters. If you use it for two purposes ("this is important"
*and* "this is dangerous"), the audience can't tell which is which.
Pick a second color or use a different visual cue.

### Rule 6: Dark text on light background

Light-on-dark *looks* cool. Light-on-dark also doesn't show up well
in the inevitable photographs the audience takes to share on
social media, and the talk's afterlife on YouTube depends on the
slide content being legible. Pick the boring, robust default.

### Rule 7: No animation that doesn't serve

A slide that "builds" — bullets appearing one at a time as the
speaker mentions them — is fine. A slide that swooshes, fades,
rotates, or bounces is a distraction. The audience's attention is
expensive; spend it on the talk.

### Rule 8: Page numbers

Put the page number small in the corner. The audience asking "go
back to slide 47" during Q&A is a feature, and "page 47 of 60" is
the most honest way the audience can know how far through they are.

## The live demo question

The rule is: **don't, unless you can survive the demo failing.**

A live demo failing on stage is one of the most uncomfortable
20-second windows in your career. The audience pity-watches. Time
expands. You lose the room.

There are four conditions under which a live demo is defensible:

1. **The demo is offline-runnable.** It runs on your laptop, doesn't
   require WiFi, doesn't depend on a third-party service.
2. **You have rehearsed it ≥ 10 times.** Not "I checked it last
   night." Ten complete runs in the last week.
3. **You have a fallback.** A pre-recorded video of the demo loaded
   in a tab, ready to play, with the cue "okay, the live one isn't
   cooperating — here's what it looks like."
4. **You can recover gracefully.** You can laugh, name the failure,
   and continue without losing your composure. If you can't, don't
   try.

The single best alternative is the *recorded demo with commentary*:
play the video, narrate over it, and you get the visual without the
risk. Audiences accept this readily.

When live demos *do* work, they generate a kind of credibility
nothing else can. The bet is real; just make sure you've stacked the
deck.

## Q&A handling

The Q&A is where the talk's reputation is decided. A great talk
followed by a confused, defensive Q&A is remembered as a confused,
defensive talk. A merely-good talk followed by a crisp Q&A is
remembered as great.

### The "I don't know" answer

There will be questions you cannot answer. The L7-grade response is:

> "Honestly, I don't know. Here's how I'd find out: [specific
> method], and if you want, leave your email at the end and I'll
> follow up next week."

This is much, much better than:

- Bullshitting. The audience knows. Half the room can fact-check
  you in real time.
- Deflecting. "That's outside the scope of this talk" comes across
  as evasive.
- Half-answering. Saying something that sounds like an answer but
  isn't.

The "I don't know" answer is L7-grade because it requires three
things at once: confidence (you are not embarrassed to not know),
seniority (you know what you'd do to find out), and respect for
the asker (you're committing to follow up).

### The hostile question

Some attendees will ask in a hostile tone. Three patterns:

- **The peer who thinks you're wrong.** Steelman their position
  back to them ("I think you're asking whether X. Yes, that's a
  real tension"), acknowledge the trade-off honestly, restate your
  position. Don't dismiss them.
- **The salesperson with a competing product.** "That's an
  interesting product question. Happy to talk after the session."
  Don't engage on stage; the audience didn't come for this.
- **The person who just wants attention.** A 30-second response,
  then "let's continue offline." Move to the next question.

You are not obligated to engage at length with hostile questions.
The audience is not on the hostile asker's side; they're on yours,
provided you don't make a fool of yourself.

### The non-question

"More of a comment than a question..." The speaker's right move is
to acknowledge ("Thanks for that"), extract a question if there is
one, and move on. Don't engage in dialogue; the rest of the room
is waiting.

### The deep-cut question

The question that makes you go "huh, I didn't think about that."
Answer honestly, even if it's "I hadn't considered that. My first
instinct is X, but I'd want to think about it more. Find me after."
The audience respects on-the-spot honest reasoning more than canned
answers.

### Time management

- 3–5 questions in a 10-minute Q&A.
- If the question is rambling, summarize it back ("So you're asking
  X — let me answer that") before answering. Helps the audience and
  helps you.
- End on time. "We have time for one more question" then take it.
  Better than overrun.
- After Q&A: "I'll be at the back of the room for the next 20
  minutes if anyone wants to follow up." Then *be at the back of the
  room*. This is where 30% of the talk's value gets delivered.

## Speaker mechanics

Most of these are obvious but get forgotten under stress:

- **Show up early.** 30 minutes before. Check the laptop adapter,
  the clicker, the mic. The AV team is your friend; learn their
  names.
- **Bring your own adapters.** USB-C → HDMI, USB-C → VGA, USB-A
  for the clicker. Conference AV is unreliable; your prep doesn't
  have to be.
- **Bring water.** A talk with a dry mouth is much harder than a
  talk with a sip of water.
- **Wear something boring.** The audience should remember the talk,
  not the shirt. (Unless your shirt *is* the talk, e.g. you're
  giving a talk about the company that's on the shirt.)
- **Speak to the back row.** Project your voice to the back of the
  room; the front row hears you fine either way.
- **Pause more than feels comfortable.** Silence in a talk feels
  longer to the speaker than to the audience. A 2-second pause
  after a key claim lets the audience absorb it; a 0.5-second pause
  feels rushed.
- **Move with purpose.** Pacing nervously is a tell. Standing
  still is fine. Moving deliberately (one side of the stage to
  the other when the topic shifts) is great. Pick one mode.
- **Do not read your slides.** The audience can read; reading is
  the fastest way to lose them.

---

## Worked outline: "Outbox without RabbitMQ: shipping reliable side-effects on Postgres alone"

This is a worked outline of a 20-minute talk based on the code in
[`projects/08-outbox-demo`](../../projects/08-outbox-demo/) and the
decision documented in ADR
[`0006-outbox-over-broker`](../01-architecture-decisions/0006-outbox-over-broker.md).

**Shape:** Opinion piece, with case-study elements.

**Venue:** A Rust/backend conference. SREcon, RustConf, P99, or
similar.

**One-sentence thesis:** Most teams that adopted Kafka or RabbitMQ
for reliable side-effects could have stayed on Postgres alone — and
the cost of the broker is hiding in your incident report.

**The three ideas the audience leaves with:**

1. The *dual-write problem* is the real reason for the outbox
   pattern. If you solve dual-write, you've solved 80% of why
   teams reach for a broker.
2. Postgres + `FOR UPDATE SKIP LOCKED` is a real queue, not a hack.
   Modern Postgres handles tens of thousands of dispatched events
   per second on a single primary.
3. The cost of a broker isn't the broker; it's the operational
   surface (a second persistence layer, a second on-call surface,
   a second incident-response playbook).

### Hook (0:00–0:30)

[Slide: large text, dark on light, no logo]
> **"We deleted our message broker. The system got more reliable."**

(Speak the line. 2-second pause. Let the audience react.)

> "I'm here to tell you you might be able to do the same thing.
> Specifically: that the side-effects layer most teams reach for
> RabbitMQ or Kafka to solve, you can solve with the database you
> already have. And I'll show you the code."

### Context (0:30–2:30)

[Slide: one sentence, "MemberClub: SaaS subscription product, ~10k
events/sec at peak, Rust + Postgres."]

> "Quick context. I'm a principal engineer at MemberClub. We run
> a subscription SaaS. Our event volume is around 10,000 events per
> second at peak — billing events, email triggers, webhook
> deliveries, that kind of thing. Our stack is Rust + Postgres."

[Slide: "Until 2024, we ran RabbitMQ for those events. Then we
didn't."]

> "Until 2024, we ran RabbitMQ in front of our Postgres for those
> events. Then we ripped it out and ran everything through Postgres
> alone. This talk is about why, what we did, and what we learned.
> Three ideas — write them down or remember them."

[Slide: the three ideas, one per line, big text.]

> "One: the dual-write problem is the real reason for the outbox.
> Two: `FOR UPDATE SKIP LOCKED` is a real queue. Three: brokers are
> cheap; their operational surface is expensive."

### Meat — Idea 1: The dual-write problem (2:30–7:00)

[Slide: a tiny code snippet showing the bad pattern]
```rust
// The bad pattern. Looks innocent. Isn't.
let order = db.insert_order(&new_order).await?;
broker.publish("order.created", &order).await?;
Ok(())
```

[Highlight color on `broker.publish`.]

> "Here's the bad pattern. Most teams' first cut of 'event-driven'
> looks like this. Insert into the database. Publish to the broker.
> Two lines. Innocent.
>
> The problem: those two lines are not atomic. Four things can
> happen."

[Slide: 4-quadrant table. DB success/fail × Broker success/fail.]

> "DB succeeds, broker succeeds — happy path. DB fails, broker
> doesn't publish — also fine, transaction rolled back. DB
> succeeds, broker fails — you have a row in the database with no
> corresponding event. DB fails — wait, but you already published?
> — Now you have an event in the broker for a thing that doesn't
> exist."

[2-second pause.]

> "Bottom row is the nightmare. In production, you will hit it.
> The frequency is roughly your network failure rate times your
> traffic. At 10k events/sec, that's a *daily* corruption event
> if your broker isn't co-located with your database. Maybe
> hourly."

[Slide: the outbox pattern]
```rust
// The outbox pattern. Atomic.
let mut tx = db.begin().await?;
let order = tx.insert_order(&new_order).await?;
tx.insert_outbox_row("order.created", &order).await?;
tx.commit().await?;
```

> "The outbox pattern fixes this by putting both writes in the same
> transaction. Now there are only two outcomes: both writes commit,
> or neither does. The broker entirely exits the request path.
> Some background worker reads the outbox table and dispatches.
>
> The first idea I want you to leave with is this: **the outbox
> pattern is not about the broker. It's about the dual-write
> problem.** Once you've solved dual-write, the choice of broker
> is a tactical detail."

[Slide: "Idea 1: The outbox pattern is about dual-write, not the
broker."]

### Meat — Idea 2: Postgres as a real queue (7:00–11:30)

> "Okay, but if you've solved dual-write with the outbox, you still
> need to dispatch. The conventional wisdom is: a database is not a
> queue, you need Kafka or RabbitMQ or SQS or whatever to actually
> deliver events. Let me show you why that's not true."

[Slide: the claim SQL]
```sql
UPDATE outbox
   SET status = 'processing',
       attempts = attempts + 1,
       claimed_at = NOW()
 WHERE id = (
    SELECT id
      FROM outbox
     WHERE status = 'pending'
       AND next_attempt_at <= NOW()
     ORDER BY next_attempt_at
     FOR UPDATE SKIP LOCKED
     LIMIT 1
)
RETURNING *;
```

[Highlight color on `FOR UPDATE SKIP LOCKED`.]

> "This is the claim query from our worker. It pulls one row at a
> time. The magic is `FOR UPDATE SKIP LOCKED`, which arrived in
> Postgres 9.5 — ten years ago — and tells the planner: lock this
> row exclusively, but if you can't, *skip* it and find another
> one. The result is that you can run N parallel workers and each
> one gets a *different* row. There is no contention. No leader
> election. No coordinator. Just N workers and the database picking
> who gets what."

[Slide: numbers]
> 10,000 events/sec sustained
> 16 worker processes
> Postgres primary, 8 vCPU, 32GB RAM
> Median claim latency: 1.2ms
> P99 claim latency: 4.8ms

> "These are the numbers from our production. 10k events/sec on a
> single primary, 16 workers, P99 claim latency under 5
> milliseconds. We are nowhere near the ceiling. People underestimate
> what modern Postgres can do."

[Slide: the rest of the worker loop]

```rust
async fn run_once(pool: &PgPool, dispatcher: &impl Dispatcher) -> Result<()> {
    let Some(row) = claim_next(pool).await? else { return Ok(()) };
    match dispatcher.dispatch(&row).await {
        Ok(()) => mark_done(pool, row.id).await?,
        Err(e) => mark_failed(pool, row.id, &e, &row).await?,
    }
    Ok(())
}
```

> "The worker loop is shorter than the slide implies. Claim a row.
> Dispatch it. Mark done or failed. Loop. The complexity is in the
> backoff schedule and the max-attempts ceiling, which I'm going
> to skip — the code is open source [point at link] and it's about
> 200 lines.
>
> Second idea: **`FOR UPDATE SKIP LOCKED` makes Postgres a real
> queue.** Not a hack. Not a temporary measure until you 'upgrade'
> to Kafka. A real, production-grade queue for most workloads
> you'll encounter at your career."

[Slide: "Idea 2: `FOR UPDATE SKIP LOCKED` is a real queue."]

### Meat — Idea 3: The hidden cost of a broker (11:30–16:00)

> "Now the opinion piece. People reach for Kafka because it's
> Kafka. Brokers are cool. They're well-engineered. They're well-
> documented. They have a thriving consulting industry. So why,
> then, did we delete ours?
>
> Because the broker isn't free."

[Slide: a 2-column comparison.]

|  | Postgres outbox | Kafka |
|---|---|---|
| Persistence layers | 1 | 2 |
| On-call rotations | 1 | 2 |
| Disaster recovery procedures | 1 | 2 |
| Monitoring dashboards | 1 set | 2 sets |
| New-engineer ramp-up | 1 system | 2 systems |
| Cross-system consistency model | N/A | Eventual, with edge cases |
| Network partition mode | DB unreachable = system down | DB OK but broker partitioned = weird state |

> "The broker is not the line item. The line item is the
> *operational surface*. A second persistence layer means a second
> on-call rotation, a second disaster recovery runbook, a second
> set of dashboards. It means a new engineer's ramp-up time
> doubles in the side-effects layer. It means every postmortem
> starts with 'which system was the source of truth at the moment
> the bug occurred?' — and that question is usually the most
> expensive part of the investigation.
>
> We measured the marginal engineering cost of Kafka in our org
> at roughly **0.8 FTE per year**, factoring in the on-call, the
> upgrades, the noisy alerts, the new-hire onboarding. Less than
> a full engineer, more than half. For our event volume, that's
> not justified."

[2-second pause.]

> "The exception, before someone asks. If you have multiple
> *consumers* of the same event stream — different teams, different
> services, fan-out — then a broker becomes a useful fan-out
> primitive. Or if your event volume is genuinely in the high
> hundreds of thousands per second, sustained, where Postgres starts
> being uncomfortable. Or if you have a regulated message-bus
> requirement. For most teams I've worked with, none of these
> apply.
>
> Third idea, the opinion: **the broker is the cheap part. The
> operational surface around it is the expensive part. Account for
> it honestly.**"

[Slide: "Idea 3: The broker is cheap. Its operational surface is
expensive."]

### Callbacks + close (16:00–19:00)

> "Let me bring it home."

[Slide: the three ideas, again. Same slide as the context section.]

> "Three ideas to leave with.
>
> **One: the outbox pattern is about dual-write, not the broker.**
> Once you've solved dual-write, the broker is a tactical detail.
>
> **Two: `FOR UPDATE SKIP LOCKED` is a real queue.** Postgres at
> 10k events/sec on a single primary, 16 workers, no contention.
> The pattern works.
>
> **Three: the broker is cheap; its operational surface is
> expensive.** Account for it honestly when you compare options."

[2-second pause.]

[Slide: just the title of the talk]

> "We deleted our message broker. The system got more reliable.
> Not because RabbitMQ was bad — RabbitMQ is great — but because
> we didn't need it, and the operational cost of running it was
> a tax we'd been paying without measuring.
>
> If you take one thing away from this talk, take this:
> **measure your operational surface before you scale it.** The
> broker is the visible cost. The on-call rotation is the
> invisible one. The invisible one is bigger.
>
> Code is at [URL — the public mirror of
> `projects/08-outbox-demo`]. ADR is published at the same place.
> I'm at the back of the room for the next half hour. Thank you."

[Slide: contact info, code link, ADR link, talk hashtag.]

### Q&A — anticipated questions and answers

The speaker should rehearse these. They will come up.

**Q: "What about at scale? Surely Kafka beats Postgres at 1M
events/sec."**

A: "Probably yes. We don't run at 1M events/sec. If you do, Kafka
is a legitimate answer — and so is partitioning your Postgres, and
so is per-tenant outboxes. The thing I'd push back on is
*reaching* for Kafka at 10k/sec because of a number you read in a
talk about a company doing 1M/sec. Measure your workload."

**Q: "What about fan-out? You have one event consumer in your
example."**

A: "Fan-out is where brokers earn their keep. In our system, the
dispatcher is in-process and dispatches to one consumer at a
time, but there's nothing stopping us from dispatching to N
consumers in parallel. The harder version of the question is:
what about cross-service fan-out where different teams own
different consumers? At that point I'd consider a broker, but I'd
also consider whether the cross-service fan-out is essential or
accidental. Most that I've seen is accidental."

**Q: "What if the dispatcher process dies mid-dispatch?"**

A: "Great question — this is the failure mode the
`max_attempts` ceiling addresses. The row stays in `processing`
state with a `claimed_at` timestamp. A separate sweeper job moves
rows from `processing` back to `pending` if `claimed_at` is older
than the timeout. This means the event might be dispatched
twice — your downstream consumer needs to be idempotent. Which
you'd need anyway for any reliable dispatch system."

**Q: "Why not use Postgres LISTEN/NOTIFY?"**

A: "LISTEN/NOTIFY is fire-and-forget — if the listener isn't
connected when the NOTIFY fires, the event is lost. It's a
notification primitive, not a queue primitive. We use it as a
*wake-up* signal for the worker (so the worker can sleep without
polling), but the outbox table is the source of truth."

**Q: "We have an existing Kafka. Should we move to Postgres?"**

A: "Probably not, unless the Kafka is causing measurable pain.
Migration cost is real. The talk is about the *initial* choice
more than the *current* state. If you already have a working
system, the bar for changing it is higher than the bar for not
adopting in the first place."

**Q (from a hostile asker): "This is just because you don't
understand Kafka."**

A: "Could be. Help me — what specifically would Kafka solve for our
workload that the Postgres outbox doesn't? I'm not asking to win
the point; I'm asking because I genuinely want to know if I'm
wrong." (This is the L7-grade "I don't know" deployed against the
unstated assumption in the question.)

---

## Preparation timeline

For a 20-minute talk at a meaningful conference:

| Weeks before | Task |
|---:|---|
| 12 | Outline written. Three ideas chosen. Hook written. |
| 10 | First slide draft. Roughly 1 slide per minute, so 20 slides ± 5. |
| 8 | First full talk-through to a wall (alone, with a timer). |
| 6 | First talk-through to a colleague. Get their feedback in writing. |
| 4 | Slides revised. Second colleague talk-through. |
| 3 | Internal brown-bag rehearsal. Wider audience, real Q&A. |
| 2 | Final slide pass. Print speaker notes. Verify equipment. |
| 1 | One full run-through per day. Sleep. |
| 0 | Show up early. Drink water. Talk. |

The biggest mistake speakers make is preparing for 2 weeks before
a talk where the slot was confirmed 3 months earlier. The talk
that lands is the one that was rehearsed 6+ times.

## Post-talk hygiene

After the talk:

- **Stay at the back of the room** for at least 20 minutes. The
  follow-up conversations are where careers move.
- **Post the slides** within 48 hours, ideally on a personal site
  you control. Conference proceedings sites lose URLs.
- **Tweet/post the link to the slides** with the talk's hook line.
  This is the moment your talk converts to a long-tail asset.
- **Write a blog post** within 2 weeks that restates the talk's
  ideas in written form. People who didn't attend read the blog
  post; the blog post is what gets shared 6 months later.
- **Open-source the demo code** if you haven't already. Linked from
  the slides and the blog post.
- **Send personal follow-ups** to anyone who asked a good question
  or whose follow-up you promised. This is where the "I'll find out"
  promise gets honored.

A talk that doesn't get a blog post is a talk whose audience is
limited to the room. A talk that gets a blog post is a talk that
will be cited for years.

---

## A final note

A conference talk is a leadership artifact. It does several things
at once: signals what your team is thinking about, attracts
contributors and hires, levels you up by forcing you to articulate
clearly, and contributes to the field's collective understanding of
how to build things.

It is also exhausting, time-expensive, and emotionally fraught.
You will give bad talks. You will give talks that don't land. The
recovery from a bad talk is to give another talk; the way to give a
better talk is to study the talks that landed and copy their
structure.

Most senior engineers under-give talks. The bar for a useful
contribution to a conference is lower than they think, and the
career return is higher than they think. If you have a thing to
say and the discipline to say it well, the talk is worth giving.

See also:

- [`README.md`](./README.md) — directory framing
- [`01-vision-doc-template.md`](./01-vision-doc-template.md) — Bet 3
  in the worked example concerns publishing `mc-auth` as a crate; a
  conference talk about that crate is one of the deliverables
- [`03-oss-maintainership-playbook.md`](./03-oss-maintainership-playbook.md)
  — talks are part of the maintenance flywheel for a popular crate
- [`../../projects/08-outbox-demo/`](../../projects/08-outbox-demo/) —
  the code referenced in the worked outline
- [`../01-architecture-decisions/0006-outbox-over-broker.md`](../01-architecture-decisions/0006-outbox-over-broker.md)
  — the ADR the talk is built on
