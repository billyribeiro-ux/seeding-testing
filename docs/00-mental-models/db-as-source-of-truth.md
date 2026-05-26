# Mental Model: The Database is the Source of Truth

> *Your application is a cache of the database. If they disagree, the
> database wins.*

This is the most consequential mental model in backend engineering.
Three corollaries follow from it.

## 1. Constraints belong in the DB

If you can express it as `NOT NULL`, `UNIQUE`, `CHECK`, or a foreign
key — *do*. Anything you express in application code is one bug away
from being violated. Anything you express in the schema is *physically
impossible* to violate.

```sql
-- Wrong: enforcing "email is required" in app code only
-- The next service that writes to this table can violate it.

-- Right: enforcing it at the DB layer
email TEXT NOT NULL UNIQUE
```

This is true for:

- **Required fields** (`NOT NULL`).
- **Uniqueness** (no two users with the same email).
- **Referential integrity** (a note's `user_id` must reference an
  existing user).
- **Business invariants you can express** (`CHECK (amount_cents >= 0)`).

## 2. Mirror, don't synchronize

When you integrate with a third party (Stripe, an email provider, a
search index), do *not* read from them in user-facing request paths.

Instead: webhooks (or polling) update your local *mirror* of the
relevant state. Reads hit your DB; writes go through the third party,
with idempotency keys.

The third party is *the rail* (it moves money, sends messages, indexes
content). Your DB is *the truth* (it's what your UI reads, what your
reports query, what survives the third party's outages).

ADR 0008 captures this for Stripe specifically.

## 3. The application is replaceable; the data is not

You will rewrite your handlers many times. Schemas migrate, but slowly,
and with care. The data outlasts the code.

Two practical consequences:

- **Migrations are forever.** A migration that runs on a 50M-row table
  cannot be undone cheaply. Treat each one as a public API change.
- **Schema design pays for itself.** Spend the hour up front to get the
  shape right. The hour you save is the day you'd spend at 3 AM later.

## What this means in practice

- Add the `CHECK` constraint, not just the application validation.
- Pull `customer_id` from your `customers` mirror, not from Stripe.
- When your code disagrees with the DB, the DB is right and your code
  has a bug.

## Counterargument: "what about eventual consistency?"

The mental model applies to *strong-consistency* state — the kind a
user can immediately observe in their own session. For analytics,
caches, feeds, recommendation indexes — these are *secondary* stores
populated from the canonical DB. The DB is still the source of truth;
secondaries are catch-up projections.

## Related

- ADR 0008 — Stripe is the rail
- ADR 0007 — Postgres RLS as belt-and-braces
- Phase 3 lessons (relational fundamentals, schema design)
- Phase 8 lessons 4 (data model) and 11 (reconciliation)
