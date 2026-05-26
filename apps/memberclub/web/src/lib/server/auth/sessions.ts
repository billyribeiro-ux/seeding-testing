// Session helpers. Same pattern as auth-demo: cookie carries the plaintext
// token; the DB stores SHA-256(token).

import { createHash, randomBytes } from 'node:crypto';
import { eq, and, isNull, gt } from 'drizzle-orm';
import { db } from '$lib/server/db';
import { sessions, users } from '$lib/server/db/schema';
import type { Session, User } from '$lib/server/db/schema';

export const SESSION_COOKIE = 'memberclub_session';
export const SESSION_TTL_SECONDS = 30 * 24 * 60 * 60; // 30 days

export function generateSessionToken(): string {
  return randomBytes(32).toString('base64url');
}

export function tokenHash(token: string): string {
  return createHash('sha256').update(token).digest('hex');
}

export function createSession(userId: number): { token: string; session: Session } {
  const token = generateSessionToken();
  const expiresAt = new Date(Date.now() + SESSION_TTL_SECONDS * 1000).toISOString();
  const [session] = db
    .insert(sessions)
    .values({ tokenHash: tokenHash(token), userId, expiresAt })
    .returning()
    .all();
  return { token, session };
}

export function findUserBySession(
  token: string | undefined
): { user: User; session: Session } | null {
  if (!token) return null;
  const now = new Date().toISOString();
  const rows = db
    .select({ session: sessions, user: users })
    .from(sessions)
    .innerJoin(users, eq(users.id, sessions.userId))
    .where(
      and(eq(sessions.tokenHash, tokenHash(token)), isNull(sessions.revokedAt), gt(sessions.expiresAt, now))
    )
    .all();
  if (rows.length === 0) return null;
  return rows[0];
}

export function revokeSession(token: string): void {
  db.update(sessions)
    .set({ revokedAt: new Date().toISOString() })
    .where(eq(sessions.tokenHash, tokenHash(token)))
    .run();
}
