# Lesson 4.3 — Routes, Nesting, and Versioning

> **Concept first:** a public API is a *contract*. Versioning the URL is the simplest way to evolve it without breaking clients.
> **Time:** 20 minutes.

## `nest("/v1", v1)`

```rust
let v1 = Router::new()
    .route("/notes",      get(list_notes).post(create_note))
    .route("/notes/{id}", get(get_note).patch(update_note).delete(delete_note));

let app = Router::new()
    .route("/healthz", get(health))
    .nest("/v1", v1);
```

Inside `v1` we write `/notes`; from outside the world sees `/v1/notes`. Nesting composes — `v1` can itself nest sub-routers.

`merge(other)` is the same shape with no prefix: combines two routers' routes at the same level. Use it for cross-cutting routers like a metrics router.

## Path parameters

```rust
.route("/notes/{id}", get(get_note))

async fn get_note(Path(id): Path<i64>) -> ... { /* id is parsed */ }
```

Axum 0.8 uses `{name}` (single curly braces) for path captures. Old Axum 0.7 used `:name`; we're past that.

Multiple captures:

```rust
.route("/orgs/{org_id}/users/{user_id}", get(get_user_in_org))

async fn get_user_in_org(Path((org_id, user_id)): Path<(i64, i64)>) -> ... {}
```

## Method routing

A single `.route()` accepts multiple methods chained:

```rust
.route("/notes", get(list_notes).post(create_note))
.route("/notes/{id}", get(get_note).patch(update).delete(delete))
```

If you send `DELETE /notes` (no id) you get a `405 Method Not Allowed` — Axum knows the method isn't bound for that path.

## Why version in the URL?

Two common schools:

| Approach | Pros | Cons |
|---|---|---|
| **URL path** (`/v1/...`, `/v2/...`) | Visible, cacheable, easy to debug, easy to evolve | URL changes for clients |
| **Header** (`Accept: application/vnd.app.v1+json`) | URL stays stable | Hidden; harder for `curl`; CDN cache keys must include the header |

Pick **URL path**. The capstone uses it. The trade-off matters less than just *being explicit*.

## When to bump the version

- **Don't bump for additive changes.** Adding a field, adding a route, adding a query param — these don't break clients.
- **Bump for removed/renamed/retyped fields.** Anything that breaks a deserializer is a breaking change.
- **Bump for changed semantics.** Same shape but different meaning is a breaking change.

In practice, well-designed APIs go a *long* time without a v2.

## Deprecation, not deletion

When a feature must be removed, follow this dance:

1. Add the replacement at v1 (additive).
2. Mark the old endpoint as deprecated. Emit a `Sunset` header (`Sunset: Sat, 1 Aug 2026 23:59:59 GMT`).
3. Add a `Deprecation` header (`Deprecation: true`).
4. Email customers; log usage so you can chase laggards.
5. After the sunset date, return `410 Gone`.

This is the IETF / RFC-8594 pattern. Real B2B SaaS lives by it.

## Why this matters

- **URL versions = forward-compatibility insurance.** Cheap to ship v1; cheap to add v2 later without breaking customers on v1.
- **Method routing = self-documenting.** `get().post()` on the same path is the REST contract distilled.
- **The Sunset/Deprecation flow** is what separates "we shipped" from "we maintain it."

## Green-bar checkpoint

- You can split a flat router into a versioned + base layout with `nest`.
- You can write a handler that takes two path params.
- You can articulate when adding a feature is *not* a breaking change.

Next: `lessons/04-extractors-and-validation.md`.
