# docs/

Project-level reference material. The curriculum points here from Phase 7
onward.

## Layout

```
docs/
├── 00-mental-models/        ← short essays; principal-engineer reference
├── 01-architecture-decisions/  ← ADRs in MADR format
├── 02-rfcs/                 ← design docs (RFCs)
├── 03-postmortems/          ← incident postmortems
└── runbooks/                ← on-call diagnostic + mitigation steps
```

## What lives where

### `00-mental-models/`

Short, durable essays you can re-read after years and still find useful.
Each is < 1000 words and standalone.

- `money.md` — `i64` cents, the $21B ceiling, the `Money` newtype
- `async.md` — cooperative multitasking, the kitchen metaphor
- `db-as-source-of-truth.md` — mirror don't synchronize

(More to add as the team discovers them.)

### `01-architecture-decisions/`

ADRs in [MADR](https://adr.github.io/madr/) format. Numbered. Immutable
once accepted (revise by writing a new one that supersedes).

- `0000-TEMPLATE.md` — copy this to start a new one
- `0001-rust-axum.md` — Rust + Axum
- `0002-orm-stance.md` — sqlx prod + Drizzle SQLite on-ramp
- `0003-money-i64-cents.md` — i64 cents, $21B ceiling
- `0004-dual-mode-auth.md` — cookies + JWT
- `0005-explicit-policies-no-casbin.md`
- `0006-outbox-over-broker.md`
- `0007-postgres-rls-tenant-isolation.md`
- `0008-stripe-is-the-rail.md`

### `02-rfcs/`

Design docs / RFCs. Authored before non-trivial work begins.

- `0000-TEMPLATE.md` — copy this to start a new one

Phase 12 capstone has you author one. Place it here.

### `03-postmortems/`

Blameless incident postmortems. One per incident over 5 minutes or
affecting > 100 users.

- `0000-TEMPLATE.md` — copy this to start a new one

Phase 12 capstone has you author one. Place it here.

### `runbooks/`

On-call diagnostic + mitigation guides. One per known failure mode.

- `5xx-spike.md` — notes-api 5xx spike triage
- `stripe-webhook-lag.md` — webhook delivery lag

## Adding a new doc

1. **Mental model:** if you find yourself explaining the same idea more
   than three times, write the essay. < 1000 words; standalone.
2. **ADR:** when you make an architectural decision that the next
   engineer will ask "why?" about, write it. Use the template.
3. **RFC:** when proposing > 1 week of work, write it. Use the template.
4. **Postmortem:** within 48 hours of an incident > 5 min. Use the
   template.
5. **Runbook:** when an alert fires and the on-call has to think to
   diagnose it, write the runbook so the next person doesn't have to.

All four artifact types are part of the Phase 12 rubric.
