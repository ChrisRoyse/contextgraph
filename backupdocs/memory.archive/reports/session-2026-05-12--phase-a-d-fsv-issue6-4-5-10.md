---
namespace: reports
agent_id: codex
timestamp: 2026-05-12T22:04:19Z
status: completed
tags: phase-a-d, fsv, github-issues, ast, instruments, corpus, vram, samp
---

# Phase A-D FSV / Issue Triage Session

## Directives followed

- Read and followed `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`.
- Read and followed `/home/cabdru/contextgraph/docs2/hardeningprompt.md`.
- Read and followed `/home/cabdru/contextgraph/docs2/flagprompt.md`.
- Used GitHub issues as active source of truth and Exa MCP for best-practice research.

## Issue #6 AST compatibility closure

Earlier in this session, the legacy `AstCodeChunker` facade was converged onto the canonical 11-language AST path and verified with:

- `cargo test -p context-graph-core memory::ast_chunker -- --nocapture`
- `cargo test -p context-graph-core memory::code_capture -- --nocapture`
- `cargo test -p context-graph-core memory::code_watcher -- --nocapture`
- `cargo check -p context-graph-core --examples`
- `cargo run -p context-graph-core --example legacy_ast_chunker_parity_fsv`
- `cargo clippy -p context-graph-core --examples --tests -- -D warnings`
- `cargo test -p context-graph-core`
- `cargo fmt --check`
- `cargo build --release -p context-graph-core --examples`
- `cargo run --release -p context-graph-core --example legacy_ast_chunker_parity_fsv`

Source-of-truth evidence:

- `/mnt/d/contextgraph/fsv/legacy-ast-chunker-parity-fsv/legacy-ast-chunker-parity-fsv.json`
- `/mnt/d/contextgraph/fsv/ast-codechunker-convergence-fsv/ast-codechunker-convergence-fsv.json`

GitHub issue #6 is closed.

## Issue #4 semantic instrument heads

Root cause found: `EmbedderHeadInput` previously accepted arbitrary `Vec<f32>` analyzer features. That meant a neural head could be projected without source-bound analyzer provenance.

Implemented in `context-graph-mejepa-instruments`:

- Added `AnalyzerFeatureEvidence` with `slot`, `source_id`, `analyzer`, `feature_version`, and `vector`.
- Added `AnalyzerFeatureEvidence::from_instrument(...)` to derive evidence from real typed instruments.
- `EmbedderBackedInstrumentHead` now requires non-empty typed analyzer evidence, exact source-id match, exact slot match, exact slot dimension, finite values, and non-zero norm.
- Existing `embedder_heads_fsv` now uses real `ETraceInstrument` analyzer evidence.
- Added `semantic_heads_fsv` covering real pretrained E7/E1/E10 forward outputs plus analyzer evidence for E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test, E_Trace, E_Reasoning, and E_Runtime analyzer output.

Source-of-truth evidence:

- `/mnt/d/contextgraph/fsv/contextgraph-mejepa-semantic-heads-fsv/semantic-heads-evidence.json`
- Evidence SHA-256: `494787707510d5b2e96a90fcbf203b524e977dba6af6de9a685cb4a174a5a515`
- 8 embedder-backed head rows, 9 output files, all `0600`.

Manual edge cases verified:

- Missing analyzer evidence fails closed.
- Analyzer source-id mismatch fails closed.
- Non-finite analyzer evidence fails closed.
- Invalid runtime metrics fail closed.

Verification:

- `cargo run -p context-graph-mejepa-instruments --example semantic_heads_fsv`
- `cargo run -p context-graph-mejepa-instruments --example embedder_heads_fsv`
- `cargo test -p context-graph-mejepa-instruments -- --nocapture`
- `cargo clippy -p context-graph-mejepa-instruments --examples --tests -- -D warnings`
- `cargo fmt --check`

Issue #4 was commented but left open because broader production gaps remain: external type/lattice analyzers, coverage delta aggregator, and production chunk pipeline feeding neural vectors into every semantic slot.

## Issue #5 300x8 corpus ship-gate

Reviewed existing local/uncommitted 300x8 ship-gate work and ran it against the real D-drive corpus.

Source-of-truth evidence:

- `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`
- RocksDB: `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/train-eval-rocksdb`
- Evidence SHA-256: `6285999e431781e6bc606d24ee5431eed0e54896ade2591f83ef802c6a9b3d95`

Readback state:

- Corpus verify: `passes=true`, `entry_count=2400`.
- Loaded splits: train=1920, calibration=240, holdout=240.
- Physical train cert CF count: 30.
- Training cert chain: verified 30, broken_at=null.
- Physical holdout report CF count: 4.
- Final holdout report readback step: 20000.
- Final agreement: `0.8666667`.
- Final `phase3_dod_passed=false`.

Blockers from evidence:

- Overall agreement `0.866667 < 0.95`.
- `swap_variable 0.766667 < 0.95`.
- `off_by_one 0.566667 < 0.95`.
- `subtle_flip 0.700000 < 0.95`.

Issue #5 was commented and left open because the real ship gate ran but failed honestly.

Verification:

- `cargo check -p context-graph-mejepa-train --examples`
- `cargo run -p context-graph-mejepa-train --example swebench_300x8_ship_gate_fsv`
- `cargo test -p context-graph-mejepa-train -- --nocapture`
- `cargo clippy -p context-graph-mejepa-train --examples --tests -- -D warnings`

## Issue #10 WSL VRAM telemetry

Reviewed existing local/uncommitted WSL VRAM hardening and verified it.

Source-of-truth evidence:

- `/mnt/d/contextgraph/fsv/contextgraph-mejepa-vram-wsl-fsv/vram-wsl-telemetry-evidence.json`
- Evidence SHA-256: `778c1c8992a9be44808770fa476e85a07fba2a288295a341ccab5274497c1f66`

Readback state:

- GPU: `NVIDIA GeForce RTX 5090`.
- Compute capability: `12.0`.
- CUDA driver total bytes: `34190458880`.
- CUDA driver free bytes: `32429309952`.
- Required bytes: `1073741824`.
- Gate `passes=true`.
- `nvidia-smi` diagnostics do not override CUDA driver readback.

Manual edge cases verified:

- Missing `nvidia-smi` command is diagnostic-only.
- WSL unsupported sentinel (`N/A`) is diagnostic-only.
- Malformed memory total fails closed for diagnostic parsing.
- Valid diagnostic output is recorded.

Verification:

- `cargo run -p context-graph-mejepa-embedders --example vram_wsl_telemetry_fsv`
- `cargo test -p context-graph-mejepa-embedders vram -- --nocapture`
- `cargo test -p context-graph-mejepa-embedders -- --nocapture`
- `cargo clippy -p context-graph-mejepa-embedders --tests --examples -- -D warnings`

GitHub issue #10 is closed.

## Exa MCP research used

- Rust API compatibility/deprecation: preserve compatibility via deprecated shims/reexports and document replacements; remove only on semver breaks.
- AST/code chunking: preserve syntactic units and use parser/AST spans rather than arbitrary string windows.
- ML evaluation: keep train/calibration/holdout distinct, prevent label leakage, persist rolling reports and source-of-truth counts.
- Static/code analysis: Tree-sitter is appropriate for fast structural parsing; semantic analyzers are still needed for deeper type/data-flow lattices.

## Remaining open issues

- #15 Phase A-D umbrella.
- #13 UTML formula/carrier reconciliation.
- #11 production calibration/OOD/latency telemetry.
- #9 production chunk-level TCT centroid refresh.
- #8 production supervision of GC/subscriber/eval jobs.
- #5 ship gate still fails target despite real FSV.
- #4 external analyzer and production semantic-head feed gaps remain.

## Notes

- The worktree contains concurrent/user changes outside the files I directly edited, including train ship-gate work and VRAM WSL hardening. I did not revert them.
- `git diff --check` passed.
