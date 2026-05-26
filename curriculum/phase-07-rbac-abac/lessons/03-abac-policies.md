# Lesson 7.3 — ABAC Policies as Pure Functions

> **Concept first:** ABAC isn't a framework. It's just a pure function from `(subject, action, resource, context)` to allow/deny. We write them in Rust, test them like Rust, deploy them with the binary.
> **Time:** 25 minutes.

## The four inputs

| Input | What's in it |
|---|---|
| **Subject** | The acting user (id, roles, tier, tenant, attributes) |
| **Action** | The operation (`doc.read`, `doc.write`, `user.impersonate`) |
| **Resource** | The thing acted upon (document, user, payment, …) — *not* always a row from the DB; sometimes a request body |
| **Context** | Time, IP, request id, whether MFA was just verified, etc. |

A policy function returns `Ok(())` to allow or `Err(reason)` to deny. **Defaults to deny.**

## Two flavours

### Predicate-style (Rust)

```rust
pub fn can_edit_doc(s: &User, doc: &Document, _ctx: &Ctx) -> Result<(), Forbidden> {
    if s.is_admin() { return Ok(()); }
    if doc.owner_id == s.id { return Ok(()); }
    Err(Forbidden::NotOwner)
}
```

Top-level rule first (admin override), then specific allows, then a catch-all `Err`. The function is short, readable, *and* testable.

### Trait-based dispatch

```rust
pub trait Policy<R> {
    fn can(&self, action: Action, resource: &R, ctx: &Ctx) -> Result<(), Forbidden>;
}

impl Policy<Document> for User {
    fn can(&self, action: Action, doc: &Document, _ctx: &Ctx) -> Result<(), Forbidden> {
        match action {
            Action::Read => can_read_doc(self, doc),
            Action::Write => can_edit_doc(self, doc, _ctx),
            Action::Delete => { /* … */ },
            _ => Err(Forbidden::Unsupported),
        }
    }
}
```

A bit more ceremony; pays off when you have many resource types.

For the capstone, we use predicate-style — flatter, faster to read.

## The `require!` macro

Handlers call:

```rust
async fn edit_doc(user: AuthenticatedUser, Path(id): Path<i64>, /* … */) -> Result<Json<DocDto>, ApiError> {
    let doc = repo::doc_by_id(&pool, id).await?;
    require!(policy::can_edit_doc(&user.0, &doc, &ctx)?);
    // ... actually edit ...
}
```

`require!` is a small macro that wraps the `Result`:

```rust
#[macro_export]
macro_rules! require {
    ($result:expr) => {
        match $result {
            Ok(()) => (),
            Err(reason) => return Err(crate::ApiError::Forbidden(reason)),
        }
    };
}
```

It's syntactic sugar over `?` — but the *name* is what matters. When you see `require!` in a handler, you know "this is the policy check."

## Forbidden reasons

The error type names *why* the check failed:

```rust
#[derive(Debug, Clone, Copy)]
pub enum Forbidden {
    NotAuthenticated,
    NotAdmin,
    NotOwner,
    WrongTenant,
    InsufficientTier,
    EmailNotVerified,
    StepUpRequired,
    Unsupported,
}
```

Two uses:

1. **Logs and audit.** Server-side, every forbidden decision is logged with the reason.
2. **Selective surfacing.** Most reasons map to a generic `403 Forbidden`. A few (e.g. `StepUpRequired`) map to a *different* status (`403` with a `WWW-Authenticate: TOTP` header) so the client can prompt the user.

**Never surface the specific reason in the public response unless you've explicitly decided to.** "You're not an admin" leaks information.

## Composition

Most real policies are conjunctions:

```rust
pub fn can_publish_doc(s: &User, doc: &Document, ctx: &Ctx) -> Result<(), Forbidden> {
    require_email_verified(s)?;
    require_role(s, Role::Moderator)?;
    require_tenant(s, doc)?;
    require_step_up(s, ctx, Duration::from_secs(15 * 60))?;
    Ok(())
}
```

Read top-down. The first failing check returns its reason. Reorder to put the cheapest checks first.

## "Allow if owner OR admin" — disjunctions

```rust
pub fn can_delete_doc(s: &User, doc: &Document, _ctx: &Ctx) -> Result<(), Forbidden> {
    if s.is_admin() { return Ok(()); }
    if doc.owner_id == s.id && require_email_verified(s).is_ok() { return Ok(()); }
    Err(Forbidden::NotOwner)
}
```

Convention: admin bypass first, then the "specific allow" path, then deny.

## Time-bounded permissions

```rust
pub fn can_view_pii(s: &User, _resource: &Customer, ctx: &Ctx) -> Result<(), Forbidden> {
    require_role(s, Role::Admin)?;
    require_step_up(s, ctx, Duration::from_secs(15 * 60))?;
    Ok(())
}
```

Step-up gates damaging actions even *after* the session is compromised. The policy ties the action to a recent TOTP check.

## Where policies live

```
projects/05-rbac-policy-lab/src/
├── policy.rs      ← all `pub fn can_X` functions in one file
├── policy_tests.rs (or inline `#[cfg(test)] mod tests`)
└── handlers.rs    ← every privileged handler calls `require!(policy::can_X(...))`
```

One file, top to bottom, scannable. When a security audit asks "what can a member do?", you point at `policy.rs` and read.

## Why this matters

- **Policies as functions are *just code*.** Refactor, test, version-control, review.
- **`require!` is a signal in every handler** that says "policy decision here."
- **Forbidden reasons enable structured audit** without leaking specifics on the wire.

## Green-bar checkpoint

- You can write a `can_X` predicate that combines a role check and an ownership check.
- You can articulate why the specific `Forbidden` reason should usually *not* be surfaced on the wire.
- You can identify the order to put checks in (cheapest first; admin bypass first).

Next: `lessons/04-policy-trait-and-macro.md`.
