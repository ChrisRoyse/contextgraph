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

.PHONY: build build-rust build-python test test-rust test-python-smoke test-python test-e2e test-mcp check check-rust check-python lint lint-rust lint-python clean clean-all disk-check trim clippy release-candidate-archive

# --- Build ---

UV ?= uv
CLIPCANNON_DIR ?= clipcannon
PYTHON_DEV_EXTRA ?= --extra dev
PYTHON_SMOKE_TESTS ?= tests/test_music_planner.py tests/test_audio_generation.py tests/test_change_classifier.py tests/voiceagent/test_config.py tests/voiceagent/test_chunker.py

build: build-rust build-python

build-rust:
	cargo build --release --workspace
	@./scripts/trim-stale-deps.sh release

build-python:
	cd $(CLIPCANNON_DIR) && $(UV) sync --locked $(PYTHON_DEV_EXTRA)

build-debug:
	cargo build --workspace
	@./scripts/trim-stale-deps.sh debug

# --- Test ---

test: test-rust test-python-smoke

test-rust:
	cargo test --workspace
	@./scripts/trim-stale-deps.sh debug

test-python-smoke:
	cd $(CLIPCANNON_DIR) && $(UV) run --locked $(PYTHON_DEV_EXTRA) pytest $(PYTHON_SMOKE_TESTS)

test-python:
	cd $(CLIPCANNON_DIR) && $(UV) run --locked $(PYTHON_DEV_EXTRA) pytest tests/

test-e2e:
	cargo test -p context-graph-cli --test e2e
	@./scripts/trim-stale-deps.sh debug

test-mcp:
	cargo test -p context-graph-mcp
	@./scripts/trim-stale-deps.sh debug

# --- Check & Lint ---

check:
	$(MAKE) check-rust
	$(MAKE) check-python

check-rust:
	cargo check --workspace --all-targets

check-python:
	cd $(CLIPCANNON_DIR) && $(UV) lock --check
	cd $(CLIPCANNON_DIR) && $(UV) run --locked $(PYTHON_DEV_EXTRA) python -c "import clipcannon, phoenix, voiceagent"

lint: lint-rust lint-python

lint-rust clippy:
	cargo clippy --workspace --all-targets -- -D warnings

lint-python:
	cd $(CLIPCANNON_DIR) && $(UV) run --locked $(PYTHON_DEV_EXTRA) ruff check src/ tests/

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

# --- 5090 JEPA Release ---

RELEASE_ID ?= mejepa_5090_artifact_v1.0.0
RELEASE_OUTPUT_ROOT ?= tmp/mejepa_release_artifacts
RELEASE_REFERENCE ?= reference/reference_release_v1.0.0.json
RELEASE_FULL_BUNDLE_SEED ?= 42
RELEASE_REQUIRE_APPTAINER ?= 0
RELEASE_APPTAINER_SIF ?=

RELEASE_APPTAINER_ARGS :=
ifneq ($(strip $(RELEASE_APPTAINER_SIF)),)
RELEASE_APPTAINER_ARGS += --apptainer-sif "$(RELEASE_APPTAINER_SIF)"
endif
ifeq ($(RELEASE_REQUIRE_APPTAINER),1)
RELEASE_APPTAINER_ARGS += --require-apptainer
endif

release-candidate-archive:
	@test -n "$(RELEASE_AGGREGATE)" || { echo "RELEASE_AGGREGATE is required"; exit 1; }
	@test -n "$(RELEASE_LOCAL_CI_MANIFEST)" || { echo "RELEASE_LOCAL_CI_MANIFEST is required"; exit 1; }
	@test -n "$(RELEASE_DOCKER_TAR_1)" || { echo "RELEASE_DOCKER_TAR_1 is required"; exit 1; }
	@test -n "$(RELEASE_DOCKER_TAR_2)" || { echo "RELEASE_DOCKER_TAR_2 is required"; exit 1; }
	@test -n "$(RELEASE_CONTAINER_VERIFY_JSON)" || { echo "RELEASE_CONTAINER_VERIFY_JSON is required"; exit 1; }
	scripts/5090jepa/prepare_release_candidate_archive.sh \
	  --release-id "$(RELEASE_ID)" \
	  --aggregate "$(RELEASE_AGGREGATE)" \
	  --local-ci-manifest "$(RELEASE_LOCAL_CI_MANIFEST)" \
	  --docker-tar-1 "$(RELEASE_DOCKER_TAR_1)" \
	  --docker-tar-2 "$(RELEASE_DOCKER_TAR_2)" \
	  --container-verify-json "$(RELEASE_CONTAINER_VERIFY_JSON)" \
	  --reference "$(RELEASE_REFERENCE)" \
	  --output-root "$(RELEASE_OUTPUT_ROOT)" \
	  --full-bundle-seed "$(RELEASE_FULL_BUNDLE_SEED)" \
	  $(RELEASE_APPTAINER_ARGS)

release-candidate-archive-v2:
	@test -n "$(RELEASE_AGGREGATE)" || { echo "RELEASE_AGGREGATE is required"; exit 1; }
	@test -n "$(RELEASE_LOCAL_CI_MANIFEST)" || { echo "RELEASE_LOCAL_CI_MANIFEST is required"; exit 1; }
	@test -n "$(RELEASE_DOCKER_TAR_1)" || { echo "RELEASE_DOCKER_TAR_1 is required"; exit 1; }
	@test -n "$(RELEASE_DOCKER_TAR_2)" || { echo "RELEASE_DOCKER_TAR_2 is required"; exit 1; }
	@test -n "$(RELEASE_CONTAINER_VERIFY_JSON)" || { echo "RELEASE_CONTAINER_VERIFY_JSON is required"; exit 1; }
	@test -n "$(RELEASE_APPTAINER_SIF)" || { echo "RELEASE_APPTAINER_SIF is required"; exit 1; }
	scripts/5090jepa/prepare_release_candidate_archive_v2.sh \
	  --release-id "$(RELEASE_ID)" \
	  --aggregate "$(RELEASE_AGGREGATE)" \
	  --local-ci-manifest "$(RELEASE_LOCAL_CI_MANIFEST)" \
	  --docker-tar-1 "$(RELEASE_DOCKER_TAR_1)" \
	  --docker-tar-2 "$(RELEASE_DOCKER_TAR_2)" \
	  --container-verify-json "$(RELEASE_CONTAINER_VERIFY_JSON)" \
	  --reference "$(RELEASE_REFERENCE)" \
	  --output-root "$(RELEASE_OUTPUT_ROOT)" \
	  --full-bundle-seed "$(RELEASE_FULL_BUNDLE_SEED)" \
	  --apptainer-sif "$(RELEASE_APPTAINER_SIF)" \
	  --require-apptainer
