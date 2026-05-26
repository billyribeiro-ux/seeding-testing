# Phase 0 — Foundations

> **Audience:** someone who has never written a line of code.
> **Outcome:** by the end you can install the toolchain, navigate the terminal, write your first commit, and push it to GitHub.
> **Time:** 1–2 weeks at a relaxed pace.

## The mental model first

Before you touch a keyboard, look at the four boxes below and make them real in your head. Every confusion you'll meet in the next month comes from blurring them.

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Editor    │ →  │  Language   │ →  │   Runtime   │ →  │  Operating  │
│  (VS Code)  │    │   (Rust)    │    │  (binary)   │    │   System    │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
   you type        compiler turns        the OS runs       talks to
   words here →    your text into  →    that binary  →    disks, network,
                   a tiny program       in a process      memory, etc.
```

- **The editor** is where text lives while you're writing it. VS Code is just a fancy notepad.
- **The language** (Rust) is a set of rules for what counts as a valid program. The *compiler* (`rustc`) reads your text, complains when it doesn't match the rules, and on success produces a binary.
- **The runtime** is the binary your OS actually runs — bytes that the CPU understands.
- **The operating system** (Linux/macOS/Windows) is the manager that owns the hardware and lets your binary "borrow" pieces of it (a disk, a network socket, some memory).

Why this matters: when something breaks, the error message *always* belongs to one of these boxes. Knowing which box owns the error is half the fix.

## What you will install

| Tool | What it is | Why we need it |
|---|---|---|
| **rustup** | The Rust toolchain installer | Manages versions of `rustc`, `cargo`, `clippy`, `rustfmt` |
| **cargo** | Rust's package manager + build tool | Compiles your code, runs tests, fetches dependencies |
| **VS Code** | The editor | Has the best Rust support via rust-analyzer |
| **rust-analyzer** | Language server | "Smart" autocomplete and inline errors |
| **git** | Version control | Time-travel for your code |
| **gh** | GitHub CLI | Talk to GitHub without leaving the terminal |
| **Docker** | Container runtime | Run Postgres / Redis / etc. without installing them on your host |
| **Node.js + pnpm** | JavaScript runtime + package manager | We'll need them for the SvelteKit phases |
| **Stripe CLI** | Local Stripe webhook testing | You'll use this from Phase 8 onward |

All install commands are in `lessons/02-toolchain.md`.

## The phase plan

| Lesson | What you'll do | Time |
|---|---|---|
| `lessons/01-mental-model.md` | Read the four-box model out loud. Internalize it. | 20 min |
| `lessons/02-toolchain.md` | Install everything above. Verify each tool prints its version. | 60–90 min |
| `lessons/03-git-and-gh.md` | Make your first commit. Push to GitHub. Run `gh run watch` on CI. | 45 min |

## Green-bar checkpoint

You're done with Phase 0 when **all of these print without errors**:

```bash
rustc --version            # rustc 1.95.0 ... (or newer stable)
cargo --version            # cargo 1.95.0 ...
git --version              # git version 2.x
gh --version               # gh version 2.x
docker --version           # Docker version 27.x or newer
node --version             # v22.x
pnpm --version             # 10.x
```

…and you have pushed at least one commit to the curriculum branch and watched CI go green via `gh run watch`.

## What's next

Phase 1 — **Rust Core**. You'll write your first real program: a small command-line tool, with tests, that you'll actually want to use.
