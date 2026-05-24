# 01 — System Overview

**Source files:** `Cargo.toml`, `CLAUDE.md`, `AGENTS.md`, `crates/context-graph-mcp/src/lib.rs`, `crates/context-graph-mejepa/src/lib.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`, `crates/context-graph-mejepa-instruments/src/lib.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs`, `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-mcp/src/tools/definitions/mod.rs`.

---

## 1. What this system is

**ME-JEPA-Code** is a **binary reality predictor**. AI coding agents (Claude Code / Codex / Cursor) call it via local MCP JSON-RPC. For any candidate code change:

```
verdict ∈ { Pass, Fail }    // "Will this change exist in working reality? Yes/No."
+ named_failure_mode         // (Fail) labeled cluster from failure-mode hierarchy
+ closest_exemplars          // top-K historical patches from 300×8 corpus
+ violating_slots            // per-slot OOD reasons + cross-embedder contradiction evidence
+ mistake_id (when wrong)    // ground truth lands in CF_MEJEPA_MISTAKE_LOG → online learning
```

One prediction per candidate. No best-of-K. No `Abstain` at the product surface. When wrong, the mistake is captured and the predictor adapts.

The five Q-questions:

| Q | Status |
|---|---|
| **Q1** | **Binary prediction.** Did the AI's claim physically land in bytes/AST? Reality = bytes on disk + AST diff. |
| **Q2** | **The predictor itself.** Output `Verdict::Pass\|Fail`. Reality = Docker oracle (`swebench.harness.run_evaluation`). |
| **Q3** | **Explanation only.** Named failure mode + closest exemplar attached to `Fail` verdict. Labels Q2; no own prediction. |
| **Q5** | **Binary prediction.** Per predicted side effect: did the corresponding shift-log event occur? Reality = `/zfs/archive/contextgraph/runtime/shift-log.jsonl`. |
| **Q4** | **Removed (#408).** Ambiguous heads (perf, accuracy, cost, reasoning) have no free binary form against reality. Frozen CFs remain readable; producer track closed. |

**Why Q4 is dropped, not thresholded.** Continuous quantities (perf wall-clock, mutation survival rate) only become binary via arbitrary thresholds. Threshold is heuristic, not reality. Mistake-loop convergence requires real disagreement with reality.

### 1.1 Ship-gate predicate

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per mathematically-valid (mutation_category × language) mixed cell
  with degenerate all-pass/all-fail cells using #532 replacement contract
  on 300×8 SWE-bench Lite Python corpus
  using Q2 Docker-oracle agreement
  AND cross-panel oracle correlation ≥ 0.95 on Panel B (#405)
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Degenerate-cell metric (#532).** Pearson is undefined on cells with zero oracle variance (e.g., `compile_error::python`, `wrong_file::python`). For those, FSV uses target-class recall ≥ 0.95, actual-class confidence ≥ 0.95, Brier ≤ 0.05, ECE ≤ 0.05, with power diagnostics (Fisher-z CI for mixed cells; Wilson lower bound for degenerate recall).

### 1.2 Current measurement

Latest ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`:

| Metric | Value |
|---|---|
| Raw holdout `prediction_oracle_correlation` | `0.4843192995` |
| Brier | `0.0923729688` |
| ECE | `0.0240316074` |
| Report sha256 | `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499` |

Honest gap to ship: `0.4657` against raw Pearson.

**Product boundary.** LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`.

**Consequence-prediction contract (#397).** Model observes explicit task text, agent claims, edits, AST deltas, frozen instruments, oracle outcomes, verification feedback. Does NOT infer hidden human intent. Predicted consequences + evidence presented to the operator/agent to decide proceed/repair/reroute.

---

### 1.3 The labeling layer (meaning surface)

Raw slot violations + cross-embedder correlations are clustered into a **4-level hierarchy of named failure modes**. The product returns the named mode that applies, grounded by closest historical exemplar.

**Shipped substrate:**

| Layer | Artifact | Rows |
|---|---|---|
| AST chunks | `python_ast_chunks.jsonl` | 399,777 |
| Level-0 labels per chunk (#413) | `chunk_constellation_labels.jsonl` | 399,777 |
| Accepted Level-0 label registry (#413) | `accepted_label_registry.jsonl` | 491 |
| Level-2 named skills (#414) | `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` | 128 |
| Chunk → skill memberships (#414) | `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` | 36,773 |
| Skill → chunk reverse (#414) | `CF_MEJEPA_SKILL_REVERSE_INDEX` | 10,579 |
| Level-1/2/3 hierarchy + named predictions (#404) | `CF_MEJEPA_FAILURE_MODE_LEVEL{1,2,3}_*`, `CF_MEJEPA_NAMED_PREDICTIONS` | 8 groups / 157 skills / 379 constellations / 536 named predictions |

**Live-input safety:** target-side labels (oracle/Docker/test outcomes) are emitted with `leakage_policy:target_side_only`. Supervision/reward inputs only — **never** live predictor inputs.

---

### 1.4 Online mistake-driven adaptation

The predictor's head over the panel is wrapped in an **online-mistake-driven update loop** (#406, #523, shipped):

1. Predictor returns `Pass | Fail` for candidate patch.
2. Reality (Docker oracle + PostToolUse capture + shift-log replay) returns ground truth.
3. Match → no update.
4. Disagree → **mistake row** in `CF_MEJEPA_MISTAKE_LOG` with full context.
5. Trainer consumes via key-local exact-panel online-head state (`CF_MEJEPA_ONLINE_HEAD_STATE`).
6. **Convergence target:** `mistake_repeat_rate ≤ 0.05` over rolling 100-mistake window.

Current protection mode: `exactPanelKeyIsolation`. Unrelated-control delta ≤ 0.02. True Fisher/EWC is NOT claimed without durable Fisher rows (#524 guards the boundary).

MCP surface (#523): `mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status`.

---

### 1.5 Cross-panel triangulation (Goodhart guard, #405 planned)

The predictor is trained against Panel A and the Gτ guard is evaluated against the same Panel A. Encoder-matched → 0.95 on Panel A could mean the predictor learned the panel.

Panel B (non-overlapping encoders) re-scores every gate-relevant patch:

| Slot | Panel A | Panel B |
|---|---|---|
| AST | tree-sitter → 384-d | CodeBERT → 384-d |
| CFG | attention encoder → 256-d | Pythia program-graph → 256-d |
| Data-flow | def-use attention → 256-d | joern → 256-d |
| Type-graph | tree-sitter type → 256-d | pytype JSON → 256-d |
| Code-semantic | Qodo-Embed-1 → 1536-d | Voyage-code-2 → 1536-d |
| Test/Problem/CommitMsg | e5-large-v2 | bge-large-en-v1.5 |
| Trace/Witness/Oracle/StaticAnalysis/Runtime/Reasoning/Scalars | unchanged | unchanged (deterministic) |

Fail-closed: Panel A ≥ 0.95 AND Panel B < 0.85 → `CROSS_PANEL_GOODHART_DETECTED`.

---

### 1.6 The slot-preserving constellation rule (LOCKED)

The `Panel` is an **array of 15 per-embedder vectors**, NOT a 5,120-d fused embedding. The contiguous 5,120 f32 buffer is byte-layout for serialization only.

**Valid:**
- Same-embedder dim-wise compare: `cosine(panel_A.E_AST, panel_B.E_AST)` ✓
- Array-vs-array per-slot map (vector of 15 same-embedder cosines)
- Strict-conjunction gate over the map
- Per-pair similarity matrix (diagonal only — off-diagonal undefined)
- Cross-slot scalar pairwise: `MI(slot_a, slot_b)`, `correlation(slot_a, slot_b)`
- Matrix-style ops respecting slot identity (per-slot weighted aggregation, masked projections)

**Forbidden:**
- `cosine(panel.E_AST, panel.E1)` (different coordinate systems) ✗
- `linear(5120, hidden)` over packed buffer (silent cross-embedder compensation) ✗
- `residual_norm_sq` over packed 5,120 buffer ✗
- Any operation where a strong slot compensates for a weak slot

**Code-level enforcement.** `crates/context-graph-mejepa-tct/src/gtau.rs:14` iterates `for embedder in EmbedderId::all()`; `cosine_similarity` at `gtau.rs:90` errors `MEJEPA_TCT_DIM_MISMATCH` on length mismatch. The slot-preserving late-fusion predictor (#387) replaced the flat-projection predictor.

---

### 1.7 Stars-as-Vectors (celestial-cartography model)

The teleological-constellation primitive is named after star constellations; the analogy is formal. See `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`.

| Astronomy | ContextGraph |
|---|---|
| One star = vector in ~100-dim observational space (Gaia DR3) | One AST chunk = array of 15 per-embedder vectors |
| Same-spectrum-band cosine valid | Same-embedder cosine valid |
| Cross-band dim-wise compare = nonsense | Cross-embedder dim-wise compare forbidden |
| Constellations perceived from viewing angle | Constellations perceived from `(mutation_category × language × cell)` |
| Big Dipper = 2D projection of 7 high-dim stars | Code-state constellation = signature over panel array |
| Star foundation model predicts missing parameters | ME-JEPA predicts missing reality outcomes |

The constellation isn't in the data, it's in the predictor.

---

### 1.8 The ambiguity boundary (locked rule)

ME-JEPA only predicts questions with **free binary form against reality**.

**Excluded from prediction contract:** perf regressions, maintainability, idiomaticity, latent bugs, code redundancy, edge-case coverage, cost, reasoning quality.

**Kept in prediction contract:**

| Surface | Reality |
|---|---|
| Q1 claim-exists | Bytes / AST on disk |
| Q2 oracle pass | Docker oracle `report.json` |
| Q5 shift events | Durable shift-log JSONL |

Q4 heads remain in schema + CFs (`CF_MEJEPA_Q4_*`) as **frozen historical display data**, readable but not trained, calibrated, or consumed. `Q4_DOCTRINE_FREEZE_ACTIVE = true` at `q4_trust_gate.rs`.

---

## 2. Process / port / tech architecture

| Process | Port / IPC | Technology | Purpose |
|---|---|---|---|
| `context-graph-mcp` server | stdio JSON-RPC (default) or HTTP | Rust, tokio, async-trait | 216-tool MCP surface; daemon mode |
| `context-graph-cli` | CLI invocation | Rust | `mejepa observe`, `mejepa project-ingest`, `mejepa compression-progress`, `mejepa active-learning`, etc. |
| ME-JEPA training daemon | embedded in CLI/binary | candle (CUDA 13.2), RocksDB | UTML training loop; reads `CF_MEJEPA_REPLAY_BUFFER`; writes `CF_MEJEPA_TRAIN_CERTS` |
| Shift-log subscriber | tokio task | tokio | Tails JSONL shift log → materializes panels |
| Heal scheduler | embedded thread | tokio | Drift detection, Fisher/EWC, distill, promotion |
| RocksDB (per runtime root) | local disk | RocksDB ≥ 8.x | ~200 column families |
| GPU runtime | CUDA 13.2 on RTX 5090 Blackwell | cudarc 0.19.x via candle 0.10.2 | Embedder forward passes, predictor training |

**Constraints:**
- **No HTTP from action paths.** MCP server is the only network surface.
- **No inner LLM.** Local generative inference is compile-locked off — `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`.
- **No intent synthesis.** Observable goals + agent claims are evidence; hidden human intent is not a prediction target.

---

## 3. Technology stack

| Layer | Technology | Version |
|---|---|---|
| Language | Rust | edition 2021, MSRV 1.95 |
| Async runtime | tokio | 1.35 |
| Serialization | serde, serde_json, bincode 1.3, toml 0.8 | — |
| Error | thiserror, anyhow | — |
| Telemetry | tracing, tracing-subscriber (json/env-filter) | — |
| ML runtime | candle-core, candle-nn (CUDA) | 0.10.2 (cudarc 0.19.x, CUDA 13.2) |
| Storage | RocksDB | per-crate manifests |
| Crypto / hashing | ed25519, sha2, blake3 | — |
| Vector search | usearch HNSW, hnsw_rs | — |
| License | PolyForm-Noncommercial-1.0.0 | — |

Build profiles (`Cargo.toml:112-128`): `release` (thin LTO, 16 CGUs, strip), `release-max` (1 CGU), `dev` (opt-0, line-info debug, incremental).

---

## 4. Workspace members (22 active crates)

| Crate | Role |
|---|---|
| `context-graph-mcp` | MCP JSON-RPC server + 216 tools (69 `mejepa_*`) |
| `context-graph-cli` | Operator CLI + helpers (`context-graph` short alias) |
| `context-graph-core` | Shared domain types |
| `context-graph-storage` | RocksDB wrapper + teleological/code/causal CFs |
| `context-graph-witness` | ed25519 + Merkle + Stage 0/F certificates |
| `context-graph-solver` | Causal masking, MMR, conformal predictor |
| `context-graph-mincut` | Clean-room Stoer-Wagner global min-cut |
| `context-graph-paths` | Fail-closed durable path resolver (aiwonder enforced) |
| `context-graph-tier-compression` | 8/7/5/3-bit groupwise scalar quantization |
| `context-graph-embeddings` | E1–E14 embedding pipeline |
| `context-graph-cuda` | CUDA helpers |
| **`context-graph-mejepa`** | **Predictor trunk** (73 modules) |
| `context-graph-mejepa-cf` | Canonical CF name registry (zero-dep) |
| `context-graph-mejepa-corpus` | 8× mutation operators + SWE-bench bridge |
| `context-graph-mejepa-instruments` | 15-slot panel + frozen instruments |
| `context-graph-mejepa-embedders` | 21 embedder slots, dynamic registry, calibration |
| `context-graph-mejepa-train` | UTML trainer, replay buffer, mistake loop, certificates |
| `context-graph-mejepa-tct` | Teleological constellations + Gτ guard |
| `context-graph-mejepa-hygiene` | Tier transitions, quota, GC, witness compress |
| `context-graph-mejepa-shift-subscriber` | JSONL shift-log tail → panel materialization |
| `context-graph-dynamicjepa` | 5090jepa MCP surface (39 tools) |
| `context-graph-test-utils` | Deterministic-by-shape helpers for storage/serialization tests only |

Per-crate detail in `02_source_map.md`.

---

## 5. Entry points

| Binary | Purpose |
|---|---|
| `context-graph-mcp` | MCP server. `--mode stdio` (default) or `--mode reality-loop` (subset). |
| `context-graph-cli` / `context-graph` | Operator CLI. `mejepa observe`, `mejepa project-ingest`, `mejepa project-report`, `mejepa compression-progress`, `mejepa active-learning {list\|label\|dismiss}`. |
| `mejepa` | Standalone predictor harness. |
| `mejepa-heal-drill` | Heal scheduler drill. |
| `mejepa-train` | Trainer entry. |
| `train-causal` | Causal embedder training. |
| 210+ FSV examples (`crates/*/examples/*_fsv.rs`) | Per-task FSV harnesses (auto-FSV ecosystem being retired per #655 — see `06 §1`). |

---

## 6. Directory layout

```
crates/                                — Rust workspace (22 active crates)
docs/futurebuild/                       — this doc suite
docs/futurebuild/specs/FSV-PROTOCOL.md  — mandatory verification contract
docs2/AICodingAgentSuperPrompt.md       — master doctrine
docs2/hardeningprompt.md                — 14-axis hardening deep-dive
scripts/cc/                             — Claude Code hook scripts
scripts/ci/                             — CI lint (e.g. check_no_c_drive_paths.sh)
scripts/swebench_lite_ops/              — Python redaction + SWE-bench Lite helpers
tasks/swebench-lite/                    — task manifest JSON
config/                                 — governed paths, model pins
.claude/                                — settings.json, agents/, loop.md
.github/workflows/, .github/labels.yml  — CI + label set (memory = Issues)

# aiwonder production durable SoT
/zfs/archive/contextgraph/
  corpus/                               — production corpus, panel cache, fingerprint registry
  fsv/                                  — per-task FSV evidence
  runtime/                              — shift log, test outcomes, scheduler events
  projects/<project_id>/                — reality-compiler per-project cache, manifest, report
  state/                                — durable runtime state (calibration, labels, q4-trust-gate)
  exports/                              — weekly eval reports, operator exports

# aiwonder hot state
/zfs/hot/contextgraph/
  repo/                                 — git checkout, release binaries
  models/                               — embedder model weights
  corpus/                               — stable hot inputs (chunks.jsonl)
  runtime/                              — active scheduler logs / hot run state
```

---

## 7. Storage tier classification

| Tier | What | Where | Regenerable? |
|---|---|---|---|
| **Sacred** (never edit) | `.env`/`.env.*`; `swebench/**`; `tests/oracle_**/**`; `crates/*/src/reward_specs/**`; `crates/*/src/verifiers/**`; `*.pem`, `*.key`, `**/credentials.json` | various | No |
| **Durable production** | RocksDB CFs (~200); FSV evidence; project-ingest manifests; weekly exports | `/zfs/archive/contextgraph/` | Source-recoverable (corpus) or replay-from-shift-log (predictions). Some FSV artifacts irreplaceable. |
| **Hot** | Active checkpoints, model weights, hot run state | `/zfs/hot/contextgraph/` | Yes |
| **Ephemeral** | `/tmp/contextgraph-*` scratch | `/tmp/` | Yes |

---

## 8. Exception hierarchy (fail-closed structured errors)

Every crate has an `Error` enum with `SCREAMING_SNAKE_CASE` codes via `.code()`:

| Crate | Sample codes |
|---|---|
| `context-graph-mejepa-instruments` | `MEJEPA_INSTRUMENTS_INVALID_INPUT`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`, `MEJEPA_INSTR_FROZEN_VIOLATION` |
| `context-graph-mejepa-corpus` | `MEJEPA_CORPUS_INVALID_INPUT`, `MEJEPA_CORPUS_NO_MUTATION_SITE`, `MEJEPA_CORPUS_LEAKAGE_DETECTED`, `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT` |
| `context-graph-mejepa` | `MEJEPA_INSTRUMENT_GRADIENT_LEAK`, `OOD_CALIBRATOR_MISSING`, `OOD_SCORE_ABOVE_THRESHOLD`, `MULTI_HEAD_CONTRADICTION`, `CONTRADICTION_THRESHOLD_MISSING`, `CROSS_PANEL_GOODHART_DETECTED`, `PATHWAY_AMBIGUOUS_LEAF_REJECTED` |
| `context-graph-mejepa-tct` | `MEJEPA_TCT_DIM_MISMATCH` |
| `context-graph-mcp` | `CCREALITY_ENGINE_RETIRED` (retired tool callers fail closed) |

Missing CFs/files/models/env vars/malformed inputs MUST return a structured error with `.code()`. No silent defaults.

---

## 9. Major subsystems

**MCP server (`context-graph-mcp`).** JSON-RPC 2.0 stdio (default) or HTTP. 216 tools (69 `mejepa_*` product surface + learning, UTL, constellation, entity, embedder, graph, sequence, file watcher, training, provenance, daemon). Retired `reality_*` tools fail closed with `CCREALITY_ENGINE_RETIRED`. Enforces multi-agent isolation via path-policy + edit-lock.

**Predictor (`context-graph-mejepa`).** CUDA-only production. 73 modules: claim graph (Q1), patch-similarity HNSW exemplars (Q3), Q5 shift-log replay impact, multi-head VICReg + Huber loss, conformal calibration, OOD detection, project-ingest reality compiler, synthetic-stress evaluator, adversarial corpus, dynamic embedder registration, foundationality (PageRank), operator override. `compile()` + `compile_with_fingerprint_observation()` are the canonical Q1-Q5 entry points.

**Frozen instruments (`context-graph-mejepa-instruments`).** `Panel` is an array of 15 per-embedder vectors: `E_AST (384)`, `E_CFG (256)`, `E_DataFlow (256)`, `E_TypeGraph (256)`, `E_Test (384)`, `E_Trace (512)`, `E_Diff (256)`, `E_Witness (256)`, `E_Oracle (128)`, `E_Problem (1024)`, `E_CommitMsg (384)`, `E_StaticAnalysis (256)`, `E_Runtime (256)`, `E_Reasoning (384)`, `Scalars (128)`. 5,120 f32 contiguous buffer is byte layout only. Cross-embedder dim-wise compare forbidden. Instruments carry zero trainable params (`ΔΞ.target_collapse = 0`).

**Embedders (`context-graph-mejepa-embedders`).** 21 slots; 12 active content (8 pretrained + 4 algorithmic). Frozen-by-design: E1, E6, E7, E8, E10, E12, E13, E14. Dynamic-registration with VRAM budget + falsification + freeze + adversarial-injection mitigation. SHA-256 manifest pins. CUDA-required.

**Corpus generator (`context-graph-mejepa-corpus`).** 8 mutation × 11 languages = 88 generic operators (Python first-class via parser-backed; other 10 deferred). Categories: `KnownGood`/`SubtleFlip`/`OffByOne`/`SwapVariable`/`DeleteTestCall`/`WrongFile`/`OverEngineer`/`CompileError`. SWE-bench Lite bridge wraps `python -m swebench.harness.run_evaluation` per-instance. Split modes: `InstanceAtomic` (default, 240/30/30) + `RepoAtomic`.

**Trainer (`context-graph-mejepa-train`).** UTML v1.0. `compute_l_step` evaluates `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. EWC++ Fisher, continual backprop (Dohare 2024), prioritized replay. The public `Trainer::run_full_training` is currently certificate-only; unifying it with the real checkpoint/export path is tracked by #683. Mistake-loop consumer (#406/#523) ships storage/MCP plus exact-panel online-head replay.

**TCT (`context-graph-mejepa-tct`).** Teleological constellations + Gτ guard. Cell key = `(mutation_category, language, additional_dims)`. Per-cell calibration, freshness audit, refresh scheduler. Gτ rejects predictions with insufficient cell support.

**Hygiene (`context-graph-mejepa-hygiene`).** Tier transitions (hot→warm→frozen), quota enforcement, GC, witness-chain Merkle compression. MCP: `mejepa_quota_status`, `mejepa_gc_run`, `mejepa_witness_compress`.

**Shift subscriber.** Tokio task tailing `/zfs/archive/contextgraph/runtime/shift-log.jsonl`. Decodes `ShiftEntry` → `shift_to_inference()` → materializes `Panel` → writes `CF_MEJEPA_PANELS` + `CF_MEJEPA_SHIFT_WATERMARK`. The watermark is the durability gate.

**Storage (`context-graph-storage`).** RocksDB wrapper. ~200 CFs (51 base + ~120 ME-JEPA + 19 teleological + 13 quantized + 5 code + 2 causal). HNSW per embedder; inverted index for sparse embedders.

**Witness (`context-graph-witness`).** Tamper-evident chain. ed25519 segment signing, Merkle aggregation, Stage 0 + Stage F + Merkle certificates with full metadata binding.

---

## 9.5 In-slot causal embedder replacement (2026-05-24 / #745 TASK-CR-007)

Operator-directed architectural shortcut: instead of adding new slots for causal embedders (which would require schema change + #733 incremental forward cache as hard prerequisite), **replace the encoders behind already-empty / proven-low-signal slots** with causal embedders.

| Replacement-candidate slot | Current state | Proposed replacement | Training corpus (aiwonder-only) |
|---|---|---|---|
| **E5** (768-d causal asymmetric, retired 2026-05-10) | empty | **C_TemporalCommit** | CommitPack 4TB |
| **E11** (768-d entity KGE, disabled) | empty | **C_CommonsenseCausal** | CauseNet 11M + ConceptNet + ATOMIC |
| **E14** (configurable, "reserved") | active but utilization-audit pending | **C_BugFix** | BugsInPy + CommitPackFT + SWE-bench Lite |

Slot identity preserved → doctrine §1.6 (formerly §1.5ter) intact. Only the per-slot forward cache re-bakes (~6-8h GPU per slot on aiwonder RTX 5090). Reverts cleanly via `models_config.toml` model_sha rollback if FSV fails.

**Pre-requisites:** #744 (auto-labeler MVP must validate extension to causal direction) + #746 (low-signal audit must confirm replaceability) + #715 (v2 9-slot static cache) + #516/#518/#680/#732 gates.

**Aiwonder-only execution:** every training run / forward cache re-bake / FSV under `/zfs/archive/contextgraph/...`; no `/mnt/d` / `/mnt/c`; per `CLAUDE.md §6.7`.

Full architectural detail in `05 §2.1`; roadmap detail in `08 §6.95`.

---

## 10. What this system is NOT

- Not a generative inference engine — no inner LLM. Product is a verifier, not a code-writer.
- Not a benchmark harness — pre-pivot benchmark crate retired.
- Not a hosted service — runs locally on operator hardware (aiwonder for production).
- Not a Python codebase — Python only shows up as corpus subject (SWE-bench Lite) + tool wrappers.
- Not cross-language at ship-gate time — current gate is Python-only (300×8).
- Not HTTP-callable from action paths.

---

## Cross-references

| Topic | Doc |
|---|---|
| Per-crate source map + dependency graph | `02_source_map.md` |
| Every RocksDB CF with key/value format | `03_storage_schema.md` |
| Every MCP tool with parameters and returns | `04_mcp_tools.md` |
| Embedder slots / instruments / heads / TCT / corpus / heal / mistake loop / training | `05_predictor_pipeline.md` |
| FSV protocol, runbook, DR, observability, test suite | `06_operations_and_fsv.md` |
| Current measurement, shipped milestones, FSV catalog | `07_results_and_benchmarks.md` |
| Current open work, critical path, post-ship plans | `08_roadmap_and_open_work.md` |
| Doctrine (universal protocol) | `docs2/AICodingAgentSuperPrompt.md` |
| Project addendum (invariants, deny-list) | `CLAUDE.md` |
| Per-turn agent contract | `AGENTS.md` |
