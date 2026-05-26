import { fail, redirect } from '@sveltejs/kit';
import { eq } from 'drizzle-orm';
import { db } from '$lib/server/db';
import { users } from '$lib/server/db/schema';
import { hashPassword, verifyPassword } from '$lib/server/auth/password';
import { createSession, SESSION_COOKIE, SESSION_TTL_SECONDS } from '$lib/server/auth/sessions';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ locals }) => {
  if (locals.user) throw redirect(303, '/notes');
  return {};
};

function validateEmail(s: string): boolean {
  return s.length > 3 && s.length < 254 && s.includes('@') && !s.includes(' ');
}

function validatePassword(s: string): boolean {
  return s.length >= 8;
}

export const actions: Actions = {
  register: async ({ request, cookies }) => {
    const data = await request.formData();
    const email = String(data.get('email') ?? '').trim();
    const password = String(data.get('password') ?? '');

    if (!validateEmail(email)) {
      return fail(400, { email, error: 'invalid email' });
    }
    if (!validatePassword(password)) {
      return fail(400, { email, error: 'password must be at least 8 characters' });
    }

    const existing = db.select().from(users).where(eq(users.email, email)).all();
    if (existing.length > 0) {
      return fail(409, { email, error: 'email already registered' });
    }

    const [user] = db
      .insert(users)
      .values({ email, passwordHash: hashPassword(password) })
      .returning()
      .all();
    const { token } = createSession(user.id);
    cookies.set(SESSION_COOKIE, token, {
      path: '/',
      httpOnly: true,
      sameSite: 'lax',
      secure: process.env.NODE_ENV === 'production',
      maxAge: SESSION_TTL_SECONDS
    });
    throw redirect(303, '/notes');
  },

  login: async ({ request, cookies }) => {
    const data = await request.formData();
    const email = String(data.get('email') ?? '').trim();
    const password = String(data.get('password') ?? '');

    if (!validateEmail(email) || !validatePassword(password)) {
      return fail(400, { email, error: 'invalid credentials' });
    }

    const found = db.select().from(users).where(eq(users.email, email)).all();
    if (found.length === 0) {
      // Match the timing of a real verify so we don't leak existence.
      hashPassword(password);
      return fail(401, { email, error: 'invalid credentials' });
    }
    const user = found[0];
    if (!verifyPassword(password, user.passwordHash)) {
      return fail(401, { email, error: 'invalid credentials' });
    }
    const { token } = createSession(user.id);
    cookies.set(SESSION_COOKIE, token, {
      path: '/',
      httpOnly: true,
      sameSite: 'lax',
      secure: process.env.NODE_ENV === 'production',
      maxAge: SESSION_TTL_SECONDS
    });
    throw redirect(303, '/notes');
  }
};
