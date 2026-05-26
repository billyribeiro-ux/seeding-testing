<script lang="ts">
  import type { PageProps } from './$types';

  let { form }: PageProps = $props();
  let mode: 'login' | 'register' = $state('login');
</script>

<svelte:head>
  <title>MemberClub — {mode === 'login' ? 'log in' : 'sign up'}</title>
</svelte:head>

<h1>{mode === 'login' ? 'Log in' : 'Sign up'}</h1>

{#if form?.error}
  <p class="error">⚠ {form.error}</p>
{/if}

<form method="POST" action={mode === 'login' ? '?/login' : '?/register'}>
  <label>
    <span>Email</span>
    <input type="email" name="email" required value={form?.email ?? ''} />
  </label>
  <label>
    <span>Password</span>
    <input type="password" name="password" required minlength="8" />
  </label>
  <button type="submit">{mode === 'login' ? 'Log in' : 'Sign up'}</button>
</form>

<p>
  {#if mode === 'login'}
    No account?
    <button type="button" class="link" onclick={() => (mode = 'register')}>Sign up</button>
  {:else}
    Already have one?
    <button type="button" class="link" onclick={() => (mode = 'login')}>Log in</button>
  {/if}
</p>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    max-width: 22rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  input {
    padding: 0.5rem;
    border: 1px solid #ccc;
    border-radius: 6px;
    font: inherit;
  }
  button {
    padding: 0.5rem 1rem;
    border-radius: 6px;
    border: 1px solid #2563eb;
    background: #2563eb;
    color: white;
    cursor: pointer;
    font: inherit;
  }
  button.link {
    background: none;
    border: none;
    color: #2563eb;
    text-decoration: underline;
    padding: 0;
    font: inherit;
    cursor: pointer;
  }
  .error {
    color: #b00020;
    background: #fdecea;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    max-width: 22rem;
  }
</style>
