#!/usr/bin/env bash
# scripts/bootstrap.sh — one-shot setup for a fresh dev machine.
#
# Installs:
#   - rustup + 1.95 toolchain + components (clippy, rustfmt, rust-analyzer, rust-src)
#   - cargo-nextest, sqlx-cli, cargo-deny, cargo-audit, cargo-watch, just
#   - the two Rust MCP servers (rust-analyzer-mcp, rust-docs-mcp) used by .claude/settings.json
#   - mise (for node) + node 22 + pnpm via corepack
#
# It does NOT install Docker, gh, or the Stripe CLI — those are platform-specific.
# See curriculum/phase-00-foundations/lessons/02-toolchain.md for those.
#
# Run from anywhere:
#   bash scripts/bootstrap.sh
#
# Re-runnable: every step is a no-op if already done.

set -euo pipefail

step() { printf "\n\033[1;34m→\033[0m %s\n" "$*"; }
ok()   { printf "  \033[1;32m✓\033[0m %s\n" "$*"; }
warn() { printf "  \033[1;33m!\033[0m %s\n" "$*"; }
err()  { printf "  \033[1;31m✗\033[0m %s\n" "$*"; exit 1; }

# ----------------------------------------------------------------------------
# Rust toolchain
# ----------------------------------------------------------------------------

step "Rust toolchain"
if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # shellcheck source=/dev/null
    . "$HOME/.cargo/env"
    ok "rustup installed"
else
    ok "rustup already installed"
fi

rustup install stable --no-self-update >/dev/null 2>&1 || true
rustup default stable >/dev/null
rustup component add rustfmt clippy rust-analyzer rust-src >/dev/null
ok "stable + rustfmt + clippy + rust-analyzer + rust-src"

# ----------------------------------------------------------------------------
# Cargo extras
# ----------------------------------------------------------------------------

step "Cargo extras"
install_cargo() {
    local name="$1"
    shift
    if cargo --list 2>/dev/null | grep -q " $name "; then
        ok "$name already installed"
    else
        cargo install --locked "$name" "$@" >/dev/null 2>&1 || warn "$name install failed (continuing)"
        ok "$name installed"
    fi
}

install_cargo cargo-nextest
install_cargo cargo-deny
install_cargo cargo-audit
install_cargo cargo-watch
install_cargo just

step "sqlx-cli (slow — compiles from source)"
if command -v sqlx >/dev/null 2>&1; then
    ok "sqlx-cli already installed"
else
    cargo install --locked sqlx-cli@0.8.6 --no-default-features --features postgres,sqlite,rustls >/dev/null 2>&1 \
        || warn "sqlx-cli install failed (continuing)"
    ok "sqlx-cli installed"
fi

# ----------------------------------------------------------------------------
# Rust MCP servers (used by .claude/settings.json)
# ----------------------------------------------------------------------------

step "Rust MCP servers (used by Claude Code)"
install_cargo rust-analyzer-mcp
install_cargo rust-docs-mcp

# ----------------------------------------------------------------------------
# Node + pnpm
# ----------------------------------------------------------------------------

step "Node 22 + pnpm"
if ! command -v node >/dev/null 2>&1; then
    if ! command -v mise >/dev/null 2>&1; then
        curl https://mise.run | sh
        # shellcheck source=/dev/null
        . "$HOME/.local/share/mise/env"
    fi
    mise use --global node@22
    ok "node@22 via mise"
else
    ok "node already present ($(node --version))"
fi

if ! command -v pnpm >/dev/null 2>&1; then
    corepack enable
    corepack prepare pnpm@latest --activate
    ok "pnpm enabled via corepack"
else
    ok "pnpm already present ($(pnpm --version))"
fi

# ----------------------------------------------------------------------------
# Final sanity
# ----------------------------------------------------------------------------

step "Final sanity"
printf "  %-12s %s\n" "rustc"   "$(rustc --version 2>/dev/null || echo MISSING)"
printf "  %-12s %s\n" "cargo"   "$(cargo --version 2>/dev/null || echo MISSING)"
printf "  %-12s %s\n" "rustfmt" "$(rustfmt --version 2>/dev/null || echo MISSING)"
printf "  %-12s %s\n" "clippy"  "$(cargo clippy --version 2>/dev/null || echo MISSING)"
printf "  %-12s %s\n" "sqlx"    "$(sqlx --version 2>/dev/null || echo MISSING)"
printf "  %-12s %s\n" "nextest" "$(cargo nextest --version 2>/dev/null || echo MISSING)"
printf "  %-12s %s\n" "node"    "$(node --version 2>/dev/null || echo MISSING)"
printf "  %-12s %s\n" "pnpm"    "$(pnpm --version 2>/dev/null || echo MISSING)"

step "Next steps"
cat <<'EOF'
  1. Install Docker, gh, and Stripe CLI per
     curriculum/phase-00-foundations/lessons/02-toolchain.md
  2. cd into the repo, then: make verify
  3. Open the curriculum: curriculum/phase-00-foundations/README.md
EOF
