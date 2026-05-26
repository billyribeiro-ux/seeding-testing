# Lesson 7.4 — The `Policy` Trait and `require!` Macro

> **Concept first:** wrap the `(subject, action, resource, ctx) → Result<(), Forbidden>` shape in a tiny trait + a one-line macro. The shape becomes a habit; handlers stay clean.
> **Time:** 15 minutes.

## The trait

```rust
pub trait Policy<Subject, Resource> {
    fn check(&self, subject: &Subject, resource: &Resource, ctx: &Ctx) -> Result<(), Forbidden>;
}
```

Implementors:

```rust
pub struct CanReadDoc;
impl Policy<User, Document> for CanReadDoc {
    fn check(&self, s: &User, doc: &Document, _ctx: &Ctx) -> Result<(), Forbidden> {
        if !doc.is_published { return Err(Forbidden::NotPublished); }
        if s.tier_rank() < doc.min_tier_rank { return Err(Forbidden::InsufficientTier); }
        Ok(())
    }
}
```

You don't *have* to wrap every policy in a struct; the project uses plain `pub fn` predicates for simplicity. The trait is most useful when you want to enumerate policies (e.g. an admin dashboard listing "policies in this service") or compose them generically.

## The `require!` macro (the actual one used in the capstone)

```rust
#[macro_export]
macro_rules! require {
    ($result:expr) => {
        match $result {
            Ok(()) => (),
            Err(reason) => return Err(crate::Forbidden::Wrap(reason).into()),
        }
    };
    ($cond:expr, $reason:expr) => {
        if !$cond {
            return Err($reason.into());
        }
    };
}
```

Two forms:

```rust
require!(policy::can_edit_doc(user, &doc, &ctx));   // pass-through of Result
require!(user.is_admin(), Forbidden::NotAdmin);     // inline assertion + reason
```

Both desugar to "return early with a typed forbidden error." Clean, greppable, hard to skip.

## The `Forbidden` enum

```rust
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum Forbidden {
    #[error("not authenticated")]    NotAuthenticated,
    #[error("not an admin")]         NotAdmin,
    #[error("not the owner")]        NotOwner,
    #[error("wrong tenant")]         WrongTenant,
    #[error("tier too low")]         InsufficientTier,
    #[error("email not verified")]   EmailNotVerified,
    #[error("not published")]        NotPublished,
    #[error("step-up required")]     StepUpRequired,
    #[error("unsupported")]          Unsupported,
}
```

`impl From<Forbidden> for ApiError` turns each variant into the same wire-level `403` (with the reason in logs/audit, not the body).

## How handlers use it

```rust
async fn delete_doc(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let doc = sqlx::query_as::<_, Document>("SELECT ... WHERE id = ?")
        .bind(id).fetch_one(&s.pool).await?;

    require!(policy::can_delete_doc(&user.0, &doc, &ctx_from_request())?);

    sqlx::query("DELETE FROM documents WHERE id = ?").bind(id).execute(&s.pool).await?;
    audit(&s.pool, &user.0, "doc.deleted", &json!({"doc_id": id})).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

Three lines of policy + audit, then the actual change. The pattern is uniform — every privileged handler reads the same way.

## Why this matters

- **`require!` is a *visible* security check.** Code reviewers grep for it; auditors find it; tools can count it.
- **One typed `Forbidden` enum** keeps decisions structured and audit-loggable.
- **Composition is free** — you can `require!(a)?; require!(b)?;` and the first failure wins.

## Green-bar checkpoint

- You can write the two-arm `require!` macro by hand.
- You can describe the trade-off between trait-based `Policy` impls and plain `pub fn` predicates.
- You can map any `Forbidden` variant to its expected user-facing wire response.

Next: `lessons/05-ownership-and-tenancy.md`.
