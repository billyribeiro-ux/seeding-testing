<script lang="ts">
  import type { PageProps } from './$types';

  let { data }: PageProps = $props();

  // Demo of Svelte 5 runes — completely client-side state.
  let count = $state(0);
  let doubled = $derived(count * 2);
</script>

<svelte:head>
  <title>MemberClub — home</title>
</svelte:head>

<h1>Welcome to MemberClub</h1>

{#if data.user}
  <p>You're signed in as <strong>{data.user.email}</strong>. Head to <a href="/notes">your notes</a>.</p>
{:else}
  <p>Please <a href="/login">log in or sign up</a> to manage your notes.</p>
{/if}

<section class="runes">
  <h2>A taste of Svelte 5 runes</h2>
  <p>Click the button. The doubled value updates automatically.</p>
  <button onclick={() => (count += 1)}>count: {count}</button>
  <p>doubled: <code>{doubled}</code></p>
</section>

<style>
  .runes {
    margin-top: 2rem;
    padding: 1rem;
    background: #f7f7f7;
    border-radius: 8px;
  }
  button {
    padding: 0.5rem 1rem;
    border-radius: 6px;
    border: 1px solid #2563eb;
    background: #2563eb;
    color: white;
    cursor: pointer;
  }
</style>
