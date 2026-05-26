<script lang="ts">
  import type { LayoutProps } from './$types';

  let { data, children }: LayoutProps = $props();
</script>

<header>
  <nav>
    <a href="/">MemberClub</a>
    <div>
      {#if data.user}
        <span class="who">hi, {data.user.email}</span>
        <a href="/notes">notes</a>
        <form method="POST" action="/logout" class="inline">
          <button type="submit" class="link">log out</button>
        </form>
      {:else}
        <a href="/login">log in</a>
      {/if}
    </div>
  </nav>
</header>

<main>
  {@render children?.()}
</main>

<style>
  header {
    background: #111;
    color: white;
    padding: 0.5rem 1rem;
  }
  nav {
    display: flex;
    justify-content: space-between;
    align-items: center;
    max-width: 60rem;
    margin: 0 auto;
  }
  nav a {
    color: white;
    text-decoration: none;
    margin-right: 0.75rem;
  }
  nav a:hover {
    text-decoration: underline;
  }
  .who {
    margin-right: 0.5rem;
    color: #aaa;
  }
  .inline {
    display: inline;
  }
  button.link {
    background: none;
    border: none;
    color: white;
    cursor: pointer;
    padding: 0;
    font: inherit;
    text-decoration: underline;
  }
  main {
    max-width: 60rem;
    margin: 2rem auto;
    padding: 0 1rem;
    font-family: system-ui, sans-serif;
    line-height: 1.5;
  }
</style>
