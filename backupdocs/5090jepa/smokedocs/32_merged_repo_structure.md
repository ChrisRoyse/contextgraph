# 32 - Merged Repo Structure

## Goal

Specify the **physical layout** of the monolith repo now that
ClipCannon is vendored at `./clipcannon/`, and the **build/test/CI**
plumbing that makes Rust + Python + CUDA build as one thing.

Pre-existing Rust workspace + freshly vendored Python tree must
coexist without becoming two parallel projects.

## State As Of 2026-04-30

ClipCannon is at `/home/cabdru/contextgraph/clipcannon/`. Confirmed:

- 153 MB total (74 MB models, 3 MB src, 1.2 MB tests, 2.3 MB data,
  small assets/config/scripts/benchmarks);
- `clipcannon/.git/`, `.claude/`, `.playwright-mcp/` removed
  (vendored, no longer a separate repo);
- `clipcannon/pyproject.toml` declares Python ≥3.12, BSL-1.1,
  `clipcannon` package + 5 sub-packages (`clipcannon`, `phoenix`,
  `voiceagent`, `license_server`, `chrome_extension`);
- `clipcannon/Dockerfile`, `CLAUDE.md`, `README.md`, `LICENSE`
  retained at the subdirectory level for now.

## Target Repo Layout

```text
contextgraph/                              # mono-repo root
  README.md                                # rewritten for monolith (smokedoc 31)
  LICENSE                                  # repo-level umbrella; sub-licenses below
  CLAUDE.md                                # already present
  Cargo.toml                               # Rust workspace root
  Cargo.lock
  pyproject.toml                           # NEW: top-level Python pointer to clipcannon/
  Makefile                                 # NEW: cross-language orchestrator
  Dockerfile                               # NEW: monolith container (replaces clipcannon/Dockerfile)
  .gitignore                               # extended for clipcannon/models/, large artifacts
  .github/workflows/                       # CI tiers (smokedoc 23)

  crates/                                  # Rust workspace (existing)
    context-graph-core/
    context-graph-storage/
    context-graph-embedders/                # 13-embedder DDA panel
    context-graph-dynamicjepa/              # ME-JEPA runtime
    context-graph-constellation/            # NEW: shared CG crate (smokedoc 28)
    context-graph-cli/                      # one CLI binary
    context-graph-mcp/                      # one MCP server
    ... (other existing crates)

  clipcannon/                              # vendored 2026-04-30
    src/
      clipcannon/                          # 23-stage DAG (Python)
      phoenix/                             # avatar engine (Python)
      voiceagent/                          # meeting bot (Python)
      license_server/                      # billing (FastAPI)
      chrome_extension/                    # browser ext (JS)
    config/                                # ClipCannon-specific configs (kept)
    data/                                  # training fixtures (small; kept)
    models/                                # FLAME2020 etc. (.gitignore candidate)
    scripts/                               # legacy scripts (kept)
    tests/                                 # ClipCannon test suite (kept)
    benchmarks/                            # ClipCannon benchmarks (kept)
    pyproject.toml                         # ClipCannon package metadata (kept)
    uv.lock                                # Python lock (kept)
    LICENSE                                # ClipCannon BSL-1.1 (kept)
    README.md                              # ClipCannon-internal README (kept)

  configs/                                 # cross-system configs
    dynamicjepa/
      domain_packs/
      centroids/                           # NEW: frozen centroid sets (smokedoc 28)
      train_configs/

  docs/
    5090jepa/                              # ME-JEPA charter + smokedocs (this dir)
    contextgraph/                          # CG architecture docs (existing)
  docs2/                                   # PAPER.md, alldata.md, transcripts

  paper/                                   # NEW: one-paper LaTeX (smokedoc 19)
    main.tex
    Makefile
    figures/
    tables/auto/                           # auto-generated from bundles
    sections/
    bibliography.bib
    supplementary/

  tmp/                                     # ephemeral: bundles, runs, models
    mejepa_research_runs/                  # existing
    onet_probe/                            # existing
```

## License Boundary At Repo Level

- Repo umbrella `LICENSE` stays as-is (current MIT/Apache or whatever
  the existing file says).
- `clipcannon/LICENSE` (BSL-1.1) stays where ClipCannon code lives.
- `crates/context-graph-constellation/` should ship Apache-2.0 dual
  MIT (per smokedoc 28 / smokedoc 35 plan; this is the most
  strategically reused crate).
- `paper/` is CC-BY-4.0 (academic work).
- A top-level `THIRD_PARTY_LICENSES/` collects bundled-data licenses
  (O*NET CC-BY, ESCO, etc., per smokedoc 21).

A top-level `LICENSING.md` enumerates which subdirectory carries
which license. Mixing licenses in one repo is fine; not documenting it
is not.

## Cross-Language Build (Top-Level Makefile)

```makefile
# Top-level build orchestrator. One command per common workflow.

.PHONY: build test lint clean container reproduce

build:                       ## Build Rust workspace + install Python tree
	cargo build --release --workspace
	cd clipcannon && uv sync --frozen

test:                        ## Run Rust + Python tests
	cargo test --workspace --release
	cd clipcannon && uv run pytest tests/

lint:                        ## Lint both languages
	cargo clippy --workspace --all-targets -- -D warnings
	cd clipcannon && uv run ruff check src/ tests/

container:                   ## Build the monolith container
	docker build -t teleox-monolith:dev -f Dockerfile .

reproduce:                   ## Reproduce headline bundle (smokedoc 20)
	./scripts/reproduce.sh paper_full_50k --seeds 42

clean:
	cargo clean
	cd clipcannon && rm -rf .venv __pycache__ .pytest_cache
```

`uv` (the Python package manager already used by ClipCannon per
`uv.lock`) is the chosen Python toolchain. No `pip install` ad hoc.

## Top-Level `pyproject.toml`

A minimal top-level pointer makes `uv` see ClipCannon from the repo
root:

```toml
[tool.uv.workspace]
members = ["clipcannon"]
```

Now `uv sync` at repo root resolves the ClipCannon environment from
its own `pyproject.toml`.

## Top-Level `Dockerfile`

One container, multi-stage:

```dockerfile
# Stage 1: Rust builder
FROM nvidia/cuda:13.2.0-devel-ubuntu24.04 AS rust-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl pkg-config libssl-dev build-essential
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.95.0
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /work
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY configs/ ./configs/
RUN cargo build --release --workspace --bins

# Stage 2: Python builder
FROM nvidia/cuda:13.2.0-devel-ubuntu24.04 AS python-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3.12 python3.12-venv python3-pip
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.cargo/bin:/root/.local/bin:${PATH}"
WORKDIR /work
COPY clipcannon/ ./clipcannon/
COPY pyproject.toml ./
RUN uv sync --frozen

# Stage 3: Runtime
FROM nvidia/cuda:13.2.0-runtime-ubuntu24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3.12 ffmpeg
WORKDIR /app
COPY --from=rust-builder /work/target/release/context-graph /app/context-graph
COPY --from=python-builder /work/clipcannon /app/clipcannon
COPY --from=python-builder /work/.venv /app/.venv
COPY scripts/ /app/scripts/
ENV PATH="/app/.venv/bin:${PATH}"
ENV CUDA_COMPUTE_CAP=120
ENTRYPOINT ["/app/context-graph"]
```

The runtime image is the **single deployable artifact**. The reviewer
recompute path (smokedoc 20) targets exactly this image.

## .gitignore Additions

```text
# ClipCannon model assets (large; deliver via separate model-pack archive)
clipcannon/models/*.safetensors
clipcannon/models/*.bin
clipcannon/models/*.pkl
clipcannon/models/FLAME2020/

# ClipCannon ephemeral
clipcannon/tmp/
clipcannon/.venv/

# Cross-language build artifacts
target/
.venv/
__pycache__/
*.pyc
```

The 74 MB `clipcannon/models/` directory should not be committed; it
is a release artifact distributed alongside the container image.
Without those models a fresh clone won't run ClipCannon-side code,
which is fine for the Rust-only side.

A separate `model-pack-v1.0.tar.gz` archived to Zenodo holds the
weights; the Apptainer/Docker container fetches it at first run.

## Cross-Language Test Strategy

Three test layers:

1. **Rust unit tests**: per-crate, fast, no Python.
2. **Python unit tests**: per-package, fast, no Rust.
3. **Cross-language integration tests** (`tests/` at repo root): one
   process, one CLI, end-to-end through both languages. Driven by
   smokedoc-04 gates and smokedoc-23 CI tiers.

CI (smokedoc 23) runs:

- Tier A (per-PR): cargo test + uv pytest + tier-A smoke (~10 min);
- Tier B (nightly): full integration (~2 hours);
- Tier C (release): reproducibility + container double-build.

## How Subsystems Interoperate (Until smokedoc 33 finalizes)

For smokedoc 26's "one process" claim to be real, we need a Rust↔Python
boundary. Two paths, one decision in smokedoc 33:

- **PyO3 path**: Rust embeds CPython via `pyo3`; ClipCannon stages
  invoked as in-process Python calls.
- **Supervisor path**: one process spawns a Rust subprocess + a
  Python subprocess; in-host IPC over MCP.

Smokedoc 33 picks one. Until then, treat both as "one repo, one
deployment unit, two-process or one-process is a binding decision".

## Operational Items (Pre-v1.0)

- [ ] Top-level `pyproject.toml` referencing ClipCannon as workspace
  member.
- [ ] Top-level `Makefile` with `build / test / lint / container /
  reproduce` targets.
- [ ] Top-level `Dockerfile` (multi-stage as specified).
- [ ] `.gitignore` extended for ClipCannon model files.
- [ ] `LICENSING.md` at root cataloguing per-subdirectory licenses.
- [ ] Top-level README rewritten for the monolith (smokedoc 31's
  capability list).
- [ ] Decision: keep `clipcannon/CLAUDE.md` or merge content into
  root `CLAUDE.md`. Recommendation: keep per-subdirectory CLAUDE
  files; they instruct sub-tree-specific assistants.
- [ ] Decision: keep `clipcannon/Dockerfile` or delete now that we have
  monolith Dockerfile. Recommendation: delete `clipcannon/Dockerfile`
  to avoid confusion.

## Acceptance

Repo merge is **complete** when:

- `make build` from repo root produces a working `target/release/
  context-graph` binary AND a populated `.venv/` for ClipCannon;
- `make test` runs both languages and exits 0;
- `make container` builds a single Docker image;
- `make reproduce` end-to-end against a sample corpus;
- README at repo root reflects the monolith, not just Context Graph;
- one git commit captures the ClipCannon vendoring (history will
  show `clipcannon/` arrived as one big import on 2026-04-30).
