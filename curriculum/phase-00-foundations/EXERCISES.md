# Phase 0 — Exercises

Five graded exercises. Hidden answers behind `<details>` tags — try first, peek second.

---

## E0.1 — Identify the box (Easy) — shipped

Pure mental-model drill. The deliverable is the learner classifying each error message; the worked answer (with the rationale for each box) is in the `<details>` block immediately below.

For each error message, name which box it belongs to (Editor / Language / Runtime / OS):

1. `error[E0308]: mismatched types`
2. `permission denied`
3. `rust-analyzer: indexing failed`
4. `address already in use`
5. `cannot find binary cargo in PATH`

<details><summary>Answer</summary>

1. Language (compiler).
2. OS (file permissions).
3. Editor (the LSP crashed; the program itself is fine).
4. OS (another process is using that port).
5. OS, sort of — really *shell* / PATH config. Not the language or the runtime.
</details>

---

## E0.2 — Path practice (Easy) — shipped

Shell drill — the seven-step ritual lives in the `<details>` answer below as a copy-pasteable bash script. The deliverable is the learner running each command in their own terminal.

Without using your file manager, in the terminal:

1. `cd` to your home directory.
2. Create a folder called `scratch`.
3. Inside `scratch`, create an empty file called `hello.txt`.
4. Add the line "first commit" to the file using `echo` and `>>`.
5. Print the file's contents with `cat`.
6. Delete the file with `rm`.
7. Delete the empty `scratch` folder with `rmdir`.

<details><summary>Answer</summary>

```bash
cd ~                         # 1
mkdir scratch                # 2
touch scratch/hello.txt      # 3
echo "first commit" >> scratch/hello.txt   # 4
cat scratch/hello.txt        # 5
rm scratch/hello.txt         # 6
rmdir scratch                # 7
```
</details>

---

## E0.3 — Conventional Commit (Easy) — shipped

Rewrite drill. Five worked conversions (type/scope/subject) are in the `<details>` answer below; learners compare their rewrites against the model answers.

Rewrite these informal commit messages in Conventional Commits form:

1. "fixed the login bug"
2. "added new homepage"
3. "wrote some tests"
4. "renamed variables for clarity"
5. "updated cargo deps"

<details><summary>Answer</summary>

1. `fix(auth): correct password verification on Argon2 v2 hashes`
2. `feat(web): add landing page`
3. `test(api): cover edge cases in user creation`
4. `refactor(money): rename Cents → MoneyCents for clarity`
5. `chore(deps): bump cargo dependencies`
</details>

---

## E0.4 — Recover a lost commit (Medium) — shipped

Hands-on git drill. Reproducer script is in the prompt, and the recovery recipe (`git reflog` + `git cherry-pick`) plus cleanup steps are in the `<details>` answer below — the deliverable is the learner running it on a throwaway `playground` branch.

Simulate the disaster everyone hits eventually:

```bash
cd ~/Projects/seeding-testing
git switch -c playground
echo "something important" > IMPORTANT.txt
git add IMPORTANT.txt
git commit -m "feat: add important note"
git reset --hard HEAD~1     # Whoops! Just lost the commit.
ls IMPORTANT.txt            # gone
```

Recover the commit using `git reflog` and `git cherry-pick`.

<details><summary>Answer</summary>

```bash
git reflog                          # find the SHA of "feat: add important note"
git cherry-pick <SHA>               # bring it back
ls IMPORTANT.txt                    # there it is
```

Then clean up:

```bash
git switch -                        # back to your previous branch
git branch -D playground            # delete the scratch branch
rm IMPORTANT.txt && git add -A && git commit -m "chore: clean up exercise"  # or amend
```

Lesson: **`git reflog` is your time machine even after `reset --hard`.**
</details>

---

## E0.5 — Watch your own CI failure (Stretch) — shipped

Experiential drill against this repo's CI. No code lands; the deliverable is the learner pushing a deliberate break, watching `gh run watch` go red, and fixing it. The `<details>` block below explains what the learner should *feel* during the loop.

Deliberately break something to *experience* a failing CI run:

1. In `README.md`, introduce a malformed table (e.g. delete a `|` so the markdown renders weirdly).
2. Commit and push.
3. `gh run watch` — confirm the run is green (markdown isn't checked in CI yet).
4. Now break Rust code: in the workspace root `Cargo.toml`, under `[workspace.dependencies]`, change `clap = { version = "4.6", features = ["derive"] }` to `clap = { version = "4.6", features = ["nope"] }`.
5. Commit and push. Watch CI go red. Click the failed job's URL and identify *which sub-step* failed.
6. Fix it. Push. Watch it go green.

<details><summary>Answer</summary>

There's no single "correct" answer here — the point is to feel:

- The reassuring `gh run watch` ticking through jobs.
- The exact, scary moment of red ❌.
- The relief of fixing it and seeing green.

If you can do this loop calmly, you have the disposition of a senior engineer.
</details>
