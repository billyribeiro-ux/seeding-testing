// A small reactive singleton — demonstrates that $state works in
// regular `.svelte.ts` files (not just inside components). This is the
// pattern Svelte 5 recommends for "module-level reactive store" in
// place of the legacy `writable()`.

export interface Entry {
  readonly id: number;
  readonly value: number;
  readonly at: string;
}

class History {
  // The reactive array — pushes to it trigger re-render in any
  // component that reads it.
  entries = $state<Entry[]>([]);

  private nextId = 1;

  add(value: number): void {
    this.entries.push({
      id: this.nextId++,
      value,
      at: new Date().toLocaleTimeString(),
    });
    // Keep memory bounded so a script-mashing demo doesn't grow forever.
    if (this.entries.length > 20) {
      this.entries.shift();
    }
  }

  clear(): void {
    this.entries = [];
  }
}

export const history = new History();
