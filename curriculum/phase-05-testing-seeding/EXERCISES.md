# Phase 5 — Exercises

Seven drills. Most extend `notes-api` and `sqlx-notes`.

---

## E5.1 — Snapshot the problem-details responses (Easy)

Add `insta` as a dev-dep to `notes-api`. Write a snapshot test for each error variant's JSON body (with the `created_at`/`request_id`-style ephemeral fields redacted). When the wire format changes intentionally, accept the new snapshot with `cargo insta accept`.

<details><summary>Answer (sketch)</summary>

```toml
[dev-dependencies]
insta = { version = "1", features = ["json"] }
```

```rust
#[tokio::test]
async fn snapshot_problem_details_404() {
    let app = app().await;
    let res = app.oneshot(Request::get("/v1/notes/999").body(Body::empty()).unwrap()).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    insta::assert_json_snapshot!(body);
}
```
</details>

---

## E5.2 — Property test `Counts::count` (Easy)

In `projects/01-hello-cli/src/lib.rs`, add a `proptest!` that asserts `Counts::count(s).chars == s.chars().count()` for every UTF-8 string.

<details><summary>Answer</summary>

```toml
[dev-dependencies]
proptest = "1"
```

```rust
#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #[test]
        fn chars_matches_string(s in ".*") {
            let c = Counts::count(&s);
            prop_assert_eq!(c.chars, s.chars().count());
        }
    }
}
```
</details>

---

## E5.3 — Cover every endpoint (Easy)

Add or extend integration tests in `notes-api` until *every* endpoint and *every* error-status code is exercised at least once. Run `cargo llvm-cov` and confirm coverage ≥ 90% on `notes-api`'s lib.

<details><summary>Answer</summary>

You already have happy-path tests for create / list / get / patch / delete. Add:
- PATCH unknown id → 404
- POST with non-JSON body → 400 (bad JSON)
- PATCH with empty body → 400

Then `cargo llvm-cov -p notes-api report --summary-only`.
</details>

---

## E5.4 — Add a `NoteFactory` (Medium)

In `sqlx-notes`, add `src/factory.rs` exposing `note()` builder. Use it in the integration tests where appropriate.

<details><summary>Answer (sketch)</summary>

```rust
// sqlx-notes/src/factory.rs
use fake::Fake;
use fake::faker::lorem::en::Sentence;
use sqlx::SqlitePool;
use crate::Note;

pub struct NoteFactory { body: Option<String> }
impl NoteFactory {
    pub fn new() -> Self { Self { body: None } }
    pub fn body(mut self, b: impl Into<String>) -> Self { self.body = Some(b.into()); self }
    pub async fn insert(self, pool: &SqlitePool) -> Note {
        let body = self.body.unwrap_or_else(|| Sentence(1..3).fake());
        super::add(pool, &body).await.expect("factory insert")
    }
}
pub fn note() -> NoteFactory { NoteFactory::new() }
```

Then in tests: `let n = factory::note().body("custom").insert(&pool).await;`.
</details>

---

## E5.5 — Build the seed CLI (Medium)

Implement the `notes-seed` binary as described in Lesson 5.8. Three profiles + safety guards. Add CI smoke tests.

---

## E5.6 — `cargo llvm-cov` in CI (Medium)

Add a `coverage` job to `.github/workflows/ci.yml` that runs `cargo llvm-cov --workspace --fail-under-lines 80` and uploads the lcov to Codecov (or just keeps it as a CI artifact).

<details><summary>Answer (sketch)</summary>

```yaml
coverage:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - uses: Swatinem/rust-cache@v2
    - uses: taiki-e/install-action@v2
      with: { tool: cargo-llvm-cov }
    - run: cargo llvm-cov --workspace --lcov --output-path lcov.info --fail-under-lines 80
    - uses: actions/upload-artifact@v4
      with: { name: lcov, path: lcov.info }
```
</details>

---

## E5.7 — Property test the cursor encoding (Stretch)

If you implemented keyset pagination in E4.5, add a property test that `decode(encode(c)) == c` for every `Cursor { i }`.

<details><summary>Answer</summary>

```rust
proptest! {
    #[test]
    fn cursor_round_trip(i in 1i64..i64::MAX) {
        let c = Cursor { i };
        let s = encode(&c);
        let back = decode(&s).unwrap();
        prop_assert_eq!(c.i, back.i);
    }
}
```
</details>
