# Appendix A — Architecture Diagrams and Component Tables

Companion to [`whitepaper.md`](whitepaper.md). This appendix collects the diagrams and tables that would otherwise crowd the main paper's narrative.

---

## A.1 System-Level Topology

```
+-----------------------------------------------------------------------+
|                        Operator surface                                |
|                                                                       |
|   /ccreality-iterate    /ccreality-smoke    /ccreality-trigger-check   |
|   /ccreality-readback   reality-loop CLI    blackbox CLI               |
+-------------------------------+---------------------------------------+
                                |
                                v
+-----------------------------------------------------------------------+
|              context-graph-mcp (JSON-RPC 2.0 server)                   |
|                                                                       |
|   141/145 MCP tools across 27 categories.                              |
|   Single-writer flock at <storage.path>/mcp.pid.                       |
|   Daemon mode: stdio-to-TCP relay.                                     |
+----+-----------+-----------+----------+--------------+----------------+
     |           |           |          |              |
     v           v           v          v              v
+--------+  +--------+  +--------+  +--------+   +--------+
| Memory |  | Causal |  | Graph  |  | Learner|   | Dynamic|
| store  |  | LLM    |  | LLM    |  | UTL    |   | JEPA   |
| (E1-E14|  |(Hermes |  |(Hermes |  | (E15-  |   |(28 CFs)|
| dense+ |  | 2 Pro  |  | 2 Pro  |  |  E21)  |   |        |
| sparse)|  | shared |  | shared |  |        |   |        |
+---+----+  +---+----+  +---+----+  +---+----+   +---+----+
    |           |           |           |            |
    +-----------+-----------+-----------+------------+
                            |
                            v
                +-----------------------+
                |      RocksDB DB       |
                |   101 column families  |
                |   shared 256MB cache   |
                +-----------------------+

+-----------------------------------------------------------------------+
|  Reality Loop (separate Rust binary; does not load MCP server)         |
|                                                                       |
|  reality-loop.rs (2,385 LOC, tokio::main)                              |
|     - Validates manifest against measurement-contract-v1               |
|     - Drives local Ollama via /api/chat                                |
|     - Applies patch with `git apply`                                   |
|     - Calls official Docker SWE-bench evaluator                        |
|     - Persists to BlackBoxFix SQLite (LEDGER_SCHEMA_VERSION=14) +      |
|       reality-events.jsonl                                             |
+-----------------------------------------------------------------------+
```

The MCP server and the Reality Loop are *separate processes*. They communicate solely through:

1. The shared filesystem (RocksDB, BlackBoxFix SQLite, runtime root, models directory).
2. The `target/debug/context-graph blackbox` CLI subcommand (invoked by the loop to write ledger rows).
3. The `mcp__cgreality__*` MCP tools (used by the outer Claude Code optimizer when running interactively).

This is intentional: the Reality Loop must be runnable with the MCP server *down*. Failures in the optimizer surface must not block measurement runs.

---

## A.2 Reality Loop Single-Attempt Flow

```
                       ┌─────────────────────────────┐
                       │  CLI args + env-clean check │
                       └──────────────┬──────────────┘
                                      │
                                      v
                       ┌─────────────────────────────┐
                       │  Validate task manifest      │  --> RUST_REALITY_TASK_*
                       │  against contract-v1         │      (dataset, instance,
                       └──────────────┬──────────────┘      repo, base_commit, ...)
                                      │
                                      v
                       ┌─────────────────────────────┐
                       │  Write measurement-contract │  --> JSON readback verify
                       │  .json (10 mandatory        │
                       │  readbacks declared)         │
                       └──────────────┬──────────────┘
                                      │
                                      v
              ┌──────────────────────────────────────────┐
              │  per-attempt loop (max_attempts | until_resolved)
              │
              │      ┌─────────────────────────┐
              │      │ nvidia-smi VRAM check    │ --> RUST_REALITY_GPU_*
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ git clone/reset/clean    │ --> RUST_REALITY_BASE_COMMIT_*
              │      │ at exact base_commit     │     RUST_REALITY_WORKSPACE_*
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Build repo context       │
              │      │ (git ls-files + grep)    │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Compile prompt           │ --> 8 prompt-quality gates
              │      │ (deterministic, hashed)  │     hidden_solution_leak_absent
              │      └─────────────┬───────────┘     (hard gate)
              │                    │
              │      ┌─────────────────────────┐
              │      │ Ollama /api/chat call    │ --> RUST_REALITY_MODEL_*
              │      │ + SHA-256 of req/resp    │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Parse JSON response →    │ --> RUST_REALITY_MODEL_PATCH_MISSING
              │      │ {patch, notes}           │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Compare against          │ --> RUST_REALITY_PATCH_REPEATED
              │      │ NO_REPEAT_PATCH_SHA256   │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ git apply --whitespace=  │ --> RUST_REALITY_PATCH_APPLY_FAILED
              │      │ nowarn                   │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Forbidden-paths scan     │ --> RUST_REALITY_PATCH_TOUCHES_BENCHMARK
              │      │ (test/fixture/harness)   │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Docker SDK preflight     │ --> RUST_REALITY_OFFICIAL_DOCKER_*
              │      │ (python -c "import       │
              │      │ docker; ...")            │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ python -m swebench       │ --> RUST_REALITY_OFFICIAL_EVAL_FAILED
              │      │ .harness.run_evaluation  │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Read official report;    │ --> RUST_REALITY_OFFICIAL_REPORT_*
              │      │ classify resolved_ids    │
              │      └─────────────┬───────────┘
              │                    │
              │      ┌─────────────────────────┐
              │      │ Persist BlackBoxFix      │ --> JSON readback verify
              │      │ score + reward rows      │     event-log readback verify
              │      └─────────────┬───────────┘
              │                    │
              │            (loop or exit)
              └──────────────────────────────────────────┘
                                      │
                                      v
                       ┌─────────────────────────────┐
                       │ stdout/stderr summary       │
                       │   RUST_REALITY_LOOP_SUMMARY=│
                       │   RUST_REALITY_LOOP_ERROR_  │
                       │   SOURCE_OF_TRUTH=          │
                       └─────────────────────────────┘
```

---

## A.3 ME-JEPA Architecture

```
                            ┌───────────────────┐
                            │  Domain pack      │
                            │  (counter_world,  │
                            │   gridworld_5x5,  │
                            │   counter_to_grid,│
                            │   career_taxonomy)│
                            └────────┬──────────┘
                                     │
                                     v
                       ┌──────────────────────────────┐
                       │  Raw events (JSONL)          │
                       │  → CF_DJ_RAW_EVENTS          │
                       └──────────────┬───────────────┘
                                      │
                                      v
                       ┌──────────────────────────────┐
                       │  Adapter run                 │
                       │  → CF_DJ_NORMALIZED_STATES   │
                       │  → CF_DJ_ACTIONS             │
                       │  → CF_DJ_OUTCOMES            │
                       │  → CF_DJ_TRANSITIONS         │
                       │  → CF_DJ_INSTRUMENT_READINGS │
                       └──────────────┬───────────────┘
                                      │
                                      v
                       ┌──────────────────────────────┐
                       │  Materialise latent panel    │
                       │  → CF_DJ_LATENT_PANELS       │
                       │  (large-blob CF; one panel   │
                       │   per (state, action,        │
                       │   outcome, time))            │
                       └──────────────┬───────────────┘
                                      │
                                      v
                       ┌──────────────────────────────┐
                       │  Compile dataset             │
                       │  (one-step rows, train/val/  │
                       │  test split)                 │
                       │  → CF_DJ_DATASET_SHARDS      │
                       └──────────────┬───────────────┘
                                      │
                                      v
              ┌──────────────────────────────────────────────────┐
              │  Phase 7 — train_dataset                          │
              │                                                  │
              │      ┌────────────────────────────────────┐      │
              │      │ Online encoder f_θ (MLP, ReLU)     │      │
              │      └─────────────┬──────────────────────┘      │
              │                    │ z_online                    │
              │                    v                             │
              │      ┌────────────────────────────────────┐      │
              │      │ Predictor g_φ(z_online ⊕ action)   │      │
              │      └─────────────┬──────────────────────┘      │
              │                    │ ẑ                          │
              │                    v                             │
              │   target panel ─▶ Target encoder f_ξ (EMA)       │
              │                    │ z_target (.detach())        │
              │                    v                             │
              │      ┌────────────────────────────────────┐      │
              │      │ Loss = w₁·MSE(ẑ, sg(z_target)) +   │      │
              │      │       w₂·VICReg_var(z_online) +    │      │
              │      │       w₃·VICReg_cov(z_online)      │      │
              │      └─────────────┬──────────────────────┘      │
              │                    │                             │
              │      ┌────────────────────────────────────┐      │
              │      │ AdamW step on f_θ + g_φ params     │      │
              │      │ EMA step on f_ξ ← m·f_ξ + (1-m)·f_θ│      │
              │      │ device.synchronize()               │      │
              │      └─────────────┬──────────────────────┘      │
              │                    │                             │
              │   (loop epochs; check stopping_metric ≤ target)  │
              └────────────────────┬─────────────────────────────┘
                                   │
                                   v
              ┌──────────────────────────────────────────────────┐
              │  Persist artefact                                │
              │  → CF_DJ_TRAINING_RUNS (Started → Completed)     │
              │  → CF_DJ_MODEL_ARTIFACTS                         │
              │  → CF_DJ_AUDIT_LOG                               │
              │  + safetensors files (model.safetensors,         │
              │    config.json, evaluation_report.json)          │
              │  + per-file SHA-256 in ModelArtifactRecord.files │
              └──────────────────────┬───────────────────────────┘
                                     │
                                     v
              ┌──────────────────────────────────────────────────┐
              │  Phase 8 — predict / plan / record_surprise       │
              │                                                  │
              │  predict_next_panel(artifact, panel, action):    │
              │    1. Hash-verify artefact files (refuse on      │
              │       mismatch)                                  │
              │    2. Encode action vector deterministically     │
              │    3. Forward through online encoder + predictor │
              │    4. Persist PredictionRecord with              │
              │       model_artifact_hash_at_inference           │
              │                                                  │
              │  plan_next_action(artifact, panel, skill):       │
              │    1. Enumerate candidates from policy           │
              │    2. Predict each → utility                     │
              │    3. Run guards (bounds_check + G_τ)            │
              │    4. Select highest-utility allowed             │
              │    5. Persist plan trace + candidate predictions │
              │       + guard decisions                          │
              │                                                  │
              │  record_surprise(prediction, observed):          │
              │    1. Re-encode observed panel under same        │
              │       artefact                                   │
              │    2. cosine(predicted, observed)                │
              │    3. If < threshold: persist surprise event     │
              └──────────────────────────────────────────────────┘
```

---

## A.4 Audit Substrate

```
                      ┌──────────────────────────────────┐
                      │       Operator query surface     │
                      │  (mcp__cgreality__reality_*,     │
                      │   blackbox CLI, replay tools)    │
                      └──────────────┬───────────────────┘
                                     │
       ┌─────────────────────────────┼─────────────────────────────┐
       │                             │                             │
       v                             v                             v
+----------------+         +-----------------+         +---------------------+
| BlackBoxFix    |         | reality-events. |         | RocksDB             |
| SQLite ledger  |         | jsonl           |         | (101 CFs)           |
|                |         |                 |         |                     |
| LEDGER_SCHEMA_ |         | append-only;    |         | dj_audit_log        |
| VERSION = 14   |         | byte-equal      |         |   (DJ_AUDIT_RECORD_ |
|                |         | readback per    |         |    VERSION = 2)     |
| Tables:        |         | append          |         | audit_log           |
|   runs         |         |                 |         |   (24-byte composite|
|   models       |         | Per-row keys:   |         |    ts_BE + uuid)    |
|   tools        |         |   loop_id       |         | audit_by_target     |
|   verifiers    |         |   loop_layer    |         | merge_history       |
|   scores       |         |   event_type    |         |   (permanent)       |
|   rewards      |         |   payload_sha256|         | importance_history  |
|                |         |                 |         |   (permanent)       |
+----------------+         +-----------------+         +---------------------+
       │                             │                             │
       └─────────────────────────────┴─────────────────────────────┘
                          all three append-only;
                       SHA-256 content addressing;
                  failed writes propagate as hard errors;
                replay is sufficient to reconstruct state.
```

---

## A.5 Component Inventory

| Subsystem | Crate(s) | Source root | LOC | Persistence | Notes |
|---|---|---|---|---|---|
| MCP server | `context-graph-mcp` | `crates/context-graph-mcp/src/` | 105 .rs files | RocksDB (read/write) | Stdio + TCP transports; daemon mode. |
| Storage | `context-graph-storage` | `crates/context-graph-storage/src/` | 130 .rs files | RocksDB (101 CFs) | 256MB shared block cache; per-CF option profiles. |
| Embedding subsystem | `context-graph-embeddings` | `crates/context-graph-embeddings/src/` | 441 .rs files | RocksDB (`emb_0..emb_13`, `e6_sparse_inverted`, `e12_late_interaction`, `e13_splade_inverted`); HNSW (15 indexes via usearch) | E1-E14 + 7 learner heads (E15-E21). |
| Causal LLM agent | `context-graph-causal-agent` | `crates/context-graph-causal-agent/src/` | 8 .rs files | RocksDB (`causal_relationships`, `causal_by_source`) | Hermes-2-Pro-Mistral-7B Q5_K_M via llama-cpp-2. |
| Graph LLM agent | `context-graph-graph-agent` | `crates/context-graph-graph-agent/src/` | 9 .rs files | RocksDB (`typed_edge_records`, `typed_edge_validations`) | Reuses `CausalDiscoveryLLM` from causal agent. |
| DynamicJEPA / ME-JEPA | `context-graph-dynamicjepa` | `crates/context-graph-dynamicjepa/src/` | 7 .rs files | RocksDB (28 `dj_*` CFs) | Candle 0.10.2 + CUDA; cudaonly. |
| CLI | `context-graph-cli` | `crates/context-graph-cli/src/` | 40 .rs files | n/a | 4 binaries: `context-graph-cli`, `context-graph`, `blackbox`, `reality-loop`. |
| Reality Loop | (binary in CLI crate) | `crates/context-graph-cli/src/bin/reality-loop.rs` | 2,385 LOC | BlackBoxFix SQLite + reality-events.jsonl | Self-contained `tokio::main`; auto-discovered. |
| CUDA kernels | `context-graph-cuda` | `crates/context-graph-cuda/src/` | 18 .rs files | n/a | CUDA driver API + GPU HDBSCAN. |
| Core types | `context-graph-core` | `crates/context-graph-core/src/` | 303 .rs files | n/a | All shared types, fingerprints, fusion math, weight profiles. |

---

## A.6 Reality Loop Lifecycle Constants

| Constant | Value | Source |
|---|---|---|
| `DEFAULT_MODEL` | `"gemma4:e4b"` | `bin/reality-loop.rs:14` |
| `DEFAULT_RUNTIME_ROOT` | `"/tmp/contextgraph-rust-reality-loop"` | `bin/reality-loop.rs:12` |
| `DEFAULT_OLLAMA_URL` | `"http://127.0.0.1:11434"` | `bin/reality-loop.rs:15` |
| `DEFAULT_OLLAMA_KEEP_ALIVE` | `"30m"` | `bin/reality-loop.rs:17` |
| `DEFAULT_OFFICIAL_DOCKER_HOST` | `"unix:///var/run/docker.sock"` | `bin/reality-loop.rs:18` |
| `DEFAULT_OFFICIAL_DOCKER_CONFIG` | `"/home/cabdru/.cache/contextgraph-docker-config"` | `bin/reality-loop.rs:19` |
| `DEFAULT_GPU_MIN_FREE_MB` | `8192` | `bin/reality-loop.rs:16` |
| `DEFAULT_OFFICIAL_TIMEOUT_SECS` | `900` | flag default |
| `DEFAULT_MODEL_TIMEOUT_SECS` | `300` | flag default |
| `DEFAULT_GIT_TIMEOUT_SECS` | `600` | flag default |
| `DATASET_NAME` | `"princeton-nlp/SWE-bench_Lite"` | `bin/reality-loop.rs:20` |
| `MEASUREMENT_CONTRACT_SCHEMA` | `"contextgraph_reality_measurement_contract_v1"` | `bin/reality-loop.rs:750` |
| `LOOP_SUMMARY_SCHEMA` | `"contextgraph_rust_reality_loop_summary_v1"` | `bin/reality-loop.rs:1926` |
| `REALITY_EVENT_SCHEMA_VERSION` | `1` | per-event field |
| `LEDGER_SCHEMA_VERSION` | `14` | `commands/blackbox.rs:40` |

---

## A.7 ME-JEPA Persistence Profile (28 CFs)

| Profile | Used by | Block size | Write buffer | Bloom | Prefix |
|---|---|---|---|---|---|
| `dj_uuid_point_cf_options` | `dj_domain_packs`, `dj_raw_events`, `dj_normalized_states`, `dj_actions`, `dj_outcomes`, `dj_transitions`, `dj_bindings`, `dj_trajectories`, `dj_model_artifacts`, `dj_skill_policies` | 8 KB | 4 MB | yes | 16 B |
| `dj_composite_index_cf_options` | `dj_domain_pack_by_nv`, `dj_instrument_registry`, `dj_adapter_registry`, `dj_instrument_readings`, `dj_constellations`, `dj_threshold_calibrations` | 8 KB | 4 MB | yes | 16 B |
| `dj_secondary_index_cf_options` | `dj_bindings_by_entity` | 8 KB | 1 MB | no | 16 B |
| `dj_append_only_cf_options(prefix_len)` | `dj_adapter_runs`, `dj_pairwise_readings`, `dj_training_runs`, `dj_predictions`, `dj_plan_traces`, `dj_guard_decisions`, `dj_surprise_events`, `dj_verification_runs`, `dj_audit_log` | 8 KB | 4 MB | yes | configurable |
| `dj_large_blob_cf_options` | `dj_latent_panels`, `dj_dataset_shards` | 64 KB | 8 MB | yes | 16 B |

The audit log uses a smaller 8-byte prefix (a timestamp prefix) for efficient time-range scans, while data CFs use a 16-byte prefix (UUID) for point lookups.

---

## A.8 Reality Loop Failure Codes (selected)

The loop's typed errors fall into four categories:

**Configuration / preconditions**

`RUST_REALITY_MAX_ATTEMPTS_INVALID`, `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL`, `RUST_REALITY_OFFICIAL_EVAL_REQUIRED`, `RUST_REALITY_CONTEXT_GRAPH_BIN_MISSING`, `RUST_REALITY_TASK_MANIFEST_MISSING`.

**Manifest validation**

`RUST_REALITY_TASK_DATASET_MISMATCH`, `RUST_REALITY_TASK_INSTANCE_ID_INVALID`, `RUST_REALITY_TASK_REPO_INVALID`, `RUST_REALITY_TASK_BASE_COMMIT_INVALID`, `RUST_REALITY_TASK_PROBLEM_STATEMENT_MISSING`, `RUST_REALITY_TASK_FAIL_TO_PASS_MISSING`, `RUST_REALITY_TASK_PASS_TO_PASS_MISSING`, `RUST_REALITY_TASK_SOURCE_OF_TRUTH_INCOMPLETE`.

**Execution gates**

`RUST_REALITY_BASE_COMMIT_READBACK_MISMATCH`, `RUST_REALITY_WORKSPACE_NOT_CLEAN_AFTER_RESET`, `RUST_REALITY_MODEL_RESPONSE_JSON_INVALID`, `RUST_REALITY_MODEL_PATCH_MISSING`, `RUST_REALITY_PATCH_REPEATED`, `RUST_REALITY_PATCH_APPLY_FAILED`, `RUST_REALITY_PATCH_MODIFIES_TESTS`, `RUST_REALITY_PATCH_TOUCHES_BENCHMARK`, `RUST_REALITY_OFFICIAL_DOCKER_UNAVAILABLE`, `RUST_REALITY_OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID`, `RUST_REALITY_OFFICIAL_EVAL_FAILED`, `RUST_REALITY_OFFICIAL_REPORT_MISSING`, `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING`, `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED`.

**Persistence integrity**

`RUST_REALITY_JSON_READBACK_MISMATCH`, `RUST_REALITY_TEXT_READBACK_MISMATCH`, `RUST_REALITY_EVENT_READBACK_MISMATCH`.

Every code lands in `<runtime_root>/failures/failure-<ts>-<code>.json` with full context, then the binary exits non-zero.

---

## A.9 Where to Read Next

For the gate-by-gate enforcement table, see [`appendix_b_invariants.md`](appendix_b_invariants.md). For terminology, see [`appendix_c_glossary.md`](appendix_c_glossary.md). The complete subsystem documentation lives at `docs2/contextgraphspecs/`, with `21_dynamicjepa_subsystem.md` and `22_reality_loop_subsystem.md` being the most directly relevant.
