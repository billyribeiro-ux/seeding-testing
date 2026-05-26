# projects/02b-sqlite-notes-svelte

Phase 3 — Step A. A tiny SvelteKit app backed by **Drizzle ORM + SQLite**, demonstrating the lowest-friction database experience in 2026.

## What it teaches

- SvelteKit `+page.server.ts` for server-only data loading and form actions.
- `$lib/server` for code that must never reach the browser bundle.
- Drizzle schema, `notes.$inferSelect` / `$inferInsert` types.
- `drizzle-kit push` to apply schema changes to a SQLite file.
- Vitest tests against an in-memory SQLite instance.
- Progressive enhancement: the form works *without JavaScript*.

## Quick start

```bash
cd projects/02b-sqlite-notes-svelte
pnpm install
pnpm db:push       # creates ./dev.sqlite and applies the schema
pnpm dev           # http://localhost:5173
```

Add a note. Refresh. It's still there. Delete it. Refresh. Gone.

## Tests

```bash
pnpm check         # type-check the whole project
pnpm test          # vitest, hermetic in-memory SQLite
pnpm build         # production build via adapter-node
```

## File map

| File | Purpose |
|---|---|
| `src/lib/server/db/schema.ts` | Drizzle schema for `notes` |
| `src/lib/server/db/index.ts` | Singleton `db` (better-sqlite3) |
| `src/lib/server/db/queries.ts` | `listNotes`, `addNote`, `deleteNote` |
| `src/lib/server/db/queries.test.ts` | Vitest unit tests against in-memory SQLite |
| `src/routes/+page.svelte` | The UI (Svelte 5 runes) |
| `src/routes/+page.server.ts` | `load` + `actions` (create, delete) |
| `drizzle.config.ts` | drizzle-kit configuration |
| `svelte.config.js` | SvelteKit + `adapter-node` |

## Compared with Step B (`projects/02c-sqlx-notes`)

| Concern | Drizzle/SQLite | sqlx/SQLite (or Postgres) |
|---|---|---|
| Query author | TypeScript builder (`db.select().from(notes).where(eq(notes.id, 1))`) | Hand-written SQL inside a macro (`sqlx::query_as!(Note, "SELECT … FROM notes WHERE id = $1", 1)`) |
| Compile-time checking | TypeScript types via inference | Real SQL parsed against a live DB or `.sqlx/` cache |
| Migrations | `drizzle-kit push` from schema | `sqlx migrate add` + handwritten SQL |
| Onboarding | < 5 minutes | ~30 minutes |
| Long-term ceiling | Hits the ORM's expressiveness limits | Whatever the database supports |

Both are tools a Principal Engineer keeps in their kit. See `curriculum/phase-03-sql-databases/lessons/10-drizzle-vs-sqlx.md` for the long-form comparison.
