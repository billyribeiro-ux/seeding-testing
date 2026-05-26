# Lesson 7.6 — Admin Bootstrap and Role Grants

> **Concept first:** the *first* admin is the boot problem. After that, admins promote others via a controlled, audited path.
> **Time:** 15 minutes.

## The first-admin problem

A fresh deployment has zero users → zero admins. You can't promote yourself without being an admin. Three solutions, ordered by preference:

### 1. First-user-becomes-admin

The first row inserted into `users` gets `roles::ADMIN` automatically:

```rust
async fn register(...) {
    let mut tx = pool.begin().await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&mut *tx).await?;
    let user = insert_user(&mut tx, ...).await?;
    if count == 0 {
        grant_role(&mut tx, user.id, Role::Admin, None).await?;
        audit(&mut tx, Some(user.id), "user.first_admin", None).await?;
    }
    tx.commit().await?;
    ...
}
```

Simple, no extra ceremony. Risk: someone races the first signup. Mitigation: bind this to a deploy-time `BOOTSTRAP_TOKEN` env var that must match the first user's signup request.

### 2. CLI grant

A standalone binary:

```bash
DATABASE_URL=postgres://... cargo run -p rbac-cli -- grant alice@b.com admin
```

The CLI requires DB access (which only operators have). No HTTP endpoint can reach this; the operator must SSH into the box or use `flyctl ssh console`.

### 3. Manual SQL

```sql
INSERT INTO user_roles (user_id, role_id) VALUES (1, (SELECT id FROM roles WHERE name = 'admin'));
```

Fine for the very first admin in a dev environment. Don't do it in production without audit.

In the capstone we use **option 1 + option 2**: first-user-becomes-admin for dev/demo, CLI for production.

## The CLI

```rust
#[derive(Parser)]
enum Cmd {
    Grant { email: String, role: String },
    Revoke { email: String, role: String },
    List { #[arg(long)] role: Option<String> },
    Promote { email: String },          // shortcut: grant admin
}
```

Each command:

1. Resolves the email to a user id.
2. Resolves the role name to a role id.
3. `INSERT` or `DELETE` into `user_roles`.
4. Writes an audit row with `action = "role.granted"`, `actor_id = NULL` (system) or the operator's id (if we wire that in).

## Granting through the API

For *human-friendly* role management (e.g. an admin dashboard), we expose:

```
POST /v1/admin/users/{id}/roles  { "role": "moderator" }
DELETE /v1/admin/users/{id}/roles/{role}
```

Both require `Forbidden::NotAdmin` + a recent step-up TOTP check.

The endpoint:

```rust
async fn grant_role(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Path(target_user_id): Path<i64>,
    Json(body): Json<GrantBody>,
) -> Result<StatusCode, ApiError> {
    require!(policy::can_grant_role(&user.0, &ctx)?);
    let mut tx = s.pool.begin().await?;
    insert_user_role(&mut tx, target_user_id, &body.role).await?;
    audit(&mut tx, Some(user.0.id), "role.granted",
          &json!({"target": target_user_id, "role": body.role})).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
```

Three things to notice:

1. **`require!` for the policy check.**
2. **Transactional `INSERT` + audit** — both happen or neither does.
3. **`Path<i64>` is the *target*; `user: AuthenticatedUser` is the *actor*.** The audit row records both.

## "Self-demotion" guardrails

A user with the `admin` role *can* revoke their own admin — which can lock everyone out if there are no other admins. Guardrail:

```rust
if target_user_id == s.id && role == Role::Admin {
    let other_admins: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_roles ur JOIN roles r ON r.id = ur.role_id
         WHERE r.name = 'admin' AND ur.user_id != ?"
    ).bind(s.id).fetch_one(&pool).await?;
    if other_admins == 0 {
        return Err(ApiError::Forbidden(Forbidden::LastAdmin));
    }
}
```

You'll learn to write this kind of check by hitting it once in production.

## Why this matters

- **Bootstrapping is a real problem.** Every fresh deploy faces it; ad-hoc solutions ("SSH in and INSERT") are how you lose your audit trail.
- **A CLI for operator actions** keeps human errors out of the API and inside an audited tool.
- **Last-admin guard** is one of those checks you write *after* the first incident.

## Green-bar checkpoint

- You can pick a first-admin strategy for a given deployment and justify it.
- You can sketch the CLI's `grant` command.
- You can articulate the last-admin guard.

Next: `lessons/07-audit-log.md`.
