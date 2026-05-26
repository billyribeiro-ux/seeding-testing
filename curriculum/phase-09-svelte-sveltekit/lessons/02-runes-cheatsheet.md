# Lesson 9.2 — Svelte 5 Runes Cheat Sheet

> **Concept first:** Svelte 5 ships *runes* — explicit functions that mark reactivity. Four cover 95% of usage: `$state`, `$derived`, `$effect`, `$props`.
> **Time:** 25 minutes.

## `$state` — reactive variables

```svelte
<script>
  let count = $state(0);
</script>

<button onclick={() => count++}>clicks: {count}</button>
```

`count` is just a number. You mutate it like any variable; the UI reacts.

Arrays and objects are **deeply** reactive — `state.value` updates trigger
re-renders. Class instances are not auto-proxied; declare `$state` on a field
inside the class.

When you don't want deep reactivity (large arrays you'll only replace
wholesale), use `$state.raw(...)`.

## `$derived` — computed values

```svelte
<script>
  let count = $state(0);
  let doubled = $derived(count * 2);
  let parity  = $derived(count % 2 === 0 ? 'even' : 'odd');
</script>
```

`doubled` updates automatically. The expression inside `$derived(...)` must
be *pure* — no side effects.

For more complex derivations, use `$derived.by(() => { … })`.

## `$effect` — side effects

```svelte
<script>
  let count = $state(0);
  $effect(() => {
    document.title = `Count: ${count}`;
  });
</script>
```

Runs after the DOM updates. Re-runs whenever its tracked dependencies (here:
`count`) change. Can return a cleanup function:

```svelte
$effect(() => {
  const id = setInterval(() => count++, 1000);
  return () => clearInterval(id);
});
```

**Effects are an escape hatch.** Don't use them to keep two pieces of state
in sync (use `$derived`); don't use them to react to events (use event
handlers). Reach for `$effect` for: third-party DOM libs, canvas, analytics
pings, timers.

## `$props` — component inputs

```svelte
<!-- Child.svelte -->
<script>
  let { name = 'world', children } = $props();
</script>

<h1>hello, {name}!</h1>
{@render children?.()}
```

`$props()` returns an object you usually destructure. Defaults via standard
JS destructuring. `children` is the equivalent of a slot — call it with
`{@render children?.()}` to render whatever the parent passed.

## When to use what

| Need | Rune |
|---|---|
| Plain reactive variable | `$state` |
| Computed from other state | `$derived` |
| Side effect (DOM, network, timer) | `$effect` |
| Component input | `$props` |
| Two-way binding | `$bindable` (advanced) |

## Migration mental shortcut from Svelte 4

| Svelte 4 | Svelte 5 |
|---|---|
| `let count = 0;` (auto-reactive in component) | `let count = $state(0);` |
| `$: doubled = count * 2;` | `let doubled = $derived(count * 2);` |
| `$: { console.log(count); }` | `$effect(() => console.log(count));` |
| `export let foo;` | `let { foo } = $props();` |
| `on:click={handler}` | `onclick={handler}` |
| `<slot />` | `{@render children?.()}` |

If you've seen Svelte 4 code, the rune syntax is a cleaner, more explicit
version of the same ideas.

## Reactivity gotchas

- **Destructuring breaks reactivity.** `let { x } = $derived(obj)` — `x` is
  a snapshot, not reactive. Use `let x = $derived(obj.x)` instead.
- **Effects only track sync reads.** A value read inside `setTimeout`
  inside an `$effect` is *not* tracked.
- **`$effect` runs only in the browser** (not during SSR).

## Where you'll see them in MemberClub

```svelte
<!-- /routes/notes/+page.svelte -->
<script lang="ts">
  let { data, form }: PageProps = $props();   // server-provided
  let draft = $state('');                      // local UI state
  let chars = $derived(draft.length);          // derived counter
</script>

<textarea bind:value={draft}></textarea>
<p>{chars}/4096</p>
```

Three lines, all four runes (`$state`, `$derived`, `$props`, and indirectly
`$bindable` via `bind:`). The whole component is a self-documenting reactive
graph.

## Why this matters

- **Runes are *explicit* reactivity.** No more "wait, is this reactive?"
  guessing — the rune tells you.
- **They compose.** A `$derived` based on `$props` based on `$state` is a
  chain of trackable values.
- **Effects sparingly.** Most "reactivity" needs are state + derived.

## Green-bar checkpoint

- You can write a counter component with `$state` + `$derived`.
- You can articulate when to use `$effect` and when *not* to.
- You can read a Svelte 4 component and rewrite it with runes.

Next: `lessons/03-routing-and-layouts.md`.
