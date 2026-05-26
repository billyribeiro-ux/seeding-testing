# Phase 9 — Svelte 5 / SvelteKit 2: The MemberClub Web App

> **Audience:** you finished Phase 8.
> **Outcome:** you can ship a SvelteKit web app — runes for state, server-only modules for secrets, form actions for mutations, hooks for session-based auth, vitest + svelte-check + production build all green.
> **Time:** 3 weeks.

## The mental model

> *SvelteKit is two things glued together: a meta-framework (routing, SSR, hooks, form actions) and a UI library (Svelte 5 with runes). The web is forms and links; SvelteKit just makes them feel like an app.*

Two grand principles:

1. **Server-side rendering by default.** Every route renders on the server first (so HTML is the source of truth) and *progressively enhances* with JavaScript. Forms work without JS; runes light up when JS arrives.
2. **`$lib/server/` is the privacy fence.** Anything imported from there cannot reach the browser bundle. The compiler refuses.

Add a third for our stack:

3. **The Rust backend stays on the backend.** SvelteKit talks to it via its own server code — `+page.server.ts` and `hooks.server.ts`. The browser never sees a Rust URL.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-mental-model.md` | What SvelteKit *is*; rendering modes; what's server, what's client |
| `lessons/02-runes-cheatsheet.md` | `$state`, `$derived`, `$effect`, `$props` — when each lives |
| `lessons/03-routing-and-layouts.md` | `+page.svelte`, `+page.server.ts`, `+layout.svelte`, `+layout.server.ts` |
| `lessons/04-form-actions.md` | Progressive enhancement; `?/named` actions; `fail()` and `redirect()` |
| `lessons/05-server-only-modules.md` | `$lib/server`, `$env/dynamic/private`, what the compiler refuses to bundle |
| `lessons/06-hooks-and-session-auth.md` | `hooks.server.ts`, `event.locals`, cookie-based session integration |
| `lessons/07-testing.md` | Vitest for logic, Playwright for browser, svelte-check for types |
| `lessons/08-build-memberclub-web.md` | Capstone walkthrough |

## The capstone — `apps/memberclub/web`

A self-contained SvelteKit app:

- **Auth:** signup + login (scrypt + signed cookie), logout, `event.locals.user` populated in `hooks.server.ts`.
- **Notes:** a protected `/notes` route with create/delete via form actions.
- **Runes:** the home page demonstrates `$state` + `$derived` (a tiny counter + doubled).
- **Drizzle + SQLite** for the local DB.
- **Tests:** vitest covers the password helpers; svelte-check passes with zero errors; `vite build` produces an `adapter-node` artifact.

In MemberClub-proper (post-curriculum), this app would call the Rust `notes-api` via `fetch` instead of using a local DB. The pattern is identical; only the data layer swaps.

## Green-bar checkpoint

```bash
cd apps/memberclub/web
pnpm install
pnpm db:push          # creates ./dev.sqlite
pnpm test             # 5/5 vitest
pnpm check            # 0 errors / 0 warnings
pnpm build            # produces build/ via adapter-node
pnpm dev              # http://localhost:5173
```

## What's next

Phase 10 — **Observability**. The app works; now we instrument it. tracing,
OpenTelemetry, structured logs, metrics, dashboards, alerting.
