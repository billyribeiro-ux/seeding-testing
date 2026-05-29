# Lesson 5.4 — Snapshot and Property Tests

> **Concept first:** two test flavors that catch different bugs. Snapshots catch unintended drift in output. Properties catch invariant violations across the whole input space.
> **Time:** 25 minutes.

## Snapshot testing with `insta`

A snapshot test records a "golden" value on first run; later runs diff against it. If something changed, the test fails until you accept the new snapshot or fix the regression.

```rust
#[test]
fn problem_details_shape_for_not_found() {
    let err = ApiError::Notes(NotesError::NotFound(42));
    let resp = err.into_response_body_for_snapshot();
    insta::assert_json_snapshot!(resp);
}
```

The first run creates `snapshots/your_test_module__problem_details_shape_for_not_found.snap`. The second run diffs. To accept legitimate changes:

```bash
cargo insta accept
```

(There's also `cargo insta review` for interactive accept/reject.)

### When snapshots earn their keep

- **API response shapes.** Snapshotting the JSON body for each problem-details variant catches unintended wire-format changes.
- **Generated artifacts.** OpenAPI spec, GraphQL schema, code-gen output, SQL `EXPLAIN` plans for critical queries.
- **Complex rendered output.** HTML emails, PDFs (snapshot the structured content, not bytes).

### When snapshots are a trap

- **Frequently-changing values.** Don't snapshot dates, UUIDs, request IDs. Use `redact`:
  ```rust
  insta::assert_json_snapshot!(resp, {
      ".request_id" => "<uuid>",
      ".created_at" => "<ts>",
  });
  ```
- **Massive lists.** A 10,000-element snapshot is noise.
- **Output you'd never *read*.** If nobody ever opens the snapshot file, it's not adding value.

## Property tests with `proptest`

A unit test covers the cases you thought of. A property test generates random inputs and checks that an invariant holds.

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn cents_round_trip(c in 0i64..2_100_000_000_000) {
        let m = Money::new(c, Currency::USD).unwrap();
        prop_assert_eq!(m.cents, c);
    }

    #[test]
    fn split_proportional_sums_to_original(c in 1i64..1_000_000, ws in proptest::collection::vec(1u64..1000, 1..10)) {
        let m = Money::new(c, Currency::USD).unwrap();
        let parts = m.split_proportional(&ws);
        let total: i64 = parts.iter().map(|p| p.cents).sum();
        prop_assert_eq!(total, c);
    }
}
```

Two ideas:

- **Invariants** are statements that should be true for *every* input — `sum(parts) == total`, `decode(encode(x)) == x`, `compose(f, inverse(f)) == identity`.
- **Shrinking** — when proptest finds a failing input, it shrinks toward the smallest input that still fails. You debug with the *minimum reproducible case*, not the giant random one.

### Where property tests shine

| Pure logic | Property idea |
|---|---|
| Money arithmetic | `(a + b) + c == a + (b + c)`; `split` sums to original |
| Parser + printer | `parse(print(x)) == x` |
| Encryption / encoding | `decrypt(encrypt(x, k), k) == x` |
| Sort / compare | `sorted(xs).len() == xs.len()`; output is monotone |
| Cursor pagination | "concat all pages" equals "single big query" |

### Where they don't help

- I/O-heavy code (use integration tests instead).
- Code where invariants are hard to phrase (i.e. the *business rules themselves* are arbitrary).

## A combined example

In Phase 8 we'll have:

```rust
// Unit test: documented behaviour
#[test]
fn split_3_ways_evenly() {
    let m = Money::new(900, Currency::USD).unwrap();
    let parts = m.split_proportional(&[1, 1, 1]);
    assert_eq!(parts.iter().map(|p| p.cents).collect::<Vec<_>>(), vec![300, 300, 300]);
}

// Snapshot test: wire format
#[test]
fn invoice_json_shape() {
    let invoice = Invoice::sample();
    insta::assert_json_snapshot!(invoice);
}

// Property test: invariant
proptest! {
    #[test]
    fn split_always_sums(c in 1i64..1_000_000_00, ws in 1usize..20) {
        let weights: Vec<u64> = (0..ws).map(|i| (i + 1) as u64).collect();
        let m = Money::new(c, Currency::USD).unwrap();
        let parts = m.split_proportional(&weights);
        prop_assert_eq!(parts.iter().map(|p| p.cents).sum::<i64>(), c);
    }
}
```

Three flavors, three different defenses, one money primitive.

## Why this matters

- **Snapshots make wire-format changes intentional.** No more "I added a field and forgot to bump the version."
- **Property tests find the case you didn't think of.** Off-by-ones, Unicode edges, negative numbers, max values.
- **Together they catch most of the bugs unit tests miss** without the cost of full integration tests.

## Green-bar checkpoint

- You can write an `insta::assert_json_snapshot!` with one redaction.
- You can write a proptest for `(a + b) + c == a + (b + c)` on `i64` with bounded ranges.
- You can articulate when *not* to snapshot.

Next: `lessons/05-factories-and-fixtures.md`.
