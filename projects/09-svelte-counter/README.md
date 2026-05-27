# 09-svelte-counter — Phase 9 warm-up

The smallest possible Svelte 5 app that exercises every rune:

- `$state` — make a value reactive (in components AND in `.svelte.ts`).
- `$derived` — compute a value from other reactive values.
- `$effect` — run a side effect after the DOM updates.
- `$props` — declare incoming component props.
- `$bindable` — let the parent two-way-bind to a child's state.

No SSR, no database, no `$app/*` modules — just runes. Once you can
read every line here, the rest of SvelteKit is "the framework on top
that wires routing, server data loading, and form actions into the
same reactive model."

## Run

```bash
cd projects/09-svelte-counter
pnpm install
pnpm dev      # http://localhost:5173
pnpm test     # vitest + @testing-library/svelte
```

## What to read in order

1. `src/App.svelte` — the top-level component. Shows `$state`,
   `$derived`, `$effect`, and a two-way `bind:value` to a child.
2. `src/Counter.svelte` — child component. Shows `$props` and
   `$bindable` defaulting, plus three event handlers (`onclick`
   instead of legacy `on:click`).
3. `src/history.svelte.ts` — a module-level reactive singleton. Shows
   that `$state` works outside components too (the modern replacement
   for the legacy `writable()` store).

## Tests

`tests/counter.test.ts` mounts the Counter with `@testing-library/svelte`
and asserts:

- the displayed value reflects the bound `value` prop,
- clicking `+` increments,
- clicking `−` decrements,
- the `min` prop floors the decrement,
- clicking `reset` returns the value to 0.

These are unit tests on the rune-driven component — the kind of test
that catches a reactivity bug before it ships to SvelteKit-land.

## Why this exists

Per Phase 9 of the curriculum, you can't really learn SvelteKit before
you've internalized the Svelte 5 reactivity model. SvelteKit is "Svelte
+ a router + load functions + form actions" — and every one of those
features assumes you already think in runes. This project is the
runes-only sandbox where you can experiment without any other moving
parts.
