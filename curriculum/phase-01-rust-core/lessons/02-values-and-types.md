# Lesson 1.2 — Values and Types

> **Concept first:** every piece of data in Rust has a *type*, and the compiler knows it. Knowing your types is half of programming.
> **Time:** 45 minutes.

## What is a "type"?

A **type** is a label that tells the compiler: "this piece of data is shaped like X, supports operations Y, and takes Z bytes of memory."

In Python you write `x = 5` and Python figures out the rest. In Rust you write `let x = 5;` and the compiler *infers* a type — but the type is real and fixed forever from that moment on. You can't add a number to a string by accident.

> Mental model: think of types as colored boxes. A red `i32` box can hold integers. A blue `String` box can hold text. You can't put text in the red box, and the compiler won't let you try.

## Scalar types

The "atoms" of Rust. Five categories:

### Integers

| Type | Range | When to use |
|---|---|---|
| `i8`, `i16`, `i32`, `i64`, `i128` | Signed, 8/16/32/64/128 bits | `i32` is the default integer; `i64` for IDs, timestamps, money cents |
| `u8`, `u16`, `u32`, `u64`, `u128` | Unsigned, same widths | Bytes (`u8`), counters that can't be negative |
| `isize`, `usize` | Pointer-sized | Indexing into arrays/slices (`usize`) |

```rust
let n: i32 = 42;
let big: i64 = 9_999_999_999;     // underscores allowed for readability
let hex: u32 = 0xFF;
let bin: u8  = 0b1010_1010;
```

Two rules to remember:

1. **No silent overflow.** In debug builds, `i32::MAX + 1` *panics*. In release builds it wraps. Use `.checked_add`, `.saturating_add`, or `.wrapping_add` when you need explicit behaviour.
2. **No implicit conversion.** `let x: i64 = some_i32;` is an error. You must write `let x: i64 = some_i32 as i64;` or `let x: i64 = some_i32.into();`.

### Floating point

```rust
let pi: f64 = 3.14159;
let half: f32 = 0.5;
```

`f64` is the default. **Never use `f32`/`f64` for money** — we'll cover the `Money` newtype in Phase 8.

### Booleans

```rust
let logged_in: bool = true;
let admin = false;            // type inferred as bool
```

### Characters

A `char` is a **single Unicode scalar value**, four bytes wide. Not a byte.

```rust
let letter: char = 'A';
let emoji:  char = '🚀';      // valid
```

### The unit type `()`

A type with exactly one value (`()`), used when a function doesn't return anything meaningful. You'll see it constantly.

```rust
fn log(msg: &str) -> () {       // explicit; the -> () is usually omitted
    println!("{msg}");
}
```

## Compound types

### Tuples

A fixed-size, heterogeneous collection. Index with `.0`, `.1`, etc., or destructure.

```rust
let user: (i32, &str, bool) = (42, "alice", true);
let (id, name, active) = user;          // destructuring
println!("{id}, {name}, {active}");

let only_id = user.0;
```

The unit type `()` is just the empty tuple.

### Arrays

A fixed-size, **homogeneous** collection on the stack. Length is part of the type.

```rust
let primes: [i32; 5] = [2, 3, 5, 7, 11];
let zeros = [0u8; 32];                  // 32 zeroes
let first = primes[0];                  // bounds-checked at runtime
```

Arrays are rare in everyday Rust because they have a fixed compile-time length. We mostly use:

### Vectors (`Vec<T>`)

A growable, heap-allocated array. The Rust equivalent of Python's `list` or Java's `ArrayList`.

```rust
let mut v: Vec<i32> = Vec::new();
v.push(1);
v.push(2);
v.push(3);
println!("len={}", v.len());            // len=3
```

There's a shortcut: `vec![1, 2, 3]`.

### Strings

This trips up everyone, so we'll cover it carefully.

- **`&str`** — a *string slice*. A view into UTF-8 text someone else owns. Always borrowed.
- **`String`** — an owned, growable, heap-allocated UTF-8 string. The thing you build up and pass around.

```rust
let literal: &str = "hello";                  // baked into the binary
let owned:   String = String::from("hello");  // heap-allocated copy
let also_owned = "hello".to_string();         // same as String::from

let mut growable = String::new();
growable.push_str("hello, ");
growable.push_str("world!");
```

> 90% of function signatures take `&str` (they don't need ownership) and return `String` (they create new text). Internalize that pattern.

## `let`, `let mut`, and shadowing

Everything is **immutable by default**.

```rust
let x = 5;
x = 6;                  // ❌ compile error: cannot assign twice
```

Opt in with `mut`:

```rust
let mut x = 5;
x = 6;                  // OK
```

You can also **shadow** a binding by `let`-ing it again — even with a different type:

```rust
let x = "5";            // x is &str
let x: i32 = x.parse().unwrap();   // x is now i32
```

Shadowing is *not* mutation; the new `x` is a brand-new variable that happens to reuse the name. It's especially useful for parse-then-validate flows.

## Type inference

The compiler is smart. Most of the time you can omit the type and it'll figure it out:

```rust
let x = 5;              // inferred i32
let y = 3.14;           // inferred f64
let z = "hello";        // inferred &str
```

But you'll see type annotations in three situations:

1. **Top-level function signatures.** Always annotate parameters and return types.
2. **When parsing.** `let n: i32 = "5".parse().unwrap();` — `parse` is generic and needs a hint.
3. **In data structures and APIs you're publishing.** Explicit is kinder to readers.

## Try it

Open a REPL-style scratch file and play:

```bash
cargo new --bin scratch && cd scratch
```

`src/main.rs`:

```rust
fn main() {
    // Numbers
    let i = 42;
    let f = 3.14;
    println!("i={i}, f={f}, sum={}", (i as f64) + f);

    // Strings
    let greeting = String::from("Hello, ");
    let name = "world";
    println!("{greeting}{name}!");

    // Tuples and arrays
    let point = (1.0, 2.0);
    println!("x={}, y={}", point.0, point.1);

    let nums = [10, 20, 30, 40];
    let sum: i32 = nums.iter().sum();
    println!("sum of {nums:?} = {sum}");

    // Vec
    let mut words: Vec<String> = vec!["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    words.push("d".to_string());
    println!("{words:?}");
}
```

```bash
cargo run
```

## Why this matters

Three principal-engineer takeaways:

1. **Knowing types lets you read code without running it.** When you see `fn save_user(u: &User) -> Result<UserId, DbError>`, you already know what it takes, what it returns, and what can go wrong.
2. **The smallest correct type is the safest one.** Using `u8` for HTTP status codes documents intent. Using `i64` for timestamps documents intent. Boring is good.
3. **Strings are surprisingly expensive.** Allocating a `String` calls into the heap. Reading docs that take `&str` instead of `String` means no copy and no allocation. We obsess over this in hot paths.

## Green-bar checkpoint

- You can explain the difference between `&str` and `String`.
- You can explain `let` vs `let mut` vs shadowing.
- You can write a function signature that takes a string slice and returns an owned string.

Next: `lessons/03-control-flow.md`.
