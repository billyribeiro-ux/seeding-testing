# Lesson 9.6 — Hooks and Session-Based Auth

> **Concept first:** `hooks.server.ts` runs *once per request, before any handler*. It's where session cookies are validated and `event.locals.user` is populated. The rest of the app just reads `locals.user`.
> **Time:** 20 minutes.

## The shape

```ts
// src/hooks.server.ts
import type { Handle } from '@sveltejs/kit';
import { findUserBySession, SESSION_COOKIE } from '$lib/server/auth/sessions';

export const handle: Handle = async ({ event, resolve }) => {
  const token = event.cookies.get(SESSION_COOKIE);
  const found = findUserBySession(token);
  event.locals.user = found ? found.user : null;
  return resolve(event);
};
```

Three things happen:

1. The cookie is read.
2. The session is looked up in the DB.
3. The user (or `null`) is attached to `event.locals`.

Every downstream `+page.server.ts`, `+server.ts`, layout server load, etc.
can read `locals.user`. No reparsing the cookie per route.

## `App.Locals` — typed access

```ts
// src/app.d.ts
declare global {
  namespace App {
    interface Locals {
      user: import('$lib/server/db/schema').User | null;
    }
    interface PageData {
      user: import('$lib/server/db/schema').User | null;
    }
  }
}
```

Now `event.locals.user` is typed as `User | null` everywhere. Reading
`event.locals.foo` (which doesn't exist) is a compile error.

## Exposing `locals.user` to the page

```ts
// +layout.server.ts (top-level)
export const load: LayoutServerLoad = async ({ locals }) => {
  return { user: locals.user };
};
```

Now every page's `data.user` is populated. The nav bar in `+layout.svelte`
can switch on `data.user` without any per-route plumbing.

## Auth gates

In any `+page.server.ts`:

```ts
export const load: PageServerLoad = async ({ locals }) => {
  if (!locals.user) throw redirect(303, '/login');
  // ... protected logic ...
};
```

Two lines per protected route. No middleware, no decorator, no magic — just
a `redirect` if the user isn't there.

For *action* routes:

```ts
create: async ({ locals }) => {
  if (!locals.user) throw error(401, 'unauthorized');
  // ... mutation ...
}
```

## Cookie set on login

```ts
// /login/+page.server.ts
login: async ({ request, cookies }) => {
  // ... verify credentials ...
  const { token } = createSession(user.id);
  cookies.set(SESSION_COOKIE, token, {
    path: '/',
    httpOnly: true,
    sameSite: 'lax',
    secure: process.env.NODE_ENV === 'production',
    maxAge: 30 * 24 * 60 * 60
  });
  throw redirect(303, '/notes');
};
```

Standard cookie flags from Phase 6.2. The `secure` flag is gated on
`NODE_ENV` so it works on `http://localhost` in dev.

## Logout — `cookies.delete` + DB revoke

```ts
// /logout/+server.ts
export const POST: RequestHandler = async ({ cookies }) => {
  const token = cookies.get(SESSION_COOKIE);
  if (token) revokeSession(token);   // mark `revoked_at` in DB
  cookies.delete(SESSION_COOKIE, { path: '/' });
  throw redirect(303, '/');
};
```

Revoke first; clear the cookie; redirect. If the user kept the cookie
somehow (browser cache shenanigans), `findUserBySession` will reject it.

## Multiple `handle`s with `sequence`

When you want two hooks (e.g. one for auth, one for request logging),
compose them:

```ts
import { sequence } from '@sveltejs/kit/hooks';
import { handle as authHandle } from './hooks/auth';
import { handle as logHandle  } from './hooks/log';
export const handle = sequence(logHandle, authHandle);
```

Order matters: `logHandle` runs first; `authHandle` runs inside it.

## Why this matters

- **One source of truth for "who's logged in."** `event.locals.user` —
  populated once, read everywhere.
- **Auth gates are two lines of code.** No middleware framework, no DI
  container.
- **Logout invalidates server-side.** Stolen cookies don't survive a
  proper logout.

## Green-bar checkpoint

- You can write a `hooks.server.ts` that populates `event.locals.user`
  from a session cookie.
- You can sketch the `+layout.server.ts` that exposes the user to every
  page.
- You can articulate why we revoke the session on logout *before* clearing
  the cookie.

Next: `lessons/07-testing.md`.
