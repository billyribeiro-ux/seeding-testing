//! Test-data factories (Phase 5 — E5.4).
//!
//! A factory is a tiny builder that mints a *fully-formed* domain
//! object with sensible defaults you can override per-test. The point
//! is to make every test obviously specify what's UNIQUE about its
//! input, without the noise of "boilerplate I need to satisfy `add`."
//!
//! ```ignore
//! // Without a factory — every test re-specifies what doesn't matter.
//! let note = sqlx_notes::add(&pool, "any old body").await?;
//!
//! // With a factory — the test focuses on what DOES matter.
//! let note = sqlx_notes::factory::note().insert(&pool).await;
//! let note = sqlx_notes::factory::note().body("specific text").insert(&pool).await;
//! ```
//!
//! The default body comes from `fake::faker::lorem::en::Sentence(1..3)`
//! — a deterministic-by-seed phrase, varied enough that tests don't
//! collide on identical text and that snapshots stay realistic.

use fake::Fake;
use fake::faker::lorem::en::Sentence;
use sqlx::SqlitePool;

use crate::Note;

/// Builder for a fully-formed `Note`. Each method returns `self` so
/// callers chain: `note().body("x").insert(&pool).await`.
#[derive(Debug, Default)]
pub struct NoteFactory {
    body: Option<String>,
}

impl NoteFactory {
    /// Build an empty factory. Prefer the free function [`note()`] in
    /// test code — it reads like English.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the body — useful when the test asserts on its exact
    /// content. Leave unset to get a fresh `fake` sentence.
    #[must_use]
    pub fn body(mut self, b: impl Into<String>) -> Self {
        self.body = Some(b.into());
        self
    }

    /// Insert into the DB and return the persisted row. Panics on
    /// failure — factories are test helpers and a DB write failing
    /// during setup is never expected.
    pub async fn insert(self, pool: &SqlitePool) -> Note {
        let body = self.body.unwrap_or_else(|| Sentence(1..3).fake::<String>());
        crate::add(pool, &body)
            .await
            .expect("NoteFactory::insert must succeed")
    }
}

/// Free function form — `note().body("x").insert(&pool).await`.
#[must_use]
pub fn note() -> NoteFactory {
    NoteFactory::new()
}
