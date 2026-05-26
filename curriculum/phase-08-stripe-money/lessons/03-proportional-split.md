# Lesson 8.3 — Proportional Split with Banker's Rounding

> **Concept first:** "split this invoice three ways equally" sounds simple. It is not. The split must sum exactly to the original — to the cent. Banker's rounding plus a fix-up step is the only way.
> **Time:** 25 minutes.

## The problem

Split $10.00 (1000 cents) into three equal parts.

Naive math: 1000 / 3 = 333.333…

If you round each part down: 333 + 333 + 333 = 999. *You've lost a penny.*

If you round each up: 334 + 334 + 334 = 1002. *You've added two pennies.*

Either way, the sum doesn't equal the original. In an invoice with line
items, those pennies become real money. In a payout split between platform
and merchant, they become customer complaints.

## The fix: largest-remainder method

```
1. Compute each share as floor(total * weight_i / sum_of_weights).
2. Compute the residue: total − sum(shares).
3. Distribute `residue` extra pennies, one to each share, in *order of largest fractional remainder*.
```

For $10 / 3:

| Step | Computation | Result |
|---|---|---|
| Quotient | `1000 * 1 / 3` = 333.333 | floor → 333 |
| All three shares floored | | [333, 333, 333] |
| Residue | 1000 − 999 | 1 |
| Distribute 1 extra cent to the largest remainder | (all are equal, so the first one) | [334, 333, 333] |

Sum: 1000. Correct.

This is the same algorithm used in tax calculation, proportional capital
calls, fair-share dividing, and parliamentary seat allocation. It's a
well-known result; reinventing it produces subtly wrong code.

## In Rust

```rust
impl Money {
    /// Split into N shares with the given weights. The sum of returned shares
    /// equals the original to the cent.
    pub fn split_proportional(self, weights: &[u64]) -> Vec<Money> {
        let total_weight: u128 = weights.iter().map(|&w| w as u128).sum();
        if total_weight == 0 || weights.is_empty() { return vec![]; }

        let cents = self.cents as i128;

        // Compute each floor share and remember the remainder for fairness.
        let mut shares: Vec<i64> = Vec::with_capacity(weights.len());
        let mut remainders: Vec<(usize, u128)> = Vec::with_capacity(weights.len());
        let mut allocated: i128 = 0;
        for (i, &w) in weights.iter().enumerate() {
            let num = cents.unsigned_abs() * w as u128;
            let q = num / total_weight;
            let r = num % total_weight;
            let signed_q = q as i64 * cents.signum() as i64;
            shares.push(signed_q);
            remainders.push((i, r));
            allocated += signed_q as i128;
        }

        // Distribute the residue to the largest remainders.
        let residue = (cents - allocated) as i64;     // absolute value of pennies to add
        remainders.sort_by(|a, b| b.1.cmp(&a.1));     // largest remainder first
        for k in 0..residue.unsigned_abs() as usize {
            let (idx, _) = remainders[k % remainders.len()];
            if cents >= 0 {
                shares[idx] = shares[idx].saturating_add(1);
            } else {
                shares[idx] = shares[idx].saturating_sub(1);
            }
        }

        shares.into_iter()
            .map(|c| Money::new(c, self.currency).expect("share within ceiling"))
            .collect()
    }
}
```

A few subtleties:

- **`u128` for the intermediate multiplication** — `i64 * u64` can overflow
  `i64` for large amounts; `u128` cannot.
- **Signed amounts (refunds)** — we compute on absolute values, then
  reapply the sign.
- **Stable order** — same input always produces the same split. Important
  for snapshot tests and reproducibility.

## The invariant

The defining property:

```rust
proptest! {
    #[test]
    fn split_sums_exactly(cents in 1i64..1_000_000, ws in vec(1u64..1000, 1..10)) {
        let m = Money::new(cents, Currency::USD).unwrap();
        let parts = m.split_proportional(&ws);
        let total: i64 = parts.iter().map(|p| p.cents).sum();
        prop_assert_eq!(total, cents);
    }
}
```

Run this with 10,000 random inputs. Every single one must satisfy
`sum(parts) == total`. If even one doesn't, the bug surfaces — even if it's
a 1-in-10,000 edge case.

We use proptest because *unit* tests of carefully-chosen examples never
catch the edge cases. The point of property tests for money is to brute-force
the corner cases.

## When to use this

- **Tax calculation** — when tax-inclusive total must split into subtotal
  and tax portions to the cent.
- **Platform/merchant splits** — Stripe Connect, marketplaces,
  revenue-sharing partners.
- **Proration** — when a subscription is upgraded mid-cycle, the unused
  portion of the old plan must split exactly.
- **Refunds** — refunding part of a multi-line invoice has to remove
  exactly the right number of cents.

## Why this matters

- **A single-cent rounding error in a single transaction is a tiny bug.**
  Repeated 10,000,000 times a year, it's $100,000 missing from your ledger.
- **Banker's rounding without the residue fix-up is *still wrong*.** Sums
  drift. The largest-remainder method is the cure.
- **Property tests are the only way to catch this safely.** Manual examples
  can't enumerate the space.

## Green-bar checkpoint

- You can explain the largest-remainder method in plain English.
- You can write `Money::split_proportional` from memory.
- You can write the proptest that proves sums-exactly.

Next: `lessons/04-stripe-data-model.md`.
