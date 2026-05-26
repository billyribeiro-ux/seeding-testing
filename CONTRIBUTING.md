# Contributing

Whether you're improving a lesson, adding a project, or filing a bug —
welcome. This document collects the conventions that keep the
curriculum coherent.

## What we welcome

| Contribution | Where |
|---|---|
| Typo / grammar fixes in lessons | PR direct |
| New TROUBLESHOOTING entries | PR direct |
| Additional exercises (E*.N) | PR direct, see "Exercises" below |
| A new ADR for an architectural change | PR with ADR + code |
| A bug in a project's source | PR with a failing test that proves it, then the fix |
| A whole new phase | RFC first (see `docs/02-rfcs/`) |

## Setup

```bash
git clone git@github.com:billyribeiro-ux/seeding-testing.git
cd seeding-testing
rustup show          # confirm 1.95+ available
make verify          # all tests should pass before you start
```

For the SvelteKit app:

```bash
cd apps/memberclub/web
pnpm install
pnpm test
pnpm check
pnpm build
```

If `make verify` fails on a clean clone, that's a bug. File an issue.

## The quality gate

Every PR must pass:

1. `make verify` (fmt + clippy + nextest). CI runs the same thing.
2. `pnpm test && pnpm check && pnpm build` for any SvelteKit project
   under `apps/` or `projects/`.
3. `cargo deny check` (license + advisory checks).
4. `cargo audit`.
5. The `ci-passed` aggregate check (no required check directly — the
   aggregate gates branch protection).

CI fails any of these → no merge.

## Commits

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(phase-08): split off the webhook receiver as a separate project
fix(notes-api): drop the default limit to 20 to fix RSS regression
docs: clarify the cookie SameSite trade-off in lesson 6.2
test(rbac-policy-lab): cover the LastAdmin guardrail
refactor(money-lab): replace `unwrap` on internal invariants with debug_assert
chore(deps): bump axum to 0.8.10
```

`type` is the verb (feat/fix/docs/refactor/test/chore/perf).
`(scope)` is the project, phase, or area. Optional but useful.
`subject` is one line, imperative mood.

## Body

For non-trivial PRs, the commit body answers:

1. **What changed?** Concrete list.
2. **Why?** Reference the issue, ADR, or RFC that motivated it.
3. **How to verify?** The `make` target or test name.

## ADRs and RFCs

If your change touches the *shape* of the system — new service, new
data store, change of an existing pattern — write an ADR or RFC first:

- **ADR:** the decision is small (< 1 page), the team agrees.
- **RFC:** the decision is non-trivial (> 1 week of work), or the team
  wants to debate.

Templates are in `docs/01-architecture-decisions/0000-TEMPLATE.md` and
`docs/02-rfcs/0000-TEMPLATE.md`.

## Adding a lesson

Each phase has its own `lessons/` folder. A lesson is a `NN-<topic>.md`
file ~600–2500 words. Structure:

```md
# Lesson N.M — Title

> **Concept first:** the idea in one sentence.
> **Time:** estimated reading + coding time.

## Why it matters (the section title varies)

The mental model.

## The mechanics

The how.

## Why this matters

Three bullets. Not "summary"; *what to do differently* tomorrow.

## Green-bar checkpoint

- Two or three concrete things the learner can do.

Next: `lessons/NN-next.md`.
```

When you ship a new lesson, add it to the phase's `README.md` map.

## Exercises

Add to the phase's `EXERCISES.md`. Each exercise has:

- **Difficulty:** Easy / Medium / Stretch.
- **One sentence stating the task.**
- **Acceptance criteria** (tests pass / behavior observed).
- An optional `<details>` answer behind a tag.

If your exercise touches a project, the answer should be a *patch* the
learner could apply, not pseudocode.

## Adding a project

A new `projects/NN-*/` follows:

```
NN-name/
├── Cargo.toml
├── README.md          ← what it teaches, how to run, how to test
├── src/lib.rs         ← business logic, unit tests at the bottom
├── src/main.rs        ← thin CLI/HTTP wrapper (if needed)
└── tests/*.rs         ← integration tests
```

Three rules:

1. **Library + binary split.** Logic in `lib.rs`; plumbing in
   `main.rs`. Tests in `tests/` exercise the public API.
2. **Workspace-inherited deps.** `[dependencies] clap = { workspace = true }`,
   not a fresh version.
3. **Lints enabled.** `#[forbid(unsafe_code)]`,
   `#[warn(clippy::pedantic)]`. Document any pedantic suppressions in
   the file with a comment.

Add the new crate to the workspace `Cargo.toml` `[workspace]` members.

## Code style

- Rust: `cargo fmt` + clippy clean.
- TypeScript: Prettier default config (Svelte project sets it).
- Tests: hermetic, parallelism-safe (each test gets its own DB / temp
  dir / port).

## When in doubt

Open an issue. We'd rather discuss before you write a 500-line PR than
review it after.

## License

Contributions are dual-licensed under MIT or Apache-2.0 (matching the
repo).
