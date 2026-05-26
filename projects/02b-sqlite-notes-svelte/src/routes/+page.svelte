<script lang="ts">
  import type { PageProps } from './$types';

  let { data, form }: PageProps = $props();
</script>

<svelte:head>
  <title>SQLite Notes</title>
</svelte:head>

<main>
  <h1>Notes</h1>

  {#if form?.error}
    <p class="error">⚠ {form.error}</p>
  {/if}

  <form method="POST" action="?/create">
    <label>
      <span>New note</span>
      <textarea name="body" rows="3" maxlength="4096" required placeholder="Write something…"
      ></textarea>
    </label>
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
      <li class="empty">No notes yet. Add one above.</li>
    {/each}
  </ul>
</main>

<style>
  main {
    max-width: 36rem;
    margin: 2rem auto;
    padding: 0 1rem;
    font-family: system-ui, sans-serif;
    line-height: 1.5;
  }
  h1 {
    border-bottom: 1px solid #e5e5e5;
    padding-bottom: 0.5rem;
  }
  .error {
    color: #b00020;
    background: #fdecea;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
  }
  form {
    margin: 1rem 0 1.5rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  textarea {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid #ccc;
    border-radius: 6px;
    font-family: inherit;
    font-size: 1rem;
    box-sizing: border-box;
  }
  button {
    margin-top: 0.5rem;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    border: 1px solid #2563eb;
    background: #2563eb;
    color: white;
    cursor: pointer;
    font-size: 0.95rem;
  }
  button.link {
    margin: 0;
    padding: 0;
    background: transparent;
    border: none;
    color: #b00020;
    text-decoration: underline;
    cursor: pointer;
    font-size: 0.85rem;
  }
  ul.notes {
    list-style: none;
    padding: 0;
    margin: 0;
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
  .empty {
    color: #888;
    text-align: center;
    padding: 1rem;
  }
</style>
