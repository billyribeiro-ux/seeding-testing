# MemberClub — db/

This directory documents the database layer for the MemberClub capstone.
**The actual migrations live next to the code that owns them**, not here —
the rule from `docs/01-architecture-decisions/0002-orm-stance.md` is that
each crate owns its own migrations and `sqlx::migrate!()` macro.

## Where to find things

| What | Where |
|---|---|
| Schema migrations | `apps/memberclub/api/migrations/*.sql` |
| Seed CLI | `cargo run -p notes-api --bin notes-seed -- --profile dev` (template; the capstone replicates the pattern) |
| ADR on the ORM stance | `docs/01-architecture-decisions/0002-orm-stance.md` |
| Postgres + Redis dev services | `compose.yaml` at the repo root |
| RLS pattern | `docs/01-architecture-decisions/0007-postgres-row-level-security.md` |

## Running

```bash
# Bring up Postgres + Redis (root-level compose).
make up

# Apply pending migrations against the live db (memberclub-api uses
# sqlx::migrate! at boot, so this only matters for ad-hoc dev fiddling).
sqlx migrate run --source apps/memberclub/api/migrations \
    --database-url postgres://app:app@127.0.0.1/app

# Seed with deterministic dev fixtures.
cargo run -p memberclub-api --bin memberclub-seed -- --profile dev   # when wired

# Tear down (-v wipes the volume).
make down
```

## Adding a migration

```bash
sqlx migrate add --source apps/memberclub/api/migrations my_change
```

The file lands as `apps/memberclub/api/migrations/<ts>_my_change.sql`.
Migrations are applied in lexicographic order; never edit one after it
has shipped — add a new one that reverses or amends.
