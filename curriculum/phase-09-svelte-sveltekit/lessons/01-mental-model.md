# Lesson 9.1 — The SvelteKit Mental Model

> **Concept first:** SvelteKit is a meta-framework. It owns routing, server-side rendering, hooks, and form actions. Svelte (the language) renders the UI. They're glued together but conceptually separate.
> **Time:** 20 minutes.

## What's server, what's client

Every SvelteKit file falls into one of three categories, decided by its name:

| File | Where it runs |
|---|---|
| `+page.svelte` | Browser **and** server (rendered to HTML on the server; hydrated on the client) |
| `+page.server.ts` | Server **only** — `load`, `actions`. Never bundled into the browser. |
| `+page.ts` | Both server and client — for universal `load`s. Rare. |
| `+layout.svelte` | Both — like `+page.svelte` but wraps every child route |
| `+layout.server.ts` | Server only — same idea, but for the layout |
| `+server.ts` | Server only — a raw HTTP endpoint (no UI) |
| `hooks.server.ts` | Server only — once-per-request, before any handler |
| `hooks.client.ts` | Client only |
| files under `$lib/server/` | **Server only.** The compiler refuses to bundle them into the browser. |

The first two are 95% of your day-to-day. Internalize them.

## Rendering modes

| Mode | When it happens | Default? |
|---|---|---|
| **SSR** (server-side rendering) | The server renders HTML on every request | ✓ |
| **Prerender** | Build-time HTML (no server at request time) | opt-in per route |
| **SPA** | No server rendering, everything client-side | opt-in via adapter-static + fallback |

We use **SSR** for MemberClub — the user's session is on the server, so the first paint depends on the server. Prerender public marketing pages if needed.

## Progressive enhancement

The default form in SvelteKit is just `<form method="POST" action="?/create">`. It works **without JavaScript** — the browser submits the form, the server handles it, returns HTML.

When JavaScript loads, SvelteKit *enhances* the form: submissions become fetches; the page doesn't full-reload; you get optimistic UI for free.

You don't have to write either path explicitly. Write the form, get both.

## The folder layout

```
src/
├── app.html            outer HTML shell (head, body)
├── app.d.ts            ambient types for App.Locals, App.PageData
├── hooks.server.ts     runs before every request
├── lib/
│   └── server/         server-only modules (DB, secrets, hash)
└── routes/
    ├── +layout.svelte  top-level shell rendered around every page
    ├── +layout.server.ts
    ├── +page.svelte    /
    ├── login/
    │   ├── +page.svelte
    │   └── +page.server.ts
    └── notes/
        ├── +page.svelte
        └── +page.server.ts
```

Folder names are URL segments. `[slug]` is a dynamic param. `[...rest]` catches everything. The convention is mechanical.

## When to reach for what

| You want… | Use |
|---|---|
| To render a page | `+page.svelte` + `+page.server.ts` |
| To share UI across pages (nav, footer) | `+layout.svelte` |
| To preload data common to a section | `+layout.server.ts` |
| To expose an HTTP endpoint (JSON, RSS, image) | `+server.ts` |
| To run before every request (auth, logging) | `hooks.server.ts` |
| To call the DB / use secrets | `$lib/server/...` |
| To define reusable types or pure helpers | `$lib/...` (without `/server`) |

## Why this matters

- **The file-name conventions are *the* SvelteKit API.** You don't import a router — you put files in folders.
- **Server / client separation is enforced by the compiler.** Mistakes turn into build errors, not runtime XSS.
- **Progressive enhancement is the *default*.** Other frameworks require you to opt in; SvelteKit gives it to you for free.

## Green-bar checkpoint

- You can name the three most common file types and where each runs.
- You can articulate why `$lib/server/` exists.
- You can predict what `form method="POST" action="?/create"` does with and without JavaScript.

Next: `lessons/02-runes-cheatsheet.md`.
