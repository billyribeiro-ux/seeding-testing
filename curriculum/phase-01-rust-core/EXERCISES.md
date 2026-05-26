# Phase 1 — Exercises

Eight graded drills. Each one builds on the lessons; some on `hello-cli` itself. Hidden answers behind `<details>` tags — try first, peek second.

---

## E1.1 — Predict the type (Easy)

For each expression, write what type Rust infers. (Hint: open `cargo expand` if you want to peek.)

```rust
let a = 5;
let b = 5_u64;
let c = 5.0;
let d = "hello";
let e = String::from("hello");
let f = vec![1, 2, 3];
let g = (1, "two", 3.0);
let h: [i32; 3] = [1, 2, 3];
```

<details><summary>Answer</summary>

- `a` — `i32` (the default integer type)
- `b` — `u64`
- `c` — `f64` (the default float)
- `d` — `&'static str`
- `e` — `String`
- `f` — `Vec<i32>`
- `g` — `(i32, &str, f64)`
- `h` — `[i32; 3]`
</details>

---

## E1.2 — Ownership trace (Easy)

Will this compile? If not, fix it.

```rust
fn main() {
    let s = String::from("hi");
    let t = s;
    println!("{s} {t}");
}
```

<details><summary>Answer</summary>

Won't compile — ownership moved from `s` to `t`. Fix either by cloning (`let t = s.clone();`) or by using a reference (`let t = &s; println!("{s} {t}");`). The reference is cheaper.
</details>

---

## E1.3 — Borrow checker fight (Easy)

Make this compile *without* `.clone()`:

```rust
fn main() {
    let mut v = vec![1, 2, 3];
    let first = &v[0];
    v.push(4);
    println!("{first}");
}
```

<details><summary>Answer</summary>

The problem: `&v[0]` borrows `v` immutably, then `v.push(4)` needs to borrow it mutably while `first` is still alive. Two fixes:

```rust
// (1) read & print first, then mutate
let first = v[0];           // i32 is Copy, so this isn't a borrow
v.push(4);
println!("{first}");

// (2) finish using first before mutating
let first = &v[0];
println!("{first}");        // last use of first
v.push(4);
```
</details>

---

## E1.4 — Replace `.unwrap()` (Medium)

Rewrite this so it returns `Result<i32, ParseIntError>` instead of panicking:

```rust
fn double_first(s: &str) -> i32 {
    let n: i32 = s.split(',').next().unwrap().parse().unwrap();
    n * 2
}
```

<details><summary>Answer</summary>

```rust
use std::num::ParseIntError;

fn double_first(s: &str) -> Result<i32, ParseIntError> {
    let first = s.split(',').next().unwrap_or("");   // empty if no comma
    let n: i32 = first.parse()?;
    Ok(n * 2)
}
```

Note: `.split(',').next()` always returns `Some(_)` for a `&str`, even if the input is empty — the first `unwrap()` is safe. But it's cleaner to swap it for `unwrap_or("")` so future readers don't have to reason about it.
</details>

---

## E1.5 — A typed error enum (Medium)

Write a `thiserror`-style error type for a tiny "username validator" with three failure modes:

- empty string
- contains whitespace
- longer than 32 chars

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UsernameError {
    // your variants here
}

pub fn validate(s: &str) -> Result<&str, UsernameError> {
    // implement
    todo!()
}
```

Then write three unit tests covering each error path.

<details><summary>Answer</summary>

```rust
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UsernameError {
    #[error("username cannot be empty")]
    Empty,
    #[error("username cannot contain whitespace")]
    HasWhitespace,
    #[error("username is too long: {0} chars (max 32)")]
    TooLong(usize),
}

pub fn validate(s: &str) -> Result<&str, UsernameError> {
    if s.is_empty() { return Err(UsernameError::Empty); }
    if s.chars().any(char::is_whitespace) { return Err(UsernameError::HasWhitespace); }
    if s.chars().count() > 32 { return Err(UsernameError::TooLong(s.chars().count())); }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rejects_empty()      { assert_eq!(validate(""),          Err(UsernameError::Empty)); }
    #[test] fn rejects_whitespace() { assert_eq!(validate("hi there"),  Err(UsernameError::HasWhitespace)); }
    #[test] fn rejects_too_long()   { assert_eq!(validate(&"a".repeat(33)), Err(UsernameError::TooLong(33))); }
    #[test] fn accepts_good_name()  { assert_eq!(validate("alice"),     Ok("alice")); }
}
```
</details>

---

## E1.6 — Add a `--bytes` flag to `hello-cli` (Medium)

Modify `projects/01-hello-cli`:

1. Add a `bytes` field to `Counts` and `Selection`.
2. Compute `bytes` as `input.len()`.
3. Add `-b, --bytes` flag to the `Cli`.
4. Update `or_default` and `render`.
5. Add unit tests for both the count and rendering.
6. Add an integration test for `--bytes`.
7. `make verify` must still pass.

<details><summary>Answer</summary>

A sketch of the patch:

```rust
// src/lib.rs
pub struct Counts { pub lines: usize, pub words: usize, pub chars: usize, pub bytes: usize }
pub struct Selection { pub lines: bool, pub words: bool, pub chars: bool, pub bytes: bool }

impl Counts {
    pub fn count(input: &str) -> Self {
        Self {
            lines: input.lines().count(),
            words: input.split_whitespace().count(),
            chars: input.chars().count(),
            bytes: input.len(),
        }
    }
}

// In Selection::or_default and Selection::render, add `bytes`.
// In src/main.rs Cli, add: #[arg(short = 'b', long)] bytes: bool
```

Tests:

```rust
#[test]
fn bytes_count_matches_utf8_byte_length() {
    let c = Counts::count("café"); // 5 bytes
    assert_eq!(c.bytes, 5);
}
```

Integration test:

```rust
#[test]
fn bytes_flag_prints_byte_count() {
    bin().arg("--bytes")
        .write_stdin("café")     // 5 bytes
        .assert().success()
        .stdout(predicate::str::contains("      5"));
}
```
</details>

---

## E1.7 — Newtype IDs (Medium)

Refactor the following function so it's impossible to call with arguments in the wrong order:

```rust
fn transfer(from: i64, to: i64, amount: i64) {
    // pretend to transfer
}

fn main() {
    let alice_id = 42;
    let bob_id = 99;
    let usd_cents = 1000;
    transfer(alice_id, bob_id, usd_cents);
    transfer(bob_id, alice_id, usd_cents); // also "valid" — but is it?
    transfer(usd_cents, alice_id, bob_id); // ☠️ catastrophic — typechecks
}
```

<details><summary>Answer</summary>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct UserId(pub i64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct Cents(pub i64);

fn transfer(from: UserId, to: UserId, amount: Cents) {}

fn main() {
    let alice = UserId(42);
    let bob   = UserId(99);
    let amount = Cents(1000);
    transfer(alice, bob, amount);
    // transfer(amount, alice, bob); // ✓ now a compile error
}
```

Newtype IDs are a *senior* habit. They cost almost nothing and eliminate an entire category of bug.
</details>

---

## E1.8 — Add a property test (Stretch)

Add the `proptest` crate to `hello-cli` (as a dev-dependency) and prove this invariant:

> For any string `s`, `Counts::count(s).chars` equals `s.chars().count()`.

<details><summary>Answer</summary>

```toml
# Cargo.toml dev-dependencies
proptest = "1"
```

```rust
// In src/lib.rs tests module:
use proptest::prelude::*;
proptest! {
    #[test]
    fn count_chars_matches_string_chars(s in ".*") {
        let c = Counts::count(&s);
        prop_assert_eq!(c.chars, s.chars().count());
    }
}
```

The regex `".*"` generates any UTF-8 string. proptest will hammer your code with thousands of random inputs — Unicode, ASCII, whitespace, empty — and only fail if the invariant breaks.
</details>
