# Lesson 9.8 — Build the MemberClub Web App

> **The capstone of Phase 9.** Walk through `apps/memberclub/web/` end to end.
> **Time:** 60 minutes.

## What's in the app

```
apps/memberclub/web/
├── package.json
├── svelte.config.js
├── vite.config.ts
├── drizzle.config.ts
├── tsconfig.json
└── src/
    ├── app.html
    ├── app.d.ts
    ├── hooks.server.ts                     ← cookie → event.locals.user
    ├── lib/server/
    │   ├── db/
    │   │   ├── schema.ts                   ← users, sessions, notes
    │   │   └── index.ts                    ← Drizzle singleton
    │   └── auth/
    │       ├── password.ts                 ← scrypt hash/verify
    │       ├── password.test.ts            ← 5 vitest tests
    │       └── sessions.ts                 ← token_hash + find/create/revoke
    └── routes/
        ├── +layout.server.ts               ← passes user to all pages
        ├── +layout.svelte                  ← nav bar
        ├── +page.svelte                    ← home; runes demo
        ├── login/
        │   ├── +page.server.ts             ← register + login actions
        │   └── +page.svelte                ← form
        ├── logout/+server.ts               ← POST logout
        └── notes/
            ├── +page.server.ts             ← protected; CRUD actions
            └── +page.svelte                ← UI
```

15 files. Three protected routes. Five vitest tests.

## The request lifecycle, end-to-end

A user clicks "Add" on `/notes`:

```
1. Browser POSTs to /notes?/create with the form body.
2. SvelteKit's server entry receives the request.
3. hooks.server.ts runs:
     - reads `memberclub_session` cookie
     - looks it up via findUserBySession()
     - sets event.locals.user = User
4. The action `create` in /notes/+page.server.ts runs:
     - throws error(401) if !locals.user
     - parses body, validates
     - inserts a row in `notes` for locals.user.id
     - returns { ok: true }
5. SvelteKit re-runs /notes/+page.server.ts's `load`:
     - fetches the user's notes (now including the new one)
6. The response is the updated /notes page HTML.
7. The browser renders it — a full page navigation.
```

These forms are plain `<form method="POST">` with no `use:enhance`, so
step 7 is always a full page reload (and it works with JS disabled). Add
`use:enhance` to a form and that same round-trip happens over `fetch` with
an in-place DOM patch instead — the server code is identical either way.

## Three patterns to memorize

### 1. Session cookie + DB lookup

```ts
// hooks.server.ts
const token = event.cookies.get(SESSION_COOKIE);
const found = findUserBySession(token);
event.locals.user = found ? found.user : null;
```

Hash the token in the DB (`sessions.token_hash = SHA-256(token)`). Look
up; verify not revoked and not expired; attach the user.

### 2. `$lib/server/` for the DB

```ts
// $lib/server/db/index.ts
import Database from 'better-sqlite3';
import { drizzle } from 'drizzle-orm/better-sqlite3';
const sqlite = new Database(env.DATABASE_URL ?? './dev.sqlite');
sqlite.pragma('journal_mode = WAL');
sqlite.pragma('foreign_keys = ON');
export const db = drizzle(sqlite, { schema });
```

A `.svelte` component that imports this would *fail the build*. Native
binding stays server-side.

### 3. Form actions for mutations, runes for ephemeral UI

```svelte
<!-- /notes/+page.svelte -->
<script>
  let { data, form } = $props();
  let draft = $state('');                  // ephemeral, never sent to the server
  let chars = $derived(draft.length);      // computed, also ephemeral
</script>

<form method="POST" action="?/create" onsubmit={() => (draft = '')}>
  <textarea name="body" bind:value={draft}></textarea>
  <p>{chars}/4096</p>
  <button>Add</button>
</form>
```

- `draft` is `$state` because it's local UI state.
- `chars` is `$derived` because it tracks `draft`.
- The submission goes through the *form action*, not a fetch from script.

Form action for *state changes*, runes for *ephemeral UI*. Each in its
proper place.

## Running it

```bash
cd apps/memberclub/web
pnpm install
pnpm db:push                 # creates ./dev.sqlite, applies schema
pnpm dev                     # http://localhost:5173

# In another tab:
pnpm test                    # 5/5 vitest (password)
pnpm check                   # svelte-check: 0/0
pnpm build                   # vite + adapter-node
```

Browser flow:

1. Visit `/`. Click "log in".
2. Sign up with `alice@example.com` and a 12-char password.
3. Redirected to `/notes`.
4. Add a note. Refresh. Note's still there.
5. Click "log out". Redirected to `/`.
6. Try to visit `/notes` while logged out — redirected to `/login`.

## What ships in production MemberClub (post-curriculum)

The same app, with two changes:

- The `notes` table lives in *Postgres* (via the Rust `notes-api`), not
  local SQLite. The SvelteKit `load`/`actions` call the Rust API via
  `fetch` instead of running Drizzle queries directly.
- The `users` + `sessions` tables also live in Postgres, served by the
  Rust `auth-demo` (Phase 6) endpoints. The SvelteKit hook fetches the
  user via the auth API on each request.

Both swaps are mechanical: replace `db.select(...)` calls with
`fetch('/api/...')` calls. The patterns — runes, actions, layouts — stay
identical.

## Why this matters

- **One mental model for the whole frontend** — runes for UI, form
  actions for mutations, `$lib/server` for secrets.
- **Auth on the server, never the client.** The browser never sees the
  password hash or the session lookup logic.
- **Progressive enhancement is one opt-in away.** Every form here works
  without JS today; add `use:enhance` and it upgrades to a no-reload fetch
  without changing the server action.

Phase 9 is complete. Phase 10 — **Observability** — instruments
everything we've built.
