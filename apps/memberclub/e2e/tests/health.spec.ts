import { expect, test } from '@playwright/test';

/**
 * Smoke: the API health probe and the web index both respond.
 *
 * This is the lowest-effort test that proves the whole stack stood up.
 * Every other suite in this directory assumes these two paths work.
 */

test('API /healthz returns ok', async ({ request }) => {
  const apiBase = process.env.MEMBERCLUB_API_BASE_URL ?? 'http://localhost:3000';
  const res = await request.get(`${apiBase}/healthz`);
  expect(res.status()).toBe(200);
  const body = await res.json();
  expect(body.status).toBe('ok');
});

test('web index renders', async ({ page }) => {
  await page.goto('/');
  // The home page should at minimum have a <title> — the fully styled
  // landing page is built across Phase 9 lessons.
  await expect(page).toHaveTitle(/MemberClub|SvelteKit/i);
});
