<script lang="ts">
  // $props declares the incoming-prop shape; $bindable marks `value`
  // as parent-writable so two-way binding works.
  //
  // The `min` and `step` props default at the prop declaration line —
  // idiomatic Svelte 5; no separate `export let` boilerplate.
  let {
    value = $bindable(0),
    min = Number.NEGATIVE_INFINITY,
    step = 1,
  }: {
    value?: number;
    min?: number;
    step?: number;
  } = $props();

  function inc() {
    value = value + step;
  }
  function dec() {
    if (value - step >= min) {
      value = value - step;
    }
  }
  function reset() {
    value = 0;
  }
</script>

<div class="counter">
  <button type="button" data-testid="dec" onclick={dec}>−</button>
  <span data-testid="value">{value}</span>
  <button type="button" data-testid="inc" onclick={inc}>+</button>
  <button type="button" data-testid="reset" onclick={reset}>reset</button>
</div>

<style>
  .counter {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-block: 1rem;
  }
  button {
    padding: 0.25rem 0.75rem;
    cursor: pointer;
  }
  span {
    min-width: 3rem;
    display: inline-block;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
</style>
