import { redirect } from '@sveltejs/kit';
import { revokeSession, SESSION_COOKIE } from '$lib/server/auth/sessions';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ cookies }) => {
  const token = cookies.get(SESSION_COOKIE);
  if (token) revokeSession(token);
  cookies.delete(SESSION_COOKIE, { path: '/' });
  throw redirect(303, '/');
};
