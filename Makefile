# Context Graph - Build & Maintenance Makefile
#
# All targets auto-trim stale deps to prevent unbounded disk growth.
# The 11-crate workspace produces ~600MB binaries per build; without
# trimming, target/ grows to 240GB+ within a few weeks.
#
# Usage:
#   make build          Build release (MCP server + CLI)
#   make test           Run all workspace tests
#   make test-e2e       Run E2E hook tests only
#   make check          Quick workspace check (no codegen)
#   make clean          Remove target/debug entirely
#   make clean-all      Remove entire target/ directory
#   make disk-check     Report disk usage and cleanable space
#   make trim           Trim stale deps only (no build)

.PHONY: build test test-e2e check clean clean-all disk-check trim clippy

# --- Build ---

build:
	cargo build --release
	@./scripts/trim-stale-deps.sh release

build-debug:
	cargo build
	@./scripts/trim-stale-deps.sh debug

# --- Test ---

test:
	cargo test --workspace
	@./scripts/trim-stale-deps.sh debug

test-e2e:
	cargo test -p context-graph-cli --test e2e
	@./scripts/trim-stale-deps.sh debug

test-mcp:
	cargo test -p context-graph-mcp
	@./scripts/trim-stale-deps.sh debug

# --- Check & Lint ---

check:
	cargo check --workspace --all-targets

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# --- Cleanup ---

trim:
	@./scripts/trim-stale-deps.sh

clean:
	rm -rf target/debug
	@echo "Removed target/debug. Release binaries preserved."

clean-all:
	cargo clean
	@echo "Removed entire target/ directory."

clean-deep: clean-all
	./scripts/clean-build-artifacts.sh --aggressive

disk-check:
	@./scripts/clean-build-artifacts.sh --check
	@echo ""
	@./scripts/disk-guard.sh
