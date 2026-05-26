// Server-only DB singleton. SvelteKit refuses to bundle this into client code
// because the file is under `$lib/server/`.

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
