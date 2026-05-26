# projects/08-outbox-demo

The outbox pattern in working Rust, 200 lines. Phase 11.5 describes it
in depth; this is the implementation you study and copy.

## What it teaches

- **Atomic business write + side-effect intent.** Both rows commit
  inside one transaction or neither does.
- **Worker lifecycle.** `claim_next` → dispatch → `mark_done` or
  `mark_failed`.
- **Exponential backoff.** `2^attempts` seconds, capped at 256 s.
- **Capped retries.** After `max_attempts`, status moves to `failed`;
  no more retries.
- **Stats.** Four-column status summary for dashboards.
- **DB-layer constraints.** A `CHECK` on `amount_cents` enforces the
  $21B ceiling even if application validation is bypassed.

## Run it

```bash
# Record three transfers (writes business + outbox atomically each time)
cargo run -p outbox-demo -- record --from alice  --to bob   --amount-cents 1000
cargo run -p outbox-demo -- record --from carol  --to dave  --amount-cents 2500
cargo run -p outbox-demo -- record --from eve    --to frank --amount-cents 9999

# Check the queue
cargo run -p outbox-demo -- stats
#   pending=3 processing=0 done=0 failed=0

# Drain it with the worker (ctrl-C to stop)
cargo run -p outbox-demo -- worker
#   processed outbox#    1 transfer.recorded : {"amount_cents":1000,...}
#   processed outbox#    2 transfer.recorded : {"amount_cents":2500,...}
#   processed outbox#    3 transfer.recorded : {"amount_cents":9999,...}

cargo run -p outbox-demo -- stats
#   pending=0 processing=0 done=3 failed=0
```

The `--database-url` flag accepts any SQLite URL; default is a local
file (`./outbox-demo.sqlite?mode=rwc`) so state persists across runs.

## Test it

```bash
cargo test -p outbox-demo
cargo clippy -p outbox-demo -- -D warnings
```

10 integration tests cover atomicity, validation, claim ordering,
backoff math, success/failure marking, retry caps, disjoint claims,
and the worker loop's idle behavior.

## File map

| File | Purpose |
|---|---|
| `migrations/...init.sql` | `transfers` + `outbox` tables with the right indexes |
| `src/lib.rs` | `record_transfer`, `claim_next`, `mark_done`, `mark_failed`, `run_once`, `run_loop`, `stats`, `backoff_for_attempt`, the `Dispatcher` trait |
| `src/main.rs` | CLI: `record`, `worker`, `stats` |
| `tests/outbox.rs` | 10 integration tests |

## How this differs from Postgres production

In production we use Postgres with `FOR UPDATE SKIP LOCKED` on the
claim query so many workers process *different* rows in parallel:

```sql
UPDATE outbox SET status = 'processing', attempts = attempts + 1
WHERE id = (
    SELECT id FROM outbox
    WHERE status = 'pending' AND next_attempt_at <= NOW()
    ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1
)
RETURNING *;
```

SQLite serializes writes, so the same query without `FOR UPDATE SKIP
LOCKED` is already single-flight. The API shape (this project's
`claim_next`) is identical; swapping the SQL is the entire migration.

## Related

- ADR 0006 — Outbox pattern over external broker
- Phase 11.5 — Background Jobs and the Outbox Pattern
- `docs/runbooks/stripe-webhook-lag.md` — what to do when the queue
  grows
