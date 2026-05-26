# Phase 5 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Pyramid awareness** | All-unit or all-integration | Mix appropriate to code shape | Picks per-function based on the trophy; static analysis maximized |
| **Integration tests** | None, or mocked DB | Real DB (testcontainers / in-memory) | Hermetic per-test; tests both happy paths and error paths |
| **Snapshot discipline** | Snapshots everything → noise | Snapshots wire-format shapes only | Redacts ephemeral fields; reviews diffs before accepting |
| **Property tests** | None | Used for money/parsing/encoding | Designs invariants up front; shrinks failures aggressively |
| **Factories / fixtures** | Hand-rolled inserts per test | Factory functions with overrides | Fixtures compose factories; named scenes ("signed-in admin") |
| **Seeding** | None or hand-edited dump | Three profiles, idempotent | Safety guards (prod refusal), batching, audit-logged, snapshot-tested demo |
| **Coverage gate** | None | Reports + reviews | CI fails under threshold; per-crate gates; PR-over-PR drift watched |
| **Test speed** | Multi-minute test runs | Sub-30s for the unit/integration mix | Profiles slow tests; partitions for matrix CI |
| **Flakiness response** | Adds retries everywhere | Retries are tagged and tracked | Treats flakiness as a bug; uses `tokio::time::pause`, deterministic seeds |

## Self-check before moving to Phase 6

- [ ] `make verify` passes locally + CI green.
- [ ] You completed Exercises E5.1 – E5.6.
- [ ] Coverage gate at 80% (or higher per crate) is enforced.
- [ ] At least one snapshot test, one property test, one factory.
- [ ] Seed CLI works for all three profiles.
- [ ] You can articulate why mocking the DB is worse than in-memory SQLite for our use case.

Phase 6 — Auth — assumes this discipline. The auth surface area is much bigger; the test investment pays off immediately.
