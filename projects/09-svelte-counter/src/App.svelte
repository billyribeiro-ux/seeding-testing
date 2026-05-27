<script lang="ts">
  // Phase 9 warm-up — exercising every Svelte 5 rune in one tiny page.
  //
  // The mental model: a rune is a primitive the compiler recognizes,
  // not a function. `$state` makes a value reactive; `$derived` builds
  // a value from other reactive values; `$effect` runs a side effect
  // after the DOM updates; `$props` declares incoming component props;
  // `$bindable` lets a parent two-way-bind to a child's state.
  //
  // No framework features below this layer are used — no SSR, no DB,
  // no $app/* modules. Just runes.

  import Counter from './Counter.svelte';
  import { history } from './history.svelte';

  // Local UI state for the demo page.
  let label = $state('clicks');

  // Reactivity demo: the "current pluralization" updates whenever
  // either `label` or the counter's value changes.
  let total = $state(0);
  let suffix = $derived(total === 1 ? '' : 's');

  // Side effect: log each new count to the in-memory history singleton.
  // `$effect` only re-runs when the values it READS change.
  $effect(() => {
    history.add(total);
  });
</script>

<main>
  <h1>Svelte 5 runes — a counter that exercises all five</h1>

  <label>
    Label:
    <input type="text" bind:value={label} />
  </label>

  <!--
    Two-way bind via $bindable: the Counter component owns its
    internal `value` $state, but exposes it as $bindable so this
    parent can read AND write it. Mutating `total` here updates the
    counter; clicking inside the counter updates `total`.
  -->
  <Counter bind:value={total} />

  <p data-testid="summary">
    <strong data-testid="count">{total}</strong> {label}{suffix}
  </p>

  <section>
    <h2>History</h2>
    <ol>
      {#each history.entries as entry (entry.id)}
        <li>{entry.value} at {entry.at}</li>
      {/each}
    </ol>
  </section>
</main>

<style>
  main {
    font-family: system-ui, sans-serif;
    max-width: 32rem;
    margin: 2rem auto;
    padding: 1rem;
  }
  label {
    display: block;
    margin-block: 1rem;
  }
  input {
    margin-inline-start: 0.5rem;
  }
  section {
    margin-top: 2rem;
    border-top: 1px solid #ccc;
    padding-top: 1rem;
  }
</style>
