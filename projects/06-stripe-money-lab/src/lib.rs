//! stripe-money-lab — `Money(i64 cents, Currency)` with checked arithmetic
//! and largest-remainder proportional split. Pure logic, no I/O.
//!
//! Rules enforced:
//!   * Storage: i64 cents only. Never f64.
//!   * `MONEY_CEILING_CENTS` bounds every value at +/- $21 billion.
//!   * Arithmetic returns `Result`; no operator overloads.
//!   * Splits sum exactly to the original (largest-remainder method).

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Constants & types
// ---------------------------------------------------------------------------

/// The 21-billion-dollar ceiling — $21,000,000,000.00 expressed in cents.
///
/// Chosen for two reasons:
/// 1. Vastly larger than any realistic single-row amount.
/// 2. Leaves headroom for intermediate multiplications inside DB functions
///    without approaching `i64::MAX / 100`.
pub const MONEY_CEILING_CENTS: i64 = 210_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    USD,
    EUR,
    GBP,
    JPY,
}

impl Currency {
    /// `true` for currencies with no fractional unit (e.g. yen).
    #[must_use]
    pub fn is_zero_decimal(self) -> bool {
        matches!(self, Currency::JPY)
    }

    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Currency::USD => "$",
            Currency::EUR => "€",
            Currency::GBP => "£",
            Currency::JPY => "¥",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MoneyError {
    #[error("amount {0} cents exceeds the $21B ceiling")]
    Overflow(i64),
    #[error("currency mismatch: {a:?} vs {b:?}")]
    CurrencyMismatch { a: Currency, b: Currency },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
    pub cents: i64,
    pub currency: Currency,
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl Money {
    /// Create a `Money` value, refusing anything that exceeds the ceiling.
    pub fn new(cents: i64, currency: Currency) -> Result<Self, MoneyError> {
        if cents.unsigned_abs() as i64 >= MONEY_CEILING_CENTS {
            return Err(MoneyError::Overflow(cents));
        }
        Ok(Self { cents, currency })
    }

    /// A constant zero in the given currency.
    #[must_use]
    pub const fn zero(currency: Currency) -> Self {
        Self { cents: 0, currency }
    }

    #[must_use]
    pub fn is_zero(self) -> bool {
        self.cents == 0
    }

    #[must_use]
    pub fn is_negative(self) -> bool {
        self.cents < 0
    }
}

// ---------------------------------------------------------------------------
// Checked arithmetic — Result-returning by design, no Add/Sub/Mul impls.
// ---------------------------------------------------------------------------

impl Money {
    pub fn checked_add(self, rhs: Money) -> Result<Money, MoneyError> {
        if self.currency != rhs.currency {
            return Err(MoneyError::CurrencyMismatch {
                a: self.currency,
                b: rhs.currency,
            });
        }
        let sum = self
            .cents
            .checked_add(rhs.cents)
            .ok_or(MoneyError::Overflow(i64::MAX))?;
        Money::new(sum, self.currency)
    }

    pub fn checked_sub(self, rhs: Money) -> Result<Money, MoneyError> {
        if self.currency != rhs.currency {
            return Err(MoneyError::CurrencyMismatch {
                a: self.currency,
                b: rhs.currency,
            });
        }
        let diff = self
            .cents
            .checked_sub(rhs.cents)
            .ok_or(MoneyError::Overflow(i64::MIN))?;
        Money::new(diff, self.currency)
    }

    pub fn checked_mul(self, by: i64) -> Result<Money, MoneyError> {
        let product = self
            .cents
            .checked_mul(by)
            .ok_or(MoneyError::Overflow(i64::MAX))?;
        Money::new(product, self.currency)
    }
}

// ---------------------------------------------------------------------------
// Proportional split via largest-remainder method.
// ---------------------------------------------------------------------------

impl Money {
    /// Split into N shares according to integer weights. The sum of the
    /// resulting `Money` values equals the original to the cent.
    ///
    /// Returns an empty vec if `weights` is empty or sums to zero.
    pub fn split_proportional(self, weights: &[u64]) -> Vec<Money> {
        let total_weight: u128 = weights.iter().map(|&w| u128::from(w)).sum();
        if weights.is_empty() || total_weight == 0 {
            return vec![];
        }

        let cents = i128::from(self.cents);
        let abs = cents.unsigned_abs();
        let sign: i64 = if cents < 0 { -1 } else { 1 };

        let mut shares: Vec<i64> = Vec::with_capacity(weights.len());
        let mut remainders: Vec<(usize, u128)> = Vec::with_capacity(weights.len());
        let mut allocated_abs: u128 = 0;
        for (i, &w) in weights.iter().enumerate() {
            let num = abs * u128::from(w);
            let q = num / total_weight;
            let r = num % total_weight;
            allocated_abs += q;
            shares.push((q as i64) * sign);
            remainders.push((i, r));
        }

        // Distribute the residue, one cent at a time, to the largest remainders.
        let residue: u128 = abs - allocated_abs;
        // Stable sort by remainder DESC then index ASC for deterministic order.
        remainders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for k in 0..residue as usize {
            let (idx, _) = remainders[k % remainders.len()];
            shares[idx] = shares[idx].saturating_add(sign);
        }

        shares
            .into_iter()
            .map(|c| Money::new(c, self.currency).expect("share remains within ceiling"))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = self.currency.symbol();
        if self.currency.is_zero_decimal() {
            write!(f, "{symbol}{}", self.cents)
        } else {
            let units = self.cents / 100;
            let cents_abs = self.cents.abs() % 100;
            // Handle the minus sign manually so "-$1.50" reads naturally.
            if self.cents < 0 && units == 0 {
                write!(f, "-{symbol}0.{cents_abs:02}")
            } else {
                write!(f, "{symbol}{units}.{cents_abs:02}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn usd(c: i64) -> Money {
        Money::new(c, Currency::USD).unwrap()
    }

    #[test]
    fn zero_is_under_ceiling() {
        assert_eq!(Money::zero(Currency::USD).cents, 0);
        assert!(Money::zero(Currency::USD).is_zero());
    }

    #[test]
    fn new_refuses_ceiling_and_above() {
        assert_eq!(
            Money::new(MONEY_CEILING_CENTS, Currency::USD).unwrap_err(),
            MoneyError::Overflow(MONEY_CEILING_CENTS)
        );
        assert!(Money::new(MONEY_CEILING_CENTS - 1, Currency::USD).is_ok());
    }

    #[test]
    fn new_refuses_below_negative_ceiling() {
        assert!(Money::new(-MONEY_CEILING_CENTS, Currency::USD).is_err());
        assert!(Money::new(-MONEY_CEILING_CENTS + 1, Currency::USD).is_ok());
    }

    #[test]
    fn add_same_currency() {
        let r = usd(150).checked_add(usd(50)).unwrap();
        assert_eq!(r.cents, 200);
    }

    #[test]
    fn add_currency_mismatch() {
        let a = Money::new(100, Currency::USD).unwrap();
        let b = Money::new(100, Currency::EUR).unwrap();
        assert!(matches!(
            a.checked_add(b),
            Err(MoneyError::CurrencyMismatch { .. })
        ));
    }

    #[test]
    fn add_overflows_ceiling() {
        let a = usd(MONEY_CEILING_CENTS - 1);
        let b = usd(2);
        assert!(matches!(a.checked_add(b), Err(MoneyError::Overflow(_))));
    }

    #[test]
    fn sub_to_negative_is_ok() {
        let r = usd(100).checked_sub(usd(150)).unwrap();
        assert_eq!(r.cents, -50);
    }

    #[test]
    fn mul_by_zero_is_zero() {
        let r = usd(123).checked_mul(0).unwrap();
        assert!(r.is_zero());
    }

    #[test]
    fn mul_overflow_to_ceiling() {
        let a = usd(MONEY_CEILING_CENTS / 2 + 1);
        assert!(a.checked_mul(2).is_err());
    }

    #[test]
    fn split_three_ways_equally_keeps_residue() {
        let m = usd(1000); // $10.00
        let parts = m.split_proportional(&[1, 1, 1]);
        let total: i64 = parts.iter().map(|p| p.cents).sum();
        assert_eq!(total, 1000);
        // One share gets the extra penny.
        let counts: std::collections::BTreeMap<i64, usize> =
            parts
                .iter()
                .fold(std::collections::BTreeMap::new(), |mut acc, p| {
                    *acc.entry(p.cents).or_insert(0) += 1;
                    acc
                });
        assert_eq!(counts.get(&334), Some(&1));
        assert_eq!(counts.get(&333), Some(&2));
    }

    #[test]
    fn split_weighted_keeps_residue() {
        let m = usd(1000);
        let parts = m.split_proportional(&[1, 2, 1]);
        assert_eq!(parts.iter().map(|p| p.cents).sum::<i64>(), 1000);
        // 1000 * 1/4 = 250, 1000 * 2/4 = 500, 1000 * 1/4 = 250. No residue.
        assert_eq!(
            parts.iter().map(|p| p.cents).collect::<Vec<_>>(),
            vec![250, 500, 250]
        );
    }

    #[test]
    fn split_handles_negative_amount() {
        let m = usd(-1000);
        let parts = m.split_proportional(&[1, 1, 1]);
        assert_eq!(parts.iter().map(|p| p.cents).sum::<i64>(), -1000);
    }

    #[test]
    fn split_empty_weights_returns_empty() {
        let m = usd(1000);
        assert!(m.split_proportional(&[]).is_empty());
    }

    #[test]
    fn split_zero_weights_returns_empty() {
        let m = usd(1000);
        assert!(m.split_proportional(&[0, 0]).is_empty());
    }

    #[test]
    fn display_positive_usd() {
        assert_eq!(format!("{}", usd(1234)), "$12.34");
    }

    #[test]
    fn display_negative_usd() {
        assert_eq!(format!("{}", usd(-1234)), "$-12.34");
    }

    #[test]
    fn display_jpy_no_subunit() {
        let m = Money::new(1234, Currency::JPY).unwrap();
        assert_eq!(format!("{m}"), "¥1234");
    }

    #[test]
    fn serializes_to_json_with_both_fields() {
        let m = usd(1234);
        let json = serde_json::to_value(m).unwrap();
        assert_eq!(json["cents"], 1234);
        assert_eq!(json["currency"], "USD");
    }
}

// ---------------------------------------------------------------------------
// Property tests — the invariants of a money library.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn any_currency() -> impl Strategy<Value = Currency> {
        prop_oneof![
            Just(Currency::USD),
            Just(Currency::EUR),
            Just(Currency::GBP),
            Just(Currency::JPY),
        ]
    }

    fn small_cents() -> impl Strategy<Value = i64> {
        // Bounded so the proptest doesn't waste cycles hitting overflow.
        // $1M to -$1M, in cents (100_000_000 cents).
        -100_000_000i64..100_000_000i64
    }

    proptest! {
        /// `Money::new` round-trips for any value strictly inside the ceiling.
        #[test]
        fn ceiling_bounded_round_trip(cents in small_cents(), currency in any_currency()) {
            let m = Money::new(cents, currency).unwrap();
            prop_assert_eq!(m.cents, cents);
            prop_assert_eq!(m.currency, currency);
        }

        /// Addition is commutative when both sides are well-formed.
        #[test]
        fn add_is_commutative(a in small_cents(), b in small_cents()) {
            let am = Money::new(a, Currency::USD).unwrap();
            let bm = Money::new(b, Currency::USD).unwrap();
            let l = am.checked_add(bm);
            let r = bm.checked_add(am);
            prop_assert_eq!(l, r);
        }

        /// (a - b) + b == a (when neither step overflows).
        #[test]
        fn add_sub_cancels(a in small_cents(), b in small_cents()) {
            let am = Money::new(a, Currency::USD).unwrap();
            let bm = Money::new(b, Currency::USD).unwrap();
            if let Ok(diff) = am.checked_sub(bm) {
                let back = diff.checked_add(bm).unwrap();
                prop_assert_eq!(back, am);
            }
        }

        /// Proportional split sums *exactly* to the original.
        #[test]
        fn split_sums_to_original(
            cents in 1i64..1_000_000_000,
            ws in proptest::collection::vec(1u64..1000, 1..10),
        ) {
            let m = Money::new(cents, Currency::USD).unwrap();
            let parts = m.split_proportional(&ws);
            let total: i64 = parts.iter().map(|p| p.cents).sum();
            prop_assert_eq!(total, cents);
        }

        /// Negative split also sums to the original.
        #[test]
        fn split_sums_to_negative_original(
            cents in 1i64..1_000_000_000,
            ws in proptest::collection::vec(1u64..1000, 1..10),
        ) {
            let m = Money::new(-cents, Currency::USD).unwrap();
            let parts = m.split_proportional(&ws);
            let total: i64 = parts.iter().map(|p| p.cents).sum();
            prop_assert_eq!(total, -cents);
        }

        /// Split shares differ by at most one cent.
        #[test]
        fn equal_weights_share_within_one_cent(
            cents in 1i64..1_000_000_000,
            n in 1usize..20,
        ) {
            let weights = vec![1u64; n];
            let m = Money::new(cents, Currency::USD).unwrap();
            let parts = m.split_proportional(&weights);
            if let (Some(max), Some(min)) = (parts.iter().map(|p| p.cents).max(), parts.iter().map(|p| p.cents).min()) {
                prop_assert!((max - min).abs() <= 1);
            }
        }

        /// Multiplication by 1 is identity; by 0 is zero.
        #[test]
        fn mul_identity_and_zero(cents in small_cents()) {
            let m = Money::new(cents, Currency::USD).unwrap();
            prop_assert_eq!(m.checked_mul(1).unwrap(), m);
            prop_assert!(m.checked_mul(0).unwrap().is_zero());
        }
    }
}
