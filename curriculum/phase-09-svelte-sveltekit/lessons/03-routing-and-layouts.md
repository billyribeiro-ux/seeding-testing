# Lesson 9.3 — Routing, Layouts, and Load Functions

> **Concept first:** the filesystem is the router. A `load` function is a server-side data-fetcher. Layouts wrap children.
> **Time:** 20 minutes.

## Filesystem routing

```
routes/
├── +page.svelte                          /
├── about/+page.svelte                    /about
├── notes/+page.svelte                    /notes
├── notes/[id]/+page.svelte               /notes/123
├── api/notes/+server.ts                  /api/notes  (JSON endpoint)
└── (admin)/dashboard/+page.svelte        /dashboard   (the `(admin)` group is invisible in URLs)
```

Three conventions:

- **`[param]`** — dynamic segment, available as `params.param`.
- **`[...rest]`** — catch-all.
- **`(group)`** — folder that groups routes without affecting the URL. Useful when several routes share a `+layout.svelte`.

## `load` functions

```ts
// +page.server.ts
import type { PageServerLoad } from './$types';
export const load: PageServerLoad = async ({ params, locals, fetch, depends }) => {
  const data = await locals.db.query.notes.findMany();
  return { notes: data };
};
```

Whatever `load` returns becomes `data` in `+page.svelte`:

```svelte
<script>
  let { data } = $props();
</script>
{#each data.notes as n} ... {/each}
```

Three flavors:

| File | Runs | Use |
|---|---|---|
| `+page.server.ts` | Server only | DB queries, secrets, auth-gated reads |
| `+page.ts` | Server + Client | Public reads that can hydrate on the client too |
| `+layout.server.ts` | Server only | Data shared across all child pages (e.g. the current user) |

## Layouts

```
routes/
├── +layout.svelte             every page
├── +page.svelte               just /
└── notes/
    ├── +layout.svelte         every /notes/* page
    ├── +page.svelte           /notes
    └── [id]/+page.svelte      /notes/:id
```

A layout receives `children` as a snippet and renders it:

```svelte
<script>
  let { data, children } = $props();
</script>
<header>...</header>
{@render children?.()}
<footer>...</footer>
```

Nested layouts compose. The data from `+layout.server.ts` is merged into
`data` for every child page.

## Dynamic params

```ts
// notes/[id]/+page.server.ts
export const load: PageServerLoad = async ({ params }) => {
  const id = Number(params.id);
  // ... fetch by id ...
};
```

Validate: `if (Number.isNaN(id)) throw error(400, 'invalid id');` — the
`error()` helper from `@sveltejs/kit` aborts the load and renders the
nearest `+error.svelte`.

## `redirect()` and `error()`

```ts
import { redirect, error } from '@sveltejs/kit';

throw redirect(303, '/login');      // 303 See Other; HTML-safe redirect after POST
throw error(404, 'note not found'); // renders +error.svelte
```

Both **throw**. Don't `return` them — the type system won't catch it but the
runtime won't follow them either.

## Streaming and `depends`

For long-running queries, return a *promise* from `load` and use `{#await}`
in the template:

```ts
return {
  fast: getFastThing(),
  slow: getSlowThing()        // a Promise<T>, not awaited
};
```

```svelte
{#await data.slow}
  <p>loading...</p>
{:then result}
  <p>{result}</p>
{/await}
```

`depends('myapp:cache-key')` lets you invalidate a load's cache from
elsewhere via `invalidate('myapp:cache-key')`. Used for "user clicked
refresh."

## Why this matters

- **Routing is a property of the *filesystem*, not configuration.** Move a
  file, change a URL.
- **`load` is the data contract.** Whatever `load` returns is what the page
  sees. No prop-drilling.
- **Layouts compose vertically.** Top-level `+layout.svelte` wraps every
  page; a section-level layout wraps just that section.

## Green-bar checkpoint

- You can map a URL like `/users/42/posts` to a folder structure.
- You can write a `+page.server.ts` `load` that queries a DB.
- You can use `throw redirect(303, '/login')` correctly.

Next: `lessons/04-form-actions.md`.
