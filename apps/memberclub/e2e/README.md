# MemberClub — e2e/

Cross-stack end-to-end suite. Drives the **real** Rust API + the **real**
SvelteKit web app + the **real** Stripe CLI in test mode through a
browser, all booted via the root `compose.yaml`.

Why not just hit the API directly? Because the e2e suite catches what
unit + integration tests can't:

  * a `+page.server.ts` `load()` returning the wrong shape and the
    component rendering nothing.
  * a SvelteKit form action hitting the API but the API's CORS rejecting
    the origin.
  * a Stripe Checkout redirect that comes back to a URL the web app
    doesn't have a route for.
  * a session cookie set by the API not being readable by SvelteKit's
    `hooks.server.ts` because of a `SameSite` mismatch.

Each one was a production incident at some company, once. The e2e suite
is the watch on the ferry.

## Layout

```
e2e/
├── package.json           # Playwright + pnpm scripts
├── playwright.config.ts   # device matrix, base URL, retries
├── tests/
│   ├── signup.spec.ts     # register + verify email + sign in
│   ├── billing.spec.ts    # Checkout → webhook → DB row → /account
│   ├── members-area.spec.ts  # gated content, login redirect
│   └── admin.spec.ts      # impersonate + audit log
├── fixtures/
│   ├── stripe-cli.ts      # spawns `stripe listen` for the test
│   └── seed.ts            # deterministic fixtures
└── playwright/.cache/     # pinned browser binaries (gitignored)
```

## Running

```bash
# Bring up the stack (Postgres + Redis + MailHog + stripe-cli).
make up

# Boot the API + web (background).
cargo run -p memberclub-api &
pnpm -C apps/memberclub/web dev &

# Run the suite.
pnpm -C apps/memberclub/e2e test

# Or, via `make`:
make e2e
```

## Conventions

  * **Hermetic.** Every test starts from a freshly-seeded DB (the
    `seed` fixture truncates and reloads). Tests do not share state.
  * **No fake Stripe.** The suite uses the real Stripe test mode via
    `stripe listen --forward-to ...`; `stripe trigger` produces the
    webhook events the test expects.
  * **Headless by default.** Pass `PWDEBUG=1 pnpm test` for an
    interactive debug session.

## Skeleton

This directory currently contains only this README — the test files are
the deliverable for `curriculum/phase-09-svelte-sveltekit/EXERCISES.md`
E9.4 ("Add a Playwright e2e test"). The first one to ship is
`signup.spec.ts`; the rest follow as the capstone fills in.
