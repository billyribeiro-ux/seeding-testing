# CAP and PACELC

> *During a partition, you choose Consistency or Availability. The
> rest of the time, you choose between Latency and Consistency.*

CAP is the one acronym every backend engineer knows and most of them
misuse. PACELC is the more honest version. This essay explains what
the real claim is, what it is **not**, and how MemberClub-style
services in this repo actually trade these off.

## The real claim of CAP

Eric Brewer, 2000. Refined by Gilbert and Lynch's proof, 2002. The
claim is:

> In the presence of a network partition (P), a distributed system
> cannot simultaneously provide both linearizable consistency (C) and
> total availability (A). It must give one of them up.

Three things to internalize:

1. **CAP is only about partitions.** "P" is not a knob you tune; it
   is a fact of the network. The system designer chooses what
   happens *during* a partition, not whether to allow partitions.
2. **"C" means linearizability**, the strongest single-key
   consistency model. Not "eventual." Not "read-your-writes." The
   real one: every read returns the most recent committed write.
3. **"A" means every non-failing node responds**, not "the cluster
   answers eventually." A node that returns `503 Try again later` is
   not available in CAP terms.

That is why CAP is sharp and useless at the same time: real systems
do not need linearizability for most operations, and a 503 with a
retry header is a perfectly reasonable answer. The framework forces
a false dichotomy.

## The meme version (which is wrong)

You will see this in slides:

```
       C
      /|\
     / | \
    /  |  \
   A---+---P     "pick any two"
```

This is wrong because:

- P is not a choice. It happens to you.
- "Pick any two" implies CA systems exist. They don't in the
  distributed sense; a single Postgres node is CA the way "a single
  person" is a "team."
- The model collapses many real distinctions (latency, durability,
  read-your-writes) into a binary.

If you hear "pick any two" in a design review, the speaker has not
read past the Wikipedia summary.

## PACELC: the better framing

Daniel Abadi, 2010. PACELC reads as:

> If there is a Partition (P), how does the system trade off
> Availability (A) and Consistency (C);
>
> **Else** (E), when running normally, how does the system trade off
> Latency (L) and Consistency (C)?

Two trade-offs, not one. The "E" half is the half that actually
applies to your service 99.9% of the time. Examples:

| System | PA/PC | EL/EC | What it means |
| --- | --- | --- | --- |
| Postgres (single primary) | PC | EC | If the primary is partitioned, the replicas can't accept writes (PC). Normally, every read sees every committed write (EC). |
| Postgres (with async read replicas) | PC | EL | Writes go to the primary; reads from replicas are stale by a few ms. Choose latency over consistency on reads. |
| Cassandra / Dynamo (QUORUM) | PA | EL | During partition, accept writes on either side; reconcile later. Normally, low-latency reads with bounded staleness. |
| etcd / Consul / Zookeeper | PC | EC | Strongly consistent. During partition, the minority side rejects writes (and reads, if linearizable). |
| Redis (single primary, async repl) | PA | EL | A partition between primary and replicas does not block writes; data on the wrong side may be lost. |

## The diagram

```
                       network
                        OK?
                         |
            +------------+-------------+
            |                          |
           yes (E)                    no (P)
            |                          |
       Choose L or C            Choose A or C
            |                          |
   +--------+--------+         +-------+-------+
   |                 |         |               |
  EL                EC        PA              PC
  "stale read     "every     "accept         "refuse
   from cache,     read       writes on      writes on
   ok"             linear-    both sides,    minority,
                   izable"    reconcile"     stay
                                              consistent"
```

You always live in one branch on the left **and** one branch on the
right. Most production stories blur because the answer differs by
*operation*, not by *system*.

## What MemberClub (this repo) trades off

MemberClub's stack:

- **Postgres** as the primary store. PC/EC for writes.
- **Read replicas** (Phase 11.3, conceptual). PC/EL for read paths
  that can tolerate staleness (`/v1/dashboard`).
- **Redis** as a cache + rate-limit store (Phase 11.4,
  `projects/11-redis-cache`). PA/EL — we accept that a Redis
  partition may briefly serve stale rate-limit windows.
- **Stripe** as the rail (ADR 0008). Stripe is its own PC system; we
  *mirror* into Postgres via the webhook. The mirror is eventually
  consistent (PA/EL) by construction: between the user paying and
  our webhook handler running, the mirror is stale by ~500 ms.

The concrete decisions:

1. **Money is PC.** A user's `subscriptions` row and `stripe_events`
   ledger are always strongly consistent with each other (they live
   in the same transaction). If Postgres is partitioned, we serve
   503s for writes — we never accept a write we cannot commit.
2. **Dashboards are EL.** `/v1/dashboard` may be a few seconds
   behind the truth; we tell the user "as of 14:03" in the UI.
3. **Sessions are PA/EL.** A logged-in user whose Redis session
   entry is missing gets a re-login prompt; we do not block the
   request waiting for Redis to come back.

These choices are not free. They show up as code:

- `apps/memberclub/api/src/billing.rs` — every Stripe event is
  ledgered *before* we touch any mirror table (PC writes).
- `projects/08-outbox-demo` — atomic write of "business row + side
  effect intent" in one transaction (PC), with the side-effect
  dispatch being eventual (EL).
- `projects/14-sagas` — when one PC store (Postgres) and one
  externally-owned store (Stripe, SendGrid) need to agree, we use a
  saga: the *whole composite operation* is now PA/EL, even though
  each participant is PC locally.

## Common misuses to push back on

- **"Postgres is CP."** Postgres is a *single-node store with
  replication*. CAP makes sense for it only when you ask "what does
  the *replica set* do during a partition?" — and the answer
  depends on how you have configured failover.
- **"NoSQL is AP."** Some NoSQL stores (DynamoDB, Cosmos) offer
  tunable consistency *per request*. Cassandra with `LOCAL_QUORUM`
  is PC under partition, EL normally. The store does not have a
  single CAP label.
- **"We need C, not A, because we handle money."** You need
  *durability* and *idempotency*. CAP-C (linearizability) is rarely
  the right specification for a money path; what you actually want
  is "no double-charge" (idempotency keys) and "no missed write"
  (outbox / saga). See `04-failure-modes.md` for why these are
  different.
- **"We're CP because we use Postgres."** You are *Postgres-CP*
  for any operation that fits in one transaction in one Postgres
  cluster. Anything that touches Stripe, SendGrid, Redis, or a
  read replica is some shade of EL.

## When does CAP actually matter?

CAP becomes operational at exactly two moments:

1. **Choosing a coordination service.** etcd vs Consul vs Zookeeper
   — all CP. Picking Redis sentinel here is a category error: it is
   PA. (See `02-consensus.md`.)
2. **Designing your failover behavior.** When the primary is
   unreachable, do you accept writes on the standby (PA, risk data
   loss) or refuse writes (PC, take downtime)? Postgres ships in
   "PC by default"; you have to configure it otherwise (and that
   configuration has been a featured Jepsen postmortem more than
   once).

For everything else, PACELC's *E* branch is where the real decisions
get made: cache TTLs, read replica lag tolerance, eventual
consistency budgets.

## The principal-engineer takeaway

- Say PACELC, not CAP. The four-letter answer is too coarse for any
  real design review.
- For each operation in your system, name where it sits on PA/PC and
  EL/EC. Most services have many answers, not one.
- "Strong consistency everywhere" is not a goal; it is a price tag.
  Pay it for money and identity; do not pay it for dashboards.
- If you cannot point at a file in the repo that *enforces* a
  consistency property, you do not have that property.

## Related

- `02-consensus.md` — how PC systems actually achieve agreement.
- `03-sagas-vs-2pc.md` — when you can't get PC across stores and
  have to live with PA/EL.
- ADR 0008 (Stripe is the rail) — the EL-of-our-mirror decision.
- ADR 0006 (outbox over broker) — the PC-then-EL pattern.
- *Designing Data-Intensive Applications*, chapter 9 (Consistency
  and Consensus).

## Going deeper

- *PACELC paper*, Daniel Abadi (2010): "Consistency Tradeoffs in
  Modern Distributed Database System Design." Six pages. Worth
  reading even if you only remember the framework.
- *CAP Twelve Years Later*, Eric Brewer (2012). Brewer himself
  walking back the meme.
- *Jepsen's analyses of Postgres replication* — the empirical view
  of what real partitions do to real clusters.
