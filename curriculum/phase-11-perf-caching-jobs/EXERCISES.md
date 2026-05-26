# Phase 11 — Exercises

Six drills that culminate in a documented load test report.

---

## E11.1 — Fix an N+1 in notes-api (Easy)

Add an endpoint `GET /v1/feed` to notes-api that returns each user's most
recent 20 notes alongside the user's email. Implement it *naively* (N+1),
then fix it with a single JOIN. Compare the queries via `EXPLAIN ANALYZE`.

---

## E11.2 — Add a composite index (Easy)

Add a composite index `(user_id, created_at DESC)` to the `notes` table.
Compare the `EXPLAIN ANALYZE` for `SELECT ... WHERE user_id = ? ORDER BY
created_at DESC LIMIT 20` before and after.

---

## E11.3 — Implement single-flight caching (Medium)

In a small Rust crate, write a `get_user_cached(redis, pool, id)` that
implements the cache-aside + single-flight pattern from Lesson 11.4. Add a
test using `mockall` or in-memory Redis (e.g. `redis::cluster_async`'s
test harness) that asserts two concurrent calls to the same uncached key
result in *one* DB hit.

---

## E11.4 — The outbox skeleton (Medium)

Add an `outbox` table to notes-api. Modify `POST /v1/notes` to insert an
outbox row (`kind = 'note.created'`) in the same transaction as the note
insert. Write a `notes-worker` binary that polls the outbox with
`FOR UPDATE SKIP LOCKED` and logs each event.

Add tests: one verifies the outbox row is committed atomically (rollback
of the business write rolls back the outbox); another verifies the
worker dedupes across two concurrent worker instances.

---

## E11.5 — RLS for a tenant-isolated table (Stretch)

In a separate scratch crate, set up a Postgres testcontainer with a
`documents` table. Enable RLS as in Lesson 11.6. Write tests that
assert:

- A query for org 1 returns only org 1's rows.
- The same query for org 2 returns only org 2's rows.
- A query with no `app.current_org_id` set returns 0 rows.

---

## E11.6 — Author a perf report (Medium)

Pick one MemberClub or notes-api route. Run `oha` with a deliberate
methodology. Author `docs/perf/<service>-<route>-<date>.md` matching the
template in Lesson 11.7. Include a hypothesis, a fix, and re-measured
numbers.
