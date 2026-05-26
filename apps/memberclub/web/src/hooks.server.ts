// SvelteKit hooks — run on every request before the handler.
// We use this to populate `event.locals.user` from the session cookie.

import { findUserBySession, SESSION_COOKIE } from '$lib/server/auth/sessions';
import type { Handle } from '@sveltejs/kit';

export const handle: Handle = async ({ event, resolve }) => {
  const token = event.cookies.get(SESSION_COOKIE);
  const found = findUserBySession(token);
  event.locals.user = found ? found.user : null;
  return resolve(event);
};
