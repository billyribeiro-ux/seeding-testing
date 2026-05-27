# Sagas vs Two-Phase Commit

> *2PC is the right answer when every participant is a database you
> own. Sagas are the right answer when at least one participant is
> Stripe. The outbox is the right answer when there is only one
> participant and you just need a side effect.*

This essay is the decision behind `projects/14-sagas` — when and why
that orchestrator exists. It also names the cases where you should
*not* use it.

## The problem

You want operation X to happen atomically across N stores:

- **Charge a card on Stripe**, **insert a `subscriptions` row in
  Postgres**, **send a welcome email via SendGrid**.

If any single one fails, you want the others to be undone (or never
to have happened). There is no transaction that spans Stripe +
Postgres + SendGrid; the network and the trust boundaries make that
impossible.

You have three families of answer:

1. **Two-phase commit (2PC).** A coordinator asks every participant
   to *prepare*, then either *commit* all or *abort* all.
2. **Saga.** A sequence of local transactions; each has a
   compensating action that undoes it. On failure, run the
   compensations of the successful prior steps in reverse.
3. **Outbox + idempotency.** Skip true atomicity; make the side
   effect idempotent and retried until it eventually lands.

The right one is whichever matches your *participant set*.

## Two-phase commit, in one minute

```
  coordinator                participants (P1, P2, P3)
       │                       │      │      │
       │── PREPARE? ──────────▶│      │      │
       │── PREPARE? ──────────────────▶│     │
       │── PREPARE? ─────────────────────────▶│
       │                       │      │      │
       │◀── yes (locked) ─────│      │      │
       │◀── yes (locked) ────────────│      │
       │◀── yes (locked) ───────────────────│
       │                       │      │      │
       │── COMMIT ────────────▶│      │      │
       │── COMMIT ────────────────────▶│     │
       │── COMMIT ───────────────────────────▶│
       │                       │      │      │
```

Phase 1 (PREPARE): every participant takes the locks needed to
guarantee they *could* commit, and replies `yes` or `no`. Phase 2
(COMMIT/ABORT): the coordinator broadcasts the outcome.

Properties:

- **Atomic.** Either all participants commit, or none does.
- **Blocking.** If the coordinator crashes between PREPARE and
  COMMIT, every participant sits holding locks indefinitely. Some
  implementations add a recovery protocol; in practice it is
  fragile.
- **Synchronous.** Latency is the max participant + 2 round trips.

You need 2PC when:

- All participants speak a `PREPARE` verb. In SQL land this is
  `XA` transactions (Postgres has it; many ORMs don't expose it).
- All participants will *honor* the prepared state indefinitely
  (until the coordinator decides). Stripe will not. SendGrid will
  not. An external HTTP API will not.

You should not use 2PC when:

- The participant set includes any external SaaS.
- You care about latency. 2PC ties you to the slowest participant
  for both phases.
- You cannot tolerate stuck-prepared transactions on a coordinator
  crash.

Therefore 2PC is the right answer for "atomic write across two
schemas in the same Postgres cluster," and basically wrong for any
cross-service problem. Most production systems never use it.

## Sagas, in one minute

A saga is a sequence of local transactions L1, L2, …, Ln. Each Li
has a *compensation* Ci. The orchestrator:

```
  L1.execute → ok
  L2.execute → ok
  L3.execute → FAIL
        │
        ▼
  C2.compensate     ← LIFO unwind
  C1.compensate
        │
        ▼
      report failure
```

Properties:

- **No global transaction.** Each Li commits locally.
- **No locks held across steps.** Each step releases as soon as
  it commits.
- **No isolation.** Mid-saga, observers can see partial state.
- **Compensations must be idempotent and semantic.** A refund is
  *not* a bit-for-bit inverse of a charge; it leaves both a charge
  and a refund on the statement. That is the right business
  answer; restore the *invariant*, not the *bits*.

Use a saga when:

- The participants are heterogeneous (Stripe + Postgres + email).
- Operations are long-lived. A saga whose individual steps each
  take ~500 ms can run for many seconds and still feel fast,
  because no step is blocked waiting for others to vote.
- You can write a meaningful compensation for each step.

Do not use a saga when:

- The participant set is one database. The local transaction
  already gives you atomicity; do not introduce a saga.
- A step has no meaningful compensation (e.g., "physically launch
  the rocket"). At that point you are in human-in-the-loop
  territory.
- You need isolation. There is no saga isolation. If your reads
  cannot tolerate seeing partial state, see "the third way" below.

`projects/14-sagas` is the worked example: an `upgrade_subscription`
saga across Stripe (charge), Postgres (subscription row), SendGrid
(welcome email). The compensations refund the charge and deactivate
the row; the welcome email is best-effort (its compensation is a
no-op because you cannot un-send email).

## The third way: outbox + idempotency

If you have **one** transactional store and want a side effect that
*eventually* happens, you do not need a saga. You need an outbox.

```
  begin tx
    INSERT INTO subscriptions (...)        -- business write
    INSERT INTO outbox (kind, payload)     -- side-effect intent
  commit tx
        │
        ▼ (async worker)
  pop next outbox row
    perform side effect (with idempotency key)
    mark done
```

Properties:

- **Atomic** business write + intent. They share one transaction.
- **At-least-once** side effect. Workers may retry; the side
  effect must be idempotent.
- **No compensation needed.** Either the side effect lands
  eventually or you page on-call.

Use the outbox when:

- The side effect is "do this thing later" rather than "do this
  thing as part of the larger atomic operation."
- The action is idempotent (email send, webhook dispatch, cache
  invalidation, search index update).

`projects/08-outbox-demo` is the worked example. ADR 0006
(`docs/01-architecture-decisions/0006-outbox-over-broker.md`) is the
decision.

## The decision matrix

| Situation | Use | Why |
| --- | --- | --- |
| All participants are tables in one Postgres cluster | Single transaction | The DB already gives you atomicity. |
| Two Postgres schemas; both speak XA | 2PC | True atomicity; participant set is closed and trusted. |
| Postgres + Redis cache invalidation | Outbox | Cache is best-effort; idempotency suffices. |
| Postgres + send-email | Outbox | Email is async by nature; "send later" is acceptable. |
| Postgres + Stripe + SendGrid + must-rollback | Saga | Heterogeneous; some participants don't honor PREPARE. |
| Postgres write that is forwarded to a search index | Outbox | The index is eventually consistent by design. |
| Long-lived business workflow across three services | Saga | Each step's local commit releases resources immediately. |
| One participant cannot be undone (physical rocket) | Approval gate first | Saga's compensation model doesn't apply; do not start until human says go. |

## When sagas win

- **Long-lived.** A saga can sleep between steps. 2PC cannot;
  participants are blocked.
- **Cross-service.** No participant needs to grant the saga library
  any special verb. A saga step is just "the same HTTP/SQL call
  you'd make anyway."
- **Tunable durability.** Persist the saga state to a table and a
  worker can resume after a crash — see "durable sagas" below.
- **Failure visibility.** Each step's failure is named, logged, and
  produces a structured error (see `SagaError::StepFailed` in
  `projects/14-sagas`).

## When 2PC wins

- **You own every participant.** All Postgres. All MySQL. A
  closed cluster of databases.
- **You need real isolation.** 2PC participants hold locks; readers
  do not see partial state.
- **The operation is short.** Tens of milliseconds, not seconds.

In this codebase, this case never applies — every cross-store write
either crosses a trust boundary (Stripe) or is naturally async (the
outbox case).

## When you need neither

- **Idempotency + outbox is enough.** "I want to email the user
  after their signup row commits." That's an outbox, not a saga.
- **Read-your-writes is enough.** "I want my own writes visible on
  the next read." That's just sticky sessions to the primary, not a
  consensus protocol.
- **Single store.** "I want both these rows to commit together."
  That's a regular SQL transaction.

The temptation to reach for a saga is real because sagas feel
*sophisticated*. Resist it. The right answer for most cross-table
writes is "wrap them in a transaction." The right answer for most
cross-service writes is "outbox the side effect."

## Durable sagas (where this gets interesting)

The orchestrator in `projects/14-sagas` is in-memory: if the process
crashes between steps 2 and 3, the saga is lost and step 2 is *not*
compensated.

Production sagas persist their state:

```sql
CREATE TABLE saga_runs (
    id            BIGINT PRIMARY KEY,
    name          TEXT NOT NULL,
    state         JSONB NOT NULL,          -- the SagaContext
    cursor        INT NOT NULL DEFAULT 0,  -- which step is next
    status        TEXT NOT NULL,           -- 'running' | 'committed' | 'compensating' | 'failed'
    last_error    TEXT,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

A worker scans `saga_runs` for `running` or `compensating` rows and
resumes them. The orchestrator commits the cursor advance in the
*same* transaction as each step's local write — that's what makes
the durability story coherent. (For external steps like Stripe, you
record the result in `state` before advancing the cursor.)

Frameworks that do this for you: Temporal, Cadence, AWS Step
Functions. We do not pull them in here because the lab's goal is
"see the bones."

## A subtler point: compensation failure

Saga compensations can fail. Three options, all worse than the 2PC
"atomic across one cluster" answer:

1. **Retry with backoff.** The orchestrator does this (`CompensationPolicy`).
2. **Park.** Move the saga to a "needs human" state; page on-call.
3. **Compensate the compensation.** Sometimes the "fix" is a new
   forward action (send a "we tried to refund you, please contact
   support" email).

Honesty about this is the difference between toy sagas and
production ones. The `SagaError::StepFailed { compensations_clean }`
field in our orchestrator surfaces this: if `false`, a human must
look at it.

## The principal-engineer takeaway

- 2PC is right *only* when you own every participant and they all
  speak PREPARE. Almost never true in 2026 architectures.
- Sagas are right when participants cross a trust boundary and each
  one has a meaningful compensation.
- Outbox is right when you can make the side effect idempotent and
  don't need rollback.
- The wrong question is "which is best?" The right question is
  "what is the participant set, and what does each member of it
  support?"

## Related

- `projects/14-sagas` — the worked example.
- `projects/08-outbox-demo` — the simpler pattern.
- `apps/memberclub/api/src/billing.rs` — webhook idempotency in
  action.
- ADR 0006 (outbox over broker), ADR 0008 (Stripe is the rail).
- `01-cap-pacelc.md` — why crossing trust boundaries forces EL.
- `04-failure-modes.md` — every saga failure mode named.

## Going deeper

- *Sagas*, Garcia-Molina & Salem (1987). The original paper. Six
  pages. Older than most of its readers.
- *Pattern: Saga* on microservices.io (Chris Richardson) — the
  modern microservice framing.
- *Temporal* documentation — what a production saga framework
  actually looks like.
- *Designing Data-Intensive Applications*, chapter 7 (Transactions)
  + chapter 9 — the textbook context.
