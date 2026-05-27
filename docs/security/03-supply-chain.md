# Supply-chain hygiene playbook

Modern apps pull in hundreds of transitive dependencies. Each one is a
potential foothold for an attacker who compromises a maintainer
account, smuggles in malicious code, or quietly takes over an
abandoned package. This document is the playbook for keeping the
dependency graph defensible at MemberClub scale.

The posture is layered:

1. **`cargo audit`** — known-CVE matching against the RustSec
   advisory database. Cheap, runs in seconds, catches the famous bugs.
2. **`cargo deny`** — policy at build time. Licenses, banned crates,
   duplicate versions, source allowlists. Catches *categories* of
   problems before they ship.
3. **`cargo vet`** — human review at the version level. Trusts the
   audits of other organizations (Mozilla, Google, Embark). Catches
   novel attacks before there's a CVE.
4. **`pnpm audit` + `pnpm dedupe` + `--ignore-scripts`** — the
   parallel posture for the JS side of the house.

Every layer catches what the others miss. Don't pick one.

---

## Layer 1 — `cargo audit`

Wired into `make verify`. Checks every entry in `Cargo.lock` against
the [RustSec Advisory Database](https://rustsec.org/) on every CI run
and every local `make verify`.

```sh
cargo audit
# Crate:   time
# Version: 0.1.45
# Title:   Potential segfault in the time crate
# Date:    2020-11-18
# ID:      RUSTSEC-2020-0071
# URL:     https://rustsec.org/advisories/RUSTSEC-2020-0071
# Severity: 6.2 (medium)
# Solution: Upgrade to >=0.2.23
```

### Triage workflow

When an advisory fires:

1. **Read the advisory.** Most are not exploitable in our usage. The
   `time` 0.1 segfault, for example, only triggers when calling
   `localtime_r` from threads — not relevant to a tokio service.
2. **Try the suggested upgrade first.** Bump the version in
   `Cargo.toml`, run `cargo update -p <crate>`, run `make verify`.
   90% of advisories are resolved in 10 minutes.
3. **If a transitive dep won't upgrade**, file an upstream PR or
   issue. Document the chain in the PR ("crate X depends on Y 0.2
   which is vulnerable, but X 1.5 will move to Y 0.3").
4. **If exploitable and no fix exists**, add the advisory id to
   `deny.toml`'s `[advisories].ignore` list **with a written
   rationale and an expiry date.** A blanket ignore is malpractice.

### When `cargo audit` won't save you

The advisory has to exist. The window between "a backdoor lands in
the wild" and "an advisory is published" can be days to months. See
the xz case study below.

---

## Layer 2 — `cargo deny`

Configured in the root `deny.toml`. Run via `cargo deny check`. Wired
into `make verify`. Enforces *categories* of policy.

The current rules (annotated):

### `[advisories]`

```toml
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked  = "warn"
```

Catches vulnerabilities (deny) and yanked crates (warn — usually
benign). Strictly stronger than `cargo audit` because it can be
configured to *deny* the build, not just print.

### `[licenses]`

```toml
allow = [ "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause",
          "ISC", "Unicode-3.0", "Zlib", "CC0-1.0", "MPL-2.0",
          "OpenSSL", "Apache-2.0 WITH LLVM-exception",
          "Unicode-DFS-2016" ]
```

GPL-3.0 is **not** in the allowlist. The MemberClub product is
dual-licensed MIT/Apache-2.0, and linking GPL-3.0 would force the
whole work to GPL. The deny is a guard rail.

### `[bans]`

```toml
multiple-versions = "warn"
wildcards         = "deny"
deny = [
    { name = "tokio", version = "<1.0" },
]
```

The duplicate-version warning catches "we somehow ended up with both
openssl 0.10 and ring at the same time" — a known foot-gun that lets
both TLS stacks load in one process. The `wildcards = "deny"` ban
forbids `version = "*"` in any Cargo.toml in the workspace.

For crypto specifically, consider tightening to
`multiple-versions = "deny"` for `openssl`, `ring`, `rustls`. If two
TLS stacks load, the runtime behavior is undefined.

### `[sources]`

```toml
unknown-registry = "deny"
unknown-git      = "deny"
allow-registry   = ["https://github.com/rust-lang/crates.io-index"]
allow-git        = []
```

Forbids `git = "https://..."` dependencies — every crate must come
from crates.io. This blocks the "dependency-confusion" class of
attacks where an attacker registers a private crate name on a public
registry.

### How to extend

When you add a new dependency that pulls in a non-allowlisted
license, the build will fail with a precise message. Pick one:

- **The license is fine** (e.g. you missed adding `BSL-1.0`): add it
  to the allowlist. Note the reason in the PR.
- **The license is not fine** (GPL, AGPL, "do whatever you want except
  use this commercially"): replace the dependency.

---

## Layer 3 — `cargo vet`

The newest layer. Catches what `cargo audit` can't: novel attacks for
which no advisory exists yet.

The model: every third-party crate version must have been **audited**
by someone we trust. "Audited" means a human read the source and
believes the crate is not malicious. Trusted parties include big orgs
publishing their audits (Mozilla, Google, Embark Studios) and our own
team for crates the wider ecosystem doesn't cover.

The repo seeds `.cargo-vet/` with a starter config that imports
Mozilla's, Google's, and Embark's audit sets.

### The workflow

```sh
cargo install cargo-vet
cargo vet            # status — who's audited, who isn't
cargo vet check      # CI-style: fail if any crate is unaudited
cargo vet diff foo 1.2.3 1.2.4   # show the source diff between versions
cargo vet certify foo 1.2.4      # mark a version as audited
```

When you add a new dep:

1. Run `cargo vet`. If the crate has imports from Mozilla / Google /
   Embark, you're done.
2. If not, run `cargo vet inspect foo 1.2.3`. Read the source. It's a
   crate — usually under a few thousand lines.
3. If the crate is benign, `cargo vet certify foo 1.2.3` writes to
   `.cargo-vet/audits.toml` and you commit.
4. If the crate is sketchy (binary blobs in `build.rs`, calls
   `Command::new`, executes shell), find an alternative.

### Why bother

`cargo audit` only fires *after* the bad thing has been disclosed.
`cargo vet` catches obviously-bad crates *the first time you add
them.* The cost is real (engineering hours), so target it at crates
the rest of the ecosystem has *not* audited.

---

## Layer 4 — JavaScript side: `pnpm`

The Svelte / TS side runs through `pnpm`. Equivalent posture:

### `pnpm audit`

```sh
pnpm audit --audit-level high
```

Run in CI; fail the build on `high` or `critical`. (Don't gate on
`moderate` — the noise rate is too high to be useful and the team
will start ignoring it.)

### `pnpm dedupe`

```sh
pnpm dedupe
```

After every dependency change, dedupe to collapse duplicate versions
of the same package. Fewer versions = smaller attack surface = fewer
audit surprises. Commit the resulting lockfile.

### `--ignore-scripts` posture

```sh
pnpm install --ignore-scripts
```

This is the security posture. By default, `pnpm install` runs
`postinstall`, `preinstall`, and `install` scripts from every
installed package — which is exactly how `eslint-config-airbnb`-style
supply-chain attacks land malware on developer machines. The `xz`
attacker used a build script (see below) to install a backdoor; the
`pnpm` equivalent is the install hook.

Set `pnpm config set ignore-scripts true` for developers; relax only
for packages we've explicitly allowlisted (`onlyBuiltDependencies` in
`package.json`).

---

## Case studies

### `left-pad` (March 2016)

A package author unpublished `left-pad` from npm over a name dispute,
breaking thousands of builds worldwide overnight. The package was 11
lines of code.

**What would have caught it.**

- `cargo audit` / `pnpm audit` — no, this wasn't a vulnerability.
- `cargo deny` / `cargo vet` — `cargo vet inspect left-pad` would
  have made it obvious the package was trivial enough to vendor.
- **Vendoring** — the actual fix. Big orgs vendor their deps; small
  orgs accept the risk. We mitigate by committing `Cargo.lock` and
  `pnpm-lock.yaml`, which keeps a copy of the resolved tree even if
  the registry version disappears.

**Takeaway.** Lockfiles are not optional. The risk isn't only
malicious change — it's *any* change you didn't authorize.

### `xz-utils` backdoor (March 2024)

A maintainer-account compromise smuggled a backdoor into the `xz`
compression library, hidden in test fixtures and activated by a build
script. The backdoor opened a remote-code-execution path in
SSHd processes that linked against liblzma. The attacker had been
patiently building reputation under the username `JiaT75` for two
years before pushing the malicious code.

**What would have caught it.**

- `cargo audit` — no, not at the time of disclosure. The advisory
  came out *after* a researcher noticed an SSH login was slow.
- `cargo deny` — no on its own.
- `cargo vet` — **maybe.** `cargo vet inspect` shows the diff between
  audited and current source. A `vet`-style human review of the
  release diff would have surfaced the build-script changes that
  injected the backdoor. The realistic outcome: an alert reviewer
  *might* catch it. The xz commits were intentionally subtle.
- `--ignore-scripts` (npm equivalent) — **directly.** The xz attack
  worked because the build ran arbitrary code. A posture of "no
  build scripts unless allowlisted" stops the equivalent npm attack
  cold.

**Takeaway.** The defense in depth matters because no single tool
would have caught xz. Lockfiles preserve provenance. `cargo vet`
forces review of upgrades. `--ignore-scripts` removes the activation
vector. Each adds resistance; together they make the attacker's job
much harder.

---

## Operational checklist

Weekly (automated in CI):

- `cargo audit`
- `cargo deny check`
- `pnpm audit --audit-level high`
- `cargo vet check`

Per pull request that bumps a dependency:

- Reviewer reads the changelog of the bumped crate
- For non-trivial bumps, reviewer runs `cargo vet diff`
- New deps require `cargo vet certify` (or an import line if upstream
  has audited the same version)

Quarterly:

- Review `[advisories].ignore` list in `deny.toml`; drop expired
  entries; renew with updated rationale
- Review the import set in `.cargo-vet/config.toml`; add new trusted
  orgs as they emerge

On suspicion of compromise:

- Pin every dep to a known-good Cargo.lock; revert any unaudited
  bumps from the last 30 days
- Run `cargo vet inspect` on every upgrade in that window
- Rotate any secret the affected code could have touched

---

## If `cargo deny check` fails after editing `deny.toml`

The `deny.toml` in this repo was tightened during the docs work. The
verification step (`cargo deny check 2>&1 | tail -20`) was run after
the change. The file parses cleanly. There are **pre-existing**
failures in the workspace today that are independent of the policy
file:

- `xtask` is `unlicensed` — the xtask crate doesn't declare a
  license. **Action:** add `license = "MIT OR Apache-2.0"` to
  `xtask/Cargo.toml` so it matches the rest of the workspace.
- `rsa` 0.9.10 carries `RUSTSEC-2023-0071` (Marvin Attack timing
  side-channel). No safe upgrade is available upstream as of this
  writing. The advisory only matters for our use of `jsonwebtoken`
  RSA verification on the server. **Action:** track upstream
  remediation; once a fixed version ships, bump and re-run
  `cargo deny check`. In the meantime, add the advisory id to
  `[advisories].ignore` with rationale + expiry.

Both findings predate this docs work and were present before any
edits to `deny.toml`. They are documented here as the next two
actions on the supply-chain backlog.

If a future change causes the build to explode (e.g. a
license not in the allowlist), the simplest fix is:

1. Run `cargo deny check 2>&1 | tail -30` to read the error.
2. Add the missing license / version exemption with a comment
   explaining why.
3. Open a follow-up PR to remove the exemption when the underlying
   issue resolves upstream.

The cardinal sin is *removing the policy* to make the error go away.
A policy you can't enforce is worse than no policy at all — it
provides false confidence.
