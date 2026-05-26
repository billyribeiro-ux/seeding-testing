# Lesson 8.1 — Money Is Not a Float

> **Concept first:** `f64` cannot exactly represent most decimal values. Use it for money and you will leak cents. Period.
> **Time:** 15 minutes.

## Five seconds of evidence

Open any REPL — JavaScript, Python, Rust, anything:

```
> 0.1 + 0.2
0.30000000000000004
```

That's not a bug. It's an inevitable consequence of representing decimal
numbers in binary. `0.1`, `0.2`, `0.3` aren't *representable* exactly in
IEEE 754. The closest representable values, when added, produce
`0.30000000000000004`.

For physics simulations, machine learning, and rendering, this is fine.
For money, it is the difference between "trustworthy company" and "lawsuit."

## A more disturbing example

```rust
fn main() {
    let invoice_lines = [0.10_f64; 10];
    let total: f64 = invoice_lines.iter().sum();
    println!("{total}");        // 0.9999999999999999
    if total == 1.00 {
        println!("ok");
    } else {
        println!("ROUNDS THE CUSTOMER TO LESS THAN THEY OWE");
    }
}
```

Ten ten-cent line items should sum to exactly $1.00. They don't. The system
overcharges or undercharges by a fraction of a cent — and that fraction
*compounds* over millions of transactions.

## The fix

Represent money as integers. Specifically: **the smallest unit of the
currency**, as `i64`.

For USD: cents (`100` for $1.00).
For JPY: yen (`100` for ¥100 — yen has no subunit).
For BHD: fils (Bahraini dinar has 1000 fils per dinar — three decimals).

The `Money(amount_cents: i64, currency: Currency)` newtype enforces this at
the type level. No `f64`, anywhere, near money.

Storage: `BIGINT` in Postgres / `INTEGER` in SQLite.

## What about taxes (7.5%)?

Tax rates *are* decimals. You can't represent them as cents. Use a real
decimal library: `rust_decimal::Decimal`. *But*:

- Decimals are used inside a single calculation (`subtotal * tax_rate`).
- The result is converted back to `i64` cents *immediately*, with
  documented rounding (banker's by default).
- Decimals never cross a process boundary — they don't end up in the DB or
  in an HTTP response.

This is the "decimal for arithmetic, integer for state" pattern.

## The $21 billion ceiling

```rust
pub const MONEY_CEILING_CENTS: i64 = 2_100_000_000_00;   // $21,000,000,000.00
```

Why $21B?

- **Safety margin from `i64::MAX`.** `i64::MAX` is about $92 quintillion in
  cents — vastly bigger than we need. The ceiling at $21B gives us 100,000×
  headroom for multiplications inside DB functions (e.g. tax × subtotal)
  without overflow risk.
- **Far above any realistic single-row amount.** No single charge, refund,
  subscription, or invoice will exceed $21B. (If yours does, you're not
  reading this; you're being recruited by central banks.)
- **Caught by `CHECK` at the DB layer.** A `CHECK (amount_cents > -MONEY_CEILING_CENTS AND amount_cents < MONEY_CEILING_CENTS)` constraint refuses bad values *even if* application code has a bug.

Pick a constant; enforce everywhere; sleep well.

## Why this matters

- **Money correctness is *the* differentiating concern of a SaaS backend.**
  Get it wrong once and customers don't trust the receipts. Get it wrong at
  scale and you owe the IRS.
- **Type-level enforcement** (`Money` newtype) is cheap and catches bugs at
  compile time, not in production.
- **DB-layer enforcement** (`CHECK` constraints, `BIGINT`) is the safety net
  for when application code has a bug.

## Green-bar checkpoint

- You can demonstrate the `0.1 + 0.2` floating-point trap in any language.
- You can articulate why JPY uses `i64` of yen (not `f64` * 100).
- You can write the $21B ceiling constant and explain its safety margin.

Next: `lessons/02-the-money-newtype.md`.
