import { error, fail, redirect } from '@sveltejs/kit';
import { and, desc, eq } from 'drizzle-orm';
import { db } from '$lib/server/db';
import { notes } from '$lib/server/db/schema';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ locals }) => {
  if (!locals.user) throw redirect(303, '/login');
  const items = db.select().from(notes).where(eq(notes.userId, locals.user.id)).orderBy(desc(notes.id)).all();
  return { notes: items };
};

export const actions: Actions = {
  create: async ({ request, locals }) => {
    if (!locals.user) throw error(401, 'unauthorized');
    const data = await request.formData();
    const body = String(data.get('body') ?? '').trim();
    if (body.length === 0) return fail(400, { error: 'body cannot be empty' });
    if (body.length > 4096) return fail(400, { error: 'body too long (max 4096)' });
    db.insert(notes).values({ userId: locals.user.id, body }).run();
    return { ok: true };
  },

  delete: async ({ request, locals }) => {
    if (!locals.user) throw error(401, 'unauthorized');
    const data = await request.formData();
    const id = Number(data.get('id'));
    if (!Number.isInteger(id) || id <= 0) return fail(400, { error: 'invalid id' });
    const r = db.delete(notes).where(and(eq(notes.id, id), eq(notes.userId, locals.user.id))).run();
    if (r.changes === 0) return fail(404, { error: 'not found' });
    return { ok: true };
  }
};
