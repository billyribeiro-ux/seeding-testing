# Lesson 3.6 — Step A: Drizzle + SQLite + SvelteKit Walkthrough

> **The friendly on-ramp.** Build `projects/02b-sqlite-notes-svelte/` step by step. No Docker, no Postgres, no server — a file on disk and a tiny web app.
> **Time:** 90 minutes.

## What we're building

A single page that lists, adds, and deletes notes:

```
+-----------------------------------+
| Notes                             |
|                                   |
| [ New note ............ ] [ Add ] |
|                                   |
| #3 — 2026-05-26T16:50:00          |
|   "buy milk"                      |
|   delete                          |
|                                   |
| #2 — 2026-05-26T16:48:12          |
|   "call mom"                      |
|   delete                          |
+-----------------------------------+
```

Persisted in a SQLite file (`./dev.sqlite`). No JavaScript required — the form works with JS disabled.

## Step 0 — Browse the project

```bash
cd projects/02b-sqlite-notes-svelte
ls -R src
```

You should see:

```
src/
├── app.d.ts
├── app.html
├── lib/server/db/
│   ├── index.ts
│   ├── queries.test.ts
│   ├── queries.ts
│   └── schema.ts
└── routes/
    ├── +page.server.ts
    └── +page.svelte
```

Read along. Each file is small and focused.

## Step 1 — The schema

`src/lib/server/db/schema.ts`:

```ts
import { sqliteTable, integer, text } from 'drizzle-orm/sqlite-core';
import { sql } from 'drizzle-orm';

export const notes = sqliteTable('notes', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  body: text('body').notNull(),
  createdAt: text('created_at').notNull()
    .default(sql`(strftime('%Y-%m-%dT%H:%M:%fZ','now'))`)
});

export type Note = typeof notes.$inferSelect;
export type NewNote = typeof notes.$inferInsert;
```

Three things to notice:

1. **Schema *is* code.** The shape of the table is a TypeScript value. `drizzle-kit push` translates it into `CREATE TABLE` SQL.
2. **Camel-case in TS, snake_case in SQL.** Drizzle handles the mapping (`createdAt` ↔ `created_at`).
3. **`$inferSelect` / `$inferInsert`** generate the row types automatically. `Note` is what you `SELECT`; `NewNote` is what you `INSERT` (so `id` is optional).

## Step 2 — The connection

`src/lib/server/db/index.ts`:

```ts
import Database from 'better-sqlite3';
import { drizzle } from 'drizzle-orm/better-sqlite3';
import { env } from '$env/dynamic/private';
import * as schema from './schema';

const sqlite = new Database(env.DATABASE_URL ?? './dev.sqlite');
sqlite.pragma('journal_mode = WAL');
sqlite.pragma('foreign_keys = ON');

export const db = drizzle(sqlite, { schema });
```

Three habits worth copying:

- **`$lib/server`** — SvelteKit hard-errors if any client-side file imports anything from here. Native SQLite never reaches the browser bundle.
- **`journal_mode = WAL`** — write-ahead logging. Reads don't block writes, writes don't block reads. The standard for SQLite in production.
- **`foreign_keys = ON`** — SQLite *doesn't* enforce FKs by default (historical quirk). Always turn it on.

## Step 3 — The queries

`src/lib/server/db/queries.ts`:

```ts
import { desc, eq } from 'drizzle-orm';

export async function listNotes(db) {
  return db.select().from(notes).orderBy(desc(notes.id)).all();
}

export async function addNote(db, body: string) {
  const trimmed = body.trim();
  if (trimmed.length === 0) throw new Error('body cannot be empty');
  if (trimmed.length > 4096)  throw new Error('body cannot exceed 4096 characters');
  const [inserted] = db.insert(notes).values({ body: trimmed }).returning().all();
  return inserted;
}

export async function deleteNote(db, id: number) {
  const result = db.delete(notes).where(eq(notes.id, id)).run();
  return result.changes > 0;
}
```

This is the Drizzle vocabulary in miniature:

| Operation | Drizzle | Generated SQL |
|---|---|---|
| Select all | `db.select().from(notes)` | `SELECT * FROM notes` |
| Order | `.orderBy(desc(notes.id))` | `ORDER BY id DESC` |
| Filter | `.where(eq(notes.id, 1))` | `WHERE id = ?` (with `1` bound) |
| Insert | `.insert(notes).values({...}).returning()` | `INSERT INTO notes (...) VALUES (?) RETURNING *` |
| Delete | `.delete(notes).where(eq(...))` | `DELETE FROM notes WHERE ...` |

Type-safe end to end. Drop a typo on a column name — TypeScript catches it.

## Step 4 — The page

`src/routes/+page.server.ts` runs *on the server*:

```ts
export const load = async () => {
  return { notes: await listNotes(db) };
};

export const actions = {
  create: async ({ request }) => {
    const body = String((await request.formData()).get('body') ?? '');
    try { return { ok: true, created: (await addNote(db, body)).id }; }
    catch (e) { return fail(400, { body, error: e.message }); }
  },
  delete: async ({ request }) => {
    const id = Number((await request.formData()).get('id'));
    if (!Number.isInteger(id) || id <= 0) return fail(400, { error: 'invalid id' });
    if (!(await deleteNote(db, id)))     return fail(404, { error: 'not found' });
    return { ok: true, deleted: id };
  }
};
```

`+page.svelte` consumes the data:

```svelte
<script lang="ts">
  let { data, form } = $props();
</script>

<form method="POST" action="?/create">
  <textarea name="body" rows="3" required></textarea>
  <button>Add</button>
</form>

<ul>
  {#each data.notes as note (note.id)}
    <li>
      <p>{note.body}</p>
      <form method="POST" action="?/delete">
        <input type="hidden" name="id" value={note.id} />
        <button>delete</button>
      </form>
    </li>
  {/each}
</ul>
```

Three things to notice:

- **No JavaScript needed.** SvelteKit progressively enhances the form *if* JS loads; without JS, the browser submits the form directly and the page re-renders server-side. Accessibility for free.
- **`<form method="POST" action="?/create">`** — the `?/create` matches the named action in `+page.server.ts`. Clean URLs, no `/api/notes` plumbing.
- **`$props()`** — Svelte 5 rune. `data` is the return of `load`; `form` is the result of the last action (or `null` on first render).

## Step 5 — Run it

```bash
pnpm install
pnpm db:push       # `drizzle-kit push` — creates ./dev.sqlite, applies schema
pnpm dev           # http://localhost:5173
```

Add a note. Refresh. Delete it. Refresh.

## Step 6 — Tests

`src/lib/server/db/queries.test.ts`:

```ts
import { describe, it, expect, beforeEach } from 'vitest';

describe('queries', () => {
  let db;
  beforeEach(() => {
    const sqlite = new Database(':memory:');
    sqlite.exec(`CREATE TABLE notes (...);`);
    db = drizzle(sqlite, { schema: { notes } });
  });

  it('lists newest-first', async () => { /* ... */ });
  it('trims whitespace', async () => { /* ... */ });
  it('rejects empty', async () => { /* ... */ });
  it('rejects too long', async () => { /* ... */ });
  // ... 7 in total
});
```

Each test gets a fresh in-memory database. Hermetic, fast, no cleanup.

```bash
pnpm test          # 7 / 7 green
pnpm check         # type-check, 0 errors
pnpm build         # production build via adapter-node
```

## Why this matters

- **SvelteKit's `+page.server.ts` collapses backend and frontend into one file.** For prototypes and admin dashboards, the friction is genuinely lower than wiring Axum + a separate Svelte SPA.
- **Drizzle's type inference catches schema/code drift.** Add a column, rename one — TypeScript fails the build until every query is updated.
- **SQLite is enough.** For a side project, a tiny SaaS, or an internal tool, SQLite + LiteFS is a perfectly serious production stack. We don't dismiss it — we just don't use it for the capstone (which needs Postgres features Stripe and RBAC will want).

## Green-bar checkpoint

- `pnpm install && pnpm db:push && pnpm dev` shows a working notes app at `localhost:5173`.
- `pnpm test` is green.
- You can explain three things SvelteKit's `$lib/server` enforces.

Next: `lessons/07-stepB-postgres-in-docker.md`.
