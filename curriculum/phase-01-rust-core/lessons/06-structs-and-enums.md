# Lesson 1.6 — Structs and Enums

> **Concept first:** types you make yourself. Structs group related data; enums say "this value is one of these alternatives." Both are central to *modeling your domain*.
> **Time:** 45 minutes.

## Structs

A `struct` bundles named fields into a new type.

```rust
struct User {
    id: i64,
    email: String,
    is_admin: bool,
}

fn main() {
    let u = User {
        id: 1,
        email: String::from("alice@example.com"),
        is_admin: false,
    };
    println!("user {} <{}>", u.id, u.email);
}
```

### Three flavours

1. **Named-field structs** (above). The default. Use them.
2. **Tuple structs.** Like tuples but with a name. Useful for newtypes:
   ```rust
   struct UserId(i64);
   struct Cents(i64);
   ```
   Now `fn pay(amount: Cents)` is *impossible* to call with a raw `i64` — you'd have to write `pay(Cents(500))`. The compiler enforces units.
3. **Unit structs.** No fields, just a name. Used as markers (we'll see them in trait code).

### Methods on structs

```rust
impl User {
    fn new(id: i64, email: String) -> Self {
        Self { id, email, is_admin: false }      // shorthand: field name = local name
    }

    fn promote(&mut self) {
        self.is_admin = true;
    }

    fn greeting(&self) -> String {
        format!("hello {}!", self.email)
    }
}
```

Three conventions to internalize:

- **`fn new` returns `Self`.** "Constructor" by convention; not a special keyword.
- **`&self`** when the method only reads.
- **`&mut self`** when the method mutates.
- **`self`** (no `&`) when the method *consumes* the value. Used in builder patterns.

### `#[derive(...)]` — free implementations

Most structs want a handful of common behaviours. Instead of writing them, ask the compiler:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct User {
    id: i64,
    email: String,
}
```

| Derive | Gives you |
|---|---|
| `Debug` | `{:?}` formatter for `println!` |
| `Clone` | `.clone()` method (deep copy) |
| `Copy` | move → copy (only for trivially-copyable types) |
| `PartialEq`, `Eq` | `==` and `!=` |
| `PartialOrd`, `Ord` | `<`, `>`, `<=`, `>=` |
| `Hash` | usable as a `HashMap` key |
| `Default` | `Default::default()` produces an all-defaults instance |
| `serde::Serialize`, `serde::Deserialize` | JSON/etc. encoding |

You'll see entire structs starting with `#[derive(Debug, Clone, Serialize, Deserialize)]` in real code.

## Enums

An `enum` declares: "a value of this type is *one of* these variants." Rust enums are *sum types* (tagged unions), much richer than C-style enums.

```rust
enum Role {
    Member,
    Moderator,
    Admin,
}
```

Each variant can also carry data:

```rust
enum LoginEvent {
    Success { user_id: i64 },
    Failure { attempts: u32, reason: String },
    Locked,
}
```

That single type can express: "either a successful login with a user_id, or a failure with a count and a reason, or a lockout." No bag-of-optional-fields. No `if some_field_is_null`.

### `match` on an enum

The compiler **forces** you to handle every variant:

```rust
fn audit(e: &LoginEvent) -> String {
    match e {
        LoginEvent::Success { user_id }      => format!("user {user_id} logged in"),
        LoginEvent::Failure { attempts, reason } => format!("fail #{attempts}: {reason}"),
        LoginEvent::Locked                   => String::from("account locked"),
    }
}
```

Add a new variant `Reactivated` to the enum and every `match` becomes a compile error until you handle it. **The compiler refuses to let you forget.** This is one of the most underrated safety features in any language.

### The two enums you'll meet every day

#### `Option<T>` — "maybe a T, maybe nothing"

```rust
enum Option<T> {
    Some(T),
    None,
}
```

This is Rust's answer to `null`. There is **no `null` in Rust** — if a value might be absent, the type is `Option<T>` and you must handle the `None` case. Half the bugs in JavaScript and Java simply cannot occur in Rust.

```rust
fn find_user(id: i64) -> Option<User> {
    if id == 1 { Some(User::new(1, "alice".into())) } else { None }
}

match find_user(1) {
    Some(u) => println!("found {u:?}"),
    None    => println!("not found"),
}
```

Helpful methods:

```rust
let x: Option<i32> = Some(5);
let y = x.unwrap_or(0);              // 5
let z = x.map(|n| n * 2);            // Some(10)
let w = x.unwrap_or_else(|| 0);      // 5

if let Some(n) = x { println!("{n}"); }    // unwrap-style without panic
```

> **Never use `.unwrap()` in production paths.** It panics on `None`. Reserve it for tests and for cases you've *proved* can't be `None`. Use `?` (next lesson) or `unwrap_or` in real code.

#### `Result<T, E>` — "either a T, or an error of type E"

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

Rust has no exceptions. Errors are *values* returned by functions. This is the single most important Rust idea after ownership.

```rust
fn parse_age(s: &str) -> Result<u32, std::num::ParseIntError> {
    s.parse::<u32>()
}

match parse_age("25") {
    Ok(n)  => println!("age = {n}"),
    Err(e) => println!("invalid: {e}"),
}
```

We'll cover error handling in depth in `lessons/08-error-handling.md`.

## Common patterns we use a lot

### Newtype wrappers for IDs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UserId(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OrgId(i64);
```

Now `fn move_user(user: UserId, to: OrgId)` *cannot* be called with the arguments swapped. The compiler catches it. We do this everywhere in the capstone.

### Builder pattern

```rust
#[derive(Default)]
struct UserBuilder {
    email: Option<String>,
    is_admin: bool,
}

impl UserBuilder {
    fn email(mut self, e: impl Into<String>) -> Self { self.email = Some(e.into()); self }
    fn admin(mut self)                          -> Self { self.is_admin = true; self }
    fn build(self) -> User {
        User { id: 0, email: self.email.unwrap(), is_admin: self.is_admin }
    }
}

let u = UserBuilder::default().email("bob@example.com").admin().build();
```

Takes `self` so each call consumes-and-returns the builder.

### State machines as enums

```rust
enum SubscriptionState {
    Trialing  { until: i64 },
    Active    { since: i64 },
    PastDue   { last_payment_at: i64 },
    Canceled  { at: i64 },
}
```

You'll use this exact pattern when we wire up Stripe subscriptions in Phase 8.

## Why this matters

- **Domain modeling.** A clear set of types is half the design of any service. If your data structure can represent invalid states (a `User` with both `is_admin: true` and `is_banned: true`?), bugs follow. Make invalid states *unrepresentable* in the type system.
- **Exhaustive matching.** Adding a new enum variant forces you to revisit every place that depends on the type. The compiler is your test suite.
- **No null.** `Option<T>` is louder and safer than `null`. You can't accidentally dereference it.

## Green-bar checkpoint

- You can model "an HTTP response is either a success body, a redirect URL, or an error with status + message" as an enum.
- You can convert a "stringly-typed" function (`fn get(role: &str)`) into one with an enum parameter.
- You understand why we wrap IDs in newtypes like `UserId(i64)`.

Next: `lessons/07-traits-and-generics.md`.
