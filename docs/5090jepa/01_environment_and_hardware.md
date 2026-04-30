# 01 — Environment & Hardware

The 5090jepa demo is targeted at exactly one machine. Before any phase you must verify the environment matches. If anything diverges from the values below, halt and resolve before writing code. No "should work" assumptions.

Current implementation status as of the Phase 6 double-check on 2026-04-30: Phases 0-6 are implemented and FSV-verified through the CLI/storage path. The DynamicJEPA service crate and MCP tool surface remain future Phase 7/9 work.

## 1. Operator Hardware (Verified)

| Item | Value | Source |
| --- | --- | --- |
| GPU | NVIDIA GeForce RTX 5090 | `nvidia-smi --query-gpu=name --format=csv,noheader` |
| Architecture | Blackwell, sm_120, compute capability 12.0 | `nvidia-smi --query-gpu=compute_cap --format=csv,noheader` |
| VRAM | 32607 MiB ≈ 32 GB | `nvidia-smi --query-gpu=memory.total --format=csv,noheader` |
| Driver | 596.21 | `nvidia-smi --query-gpu=driver_version --format=csv,noheader` |
| CUDA Toolkit | 13.2 (V13.2.78) | `nvcc --version` |
| Host OS | Windows 11 Pro | Geekbench dump in `docs2/systemspecs.md` |
| Linux runtime | WSL2 Ubuntu (kernel 6.6.87.2-microsoft-standard-WSL2) | `uname -a` |
| CPU | AMD Ryzen 9 9950X3D, 16C / 32T, AVX‑512 | `lscpu` |
| RAM | 128 GB DDR5‑3592 dual‑channel | `free -h` |

A fresh agent must run all of these probes (Section 2) and confirm every value. If `nvidia-smi` cannot be found inside WSL, CUDA is not exposed to WSL — escalate before proceeding.

## 2. Environment Probe (Run This Verbatim Before The Next Phase)

```bash
set -e
echo "== gpu =="
nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total --format=csv,noheader
echo "== cuda =="
nvcc --version | grep "release"
echo "== rust =="
rustc --version
cargo --version
echo "== workspace =="
cd /home/cabdru/contextgraph
git status --short
git log --oneline -5
echo "== candle pin =="
grep -E "candle-(core|nn)" Cargo.toml
echo "== existing crates =="
ls crates/
echo "== existing storage =="
ls crates/context-graph-storage/src/
echo "== dynamicjepa current phase state =="
test ! -d crates/context-graph-dynamicjepa || (echo "ERROR: Phase 7 service crate exists; audit before continuing" && exit 1)
test -d crates/context-graph-core/src/dynamicjepa || (echo "ERROR: Phase 1 core module missing" && exit 1)
test -d crates/context-graph-storage/src/dynamicjepa || (echo "ERROR: Phase 2 storage module missing" && exit 1)
test -f crates/context-graph-cli/src/commands/dynamicjepa.rs || (echo "ERROR: Phase 3 CLI module missing" && exit 1)
rg -q "IngestEvent" crates/context-graph-cli/src/commands/dynamicjepa.rs || (echo "ERROR: Phase 4 ingest CLI missing" && exit 1)
rg -q "MaterializePanel" crates/context-graph-cli/src/commands/dynamicjepa.rs || (echo "ERROR: Phase 4 panel CLI missing" && exit 1)
echo "ENV OK"
```

If the script prints `ENV OK`, you may proceed. Any other outcome is a red flag.

## 3. Required Build Environment Variables

Set these in your shell before any `cargo build`:

```bash
# 5090jepa demo build env
export CUDA_COMPUTE_CAP=120
export CUDA_HOME=/usr/local/cuda-13.2
export PATH="$CUDA_HOME/bin:$PATH"
export LD_LIBRARY_PATH="$CUDA_HOME/lib64:${LD_LIBRARY_PATH:-}"
# context-graph specifics
export CONTEXT_GRAPH_ENV=development
export CONTEXT_GRAPH_DAEMON=0           # we run MCP/CLI synchronously for FSV
export RUST_LOG=info,context_graph_dynamicjepa=debug,context_graph_storage=info
```

**Do not** export `CUDA_COMPUTE_CAP` to a value other than `120`. Building for a different cap and then running on Blackwell triggers JIT PTX compilation on first kernel launch, which is slow and masks runtime errors. Force AOT cubin for sm_120.

If `CUDA_COMPUTE_CAP=120` causes a candle build error, capture the full error and stop. Do not switch to a lower compute cap or a PTX JIT path; fix the CUDA/Candle/toolchain mismatch directly.

## 4. Build Commands

```bash
# clean release build of all default-members (excludes benchmark crate)
cargo build --workspace --release

# build with full features including dynamicjepa once the crate is added
cargo build --workspace --release --all-features

# run dynamicjepa-only tests through the current implemented phase
cargo test -p context-graph-core dynamicjepa --all-features
cargo test -p context-graph-storage dynamicjepa --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase3_fsv --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase4_fsv --all-features

# future Phase 7 service crate, once it exists
# cargo test -p context-graph-dynamicjepa --all-features
```

Expected first-build time on this hardware: ~3–5 minutes for incremental, ~12–15 minutes cold. If a cold build exceeds 20 minutes, something is recompiling LLVM IR — investigate (almost always `CUDA_COMPUTE_CAP` flapping between values).

## 5. Runtime Pre‑Flight (Before Each FSV Run)

```bash
# Confirm GPU is alive and uncontested
nvidia-smi --query-gpu=memory.used,memory.free --format=csv,noheader

# Confirm no other context-graph-mcp daemon holds the demo DB lock
test ! -f /tmp/5090jepa-cw-fsv/mcp.pid || (echo "ERROR: stale lock on demo DB"; exit 1)
test ! -f /tmp/5090jepa-gw-fsv/mcp.pid || (echo "ERROR: stale lock on demo DB"; exit 1)

# Wipe demo DBs (no migration policy — wipe and rebuild per R1)
rm -rf /tmp/5090jepa-cw-fsv /tmp/5090jepa-gw-fsv
rm -rf /tmp/5090jepa_artifacts
mkdir -p /tmp/5090jepa_artifacts
```

These commands are safe — they only touch `/tmp/` paths the demo creates. They never touch the operator's real Context Graph DB.

## 6. Known Failure Modes On Blackwell + WSL2

Document them here so the next fresh agent does not re‑debug:

- **`Could not start backend: Runtime compute cap 120 is not compatible with compile time compute cap X`** — Means a built artifact targeted a different cap. Fix with `cargo clean -p candle-core -p candle-nn && CUDA_COMPUTE_CAP=120 cargo build --release`.
- **`CUDA_ERROR_INVALID_IMAGE` / module load failure** — Native CUDA image load failed. Almost always: `nvcc` on PATH is not CUDA 13.2 or the cubin was built for the wrong architecture. Confirm `nvcc --version` reports 13.2 and `cuobjdump --dump-elf` reports `sm=120a`.
- **`no kernel image is available for execution on the device`** — Build was cubin‑only for the wrong sm. Same fix as the first item.
- **WSL2 driver mismatch** — If `nvidia-smi` works on Windows but not in WSL, the WSL CUDA passthrough is broken. Check `/usr/lib/wsl/lib/libcuda.so.1` symlink and `/etc/wsl.conf` GPU support.

## 7. Disk Layout For The Demo

```
/tmp/5090jepa-cw-fsv/        # counter_world demo DB (wiped per run)
/tmp/5090jepa-gw-fsv/        # gridworld demo DB (wiped per run)
/tmp/5090jepa-phase4-manual-cw/ # Phase 4 manual counter_world source-of-truth DB
/tmp/5090jepa-phase4-manual-gw/ # Phase 4 manual gridworld source-of-truth DB
/tmp/5090jepa_artifacts/     # tiny model artifact files (wiped per run)
tmp/5090jepa_evidence/       # JSON evidence logs committed to repo? NO — never commit /tmp; this is under repo `tmp/` so it persists across runs
```

The repo's `tmp/` directory exists per CLAUDE.md ("never save working files… to the root folder"). All ephemeral demo evidence goes to `tmp/5090jepa_evidence/<domain>/<run-id>/` so it survives in case the user wants to inspect it but never gets committed (`tmp/` should be in `.gitignore`; verify before Phase 0 and add if missing).

## 8. What You Are Not Allowed To Change

- The pinned candle versions (`0.10.2`). Bumping candle beyond this requires a fresh cudarc/CUDA 13.2 verification pass.
- The Rust toolchain (`stable`, `1.75` MSRV in workspace). Demo runs on whatever stable is installed (currently 1.92.0); we do not pin a newer minimum here.
- The existing 73 RocksDB column families. The DynamicJEPA CFs are *additive*.
- The existing 110 MCP tools with `llm` enabled (106 without). DynamicJEPA tools are *additive*.

If you encounter a real blocker that requires bumping candle or the toolchain, stop and ask the user.
