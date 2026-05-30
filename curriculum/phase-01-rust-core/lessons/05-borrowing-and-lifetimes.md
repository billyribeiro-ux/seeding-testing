# Lesson 1.5 — Borrowing and Lifetimes

> **Concept first:** ownership says "one owner, one drop." Borrowing says "you can let others *look* without giving up ownership." Lifetimes are how the compiler tracks how long looks are allowed.
> **Time:** 60 minutes.

## The problem borrowing solves

The previous lesson left us with an awkward pattern:

```rust
fn append_world(mut s: String) -> String {     // takes ownership
    s.push_str(", world");
    s                                          // gives it back
}
```

Every function would need to take and return ownership of everything it touches. Reading a `Vec<User>` to count its length would require giving the function the entire vector and then asking for it back. Madness.

Borrowing is the fix: a function can *borrow* a value (get a temporary view of it) without owning it.

## References — `&T` and `&mut T`

A **reference** is a non-owning pointer to a value. Two flavours:

- `&T` — **shared reference** (read-only). You can have many of these at once.
- `&mut T` — **exclusive reference** (read/write). You can have exactly one at a time, and no shared refs while it exists.

```rust
fn length(s: &String) -> usize {              // borrows
    s.len()
}                                              // borrow ends; s still valid for caller

fn main() {
    let g = String::from("hello");
    let n = length(&g);                        // pass a shared reference
    println!("{g} is {n} chars");              // ✓ g still owned by main
}
```

The `&` in `&g` says "lend a shared reference to `g`". The `&String` in the function signature says "I take a shared reference to a `String`."

### Mutable references

```rust
fn append_world(s: &mut String) {              // exclusive borrow
    s.push_str(", world");
}

fn main() {
    let mut g = String::from("hello");
    append_world(&mut g);
    println!("{g}");                            // "hello, world"
}
```

Two requirements for `&mut T`:

1. The original binding must be `let mut`.
2. The caller writes `&mut g`, not just `&g`.

## The borrow checker's rule

> **You can have either:**
> - **Many shared references (`&T`)** — readers, no writers.
> - **One mutable reference (`&mut T`)** — one writer, no readers.

You can't have both at the same time. This single rule eliminates data races and aliasing bugs at compile time.

```rust
let mut v = vec![1, 2, 3];
let r1 = &v;
let r2 = &v;
println!("{r1:?} {r2:?}");                     // ✓ two shared refs
let r3 = &mut v;
println!("{r3:?}");                            // ❌ shared refs still in scope above
```

The compiler will tell you exactly which line conflicts with which.

> **Since the 2018 edition, the rule is "non-lexical lifetimes" (NLL):** a reference's lifetime ends at its *last use*, not the end of the enclosing block. So this works:
>
> ```rust
> let mut v = vec![1, 2, 3];
> let r1 = &v;
> println!("{r1:?}");                          // last use of r1
> let r2 = &mut v;                             // ✓ r1 is no longer "alive"
> r2.push(4);
> ```

## `&str` is a shared slice

We met `&str` in Lesson 1.2. Now you can see it for what it is: **a shared reference (`&`) to some UTF-8 text owned by someone else**. That's why functions prefer `&str`:

```rust
fn shout(s: &str) -> String {                  // borrow, don't own
    s.to_uppercase()                           // returns a new String
}

let owned = String::from("hi");
shout(&owned);                                  // pass &owned (auto-deref to &str)
shout("literal");                               // pass literal directly
```

The trick: `&String` automatically *coerces* to `&str` thanks to a feature called "deref coercion." Don't worry about the mechanics — just know it Just Works.

The same coercion exists for `Vec<T>` → `&[T]` (a *slice*). Prefer `&[T]` in function signatures.

## Lifetimes — when references go in and out of scope

A **lifetime** is the period during which a reference is valid. Most of the time, the compiler infers them and you don't have to think about it. But occasionally you have to write them down.

The syntax: `'a` (an apostrophe followed by a name). `'a` is a generic *lifetime parameter*, analogous to `T` for type parameters.

```rust
// "The returned &str lives at least as long as the input &str called 'a."
fn first_word<'a>(s: &'a str) -> &'a str {
    s.split_whitespace().next().unwrap_or("")
}
```

Without the `'a`, the compiler would have no idea whether the returned slice points into `s` or into a global static. The annotation says: *the output borrows from the input*.

### When you actually have to write lifetimes

Three situations:

1. **Functions that return references.** Tell the compiler which input the output borrows from.
2. **Structs that hold references.** Same idea.
3. **Trait implementations that involve references.** Same idea.

For 99% of application code, you'll never write a lifetime. Lifetime elision rules handle it. When the compiler asks you to write one, follow its suggestion — it's almost always exactly the right `<'a>` you needed.

### A struct holding a reference

```rust
struct Document<'a> {                           // 'a = "the input text's lifetime"
    title: &'a str,
}

impl<'a> Document<'a> {
    fn new(title: &'a str) -> Self {
        Self { title }
    }
}

fn main() {
    let s = String::from("The Hobbit");
    let doc = Document::new(&s);
    println!("{}", doc.title);
}
```

Why does this exist? Sometimes you want a struct that doesn't own its data — it just *points* at data owned elsewhere. Useful for parsers and zero-copy designs.

In *most* application code (Axum handlers, services, repositories), we prefer owned data (`String`, not `&str`). The reduced friction is worth a tiny allocation. We use references heavily in tight inner loops where allocations would dominate.

## A worked example: building on `hello-cli`

Recall `projects/01-hello-cli/src/lib.rs`:

```rust
impl Counts {
    pub fn count(input: &str) -> Self {        // borrow, don't own
        Self {
            lines: input.lines().count(),
            words: input.split_whitespace().count(),
            chars: input.chars().count(),
        }
    }
}
```

`Counts::count` takes `&str` because:

- It only *reads* the input; no need for `&mut str`.
- It doesn't store it; no need to own it.
- It works for *any* caller — `String`, `&str`, `&String`, even slices of a buffer.

This is the borrowing rule applied: take the weakest reference that does the job.

## Common borrow-checker fights and their fixes

| Compiler says… | What's wrong | Fix |
|---|---|---|
| "cannot borrow `x` as mutable, as it is also borrowed as immutable" | You have an `&x` still alive when you tried to make `&mut x` | Make the immutable borrow end sooner (let-bind and use, or restructure) |
| "borrowed value does not live long enough" | You're trying to return a reference to a local | Return the owned value instead, or take an `&'a T` input and tie the output's lifetime to it |
| "use of moved value" | A non-`Copy` value was moved and you used the old binding | Pass a reference (`&x`), not the value |
| "captured variable cannot escape `FnMut` closure body" | A closure tried to return a borrow of a temporary | Restructure to compute the value inside the closure |

When in doubt, **listen to the compiler**. Rust's error messages are the best in the industry — usually the suggested fix is correct.

## Why this matters

- **You stop allocating to placate the borrow checker.** Cloning is a code smell; references are the cure.
- **Function signatures become documentation.** `fn save(&self, u: &User) -> Result<()>` tells you at a glance: it borrows `self` shared, borrows the user shared, returns a unit on success.
- **Concurrency is approachable.** Phase 2 will lean on the fact that `&T` means "shareable" and `&mut T` means "exclusive." Threads can take `Arc<T>` (shared) but not `&mut T` to the same data simultaneously.

## Green-bar checkpoint

- You can rewrite a function that takes `String` to take `&str` instead.
- You can articulate why "many readers OR one writer" is the same idea as Rust's borrow rule.
- You can read a function signature like `fn longest<'a>(a: &'a str, b: &'a str) -> &'a str` and explain it in English.

Next: `lessons/06-structs-and-enums.md`.
