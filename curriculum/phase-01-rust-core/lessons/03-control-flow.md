# Lesson 1.3 — Control Flow

> **Concept first:** programs make decisions and loops. Rust's `if`, `match`, `for`, `while`, and `loop` cover all of them — but `match` is the secret weapon.
> **Time:** 30 minutes.

## `if` is an expression, not a statement

In many languages, `if` is a statement (it does something but doesn't have a value). In Rust, `if` is an **expression** — it produces a value, which you can assign.

```rust
let age = 17;
let category = if age >= 18 { "adult" } else { "minor" };
println!("{category}");
```

Three rules:

- The condition must be exactly a `bool`. `if 0 { … }` won't compile (no implicit truthiness).
- Both branches must produce the same type.
- Drop the trailing semicolon to make a block "return" its last expression.

`else if` chains as you'd expect:

```rust
let temp = 72;
let mood = if temp < 50 {
    "cold"
} else if temp < 75 {
    "comfortable"
} else {
    "hot"
};
```

## `match` — the secret weapon

`match` is `switch`-on-steroids: every possible value must be handled (exhaustiveness), and the compiler enforces it. It works on any type, including enums (Phase 1.6) and complex patterns.

```rust
let status = 404;
let label = match status {
    200..=299 => "success",
    300..=399 => "redirect",
    400 => "bad request",
    404 => "not found",
    400..=499 => "client error",
    500..=599 => "server error",
    _ => "unknown",
};
println!("{status} -> {label}");
```

Notes:

- `200..=299` is a *range pattern* (inclusive).
- The order matters: the first matching arm wins. (`404` is matched before `400..=499`.)
- `_` is "anything else" — required when other arms don't cover everything.
- The whole `match` is an expression, just like `if`. We use that constantly.

### Matching with bindings

```rust
let maybe_id: Option<i64> = Some(42);
match maybe_id {
    Some(id) if id > 0 => println!("positive id: {id}"),
    Some(id)           => println!("non-positive id: {id}"),
    None               => println!("no id"),
}
```

The `if id > 0` is a **guard** — a condition layered on top of the pattern. We use these in policy/RBAC code in later phases.

### `if let` — when you only care about one case

If you only care about *one* arm, `match` is overkill. `if let` exists for that:

```rust
let x: Option<i32> = Some(5);
if let Some(n) = x {
    println!("got {n}");
}
```

And the symmetric `let … else`:

```rust
fn parse_id(s: &str) -> i64 {
    let Ok(n) = s.parse::<i64>() else {
        return -1;                              // early return on parse failure
    };
    n
}
```

This early-return pattern is *everywhere* in production Rust. Get used to it.

## Loops

Three flavours, in order of preference:

### `for` — over an iterator (the boring, correct choice)

```rust
for i in 0..5 {
    println!("{i}");                            // 0 through 4
}

let names = vec!["alice", "bob", "carol"];
for name in &names {                            // & avoids consuming the Vec
    println!("hello, {name}");
}
```

> Use `for` for almost everything. Iterators in Rust are *zero-cost* — they compile down to the same machine code as a hand-rolled loop. Range, vector, hashmap, file lines — they all work the same way.

### `while` — when you need a condition, not a fixed range

```rust
let mut n = 10;
while n > 0 {
    println!("{n}");
    n -= 1;
}
```

### `loop` — infinite loop, optionally returning a value with `break`

```rust
let result = loop {
    let answer = compute();
    if answer.is_ok() {
        break answer.unwrap();                  // loop expression evaluates to this
    }
};
```

Rare but powerful — we use it in event loops and retry-with-backoff patterns.

### Labels and `continue`

When you need to break out of a nested loop:

```rust
'outer: for x in 0..10 {
    for y in 0..10 {
        if x * y > 50 {
            break 'outer;
        }
    }
}
```

You'll see `continue` and `break` with labels in algorithm-heavy code.

## A small worked example

A function that classifies an HTTP status code into a category, then logs every code from 100 to 599:

```rust
#[derive(Debug)]
enum Category { Info, Success, Redirect, ClientError, ServerError, Unknown }

fn categorize(status: u16) -> Category {
    match status {
        100..=199 => Category::Info,
        200..=299 => Category::Success,
        300..=399 => Category::Redirect,
        400..=499 => Category::ClientError,
        500..=599 => Category::ServerError,
        _         => Category::Unknown,
    }
}

fn main() {
    for code in 100..600 {
        if code % 100 == 0 {                   // print one per category boundary
            println!("{code} -> {:?}", categorize(code));
        }
    }
}
```

Drop that in `scratch/src/main.rs` and `cargo run`.

## Why this matters

- **Exhaustiveness** is your friend. When you add a new variant to an enum (e.g. a new `Role::Owner` to `enum Role { Admin, Member }`), every `match` on `Role` becomes a compile error until you handle the new case. **The compiler refuses to let you forget.**
- **`if let` and `let else`** are the antidotes to nested-`match` pyramids of doom.
- **`match` is an expression**, so you write `let x = match { … };` instead of `let mut x; match … { … x = …; }`. Cleaner, no uninitialized variables.

## Green-bar checkpoint

- You can rewrite a chain of `if/else if/else` as a `match`.
- You can use `let … else` to early-return on parse failure.
- You understand why iterating with `for x in &v` is preferable to indexing.

Next: `lessons/04-ownership.md`. The big one.
