# apps/memberclub/web

The MemberClub web app — SvelteKit 2 + Svelte 5 + Drizzle + SQLite.
Self-contained for the Phase 9 capstone; later phases swap the local
DB for HTTP calls to the Rust services.

## What's in it

- Auth: signup + login + logout with scrypt password hashing and
  signed HttpOnly cookies. `hooks.server.ts` populates
  `event.locals.user` for every request.
- Notes: a protected `/notes` route with create + delete form actions.
- Runes demo: the home page shows `$state` + `$derived`.
- Tests: 5 vitest cases for the password helpers.

## Run it

```bash
pnpm install
pnpm db:push          # creates ./dev.sqlite + applies schema
pnpm dev              # http://localhost:5173
```

## Verify

```bash
pnpm test             # vitest: 5/5
pnpm check            # svelte-check: 0 errors, 0 warnings
pnpm build            # adapter-node production build
```

## File map

| Path | Purpose |
|---|---|
| `src/hooks.server.ts` | session cookie → `event.locals.user` |
| `src/lib/server/db/{schema,index}.ts` | Drizzle schema + singleton DB |
| `src/lib/server/auth/password.ts` | scrypt-based password hashing |
| `src/lib/server/auth/sessions.ts` | session token + DB row management |
| `src/routes/+layout.{svelte,server.ts}` | nav bar + exposes user to children |
| `src/routes/login/+page.{svelte,server.ts}` | signup + login form actions |
| `src/routes/logout/+server.ts` | logout endpoint |
| `src/routes/notes/+page.{svelte,server.ts}` | protected notes CRUD |
| `src/routes/+page.svelte` | home page with runes demo |
