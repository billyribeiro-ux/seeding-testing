# Lesson 7.2 — The RBAC Schema

> **Concept first:** four tables (or three, if you skip role groups). They model "users hold roles, roles grant permissions." Indexes make lookups O(1)-ish.
> **Time:** 20 minutes.

## The canonical shape

```sql
CREATE TABLE roles (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE       -- 'member', 'moderator', 'admin'
);

CREATE TABLE permissions (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE       -- 'doc.read', 'doc.write', 'doc.delete'
);

CREATE TABLE role_permissions (
    role_id       INTEGER NOT NULL REFERENCES roles(id)       ON DELETE CASCADE,
    permission_id INTEGER NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE user_roles (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    scope   TEXT,                                 -- NULL = global; else an org_id, project_id, etc.
    PRIMARY KEY (user_id, role_id, scope)
);
CREATE INDEX user_roles_user_id_idx ON user_roles (user_id);
```

Four tables. Two with one column each (effectively enums); two with foreign-key pairs (junctions).

## Why use IDs instead of strings?

You *could* store the role/permission names directly in `user_roles`:

```sql
CREATE TABLE user_roles (
    user_id INTEGER NOT NULL,
    role_name TEXT NOT NULL,                   -- denormalized
    ...
);
```

Cheaper to query (no join), but:
- Renaming a role becomes a migration of every user.
- Typos slip in (`"admins"` vs `"admin"`).
- Listing all roles is a `DISTINCT` over a large table.

Stick with normalized IDs. The extra join is microseconds.

## Scope: "admin of *what*"

The `scope` column on `user_roles` lets the same role apply to different resources:

```
user_id=42, role_id=admin, scope=NULL          → global admin
user_id=42, role_id=admin, scope='org:7'       → org 7's admin only
user_id=42, role_id=member, scope='org:7'      → also a member of org 7
```

When a request arrives in the context of a specific org, the policy code:

1. Loads `user_roles` rows where `user_id = $1 AND (scope IS NULL OR scope = $2)`.
2. Treats `NULL` scope as "applies everywhere."
3. Treats matching-string scope as "applies here."

For single-tenant apps, `scope` is always `NULL` and you can omit it. Add it the day you go multi-tenant.

## Seeding roles + permissions

A canonical set is bootstrapped on first migration:

```sql
INSERT INTO roles (id, name) VALUES (1, 'member'), (2, 'moderator'), (3, 'admin'), (4, 'owner');

INSERT INTO permissions (id, name) VALUES
    (1, 'doc.read'),
    (2, 'doc.write'),
    (3, 'doc.delete'),
    (4, 'doc.publish'),
    (5, 'user.impersonate'),
    (6, 'admin.users.list');

INSERT INTO role_permissions (role_id, permission_id) VALUES
    (1, 1),                          -- member: doc.read
    (2, 1), (2, 2),                  -- moderator: doc.read, doc.write
    (3, 1), (3, 2), (3, 3),(3, 4),   -- admin: read/write/delete/publish
    (3, 6),                          -- admin: users.list
    (4, 1), (4, 2), (4, 3), (4, 4), (4, 5), (4, 6);   -- owner: everything
```

Pin IDs explicitly so application code can refer to `roles::ADMIN` constants and not pray to alphabetical order.

## The fast lookup

To answer "does user X have permission Y?":

```sql
SELECT 1
FROM user_roles ur
JOIN role_permissions rp ON rp.role_id = ur.role_id
JOIN permissions       p  ON p.id      = rp.permission_id
WHERE ur.user_id = $1 AND p.name = $2
LIMIT 1;
```

With the `user_roles_user_id_idx` index, this is one index seek plus two trivial joins. Cache it in the request scope if you check many permissions for one user.

## Caching strategies

The naive "one query per permission check" is fine up to a few hundred QPS per user. Past that:

- **Per-request cache.** Load all the user's permissions on first check; serve subsequent checks from a `HashSet<String>` in the request extensions.
- **In-process cache** with TTL.
- **External cache** (Redis) for distributed systems.

Make sure the cache is invalidated on role changes. Cheap fix: include `users.updated_at` in the cache key.

## Permission-name conventions

Tiny but important:

- **Dot-separated namespace.** `doc.read`, `user.impersonate`, `admin.users.list`. Consistent levels.
- **Verb-last.** `doc.read`, not `read.doc`.
- **`admin.*`** for super-permissions, by convention. Useful for audit dashboards.
- **No abbreviations.** `permission` not `perm`; `delete` not `del`.

These conventions are mechanical, free to apply, and they pay off when your permission set is hundreds of rows.

## Why this matters

- **A normalized RBAC schema is forward-compatible.** Adding a role doesn't change application code; adding a permission requires one row plus the code that checks it.
- **Scope makes RBAC multi-tenancy-aware.** You can layer per-tenant grants on top of global roles in one schema.
- **The fast lookup is O(1) with the right indexes.** No "RBAC made my API slow" story.

## Green-bar checkpoint

- You can sketch the four-table RBAC schema from memory.
- You can articulate the trade-off between `user_roles.role_name` (string) and `user_roles.role_id` (FK).
- You can write the fast lookup SQL.

Next: `lessons/03-abac-policies.md`.
