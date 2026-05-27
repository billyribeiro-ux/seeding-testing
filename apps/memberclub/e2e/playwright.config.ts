import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright config for the MemberClub e2e suite.
 *
 * Two modes:
 *   * Local: assumes the API (port 3000) and web (port 5173) are already
 *     running. Run them via `make e2e-up` before `pnpm test`.
 *   * CI: the workflow boots both via `docker compose -f
 *     apps/memberclub/infra/compose.prod.yaml up -d` and points the
 *     suite at the Caddy front door.
 */

const BASE_URL = process.env.MEMBERCLUB_BASE_URL ?? 'http://localhost:5173';

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  expect: { timeout: 5_000 },

  // Retries: bumping flake tolerance on CI only; local runs fail fast.
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,

  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',

  use: {
    baseURL: BASE_URL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    // Mobile suite is a stretch goal — uncomment when the web app is
    // mobile-responsive end-to-end.
    // {
    //   name: 'Mobile Safari',
    //   use: { ...devices['iPhone 14'] },
    // },
  ],
});
