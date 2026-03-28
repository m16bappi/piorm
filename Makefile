.DEFAULT_GOAL := help

# ── Variables ──────────────────────────────────────────────────────────────────
PY_SRC := python/ tests/

# ── Help ───────────────────────────────────────────────────────────────────────
.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
	  | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-18s\033[0m %s\n", $$1, $$2}'

# ── Setup ──────────────────────────────────────────────────────────────────────
.PHONY: install
install: ## Bootstrap: sync all deps + build Rust extension (first-time setup)
	uv sync --all-groups
	uv run maturin develop

.PHONY: dev
dev: ## Rebuild the Rust extension in dev mode (incremental)
	uv run maturin develop

.PHONY: dev-release
dev-release: ## Rebuild the Rust extension with optimisations
	uv run maturin develop --release

# ── Build ──────────────────────────────────────────────────────────────────────
.PHONY: build
build: ## Build a release wheel into target/wheels/
	uv run maturin build --release

# ── Tests ──────────────────────────────────────────────────────────────────────
.PHONY: test
test: ## Run the test suite
	uv run pytest

.PHONY: test-v
test-v: ## Run tests with verbose output
	uv run pytest -v

.PHONY: test-fast
test-fast: ## Run tests, stop on first failure
	uv run pytest -x

# ── Lint / Format ──────────────────────────────────────────────────────────────
.PHONY: lint
lint: ## Lint Python sources with ruff
	uv run ruff check $(PY_SRC)

.PHONY: fmt
fmt: ## Format Python sources with ruff
	uv run ruff format $(PY_SRC)

.PHONY: fmt-check
fmt-check: ## Check formatting without modifying files
	uv run ruff format --check $(PY_SRC)

.PHONY: typecheck
typecheck: ## Run mypy type checking
	uv run mypy python/

# ── Rust ───────────────────────────────────────────────────────────────────────
.PHONY: cargo-test
cargo-test: ## Run Rust unit tests
	cargo test

.PHONY: cargo-check
cargo-check: ## Run cargo check (fast Rust type-check, no codegen)
	cargo check

.PHONY: cargo-clippy
cargo-clippy: ## Run clippy linter on Rust code
	cargo clippy -- -D warnings

.PHONY: cargo-fmt
cargo-fmt: ## Format Rust code with rustfmt
	cargo fmt

.PHONY: cargo-fmt-check
cargo-fmt-check: ## Check Rust formatting without modifying files
	cargo fmt -- --check

# ── CI / Combined ──────────────────────────────────────────────────────────────
.PHONY: check
check: fmt-check lint typecheck cargo-fmt-check cargo-clippy ## Run all checks (non-destructive)

.PHONY: ci
ci: install check test cargo-test ## Full CI pipeline

# ── Clean ──────────────────────────────────────────────────────────────────────
.PHONY: clean
clean: ## Remove build artifacts and caches
	cargo clean
	rm -rf target/wheels dist
	find . -type d -name __pycache__ -not -path './.venv/*' | xargs rm -rf
	find . -name "*.pyc" -not -path './.venv/*' -delete
	find . -name ".mypy_cache" -type d | xargs rm -rf
	find . -name ".ruff_cache" -type d | xargs rm -rf
