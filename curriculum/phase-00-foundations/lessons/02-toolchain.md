# Lesson 0.2 — Install the Toolchain

> **60–90 minutes.** Yes, an hour of installing things before you write code. This is normal in software. Do it once, do it right.

We're installing nine pieces of software. They fall into four groups:

1. **Rust** (the language)
2. **Editor** (where you'll write code)
3. **Git + GitHub CLI** (version control)
4. **Docker, Node, Stripe CLI** (operational tooling)

Every command below is copy-pastable. Every command ends with a verification line you must see succeed before moving on.

---

## 1. Rust

### Linux / macOS / WSL

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

The script installs **rustup** (the version manager), which then pulls down **rustc** (the compiler) and **cargo** (the package manager).

### Windows (native, no WSL)

Download and run the installer from <https://rustup.rs>. Accept the defaults.

### Add the components we'll need

```bash
rustup component add rustfmt clippy rust-analyzer rust-src
```

What they do:
- **rustfmt** — auto-formats your code (`cargo fmt`).
- **clippy** — catches common mistakes (`cargo clippy`).
- **rust-analyzer** — the brain of your editor.
- **rust-src** — the Rust standard library source (so go-to-definition works).

### Verify

```bash
rustc   --version    # rustc 1.95.0 ... (or 1.94.x is fine for now)
cargo   --version    # cargo 1.95.0 ...
rustfmt --version
cargo clippy --version
```

If `rustc` is missing from your PATH after a fresh install, close and reopen the terminal — or run `source "$HOME/.cargo/env"`.

> **Troubleshooting** entries live in `/TROUBLESHOOTING.md` under "Toolchain & install" if the above doesn't work.

---

## 2. VS Code + rust-analyzer

1. Install VS Code: <https://code.visualstudio.com/>.
2. Open VS Code. Press `Ctrl+Shift+X` (or `Cmd+Shift+X` on macOS) to open Extensions.
3. Install:
   - **rust-analyzer** (the official one — author `rust-lang`).
   - **Even Better TOML** (for `Cargo.toml` syntax).
   - **CodeLLDB** (for debugging Rust).
   - **Error Lens** (shows errors inline as you type).

### Verify

Open this repo in VS Code (`File → Open Folder…`). Open any `.rs` file. You should see green checkmarks in the bottom bar within ~10 seconds (rust-analyzer indexing).

---

## 3. Git + GitHub CLI

### git

Almost certainly already installed. Check:

```bash
git --version    # git version 2.x
```

If missing:
- Debian/Ubuntu: `sudo apt update && sudo apt install -y git`
- macOS: `brew install git`
- Windows: <https://git-scm.com/download/win>

Configure once, globally:

```bash
git config --global user.name  "Your Name"
git config --global user.email "you@example.com"
git config --global init.defaultBranch main
git config --global pull.rebase true
git config --global push.autoSetupRemote true
```

### gh (GitHub CLI)

- Linux: <https://github.com/cli/cli/blob/trunk/docs/install_linux.md>
- macOS: `brew install gh`
- Windows: `winget install --id GitHub.cli`

Then authenticate:

```bash
gh auth login
# choose: GitHub.com → HTTPS → "Login with a web browser"
gh auth status      # should show "Logged in to github.com"
```

---

## 4. Docker, Node, pnpm, Stripe CLI

### Docker

- Linux: <https://docs.docker.com/engine/install/>. After install, add your user to the `docker` group: `sudo usermod -aG docker $USER && newgrp docker`.
- macOS / Windows: install **Docker Desktop**.

Verify:

```bash
docker --version              # Docker version 27.x or newer
docker compose version        # Docker Compose version v2.x
docker run --rm hello-world   # should print "Hello from Docker!"
```

### Node + pnpm

Pick **one** of these two approaches.

**Option A — `mise` (recommended; manages many runtimes):**

```bash
curl https://mise.run | sh
echo 'eval "$(mise activate bash)"' >> ~/.bashrc   # or your shell's rc
source ~/.bashrc
mise use --global node@22
corepack enable
corepack prepare pnpm@latest --activate
```

**Option B — `fnm` (lighter, Node only):**

```bash
curl -fsSL https://fnm.vercel.app/install | bash
source ~/.bashrc
fnm install 22 && fnm default 22
corepack enable
corepack prepare pnpm@latest --activate
```

Verify:

```bash
node --version       # v22.x
pnpm --version       # 10.x
```

### Stripe CLI

Used from Phase 8 onward. Install now so it's ready.

- Linux: download from <https://github.com/stripe/stripe-cli/releases/latest>, e.g.:
  ```bash
  curl -L https://github.com/stripe/stripe-cli/releases/latest/download/stripe_linux_x86_64.tar.gz \
    | tar xz -C ~/.local/bin
  ```
- macOS: `brew install stripe/stripe-cli/stripe`
- Windows: `scoop install stripe`

Verify:

```bash
stripe --version
```

(You don't have to log in until Phase 8.)

---

## 5. Cargo extras

```bash
cargo install --locked sqlx-cli@0.8.6 cargo-nextest cargo-watch cargo-deny cargo-audit just
```

What they do:
- **sqlx-cli** — database migrations (Phase 3).
- **cargo-nextest** — faster, prettier test runner.
- **cargo-watch** — re-runs `cargo` on file save (`cargo watch -x run`).
- **cargo-deny** — license + supply-chain checks.
- **cargo-audit** — security advisories.
- **just** — a friendly `make` alternative (we use `make` in this repo but `just` is handy).

> If `cargo install` is slow, that's normal — it's compiling these tools from source. Get coffee.

### Rust MCP servers (for Claude Code)

You asked for these in the plan; install both and they'll plug into Claude:

```bash
cargo install --locked rust-analyzer-mcp
cargo install --locked rust-docs-mcp
```

They are pre-registered in `.claude/settings.json`. Once installed, restart Claude Code and verify the MCP icon shows both servers connected.

---

## 6. Green-bar checkpoint

Paste this single block into your terminal. Every line must print a version, not "command not found".

```bash
rustc --version   && \
cargo --version   && \
rustfmt --version && \
cargo clippy --version && \
git --version     && \
gh --version | head -1 && \
docker --version  && \
docker compose version && \
node --version    && \
pnpm --version    && \
stripe --version  && \
sqlx --version    && \
cargo nextest --version && \
echo "ALL GOOD ✓"
```

When that block ends in `ALL GOOD ✓`, you're done. Next: `lessons/03-git-and-gh.md`.
