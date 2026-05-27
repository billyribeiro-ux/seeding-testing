# B-tree, WAL, and VACUUM

This file covers three Postgres internals that share a chapter because they
are mutually entangled. The B-tree is the on-disk shape of (almost) every
index you create. The WAL is what makes index updates (and everything else)
crash-safe and replicable. VACUUM is the periodic cleanup that keeps
both from spiraling out of control.

If you have not read
[`01-mvcc-and-isolation-levels.md`](./01-mvcc-and-isolation-levels.md) yet,
read it first — this file assumes the vocabulary (tuple, xmin, xmax,
visibility) it establishes.

## What a B-tree index actually is

A B-tree is a self-balancing tree where every internal node has many
children and every leaf is at the same depth. Postgres uses a variant
called a **B+ tree**: only the leaves contain pointers to actual table
rows; the internal nodes only contain routing keys.

A picture, for a B-tree with branching factor 4 (real Postgres B-trees have
branching factors in the hundreds):

```
                       [ 50 | 100 | 150 ]                  <- root
                      /     |      |     \
              [10|20|30] [60|70|80] [110|120] [160|170|180]   <- internal
              /  |  |  \   ...
          [1..9][11..19]...                                   <- leaves
                                  each leaf has a ctid pointing into the heap
```

Key properties:

- **Sorted**: leaves are sorted by the index key, and leaves are linked left-
  to-right. Range scans (`WHERE x BETWEEN 10 AND 50`) walk leaves
  sequentially.
- **Logarithmic lookup**: depth is `log_branch(N)`. For 100M rows and a
  branching factor of ~200, depth is ~4. Four random page reads to find
  any row. This is why an index lookup is "fast" in a way a sequential
  scan can never be.
- **Branching factor depends on key size**: Postgres pages are 8KB. With
  a `BIGINT` key (8 bytes) and ~8 bytes of overhead per entry, you fit
  ~500 entries per internal page. With a wide `TEXT` key you fit far fewer,
  the tree gets taller, and lookups touch more pages. This is why "index
  the integer FK, not the email" is conventional wisdom.

### Why fan-out matters

Each level of the tree is potentially a page fault — a disk read that takes
hundreds of microseconds (SSD) or milliseconds (HDD; we know, nobody uses
HDDs anymore, but RDS used to default to magnetic). A 4-level tree on cold
cache is ~4 page reads; a 7-level tree on the same data with fatter keys is
~7 page reads. Same algorithm, almost double the latency. Branching factor
is not a tuning knob you have, but key-width *is* — a covering index on
`(tenant_id, created_at)` is faster than one on
`(tenant_id, created_at, description)` not because there's more data but
because the tree is taller per level.

### How an `INSERT` updates the index

1. Walk the tree to find the leaf where the new key belongs.
2. Insert. If the leaf has room, done.
3. If the leaf is full, **split**: half the entries stay, half go to a new
   leaf, and a new routing key is inserted into the parent.
4. If the parent is full, split it too. Cascade up. If the root splits, the
   tree gains a level.

This is why mass `INSERT`s into an indexed table benefit from dropping the
index, loading the data, and rebuilding — bulk-build is one sort, not N
incremental walks-and-maybe-splits.

### Index-only scans and the visibility map

In an MVCC world, the index alone can't tell you if a row is visible. The
index entry has a `ctid` pointing into the heap, and the heap tuple has the
`xmin`/`xmax` you need to do the visibility check. So an "index lookup" is
really two reads: index page, then heap page.

The **visibility map** is a per-table bitmap saying "every tuple on this
heap page is visible to everyone." If your index entry points to a page
marked all-visible, the planner can skip the heap fetch entirely. This is
the **index-only scan**. VACUUM maintains the visibility map. This is one
of the load-bearing reasons VACUUM exists.

### Other index types (briefly)

- **GIN** (Generalized Inverted iNdex): for arrays, JSONB containment, full
  text. "Which documents contain word X?"
- **GiST** (Generalized Search Tree): for geometric and range queries.
- **BRIN** (Block Range INdex): for very large tables where physical order
  correlates with logical order (time-series). Tiny indexes, lossy lookup.
- **Hash**: now WAL-logged and replicated as of PG10, but rarely used. The
  B-tree is almost always the right answer.

### Cross-reference: keyset pagination in `projects/03-notes-api`

[`projects/03-notes-api`](../../projects/03-notes-api/) exposes
`GET /notes?cursor=<last_id>&limit=20`. The handler issues:

```sql
SELECT id, body, created_at
FROM notes
WHERE id > $cursor
ORDER BY id
LIMIT 20;
```

This is a B-tree's best case. We descend the tree to find the first leaf
entry with `id > $cursor` (depth-many page reads), then walk the leaf chain
to the right reading 20 entries in physical order, then fetch 20 heap rows
(or skip the heap entirely if the visibility map says so). Total cost:
`O(log N + 20)`.

Contrast with the naive offset-based pagination `LIMIT 20 OFFSET 10000`,
which forces Postgres to actually read and discard the first 10,000 rows
to find the next 20. That is `O(N)` in the offset — perfectly fine on page
1, ruinous on page 500. Keyset pagination is the same logical scroll-
through-the-table without the linear penalty. The keyset cursor is the
B-tree's natural interface; offset pagination is fighting the data
structure.

## Write-Ahead Log (WAL)

### The durability problem

When you `COMMIT`, the database has to guarantee that even if power is cut
the moment after `COMMIT` returns, the data will still be there on restart.
The naive approach — flush every modified page to disk on commit — is
catastrophic for performance (random 8KB writes everywhere).

The WAL trick: before modifying any page in memory, write a tiny record to
a **sequential** append-only file describing the change. On commit, `fsync`
the WAL up to your commit record. The actual data pages can be flushed lazily
later. On recovery, replay the WAL from the last known-flushed point to
reconstruct the post-crash state.

This is "write-ahead" because the log is written ahead of (before) the data.

### Why this is a huge performance win

- Many small modifications, one sequential write.
- The WAL is on its own file(s); on RAID systems you can put it on a
  dedicated disk so it doesn't contend with heap writes.
- Group commit: many small transactions can share a single `fsync` of the
  WAL by piling up their commit records and flushing the log once.

### What's actually in the WAL

Each record describes a physical page change: "on page 4231 of table
`subscriptions`, at offset 312, insert these 84 bytes." It is not a
SQL statement. Logical decoding (used for tools like Debezium) is a
post-processor that turns these physical records back into
"row X in table T was updated from V1 to V2."

### WAL drives replication

Streaming replication is "ship the WAL bytes to the replica as they're
produced and have the replica replay them." This is why:

- Replicas are byte-identical copies, not query-replays. The same physical
  page-level changes happen on both sides.
- Replication lag is measured in WAL bytes (or seconds-equivalent).
- Logical replication is a different mechanism (slot + walsender +
  decoder) but rides on the same WAL.

### WAL drives PITR (Point-In-Time Recovery)

If you have:

- A base backup taken at time T0.
- Every WAL segment produced between T0 and now, archived somewhere.

Then you can recover to any point T in `[T0, now]` by:

1. Restore the base backup.
2. Configure Postgres to fetch WAL segments from your archive.
3. Set `recovery_target_time = '<T>'`.
4. Start Postgres. It replays WAL forward from the base backup, stops at T.

See [`../runbooks/disaster-recovery.md`](../runbooks/disaster-recovery.md)
for the operational details. This is the primary reason we maintain WAL
archiving even though we also take nightly `pg_dump`s.

### WAL operational concerns

- **`max_wal_size`** caps how much WAL can accumulate between checkpoints.
  Too small: constant checkpointing, I/O thrash. Too large: long recovery
  after crash (replay has to cover more). Defaults are fine for small/
  medium databases.
- **WAL archiving must keep up.** If `archive_command` fails repeatedly
  (S3 outage, disk full), WAL piles up in `pg_wal/` and eventually fills
  the disk, taking the database down. Monitor this.
- **Unarchived WAL is unreplayable from backup.** If you have a base backup
  from yesterday but no WAL since, you can only recover to "yesterday's
  base backup time," not point-in-time.

## VACUUM

VACUUM exists because of MVCC. Recap:

- An `UPDATE` leaves the old tuple in place with `xmax` set.
- A `DELETE` just sets `xmax`.
- A tuple is "dead" once no in-progress transaction's snapshot could see it.
- Dead tuples eat disk, eat cache, slow scans.

`VACUUM` does, roughly:

1. Walk the table.
2. For each page, find dead tuples (tuples with `xmax` < the global xmin
   horizon of any running transaction).
3. Mark their space reusable for future inserts on the same page.
4. Update the visibility map.
5. Update the free space map.
6. If the trailing pages of the table are entirely empty, truncate the
   table file.

It does NOT defragment within pages or rewrite the whole table — that's
`VACUUM FULL`, which takes an exclusive lock.

### Autovacuum

The `autovacuum` daemon runs `VACUUM` (and `ANALYZE`) automatically when
tables exceed thresholds:

- `autovacuum_vacuum_threshold` (default 50 rows) plus
- `autovacuum_vacuum_scale_factor` (default 0.2 = 20% of table) dead
  tuples means: trigger VACUUM.

On a 10M-row table, that's 50 + 2M dead tuples before autovacuum kicks in.
On a hot table this is too lax; you tune the scale factor down per-table:

```sql
ALTER TABLE outbox SET (
    autovacuum_vacuum_scale_factor = 0.02,
    autovacuum_analyze_scale_factor = 0.01
);
```

### ANALYZE

`ANALYZE` updates the statistics the planner uses to estimate query costs
(row counts, value distributions, correlation between physical and logical
order). Without recent stats, the planner picks wrong plans. Autovacuum
runs `ANALYZE` based on similar thresholds. After a big bulk-load, run
`ANALYZE` manually before opening for traffic.

### Transaction-ID wraparound

This is the operational concern that has killed companies. Read carefully.

- Transaction IDs (`xid`) are 32-bit unsigned integers. There are ~4B
  available.
- Postgres uses the high bit for "wraparound" arithmetic: XIDs are compared
  modulo 2^32, so XIDs that are more than ~2B apart are "in the past" or
  "in the future" relative to current.
- Every committed tuple has an `xmin` that must be considered visible by
  current and future transactions. If an old `xmin` becomes more than ~2B
  XIDs in the past, the comparison logic breaks and tuples *that are
  visible to everyone* would suddenly look invisible.

Postgres prevents this by **freezing**: VACUUM, when it sees a tuple that's
older than `vacuum_freeze_min_age`, rewrites the `xmin` to a special
sentinel meaning "always visible." Freezing is what eats CPU during the
"anti-wraparound VACUUM" panic mode that you'll occasionally see in logs.

If a table doesn't get vacuumed for long enough — because autovacuum is
disabled, or starved, or blocked by a long transaction — its oldest `xmin`
approaches the wraparound horizon. At about 200M transactions remaining,
Postgres starts a forced anti-wraparound VACUUM. At about 1M remaining, it
refuses to allocate new XIDs and goes read-only with a stark error message.
At zero, you have a manual single-user-mode recovery on your hands.

How to watch for this:

```sql
SELECT datname, age(datfrozenxid)
FROM pg_database
ORDER BY age(datfrozenxid) DESC;
```

`age` returns "how many XIDs ago" the oldest unfrozen XID in that database
is. If this is climbing past, say, 500M, alert. Past 1B, page someone.
Past 1.5B, you have hours.

The most common cause is a single long-running transaction (often an
"idle in transaction" connection from a misbehaving worker) holding the
horizon. Find with:

```sql
SELECT pid, age(backend_xid), state, query
FROM pg_stat_activity
WHERE state IN ('idle in transaction', 'idle in transaction (aborted)')
ORDER BY age(backend_xid) DESC;
```

`pg_terminate_backend(pid)` is the immediate mitigation. Then fix the
client.

### VACUUM operational symptoms

- **Autovacuum is "always running" on table X.** Probably fine. It's
  working. Worry when it's *never* running on a hot table.
- **`VACUUM` blocked.** It takes a `SHARE UPDATE EXCLUSIVE` lock, which
  conflicts only with DDL and other VACUUMs. A migration that holds an
  `ACCESS EXCLUSIVE` lock will block autovacuum and stall the wraparound
  defense.
- **Bloat reported by `pgstattuple` is high.** Autovacuum is keeping up
  with dead-tuple cleanup but not reclaiming the disk space (because
  trailing-page truncation requires the trailing pages to be entirely
  empty). For a permanently-bloated table, schedule `pg_repack` or
  `VACUUM FULL` during a maintenance window.

## Putting it together

You insert a row into `notes`:

1. The B-tree on `notes(id)` walks down to find the leaf where the new
   `id` belongs. Possibly splits the leaf (and possibly the parent, etc.).
   Each modified page is described in a WAL record.
2. The new heap tuple is written to a free slot on some heap page. Another
   WAL record.
3. The WAL records are written to the WAL buffer in shared memory.
4. On `COMMIT`, the WAL is `fsync`'d to disk up to and including a commit
   record. Postgres returns success to your client.
5. The modified heap and index pages may still only be dirty in memory.
   They get flushed at the next checkpoint.

You query that note back via `WHERE id > $cursor LIMIT 20`:

1. Plan: index scan on `notes(id)`. Estimated cost based on most-recent
   ANALYZE stats.
2. Walk the B-tree to find the first leaf with `id > $cursor`.
3. Read leaves sequentially, fetching 20 entries.
4. For each entry, check the visibility map. If the heap page is all-
   visible, skip the heap fetch. Otherwise read the heap page, check
   `xmin`/`xmax` against your snapshot.
5. Return rows.

Some time later VACUUM runs:

1. Finds tuples whose `xmax < global_xmin_horizon` — these are dead.
2. Marks their slots reusable.
3. Updates the visibility map to mark pages all-visible where possible.
4. Updates the free space map.
5. If autovacuum ran ANALYZE in the same pass, refreshes statistics.

The whole system is interlocking. Skipping any one of the three pieces
(B-trees for indexed lookup, WAL for durability, VACUUM for hygiene)
makes the other two fail.
