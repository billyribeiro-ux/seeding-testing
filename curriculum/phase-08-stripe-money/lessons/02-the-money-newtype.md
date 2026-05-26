# Lesson 8.2 — The `Money` Newtype

> **Concept first:** wrap `(i64 cents, Currency)` in a single type with checked arithmetic. Make invalid operations *unrepresentable*.
> **Time:** 25 minutes.

## The shape

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
    pub cents: i64,
    pub currency: Currency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    USD, EUR, GBP, JPY,    // extend as needed
}
```

Two fields. One newtype. Every monetary value in the system flows through
this.

## Construction — bounded at birth

```rust
pub const MONEY_CEILING_CENTS: i64 = 2_100_000_000_00;

impl Money {
    pub fn new(cents: i64, currency: Currency) -> Result<Money, MoneyError> {
        if cents.unsigned_abs() as i64 >= MONEY_CEILING_CENTS {
            return Err(MoneyError::Overflow);
        }
        Ok(Money { cents, currency })
    }

    pub const fn zero(currency: Currency) -> Money {
        Money { cents: 0, currency }
    }
}
```

Three rules:

1. **`new` is fallible.** You cannot create an out-of-bounds `Money` value.
2. **`new` is the *only* public way to construct.** Don't expose the field
   for direct mutation; use accessors.
3. **`zero` is `const`-safe** for compile-time uses.

## Arithmetic — `checked_` everywhere

```rust
impl Money {
    pub fn checked_add(self, rhs: Money) -> Result<Money, MoneyError> {
        if self.currency != rhs.currency { return Err(MoneyError::CurrencyMismatch); }
        let sum = self.cents.checked_add(rhs.cents).ok_or(MoneyError::Overflow)?;
        Money::new(sum, self.currency)
    }
    pub fn checked_sub(self, rhs: Money) -> Result<Money, MoneyError> { /* … */ }
    pub fn checked_mul(self, by: i64) -> Result<Money, MoneyError> { /* checked_mul + new */ }
}
```

Two reasons for the `checked_` prefix:

- **No silent overflow.** `i64::MAX + 1` *panics* in debug, *wraps* in
  release. Both are catastrophic for money. `checked_add` returns `None` on
  overflow, which we convert to `Err(MoneyError::Overflow)`.
- **Reminds the caller to handle the error.** Implementing `std::ops::Add`
  would let people write `a + b` and forget to check.

We deliberately *don't* implement `std::ops::Add`. Code that handles money
visibly handles money.

## Currency mismatch — refuse to operate

```rust
let usd = Money::new(100, Currency::USD).unwrap();
let eur = Money::new(100, Currency::EUR).unwrap();
let _   = usd.checked_add(eur);    // Err(MoneyError::CurrencyMismatch)
```

You cannot add $1 to €1 directly. The conversion has to happen *first*
through an explicit FX call, and the FX call produces a new `Money(EUR)` or
`Money(USD)` value. The type system enforces it.

## Display — render the currency symbol

```rust
impl Display for Money {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let units = self.cents / 100;
        let cents = self.cents.abs() % 100;
        let symbol = match self.currency {
            Currency::USD => "$",
            Currency::EUR => "€",
            Currency::GBP => "£",
            Currency::JPY => "¥",
        };
        // JPY has no subunit
        if self.currency == Currency::JPY {
            write!(f, "{symbol}{}", self.cents)
        } else {
            write!(f, "{symbol}{units}.{cents:02}")
        }
    }
}
```

Hand-rolled because:

1. Locale-aware formatting is the *frontend's* job.
2. The server format is deterministic — no surprises in logs.

## Database integration

For sqlx, derive `sqlx::Type`:

```rust
#[derive(sqlx::Type)]
pub struct MoneyCents(pub i64);
```

In the DB, money is two columns: `amount_cents BIGINT NOT NULL` and
`currency CHAR(3) NOT NULL`. The application code wraps the pair into
`Money`.

```sql
CREATE TABLE invoices (
    ...
    amount_cents BIGINT NOT NULL CHECK (
        amount_cents > -2_100_000_000_00 AND amount_cents < 2_100_000_000_00
    ),
    currency CHAR(3) NOT NULL CHECK (currency IN ('USD','EUR','GBP','JPY')),
    ...
);
```

Belt-and-braces: type-level enforcement + DB-layer enforcement.

## A worked test

```rust
#[test]
fn cannot_add_different_currencies() {
    let a = Money::new(100, Currency::USD).unwrap();
    let b = Money::new(100, Currency::EUR).unwrap();
    assert_eq!(a.checked_add(b), Err(MoneyError::CurrencyMismatch));
}

#[test]
fn refuses_to_overflow_the_ceiling() {
    let huge = Money::new(MONEY_CEILING_CENTS - 1, Currency::USD).unwrap();
    let one  = Money::new(2, Currency::USD).unwrap();
    assert_eq!(huge.checked_add(one), Err(MoneyError::Overflow));
}
```

The whole library is tested by a few dozen of these plus a handful of
property tests.

## Why this matters

- **Newtype + fallible constructor + no operator overloading** means money
  bugs become *compile errors*, not runtime surprises.
- **Currency mismatch is a separate failure** from overflow — they need
  different remediations (FX conversion vs reject the input).
- **DB constraints are the airbag** for when application code has a bug.

## Green-bar checkpoint

- You can write the `Money::new` constructor with the ceiling check.
- You can write a checked-add that handles both currency mismatch and `i64`
  overflow.
- You can articulate why we don't implement `std::ops::Add` for `Money`.

Next: `lessons/03-proportional-split.md`.
