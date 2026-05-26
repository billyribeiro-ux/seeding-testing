# Lesson 7.1 — RBAC vs ABAC

> **Concept first:** "what can this user do?" has two complementary answers — by their *role* and by the *attributes* of the request. Most real systems use both.
> **Time:** 20 minutes.

## RBAC in one sentence

Each user has one or more **roles**. Each role grants a set of **permissions**. To decide whether a user can do X, check: does any of their roles grant the permission required for X?

Example:

```
member     →  doc.read
moderator  →  doc.read, doc.hide
admin      →  doc.read, doc.hide, doc.delete, user.impersonate
```

A user with role `admin` can `doc.delete`. A user with role `member` cannot. The decision needs only the user's roles and the action's required permission. Cheap; cacheable.

## When RBAC alone is enough

- The set of distinct permissions is small (< 50).
- Decisions don't depend on the *specific* resource.
- All authorized users can do a given action *equally*.

Examples: an admin dashboard. A help-desk tool. A wiki where every editor can edit any page.

## ABAC in one sentence

Decide based on the *attributes* of the subject, action, resource, and context — not just on roles. Roles can still inform the decision; they're just one attribute.

Example: "you can edit a document if you own it."

- Subject attribute: `user.id`.
- Resource attribute: `document.owner_id`.
- Rule: `user.id == document.owner_id`.

No role check; just a comparison. We use this constantly.

## When you need ABAC

- "Users can edit *their own* X." — ownership.
- "Tenants can only see their tenant's data." — multi-tenancy isolation.
- "Approve only orders under $10,000." — value-bounded permissions.
- "Read access during business hours only." — time-bounded.
- "Moderators in EU can't see PII of US users." — jurisdiction-bounded.

These need attributes that aren't captured by a role alone.

## A real example: MemberClub's content policy

```
A user can READ a post P iff
    P.published_at IS NOT NULL                  (it's published)
  AND
    user.tier >= P.min_tier                     (user has the right plan)
  AND
    (user.role == 'admin' OR user.org_id == P.org_id)
                                                (tenant isolation, with admin override)
```

Three conjoined rules. Pure ABAC. No "role can read" in the sense RBAC means it.

But:

```
A user can DELETE a post P iff
    user.role IN ('moderator', 'admin')        (RBAC gate)
  AND
    user.org_id == P.org_id OR user.role == 'admin'
                                                (ABAC: tenant + override)
```

RBAC and ABAC both apply.

## The decision rule we use

One function:

```rust
fn check(subject: &User, action: Action, resource: &Resource, ctx: &Ctx) -> Result<(), ForbiddenReason>;
```

Inside, RBAC and ABAC compose freely. If the function returns `Ok(())`, the action proceeds. If it returns `Err(reason)`, the handler returns `403` with the reason in the audit log (but a *generic* message in the wire response — see Lesson 7.7).

## Deny by default

Two cardinal sins, both rooted in the same instinct:

1. **"If no policy matches, allow."** This is the path to disasters. A new resource type with no policy is *not* "trusted by default" — it's "unintended exposure."
2. **"If we can't decide, allow."** Same problem. A bug in the policy code shouldn't grant access.

**Default deny.** Every policy must return `Ok(())` *explicitly* to permit. Anything else is a 403.

## When *not* to use a policy library (Casbin, OPA, etc.)

Libraries like Casbin and OPA are powerful — Casbin compiles a policy language to a decision engine; OPA externalizes policy entirely. They're worth their complexity when:

- You have *hundreds* of policies.
- You need to *change policies without redeploying*.
- You have a dedicated security team writing policies separately from app engineers.

For MemberClub (and most early-stage products), **plain Rust functions are clearer and faster**:

- They live next to the handlers; nobody asks "where is this policy?"
- They're typed; you can't accidentally compare a `UserId` to a `OrgId`.
- They're tested as normal Rust tests.
- They compile.

We document the choice in an ADR (`docs/01-architecture-decisions/0005-explicit-policies-no-casbin.md`).

## Why this matters

- **RBAC alone is too coarse for modern SaaS.** Multi-tenant + tier + ownership demands ABAC.
- **A central policy function is the security perimeter.** It's the boundary you test the most.
- **Deny-by-default catches the cases you didn't think of.** That's the whole point.

## Green-bar checkpoint

- You can explain the difference between RBAC and ABAC with an example.
- You can write a one-line policy that combines both (role *and* ownership).
- You can articulate why we don't reach for Casbin.

Next: `lessons/02-rbac-schema.md`.
