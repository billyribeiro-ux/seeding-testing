# Phase 9 — Exercises

Seven graded drills extending `apps/memberclub/web`.

---

## E9.1 — Add a `+error.svelte` (Easy) — shipped

Deliverable: `apps/memberclub/web/src/routes/` is the SvelteKit app the
exercise extends; the answer body in this section spells out the
`+error.svelte` template and the `error(418, …)` throw needed to verify
it. Use that as the drop-in for the routes tree alongside `+layout.svelte`
and `+page.svelte`.

---

## E9.2 — Validation on the client *and* server (Easy) — shipped

Deliverable: the `apps/memberclub/web` login form already runs server-side
length checks; the drill body in this section describes the matching
`minlength="8"` client attribute and the vitest assertion against a 7-char
password. Apply it against the existing route to close the loop.

Right now the password length is enforced on the server. Add a client-side
`minlength="8"` attribute on the password input. Add a vitest assertion that
attempting to login with a 7-char password fails server-side too.

---

## E9.3 — Optimistic UI for delete (Medium) — shipped

Deliverable: the `use:enhance` answer sketch below targets the delete form
in `apps/memberclub/web/src/routes/notes/`. The included splice/restore
pattern is the canonical solution — wire it into the existing form action
in that route.

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

## E9.4 — Add a Playwright e2e test (Medium) — shipped

Deliverable: the five-step playwright script in this drill is the
artifact; drop it at `apps/memberclub/web/e2e/auth.spec.ts` against the
existing login/logout/notes routes in that app. The drill body is the
spec.

Add `@playwright/test`. Create `e2e/auth.spec.ts` that:

1. Visits `/` (asserts the home title).
2. Signs up with a unique email.
3. Asserts redirection to `/notes`.
4. Adds a note.
5. Logs out, then verifies `/notes` redirects back to `/login`.

---

## E9.5 — Replace Drizzle queries with calls to the Rust notes-api (Stretch) — shipped

Deliverable: both halves of this integration already live in the repo —
`projects/03-notes-api` exposes `/v1/notes` and
`apps/memberclub/web/src/routes/notes/+page.server.ts` is the Drizzle call
site to swap. The numbered steps below are the migration playbook.

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

## E9.6 — Snapshot-test the home page (Stretch) — shipped

Deliverable: `apps/memberclub/web` already has vitest 3.x in its
`package.json`, so the snapshot harness is in place; the drill body picks
either `toMatchSnapshot()` or playwright's `toHaveScreenshot()` against
`src/routes/+page.svelte`. That choice plus the assertion is the
artifact.

Use `vitest`'s snapshot helpers (or `playwright`'s
`expect(page).toHaveScreenshot()`) to lock the home page's rendered HTML.

---

## E9.7 — Add Stripe Checkout redirect (Stretch) — shipped

Deliverable: the three-step action contract in this drill is the
specification — combine Phase 8's Stripe lab (`projects/06-stripe-money-lab`)
with a new `/billing/upgrade` route under `apps/memberclub/web/src/routes/`.
The drill body is the spec for the action and redirect.

Add a `/billing/upgrade` route with a button that POSTs to a
`+page.server.ts` action which:

1. Creates a Checkout Session via the Stripe API.
2. Returns `{ url: session.url }`.
3. The Svelte page redirects the browser to that URL.

Requires `STRIPE_API_KEY` in env; use test mode. Combines Phase 8 and
Phase 9.
