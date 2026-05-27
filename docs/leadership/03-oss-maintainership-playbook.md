# Open Source Maintainership Playbook

What it takes to maintain a popular crate without burning out, getting
exploited, or shipping breakage. Written from the assumption that the
reader is an L6/L7 engineer about to release (or has just released) a
crate that other people depend on.

The cautionary tale that frames this whole document is xz-utils. In
2024, a state-level actor compromised one of the most widely deployed
compression utilities in the world by patiently building social
trust with the sole maintainer of an under-funded, under-staffed
project. Every concrete recommendation here is, in part, a defense
against that pattern.

## Table of contents

- [Before you publish: a pre-flight checklist](#before-you-publish-a-pre-flight-checklist)
- [CHANGELOG discipline](#changelog-discipline)
- [Semantic versioning, with practical edges](#semantic-versioning-with-practical-edges)
- [Yanked vs patched](#yanked-vs-patched)
- [Triage cadence](#triage-cadence)
- [Conventional commits and automated release notes](#conventional-commits-and-automated-release-notes)
- [The first-PR experience](#the-first-pr-experience)
- [Funding and sustainability](#funding-and-sustainability)
- [The maintainer's bill of rights](#the-maintainers-bill-of-rights)
- [Deprecation and the 18-month rule](#deprecation-and-the-18-month-rule)
- [Security disclosures](#security-disclosures)

## Before you publish: a pre-flight checklist

Most projects do not need to be public. Before you click `cargo
publish`, run this list. If you cannot say yes to all of them, keep
the crate internal.

- [ ] **A second maintainer is named in `CODEOWNERS` and has commit
      rights.** No solo-maintained public crates.
- [ ] **The crate has a `LICENSE-MIT` and `LICENSE-APACHE`** (or your
      org's explicit license choice — but having *some* license is
      non-negotiable; "no license" means "all rights reserved" and
      no one can legally use the code).
- [ ] **`README.md` shows what the crate is for, one example of
      using it, and the minimum supported Rust version (MSRV).**
- [ ] **`CHANGELOG.md` is initialized with `[0.1.0]` and a date.**
- [ ] **`CONTRIBUTING.md` exists and links to the issue templates.**
- [ ] **The repo has a `SECURITY.md` with a private disclosure
      address.** Not `security@yourcrate.dev` — a real address that a
      real human reads.
- [ ] **CI is green, including `cargo clippy -- -D warnings` and
      `cargo deny check`.**
- [ ] **You have a 12-month plan for who triages issues.** Not
      "we'll figure it out." A named cadence with named people.
- [ ] **You have answered the question "what is the support
      promise?"** in writing, even if the answer is "best effort, no
      SLA."

Reference: the root [`CONTRIBUTING.md`](../../CONTRIBUTING.md) in
this repo is a reasonable starting point.

## CHANGELOG discipline

Use [Keep a Changelog](https://keepachangelog.com/) format, with
five headings:

```
## [Unreleased]

### Added
### Changed
### Deprecated
### Removed
### Fixed
### Security
```

Two rules:

1. **The CHANGELOG is the source of truth, not the commit log.** A
   user who reads `git log` should be confused by the noise; a user
   who reads `CHANGELOG.md` should understand what changed and
   why. The commit log is for archaeologists; the CHANGELOG is for
   consumers.

2. **Every PR touches the CHANGELOG.** Make it a CI check. The
   reviewer doesn't need to enforce it; the bot does. We do this in
   our internal repos with a simple grep-based check in the root
   `.githooks/` directory.

Three tells of an unhealthy changelog:

- It hasn't been updated in 6 months. Either the crate is dead or
  the maintainer is shipping silently.
- Entries are commit-message-style ("fixed bug"). They should be
  user-style ("`Money::new` no longer rejects values exactly equal
  to the ceiling; the ceiling is now exclusive as documented").
- The CHANGELOG is generated from commit messages by a bot with no
  human review. Generation is fine; *publication* without review is
  not.

This repo's root [`CHANGELOG.md`](../../CHANGELOG.md) is your
reference template.

## Semantic versioning, with practical edges

Standard SemVer (MAJOR.MINOR.PATCH) is the contract. Three practical
edges that the spec doesn't address well:

### MAJOR bumps

You promise the world a breaking change. Three reasons to bump:

1. **A type signature changed.** Including subtle: a function that
   used to return `Result<T, E1>` now returns `Result<T, E2>` is a
   MAJOR bump even if `E2: From<E1>` (because users matching on the
   error variants break).
2. **A trait got a new required method.** Default impls keep it
   MINOR; required methods are MAJOR.
3. **A dependency you re-export bumped its own MAJOR.** This is the
   one people forget. If you re-export `serde::Deserialize` and
   `serde` bumps MAJOR, you must MAJOR-bump too.

### MINOR bumps

You added something new. The bar for MINOR is "consumers don't have
to change a single line of their code to upgrade." Subtle traps:

- Adding a new variant to a `pub enum`. This is breaking unless you
  marked the enum `#[non_exhaustive]` from day 1. If you didn't,
  treat it as MAJOR.
- Adding a new field to a `pub struct`. Same rule, same fix:
  `#[non_exhaustive]` from day 1.
- "Loosening" an MSRV downward is MINOR (no one is hurt by older
  Rust working). Raising it is MAJOR (people on the old Rust break).

### PATCH bumps

Bug fixes. The bar: "the *documented* behavior didn't change; an
*undocumented* behavior may have." The slippery slope is the user
who depended on the undocumented behavior. The rule:

- If the change is fixing a clear bug (panic on input the docs said
  was supported), it's PATCH and any user who depended on the bug
  is out of luck.
- If the change is *normalizing* a behavior the docs were silent
  on, write a CHANGELOG entry and consider it PATCH-with-warning.
- If the change is fixing a security issue that requires user
  action, it's PATCH but it is also a SECURITY entry in the
  CHANGELOG and a published advisory.

### 0.x.y is a trap

Crates that stay on 0.x indefinitely send the message "the API
isn't stable." Some maintainers do this intentionally and forever;
the result is that downstream consumers either pin to a specific
0.x.y forever or live with constant churn. Pick: ship 1.0 within
12 months of public release, or commit publicly to a longer 0.x
phase with a date attached.

## Yanked vs patched

These get confused. They're different tools.

**Patched** = "We released a new version that fixes the bug. Please
upgrade." The old version remains on crates.io; existing builds
keep working.

**Yanked** = "Please don't *start* using this version. Builds that
already use it keep working (Cargo respects the lockfile), but new
projects will not resolve to it."

Use **patched** for almost every bug. Use **yanked** when:

- You shipped a version with a security advisory and want to push
  new projects off of it.
- You shipped a version that *cannot build* (broken Cargo.toml,
  missing files, etc.).
- You accidentally shipped a version with the wrong MAJOR/MINOR
  (it should have been 0.2.0, you tagged 0.3.0).

Three things yank does not do:

- It does **not** delete the version. crates.io is append-only by
  design.
- It does **not** notify users automatically. You still write a
  CHANGELOG entry and (for security) a published advisory.
- It does **not** break existing lockfiles. Yanking is a *new
  resolution* hint, not a stop-the-world.

When in doubt, patch and don't yank. Yanking is for emergencies.

## Triage cadence

Cadence is everything. The xz-utils story is, partly, the story of
a maintainer who triaged irregularly and was overwhelmed by the
incoming volume. The fix is to triage on a *schedule*, in *batches*,
with a hard *time budget*.

### The Monday morning batch (recommended)

Every Monday, the maintainer (or rotating maintainer) spends 90
minutes triaging issues and PRs. The bar is *triage*, not *resolve*.

For each open item:

- Is it a duplicate? Close with a link to the canonical issue.
- Is it a question? Convert to a discussion thread.
- Is it actionable? Label it (`bug`, `enhancement`,
  `documentation`, `good first issue`, `help wanted`, `wontfix`).
- Is it stale (no activity in 90 days)? Add a stale comment; close
  in 30 if no response.
- Is it a security issue posted publicly? Move it to private
  disclosure immediately, apologize to the reporter, follow the
  security policy.

What 90 minutes does *not* mean: 9 hours spread thinly across the
week. The point of the batch is that it has an *end*. When the 90
minutes is up, you stop. Items that didn't get triaged this week
get triaged next week. You can ship code on Tuesday with a clear
conscience.

### Why batching matters

Two reasons:

1. **Context-switching cost.** Bouncing between triage and code work
   is the most expensive thing a maintainer's brain can do. Batching
   contains the cost.

2. **Boundaries.** A maintainer who answers issues at 11pm signals
   to the community that answering issues at 11pm is the norm. Burn
   out follows. Batching teaches users what to expect.

### Labels that earn their keep

- `good first issue` — clearly scoped, well-documented, no prior
  context required. Most repos under-label this; consequence: no
  one new contributes.
- `help wanted` — the maintainer can name what help they need but
  doesn't have time. Honest.
- `discussion` — meta or design conversation; not actionable.
- `blocked` — actionable but waiting on an upstream change.
- `breaking-change` — flagged for the next MAJOR bump.
- `do-not-merge` — sometimes needed; explain why in a comment.

Resist label sprawl. 8 labels is plenty. 30 labels is theater.

## Conventional commits and automated release notes

[Conventional Commits](https://www.conventionalcommits.org/) is the
format:

```
<type>(<scope>): <description>

<body>

<footer>
```

The types:

- `feat:` → MINOR bump.
- `fix:` → PATCH bump.
- `feat!:` or `fix!:` or any type with `BREAKING CHANGE:` in the
  footer → MAJOR bump.
- `docs:`, `test:`, `chore:`, `refactor:`, `perf:`, `ci:`, `style:`
  → no version bump.

Why this matters: if every commit on `main` follows the format, the
*next version number* is automatically computable, and the
*CHANGELOG entry* is automatically generated.

This repo's root [`CONTRIBUTING.md`](../../CONTRIBUTING.md) sets
this up; the principle is the same for any crate.

### Automation, with a human checkpoint

- Set up a release-please-style bot (or `cargo-release`) to generate
  a release PR with the next version number and the CHANGELOG diff.
- **Do not** auto-merge the release PR. The human checkpoint is
  what separates "I shipped" from "the bot shipped on my behalf."
- The bot's CHANGELOG is a *draft*. The human reviews it, edits the
  entries for user-style prose, and *then* merges.

### Three things automation will not do for you

1. **It will not write the user-facing migration guide.** A MAJOR
   bump needs prose, not just a changelog list of changed types.
2. **It will not decide when a feature is ready.** "We have N
   `feat:` commits queued" is not the same as "the next release is
   ready to ship."
3. **It will not notice that a `fix:` is actually a `feat!:`.** The
   maintainer is the only check on commit-message honesty. Cultivate
   it; the team's discipline here is load-bearing.

## The first-PR experience

The single biggest predictor of a healthy contributor community is
what happens to a person who lands their first PR. If it's smooth,
they come back. If it's not, they don't, and the people they tell
won't either.

The mechanics:

### Issue templates

`.github/ISSUE_TEMPLATE/bug_report.md` and `feature_request.md`
should ask, at minimum:

- The version of the crate.
- The Rust version (`rustc --version`).
- Steps to reproduce. (Not "it doesn't work.")
- Expected vs actual behavior.
- Any error message or backtrace.

Resist asking for more than this. Every additional field is a
barrier; barriers are paid most heavily by the people you most want
to attract.

### PR templates

`.github/PULL_REQUEST_TEMPLATE.md`:

- What changed and why.
- Linked issue (if any).
- Did you update the CHANGELOG?
- Did you add tests?
- Did you check `cargo clippy -- -D warnings`?

### Friendly on-ramp

Tag at least 3–5 open issues `good first issue` at all times. When
one gets claimed, replace it. These are your customer-acquisition
funnel for contributors.

A good "first issue" has:

- A clear, scoped description (1–3 paragraphs).
- A pointer to the file(s) that need to change.
- An expected test that should pass when done.
- A note that the maintainer is happy to mentor.

### The first-PR review

Your tone here matters more than your code review skill. Three
rules:

1. **Greet by name.** "Hey [name], thanks for the PR." Two
   seconds, infinite goodwill.
2. **Acknowledge the work before suggesting changes.** "This is a
   good direction. A couple of tweaks before we merge:"
3. **Suggest, don't dictate.** "Could we extract this into a helper
   function? It's used three times now." not "Extract this into a
   helper."

If you have to reject a PR, do it in two messages: the first thanks
them and explains why; the second invites them to a related issue
they could work on instead.

### What "merging" should feel like

- The PR should not sit open for more than 2 weeks without
  maintainer activity. Better: a comment within 72 hours, even if
  it's "I'll get to this Monday."
- Merge the PR with the contributor's commits; do not squash unless
  the repo policy is squash-only.
- Tag the contributor in the next release notes.
- Add them to a `CONTRIBUTORS.md` or use the all-contributors bot.

Small gestures. They compound.

## Funding and sustainability

This is where the xz-utils lesson lives.

The xz-utils maintainer was, by all public accounts, working on the
crate unpaid, alone, and exhausted. The attacker spent 2+ years
building reputation by sending "helpful" PRs, and the maintainer —
worn out and grateful for help — handed over commit access. The
backdoor followed.

You are not immune to this. The defenses are structural, not
willpower-based.

### Defense 1: A second maintainer from day 1

Listed earlier in the pre-flight checklist. Repeat: **no solo
public crates**. The second maintainer doesn't have to be
hyperactive — they just have to be a second pair of eyes on
merges. If you can't find a second person, your project is not
ready to be public.

### Defense 2: Funding, even if it's small

Set up GitHub Sponsors (or Open Collective, or Tidelift). The
*amount* matters less than the *presence*: a project that has
sponsors signals that its maintenance is a serious activity. Even
$50/month from a few users:

- Reframes the maintainer's relationship from "free labor" to
  "compensated, even if modestly, work."
- Gives the maintainer the moral standing to say "no" to entitled
  requests.
- Surfaces which companies *value* the crate enough to pay; those
  are your best leads for corporate sponsorship later.

Don't expect funding to make you whole. *Most* funded OSS projects
collect substantially less than the maintainer's hourly rate. The
point is signal and standing, not income.

### Defense 3: Disclose your time budget

A `MAINTAINERS.md` in the repo with:

```
We maintain this crate on best-effort. Allocated time:
- Maintainer A: 4 hours/week, Mondays
- Maintainer B: 2 hours/week, Wednesdays

We aim to:
- Respond to issues within 7 days.
- Review PRs within 14 days.
- Cut releases monthly (or when there's a security fix).

We do NOT:
- Provide email support.
- Provide enterprise SLAs without a paid relationship.
- Triage issues that lack a reproduction.
```

This is a contract. It manages expectations on both sides. It also
gives you an artifact to point at when an entitled user demands
more.

### Defense 4: Succession planning

If the project survives 2+ years, you will eventually want to step
away. Plan for it before it's urgent.

- **Identify a successor early.** Someone who's been active in
  the project, whose code you trust, whose values align.
- **Hand over in stages.** Give them commit access months before
  you give them release access; give them release access months
  before you hand over the keys to the GitHub org.
- **Document the handover** in a public issue or `MAINTAINERS.md`
  diff so the community sees the transition is intentional.
- **Stay reachable for 6 months** as an advisor, then step back.

The wrong pattern: the maintainer disappears, an unfamiliar
contributor offers help, the maintainer (relieved) hands over
everything. *This is the xz-utils pattern.* Refuse it.

### Defense 5: Verify identities for new maintainers

If you do bring in a new maintainer:

- Have at least one video call with them. (Yes, this is friction.
  It's also the simplest social-engineering filter we have.)
- Look at their public contribution history beyond your project.
- If their identity is opaque (pseudonym only, no other public
  work), treat their commit access as `review-required` for at
  least 6 months — don't give them merge rights yet.

Reputation accrues slowly. Trust extension should too.

## The maintainer's bill of rights

Adapted from Charity Majors' framing of operational engineering as
also-a-job-with-boundaries:

1. **You can say no.** No to features, no to deadlines, no to support
   requests, no to companies who want enterprise help without
   paying. "No" is a complete sentence and a load-bearing tool.

2. **You can say "not now."** The issue queue is not a to-do list.
   You triage on your schedule, not on the demand of whoever shouted
   loudest.

3. **You can take time off.** A 2-week silence on a public repo is
   not abandonment. It's a Tuesday. Set an away-mode in the
   `MAINTAINERS.md`; expect people to read it.

4. **You can deprecate features.** Anyone telling you "but I depend
   on this!" is welcome to fork or pay you to maintain it.

5. **You can decline a contribution.** "This is good work, but it
   doesn't fit the direction of the project. Here's a fork-friendly
   suggestion." is a perfectly kind no.

6. **You can charge.** If a company is making money on top of your
   work, you can ask them to sponsor you, hire you for consulting,
   or pay for an enterprise support contract. Asking is not
   greedy; it is professional.

7. **You can step down.** Find a successor (see Defense 4 above).
   Move on. The project is not your identity.

8. **You can be wrong.** A library design decision can be wrong; a
   release can be broken; a feature can be a mistake. Owning the
   error builds more credibility than defending the mistake.

9. **You can ignore drama.** Open source attracts personalities.
   You can mute, block, or close threads. This is not censorship;
   it is moderation, and it is part of the job.

10. **You can keep your boundaries when you're emotionally
    invested.** Burnout is real; loving the project doesn't immunize
    you. The bill of rights is not for the people who don't care.
    It's for the people who care too much.

If you find yourself violating one of these rights regularly,
something in the project's structure is broken and the fix is
structural, not individual willpower.

## Deprecation and the 18-month rule

Deprecation is the kindest user experience there is. The cruelest
is "we shipped 2.0 and removed the function you depended on; here's
the migration guide." The kindest is "we marked the function
deprecated in 1.5, kept it working for 18 months, and only removed
it in 2.0."

### The 18-month rule

If you decide a feature must go:

1. **Add `#[deprecated]` to the symbol in the next MINOR release.**
   Include a `note` pointing at the replacement.
2. **Update the CHANGELOG's `Deprecated` section.**
3. **Wait at least 18 months.** Or, if your release cadence is
   slow, "at least 3 minor releases" — whichever is longer.
4. **Then remove it in the next MAJOR release.**

Why 18 months? Because enterprise users upgrade their dependencies
on annual cycles; 18 months gives them at least one full upgrade
window with the deprecation warning visible.

Three things 18 months is not:

- It is not "wait 18 months from when we decided." It's "wait 18
  months from when the deprecation hit consumers."
- It is not "we will absolutely never remove this in 18 months."
  Security issues override the rule.
- It is not "the deprecation must be silent during the 18 months."
  Loud-but-not-error is the right volume: a `#[deprecated]`
  attribute, a CHANGELOG entry, and a one-paragraph blog post on
  the project site.

### When to deprecate

- A primitive turned out to be the wrong abstraction.
- A dependency is being removed (e.g. the crate it depended on
  is unmaintained).
- A feature was experimental and we promised "subject to change."
- A feature was used by ≤ 1% of downloads (according to whatever
  telemetry we have) for ≥ 6 months.

### When not to deprecate

- "I personally don't like this design any more." Personal taste
  changes; the API doesn't deserve to.
- "It's slightly nicer with the new pattern." Slightly nicer
  doesn't justify the user-side migration cost.
- "We want to clean up the API surface before 2.0." If the API
  works for users, the cleanup is your job, not theirs. Use
  `#[doc(hidden)]` or move the symbol; don't deprecate it.

### Deprecation hygiene

- Every deprecated symbol points at its replacement in its `note`.
- The replacement exists and works *before* the deprecation lands.
- The migration guide is published *at the same time* as the
  deprecation, not 6 months later.
- A `deprecations.md` page in the docs site lists every deprecation,
  when it landed, and when the removal is planned.

## Security disclosures

The minimum:

- `SECURITY.md` in the repo with:
  - Where to report vulnerabilities (a real email address that a
    real human checks).
  - The PGP key or Signal handle if you want encrypted reports.
  - Your acknowledgment SLA (e.g. "within 48 hours").
  - Your disclosure timeline (e.g. "we aim to patch within 30 days
    and publish an advisory; coordinated disclosure with the
    reporter is preferred").

- For Rust crates, register the crate with [RustSec](https://rustsec.org/)
  and learn how to publish advisories there. The
  `cargo audit` tool reads RustSec; your users' CI depends on it.

- If a vulnerability is reported privately:
  - Acknowledge receipt within the SLA.
  - Investigate, reproduce, and patch.
  - Coordinate disclosure with the reporter.
  - Publish a CHANGELOG entry, a RustSec advisory, and a release.
  - Yank the affected version *if* the patch cannot be applied
    cleanly downstream.

- If a vulnerability is reported publicly (e.g. someone opens a
  GitHub issue with the words "SQL injection" and a PoC):
  - Move the issue to private disclosure immediately. (GitHub's
    "Report a vulnerability" flow can do this.)
  - Apologize to the reporter for the friction; explain why we
    move it private.
  - Proceed as above.

### What not to do

- Do not silently patch a security issue and hope no one notices.
  The fix is in the public diff; an attacker reading the diff sees
  the bug. Patch *and* announce.
- Do not delay disclosure indefinitely waiting for a "perfect"
  patch. A documented mitigation plus a roadmap to the full fix is
  better than 90 days of silence.
- Do not retaliate against a security reporter who follows your
  disclosure policy. They are your friends, even when their report
  is annoying.

---

## A final note on motivation

Maintaining a popular open-source crate is one of the highest-leverage
things an engineer can do. It is also one of the most exhausting,
unpaid, and emotionally fraught. Most of the discipline in this
playbook is not for the high points; it's for the days you don't
want to triage and the requests that feel ungrateful.

The two things that protect a maintainer over years, more than any
process: a co-maintainer who pulls equal weight, and a workplace
that recognizes the OSS work as legitimate engineering output. If
you are an L7 sponsoring an L5's OSS work, your job is to make
sure both of those are in place. Without them, the playbook is
just paper.

See also:

- [`README.md`](./README.md) — directory framing
- [`01-vision-doc-template.md`](./01-vision-doc-template.md) — Bet
  3 in the worked example proposes publishing `mc-auth`; this
  playbook is the operational counterpart to that bet
- [`../../CHANGELOG.md`](../../CHANGELOG.md) — the format we
  actually use in this repo
- [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md) — the conventional
  commits policy and contributor on-ramp
