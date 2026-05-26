// The server-side endpoint for /. Runs only on the server (Node/SvelteKit).
//
// `load` produces page data; `actions` handle form POSTs.

import { fail } from '@sveltejs/kit';
import { db } from '$lib/server/db';
import { listNotes, addNote, deleteNote } from '$lib/server/db/queries';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async () => {
  const items = await listNotes(db);
  return { notes: items };
};

export const actions: Actions = {
  create: async ({ request }) => {
    const data = await request.formData();
    const body = String(data.get('body') ?? '');
    try {
      const n = await addNote(db, body);
      return { ok: true, created: n.id };
    } catch (e) {
      const message = e instanceof Error ? e.message : 'unknown error';
      return fail(400, { body, error: message });
    }
  },
  delete: async ({ request }) => {
    const data = await request.formData();
    const id = Number(data.get('id'));
    if (!Number.isInteger(id) || id <= 0) {
      return fail(400, { error: 'invalid id' });
    }
    const removed = await deleteNote(db, id);
    if (!removed) {
      return fail(404, { error: 'note not found' });
    }
    return { ok: true, deleted: id };
  }
};
