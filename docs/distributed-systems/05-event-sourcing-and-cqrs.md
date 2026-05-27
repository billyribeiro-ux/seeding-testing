# Event Sourcing and CQRS

> *Store the events. State is what you fold them into. Read models are
> what you project them onto. Everything else is a consequence.*

This essay is the decision behind `projects/16-event-sourcing` — when
and why that lab exists, and the much louder warning about when *not*
to reach for it. Event sourcing is the most-misapplied pattern in the
distributed-systems toolbox; the failure mode is usually a team that
adopted it because "audit trail" sounded compelling and discovered six
months later that they own a custom database they cannot evolve.

## The problem

You have a domain object — a `BankAccount`, a `Subscription`, an
`Order` — and you want to know:

- What is its **current state**? (Standard CRUD answers this.)
- What is the **history** of how it got there?
- What would it have looked like **last Tuesday at 14:32**?
- If we change our reporting requirements, can we **regenerate**
  yesterday's reports without losing data?
- The auditors are here — can we **prove** the current balance is the
  exact sum of every credit and debit ever recorded?

Standard CRUD answers question 1 in a single SELECT and answers
questions 2–5 by writing a separate `audit_log` table that you pray
nobody forgets to write to. Event sourcing inverts the relationship:
the log is the source of truth, and the current state is a derived
view.

## What event sourcing *is*

Three rules:

1. **Every state change is captured as a past-tense event.** Not
   "deposit \$10," but "Deposited(10)." Events are immutable facts. We
   record what happened; we do not record what we wish had happened.
2. **The log is append-only.** Once an event is written, it never
   changes. (Compaction and snapshotting are *additions* to the log,
   not edits — see below.)
3. **State is a left-fold over the log.** `state = events.fold(empty,
   apply)`. The current balance is `Σ deposits − Σ withdrawals`,
   computed from the log every time you need it. (In practice you
   cache the fold; see "snapshotting.")

```
   commands         events                      state
      │                │                          │
      ▼                ▼                          │
   handle ──── pure ────▶ Vec<Event>              │
                              │                   │
                              ▼                   │
                       append(stream, version)    │
                              │                   │
                              ▼                   │
                         event log ─── replay ───▶ apply
                              │
                              └─── subscribe ───▶ projections (read models)
```

The `projects/16-event-sourcing` lab implements this picture in ~700
lines. The `Aggregate` trait enforces the split that makes it work:

```rust
trait Aggregate {
    type Event;
    type Command;
    type Error;

    // The ONLY mutator. Deterministic, total, never fails.
    fn apply(&mut self, event: &Self::Event);

    // Pure. Returns the events a command would produce, or an error.
    // Does NOT mutate `self`.
    fn handle(&self, command: Self::Command) -> Result<Vec<Self::Event>, Self::Error>;
}
```

`handle` is where business rules live. `apply` is mechanical state
transition. Replay calls `apply` over the log; it never re-runs
`handle`. This is the rule that makes the pattern safe: yesterday's
events were valid when they were written, and replay must reproduce
yesterday's state byte-for-byte, even if today's business rules would
reject them. (A common bug: putting a "is the user still allowed to do
this?" check inside `apply`. Now replay depends on the *current*
permission state, and you cannot reconstruct old states.)

## What event sourcing *isn't*

This is the second half of the essay because most adoption failures
come from treating ES as the answer to a question it does not answer.

- **It is not a message queue.** Kafka can store events, but storing
  events in Kafka does not make your system event-sourced; it just
  means your message queue is durable. Event sourcing is a
  *modeling* choice ("the log is the truth"), not a deployment choice.
- **It is not pub/sub.** Projections subscribe to the event log, but
  the log is not a topic and the projection is not a consumer group.
  See `projects/16-event-sourcing/src/lib.rs::EventStore::subscribe` —
  it is a poll loop against a SQL table, deliberately mundane, so the
  reader does not mistake the pattern for a broker.
- **It is not a free upgrade over CRUD.** You will spend the cost
  difference somewhere: schema evolution, snapshotting, projection
  rebuild costs, the test surface area of replay correctness.
- **It is not "you can rewind your database."** You can rewind the
  log; rebuilding the current read models from a rewound log takes
  hours-to-days on a real-sized stream. Plan for it.
- **It is not the outbox pattern.** Outbox (see
  `projects/08-outbox-demo` and ADR 0006) commits a business write and
  a side-effect intent in one transaction, then a worker drains the
  intents. Event sourcing makes the *business state itself* a sequence
  of events. You can run an outbox without ES; you almost always run
  ES with some outbox-shaped mechanism on top.
- **It is not the saga pattern.** Sagas (see `projects/14-sagas` and
  `03-sagas-vs-2pc.md`) orchestrate a workflow across multiple stores
  with compensations on failure. ES is a way to *store one store*.
  You can model a saga's progress as an event stream; that is a
  legitimate use, and it is what some teams adopt ES for. But the two
  patterns answer different questions.

## When event sourcing wins

A short list. If your domain is not on it, default to CRUD.

1. **Audit-required domains.** Banking, healthcare, government, any
   domain where the regulator can ask "show me every change to this
   record" and "we lost the audit log" is a career-ending answer.
   Event sourcing makes "the audit log IS the database" — there is no
   secondary log to forget to write to, no `audit_log` row out of sync
   with the business row, no race condition between the business write
   and the audit insert.
2. **"What did the system know on date X?" is a real query.**
   Insurance claims processing, regulatory reporting, anything where
   you need to reconstruct a past view ("what was the customer's plan
   on March 14?"). With the events you can fold up to any
   `occurred_at` cutoff; with CRUD you can't.
3. **Read-model evolution is faster than write-model evolution.** This
   is the most-underrated reason. New product question shows up
   ("how many customers withdrew >\$1000 the day after we sent the
   promo email?"); you write a new projection, replay it against the
   log, and the answer is in the read model. With CRUD you would have
   needed that report in your schema from day one.
4. **Temporal logic is the domain.** Workflows, state machines, leases,
   subscriptions with grace periods. The events are the conversation;
   the current state is the summary. CRUD models often lose the
   "how did we get here" information that the next workflow step needs.
5. **The read patterns are heterogeneous.** OLTP write path + OLAP
   reporting + a graph-shaped search index + a time-series dashboard
   from the same underlying data. Each is a projection; each is the
   right tool for its query. (This is the "Q" in CQRS.)

## When event sourcing loses

If any of these apply, do not adopt event sourcing.

1. **Simple CRUD with a low audit bar.** A blog. A todo app. The
   pricing page CMS. You will spend more time wiring projections than
   you would have spent on a `notes` table.
2. **Low write volume but every read is a complex query.** Folding 30
   events on every read is fine. Folding 30 million events on every
   read is not, and snapshotting buys you time but adds complexity.
   If the read shape is what makes your system hard, ES probably
   isn't where to spend.
3. **The team has never done it.** ES is a paradigm shift. The first
   six months are expensive (and the first big mistake — putting
   business logic in `apply`, or storing state-as-events instead of
   facts-as-events — is a multi-week rewrite). If "we already do
   CRUD well" is true, the bar for switching is high.
4. **Strong cross-aggregate consistency is a hard requirement.**
   Aggregates are consistency boundaries. Cross-aggregate consistency
   is eventual, achieved through projections + sagas. If the domain
   needs "the same transaction commits across these three entities or
   nothing does," CRUD + a relational DB is a better fit than ES.
5. **You are tempted to adopt ES *because* you discovered a missing
   audit feature.** Add an `audit_log` table first. Live with it for a
   year. *Then* decide.

## CQRS, and why it is a *consequence* of ES

CQRS = Command Query Responsibility Segregation. The shortest version:
the model you write through and the model you read through are
different.

This is not specifically an ES idea — you can CQRS a CRUD app by
giving the read side its own denormalized tables — but ES makes it
*free*, because once events are the source of truth, every read model
is a derived view:

```
                  ┌──────────────────────────┐
   commands ─────▶│  Aggregate (write side)  │── events ──▶ event log
                  └──────────────────────────┘                  │
                                                                ├─▶ balances table
                                                                ├─▶ daily-statements table
                                                                ├─▶ fraud-detection index
                                                                └─▶ ...
```

In the lab, `BalanceProjection` is the simplest possible read model: a
poll loop that reads from the event store and maintains
`account_balances(stream_id, balance_cents, status)`. It demonstrates
two production rules:

1. **The read model + checkpoint live in one transaction.** Otherwise a
   crash can leave the checkpoint ahead of the read-model write, and
   the projection has silently skipped an event. See
   `BalanceProjection::handle` — it bumps `projection_checkpoints` and
   updates `account_balances` inside the same `tx.commit()`.
2. **Eventual consistency is the default; you must opt in to "wait for
   the read model to catch up."** A user opens an account and
   immediately queries the balance: the projection may not have run
   yet. Production answers are (a) read your own writes from the
   write side, (b) attach a `position` to the response and have the
   client poll until the read model shows that position, or (c) accept
   the staleness and design the UI for it.

## Trade-offs vs the rest of this curriculum

The curriculum already teaches two other "I need to coordinate
durable side effects" patterns. The decision table:

| Situation | Pattern | Where |
| --- | --- | --- |
| One store, one business write + one side effect, atomic | Outbox | `projects/08-outbox-demo`, ADR 0006 |
| Many stores (Stripe + DB + email), need to undo on failure | Saga + compensation | `projects/14-sagas`, `03-sagas-vs-2pc.md` |
| The history of *one entity* is the business value | Event sourcing | `projects/16-event-sourcing`, this essay |
| Cross-service consistency over a long workflow | Saga whose state IS an event stream | combine the two |

The relationships:

- ES and outbox compose. A common production shape is "event-sourced
  aggregates write events to the local log; an outbox-style worker
  publishes selected events to other services." The same transaction
  that appends to the event log also writes the outbox row.
- ES and sagas compose. The saga orchestrator's state can itself be
  event-sourced (every step result is an event); the saga steps that
  touch event-sourced aggregates use those aggregates' command API.
  ADR 0008 ("Stripe is the rail") explains why MemberClub uses a saga
  for billing even if the aggregates were event-sourced: Stripe is
  not a participant we can replay events into.
- ES and 2PC do not compose. ES gives up the global-transaction story
  the moment you have a second store; you reach for sagas to bridge.
  See `03-sagas-vs-2pc.md` for why this is fine.

## The gotchas, in order of "you will hit this"

### 1. Optimistic concurrency control

Two writers each load a stream at version N, each handle a command,
each try to append at version N+1. Without OCC, one of them silently
overwrites the other and the log is corrupted (two events claim to
follow the same predecessor; replay is ambiguous).

The lab's store enforces OCC via a `UNIQUE(stream_id, version)` index.
Two INSERTs at the same `(stream_id, version)` → one wins, the other
fails with a unique-violation, which the store surfaces as
`EsError::ConcurrencyConflict { expected, actual }`. The caller's
retry loop is:

```rust
loop {
    let agg: BankAccount = store.load_aggregate(stream_id).await?;
    let version = store.current_version(stream_id).await?;
    let events = agg.handle(cmd.clone())?;
    match store.append(stream_id, version, &events).await {
        Ok(()) => break,
        Err(EsError::ConcurrencyConflict { .. }) => continue,
        Err(e) => return Err(e),
    }
}
```

Postgres production uses the same shape via a unique index + `ON
CONFLICT DO NOTHING ... RETURNING` or a `SERIALIZABLE` transaction.
The mechanism does not depend on the database; the constraint does.

### 2. Schema evolution

You renamed `Deposited.cents` to `Deposited.amount_cents` two sprints
ago. Yesterday a customer asks why their statement is wrong. You
replay their stream from 2019. What happens?

Three options, in increasing order of discipline:

1. **Tolerant readers.** Make events JSON, make every field optional,
   make the aggregate tolerate missing fields. Cheap to start; turns
   into a swamp by year three.
2. **Versioned event types.** `DepositedV1`, `DepositedV2`. The
   aggregate's `apply` matches on both. Newer code writes only V2.
   Explicit; survives long-term.
3. **Upcasters.** A pre-deserialization layer that rewrites V1
   payloads into V2 on the way out of the store. The aggregate sees
   only V2. The on-disk log still has V1 events (immutability!), but
   in-memory they are all V2. This is what big production engines
   (EventStoreDB, Axon) build in. The lab does not ship one — it is
   called out here so you know the standard escape hatch.

The teaching version of the rule: **events on disk are immutable;
events in memory are whatever the latest code understands.** The
upcaster is the bridge.

### 3. Snapshotting

Folding 30 events on every read is fine. Folding 3 million is not.

A snapshot is a serialized copy of the aggregate state at version V.
On load: find the latest snapshot ≤ current version, deserialize it,
then apply only the events after V. Loading is O(events since
snapshot) instead of O(events ever).

Rules:

1. **Snapshots are a cache.** You must be able to throw them all away
   and rebuild from the log. Otherwise they are a second source of
   truth, and now you have two sources of truth, which is the bug ES
   exists to prevent.
2. **Snapshots have versions too.** A schema change in the aggregate
   invalidates older snapshots. Tag them; ignore stale ones.
3. **Take them on a schedule, not on every write.** Every Nth event,
   or every K seconds. The lab does not ship snapshotting; with
   per-stream sizes below ~1000 events you do not need it.

### 4. GDPR and the immutable log

The right-to-be-forgotten meets the append-only log head-on. You
cannot just `DELETE FROM events WHERE owner_id = ?` — that breaks
replay, breaks projections, and is the bug your auditors will find
first.

Three workarounds, all imperfect:

1. **Crypto-shredding.** Encrypt PII inside the event payload with a
   per-user key. Store the keys in a separate keystore. To "forget"
   the user, destroy their key. The events remain; their payload
   becomes unreadable. Replay still works for everything that does
   not need the PII; projections that do need it see a `Tombstone`.
2. **Rewrite the stream.** Snapshot the aggregate at the user's
   request, replace the stream with a single "Anonymized" event whose
   payload contains the snapshot minus PII. You lose history of *that
   user*; the rest of the system is intact. This is the "fork the
   log" answer and it makes auditors nervous.
3. **Per-user streams + per-stream deletion.** If your event store
   gives each user their own stream, you can drop the stream
   wholesale. Projections downstream will see the drop and react. The
   schema discipline this requires is significant.

The honest answer is: **GDPR + event sourcing is a design constraint
you commit to up front, not a feature you bolt on later.**
Crypto-shredding is the most-common production pattern; it is what
most commercial engines support out of the box.

### 5. Replay performance and projection rebuilds

A new projection on a 100M-event log takes hours. Plan for it:

- Have a "rebuild" command that runs offline, against a snapshot of
  the event store, into a side table; swap the table in atomically
  when done.
- Make projections idempotent (key by `position`) so you can pause /
  resume / retry without double-application.
- Monitor projection lag (`max(position) − projection_checkpoint`).
  An alert on lag > N seconds catches the case where a projection
  silently dies.

The lab's `BalanceProjection::checkpoint()` exists so projection
runners can resume from where they left off. Production projections
add the lag metric, the rebuild path, and a way to *replace* a
projection without dropping the old read model until the new one is
ready.

## Further reading

1. **Greg Young's CQRS talks.** The original loud advocate; his "Why
   I created CQRS" essay is the shortest version. Skim the practical
   advice; ignore the architectural maximalism.
2. **Martin Fowler, "Event Sourcing"** (2005). The pattern's original
   canonical write-up. Still the clearest definition.
3. **Kleppmann, *Designing Data-Intensive Applications*, chapter 11**
   ("Stream Processing"). The most-honest treatment of the trade-offs;
   the chapter on event-sourced state stores is excellent.
4. **EventStoreDB documentation, "Persistent Subscriptions" page.**
   What a production ES engine actually exposes — competing
   consumers, retries, parking. Read it to calibrate how much our
   lab elides.
5. **The lab itself.** `projects/16-event-sourcing/src/lib.rs` is ~700
   lines. The tests are the executable specification of the rules in
   this essay.

## Where to go next in this curriculum

- `projects/16-event-sourcing` — implement a third aggregate
  (`Subscription` with a grace period, `Order` with line items) and a
  second projection (monthly statements). The shapes generalize.
- `projects/08-outbox-demo` — wire it into the event store: every
  appended event also writes an outbox row that publishes to a
  downstream consumer. The transaction makes the two atomic.
- `projects/14-sagas` — model the saga's progress as an event stream
  (`SagaStepCompleted`, `SagaCompensated`) and replace the in-memory
  state with `load_aggregate`. The orchestrator now survives a
  restart for free.
- ADRs 0006 (outbox) and 0008 (Stripe is the rail) for the
  decisions this lab does not revisit.
- `01-cap-pacelc.md` for the latency story of "the read model is
  eventually consistent with the write side."

If you read only one thing here: the **"What event sourcing isn't"**
section. The pattern is correct in a narrow band of problems. Most
"we should event-source this" instincts are answered by an
`audit_log` table and an outbox.
