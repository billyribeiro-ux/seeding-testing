# Lesson 0.3 — Git, GitHub, and Your First Commit

> **45 minutes.** You will: clone this repo, make a tiny change, commit it, push it, and watch CI go green.

## The mental model for git

Git is a **time machine for files**. Three concepts cover 95% of daily use:

1. **Working tree** — the actual files on disk you can see and edit.
2. **Staging area (index)** — a clipboard where you queue changes you intend to commit.
3. **History** — an append-only chain of commits. Each commit is a snapshot + a message.

```
working tree   ──▶  git add ──▶   staging   ──▶  git commit ──▶   history
   (edits)                       (queued)                       (immutable snapshots)
```

Three rules:

- **A commit is forever** (well, almost — rewriting history is possible but costly). Make small, well-described commits.
- **Branches are cheap.** When you start work, make a branch. Merge back to `main` when done.
- **Push often.** Code that lives only on your laptop is one spilled coffee from gone.

## Clone the curriculum repo

If you haven't already:

```bash
cd ~/Projects                                         # or wherever you keep code
gh repo clone billyribeiro-ux/seeding-testing
cd seeding-testing
```

Check what branch you're on:

```bash
git status
git branch --show-current
```

The curriculum branch is `claude/rust-backend-curriculum-plan-LxNOP`. If you're not on it:

```bash
git fetch origin
git checkout claude/rust-backend-curriculum-plan-LxNOP
```

## Make your first change

Open `curriculum/phase-00-foundations/STUDENTS.md` (it doesn't exist yet — you'll create it):

```bash
echo "- Your Name <your.email@example.com> — joined $(date +%Y-%m-%d)" >> curriculum/phase-00-foundations/STUDENTS.md
```

Check what changed:

```bash
git status                                # which files moved
git diff curriculum/phase-00-foundations/STUDENTS.md   # what changed inside
```

## Stage, commit, push

```bash
# 1. Stage the file (queue it for the next commit)
git add curriculum/phase-00-foundations/STUDENTS.md

# 2. Verify what's staged
git status

# 3. Commit with a Conventional Commit message
git commit -m "docs(phase-00): add my name to STUDENTS.md"

# 4. Push to your branch on GitHub
git push
```

> **Conventional Commits** is a tiny convention: `<type>(<scope>): <subject>`. Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`. It pays off later: release notes are generated from these.

## Watch CI run

GitHub Actions runs on every push. Watch the run from your terminal:

```bash
gh run watch
```

You should see a list of jobs (`fmt`, `clippy`, `test`, `deny`, `audit`) tick through to green checkmarks.

If a job fails, click the URL `gh run watch` prints to see the logs, **or**:

```bash
gh run view --log-failed
```

When all green:

```bash
gh run list --limit 1
```

…shows the run with `completed/success`. Congratulations — you've shipped your first commit.

## The everyday loop

This is the loop you'll repeat hundreds of times in this curriculum:

```
┌────────────────────────────────────────────────────────────────────┐
│ 1. Pull the latest:    git pull --rebase                           │
│ 2. Create a branch:    git switch -c phase-NN-something            │
│ 3. Edit files in VS Code                                           │
│ 4. Save, then verify:  make verify                                 │
│ 5. Stage + commit:     git add -p && git commit -m "feat: ..."     │
│ 6. Push:               git push                                    │
│ 7. Watch CI:           gh run watch                                │
│ 8. Open PR (later):    gh pr create --fill --web                   │
└────────────────────────────────────────────────────────────────────┘
```

## Things you'll wish you knew sooner

- `git log --oneline --graph --all --decorate` is the prettiest log. Alias it: `git config --global alias.lg "log --oneline --graph --all --decorate"`.
- `git add -p` (patch mode) lets you stage *parts* of a file. Use it.
- `git commit --amend` rewrites your *most recent* commit. Use it only on commits you haven't pushed yet.
- `git stash` parks dirty changes you don't want to commit (e.g. you need to switch branches). `git stash pop` brings them back.
- When the worst happens: `git reflog` shows every position HEAD has ever been at. You can almost always recover.

## Branch protection (read-only awareness for Phase 0)

On real teams, `main` is *protected*: nobody can push to it directly. You must open a Pull Request, get review, and pass CI. We mirror that pattern by default in this repo. For now, you push to your branch (`claude/...`), CI runs there, and you'll learn to open PRs in later phases.

## Green-bar checkpoint

You're done with Phase 0 when:

- `git log -1` shows your commit message in the curriculum repo.
- `gh run list --limit 1` shows your CI run as `completed/success`.
- You can explain the working-tree → staging → history flow to an imaginary friend.

Next: Phase 1 — Rust Core. Open `curriculum/phase-01-rust-core/README.md`.
