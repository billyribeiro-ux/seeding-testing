// Drizzle schema for the notes table. Lives under $lib/server so it can never
// be imported from a client component (SvelteKit refuses to bundle it).

import { sqliteTable, integer, text } from 'drizzle-orm/sqlite-core';
import { sql } from 'drizzle-orm';

export const notes = sqliteTable('notes', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  body: text('body').notNull(),
  createdAt: text('created_at')
    .notNull()
    .default(sql`(strftime('%Y-%m-%dT%H:%M:%fZ','now'))`)
});

export type Note = typeof notes.$inferSelect;
export type NewNote = typeof notes.$inferInsert;
