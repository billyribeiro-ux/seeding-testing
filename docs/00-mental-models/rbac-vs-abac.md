# Mental Model: RBAC vs ABAC

> *RBAC answers "what role can you act as?". ABAC answers "can this
> specific subject do this specific action on this specific resource,
> right now?". Most real systems need both.*

## The two questions

| Question | Inputs | Example |
|---|---|---|
| **What role can you act as?** (RBAC) | The user's roles | "Admins can delete any document" |
| **Can this subject do this action right now?** (ABAC) | Subject + action + resource + context | "You can edit a document if you own it AND your email is verified" |

RBAC is coarse-grained and cacheable. ABAC is contextual and per-request.

## When RBAC alone is enough

- The set of distinct permissions is small (< 50).
- Decisions don't depend on *which specific resource* you're acting on.
- All authorized users can do an action *equally* — no ownership, no
  tenancy, no time bounds.

Example: an internal admin dashboard where every staff engineer can
see every customer's invoices. Roles = `staff`, `engineering`, `admin`.
Done.

## When you need ABAC

- "Users can edit *their own* X." — ownership.
- "Members of org 7 can read org 7's content." — tenancy.
- "Refunds under $100 don't need approval; over $100 do." — value bound.
- "Read access during business hours." — time bound.
- "Step-up TOTP required for this action." — context-sensitive.

These can't be expressed with role alone. ABAC handles them as predicate
functions over the four inputs.

## The combined pattern

```rust
fn can_publish_doc(s: &User, doc: &Document, ctx: &Ctx) -> PolicyResult {
    require_email_verified(s)?;                  // baseline
    require_role(s, Role::Moderator)?;           // ← RBAC
    require_same_tenant_or_admin(s, doc.org_id)?; // ← ABAC: tenancy
    require_step_up(s, ctx, Duration::minutes(15))?; // ← ABAC: time
    Ok(())
}
```

Read top-down. First failure wins. The `?` operator carries denial
reasons. This is the pattern *every* MemberClub policy uses.

## Deny by default

Two cardinal sins:

1. **"If no policy matches, allow."** A new resource type with no
   policy is *not* "trusted by default" — it's "unintended exposure."
2. **"If we can't decide, allow."** A bug in the policy code shouldn't
   grant access.

Default to *deny*. Every `can_X` function returns `Ok(())` only when the
allow path is explicit. Anything else is `Err(Forbidden::...)`.

## Why we don't use Casbin / OPA

Both are excellent for the right problem. For MemberClub-scale
(< 50 policies, all owned by engineering, infrequent changes), plain
Rust functions are clearer and faster:

- Type-safe (you can't compare a `UserId` to an `OrgId` by accident).
- Testable as regular Rust (every policy has unit tests; Phase 7.8).
- Co-located with handlers (no "where does this policy live?").
- One file, top to bottom, scannable.

ADR 0005 documents the decision and the conditions under which we'd
revisit it.

## Wire format: a 403 with a *generic* body

The wire response is *always* a generic problem-details 403:

```json
{
  "type":   "https://memberclub.test/problems/forbidden",
  "title":  "Forbidden",
  "status": 403,
  "detail": "You don't have permission to perform this action."
}
```

The specific `Forbidden::NotOwner` / `WrongTenant` / `InsufficientTier`
reason lives in:

- The server logs.
- The audit row.

…**never** the wire body. Why? Surfacing "you're not the owner" tells
an attacker the resource exists. Surfacing "wrong tenant" tells them
the tenant exists. The generic 403 leaks zero information.

## Multi-tenancy: two layers of defense

For tenant-scoped resources, RBAC + ABAC at the *application* layer is
the floor, not the ceiling. Add **Postgres Row-Level Security** for the
belt:

```sql
CREATE POLICY tenant_iso ON documents
    USING (org_id = current_setting('app.current_org_id')::bigint);
```

Even if a handler forgets `WHERE org_id = $1`, the database *physically
refuses* to return rows from other tenants. Two layers, two independent
failures required to leak. (ADR 0007.)

## When to add a new policy

When you add a new endpoint that touches data:

1. Decide who's allowed.
2. Write the `can_X` predicate.
3. Write the unit tests (allow + deny per branch — Phase 7.8).
4. Add `require!(policy::can_X(...))` at the top of the handler.
5. Add an audit-log write inside the same transaction as the state
   change.

Five mechanical steps. Skipping any one is the bug.

## Why this matters

- **The audit log + policy code is your security spec.** A reviewer
  reads them top-down to know what the system allows.
- **"Just check the role" works until your second tier (tiers / owners
  / tenants).** Plan for ABAC from day one.
- **Deny by default + admin-override-first** is the pattern that scales
  to dozens of policies without becoming spaghetti.

## Related

- ADR 0005 — Explicit policies, not Casbin
- ADR 0007 — Postgres RLS as belt-and-braces
- Phase 7 lessons (9 of them)
- `projects/05-rbac-policy-lab` — the worked policy library
- `docs/00-mental-models/auth.md` — AuthN ≠ AuthZ
