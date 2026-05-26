// See https://svelte.dev/docs/kit/types#app
declare global {
  namespace App {
    interface Locals {
      /** Populated by hooks.server.ts when a valid session cookie is present. */
      user: import('$lib/server/db/schema').User | null;
    }
    interface PageData {
      user: import('$lib/server/db/schema').User | null;
    }
  }
}
export {};
