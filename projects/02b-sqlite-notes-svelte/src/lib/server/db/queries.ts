// Pure query functions over the Drizzle schema.
//
// Splitting these out keeps the +page.server.ts file thin and makes the
// queries unit-testable via vitest with an in-memory SQLite instance.

import { desc, eq } from 'drizzle-orm';
import type { BetterSQLite3Database } from 'drizzle-orm/better-sqlite3';
import { notes, type Note, type NewNote } from './schema';

export async function listNotes(db: BetterSQLite3Database<{ notes: typeof notes }>): Promise<Note[]> {
  return db.select().from(notes).orderBy(desc(notes.id)).all();
}

export async function addNote(
  db: BetterSQLite3Database<{ notes: typeof notes }>,
  body: string
): Promise<Note> {
  const trimmed = body.trim();
  if (trimmed.length === 0) {
    throw new Error('body cannot be empty');
  }
  if (trimmed.length > 4096) {
    throw new Error('body cannot exceed 4096 characters');
  }
  const row: NewNote = { body: trimmed };
  const [inserted] = db.insert(notes).values(row).returning().all();
  return inserted;
}

export async function deleteNote(
  db: BetterSQLite3Database<{ notes: typeof notes }>,
  id: number
): Promise<boolean> {
  const result = db.delete(notes).where(eq(notes.id, id)).run();
  return result.changes > 0;
}
