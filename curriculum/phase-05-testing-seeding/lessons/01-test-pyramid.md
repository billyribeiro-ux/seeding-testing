# Lesson 5.1 — The Test Pyramid (or Trophy)

> **Concept first:** tests come in flavors. Spending the right amount on each flavor is half of "we ship reliably."
> **Time:** 20 minutes.

## The pyramid (classic)

```
            /\
           /  \   E2E (Playwright, smoke)
          /----\
         /      \  Integration (HTTP, DB)
        /--------\
       /          \  Unit (pure functions)
      /------------\
```

Read top-down:

- **E2E** — slowest, costliest, most fragile. A few golden-path tests max.
- **Integration** — exercises real boundaries (DB, HTTP, mail). Dozens per service.
- **Unit** — fastest, cheapest, most precise. Hundreds.

The pyramid balances *cost* (lower = cheap) against *confidence* (higher = closer to the user).

## The trophy (modern)

```
                  E2E  (few)
                 -----
                /     \
               /       \
              / Integ.   \  ← most of your tests
             /            \
            /--------------\
           /                \
          /   Unit (some)    \
         /                    \
        /  static analysis,    \
       / type-check, lint, fmt  \
        ------------------------
```

The trophy is the same idea with a different shape:

- **A wide base of static analysis** — Rust's type system, clippy, `cargo deny`, `cargo audit` — catches whole classes of bugs *for free*. Spend zero test minutes on them.
- **The middle tier (integration) is where most tests live** because that's where Rust's static analysis can't see anymore.
- **Unit tests target pure logic** — money math, parsers, transformations.
- **A small E2E layer** covers the golden paths a customer would walk.

We use the trophy.

## What to put where

| Code | Test it as |
|---|---|
| `Money::checked_add` | Unit + property |
| `sqlx_notes::add` | Integration (in-memory SQLite or testcontainers) |
| `notes-api` `/v1/notes` | Integration via `oneshot` |
| Stripe webhook handler | Integration (wiremock or stripe-cli trigger) |
| User signup → email → click link → login | E2E (Playwright) |

**Rule of thumb:** if your test mocks the database, you're testing the mock, not the system. Push it up to integration.

## What good coverage looks like

| Layer | Target coverage |
|---|---|
| Pure logic (`Money`, validators) | 100% (it's cheap) |
| Library crates with I/O (`sqlx-notes`) | ≥90% |
| HTTP handlers (`notes-api`) | ≥80% (every endpoint exercised, every error path probed) |
| Binary (`main.rs`) | not really measurable — keep it tiny |

`make verify`'s coverage step will fail the build under 80% per crate. Adjust per crate as you ship.

## The two questions before writing a test

1. **What invariant does this test prove?** "User cannot create a note longer than 4096 chars." If you can't name the invariant, you don't know what you're testing.
2. **Will this test fail when the bug it's meant to catch comes back?** A test that's never going to fail is documentation, not a test.

If neither answer is clean, delete the test.

## Anti-patterns to avoid

- **Tests with `if` branches.** A test should assert *one specific thing*. If it has conditionals, it's two tests pretending to be one.
- **`sleep(...)` in tests.** Flaky. Use `tokio::time::pause()` or polling with a budget.
- **Shared mutable state between tests** (a static `Vec`, a shared DB file). Hermetic > clever.
- **Snapshot tests of every JSON response.** Snapshot the *shape* of the API contract; not the body of every test fixture.
- **`#[ignore]` accumulating.** An ignored test is a dead test. Fix or delete.

## Why this matters

- **The pyramid/trophy gives you a *budget*** for testing — you can decide how much to invest at each layer instead of testing randomly.
- **Static analysis is the cheapest win in any codebase.** Lint and type-check first; cover the gaps with tests.
- **Integration tests at the boundary are the highest-leverage flavor for backend services** because they exercise the whole stack realistically.

## Green-bar checkpoint

- You can articulate the difference between pyramid and trophy and pick one.
- You can name the right test flavor for `Money::checked_add` vs `POST /v1/notes` vs "user signs up and pays."
- You can articulate two anti-patterns and one fix for each.

Next: `lessons/02-nextest.md`.
