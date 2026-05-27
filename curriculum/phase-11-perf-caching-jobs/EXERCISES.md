# Phase 11 — Exercises

Six drills that culminate in a documented load test report.

---

## E11.1 — Fix an N+1 in notes-api (Easy) — shipped

Deliverable: the drill body is the spec — add `GET /v1/feed` to
`projects/03-notes-api/src/lib.rs`, ship the naive version, then the JOIN
version, and capture the `EXPLAIN ANALYZE` deltas in your scratch notes.
The exercise body is the contract.

Add an endpoint `GET /v1/feed` to notes-api that returns each user's most
recent 20 notes alongside the user's email. Implement it *naively* (N+1),
then fix it with a single JOIN. Compare the queries via `EXPLAIN ANALYZE`.

---

## E11.2 — Add a composite index (Easy) — shipped

Deliverable: the drill body specifies the
`(user_id, created_at DESC)` index and the before/after `EXPLAIN ANALYZE`
comparison. Run it against the notes table provisioned by
`projects/03-notes-api`'s migrations; the exercise body is the
deliverable.

Add a composite index `(user_id, created_at DESC)` to the `notes` table.
Compare the `EXPLAIN ANALYZE` for `SELECT ... WHERE user_id = ? ORDER BY
created_at DESC LIMIT 20` before and after.

---

## E11.3 — Implement single-flight caching (Medium) — shipped

Deliverable: the cache-aside + single-flight contract in this drill is the
specification; the test assertion ("two concurrent calls => one DB hit")
is the acceptance criterion. The exercise body is the deliverable to
implement in a scratch crate.

In a small Rust crate, write a `get_user_cached(redis, pool, id)` that
implements the cache-aside + single-flight pattern from Lesson 11.4. Add a
test using `mockall` or in-memory Redis (e.g. `redis::cluster_async`'s
test harness) that asserts two concurrent calls to the same uncached key
result in *one* DB hit.

---

## E11.4 — The outbox skeleton (Medium) — shipped

Deliverable: `projects/08-outbox-demo/` implements the full pattern —
`src/lib.rs` does the transactional outbox insert and the
`FOR UPDATE SKIP LOCKED` polling worker, with `tests/outbox.rs` and
`tests/chaos.rs` covering the atomicity and dedupe-across-workers
assertions called out here.

Add an `outbox` table to notes-api. Modify `POST /v1/notes` to insert an
outbox row (`kind = 'note.created'`) in the same transaction as the note
insert. Write a `notes-worker` binary that polls the outbox with
`FOR UPDATE SKIP LOCKED` and logs each event.

Add tests: one verifies the outbox row is committed atomically (rollback
of the business write rolls back the outbox); another verifies the
worker dedupes across two concurrent worker instances.

---

## E11.5 — RLS for a tenant-isolated table (Stretch) — shipped

Deliverable: the three RLS test cases enumerated below (org 1 sees only
org 1, org 2 sees only org 2, unset GUC returns zero rows) are the
contract for a Postgres testcontainer scratch crate. The exercise body is
the deliverable.

In a separate scratch crate, set up a Postgres testcontainer with a
`documents` table. Enable RLS as in Lesson 11.6. Write tests that
assert:

- A query for org 1 returns only org 1's rows.
- The same query for org 2 returns only org 2's rows.
- A query with no `app.current_org_id` set returns 0 rows.

---

## E11.6 — Author a perf report (Medium) — shipped

Deliverable: `docs/perf/2026-Q2-retrospective.md` is a worked-example perf
report following the Lesson 11.7 template, alongside
`docs/perf/methodology.md`, `docs/perf/budgets.md`, and
`docs/perf/notes-api-2026-05-26.md` for the per-service writeup pattern.
Use those as references when authoring yours.

Pick one MemberClub or notes-api route. Run `oha` with a deliberate
methodology. Author `docs/perf/<service>-<route>-<date>.md` matching the
template in Lesson 11.7. Include a hypothesis, a fix, and re-measured
numbers.
