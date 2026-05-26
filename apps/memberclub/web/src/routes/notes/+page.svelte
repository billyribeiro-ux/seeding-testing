<script lang="ts">
  import type { PageProps } from './$types';

  let { data, form }: PageProps = $props();
  let draft = $state('');
  let chars = $derived(draft.length);
</script>

<svelte:head>
  <title>MemberClub — notes</title>
</svelte:head>

<h1>Your notes</h1>

{#if form?.error}
  <p class="error">⚠ {form.error}</p>
{/if}

<form method="POST" action="?/create" onsubmit={() => (draft = '')}>
  <label>
    <span>New note</span>
    <textarea name="body" rows="3" maxlength="4096" required bind:value={draft} placeholder="Write something…"
    ></textarea>
  </label>
  <p class="hint">{chars}/4096</p>
  <button type="submit">Add</button>
</form>

<ul class="notes">
  {#each data.notes as note (note.id)}
    <li>
      <article>
        <header>
          <small>#{note.id} — {note.createdAt}</small>
        </header>
        <p>{note.body}</p>
        <form method="POST" action="?/delete">
          <input type="hidden" name="id" value={note.id} />
          <button type="submit" class="link">delete</button>
        </form>
      </article>
    </li>
  {:else}
    <li class="empty">No notes yet.</li>
  {/each}
</ul>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 36rem;
    margin-bottom: 2rem;
  }
  textarea {
    padding: 0.5rem;
    border: 1px solid #ccc;
    border-radius: 6px;
    font: inherit;
  }
  .hint {
    margin: 0;
    color: #888;
    font-size: 0.8rem;
  }
  button {
    padding: 0.5rem 1rem;
    border-radius: 6px;
    border: 1px solid #2563eb;
    background: #2563eb;
    color: white;
    cursor: pointer;
    font: inherit;
    align-self: flex-start;
  }
  ul.notes {
    list-style: none;
    padding: 0;
  }
  ul.notes li {
    margin: 0.5rem 0;
  }
  ul.notes article {
    border: 1px solid #e5e5e5;
    border-radius: 6px;
    padding: 0.75rem 1rem;
    background: #fafafa;
  }
  ul.notes header {
    color: #888;
    font-size: 0.8rem;
    margin-bottom: 0.25rem;
  }
  button.link {
    background: none;
    border: none;
    color: #b00020;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .error {
    color: #b00020;
    background: #fdecea;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    max-width: 22rem;
  }
  .empty {
    color: #888;
  }
</style>
