# Distributed-Systems Failure Modes

> *"It's working" is a state your distributed system passes through
> on the way to something interesting.*

A catalog of the failure modes you have to design for, with one
sentence each on what they do to the saga lab in `projects/14-sagas`.
None of these are exotic; all of them have happened to MemberClub-
flavored services in real postmortems.

## How to use this document

In a design review, walk down the list and ask "what does the system
do when **this** happens?" If you cannot answer for a failure mode,
that is your next design task.

The saga orchestrator (`projects/14-sagas/src/lib.rs`) is referenced
throughout. The points generalize to any cross-service workflow.

---

## 1. Split-brain

**What it is.** A network partition leaves two halves of a cluster
each believing it is the primary. Both accept writes; the writes
diverge; reconciliation later is impossible to do without data loss.

**Where it bites.** Any leader-replica system without proper quorum
(Redis Sentinel, naive Postgres failover). Raft (see
`02-consensus.md`) is *designed* to prevent this — the minority
side has no quorum and refuses writes.

**Detection.** Monitor "who thinks they are primary." Two answers at
once is a sev1.

**Mitigation.** Use a coordination service with proper quorum
(etcd / Consul). Fence the loser: STONITH ("shoot the other node in
the head") or a generation token that the storage layer enforces.

**What happens to the saga lab if this happens.** If our saga
orchestrator state were stored in a non-quorum-protected store, two
orchestrators could try to compensate the same saga concurrently,
leading to double-refund. Our compensations are idempotent
(`ChargeCard::compensate` is a no-op if already refunded), which
defends against this, but the right answer is to keep the saga state
in a store with proper quorum, or to use a unique cursor / lease per
saga run.

---

## 2. Clock skew

**What it is.** Two machines disagree about the current time. NTP
typically keeps the skew under ~100 ms in a well-run datacenter;
the public internet routinely sees seconds; misconfigured VMs have
been seen at *days*.

**Where it bites.**
- TTL-based caches return "fresh" data that was actually expired on
  the writer.
- Token validity checks reject good tokens (`iat > now`).
- "Last write wins" reconciliation picks the wrong write.
- Distributed-rate-limit windows over- or under-count.

**Detection.** Compare `now()` across nodes in your tracing data;
alert on > 100 ms drift.

**Mitigation.**
- Do not use wall-clock time for ordering distributed events. Use
  monotonic clocks for durations; use logical clocks (Lamport,
  vector clocks) or version vectors for ordering.
- Time-based caches should use a generation number when accuracy
  matters more than freshness.
- Use TrueTime-like APIs (Spanner) when you have them; otherwise,
  treat any cross-node timestamp as ±100 ms accurate at best.

**What happens to the saga lab if this happens.** Our saga
orchestrator uses `tokio::time::sleep` only for compensation backoff
(monotonic — safe). It does not order events by wall clock. But: if
a step writes a row with `updated_at = NOW()` on one node and the
compensation reads it on another with a skewed clock, an "is this
row still ours to undo?" check could be wrong. Use generation
numbers or saga run ids, not timestamps, for that check.

---

## 3. Network partitions

**What it is.** Two halves of the network cannot reach each other.
Each half is internally healthy. Lasts for seconds, minutes, hours.
"Big" partitions are dramatic; "small" ones (one rack from another,
one AZ from another) are the dangerous ones — they look healthy
from inside.

**Where it bites.**
- 2PC participants left holding locks forever.
- Replication lag exploding without alerts (replicas look "up" but
  cannot pull WAL).
- Health checks return 200 from a node that cannot reach the DB.

**Detection.** End-to-end synthetic checks ("can the API actually
talk to the DB?"), not just per-node liveness probes.

**Mitigation.**
- Bounded retries with backoff, then escalate.
- Circuit breakers: stop calling a partitioned dependency, return
  a graceful degradation.
- Quorum-based protocols (see `02-consensus.md`) for anything
  needing agreement.

**What happens to the saga lab if this happens.** A partition
between our orchestrator and Stripe means `ChargeCard::execute`
returns a transient error. The saga reports `SagaError::StepFailed`
at step 0 with `compensations_run == 0` — clean failure, nothing to
roll back. A partition *after* the charge committed (but the
response was lost) is worse: we think the step failed and we will
compensate, leading to a refund of a charge the user paid. Idempotency
keys (`ChargeCard.idempotency_key`) defend against the *retry* side
of this. Defending against the *compensation* side requires reading
back from Stripe before refunding ("does the charge exist?") — left
as an exercise; the lab orchestrator does not do this.

---

## 4. Slow vs failed

**What it is.** A dependency is *slow* but not *down*. It returns
200s, just at p99 = 30 s instead of p99 = 100 ms. To upstream
callers it looks like everything works; downstream, queues are
filling up and customer-facing latency is climbing.

**Where it bites.**
- Thread / connection-pool exhaustion. A handler that normally takes
  10 ms now occupies a worker for 30 s; you run out of workers; new
  requests pile up; the whole service tips over.
- Cascading timeouts: A → B → C, each with a 5 s timeout; B and C
  both succeed at 4.9 s; A's overall budget is blown.
- Retries amplifying load: a slow dependency gets retried, the
  retry adds load, things get slower, more retries, you've
  built a positive-feedback explosion.

**Detection.** Track p99 latency per dependency, not just error
rate. The shape of "is it slow" is "p50 unchanged, p99 way up."

**Mitigation.**
- Timeouts on every cross-service call. The right value is
  "smaller than my caller's timeout."
- Bulkheads: separate connection pools per dependency.
- Load shedding: when overloaded, return 503 fast rather than
  queuing.
- Hedged requests for read paths only; never for writes.

**What happens to the saga lab if this happens.** If Stripe is slow
but eventually succeeds, `ChargeCard::execute` blocks; the saga
takes longer; if we have a per-saga timeout (we don't, but in
production you would), the saga returns a transient failure mid-
step. The compensation needs to handle "the operation may or may
not have happened on the other side" — which means *idempotent*
compensations and an optional read-back to verify. The lab models
this with the `ChargeCard::execute` injected failure but not with
slow responses; a production version would add a `tokio::time::timeout`
wrapper around every step.

---

## 5. Byzantine failures

**What it is.** A node returns *wrong* answers, not just *no*
answers. Memory corruption, malicious actor, buggy SDK, mismatched
schema between versions. The hardest class of failure: you cannot
trust the data you do receive.

**Where it bites.**
- Storage corruption (silent disk errors).
- A buggy upgrade producing wrong but plausible writes.
- Untrusted clients in multi-tenant systems.

**Detection.**
- End-to-end checksums (database page checksums, message HMACs).
- Cross-replica consistency checks ("do all three replicas have the
  same row?").
- For untrusted clients, server-side validation of every invariant.

**Mitigation.**
- Postgres page checksums; ZFS / btrfs for storage; ECC memory.
- ADR 0007 RLS (Postgres row-level security): the *DB* enforces
  tenant isolation, so a compromised app server cannot forge a
  tenant id.
- For truly hostile environments (public blockchains, untrusted
  consensus), use BFT consensus (PBFT, Tendermint). For our
  case — internal services on private network — assume crash-fault,
  not byzantine.

**What happens to the saga lab if this happens.** The lab assumes
each step's success / failure is honestly reported. A byzantine
participant could lie ("I committed!" when it didn't, or vice
versa) and the saga would silently corrupt state. The standard
defense is *cross-check via read-back* — but doing it on every
step is expensive. Defend at the boundaries: validate Stripe
signatures (HMAC) on webhook receipts; validate JWT signatures on
every request; treat anything beyond a trusted internal call as
hostile.

---

## 6. Gray failure

**What it is.** The failure that the system *itself* does not see.
A node is healthy according to its own probe but unreachable by its
peers, or returns 200s that contain wrong data, or is dropping 30%
of packets but ICMP still works.

**Where it bites.**
- Load balancers that probe locally and never observe the upstream
  view.
- Health checks that pass while the request path is broken.
- Replication that is "up" but lagging by hours.

**Detection.**
- End-to-end synthetic probes (from outside the cluster, hit the
  real API, assert the real result).
- Disagreement detection: do clients see what the server thinks they
  see? Compare client-side logs to server-side logs.

**Mitigation.**
- "External view" health checks (a Lambda outside your VPC hitting
  your real endpoints, alerting if it differs from internal).
- Reconciliation jobs: every night, replay the canonical store
  against derived stores and alert on divergence.
- For sagas specifically: a periodic job that lists sagas in
  `running` state for > N minutes and pages on-call.

**What happens to the saga lab if this happens.** A gray failure in
the saga orchestrator process itself (it's stuck somewhere between
steps) looks like a "running" saga that never completes. We have
no rescuer in the lab — an out-of-process worker would be needed,
and `03-sagas-vs-2pc.md` sketches the `saga_runs` table that makes
this possible. Without it, the saga is lost on crash; with it, a
new worker picks up where the old left off.

---

## Bonus: the failures that don't have a Wikipedia page

- **Misconfiguration.** The single largest source of outages. A
  one-character typo in a config push. Defenders: typed configs,
  schema validation, canary deploys, audit logs.
- **Cardinality explosion.** A metric label that takes the user-id
  as a value, blowing up Prometheus storage. Defender: limit
  high-cardinality labels at the metrics layer.
- **Resource exhaustion under load.** Connection pools, file
  descriptors, threads, memory. Always cap before the OS does.
- **Deploy-time skew.** Two versions of a service serving traffic
  simultaneously, with incompatible expectations of the schema
  or of each other. Defender: backward-compatible migrations,
  feature flags, single-version invariants in protocol.

These do not have Greek-letter names because they are everywhere.
They cause more incidents than every glamorous failure mode
combined.

---

## How this maps to the curriculum

- **Phase 8** introduces idempotency keys for Stripe webhooks — the
  defense against slow-vs-failed and network partition.
- **Phase 10** introduces tracing — the only way to *see* most of
  these failures in production.
- **Phase 11** introduces caching, where clock skew and gray
  failure show up; rate limiting, where slow vs failed shows up
  most often.
- **Phase 12** asks you to write a postmortem; pick any of the
  above and the template at
  `docs/03-postmortems/0000-TEMPLATE.md` writes itself.

## The principal-engineer takeaway

- Distributed-systems failures are *not* exotic. They are the
  default state.
- Per failure mode, you should be able to point at *one specific
  defense* in the codebase. If you can't, that is the next task.
- "Eventual consistency" is not a free pass. It is a *budget* you
  spend on latency-versus-correctness; spend it on dashboards, not
  on money.
- The saga orchestrator in `projects/14-sagas` defends against
  some of these (idempotent compensations; retry with backoff;
  clear failure reporting); leaves others as exercises (durable
  state for split-brain / gray failure; read-back for byzantine;
  per-step timeouts for slow-vs-failed).

## Related

- `01-cap-pacelc.md` — the trade-off most of these failures force.
- `02-consensus.md` — split-brain's specific defender.
- `03-sagas-vs-2pc.md` — the cross-service answer when none of the
  classical guarantees survive contact with reality.
- `projects/14-sagas` — the worked example for compensation under
  failure.
- `apps/memberclub/api/src/billing.rs` — idempotency keys, replay
  windows.
- `docs/runbooks/` — the per-incident response playbooks.

## Going deeper

- *Fallacies of Distributed Computing* (Peter Deutsch, 1994). The
  classic. Eight assumptions every junior engineer makes that turn
  out to be wrong.
- *The Tail at Scale*, Dean & Barroso (2013). Why slow-vs-failed is
  the failure mode that scales worst.
- *Gray Failure: The Achilles' Heel of Cloud-Scale Systems*, Huang
  et al (2017). The paper that named the pattern.
- *Jepsen* reports (jepsen.io). What real partitions do to real
  databases. Pick three at random.
- *Postmortems from Google's SRE Book*, especially the YouTube
  one. The "slow vs failed" case study.
