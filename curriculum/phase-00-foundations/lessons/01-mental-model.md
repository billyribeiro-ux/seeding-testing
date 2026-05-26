# Lesson 0.1 — Mental Model

> **15–20 minutes.** No keyboard yet. Read carefully, twice if you need to.

## A program is a recipe

Imagine a recipe card for a pancake. It has:

1. **A list of ingredients** (what data you'll need: flour, eggs, milk).
2. **A list of steps** (the order things happen: mix, pour, flip).
3. **An outcome** (pancakes on a plate, or — if you misread step 4 — a smoke alarm).

A program is exactly this. A *value* is an ingredient. A *function* is a step. The *output* is what the program returns or what shows up on screen.

The whole craft of programming is just: **describe the recipe precisely enough that a machine can follow it without you in the room.**

## The four boxes (revisited)

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Editor    │ →  │  Language   │ →  │   Runtime   │ →  │  Operating  │
│  (VS Code)  │    │   (Rust)    │    │  (binary)   │    │   System    │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
```

When you save a `.rs` file in VS Code, none of that text *runs* yet. It's just text on disk. To turn it into something the computer can execute:

1. **Compile.** You ask `cargo build`. Cargo calls `rustc`. `rustc` reads your file, checks every rule of Rust, and if everything passes it produces a *binary* — a file full of CPU instructions.
2. **Run.** You ask the operating system to execute that binary. The OS loads it into memory, gives it a process ID, and hands it the CPU.
3. **Interact.** The binary asks the OS for resources (read a file, open a socket, print to the terminal). The OS grants or refuses each request.

When something breaks, the error always lives in one box:

- "expected `i32`, found `String`" → **language box**. The compiler hated your text. Fix the recipe.
- "permission denied" → **OS box**. You asked for a file you don't own. Fix the permissions.
- "connection refused" → **OS box**, networking. The thing you tried to talk to isn't listening.
- "rust-analyzer not running" → **editor box**. The editor is fine but its smart helper crashed.

Knowing which box owns the symptom is the first move in every debugging session.

## What "the terminal" is

A *terminal* is just a window where you type commands at a *shell*. A shell is a program that reads your text, finds the right binary, and runs it.

```
You type:        cargo --version
Shell finds:     /home/you/.cargo/bin/cargo (binary on disk)
Shell tells OS:  "run /home/you/.cargo/bin/cargo with arg --version"
OS runs it.
Cargo prints:    cargo 1.95.0
```

Everything you'll do in this curriculum starts with one of three sentences:

- "Open a terminal."
- "Open VS Code."
- "Open the browser."

That's the whole user interface for a backend engineer.

## Files, paths, and the current directory

Your computer's storage is a tree. The root is `/` on Linux/macOS, `C:\` on Windows. Every file has an *absolute path* like `/home/you/Projects/seeding-testing/README.md`.

In a terminal, you are always *inside* a folder (the "current working directory" or `pwd`). Commands like `ls` and `cat` look at files relative to where you are. So:

```bash
cd ~/Projects/seeding-testing      # change directory
pwd                                 # print working directory
ls                                  # list files
cat README.md                       # show that file's contents
```

If a tutorial says "run `cargo build`", it almost always means: open a terminal, `cd` into the project folder, then type `cargo build` and press Enter.

## The single best habit

When in doubt, **read the error message out loud.** Compilers — Rust's especially — are written by people who care about the human on the other end. They will tell you:

- What rule you broke.
- Which file and line broke it.
- What they think you meant.

The slowest learners are the ones who skim error messages. The fastest read them like a letter from a friend.

## You're done with this lesson when

- You can explain the four boxes to an imaginary friend without looking.
- You can answer: "I see `permission denied` when I run my program. Which box is the error in?" (OS box.)
- You can answer: "I see `cannot find function foo` when I compile. Which box?" (Language box.)

Next: install the tools. Open `lessons/02-toolchain.md`.
