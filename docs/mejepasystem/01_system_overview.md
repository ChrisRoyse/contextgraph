# 01 — System Overview

Source files covered: `Cargo.toml`, `crates/*/Cargo.toml`, `crates/*/src/lib.rs`, `crates/context-graph-mcp/src/main.rs`, `crates/context-graph-mcp/src/daemon.rs`, `crates/context-graph-mcp/src/protocol.rs`, `crates/context-graph-cli/src/main.rs`, `docs/ruvectorfindings/specs/constitution.md`, `docs/futurebuild/00_INDEX.md`, `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md`, `docs/futurebuild/02_CURRENT_STATE_HONEST.md`, `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md`, `CLAUDE.md`.

---

## 1. What the system does

ME-JEPA-Code is a **deterministic predictive truth system** for AI-generated code. It accepts a (code change + context) tuple and returns a calibrated, multi-head **`RealityPrediction`** answering five operator questions (Q1–Q5 in `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md §3`): does the agent's claim **EXIST** on disk, does the code **WORK**, **WHY** would it fail, **WHAT ELSE** could go wrong (edge cases / tech debt / perf / security / accuracy / cost), and how does the patch **IMPACT REALITY**. The system runs as an **MCP (Model Context Protocol) server** locally on operator hardware. AI agents (Claude Code, Cursor, etc.) call it via JSON-RPC stdio for predictions and feedback. Underneath, a **continuous self-optimization daemon** consumes every Edit shift, every test outcome, and every agent-feedback event to retrain itself online, making the predictor measurably better the more it is used.

The system is JEPA-class. The architectural innovation that distinguishes ME-JEPA-Code from prior JEPA work is **structural collapse-immunity on the target side**: the target representation is produced by a panel of **frozen externally-defined instruments** (deterministic functions of typed events — AST chunks, oracle outcomes, witness chain entries, type-graph features, etc.), with zero trainable parameters. Per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`: `ΔΞ.target_collapse = 0` by construction.

The ship-gate threshold is `prediction_oracle_correlation ≥ 0.95` stable over four consecutive rolling windows on the 300×8 SWE-bench Lite mutation corpus, stratified per (mutation_category × language). When that's met, human code review of AI-generated patches is statistically unjustified.

---

## 2. Architecture (process / port / technology / purpose)

| Process | Port / Channel | Technology | Purpose |
|---|---|---|---|
| **`context-graph-mcp`** (MCP server) | stdio JSON-RPC; optional `--daemon-port 3100` / `--http-port 3101` | Rust 1.84+ / Tokio | Exposes 19+ `mcp__cgreality__mejepa_*` tools + the broader `cgreality` family; owns path-policy + edit-lock + ledger; gates all on-disk reality access |
| **`mejepa-daemon` (supervised tasks)** | inherits the MCP process; supervised via `tokio::JoinSet` | Rust / Tokio | Runs the shift subscriber + continuous self-optimization scheduler + nightly GC + weekly evaluation as long-running supervised tasks |
| **Shift subscriber** | tails `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` | tokio file watcher | Materializes a Panel within 250ms of each Edit/Write; triggers `verify()` → persists `RealityPrediction` to `CF_MEJEPA_LIVE_PREDICTIONS` |
| **Self-optimization scheduler** | 7 internal tickers (250ms / 60s / 600s / 3600s / 1800s) | tokio intervals | Drains pending shifts → `observe()`; classifies drift; surfaces active-learning queue; triggers LoRA refresh / A/B/C promotion / continual backprop |
| **Predictor forward** | CUDA queue (single GPU; CUDA 13.2; sm_120) | candle 0.9.x + cudarc | 6-layer transformer trunk (hidden 1024, 8 heads) + Panel head + Oracle head + 8 futurebuild auxiliary heads |
| **`context-graph-cli`** | command-line (operator + hook scripts) | Rust + clap | Operator surface: `mejepa active-learning`, `dynamicjepa`, `learning`, `utl`, `warmup`, `watch`, `setup`, `ccreality record-harness-transition`, `blackbox` (BlackBoxFix), `backfill-e14`, etc. |
| **Claude Code session** (the agent) | invokes MCP via Anthropic CLI | Anthropic Claude Code | The outer-loop reality-loop driver per `CLAUDE.md §11`. Edits land via Edit/Write/Bash; hook scripts append shift entries |
| **Hook scripts** | `.claude/settings.json` SessionStart / PreToolUse / PostToolUse / UserPromptSubmit / Stop / PreCompact | bash + cli helpers | Inject reality context per prompt; record pre/after SHAs; require Stop-hook recommendation |
| **RocksDB store** | unix domain / direct mmap | `rocksdb 0.21.x` | Persistent storage; 101 CFs total, 28+ ME-JEPA-specific. Lives under `/mnt/d/contextgraph/storage/contextgraph-rocksdb/` |
| **Docker oracle** | invoked by Phase 0 corpus generator + capture-mode test runs | `swebench.harness.run_evaluation` (Python) | Authoritative test outcome for SWE-bench Lite; produces `OracleVerdict { resolved, F2P / P2P / F2F / P2F }` |
| **Embedder model artifacts** | local files under `/mnt/d/cabdru/contextgraph/models/` | GGUF / safetensors / ONNX | Per-embedder SHA-pinned weight files; lazy-loaded by E1/E7/E10/E12/E13/E14/E17/E8/E11(retired) |
| **Witness chain** | append-only | SHAKE-256 73-byte / ed25519 | Cryptographic provenance for every panel, training cert, prediction, feedback event |
| **SMAP shared mind** | `./memory/` 8-namespace tree | markdown + YAML frontmatter | Cross-turn team narrative; replaces ephemeral context |
| **WSL host** | Linux 6.6 on Windows + RTX 5090 | WSL2 + CUDA 13.2 | Single-node deployment surface |

---

## 3. Technology stack

Source: `docs/ruvectorfindings/specs/constitution.md §tech_stack` + the per-crate `Cargo.toml` files.

| Layer | Technology | Version constraint | Notes |
|---|---|---|---|
| Language | Rust | 1.84+, edition 2021 | Workspace; no nightly required |
| Runtime | Tokio | 1.x (full features) | Async + supervised tasks via JoinSet |
| ML framework | candle | 0.9.x (alpha) | Pure-Rust GPU (`candle-core`, `candle-nn`, `candle-transformers`); CUDA via `cudarc` |
| CUDA target | sm_120 (Blackwell / RTX 5090) | CUDA 13.2 | Production predictor is CUDA-only; CPU construction is rejected, not silently down-graded |
| AST parser | tree-sitter | 0.23.x | 11 grammars: rust, python, javascript, typescript, go, java, c, cpp, c-sharp, ruby, php |
| Storage | RocksDB | 0.21.x | Hybrid backend with AgentDB (per CLAUDE.md §4); 101 CFs; tier-compressed cold storage |
| Hashing | sha3 | 0.7.x | SHAKE-256 for the 73-byte witness chain |
| Signing | ed25519-dalek | 2.x | Feature-gated `ed25519` segment signing in `context-graph-witness` |
| Tokenizers | HuggingFace `tokenizers` | 0.4.x | E1, E7, E10, E14 tokenizers |
| Serialization | serde, serde_json | 1.0.x | JSON for provenance / SourceMetadata; **never bincode + skip_serializing_if** (locked) |
| Quantization | clean-room | crate-local | 8/7/5/3-bit groupwise linear scalar quantization (`context-graph-tier-compression`) |
| Mincut | clean-room | crate-local | Stoer-Wagner O(V³) global mincut (`context-graph-mincut`) |
| Mutation operators | clean-room | crate-local | 8 Python operators shipped; 10 non-Python pending (Phase C) |
| MCP protocol | JSON-RPC 2.0 over stdio | spec-compliant | Single network surface per `CLAUDE.md §11.9`. No HTTP from runtime |
| Docker oracle | upstream `swebench` | unmodified | Reality boundary per `CLAUDE.md §8`. **NEVER edit `swebench/**`.** |
| Test runner | `cargo test` | std | Real synthetic data only; no mocks |
| Lint | `cargo clippy` | std | Required before commit |

---

## 4. Complete public API / tools / endpoints

### 4.1 ME-JEPA MCP tool surface (19 live tools at HEAD)

Per `docs/ruvectorfindings/specs/_README.md`. Full details in [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md).

| Tool | One-line behavior |
|---|---|
| `mejepa_verify` | Synchronous: take a patch + context, return a full `RealityPrediction` (Q1–Q5) |
| `mejepa_predict_latest` | Read the latest `RealityPrediction` for a session from `CF_MEJEPA_LIVE_PREDICTIONS` |
| `mejepa_constellation_inspect` | Inspect TCT centroids; per-chunk-type nearest-cell lookup; force refit |
| `mejepa_heal_status` | Current drift level, queued interventions, last A/B/C result |
| `mejepa_rollback_to` | Operator-forced rollback to a previous model variant (active pointer change) |
| `mejepa_load_learner_state` | Load saved learner-state (E15–E21) snapshot |
| `mejepa_routing_lookup` | Lookup the per-(Language × EntityType) embedder routing table |
| `mejepa_vram_budget_report` | GPU VRAM accounting vs budget; per-model headroom |
| `mejepa_gc_run` | Force nightly GC run (demote + compact + evict + prune + verify); supports `--force-eviction` |
| `mejepa_quota_status` | Disk usage vs 115 GB hard quota; per-slice breakdown |
| `mejepa_witness_compress` | Force Merkle compression on witness chain entries > 90 days |
| `mejepa_observe_shift` | Manually trigger an observe() over a specific shift_id (daemon does this automatically) |
| `mejepa_subscriber_status` | Shift subscriber lag, latency p50/p99, last panic info |
| `mejepa_capture_audit` | Bind an outer-loop SWE-bench attempt to the corpus (write audit entry) |
| `mejepa_eval_run` | Force a holdout-eval run; per-(category × language) correlation |
| `mejepa_ship_gate_status` | Per-cell ≥0.95 ship-gate status with shrinkage; pass/fail cells |
| `mejepa_active_learning_queue` | High-priority queue contents (uncertain predictions, agent surprises) |
| `mejepa_eval_build_graph` | Rebuild patch-similarity graph for cross-task transfer |
| `mejepa_audit_pairwise_mi` | Weekly pairwise-MI audit across embedder outputs; CSV at `/mnt/d/contextgraph/exports/pairwise-mi/` |

### 4.2 Futurebuild additions (Phase A–B)

| Tool | Phase | Purpose |
|---|---|---|
| `mejepa_record_agent_feedback` | A | Agent records `SurpriseEvent { kind, severity, agent_explanation, actual_outcome }` bound to a `prediction_id` |
| `mejepa_predict_what_if` | B | Hypothetical prediction without applying the edit |
| `mejepa_explain_prediction` | B | Per-prediction reasoning trace (saliency, contributing embedders, counterfactual minimum edit) |
| `mejepa_promote_approval` | E | Operator approval for catastrophic A/B/C promotions |
| `mejepa_daemon_status` | F | Aggregator wrapping subscriber + heal + quota + vram status |
| `mejepa_inspect_prediction` | F | Full provenance trace for a `prediction_id` |
| `mejepa_pause_predictions` | F | Suspend prediction emission for `duration-mins` (incident response) |

### 4.3 Broader `cgreality` MCP surface

Per `.claude/loop.md §4.2`:

- **Reality-shift readback**: `reality_latest_root`, `reality_attempt_summary`, `reality_official_report`, `reality_failure`, `reality_signal`, `reality_problem_packet`, `reality_trigger_decision`, `reality_harness_transitions`, `reality_compare_attempts`, `reality_audit_trail`, `reality_query_ledger`, `reality_replay_artifact`, `reality_shift_compare_to_my_view`, `reality_shift_log`.
- **Recommendation / witness**: `optimizer_record_recommendation` (Stop-hook required), `optimizer_witness_chain_verify`, `optimizer_witness_chain_diff`, `optimizer_witness_chain_repair_legacy`, `optimizer_compute_influence`, `optimizer_recall_recommendations`.
- **Harness ops** (file I/O via MCP): `open_file_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_git_diff`, `harness_git_status`, `harness_verify_state`, `optimizer_record_harness_transition`.
- **Retired (fail-closed)**: `mcp__cgreality__reality_run_attempt` → `CCREALITY_ENGINE_RETIRED`. **DO NOT** re-enable.

### 4.4 CLI command groups (`context-graph-cli`)

Source: `crates/context-graph-cli/src/main.rs`, `crates/context-graph-cli/src/commands/`. Full reference in [`19_cli_reference.md`](19_cli_reference.md).

| Group | Sub-commands | Purpose |
|---|---|---|
| `session` | `restore-identity`, `persist-identity` | Session state persistence |
| `hooks` | `session-start`, `pre-tool`, `post-tool`, `prompt-submit`, `session-end`, `pre-compact`, `task-completed`, `generate-config` | Claude Code hook entrypoints; each is timeout-bounded |
| `memory` | `inject-context`, `inject-brief`, `capture-memory`, `capture-response` | Context injection + memory capture |
| `topic` | `portfolio`, `stability`, `detect` | Topic clustering (HDBSCAN at threshold ≥2.5) |
| `divergence` | `check` | Divergence detection on semantic embedders (E1/E5/E6/E7/E10/E12/E13) |
| `dynamicjepa` | 44 ops | Domain pack registration, panel materialization, trajectory compilation, dataset shards, training, prediction, constellation, threshold calibration, MI audit, cross-domain transfer, FSV verification, research-smoke bundles |
| `learning` | `count`, `list`, `get`, `export-events-jsonl`, `export-dataset-jsonl`, `record-events-jsonl`, `record-synthetic` | UTL learning event surface |
| `utl` | `list-embedders`, `fixture`, `preflight-assets`, `count`, `record-synthetic`, `get-delta`, `get-m` | UTL learner-state computation (E15–E21) |
| `setup` | (direct) | Initialize Claude Code hooks |
| `warmup` | (direct) | Pre-load embedding models into VRAM |
| `watch` | (direct) | File watcher: chunk .md files (200w + 50w overlap) → memory |
| `export` | `training-corpus`, `dynamicjepa-episodes` | Parquet / JSONL exports |
| `backfill-e14` | (direct) | Backfill E14 BGE-M3 Dense vectors |
| `blackbox` | 44+ ops | BlackBoxFix Phase 1 local ledger and API server |
| `ccreality` | `record-harness-transition` | Post-edit harness transition recording via MCP |

Total: **15 top-level commands; ~150 sub-subcommands.**

### 4.5 Daemon HTTP probe (futurebuild Phase F)

`TASK-OBS-014-health-readiness-endpoint`: a small HTTP probe on `--http-port 3101` returning `200 OK` with body `{ "live": true, "ready": <bool>, "subscriber_lag_ms": N, ... }` for systemd/k8s health checks.

---

## 5. Entry points

| Entry | How invoked | Purpose |
|---|---|---|
| `cargo run --release -p context-graph-mcp` | from any directory | Foreground MCP stdio server (typical for Claude Code MCP attach) |
| `cargo run --release -p context-graph-mcp -- --daemon-server-only --d-root /mnt/d/contextgraph --daemon-port 3100 --http-port 3101` | `scripts/ensure_daemon.sh` or systemd | Long-running daemon: validates D-root, opens RocksDB, starts supervised tasks |
| `context-graph-cli <subcommand>` | operator / hooks | CLI surface (15 groups) |
| `context-graph` | symlink | Alias for `context-graph-cli` |
| `blackbox` | (separate binary) | BlackBoxFix ledger + API server |
| `context-graph-mejepa-corpus generate/verify/stats/prepare-images/cleanup` | operator | Phase 0 mutation corpus CLI |
| `context-graph-mejepa-train` (binary in `mejepa-train`) | operator | Training driver (cert chain, holdout eval) |
| `mejepa-heal-drill` (binary in `mejepa`) | operator / tests | Synthetic drift drill |
| `mejepa-verify` | operator / tests | Inference CLI: `verify` → `Verdict` |
| `.claude/settings.json` hooks | Claude Code | `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact` (Claude Code-managed) |
| `scripts/cc/hook_*.sh` | invoked by Claude Code hooks | SHA recording, shift-log append, cargo check, prediction injection |
| `scripts/ensure_daemon.sh` | operator | Bootstraps the warm daemon + stdio proxy |
| `scripts/systemd/mejepa-daemon.service` | `systemctl --user enable --now` | Systemd user-unit running daemon mode |

---

## 6. Directory structure (runtime)

Source: `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`.

```
/mnt/d/contextgraph/                            # ALL durable bulk state lives here (locked)
├── storage/
│   ├── contextgraph-rocksdb/                   # RocksDB CFs (101 total, 28+ ME-JEPA)
│   ├── replay/                                 # replay-buffer materializations
│   ├── cold/                                   # 3-bit / 1-bit frozen tier blobs
│   ├── witness/                                # witness chain segments + Merkle bundles
│   ├── train-certs/                            # per-step training certs (mirrored from CF)
│   ├── live-predictions/                       # mirrors CF_MEJEPA_LIVE_PREDICTIONS
│   └── embedder-cache/                         # per-(embedder, chunk_sha) cache
├── runtime/
│   ├── cgreality-shift-log/<session>.jsonl     # per-session shift-log JSONL (append-only)
│   └── test-outcomes/                          # parsed pytest/cargo-test/jest TestOutcomeEvent
├── logs/
│   └── mejepa-daemon.stdout.log                # rotated daily; 30-day retention
├── state/
│   ├── locks/
│   │   ├── context-graph-daemon.pid
│   │   ├── mcp.sock                            # MCP unix domain socket (daemon mode)
│   │   └── reality-loop-stdio.lock             # singleton lock (replaces stale MCP)
│   ├── cgreality/
│   │   └── active_runtime_root                 # active runtime root pointer
│   └── gold-labels/                            # operator-approved gold labels (permanent)
├── fsv/<crate>-fsv/                            # FSV evidence files (mode 0600)
├── corpus/<corpus-id>/                         # SWE-bench mutation corpus runs
│   ├── index.json
│   ├── stats.json
│   ├── verify-evidence.json
│   └── oracle-store/                           # RocksDB with CF_MEJEPA_MUTATION_CORPUS, etc.
└── exports/
    ├── active-learning/<date>.json
    ├── eval/<date>/weekly.md                   # weekly operator report
    └── pairwise-mi/<date>.csv

/mnt/d/cabdru/contextgraph/models/              # SHA-pinned embedder model artifacts (separate budget)
└── <embedder_id>/                              # GGUF / safetensors / ONNX + tokenizer
```

Stays on C:\ (under `/home/cabdru/contextgraph/`):

```
crates/                # source tree (25 crates)
docs/                  # design docs (futurebuild/, ruvectorfindings/, ccreality/ historical, mejepasystem/)
config/                # governed paths, model pins, reward specs
configs/               # alternate config tree
scripts/               # cli helpers, hook scripts, systemd unit
tasks/swebench-lite/   # 300 SWE-bench Lite task manifests (KEEP)
memory/                # SMAP shared mind (8 namespaces)
.claude/               # settings.json, agents/*, skills/, loop.md
target/                # cargo build artifacts (reproducible)
```

Ephemeral test scratch stays in `/tmp/` (must wrap in `tempfile::TempDir`).

---

## 7. Storage tier classification

Per `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md §5`. Quota = 115 GB hard cap on `/mnt/d/contextgraph/`.

| Tier | What | Examples | Treatment |
|---|---|---|---|
| **Sacred** | reality-bound; never auto-mutated | Witness chain segments, training certs, gold-standard operator labels | Append-only; Merkle compress > 90 days; survive disasters via backup |
| **Hot** | growing, queried often | `CF_MEJEPA_PANELS`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_DDA_SIGNALS`, latest shift logs | 8-bit groupwise quantization where applicable; daily-active |
| **Warm** | 30-day window | recent training certs, recent feedback events | 7-bit |
| **Cool** | older but not frozen | aging panels, dormant predictions | 5-bit |
| **Cold** | rarely touched, long-tail | frozen panel slabs, archived shift logs > 30 days | 3-bit groupwise; ttl per CF |
| **Frozen** | provably immutable | constellation cells past freshness check, locked baseline | 1-bit / mark `frozen=true` |
| **Regenerable** | rebuild from source | `target/release/`, `models/` (recoverable via re-download with SHA), corpus from mutation operators | Wipe and rebuild on demand |
| **Ephemeral** | survive one operation | `/tmp/*` test scratch, intermediate computation buffers | Must wrap in `tempfile::TempDir` |

---

## 8. Exception / error hierarchy

Source: `docs/ruvectorfindings/specs/constitution.md §failure_taxonomy` + per-crate `error.rs` files.

### 8.1 Top-level error namespaces

| Namespace | Where | Purpose |
|---|---|---|
| `MEJEPA_*` | all ME-JEPA crates | ME-JEPA-specific failures (observe, infer, retrain, calibration, chunker, corpus) |
| `CCREALITY_*` | `context-graph-mcp` | ccreality MCP-tool-layer failures (engine retired, governed-path violations) |
| `CONTEXTGRAPH_*` | `context-graph-core`, `context-graph-storage` | Storage / domain failures |
| `WITNESS_*` | `context-graph-witness` | Witness-chain integrity failures |

### 8.2 Major specialized error codes

| Code | Trigger | Recovery |
|---|---|---|
| `MEJEPA_OBSERVE_BATCH_NAN` | gradient/activation non-finite during `observe()` | rollback to prior checkpoint; surface to operator |
| `MEJEPA_OBSERVE_DRIFT_DETECTED` | empirical conformal coverage < 0.85 | trigger A/B/C retrain |
| `MEJEPA_OBSERVE_INTEGRITY_VIOLATION` | witness-chain replay broken | halt inference; restore from last verified checkpoint |
| `MEJEPA_INFER_OOD_REFUSE` | `ood_score > 0.5` | refuse to predict; fail-closed |
| `MEJEPA_INFER_DIM_MISMATCH` | panel dimensionality wrong | reject input; do not silently pad or truncate |
| `MEJEPA_INFER_INSTRUMENT_MISSING` | required instrument unavailable | fail-closed; report missing toolchain |
| `MEJEPA_INFER_CONSTELLATION_VIOLATION` | Gτ rejected the predicted panel | persist with `violating_embedders` + `violating_chunks` |
| `MEJEPA_INFER_GTAU_STALE_CONSTELLATION` | constellation > max_age | refuse without operator override |
| `MEJEPA_RETRAIN_HOLDOUT_REGRESSION` | Mode B/C didn't beat Mode A on holdout | do not promote |
| `MEJEPA_RETRAIN_QUOTA_EXCEEDED` | disk/RAM quota hit during retrain | pause; trigger eviction |
| `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES` | < N samples in calibration window | refuse calibration; fall back to last-known threshold |
| `MEJEPA_CHUNKER_PARSE_FAILED` | tree-sitter ERROR or MISSING node | emit byte-offset + line + node-kind diagnostics; never fall back to text chunking |
| `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE` | language not in 11-language matrix | reject fail-closed |
| `MEJEPA_CHUNKER_EMPTY_SOURCE` | empty source | no empty chunks emitted |
| `MEJEPA_CHUNKER_MIXED_LANGUAGE` | embedded language detected (HTML+JS, PHP+HTML) | reject; future work routes per-block |
| `MEJEPA_CORPUS_INVALID_INPUT` | source has parse errors | mutation operators refuse |
| `MEJEPA_CORPUS_OPERATOR_FAILED` | mutation site bounds invalid | refuse |
| `MEJEPA_EMBEDDER_DIGEST_MISMATCH` | SHA-256 digest mismatch on model load | refuse to start daemon |
| `MEJEPA_EMBEDDER_RETIRED` | call to E5 or E11 | fail-closed; do not return placeholder |
| `MEJEPA_INSTRUMENT_GRADIENT_LEAK` | non-zero `target_collapse` in `compute_delta_xi` | critical bug: an instrument is accidentally trainable |
| `MEJEPA_D_DRIVE_UNAVAILABLE` | `/mnt/d/contextgraph/` missing or not writable | daemon refuses to start |
| `MEJEPA_SCHEDULER_FLAPPING` | > 5 supervised-task restarts in 5 min | operator alert; manual cooldown |
| `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` | > 500 unprocessed shifts / 1 min sustained | escalate after 5 min |
| `MEJEPA_FEEDBACK_PREDICTION_NOT_FOUND` | `prediction_id` not in retention window | reject feedback |
| `MEJEPA_FEEDBACK_RATE_LIMITED` | > 100 feedback events / hour / agent | anti-DoS |
| `MEJEPA_VERIFY_INVALID_PATCH` | patch could not be parsed | reject |
| `MEJEPA_VERIFY_OUT_OF_DISTRIBUTION_HARD` | OOD score exceeds even-prediction-untrustworthy threshold | verdict returned but flagged |
| `MEJEPA_VERIFY_GUARD_REJECTED_HARD` | every chunk fails Gτ | alert operator |
| `MEJEPA_DISSIPATION_DETECTED` | training loss falls but holdout doesn't | trigger LoRA refresh + EWC re-anchor |
| `CCREALITY_ENGINE_RETIRED` | `reality_run_attempt` called | retired binary; do not respawn |

---

## 9. Major subsystem summary

One paragraph each, what each subsystem does + key algorithm + inputs/outputs. Deep dives in [`05`](05_ast_chunker_subsystem.md) – [`21`](21_loop_contract.md).

### 9.1 AST Chunker (`context-graph-core::memory::ast`)

11-language tree-sitter-driven chunker producing AST-perfect atomic chunks bounded by syntactic entities (function / class / impl / module-decl). Inputs: source text + `Language`. Outputs: `Vec<Chunk { chunk_id, language, kind, byte_range, line_range, sha }>`. Chunk size target ~500 non-whitespace chars per Qodo / cAST methodology. SHA-256 chunk IDs; fails closed on `MEJEPA_CHUNKER_PARSE_FAILED` / `MEJEPA_CHUNKER_MIXED_LANGUAGE`. The reformat-invariant chunk-ID audit is tracked in `[#12]`.

### 9.2 Embedder Substrate (`context-graph-mejepa-embedders`, `context-graph-embeddings`)

21 declared embedder slots; **13 active for code-domain** (E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14, E17); E5 and E11 retired post-2026-05-09. SHA-256 digest verification on every model load; VRAM budget gate; E17 requires calibration cert. **`EmbedderForward` / `PretrainedEmbedderForward` wrappers exist for E1/E6/E7/E8/E10/E12/E13/E14 with fail-closed artifact checks**; remaining work is full per-embedder FSV + cache + production integration ([#3](https://github.com/ChrisRoyse/contextgraph/issues/3)). Routing table = 143 rows mapping (Language × EntityType) → embedder set.

### 9.3 Instruments + Panel (`context-graph-mejepa-instruments`)

15 instrument slots summing to 5,120-d. Each slot is a deterministic, fail-closed encoder from a typed event → fixed-dim subvector (e.g., `E_Oracle` 128-d encodes pass/fail/skip/error fractions + 10-class exception one-hot). `Panel` and `PanelBuilder` enforce: dim-check, finite-check, double-fill rejection, JSON round-trip via `try_from` invariant gate. `PanelStore` persists to `CF_MEJEPA_PANELS`. Linear-probe gate validates panel composition.

### 9.4 DDA Enrichment + TCT Constellation (`context-graph-mejepa-tct`)

For each chunk × N embedders, produces `n · (N + N²)` signals: N per-embedder cosines-to-centroid + N² pairwise cosines + N² cached pairwise mutual information. For N=13 (active code-domain), that's ~169 signals per chunk; for ~30 chunks per patch, ~5,070 signals per training example. `TctConstellation` holds per-(category × entity × language × embedder) centroids (~13,728 cells max); Bayesian shrinkage when `N < 50`. Gτ guard = deterministic O(N · d_max) test: `cos(embedder_m(predicted), centroid_m) ≥ τ_m` for every m. Failure → `Verdict::GuardRejected` with `violating_embedders` + `violating_chunks`.

### 9.5 Predictor (`context-graph-mejepa::predictor`)

6-layer transformer trunk (hidden 1024, 8 heads, GeLU, residual + LayerNorm). Inputs: `(panel_t-1, panel_t)` concat = 10,240-d (futurebuild: 8,192-d with action). Output: predicted 5,120-d panel + per-head logits. **Shipped heads**: Panel (VICReg-targeted), Oracle (per-test pass/fail). **Futurebuild Phase B heads**: Failure-mode, Edge-case, Tech-debt, Perf, Security, Accuracy, Cost, Reasoning — each a thin Linear projection on the shared trunk. `FrozenTargetAdapter` reads panel[t=2] from `CF_MEJEPA_PANELS`.

### 9.6 Training Loop + UTML (`context-graph-mejepa-train`)

AdamW + cosine LR (`lr=5e-4`, `wd=0.04`, batch 64, 10 epochs). `Trainer` orchestrates: `BatchSampler` (with sampling_weight = base × L_step × (1 + 5·operator_override) × (1 + 2·agent_surprise_severity) × decay^age), `Trunk + Heads` forward, multi-loss aggregation (panel + pairwise + action + constellation + anti-pole + uncertainty + invariant + per-head λ_h losses). Every step writes a **TrainingCertificate** with ΔP / ΔK / ΔΩ / ΔΞ + components + grad_norm + drift_coverage + Merkle root, hash-chained via `parent_witness_hash → self_hash` into `CF_MEJEPA_TRAIN_CERTS`. UTML invariant: `ΔΞ.target_collapse == 0` enforced via `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. `compute_delta_xi` is multiplicative non-collapse. PairwiseMI auditor + dormant-unit detector + state-transfer T detector all shipped (REQ-UTML-06/07/08).

### 9.7 Inference (`context-graph-mejepa::compiler`)

`MeJepaCompiler::{compile, verify, observe, compile_with_panel}`. `verify(patch, context)` → `Verdict { Pass, Fail, OutOfDistribution, Abstain, GuardRejected }`. Forward path: chunk → embed → DDA signals → Panel → predictor → conformal interval wrap → Gτ guard → assemble `RealityPrediction`. Conformal: SplitConformal with target 90% coverage; OOD via reconstruction-error / σ²; threshold 0.5. Live latency p50 < 100ms (cached instruments). MCP surface: `mejepa_verify`.

### 9.8 Self-Healing Scheduler (`context-graph-mejepa::heal`)

18-module tree: `drift`, `ewc`, `promote`, `full_retrain`, `integrity`, `lora_refresh`, `calibration`, `distill`, `regulate`, `pipeline`, `drill`, `store`, `cf`, `fsv`, `scheduler`, `scheduler_state`. Long-running tokio scheduler runs 7 tickers (250ms observe / 60s drift / 600s active-learning / 3600s promote / 1800s continual_backprop / 3600s constellation_freshness / 600s emergency_eviction). Drift classes: Soft (88-85%), Hard (85-80%), Catastrophic (<80%). A/B/C promotion gate: `score = holdout_correlation - 0.05 · inference_latency_multiplier`. EWC++ Fisher with per-head matrices (futurebuild). Phase A scheduler skeleton shipped; full autonomous policy is Phase E.

### 9.9 Shift Subscriber (`context-graph-mejepa-shift-subscriber`)

Tokio task tailing `cgreality-shift-log/<session>.jsonl`. `ShiftLogTail` cursor-based; `WatermarkWriter` persists last-consumed offset per session to `CF_MEJEPA_SHIFT_WATERMARK`. `InstrumentCache` keyed by `(slot, file_sha)`. `ShiftSubscriber::{run_until_idle, run_until_shutdown}` drives `shift_to_inference → observe → prediction`. Latency: p50 < 5s, p99 < 30s. Lag alert > 500 unprocessed shifts / 1 min. Three MCP tools: `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`.

### 9.10 Hygiene (`context-graph-mejepa-hygiene`)

Tier transitions (`tier_for_score(access_count_30d) → Hot/Warm/Cool/Cold/Frozen`), quota + emergency eviction (115 GB hard cap), witness Merkle compression of segments > 90 days, nightly GC orchestrator. 3 MCP tools (`mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`) + `mcp.rs` dispatch. JSON codec for the GC report. Path discipline enforced via `context-graph-paths` (no `/tmp`, no `$HOME`, no C-drive fallback).

### 9.11 Phase 0 Corpus (`context-graph-mejepa-corpus`)

8 Python mutation operators: `KnownGood`, `SubtleFlip`, `OffByOne`, `SwapVariable`, `DeleteTestCall`, `WrongFile`, `OverEngineer`, `CompileError`. Pure-Rust, deterministic (SplitMix64 PRNG), fail-closed. Includes `python_lex.rs` (triple-quote / f-string-aware Python masker). `swebench.rs` invokes `python -m swebench.harness.run_evaluation` for single-instance grading; parses per-instance `report.json` → `OracleVerdict`. Docker-name-safe run_id validator. Repo-atomic + Instance-atomic split via `split.rs` (Stoer-Wagner mincut + LPT pre-sort + bin-pack). 10 non-Python languages pending (Phase C).

### 9.12 Witness Chain (`context-graph-witness`)

Clean-room reimplementation: 73-byte SHAKE-256 chain entry, ed25519 segment signing (feature-gated `ed25519`), Stage 0 / Stage F / Merkle compression certificates. Every panel, training cert, prediction, feedback event binds to a `witness_hash`. `optimizer_witness_chain_verify` / `_diff` / `_repair_legacy` for integrity.

### 9.13 Solver Primitives (`context-graph-solver`, `context-graph-mincut`, `context-graph-tier-compression`)

`solver`: MMR (top-k diversified retrieval), conjugate gradient, PageRank, conformal predictor, Granger F-test (OLS + Lentz-method incomplete beta + Lanczos lgamma), causal masking. `mincut`: Stoer-Wagner global min-cut (O(V³) dense) for repo-atomic / A-B split with min cross-set edge weight. `tier-compression`: 8/7/5/3-bit groupwise linear scalar quantization with magic+version header (compression ratios 3.89× / 4.43× / 6.12× / 9.91×).

### 9.14 Storage Layer (`context-graph-storage`)

RocksDB with 101 column families total, including the 28+ ME-JEPA CFs. Centralized CF registry in `context-graph-mejepa-cf` (dependency-free) so runtime / hygiene / MCP / train share names without cycles. `column_families.rs` enumerates all CFs; `indexes.rs` manages secondary indexes; `graph_edges/` + `code/` + `dynamicjepa/` + `teleological/` sub-modules.

### 9.15 MCP Server (`context-graph-mcp`)

JSON-RPC 2.0 over stdio. `handlers/tools/` houses every dispatcher; `protocol.rs` owns the JSON-RPC wire format. `daemon.rs` + `daemon_validate.rs` host the supervisor for daemon mode. Singleton flock at `~/.cache/cgreality/reality-loop-stdio.lock` evicts stale instances on `/mcp reconnect`. Path-policy gate **removed 2026-05-07**; path discipline is behavioral per `CLAUDE.md §8`.

### 9.16 CLI (`context-graph-cli`)

15 top-level commands; ~150 sub-subcommands. Three binaries: `context-graph-cli` (main), `context-graph` (symlink alias), `blackbox` (BlackBoxFix ledger). Notable: `dynamicjepa` group (44 ops covering domain packs, panels, trajectories, datasets, training, prediction, constellation, MI audit, cross-domain transfer); `hooks` group (Claude Code hook entrypoints, one per hook type, each timeout-bounded); `warmup` (preload 14 embedding models in 20-30s on RTX 5090).

### 9.17 Hooks Layer (`.claude/settings.json` + `scripts/cc/hook_*.sh`)

6 hook types: `SessionStart` (active runtime root injection), `UserPromptSubmit` (last 3 shifts + latest `RealityPrediction` markdown), `PreToolUse` Edit/Write (pre-edit SHA), `PostToolUse` Edit/Write (after_sha + cargo check + shift-log append), `Stop` (`optimizer_record_recommendation` required), `PreCompact` (SMAP narrative dump). Hook scripts shell out to `context-graph-cli hooks <subcommand>`. Path-policy gate has been REMOVED; the pre-edit hook records SHAs for provenance only.

### 9.18 Daemon Supervisor (`context-graph-mcp::daemon`)

`run_supervised_tasks(tasks: Vec<SupervisedTask>) -> Result<()>` using `tokio::JoinSet`. Each task has exponential backoff (cap 60s), 5-restart-in-5-min flap detection → `MEJEPA_SCHEDULER_FLAPPING` alert, watch-channel shutdown signal. Supervised tasks: shift subscriber, self-optimization scheduler, nightly GC scheduler (cron-driven at 03:00 local), weekly evaluation scheduler. D-root validator at startup → fails closed with `MEJEPA_D_DRIVE_UNAVAILABLE`.

### 9.19 Eval Harness (`context-graph-mejepa::eval`)

`EvalRunner::run_holdout_eval` + `metrics.rs` (Pearson correlation per cell, conformal health, OOD AUC, Gτ pass rate, Bayesian shrinkage), ship-gate failure analysis, `ActiveLearningQueueState`, `PatchSimilarityGraph`, weekly report writer, `RocksDbEvalStore` with 9 dedicated CFs. 4 MCP tools (`mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`). `phase8_eval_fsv` writes 17 evidence files.

### 9.20 SMAP Shared Mind (`./memory/`)

8-namespace tree (`context/`, `decisions/`, `discoveries/`, `handoff/`, `journal/`, `blockers/`, `patterns/`, `reports/`). Every `.md` file requires YAML frontmatter (`namespace`, `agent_id`, `timestamp`, `status`, `tags`). Per CLAUDE.md §6: resolved blockers stay (`status: resolved`); superseded decisions stay (`status: superseded`); reports are append-only. Lifecycle every agent follows: READ → ORIENT → WORK → WRITE → REPORT.

---

## 10. What is NOT covered

- **Web UI dashboard**: out of scope per `docs/futurebuild/17 §O1` (CLI + weekly markdown reports cover operator needs).
- **Multi-region / cloud deployment**: local single-machine is the design point; cloud is future work.
- **Federated training across operators**: requires multi-org consent; deferred.
- **Concrete ME-JEPA-Voice / -Image implementations**: template documented in [`28_future_domains_mejepa_x.md`](28_future_domains_mejepa_x.md); concrete builds wait for operator priority.
- **Training the underlying embedders**: embedders are frozen by design (constitution `<anti_patterns>`); fine-tuning is an explicit future-domain choice.
- **LLM-augmented label generation**: per the no-inner-LLM rule; only deterministic / analyzer-based labels.
- **Real-time web/Slack integration**: CLI + weekly report sufficient for v1.

---

## 11. Falsifiable end-state commitments

Source: `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md §8`.

| Commitment | Falsifier |
|---|---|
| `prediction_oracle_correlation ≥ 0.95` stable over 4 consecutive rolling windows on 300×8 SWE-bench Lite | any window drops < 0.92 without coincident known incident |
| Per-language conformal coverage stays within 88–92% of stated 90% ± 2% | any language exceeds ±5% deviation for two consecutive cycles |
| OOD AUC ≥ 0.85 between in-distribution and synthetic OOD | any week < 0.80 |
| Subscriber p50 < 5s, p99 < 30s under production traffic | two weeks at p99 ≥ 60s |
| Self-healing detects soft drift within 1,000 observations and promotes mode-winner without operator intervention | operator forced to manually trigger A/B/C in any non-catastrophic case |
| Disk footprint ≤ 110 GB under nightly GC with 115 GB hard quota | quota exceeded > 24h without emergency eviction |
| Inner-LLM residue absent from build surface (no llama-cpp-2, no Qwen pinning, no Mode B drip) | any commit reintroduces these without explicit operator override |
