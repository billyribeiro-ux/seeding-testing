# Curriculum-wide Make targets.
# Every phase ends with `make verify` passing.

SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help verify fmt fmt-check clippy test build clean up down logs seed migrate web-dev web-build web-test ci

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z0-9_.-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# --- Quality gate (every phase must keep this green) ---

verify: fmt-check clippy test ## Run the full quality gate (matches CI)
	@echo "verify: OK"

fmt: ## Format the workspace
	cargo fmt --all

fmt-check: ## Check formatting without writing
	cargo fmt --all -- --check

clippy: ## Lint (treat warnings as errors)
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test: ## Run all Rust tests (uses nextest if available, falls back to cargo test)
	@if command -v cargo-nextest >/dev/null 2>&1; then \
		cargo nextest run --workspace --all-features; \
	else \
		echo "(cargo-nextest not installed; falling back to cargo test — install with: cargo install --locked cargo-nextest)"; \
		cargo test --workspace --all-features; \
	fi

build: ## Release build
	cargo build --workspace --release

clean: ## Remove build artifacts
	cargo clean
	rm -rf node_modules .svelte-kit build target

# --- Docker compose (Postgres + Redis + MailHog) ---

up: ## Start dev services (compose.yaml)
	docker compose up -d
	@docker compose ps

down: ## Stop dev services (keep volumes)
	docker compose down

down-clean: ## Stop dev services AND delete volumes (destructive)
	docker compose down -v

logs: ## Tail compose logs
	docker compose logs -f

# --- Database (sqlx-cli) ---

migrate: ## Run pending migrations against $$DATABASE_URL
	sqlx migrate run

seed: ## Seed dev fixtures (placeholder until Phase 5)
	@echo "seed: placeholder — added in curriculum/phase-05-testing-seeding/"

# --- SvelteKit (added at Phase 9) ---

web-dev: ## Start the SvelteKit dev server (placeholder until Phase 9)
	@echo "web-dev: placeholder — added in curriculum/phase-09-svelte-sveltekit/"

web-build:
	@echo "web-build: placeholder — added in curriculum/phase-09-svelte-sveltekit/"

web-test:
	@echo "web-test: placeholder — added in curriculum/phase-09-svelte-sveltekit/"

# --- CI alias (what the GHA workflow runs) ---

ci: verify ## Alias used by the GitHub Actions workflow
