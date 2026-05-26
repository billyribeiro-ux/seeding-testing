# Lesson 4.8 — Pagination: Offset is a Lie, Keyset is the Truth

> **Concept first:** `OFFSET 1000 LIMIT 20` looks innocent and is a performance nightmare. Keyset pagination is the cure.
> **Time:** 20 minutes.

## What offset pagination is

```sql
SELECT * FROM notes ORDER BY created_at DESC LIMIT 20 OFFSET 1000;
```

The DB has to *physically read and discard* the first 1000 rows just to get to row 1001. As the offset grows, the query slows. By page 100 on a real table you're at hundreds of milliseconds; by page 1000 you've timed out.

A second problem: if rows are added or deleted between page loads, the user sees *the same row twice* (or misses one). Offset is unstable under writes.

## Keyset pagination

Use the *last value* of the sort key as the bookmark:

```sql
SELECT id, body, created_at
FROM notes
WHERE (created_at, id) < ($1, $2)        -- continue strictly after the last seen
ORDER BY created_at DESC, id DESC
LIMIT 20;
```

The query uses the index directly — `LIMIT 20` is `O(20)` no matter how deep into the list you go. Adding/removing rows above the cursor never causes a duplicate or a skip.

## The cursor encoding

We don't expose `(created_at, id)` to clients directly. We *encode* them into an opaque `cursor` string:

```
cursor = base64url({"c":"2026-05-26T16:50:00.000Z","i":42})
```

Three benefits:

1. Clients treat it as opaque — they pass it back unchanged. We can change the encoding later.
2. It's URL-safe and copy-pasteable.
3. It encodes *both* sort keys at once, simplifying queries.

## API shape

```http
GET /v1/notes?limit=20
{
  "items": [...],
  "next":  "eyJjIjoiMjAyNi0wNS0yNlQxNjo1MDowMC4wMDBaIiwiaSI6NDJ9"
}
```

Then:

```http
GET /v1/notes?limit=20&cursor=eyJjIjoiMjAyNi…
{
  "items": [...],
  "next":  "…"   (or null if no more)
}
```

`next` is `null` when fewer than `limit` rows are returned.

## When offset is acceptable

- **Tiny lists** with hard upper bounds. Settings menus, admin dropdowns with < 1000 items.
- **Search results** where the user expects "page numbers" UX. Even then, cap at page 100; redirect to "refine your search" beyond.

Otherwise: **keyset**.

## In `notes-api` today

The capstone currently uses a simple `?limit=N` parameter and returns the entire list. That's intentional for the lesson: keyset is in the EXERCISES (E4.5) for you to implement.

The migration adds:

```sql
CREATE INDEX notes_created_at_id_idx ON notes (created_at DESC, id DESC);
```

(`id DESC` resolves ties when two rows share a `created_at`.)

## Why this matters

- **Pagination is the most common source of "the API is slow" reports.** Keyset is one of the highest-impact fixes in any junior-to-senior progression.
- **Cursors are opaque on purpose.** You'll thank yourself in 18 months when you want to change the sort key.
- **Indexes pay for keyset.** Plan the index when you plan the endpoint.

## Green-bar checkpoint

- You can rewrite an offset query as a keyset query.
- You can describe the cursor encoding and the trade-off of opacity.
- You can name two situations where offset is *fine* and articulate why.

Next: `lessons/09-build-notes-api.md`.
