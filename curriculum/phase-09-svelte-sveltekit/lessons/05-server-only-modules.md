# Lesson 9.5 — Server-Only Modules

> **Concept first:** `$lib/server/` is a *privacy fence* enforced by the SvelteKit compiler. Modules under it can never reach the browser bundle. The compiler refuses to build the project if a client component imports from there.
> **Time:** 15 minutes.

## What lives under `$lib/server/`

- Database clients (`better-sqlite3`, `pg`, `redis`).
- Native modules with N-API bindings.
- Secrets-reading code.
- Cryptography helpers.
- The Rust API client (when MemberClub calls notes-api).

The pattern in MemberClub web:

```
src/lib/server/
├── db/
│   ├── schema.ts
│   ├── index.ts          ← singleton SqlitePool
│   └── queries.ts
└── auth/
    ├── password.ts       ← scrypt; uses node:crypto
    └── sessions.ts       ← cookie token + DB lookup
```

A component that accidentally `import { db } from '$lib/server/db'` would
fail the build:

```
Cannot import $lib/server/db/index.ts from client-only code.
```

That's the entire feature. Strict. Enforced at compile time.

## `$env/dynamic/private` and `$env/static/private`

The other half of the privacy fence:

```ts
import { env } from '$env/dynamic/private';
const databaseUrl = env.DATABASE_URL;       // never reaches the client
```

| Module | Read | Bundled |
|---|---|---|
| `$env/dynamic/private` | Server runtime | Server only |
| `$env/static/private` | Server build time | Server only |
| `$env/dynamic/public` | Server runtime | Server + client |
| `$env/static/public` | Server build time | Server + client |

Naming-convention rule: variables starting with `PUBLIC_` show up in the
`public` modules; everything else is private.

```bash
# .env
DATABASE_URL=postgres://...                # private
SESSION_SECRET=...                         # private
PUBLIC_STRIPE_PUBLISHABLE_KEY=pk_test_...  # public (client can see)
```

Forget the `PUBLIC_` prefix and your secret stays on the server. Add it and
the variable is bundled into the client. The compiler tells you which when
you try to import.

## Why "compiler-enforced" matters

Other frameworks rely on convention: "don't import the DB client from
client code." Conventions get broken. The SvelteKit rule is *the compiler
refuses*. The fence is real.

## How we structure the boundary in MemberClub

```
+page.server.ts        ┐ server only
hooks.server.ts        ┘
                       │
$lib/server/...       ┘ server only (compiler-enforced)
                       
+page.svelte           ┐
$lib/...   (no /server) ┤ server + client
$lib/...               ┘
```

The arrow direction matters: `+page.svelte` → `+page.server.ts` is
implicit at framework level (load data through the route file). Imports
between them aren't direct — `+page.svelte` only sees `data` and `form`
from `+page.server.ts`.

## Common mistake: native modules

`better-sqlite3` is a native N-API addon. Importing it from a `.svelte`
component would try to bundle the native binary into the browser, which
fails. The fix is always: move to `$lib/server/`.

The compiler's error message is sometimes confusing for natives — it'll
complain about "module not found" or "process is not defined." The root
cause is almost always "should be `$lib/server/`."

## Why this matters

- **The privacy fence is the cheapest security control in the stack.** No
  config, no convention — the compiler enforces it.
- **Naming `PUBLIC_*` is a *deliberate* opt-in.** No accidents.
- **`$lib/server/` keeps native deps off the browser.** Build errors
  surface immediately, not at runtime.

## Green-bar checkpoint

- You can name three things that belong in `$lib/server/`.
- You can articulate the difference between `$env/dynamic/private` and
  `$env/dynamic/public`.
- You can predict the compile error when a `.svelte` file imports from
  `$lib/server/`.

Next: `lessons/06-hooks-and-session-auth.md`.
