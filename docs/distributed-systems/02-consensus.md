# Consensus and Raft

> *Consensus is how N machines agree on the order of events. Raft is
> the consensus algorithm you can sketch on a whiteboard in 90
> seconds and an interviewer will believe you understand.*

This essay covers Raft at the level a senior engineer should be able
to *discuss* in a design interview. Not at the level you need to
*implement*. If you find yourself implementing Raft in production
code, reread this essay and use etcd instead.

## The problem

You have N machines. They want to agree on the next entry in a log
(equivalently: the next state transition of a replicated state
machine). The network may drop messages, deliver them out of order,
or partition some nodes off entirely. Machines may crash.

Consensus is solved iff a majority (a *quorum*) of machines can hear
each other.

**Why a majority?** If you required only a plurality, two sub-groups
could each commit their own conflicting decisions during a partition
("split brain"). Majorities can't overlap two ways, so the algorithm
stays safe.

## Raft in one screen

Raft is three sub-problems, taught one at a time:

1. **Leader election.** At any moment, at most one *leader* exists.
   The leader is the only node that may append to the log.
2. **Log replication.** The leader receives client writes,
   replicates each entry to followers, and *commits* an entry once a
   majority have stored it.
3. **Safety.** A committed entry is never lost, even across leader
   changes.

```
                    ┌──────────────┐
       writes ─────▶│   LEADER     │
                    │ (1 at a time)│
                    └───┬───┬───┬──┘
              AppendEntries (heartbeat + log)
                        │   │   │
                    ┌───▼┐ ┌▼──┐ ┌▼──┐
                    │ F  │ │ F │ │ F │
                    │    │ │   │ │   │
                    └────┘ └───┘ └───┘
                     followers (mirror the log)
```

The leader's job: receive a write, append it to its local log, send
`AppendEntries` to every follower, wait until a *quorum* (including
itself) has the entry, then **commit** it (apply to state machine,
return to client).

## Sub-problem 1: leader election

Every node is in one of three roles: `Follower`, `Candidate`,
`Leader`. The clock that drives elections is a randomized timeout
(~150-300ms in etcd).

```
   start as Follower
        │
        │ no heartbeat from leader for `election_timeout`
        ▼
    Candidate
        │  vote for self; ask everyone else for their vote
        │
        ├── received majority votes  ──▶ Leader
        │
        ├── heard from a higher-term leader ──▶ Follower
        │
        └── timeout again ──▶ new election (next term)
```

Each election starts a new *term* — a monotonic integer. Term acts
as a logical clock: any message carrying a stale term is rejected;
any node that sees a higher term becomes a follower.

The randomized timeout breaks symmetry: in a tie, both candidates
time out at different times and one re-runs.

## Sub-problem 2: log replication

Once elected, the leader sends `AppendEntries` RPCs to followers.
Each RPC carries:

- The leader's `term`.
- `prevLogIndex`, `prevLogTerm` — the last entry the follower
  should already have. Used to detect divergence.
- The new `entries` to append (zero or more).
- The `leaderCommit` index.

A follower accepts an `AppendEntries` only if its log already
matches at `prevLogIndex`. If it doesn't, it rejects, the leader
decrements `prevLogIndex`, and retries. This walks back to the last
matching entry and overwrites everything after.

An entry is **committed** once it has been stored on a majority of
nodes. The leader then bumps `commitIndex`; on the next heartbeat,
followers learn about it and apply the entry to their state
machines.

## Sub-problem 3: safety

The key safety property is *State Machine Safety*: if two nodes have
applied an entry at index `i`, the entries are identical.

Raft guarantees this with two rules:

1. **Election restriction.** A node will not vote for a candidate
   whose log is "less up-to-date" than its own ("up-to-date" =
   higher `lastTerm`, or same `lastTerm` and longer log). Result: a
   candidate that wins an election holds every committed entry.
2. **Commit rule for previous terms.** A leader never commits an
   entry from a previous term by counting replicas alone; it must
   first commit one entry from its *own* term, which transitively
   carries the older ones forward.

These two rules together prove that a committed entry survives any
leader change — the new leader has it before it can be elected.

## What about availability?

Raft is CP in the PACELC sense (see `01-cap-pacelc.md`). When a
partition isolates the leader from a majority:

- The minority partition (including the old leader) cannot commit
  new entries — every commit needs a majority. The leader steps
  down on the next election timeout.
- The majority partition holds a new election; whichever node wins
  has the most up-to-date log (by the election restriction) and
  becomes the new leader.
- New writes go through the new leader. The old partition catches
  up when the network heals.

**Practical consequence:** with N=3 you tolerate one failure. With
N=5 you tolerate two. Even N is a category error: it gives you no
extra fault tolerance but doubles your latency tail and adds tie
risk. Always odd.

## Concrete prior art

You will never write Raft yourself. The good implementations are:

| System | Language | What it is | Where you've seen it |
| --- | --- | --- | --- |
| **etcd** | Go | Key-value store. The Raft library is also reused by many other projects. | Kubernetes' control plane stores everything in etcd. |
| **Consul** | Go | Service discovery, KV, ACL. Raft for the cluster state. | HashiCorp ecosystem; on-prem service mesh. |
| **Zookeeper** | Java | The granddaddy. Uses ZAB (not Raft) — same family. | Kafka used to depend on it; Hadoop/HBase still does. |
| **Apache Ratis** | Java | A Raft library reused by Ozone, Iceberg's catalog, others. | Hadoop ecosystem. |
| **RaftStore (TiKV)** | Rust | The Raft layer of TiDB. | Multi-region SQL. |
| **CockroachDB** | Go | Per-range Raft groups for sharded SQL. | Distributed Postgres-API DB. |

All of these are PC under partition (no progress without a quorum).
That is what you want from a coordination service: it would be
strictly wrong if it served stale or contradictory answers during a
split.

## Where this affects MemberClub

MemberClub's stack does not run Raft directly. We do, however, depend
on it transitively:

- **Postgres** is single-leader with optional streaming replication.
  It uses a *much* simpler protocol than Raft (the primary writes
  WAL, replicas pull). If we ever ran HA Postgres via Patroni, the
  cluster leader election runs in etcd — that is Raft underneath.
- **Redis Sentinel** (if we ever ran it) is a quorum-style leader
  election but *not* CP for writes — Redis primary failover loses
  data by design. We use Redis for caches and rate limits, where
  this is acceptable. See `01-cap-pacelc.md`.
- **Service discovery** (if we deployed to multiple regions) would
  use Consul or etcd. Raft is doing the work even when you're not
  thinking about it.

The principal-engineer move: *do not build the consensus layer
yourself*. Pick etcd or Consul; let them be the truth for things
that need true agreement (leader of a job-runner cluster, sharding
table, feature flag store); keep that data set small (megabytes,
not gigabytes) so Raft can keep up.

## What Raft is **not**

- **Not a database.** It's a log. You build the database on top: the
  state machine applies the log.
- **Not a fix for byzantine faults.** Raft assumes nodes crash but
  do not lie. A malicious node breaks safety. Real byzantine
  consensus (PBFT, Tendermint) is its own animal and you should not
  reach for it unless you are building a blockchain.
- **Not free.** Every committed entry costs an extra network round
  trip to a majority. Designs that "just put it in etcd" sometimes
  collapse under load because every config change becomes a
  consensus round.
- **Not the only consensus algorithm.** Paxos is older, more
  general, and harder to reason about. Multi-Paxos is what Raft is
  trying to make explainable. ZAB (Zookeeper) is a third cousin.

## Design-interview cheat sheet

If you have to explain Raft in a 30-second whiteboard:

1. "Three roles: follower, candidate, leader. Always at most one
   leader per term."
2. "Election by randomized timeout; majority of votes wins; term is
   a logical clock."
3. "Leader replicates AppendEntries to followers; commits when a
   majority has each entry."
4. "Safety from the election restriction: a candidate must have all
   committed entries to win."
5. "Use etcd / Consul. Do not implement this yourself."

If you can also say "PC under partition, no quorum no progress, N
must be odd, three tolerates one failure, five tolerates two," you
have said enough.

## The principal-engineer takeaway

- Consensus is a small primitive used in surprisingly few places.
  Most distributed systems are built *around* it, not *with* it.
- The strongest consistency model you can offer your users is no
  stronger than the consensus layer underneath. If your config
  store is Raft-backed, your config changes are linearizable. If
  it's Redis, they are not.
- Quorum arithmetic is non-negotiable. N=2 is worse than N=1 (zero
  fault tolerance, double the latency); N=4 is no better than N=3.
  Always odd.
- The principal move is *picking* the right pre-built consensus
  store, not *building* one. Match the data shape: keys → etcd; KV
  + ACLs → Consul; legacy + Java → Zookeeper.

## Related

- `01-cap-pacelc.md` — Raft is the canonical PC/EC system.
- `03-sagas-vs-2pc.md` — when you cross trust boundaries, consensus
  does not help; you need sagas.
- `04-failure-modes.md` — split-brain, the failure consensus
  prevents.

## Going deeper

- *In Search of an Understandable Consensus Algorithm* — Ongaro &
  Ousterhout (2014). The original Raft paper. 18 pages. Readable
  on a long flight.
- *Raft visualization* at raft.github.io. Watch one round in real
  time.
- *Designing Data-Intensive Applications*, chapter 9. The textbook
  treatment in context.
- *Paxos Made Simple*, Lamport (2001). The contrast that makes you
  appreciate Raft.
