# Phase 9 — Exercises

Seven graded drills extending `apps/memberclub/web`.

---

## E9.1 — Add a `+error.svelte` (Easy)

Create a top-level `src/routes/+error.svelte` that displays
`{$page.error.message}` and the status code in a friendly way. Throw
`error(418, "I'm a teapot")` from `/+page.server.ts`'s `load` to verify it
renders.

---

## E9.2 — Validation on the client *and* server (Easy)

Right now the password length is enforced on the server. Add a client-side
`minlength="8"` attribute on the password input. Add a vitest assertion that
attempting to login with a 7-char password fails server-side too.

---

## E9.3 — Optimistic UI for delete (Medium)

Add `use:enhance` to the delete form on `/notes`. Before the server
responds, optimistically remove the deleted note from the DOM. On error
(non-200), restore it.

<details><summary>Answer (sketch)</summary>

```svelte
<form method="POST" action="?/delete" use:enhance={({ formData, cancel }) => {
  const id = Number(formData.get('id'));
  const idx = data.notes.findIndex(n => n.id === id);
  const removed = data.notes.splice(idx, 1)[0];   // optimistic
  return async ({ result }) => {
    if (result.type !== 'success') {
      data.notes.splice(idx, 0, removed);          // restore
    }
  };
}}>
  ...
</form>
```
</details>

---

## E9.4 — Add a Playwright e2e test (Medium)

Add `@playwright/test`. Create `e2e/auth.spec.ts` that:

1. Visits `/` (asserts the home title).
2. Signs up with a unique email.
3. Asserts redirection to `/notes`.
4. Adds a note.
5. Logs out, then verifies `/notes` redirects back to `/login`.

---

## E9.5 — Replace Drizzle queries with calls to the Rust notes-api (Stretch)

The point of MemberClub's full architecture is that the SvelteKit web
talks to the Rust API, not its own DB. Refactor `/notes/+page.server.ts` to
call `notes-api` over HTTP instead of Drizzle.

Steps:

- Bring up `notes-api` on `127.0.0.1:3000`.
- In `+page.server.ts` `load`, `fetch('http://127.0.0.1:3000/v1/notes')`.
- In `create`, `fetch(... POST ...)`.
- In `delete`, `fetch(... DELETE ...)`.
- Handle errors: 4xx → `fail(...)`; 5xx → `error(500, ...)`.

This is the production architecture in miniature.

---

## E9.6 — Snapshot-test the home page (Stretch)

Use `vitest`'s snapshot helpers (or `playwright`'s
`expect(page).toHaveScreenshot()`) to lock the home page's rendered HTML.

---

## E9.7 — Add Stripe Checkout redirect (Stretch)

Add a `/billing/upgrade` route with a button that POSTs to a
`+page.server.ts` action which:

1. Creates a Checkout Session via the Stripe API.
2. Returns `{ url: session.url }`.
3. The Svelte page redirects the browser to that URL.

Requires `STRIPE_API_KEY` in env; use test mode. Combines Phase 8 and
Phase 9.
