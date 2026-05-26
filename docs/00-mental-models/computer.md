# Mental Model: What a Computer Is, For Programmers

> *Every program is a sequence of CPU instructions that read and write
> memory and occasionally ask the operating system for help. Every
> program. The fancy framework, the AI model, the IDE — all of them.*

This essay exists because the absolute-beginner on Phase 0 needs the
mental scaffolding before any code makes sense. Skim it once; come back
when something feels mysterious.

## The four boxes

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Editor    │ →  │  Language   │ →  │   Runtime   │ →  │  Operating  │
│  (VS Code)  │    │   (Rust)    │    │  (binary)   │    │   System    │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
```

- **Editor.** Where text lives while you're writing it. The editor is a
  notepad with autocomplete; it does not run your code.
- **Language.** The rules for what counts as a valid program. The
  *compiler* (`rustc`) reads your text, complains when it doesn't
  follow the rules, and produces a *binary*.
- **Runtime.** The binary your OS actually runs — bytes a CPU
  understands.
- **Operating system.** The manager that owns the hardware and lets
  your binary "borrow" pieces of it (a disk, a network socket, some
  memory).

Errors *always* belong to one box. Knowing which box owns a symptom is
half the debugging.

## The CPU + memory model

A program is a sequence of *instructions*. The CPU pulls instructions
one at a time and either:

1. Does math on some registers.
2. Reads or writes a byte of memory.
3. Branches to a different instruction.
4. Asks the OS for help via a *syscall*.

That's it. Every other concept — "object," "thread," "string" — is
built from those four primitives.

Memory is a giant array of bytes. Some bytes are *the program itself*
(read-only). Some are *the stack* (function locals; grows down).
Some are *the heap* (long-lived allocations; grows up). The OS gives
each process its own view of memory so processes can't see each other.

## The stack and the heap

Two regions of memory with different lifetimes:

- **Stack** — automatic. When a function returns, its locals are gone.
  Fast (just bump a pointer), bounded (~MB per thread).
- **Heap** — manual or GC. You ask the OS for N bytes; you eventually
  return them. Slower (allocator overhead), unbounded (limited by RAM).

In Rust, `String`, `Vec<T>`, `Box<T>` are on the heap. `i32`, `bool`,
`(i32, bool)` are on the stack. The compiler picks; you don't think
about it directly. But knowing which is which explains a lot of "why
does it work this way?"

## Syscalls — asking the OS for help

The CPU can't open a file or read from a socket on its own. It has to
ask the kernel. Each ask is a *syscall*: a function call to the
operating system.

```
your_program → CPU → syscall(open, "/etc/hosts") → kernel reads disk → returns a file descriptor → your_program continues
```

Syscalls are *slow* compared to CPU work (microseconds vs nanoseconds).
The number of syscalls per second is often what limits a program's
throughput. Most performance work boils down to "do fewer syscalls."

## Processes, threads, and tasks

- **Process.** A running program with its own memory view, file
  descriptors, scheduling priority. Heavyweight; takes milliseconds to
  start.
- **Thread.** A second sequence of instructions inside the same process.
  Cheaper than a process but still ~MB of stack. The OS scheduler
  decides which thread runs on which CPU core.
- **Task** (in async Rust). A small state machine inside one or more
  threads, scheduled by Tokio. ~hundreds of bytes; you can have
  millions.

Phase 2 of the curriculum unpacks tasks in detail. For now: a process
is what you launch from the shell, threads are what `tokio::spawn`
ultimately runs on, and tasks are the *language-level* unit of
concurrency.

## I/O is the slow part

Three orders of magnitude:

| Operation | Realistic time |
|---|---|
| Add two integers | ~0.3 ns |
| Read from CPU cache | ~1 ns |
| Read from main memory | ~100 ns |
| Read from local SSD | ~50 µs (50 000 ns) |
| Read from local network | ~500 µs |
| Read from cross-region network | ~50 ms |

Disk is 500× slower than memory. Cross-region network is 500× slower
than local. Most "this program is slow" stories trace to a syscall
doing I/O the program could have avoided.

## The shell

A shell (bash, zsh, fish) is just a program that reads your typed
commands and runs other programs:

```
You type:        cargo build
The shell:       finds /home/you/.cargo/bin/cargo on the PATH
                 forks itself + execs that binary, handing it the args
The cargo binary: runs; prints output to stdout/stderr
When it exits:   the shell gets the exit code, prints the next prompt
```

PATH is a colon-separated list of directories the shell searches for
executables. When you install something new, it puts a file in one of
those directories. When you can't run something you just installed, the
PATH is the first place to look.

## Files and the filesystem

Everything is a path. `/etc/hosts`, `~/.config/rustup`, `./README.md`.
Three concepts:

- **Absolute path** — starts with `/`. Unambiguous.
- **Relative path** — relative to the current working directory.
- **Tilde (`~`)** — the shell expands it to your home directory.

When you write a program that reads a file, give it an absolute path or
make the directory explicit. "Why can't my program find the file?" is
almost always "the working directory isn't what you thought."

## Why this matters

- **Knowing what a binary is** demystifies "I built it; how do I run
  it?".
- **Knowing about syscalls** demystifies why I/O is slow and why async
  exists.
- **Knowing the PATH** demystifies "command not found."

The four-box model is the diagnostic flowchart you'll mentally invoke
hundreds of times.

## Green-bar checkpoint

- You can name the four boxes and what each does.
- You can articulate why disk I/O is ~500× slower than memory.
- You can debug "command not found" by checking the PATH.

## Related

- Phase 0 lesson 0.1 (Mental Model)
- Phase 0 lesson 0.2 (Toolchain)
- `docs/00-mental-models/async.md` — why we don't block threads on I/O
