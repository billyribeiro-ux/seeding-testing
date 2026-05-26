// Layout load function — runs for every page. Exposes the current user (or null)
// to all children as `data.user`.

import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ locals }) => {
  return { user: locals.user };
};
