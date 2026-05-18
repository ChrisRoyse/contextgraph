# 01 — System Overview

**Last refreshed:** 2026-05-18. **Status:** Phase G in flight (ship gate `prediction_oracle_correlation ≥ 0.95 × 4 rolling windows` still open; latest holdout 0.866667). Phase A–F + H code/FSV-shipped.

Source files covered: `Cargo.toml`, `CLAUDE.md`, `AGENTS.md`, `crates/context-graph-mcp/src/lib.rs`, `crates/context-graph-mejepa/src/lib.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`, `crates/context-graph-mejepa-instruments/src/lib.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs`, `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-mcp/src/tools/definitions/mod.rs`.

---

## 1. What This System Is

**ME-JEPA-Code** is a deterministic predictive truth system. AI coding agents (Claude Code / Codex / Cursor) call it via a local MCP JSON-RPC server. It returns a calibrated multi-head `RealityPrediction` answering five operator questions for any candidate code change:

| Q | Question | Answered by |
|---|---|---|
| **Q1** | Does what the AI **CLAIMED** exist? | Claim graph reconciled against bytes-on-disk + AST (`crates/context-graph-mejepa/src/claim_graph.rs`, `CF_MEJEPA_CLAIM_RECONCILIATION`) |
| **Q2** | Does it **WORK**? | Predictor + split-conformal interval + TCT Gτ guard → `Verdict::{Pass, Fail, OutOfDistribution, Abstain, GuardRejected}` (`compiler.rs`, `verdict_assembly.rs`) |
| **Q3** | **WHY** would it fail? | Ranked `predicted_failure_modes`, `closest_exemplars` from patch-similarity HNSW, violating embedders (`failure_fingerprint.rs`, `patch_similarity.rs`) |
| **Q4** | **WHAT ELSE** could go wrong? | Always-on heads: security (Bandit/Semgrep), perf, cost, accuracy, edge cases, complexity, churn, doc-coverage, dep-CVE (`q4_security_labels.rs`, `composite_reward_signals.rs`) |
| **Q5** | How is it **IMPACTING REALITY**? | Shift-log replay against predicted side effects (`reality_impact.rs`) |

**Ship-gate predicate:** `prediction_oracle_correlation ≥ 0.95` stable across `(mutation_category × language)` on four consecutive rolling windows of the 300×8 SWE-bench Lite Python corpus.

**Product boundary.** LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, and docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`.

**Consequence-prediction contract (#397).** In this project, "predicting reality" means predicting the consequences of code being live: whether the change exists, whether it works, why it would fail, what secondary risks it creates, and how it changes durable repository/test/runtime/dependency reality over time. ME-JEPA presents those predicted consequences, confidence, and evidence to the human operator or the operator's AI agent so they can decide whether to proceed, repair, or choose a different route. The model observes explicit task text, agent claims, edits, AST deltas, frozen instruments, oracle outcomes, and verification feedback; it must not infer or classify hidden human intent (#358).

---

## 2. Process / Port / Tech Architecture

| Process | Port / IPC | Technology | Purpose |
|---|---|---|---|
| `context-graph-mcp` server (`--mode stdio` or `--mode reality-loop`) | stdio JSON-RPC (default) or HTTP transport | Rust, tokio, async-trait | 196-tool MCP surface; daemon mode supervises long-lived state |
| `context-graph-cli` | CLI invocation | Rust | Operator commands: `mejepa observe`, `mejepa project-ingest`, `mejepa compression-progress`, `mejepa active-learning`, etc. |
| ME-JEPA training daemon | embedded in CLI/binary | candle (CUDA 13.2), RocksDB | UTML training loop; reads `CF_MEJEPA_REPLAY_BUFFER`; writes `CF_MEJEPA_TRAIN_CERTS` |
| Shift-log subscriber | tokio task | tokio, async streams | Tails JSONL shift log → materializes panels (`crates/context-graph-mejepa-shift-subscriber`) |
| Heal scheduler | embedded thread | tokio | Drift detection, Fisher/EWC, distill, promotion (`crates/context-graph-mejepa/src/heal/`) |
| Single RocksDB (per runtime root) | local disk | RocksDB ≥ 8.x | 200+ column families (51 base + ~120 ME-JEPA + 19 teleological + 13 quantized + 5 code + 2 causal). See `03_storage_schema.md`. |
| GPU runtime | CUDA 13.2 on RTX 5090 Blackwell | cudarc 0.19.x via candle 0.10.2 | Embedder forward passes, predictor training |

**No HTTP from action paths.** `WebSearch`/`WebFetch`/`mcp__exa__*` are read-only research tools. The MCP server is the only network surface for runtime work.

**No inner LLM.** Local generative inference (Qwen / Hermes / Ollama / llama.cpp) was retired 2026-05-09. `feature = "llm"` triggers `compile_error!` (`crates/context-graph-mcp/src/lib.rs:13`).

**No intent synthesis.** North-star / intent modeling is retired by ADR #358. Observable goals and agent claims may be stored as evidence in the reality chain; hidden human intention is not a prediction target.

---

## 3. Technology Stack

| Layer | Technology | Version constraint | Where pinned |
|---|---|---|---|
| Language | Rust | edition 2021, MSRV 1.95 | `Cargo.toml:64-66` |
| Async runtime | tokio | 1.35 (`features = ["full"]`) | `Cargo.toml:72` |
| Serialization | serde, serde_json, bincode 1.3, toml 0.8 | — | `Cargo.toml:76-79` |
| Error | thiserror, anyhow | — | `Cargo.toml:82-83` |
| Telemetry | tracing, tracing-subscriber (json/env-filter) | — | `Cargo.toml:86-87` |
| Config | `config` crate | 0.14 | `Cargo.toml:90` |
| IDs / time | uuid (v4/v5/serde), chrono (serde) | uuid 1.6, chrono 0.4 | `Cargo.toml:93-94` |
| ML runtime | candle-core, candle-nn (CUDA) | 0.10.2 (cudarc 0.19.x, CUDA 13.2) | `Cargo.toml:101-104` |
| Storage | RocksDB | via `rocksdb` crate (see per-crate manifests) | `crates/context-graph-storage/Cargo.toml` |
| Crypto / hashing | ed25519, sha2, blake3 | — | `crates/context-graph-witness/Cargo.toml` |
| Vector search | usearch HNSW (patch-similarity), hnsw_rs (embedder indexes) | — | per-crate |
| Build profiles | `release` (thin LTO, 16 CGUs), `release-max` (1 CGU), `dev` (opt 0, line-info debug, incremental) | — | `Cargo.toml:112-128` |
| License | PolyForm-Noncommercial-1.0.0 | — | `Cargo.toml:67` |

---

## 4. Workspace Members (25 active default + 1 excluded)

`Cargo.toml:6-32` lists all members; `default-members` (lines 36-61) excludes `context-graph-benchmark` (87K lines, build explicitly with `-p`). Full per-crate detail in `02_source_map.md`.

| Crate | Role | LOC (src lib.rs only) |
|---|---|---|
| `context-graph-mcp` | MCP JSON-RPC server + 196 tools | 29 (lib) + 136K (main.rs) |
| `context-graph-cli` | Operator CLI, helpers | 14 (lib) + 230 (main.rs) |
| `context-graph-core` | Shared domain types | — |
| `context-graph-storage` | RocksDB wrapper, teleological/code/causal CFs | 190 |
| `context-graph-witness` | Tamper-evident chain primitives (ed25519, Merkle) | 225 |
| `context-graph-solver` | Causal masking, MMR, conformal predictor | 31 |
| `context-graph-mincut` | Clean-room Stoer-Wagner global min-cut | 333 |
| `context-graph-paths` | Fail-closed durable path resolver | 614 |
| `context-graph-tier-compression` | 8/7/5/3-bit groupwise scalar quantization | 516 |
| `context-graph-embeddings` | E1-E14 embedding pipeline | 135 |
| `context-graph-cuda` | CUDA helpers | — |
| `context-graph-mejepa` | **Predictor trunk** (compiler, conformal, OOD, claim graph, patch similarity, project-ingest, ...) | 175 (lib); 73 module files |
| `context-graph-mejepa-cf` | Canonical column-family name registry (zero-dep) | 819 |
| `context-graph-mejepa-corpus` | 8× mutation operators + SWE-bench bridge | 822 |
| `context-graph-mejepa-instruments` | 15-slot typed panel storage layout + frozen instruments | 595 |
| `context-graph-mejepa-embedders` | 21 embedder slots, dynamic registry, calibration | 63 |
| `context-graph-mejepa-train` | UTML trainer, replay buffer, certificates | 51 |
| `context-graph-mejepa-tct` | Teleological constellations + Gτ guard | 49 |
| `context-graph-mejepa-hygiene` | Tier transitions, quota, GC, witness compress | 37 |
| `context-graph-mejepa-shift-subscriber` | JSONL shift-log tail → panel materialization | 26 |
| `context-graph-dynamicjepa` | 5090jepa MCP surface (39 tools) | — |
| `context-graph-causal-agent` | Deterministic causal extraction | — |
| `context-graph-graph-agent` | Graph extraction / discovery | 66 |
| `context-graph-test-utils` | Shared test fixtures (NO MOCK DATA) | 23 |
| `context-graph-benchmark` *(excluded from default build)* | 87K-line benchmarking suite | — |

---

## 5. Entry Points

| Binary / command | Crate | Purpose |
|---|---|---|
| `context-graph-mcp` | `context-graph-mcp` | MCP server. `--mode stdio` (default) or `--mode reality-loop` (lightweight subset). Multi-agent observability via `mejepa_daemon_status`. |
| `context-graph-cli` | `context-graph-cli` | Operator CLI. Subcommands include `mejepa observe`, `mejepa project-ingest`, `mejepa project-report`, `mejepa compression-progress`, `mejepa active-learning {list|label|dismiss}`, `mejepa stop-self-verify`, `mejepa eval synthetic-stress`. |
| `mejepa` | `context-graph-mejepa` `[[bin]]` | Standalone predictor harness. |
| `mejepa-heal-drill` | `context-graph-mejepa` `[[bin]]` | Heal scheduler drill. |
| `mejepa-train` | `context-graph-mejepa-train` `[[bin]]` | Trainer entry. |
| `train-causal` | `context-graph-embeddings` `[[bin]]` | Causal embedder training. |
| 50+ FSV examples (`crates/*/examples/*_fsv.rs`) | per crate | Per-task FSV harnesses. See `07_results_and_benchmarks.md` for the full index. |

---

## 6. Directory Layout (what exists at runtime)

```
crates/                                — Rust workspace (1567 .rs src files, 189 example files, 258 test files)
docs/futurebuild/                       — this doc suite (8 condensed docs as of 2026-05-18)
docs/futurebuild/_archived/             — original 31 planning docs (read-only history)
docs/futurebuild/specs/FSV-PROTOCOL.md  — mandatory verification contract (still authoritative)
docs2/AICodingAgentSuperPrompt.md       — master doctrine (universal protocol)
docs2/hardeningprompt.md                — 14-axis hardening deep-dive
scripts/cc/                             — Claude Code hook scripts (PostToolUse, Stop, UserPromptSubmit)
scripts/ci/                             — CI lint (e.g. check_no_c_drive_paths.sh)
scripts/swebench_lite_ops/              — Python redaction + SWE-bench Lite helpers
tasks/swebench-lite/                    — task manifest JSON (workstreams)
config/                                 — governed paths, model pins
.claude/                                — settings.json, agents/, loop.md (1 line; see specs)
.github/workflows/, .github/labels.yml  — CI + label set (memory = Issues)
memory.archive/                         — read-only legacy SAMP/SMAP (526 files, do not write)

/zfs/archive/contextgraph/              — **aiwonder production durable SoT** (see 03_storage_schema.md §5)
  corpus/                               —   production corpus, panel cache, fingerprint registry
  fsv/                                  —   per-task FSV evidence (post-migration target)
  runtime/                              —   shift log, test outcomes, scheduler events
  projects/<project_id>/                —   reality-compiler per-project cache, manifest, report
  state/                                —   durable runtime state surviving restarts
  exports/                              —   weekly eval reports, operator exports

/zfs/hot/contextgraph/                  — **aiwonder hot state**
  repo/                                 —   git checkout, release binaries
  models/                               —   embedder model weights
  corpus/                               —   stable hot inputs
  runtime/                              —   active scheduler logs / hot run state

/mnt/d/contextgraph/                    — **legacy/local SoT** (327 FSV dirs as of 2026-05-18)
                                          historical for production unless an issue explicitly
                                          scopes a local-only smoke (#369 tracks revalidation)
```

---

## 7. Storage Tier Classification

| Tier | What | Where | Regenerable? |
|---|---|---|---|
| **Sacred** (never edit) | Operator `.env`/`.env.*`; `swebench/**`; `tests/oracle_**/**`; `crates/*/src/reward_specs/**`; `crates/*/src/verifiers/**`; `crates/context-graph-benchmark/datasets/swe_bench_*/**`; `*.pem`, `*.key`, `**/credentials.json` | various | No |
| **Durable production** | RocksDB CFs (51+ teleological/quantized/code/causal + 120+ ME-JEPA); FSV evidence; project-ingest manifests; weekly exports | `/zfs/archive/contextgraph/` on aiwonder | Source-recoverable (corpus) or replay-from-shift-log (predictions). Many FSV artifacts are irreplaceable single events. |
| **Hot** | Active checkpoints, model weights, hot run state | `/zfs/hot/contextgraph/` on aiwonder | Yes (re-download / rebuild) |
| **Ephemeral** | `/tmp/contextgraph-*` scratch | `/tmp/` | Yes |
| **Legacy/local** | Pre-migration FSV + corpus + shift-log | `/mnt/d/contextgraph/` | Frozen as historical evidence; #369 reclassifies |

---

## 8. Exception Hierarchy (fail-closed structured errors)

Every crate defines its own `Error` enum with `SCREAMING_SNAKE_CASE` code via `.code()`. Truncated catalog:

| Crate | Top-level error type | Sample error codes | Where defined |
|---|---|---|---|
| `context-graph-mejepa-instruments` | `InstrumentError` (`InvalidInput`, `NumericalInvariant`, `FrozenViolation`, `Store`) | `MEJEPA_INSTRUMENTS_INVALID_INPUT`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`, `MEJEPA_INSTR_FROZEN_VIOLATION`, `MEJEPA_INSTRUMENTS_STORE_ERROR` | `crates/.../src/lib.rs:62-105` |
| `context-graph-mejepa-corpus` | `MutationError` (`InvalidInput`, `NoMutationSite`, `OperatorFailed`, `LeakageDetected`, `SubprocessTimeout`) | `MEJEPA_CORPUS_INVALID_INPUT`, `MEJEPA_CORPUS_NO_MUTATION_SITE`, `MEJEPA_CORPUS_OPERATOR_FAILED`, `MEJEPA_CORPUS_LEAKAGE_DETECTED`, `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT` | `crates/.../src/lib.rs:111-160` |
| `context-graph-mejepa` | `PredictorError`, `MejepaInferError`, `LossError`, `NanSource` | (see `crates/context-graph-mejepa/src/error.rs`) | `error.rs` |
| `context-graph-mejepa-train` | `TrainerError`, `TrainerErrorCode`, `UtmlError`, `UtmlErrorCode` | (see `error.rs`) | `crates/.../src/error.rs` |
| `context-graph-mejepa-tct` | `Tct*Error` | constellation freshness, calibration cell support, Gτ guard | `error.rs` |
| `context-graph-mejepa-hygiene` | hygiene `Error` | tier transitions, quota overflow, GC | `error.rs` |
| `context-graph-mejepa-shift-subscriber` | `SubscriberError` | watermark, decode, tail | `error.rs` |
| `context-graph-storage` | `StorageError`, `QuantizedStorageError`, `TeleologicalStoreError`, `CodeStorageError`, `GraphEdgeStorageError` | open / CF missing / decode failure | `error.rs` |
| `context-graph-mcp` (server/handlers) | retired tools → `CCREALITY_ENGINE_RETIRED` fail-closed | — | `deprecation.rs` |

**Fail-closed contract.** Missing CFs, missing files, missing models, missing env vars, malformed inputs MUST return a structured error with `code()`. No silent defaults; no permissive fallbacks. Doctrine § 1 / FSV-PROTOCOL § 3.

---

## 9. Major Subsystems (one paragraph each)

**MCP server (`context-graph-mcp`).** JSON-RPC 2.0 stdio (default) or HTTP transport. 196 tool definitions exported by `tools/definitions/mod.rs::get_tool_definitions()`: 49 are `mejepa_*` (Phase 4/5/6/8/F/G/EK predictor + heal + observe + ship-gate dashboard). Also: 39 `dynamicjepa_*` (5090jepa surface), 14 learning, 18 UTL learner-state, 6 constellation, 6 entity, 10 embedder, 5 core memory, 4 each for topic/file_watcher/sequence/causal/graph_link/contrastive, 3 each for typed_edges/provenance, plus singletons. Retired `reality_*` tools fail closed with `CCREALITY_ENGINE_RETIRED`. Reality-loop mode (`--mode reality-loop`) exposes the post-pivot Claude-Code-outer-loop subset only. Server enforces multi-agent isolation via path-policy + edit-lock (see `06_operations_and_fsv.md`).

**Predictor (`context-graph-mejepa`).** CUDA-only production predictor. 73 modules covering claim graph (Q1), patch-similarity HNSW exemplars (Q3), Q4 security/perf/cost/accuracy heads, Q5 shift-log replay impact, multi-head VICReg + Huber loss, conformal calibration, OOD detection, claim-vs-reality reconciliation, project-ingest reality compiler, synthetic-stress evaluator, adversarial corpus, dynamic embedder registration, foundationality (PageRank on chunk/library/embedder dep graphs), label transfer audit, operator override surface. `compile()` and `compile_with_fingerprint_observation()` are the canonical Q1-Q5 entry points.

**Frozen instruments (`context-graph-mejepa-instruments`).** 15-slot typed panel storage layout: `E_AST (384)`, `E_CFG (256)`, `E_DataFlow (256)`, `E_TypeGraph (256)`, `E_Test (384)`, `E_Trace (512)`, `E_Diff (256)`, `E_Witness (256)`, `E_Oracle (128)`, `E_Problem (1024)`, `E_CommitMsg (384)`, `E_StaticAnalysis (256)`, `E_Runtime (256)`, `E_Reasoning (384)`, `Scalars (128)`. These slots are independent evidence surfaces inside a Teleological Constellation, not one fused semantic embedding. `Panel`/`PanelBuilder` enforce fail-closed dim+finite+double-fill validation. Instrument trait carries zero trainable params (structural collapse-immunity invariant `ΔΞ.target_collapse = 0`).

**Embedders (`context-graph-mejepa-embedders`).** 21 slots (E5 retired). 12 active content embedders (8 pretrained + 4 algorithmic). Frozen-by-design weights: E1, E6, E7, E8, E10, E12, E13, E14. Dynamic-registration scaffolding (`dynamic_registry.rs`) + VRAM budget enforcement (`vram.rs`) + falsification gate + freeze audit + adversarial-injection mitigation. SHA-256 manifest pins for every pretrained weight. CUDA-required for production; CPU construction rejected.

**Corpus generator (`context-graph-mejepa-corpus`).** 8 mutation categories × 11 languages = 88 generic operators (Python uses the parser-backed operators; other 10 languages use `generic_language_ops`). Deterministic SplitMix64 PRNG. Categories: `KnownGood`/`SubtleFlip`/`OffByOne`/`SwapVariable`/`DeleteTestCall`/`WrongFile`/`OverEngineer`/`CompileError`. SWE-bench Lite bridge (`swebench.rs`) wraps `python -m swebench.harness.run_evaluation` per-instance, parses `report.json`, converts `FAIL_TO_PASS`/`PASS_TO_PASS`/`FAIL_TO_FAIL`/`PASS_TO_FAIL` → `OracleVerdict`. Split modes: `InstanceAtomic` (default, 240/30/30 on Lite) and `RepoAtomic` (195/77/28, repo-bounded). Provenance via SHA256 hash of sorted `(task_id, category, patch_sha256, oracle_verdict_sha256)` rows.

**Trainer (`context-graph-mejepa-train`).** UTML v1.0 implementation. `compute_l_step` evaluates `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. EWC++ (`task-py-g-065`), continual backprop (Dohare 2024, `task-py-g-066`), prioritized replay buffer over `CF_MEJEPA_REPLAY_BUFFER` keyed by surprise + per-cell coverage gap, inverse-map head (TASK-EK-012), per-fingerprint Fisher / dormancy (TASK-FP-008), compression-progress reporter (TASK-EK-001) producing CDL_bits weekly. Outputs `TrainingCertificate` rows in `CF_MEJEPA_TRAIN_CERTS`.

**TCT (`context-graph-mejepa-tct`).** Teleological constellations + Gτ guard. Cell key = `(mutation_category, language, additional_dims)`. Per-cell calibration, freshness audit (`freshness_policy.rs`), constellation refresh scheduler (`CF_MEJEPA_CONSTELLATION_REFRESH_LOG`). `Gτ` rejects predictions with insufficient cell support. Refresh-report renderer emits human-readable triage.

**Hygiene (`context-graph-mejepa-hygiene`).** Tier transitions (hot → warm → frozen), quota enforcement, GC reports, witness-chain Merkle compression. MCP surfaces: `mejepa_quota_status`, `mejepa_gc_run`, `mejepa_witness_compress`.

**Shift subscriber (`context-graph-mejepa-shift-subscriber`).** Tokio task tailing JSONL shift log (`/zfs/archive/contextgraph/runtime/shift-log.jsonl` in production, `/mnt/d/...` historical). Decodes `ShiftEntry` → calls `shift_to_inference()` → materializes `Panel` → writes `CF_MEJEPA_PANELS` + `CF_MEJEPA_SHIFT_WATERMARK`. Watermark write is the durability gate.

**Storage (`context-graph-storage`).** RocksDB wrapper. 19 teleological CFs (fingerprints, e1_matryoshka_128, e12_late_interaction, e13_splade_inverted, e6_sparse_inverted, content, source_metadata, file_index, topic_portfolio, entity_provenance, audit_log, audit_by_target, merge_history, importance_history, tool_call_index, consolidation_recommendations, embedding_registry, custom_weight_profiles, topic_profiles). 13 quantized embedder CFs (`emb_0..emb_12`). 5 code CFs (entities, e7 embeddings, file/name/signature indexes). 2 causal CFs (relationships, by_source). Per-CF `Options` with column-specific compaction. HNSW configs per embedder; inverted index for sparse embedders.

**CLI (`context-graph-cli`).** Operator CLI. Helpers in `governed_edit.rs` (path-policy edits), `mcp_client.rs` (call MCP from CLI scripts), `mcp_helpers.rs`. `exit_code_for_error()` maps known structured errors to deterministic exit codes for CI/scripts; `is_corruption_indicator()` triggers harder failure modes.

**Witness (`context-graph-witness`).** Tamper-evident chain primitives. ed25519 segment signing (feature `ed25519`), Merkle aggregation, Stage 0 + Stage F + Merkle certificates with full metadata binding (corpus_id, policy_version, query_id, head_id, latency_ms, trajectory_count).

---

## 10. What This System Is NOT

- Not a generative inference engine — no inner LLM. The product is the *verifier*, not a code-writer.
- Not a benchmark harness — `context-graph-benchmark` exists separately and is intentionally not in the default build.
- Not a hosted service — runs locally on operator hardware (aiwonder for production durable state).
- Not a Python codebase — Python only shows up as the *corpus subject* (SWE-bench Lite) and in tool wrappers (`scripts/swebench_lite_ops/*`).
- Not cross-language at ship-gate time — current ship gate is Python-only (300×8 Python corpus). Non-Python `Language::*` variants exist in the corpus generator but are deferred from the current gate.
- Not HTTP-callable from action paths — the MCP server is the only network surface for runtime work.

---

## Cross-references

- Per-crate source map + dependency graph → `02_source_map.md`
- Every RocksDB column family with key/value format → `03_storage_schema.md`
- Every MCP tool with parameters and returns → `04_mcp_tools.md`
- Embedder slots / instruments / heads / TCT / corpus / heal / training → `05_predictor_pipeline.md`
- FSV protocol, runbook, DR, observability, test suite → `06_operations_and_fsv.md`
- FSV evidence history, ship-gate progress, ablations → `07_results_and_benchmarks.md`
- Current open work, Phase G/I/J, deferred items → `08_roadmap_and_open_work.md`
- Doctrine (universal protocol) → `docs2/AICodingAgentSuperPrompt.md`
- Project addendum (invariants, deny-list) → `CLAUDE.md`
- Per-turn agent contract → `AGENTS.md`
