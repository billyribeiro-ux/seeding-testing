# ADR 0003 — Money: i64 cents, $21B ceiling, never floats

- Status: Accepted
- Date: 2026-05-26
- Deciders: billing-team, api-team
- Tags: money, billing, correctness

## Context and Problem Statement

Money correctness is the single highest-stakes property of MemberClub.
A 1-cent drift compounded across millions of transactions becomes real
money lost; a wrong representation causes lawsuits. The representation
must be chosen once and enforced everywhere.

## Decision Drivers

- IEEE 754 floats cannot represent most decimals exactly. `0.1 + 0.2`
  isn't `0.3`. Disqualifying for money.
- Type-system enforcement beats discipline.
- DB-layer enforcement is the airbag for application bugs.
- Currency mismatch must be a *typed* error, not a silent miscalc.

## Considered Options

1. **`f64` for money.** Industry's most common bug. Disqualified.
2. **`rust_decimal::Decimal` everywhere.** Correct; type is awkward to
   pass through HTTP/JSON boundaries; slightly slower.
3. **`i64` cents in a `Money(cents, currency)` newtype.** Correct;
   bounded; trivially serialized as `{ "cents": 1000, "currency": "USD" }`.
4. **i64 cents *without* a newtype.** Easy to confuse with line IDs,
   timestamps, etc.

## Decision Outcome

Chose **option 3**.

- A single `Money(i64 cents, Currency)` newtype defined in
  `projects/06-stripe-money-lab`.
- Constants: `MONEY_CEILING_CENTS = 2_100_000_000_000` ($21B, i.e. 2.1 trillion cents).
  Constructor refuses values whose absolute value reaches the ceiling.
- All arithmetic via `checked_add`, `checked_sub`, `checked_mul`.
  No `std::ops::Add`/`Sub`/`Mul` implementations — overflow cannot be
  silently ignored.
- Currency mismatch returns a typed `MoneyError::CurrencyMismatch`.
- DB columns: `BIGINT NOT NULL` with
  `CHECK (amount_cents > -2100000000000 AND amount_cents < 2100000000000)`
  alongside a `currency CHAR(3) NOT NULL` column.
- Proportional splits use the largest-remainder method
  (`Money::split_proportional`) — proven by proptest to sum exactly.

## Consequences

- **Positive:** money bugs become *compile errors*; the DB layer is the
  safety net; the proptest catches the corner cases unit tests miss.
- **Negative:** no operator overloading is unergonomic. Multi-currency
  arithmetic requires an explicit FX conversion step.
- **Mitigations:** the `Money` newtype lives in one crate, reused
  everywhere; documentation makes the no-overload choice explicit;
  rust_decimal is allowed *only* inside the proportional-split math,
  then converted back to `i64` cents.

## Notes

We re-evaluate the ceiling if we ever sell to customers whose individual
invoice could exceed $20B. (Unlikely.) Multi-currency support is part of
the API today; FX rates live in a separate service.

Related: Phase 8 lessons 1, 2, 3; project `06-stripe-money-lab`.
