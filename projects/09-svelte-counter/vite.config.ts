import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: !process.env.VITEST })],
  // Svelte 5 + vitest 2: explicitly opt into the browser-side resolve so
  // `mount()` (the runtime entry point) is the one resolved, not the
  // server-side stub that throws lifecycle_function_unavailable.
  resolve: {
    conditions: ['browser'],
  },
  test: {
    environment: 'jsdom',
    globals: true,
    server: {
      deps: {
        inline: [/^svelte/],
      },
    },
  },
});
