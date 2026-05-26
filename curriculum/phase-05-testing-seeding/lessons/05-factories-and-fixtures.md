# Lesson 5.5 — Factories and Fixtures

> **Concept first:** repeating `INSERT INTO users VALUES (...)` in every test is technical debt. Factories generate plausible data once, in one place, and every test calls them.
> **Time:** 20 minutes.

## What a factory is

A factory is a builder that creates valid domain values with realistic defaults, letting tests override only what they care about.

```rust
let alice = factory::user()
    .email("alice@example.com")
    .admin(true)
    .insert(&pool)
    .await;
```

Compared to the alternative:

```rust
let alice = sqlx::query_as::<_, User>(
    "INSERT INTO users (email, password_hash, is_admin, is_email_verified, created_at)
     VALUES ($1, $2, $3, $4, $5) RETURNING *",
)
.bind("alice@example.com")
.bind("$argon2id$v=19$m=65536,t=3,p=4$randomsalt$randomhash")
.bind(true)
.bind(true)
.bind(chrono::Utc::now())
.fetch_one(&pool)
.await
.unwrap();
```

…the factory call is *the test that's actually about the test.* The other code is noise.

## The `fake` crate

```rust
use fake::Fake;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;

let email: String = SafeEmail().fake();
let name:  String = Name().fake();
let age:   u8     = (18..80).fake();
```

`fake` ships generators for emails, names, addresses, lorem text, UUIDs, dates, and more. Use it inside factories to generate plausible defaults.

## Designing a factory

```rust
// sqlx-notes/src/factory.rs (added in this phase)
use fake::Fake;
use fake::faker::lorem::en::Sentence;

pub struct NoteFactory {
    body: Option<String>,
}

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

Three principles:

1. **`new()` defaults to *valid* data.** A factory call without overrides must produce a row the DB accepts.
2. **Overrides are typed and additive.** `.body("...")`, `.admin(true)` — chainable.
3. **`insert(&pool)` performs the side effect.** Don't combine "build" and "insert" — return the value if you want to inspect it before saving.

## Deterministic factories

Random data is nice for fuzzing, but a deterministic seed makes tests reproducible:

```rust
use fake::Fake;
use rand::SeedableRng;
use rand::rngs::StdRng;

let mut rng = StdRng::seed_from_u64(42);
let email: String = SafeEmail().fake_with_rng(&mut rng);
```

When a flaky test surfaces, you can rerun with the same seed and reproduce the exact data.

## Fixtures: composing factories

A *fixture* is a higher-level setup that uses multiple factories:

```rust
pub async fn signed_in_admin(pool: &PgPool) -> (User, Session) {
    let admin = factory::user().admin(true).insert(pool).await;
    let session = factory::session().for_user(admin.id).insert(pool).await;
    (admin, session)
}
```

Tests pull fixtures, not factories, when they care about *the relationship between* entities. Layer them: factories → fixtures → tests.

## What about test data for *demos*?

Factories generate noise; demo data needs to be *deliberate*. We separate them:

- `factory::user()` produces randomish users for tests.
- `seed::demo()` produces a curated, idempotent set of users (`alice@example.com`, `bob@example.com`, …) — see the next lesson.

Don't reuse factories in demo seeds. Different audiences, different goals.

## Why this matters

- **A test's signal-to-noise ratio is the difference between maintainable and dreaded.** Factories push the noise into one file.
- **Plausible defaults catch bugs random data might hide.** A factory that always produces `is_email_verified: true` because that's the "happy path" hides the unverified case — write a separate `.unverified()` builder.
- **Fixtures encode the system's *vocabulary*.** "Signed-in admin," "user with a pending subscription" — these become the named scenes your tests draw from.

## Green-bar checkpoint

- You can write a factory with two overrides and a sensible default.
- You can write a fixture that composes two factories.
- You can articulate when to use `fake` random data vs deterministic data.

Next: `lessons/06-seeding-strategies.md`.
