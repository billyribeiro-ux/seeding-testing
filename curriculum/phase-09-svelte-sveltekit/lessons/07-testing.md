# Lesson 9.7 — Testing SvelteKit Apps

> **Concept first:** three test flavors per the trophy (Phase 5.1): unit (vitest), browser (Playwright), type-check (svelte-check). Each catches a different bug class.
> **Time:** 15 minutes.

## The three layers

| Tool | What it catches |
|---|---|
| **svelte-check** | Type errors, unused props, missing imports, a11y warnings |
| **vitest** | Pure-logic regressions (password hashing, validators, query builders) |
| **Playwright** | "The button leads to the page" browser-level flows |

We run all three in CI. Each fails the build independently.

## svelte-check

```bash
pnpm check
```

Runs `svelte-kit sync` (to regenerate `.svelte-kit/types/`) then
`svelte-check --tsconfig ./tsconfig.json`. Output:

```
COMPLETED 526 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS
```

No errors, no warnings = ship it. Treat *all* warnings as errors in CI —
they accumulate otherwise.

## vitest for pure logic

```ts
// src/lib/server/auth/password.test.ts
import { describe, it, expect } from 'vitest';
import { hashPassword, verifyPassword } from './password';

describe('password', () => {
  it('round-trips', () => {
    const h = hashPassword('correct horse battery staple');
    expect(verifyPassword('correct horse battery staple', h)).toBe(true);
  });
});
```

Run with `pnpm test`. Vitest is fast; tests live next to the code they
test (`<name>.test.ts` beside `<name>.ts`).

## Playwright for end-to-end

```ts
// e2e/login.spec.ts
import { test, expect } from '@playwright/test';

test('user can sign up, log in, and see notes', async ({ page }) => {
  await page.goto('/');
  await page.click('a[href="/login"]');
  await page.fill('[name=email]', 'test@example.com');
  await page.fill('[name=password]', 'correct horse battery staple');
  await page.click('button[type=submit]');
  await expect(page).toHaveURL('/notes');
  await expect(page.getByText('Your notes')).toBeVisible();
});
```

A real browser; a real form submission; the real database. Slow (seconds
per test) but catches *integration* bugs.

Run with `pnpm exec playwright test`. CI uses `@playwright/test`'s
`@actions/setup-node` integration; locally you install Chromium once with
`pnpm exec playwright install --with-deps chromium`.

## Vitest with the DOM (component tests)

For component-level tests you can use `@testing-library/svelte`:

```ts
import { render, screen, fireEvent } from '@testing-library/svelte';
import Counter from './Counter.svelte';

it('increments', async () => {
  render(Counter);
  const btn = screen.getByRole('button');
  expect(btn).toHaveTextContent('clicks: 0');
  await fireEvent.click(btn);
  expect(btn).toHaveTextContent('clicks: 1');
});
```

Faster than Playwright; closer to the user than pure logic.

## What MemberClub web ships today

```
src/lib/server/auth/password.test.ts   5 vitest tests
```

The exercises add Playwright tests for login + notes flow.

## CI integration

```yaml
# .github/workflows/ci.yml (snippet from the matrix `web` job)
- run: pnpm install --frozen-lockfile
- run: pnpm check          # svelte-check
- run: pnpm test           # vitest
- run: pnpm build          # production build
```

Three commands; three checks. The `web` matrix expands to every SvelteKit
project under `projects/` and `apps/`.

## Why this matters

- **svelte-check is *the* cheapest test you have.** Type errors caught
  pre-commit save hours of debugging.
- **Vitest tests should target *pure functions*.** Anything touching the
  DOM goes to Playwright.
- **Playwright is the *integration* layer.** Slow but precise.

## Green-bar checkpoint

- You can run `pnpm test && pnpm check && pnpm build` and explain each.
- You can pick vitest vs Playwright vs svelte-check for a given concern.
- You can read a Playwright test and predict what the browser will do.

Next: `lessons/08-build-memberclub-web.md`.
