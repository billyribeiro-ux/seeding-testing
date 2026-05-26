# Lesson 1.7 — Traits and Generics

> **Concept first:** a *trait* is a contract that says "any type that wants to be called X must provide these methods." A *generic* is a function or type that works for any type that fits a contract. Together they're how Rust does polymorphism.
> **Time:** 60 minutes.

## The big picture

In Python, "duck typing" says: if it walks like a duck and quacks like a duck, treat it as a duck. The check happens at runtime.

In Java, you'd write a `Duck` interface and have classes implement it. The check happens at compile time, but the dispatch happens at runtime via a vtable.

In Rust, you declare a `trait` (the contract), `impl` it for each type (the implementation), and call methods on it. The compiler picks the right implementation at *compile time* (zero runtime cost), and you can also opt into runtime dispatch when needed.

## Defining a trait

```rust
trait Greet {
    fn hello(&self) -> String;

    fn greet_twice(&self) -> String {              // default method
        format!("{} {}", self.hello(), self.hello())
    }
}
```

Two kinds of methods:

- **Required** (no body) — every implementor must provide them.
- **Default** (with body) — implementors may override, but don't have to.

## Implementing a trait

```rust
struct English;
struct Spanish;

impl Greet for English {
    fn hello(&self) -> String { String::from("hello") }
}

impl Greet for Spanish {
    fn hello(&self) -> String { String::from("hola") }
    fn greet_twice(&self) -> String { String::from("¡hola hola!") }   // override
}

fn main() {
    println!("{}", English.hello());                 // hello
    println!("{}", Spanish.greet_twice());           // ¡hola hola!
    println!("{}", English.greet_twice());           // hello hello
}
```

## Generics — parameterizing over types

A generic function works for *any* type that satisfies a constraint:

```rust
fn largest<T: PartialOrd>(items: &[T]) -> &T {
    let mut best = &items[0];
    for it in items.iter().skip(1) {
        if it > best { best = it; }
    }
    best
}

fn main() {
    let nums = [3, 1, 4, 1, 5, 9, 2, 6];
    println!("{}", largest(&nums));                  // 9

    let words = ["pear", "apple", "banana"];
    println!("{}", largest(&words));                 // pear
}
```

`<T: PartialOrd>` reads: "for any type `T` that implements `PartialOrd` (i.e. supports `<`/`>`)." This is a **trait bound**.

You can stack bounds:

```rust
fn print_and_compare<T: PartialOrd + std::fmt::Debug>(a: T, b: T) {
    println!("{a:?} vs {b:?}");
    if a > b { println!("a wins"); } else { println!("b wins"); }
}
```

Or use the `where` clause for readability when bounds get long:

```rust
fn complicated<T, U>(t: T, u: U) -> String
where
    T: std::fmt::Debug + Clone,
    U: AsRef<str> + Default,
{
    format!("{:?} {}", t, u.as_ref())
}
```

## Generic types

Same idea, applied to structs and enums.

```rust
struct Wrapper<T> {
    inner: T,
}

impl<T: std::fmt::Display> Wrapper<T> {
    fn shout(&self) -> String { format!(">>> {} <<<", self.inner) }
}

fn main() {
    let w = Wrapper { inner: 42 };
    println!("{}", w.shout());                       // >>> 42 <<<
}
```

`Option<T>` and `Result<T, E>` are just generic enums you can write yourself.

## Trait objects — runtime dispatch

When you need a *collection of mixed types* that all share a trait, you use a **trait object** with `Box<dyn Trait>` or `&dyn Trait`:

```rust
trait Speak {
    fn say(&self) -> String;
}

struct Dog;
struct Cat;
impl Speak for Dog { fn say(&self) -> String { "woof".into() } }
impl Speak for Cat { fn say(&self) -> String { "meow".into() } }

fn main() {
    let animals: Vec<Box<dyn Speak>> = vec![Box::new(Dog), Box::new(Cat)];
    for a in &animals {
        println!("{}", a.say());
    }
}
```

`dyn Speak` is "some unknown type that implements `Speak`." The compiler generates a vtable lookup. Slightly slower than the static version, but flexible. Use it when you genuinely need heterogeneity.

> Rule of thumb: prefer **`impl Trait`** for return types (static dispatch, faster) and **`dyn Trait`** when you really need runtime polymorphism (mixed collections, plugin systems).

## `impl Trait` — the "I don't want to write the type" feature

```rust
fn iter_evens() -> impl Iterator<Item = i32> {
    (0..).step_by(2)
}

fn main() {
    for n in iter_evens().take(5) {
        println!("{n}");                             // 0 2 4 6 8
    }
}
```

`impl Iterator<Item = i32>` means "some concrete iterator type the caller doesn't need to know." The compiler still picks one specific type — it just hides it from you.

## Standard-library traits you'll use constantly

| Trait | Used for | Notes |
|---|---|---|
| `Debug` | `{:?}` formatting | `#[derive(Debug)]` on everything |
| `Display` | `{}` formatting | Manual `impl` for user-facing strings |
| `Clone` / `Copy` | Duplicating values | `#[derive(Clone)]`; `Copy` only for trivially-copyable types |
| `Default` | A "zero" value | `#[derive(Default)]` |
| `From` / `Into` | Conversion | Implement `From` and you get `Into` for free |
| `Iterator` | `for` loops, `.map/.filter/.collect` | The workhorse of data pipelines |
| `PartialEq` / `Eq` / `PartialOrd` / `Ord` / `Hash` | Comparison / containers | Required for sets, maps, sorting |
| `Send` / `Sync` | Thread-safety markers | Phase 2 (Tokio) leans on these heavily |
| `Drop` | Resource cleanup | RAII; usually you don't implement it yourself |

## A worked example: `Display` for `Counts`

Recall in `projects/01-hello-cli/src/lib.rs`:

```rust
impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:>7} {:>7} {:>7}", self.lines, self.words, self.chars)
    }
}
```

Now `format!("{c}")` works for any `Counts`. That's the pattern: define a clean `Display` for any type users will see.

## Why this matters

- **Generics + traits are how Rust's stdlib stays tiny and powerful.** Iterators, collections, formatting — all built on the same handful of traits.
- **Trait bounds document contracts.** `fn save<T: Serialize>(t: &T) -> Result<()>` tells you at a glance what `save` requires of `T`.
- **Static dispatch is free.** `impl Trait` and generics compile to the same machine code as if you'd written each version by hand. No virtual-call cost.
- **Trait objects are a deliberate choice.** When you reach for `Box<dyn Trait>`, you're trading a tiny runtime cost for flexibility — that's a senior-engineer decision, not a default.

## Green-bar checkpoint

- You can write a trait, implement it for two types, and call methods on a `Vec<Box<dyn Trait>>`.
- You can read `fn save<T: Clone + Send + 'static>(item: T)` and explain each bound.
- You know when to use `impl Trait` vs `dyn Trait` vs a concrete type.

Next: `lessons/08-error-handling.md`.
