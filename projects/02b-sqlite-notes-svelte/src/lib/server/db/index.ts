// The DB connection. Created once per process. Exposed as `db`.
//
// IMPORTANT: this file lives under $lib/server. SvelteKit will throw at build
// time if any client-side component imports anything from here, so the SQLite
// driver never reaches the browser bundle.

import Database from 'better-sqlite3';
import { drizzle } from 'drizzle-orm/better-sqlite3';
import { env } from '$env/dynamic/private';
import * as schema from './schema';

const url = env.DATABASE_URL ?? './dev.sqlite';

const sqlite = new Database(url);
sqlite.pragma('journal_mode = WAL');
sqlite.pragma('foreign_keys = ON');

export const db = drizzle(sqlite, { schema });
export { schema };
