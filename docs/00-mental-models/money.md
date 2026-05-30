# Mental Model: Money

> *Money is not a float. Money is an integer count of the smallest unit
> of a currency, paired with the currency code.*

This is the most expensive bug-class to recover from in backend
engineering. We codify the antidote once and enforce it everywhere.

## The four rules

### 1. Storage: `i64` cents

For USD, EUR, GBP: store the amount as `i64` representing **cents**.
`$10.50` is `1050`. The literal `1050` is an integer; no rounding is
possible.

For JPY: store yen directly (yen has no subunit). `¥1000` is `1000`.
The `Currency` enum carries the granularity (`is_zero_decimal`).

For other currencies: smallest currency unit. Bahraini dinar has fils
(1000 per dinar), so a BHD value `1.500` is `1500` *fils*.

In SQL: `BIGINT NOT NULL`. Never `NUMERIC`. Never `FLOAT` / `DOUBLE`.

### 2. The $21B ceiling

```rust
pub const MONEY_CEILING_CENTS: i64 = 2_100_000_000_000; // $21,000,000,000.00 (2.1 trillion cents)
```

Bounds every value. The constructor (`Money::new`) refuses values whose
absolute value reaches the ceiling. The DB enforces the same via
`CHECK`.

Why $21B?

- Vastly larger than any plausible single-row monetary amount.
- Leaves headroom for intermediate multiplications inside DB functions
  without approaching `i64::MAX / 100`.

### 3. No silent arithmetic

The `Money` newtype provides `checked_add`, `checked_sub`, `checked_mul`.
It does *not* implement `std::ops::Add`/`Sub`/`Mul` — overflow cannot
be silently ignored.

```rust
// Compile error: Add is not implemented for Money
let total = subtotal + tax;

// Right: explicit and fallible
let total = subtotal.checked_add(tax)?;
```

### 4. Currency lives next to the amount

A `Money` value is always `(cents, currency)`. Two `Money` values of
different currencies cannot be added; the function returns
`MoneyError::CurrencyMismatch`.

FX conversion is a *separate* concern — a function that produces a new
`Money` in the target currency, with the conversion rate audited.

## The splitting problem

"Split $10 three ways" is *not* `1000 / 3 = 333` with one penny lost.
Use the **largest-remainder method** (Lesson 8.3) so `sum(parts) ==
total` exactly.

`Money::split_proportional` implements this. The proptest in
`stripe-money-lab` proves the invariant for thousands of random inputs.

## When `Decimal` is OK

Inside a calculation that needs proportional precision (e.g. computing
tax on a complex invoice, computing a percentage split), `rust_decimal`
can be used as the intermediate type. The result is converted back to
`i64` cents *immediately*, with documented rounding (banker's by
default).

`Decimal` never crosses a process boundary — never goes to the DB,
never goes to JSON, never goes on the wire.

## Common bugs and their cures

| Symptom | Likely cure |
|---|---|
| Invoice total ≠ sum of line items | A float used somewhere. Grep `f64`/`f32` in money paths. |
| "Sometimes off by a cent" | Naive `total / n` rounding. Use `split_proportional`. |
| Tax calculations differ across countries | Tax rate stored as `f64`. Store as `Decimal` or `i64` permille. |
| Overflow in DB function | Missing `CHECK` constraint allowed a huge value in. |
| Cross-currency total nonsense | A `Money` without currency, or implicit conversion. Force the `Currency` parameter. |

## Related

- ADR 0003 — i64 cents, $21B ceiling, never floats
- Phase 8 lessons (12 of them, but especially 1, 2, 3)
- `projects/06-stripe-money-lab`
