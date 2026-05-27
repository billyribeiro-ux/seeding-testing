# docs/distributed-systems/

Short essays on the parts of distributed systems an L7 backend
engineer actually uses on the job. None of this is "from scratch
Paxos." All of it is "what trade-off do I name in the design review."

## Layout

```
docs/distributed-systems/
├── README.md             ← this file (the index)
├── 01-cap-pacelc.md      ← CAP, PACELC, and what Postgres + MemberClub trade
├── 02-consensus.md       ← Raft basics for design-interview level
├── 03-sagas-vs-2pc.md    ← the choice this curriculum makes
└── 04-failure-modes.md   ← the catalog: split-brain, clock skew, slow vs failed, …
```

Each essay is concept-first, then how, then why-it-matters. 200-400
lines. Read in order on a first pass; thereafter use as reference.

## When to reach for what

| Situation | Pattern | Where |
| --- | --- | --- |
| "I need atomic side effect + DB write in **one** store" | Outbox in the same transaction | `projects/08-outbox-demo` + ADR 0006 |
| "I need atomic side effect across **multiple** stores (Stripe + DB + email)" | Saga + compensation | `projects/14-sagas` + `03-sagas-vs-2pc.md` |
| "I need a single-leader cluster of N nodes to agree on a log" | Raft (etcd / Consul) | `02-consensus.md` |
| "I need to reason about latency under network partition" | PACELC | `01-cap-pacelc.md` |
| "I'm debugging mysterious lost-update bugs" | Failure-mode catalog | `04-failure-modes.md` |
| "The user retried; did anything happen twice?" | Idempotency keys | `apps/memberclub/api/src/billing.rs` |

If you are not sure which essay applies: start at `04-failure-modes.md`
and find the failure you are reasoning about; it points you to the
right tool.

## What this directory is *not*

- **Not a textbook.** Read Designing Data-Intensive Applications
  (Kleppmann, 2017) for that. Each essay here ends with a "go deeper"
  pointer.
- **Not a survey of every database.** We point at Postgres, etcd,
  Consul, Zookeeper as concrete prior art and leave the comparison
  matrices to vendor blogs.
- **Not a defense of any one approach.** Saga, 2PC, outbox, Raft —
  each one is correct in some context. The essays say *which*.

## Curriculum cross-references

- Phase 11 lessons cover background jobs (`05-background-jobs.md`),
  multi-tenancy (`06-multi-tenancy.md`), perf budgets.
- Phase 12 capstone asks for an ADR + an RFC + a postmortem; these
  essays give you the vocabulary.
- `projects/08-outbox-demo` is the worked example for the outbox.
- `projects/14-sagas` is the worked example for the saga.
- ADR 0006 (`docs/01-architecture-decisions/0006-outbox-over-broker.md`)
  is the decision the outbox project enacts.
- ADR 0008 (`docs/01-architecture-decisions/0008-stripe-is-the-rail.md`)
  is the rule that explains why MemberClub *needs* a saga and not just
  an outbox: Stripe is an external store, not a Postgres table.

## How to read these

1. **One pass top-to-bottom on a sleepy Friday afternoon.** You're not
   trying to memorize; you're trying to be able to recognize the shape
   later. ~30 minutes.
2. **As a lookup table in a design review.** Someone says "what about
   split-brain?" — `04-failure-modes.md` is open in another tab.
3. **As reference when writing an ADR.** Phase 12 RFC template lists
   "Failure modes considered" — this directory is your checklist.

## Style

- Concept first. The reader does not yet know what split-brain is when
  they open `04-failure-modes.md`.
- Then how. One small diagram (ASCII), one short example.
- Then why it matters *to this codebase*. Every essay names at least
  one file in `projects/` or `apps/` that is affected by the concept.
- Plain English. If a term is jargon ("quorum"), define it on first
  use.
- < 400 lines. Anything longer wants to be two essays.

## Adding a new essay

1. Pick a topic where you have caught yourself explaining the same
   distributed-systems concept three times to different audiences.
2. Open one of the existing essays and use it as a template.
3. Concept → How → Why-it-matters → Related.
4. Add an entry to the table above and to the index near the top of
   this file.
5. PR with a one-line summary in the description.

## Going further

Reading list, in priority order:

1. *Designing Data-Intensive Applications*, Martin Kleppmann (2017).
   Chapters 5-9 cover everything in this directory in 10x the depth.
2. *Raft paper*, Ongaro & Ousterhout (2014). 18 pages; the whole
   protocol fits on a single fold-out diagram.
3. *Distributed Systems for Fun and Profit*, Mikito Takada (free
   online). A breezier intro than Kleppmann.
4. *Sagas*, Garcia-Molina & Salem (1987). The original paper; 6 pages;
   still readable. The patterns here are 35 years old.
5. *Jepsen reports*, jepsen.io. What happens when distributed systems
   meet a chaos monkey. Pick three at random and read them.

If you read only one thing: chapter 8 of Kleppmann ("The Trouble with
Distributed Systems"). It is the why behind every essay here.
