# Lesson 9.4 — Form Actions and Progressive Enhancement

> **Concept first:** a form action is a server-side handler attached to a `<form>` element. SvelteKit calls it on submit, returns data, re-renders. A plain form does a full page reload on submit. Add `use:enhance` and the *same* action runs over `fetch` with no reload — same server code either way.
> **Time:** 20 minutes.

## A minimal example

```ts
// +page.server.ts
import type { Actions } from './$types';
import { fail } from '@sveltejs/kit';

export const actions: Actions = {
  create: async ({ request }) => {
    const data = await request.formData();
    const body = String(data.get('body') ?? '');
    if (body.length === 0) return fail(400, { body, error: 'cannot be empty' });
    await db.insert(notes).values({ body }).run();
    return { ok: true };
  },
  delete: async ({ request }) => { /* ... */ }
};
```

```svelte
<!-- +page.svelte -->
<script>
  let { form } = $props();
</script>

{#if form?.error}<p class="error">{form.error}</p>{/if}

<form method="POST" action="?/create">
  <textarea name="body" required></textarea>
  <button>Save</button>
</form>
```

Three behaviors to understand:

1. **Plain form (no `use:enhance`).** The browser does a native POST, the
   server runs the `create` action, and returns HTML. The page does a full
   reload and re-renders with `form` set to the action's return value. This
   is the default — it works even with JavaScript disabled.
2. **Enhanced form (`use:enhance`).** Opt in and the submission becomes a
   `fetch`; the page doesn't reload, but `form` still gets the action's
   return value. SvelteKit does *not* enhance forms automatically — you add
   `use:enhance` yourself (see below).
3. **Named actions.** `action="?/create"` matches `actions.create`. You can
   have many actions on one page.

## `fail()` vs `error()` vs `redirect()`

| Helper | When | Effect |
|---|---|---|
| `return fail(400, { ... })` | Validation failed; show the user what's wrong | Sets `form` on the next render; status 400 |
| `throw error(401, 'unauthorized')` | The user shouldn't be here at all | Renders `+error.svelte` |
| `throw redirect(303, '/notes')` | Action succeeded; go somewhere else | HTTP 303 |

The `fail`/`error`/`redirect` distinction maps cleanly to UX: validation
errors stay on the page; auth errors throw to an error page; successful
mutations redirect to the next step.

## Returning data from actions

The return value is serialized to JSON and passed back as `form`. Keep it
small. Don't return the entire DB row; return a flag and an id at most:

```ts
return { ok: true, created: row.id };
```

Then in the page:

```svelte
{#if form?.ok}
  <p>Saved!</p>
{/if}
```

## `use:enhance` — tweaking the enhancement

Enhancement is opt-in: add the `use:enhance` action to a form and SvelteKit
intercepts the submit, runs the action over `fetch`, and applies the result
without a full reload. With no arguments it gives you the sensible default
behavior; pass a callback for custom behavior (optimistic UI, focus
management, progress indicators):

```svelte
<script>
  import { enhance } from '$app/forms';
</script>

<form method="POST" action="?/create" use:enhance={({ formElement, formData, cancel }) => {
  // before submit
  return async ({ result, update }) => {
    // after — `result` is the server's response
    if (result.type === 'success') {
      formElement.reset();
    }
    await update();
  };
}}>
  ...
</form>
```

For most cases bare `use:enhance` (no callback) is enough — it updates
`form`, the page data, and focus for you. Pass a callback only when you need
extra UX polish.

## Files and binary data

```svelte
<form method="POST" action="?/upload" enctype="multipart/form-data">
  <input type="file" name="avatar" />
  <button>Upload</button>
</form>
```

```ts
upload: async ({ request }) => {
  const data = await request.formData();
  const file = data.get('avatar');
  if (!(file instanceof File)) return fail(400, { error: 'no file' });
  const bytes = new Uint8Array(await file.arrayBuffer());
  // store, validate, etc.
}
```

## Why this matters

- **The default form works without JS.** Accessibility wins.
- **`fail`/`error`/`redirect` are a clean three-way split** for what an
  action ends in.
- **Returning serializable data** keeps the boundary explicit and forces
  you to think about *what the page actually needs*.

## Green-bar checkpoint

- You can write a `+page.server.ts` with two named actions.
- You can pick `fail`, `error`, or `redirect` for a given outcome.
- You can articulate why the default action works without JavaScript.

Next: `lessons/05-server-only-modules.md`.
