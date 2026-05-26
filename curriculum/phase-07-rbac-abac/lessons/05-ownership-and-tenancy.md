# Lesson 7.5 — Ownership, Tenancy, and Time-Bounded Permissions

> **Concept first:** three ABAC patterns appear in nearly every SaaS. Recognize them and you'll write 80% of your policies on autopilot.
> **Time:** 20 minutes.

## Pattern 1: Ownership

"The owner can — anyone else needs a stronger justification."

```rust
fn can_edit_doc(s: &User, doc: &Document, _ctx: &Ctx) -> Result<(), Forbidden> {
    if s.is_admin() { return Ok(()); }
    if doc.owner_id == s.id { return Ok(()); }
    Err(Forbidden::NotOwner)
}
```

Variants:

- **Admin bypass first** — almost always.
- **Owner allowed unconditionally** — sometimes you want "owner *and* verified" or "owner *and* not banned."
- **Co-ownership** — a `document_owners` join table; check membership.

## Pattern 2: Tenancy (multi-tenancy isolation)

Every resource belongs to an *organization* (or workspace, team, project). Cross-tenant access is the security disaster of the SaaS era.

```rust
fn can_read_in_org(s: &User, resource_org_id: i64, _ctx: &Ctx) -> Result<(), Forbidden> {
    if s.org_id == resource_org_id { return Ok(()); }
    if s.is_global_admin() { return Ok(()); }
    Err(Forbidden::WrongTenant)
}
```

Apply this to *every* resource fetch. Don't trust the URL; even if `GET /orgs/7/users/42` was issued by a user from org 3, the user in question must belong to org 7 to be returned.

### Belt-and-braces: row-level security in the DB

Postgres supports Row-Level Security (RLS) policies that *also* enforce tenancy:

```sql
ALTER TABLE documents ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso ON documents
    USING (org_id = current_setting('app.current_org_id')::bigint);
```

The application sets `SET LOCAL app.current_org_id = $1` at the start of each request. The DB *physically* refuses to return rows from other tenants.

Both layers (application + DB) is the standard for sensitive data. SQLite doesn't have RLS; for Phase 7 we enforce at the application layer only. MemberClub adds RLS in Phase 11.

## Pattern 3: Time-bounded permissions

"This permission only applies *now*."

```rust
fn require_step_up(s: &User, ctx: &Ctx, max_age: Duration) -> Result<(), Forbidden> {
    let last = s.totp_last_verified_at.ok_or(Forbidden::StepUpRequired)?;
    if ctx.now.signed_duration_since(last) > chrono::Duration::from_std(max_age).unwrap() {
        return Err(Forbidden::StepUpRequired);
    }
    Ok(())
}
```

Used for: sensitive admin actions (impersonation, viewing PII, deleting accounts), payment-method changes, viewing/regenerating recovery codes.

Variants:

- **Business hours** — `9 ≤ ctx.now.hour() ≤ 17` plus a timezone.
- **Maintenance windows** — block writes during scheduled maintenance.
- **Trial expiry** — `user.trial_expires_at > now` is a permission gate.

## Composing patterns

A realistic "publish a doc to Pro members of org 7" policy:

```rust
fn can_publish_doc(s: &User, doc: &Document, ctx: &Ctx) -> Result<(), Forbidden> {
    require_email_verified(s)?;            // baseline
    require_role(s, Role::Moderator)?;     // RBAC
    if s.org_id != doc.org_id && !s.is_global_admin() {
        return Err(Forbidden::WrongTenant);
    }
    require_step_up(s, ctx, Duration::from_secs(15 * 60))?;
    if doc.min_tier > Tier::Pro {
        return Err(Forbidden::InsufficientTier);  // can't publish above Pro from this UI
    }
    Ok(())
}
```

Six conjunctions; reads top-down; each failure is named.

## What about "list all docs across all tenants"?

The hardest pattern. Two options:

1. **Privileged endpoint** — only callable with `Forbidden::NotAdmin` blocking everyone else. Returns rows across tenants. *Used internally.*
2. **Per-tenant union** — a public endpoint that returns "all docs in the orgs this user belongs to." Filter, don't bypass.

Be especially careful with cross-tenant *writes*. Any write that ignores `org_id` is a vulnerability waiting to happen.

## Why this matters

- **Multi-tenant isolation is the #1 SaaS security concern.** A leak across tenants is far worse than a leak within one.
- **Three patterns cover 80% of real policies.** Recognizing them is half the work.
- **Composing patterns explicitly** beats hiding them in framework configuration.

## Green-bar checkpoint

- You can write a policy that combines all three patterns (ownership, tenancy, time-bounded).
- You can articulate the value of RLS *in addition to* application-level checks.
- You can identify when a cross-tenant endpoint should exist and what it should require.

Next: `lessons/06-admin-bootstrap.md`.
