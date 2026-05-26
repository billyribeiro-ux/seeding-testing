# Phase 0 — Rubric

How a Principal Engineer would grade your Phase 0 work.

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Toolchain hygiene** | Rust installed somehow, paths unclear | All tools installed via standard installers, versions match curriculum targets | `rust-toolchain.toml` committed, versions pinned, can explain why pinning matters |
| **Terminal fluency** | Copies commands without reading | Can navigate, edit, and inspect files without leaving the terminal | Writes one-liner pipelines (`gh run list \| jq ...`), has personal aliases |
| **Git mental model** | Can `add` / `commit` / `push` if told the exact commands | Confidently branches, stashes, rebases, resolves easy conflicts | Has used `git reflog` to recover from a mistake. Never `--force` pushes to shared branches |
| **Commit hygiene** | Messages like "stuff" / "fix" | Conventional Commits, one logical change per commit | Atomic commits with body text explaining *why*, links to issue/ADR when relevant |
| **CI awareness** | "It pushed, I guess it's done?" | Watches `gh run watch`, fixes red runs | Reads CI logs deeply; understands which jobs gate `main`; can explain *why* each job exists |
| **Error reading** | Skims, googles immediately | Reads the message, identifies which "box" it lives in | Reproduces locally, narrows to minimal example, only then escalates |
| **Curiosity** | Follows the lesson | Tries the bonus exercises | Reads the docs of every tool they install before using it |

## Self-check before moving to Phase 1

- [ ] All nine verification commands in `lessons/02-toolchain.md` print versions.
- [ ] You have pushed at least one commit; `gh run list --limit 5` shows your runs.
- [ ] You can explain `git add` vs `git commit` vs `git push` to a non-programmer.
- [ ] You completed Exercises E0.1 – E0.4. (E0.5 is optional but transformative.)
- [ ] You have skimmed `PLAYBOOK.md` and `TROUBLESHOOTING.md` so you know they exist.

If any box is empty, stay in Phase 0 another day. Phase 1 ramps up fast.
