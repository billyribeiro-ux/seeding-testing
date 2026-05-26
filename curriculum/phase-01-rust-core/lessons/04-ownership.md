# Lesson 1.4 — Ownership

> **The most important lesson in this curriculum.** Read it twice. Sleep on it. Read it again.
> **Time:** 60 minutes minimum.

## Why does this exist?

Other languages solve memory management in one of two ways:

1. **Garbage collection** (Java, Go, Python, JavaScript). A separate thread periodically scans your memory and frees what's no longer used. You don't think about memory; you pay a runtime cost (pauses, more CPU, more RAM).
2. **Manual** (C, C++). You explicitly allocate and free. Bugs include: forgetting to free (leak), freeing twice (corruption), using after freeing (security holes), data races between threads.

Rust took a third path: **the compiler proves at compile time that memory is freed exactly once, when nobody is still looking at it**. No garbage collector, no manual `free`. The mechanism is called **ownership**.

The cost is up-front mental effort. The benefit is: programs that compile are usually correct. Bugs that plague every other systems language *cannot exist* in safe Rust.

## The three rules

Memorize these. Tattoo them. They are the entire system:

> 1. **Every value has exactly one *owner*.**
> 2. **When the owner goes out of scope, the value is dropped (memory freed).**
> 3. **There can be many immutable references OR one mutable reference. Never both.**

Rule 3 covers the next lesson (borrowing). Rules 1 and 2 are this lesson.

## Move semantics

When you assign one variable to another, ownership *moves*. The original variable is no longer valid.

```rust
let s1 = String::from("hello");
let s2 = s1;                          // ownership moves from s1 to s2
println!("{s1}");                     // ❌ compile error: borrow of moved value
println!("{s2}");                     // ✓
```

That looks weird the first time. In Python, `s2 = s1` would just give you two names for the same object. In C++, it'd be a copy. In Rust, it's a move — `s1` is no longer a thing you can use.

Why? Because `String` owns a heap allocation. If both `s1` and `s2` pointed to it, then when `s1` goes out of scope at the end of the function it would try to free the allocation — but `s2` still holds a pointer to it. Double-free. Crash. Security hole. Rust prevents this by saying: *only one binding owns the buffer.*

Visually:

```
Before:   s1 ───► [ "hello" on heap ]
After:    s2 ───► [ "hello" on heap ]
          s1 ──╳── (invalid, can't use)
```

## What about `Copy` types?

Some types are cheap and trivial to duplicate — integers, booleans, characters, tuples of `Copy` types, and a few others. They opt into the **`Copy` trait**, which means assignment *copies* instead of moves:

```rust
let x = 5;
let y = x;                            // copy, not move
println!("{x} {y}");                  // ✓ both still valid
```

Rule of thumb: anything that lives entirely on the stack (no heap allocation) is `Copy`. `String`, `Vec<T>`, `Box<T>`, `File`, `TcpStream` — these are not `Copy`. They have heap-allocated parts or OS resources that need careful handoff.

## Passing to functions

Same rule. When you pass a non-`Copy` value to a function, ownership moves into the function.

```rust
fn takes_ownership(s: String) {       // s now owns the string
    println!("{s}");
}                                     // s goes out of scope, string is dropped

fn main() {
    let greeting = String::from("hi");
    takes_ownership(greeting);
    println!("{greeting}");           // ❌ moved into the function, gone
}
```

For `Copy` types, the value is *copied* in, so the original is still valid:

```rust
fn takes_a_number(n: i32) {
    println!("{n}");
}

fn main() {
    let x = 5;
    takes_a_number(x);
    println!("{x}");                  // ✓ still valid (i32 is Copy)
}
```

## Returning ownership

A function can give ownership back by returning the value:

```rust
fn append_world(mut s: String) -> String {
    s.push_str(", world");
    s                                 // returns ownership
}

fn main() {
    let s = String::from("hello");
    let s = append_world(s);          // re-bind to recover
    println!("{s}");                  // "hello, world"
}
```

This works but is awkward. The next lesson introduces *references*, which let you "lend" a value to a function without giving it away. That's how 95% of real code passes data around.

## The `Drop` trait — when ownership ends

When a value's owner goes out of scope, Rust calls a method called `drop()` to clean up. For `String`, `drop` frees the heap allocation. For `File`, `drop` closes the file descriptor. For `TcpStream`, `drop` closes the socket.

You almost never write `Drop` implementations yourself in application code. You rely on the standard library and libraries you use to clean up correctly. This pattern is called **RAII** (Resource Acquisition Is Initialization) — every resource is tied to a value's lifetime.

Example: file handles close automatically.

```rust
{
    let f = std::fs::File::open("README.md").unwrap();
    // … read from f …
}                                     // f goes out of scope here; file closes
```

No `defer`, no `try-finally`, no `using` block. The scope rules do the work.

## Three patterns that confuse beginners

### 1. Cloning

When you genuinely need two independent copies of a non-`Copy` value, call `.clone()`. It's explicit (so you can see the allocation in the source) and intentional.

```rust
let s1 = String::from("hello");
let s2 = s1.clone();                  // deep copy
println!("{s1} {s2}");                // both valid
```

Beginners over-use `.clone()`. The first instinct on a compile error is "just clone it." Resist this. Most of the time, the right answer is a *reference* (next lesson).

### 2. Returning a function-local value

```rust
fn make_greeting(name: &str) -> String {
    let g = String::from("hello, ") + name;
    g                                 // ownership moves out of the function
}
```

Returning an owned value from a function is fine — ownership simply moves out. This is how factories, builders, and constructors work.

### 3. `String` vs `&str` in function signatures

```rust
fn shout(s: String) -> String { s.to_uppercase() }      // takes ownership
fn shout(s: &str)   -> String { s.to_uppercase() }      // borrows, returns owned
```

Prefer the `&str` version unless you specifically need to consume / mutate / store the string. The `&str` version is more flexible — callers can pass `&my_string`, `"literal"`, `&s[..]`, etc.

## A worked example

```rust
fn main() {
    let owner = String::from("the ring");

    let owner = forge(owner);                   // ownership moves in, comes back
    println!("forged: {owner}");

    let copy = owner.clone();                   // explicit deep copy
    consume(copy);                              // copy moves in and is dropped
    // println!("{copy}");                      // ❌ would fail: copy was consumed
    println!("original still alive: {owner}");
}

fn forge(mut s: String) -> String {
    s.push_str(" of power");
    s
}

fn consume(s: String) {
    println!("consuming: {s}");
}                                               // s dropped here
```

Run it; trace the ownership in your head; predict the output before checking.

## Why this matters

- **Performance.** No GC overhead, no reference counts. Data structures look exactly like C — they just can't be misused.
- **Concurrency.** Because ownership is so strict, the compiler can prove that a thread either owns a value or shares it immutably. Data races become a *compile error*. Phase 2 (Tokio) leans on this hard.
- **Resource safety.** Files, sockets, DB connections, lock guards — they all close themselves when their owner goes out of scope. No leaks.

## Green-bar checkpoint

- You can predict whether a line will compile or not, based on whether ownership has moved.
- You can explain why `String` is not `Copy` but `i32` is.
- You can rewrite a `clone`-heavy function to use references instead — *after* reading the next lesson.

Take a break. Walk around. Re-read the three rules. Move to `lessons/05-borrowing-and-lifetimes.md` when you're ready.
