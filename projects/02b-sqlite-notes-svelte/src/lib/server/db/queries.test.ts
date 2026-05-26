// Unit tests for the query layer. Uses an in-memory SQLite instance so
// every test is hermetic — no shared state, no file cleanup.

import { describe, it, expect, beforeEach } from 'vitest';
import Database from 'better-sqlite3';
import { drizzle, type BetterSQLite3Database } from 'drizzle-orm/better-sqlite3';
import { sql } from 'drizzle-orm';
import { notes } from './schema';
import { addNote, deleteNote, listNotes } from './queries';

function makeDb(): BetterSQLite3Database<{ notes: typeof notes }> {
  const sqlite = new Database(':memory:');
  sqlite.exec(`
    CREATE TABLE notes (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      body TEXT NOT NULL,
      created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
  `);
  return drizzle(sqlite, { schema: { notes } });
}

describe('queries', () => {
  let db: BetterSQLite3Database<{ notes: typeof notes }>;
  beforeEach(() => {
    db = makeDb();
  });

  it('lists notes newest-first', async () => {
    await addNote(db, 'first');
    await addNote(db, 'second');
    const items = await listNotes(db);
    expect(items.map((n) => n.body)).toEqual(['second', 'first']);
  });

  it('trims whitespace from body', async () => {
    const note = await addNote(db, '   hello   ');
    expect(note.body).toBe('hello');
  });

  it('rejects empty body', async () => {
    await expect(addNote(db, '   ')).rejects.toThrow(/empty/);
  });

  it('rejects body over 4096 chars', async () => {
    const long = 'a'.repeat(4097);
    await expect(addNote(db, long)).rejects.toThrow(/4096/);
  });

  it('deletes returns true when row existed', async () => {
    const n = await addNote(db, 'gone');
    expect(await deleteNote(db, n.id)).toBe(true);
    expect(await listNotes(db)).toHaveLength(0);
  });

  it('deletes returns false for missing id', async () => {
    expect(await deleteNote(db, 99_999)).toBe(false);
  });

  it('sets created_at automatically', async () => {
    const n = await addNote(db, 'has timestamp');
    expect(typeof n.createdAt).toBe('string');
    expect(n.createdAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });
});
