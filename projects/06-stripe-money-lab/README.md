# projects/06-stripe-money-lab

Phase 8, capstone 1 — the `Money` primitive. Pure Rust, no I/O. The
mathematical foundation that every subsequent Stripe interaction sits on.

## What it teaches

- `Money(i64 cents, Currency)` as a single newtype.
- The $21 billion ceiling and why it exists.
- Checked arithmetic (`checked_add`, `checked_sub`, `checked_mul`) — no
  `Add`/`Sub` operator overloads so callers can't accidentally ignore an
  overflow.
- Currency mismatch as a typed error.
- Largest-remainder proportional split: sum of parts equals original to
  the cent.
- Proptest as the way to prove "sums exactly" for thousands of inputs.

## Run it

```bash
cargo test  -p stripe-money-lab
cargo clippy -p stripe-money-lab -- -D warnings
```

Or `make verify` from the repo root.

## File map

| File | Purpose |
|---|---|
| `src/lib.rs` | `Money`, `Currency`, `MoneyError`, arithmetic, split, Display |
| `src/lib.rs` (tests) | 21 unit tests + 8 property tests |

## What's deliberately not here

- **No I/O.** The library is intentionally pure so it composes everywhere.
- **No `Add`/`Sub` operator impls.** Callers must call `checked_add` etc;
  the compiler refuses to let a bug-via-overflow happen.
- **No FX rates.** Conversion is a separate concern that produces a new
  `Money` value of the target currency.

## The invariants you can rely on

| Invariant | Where |
|---|---|
| Constructor refuses values >= the ceiling | `Money::new` |
| All arithmetic returns `Result`; no silent overflow | `checked_*` |
| Currency mismatch returns a typed error | `checked_add` / `checked_sub` |
| `sum(split_proportional(m, w)) == m.cents` for valid `w` | `split_proportional` + proptest |
| Display renders deterministically (no locale) | `Display` impl |
| Serializes to `{ "cents": N, "currency": "USD" }` | `Serialize` |
