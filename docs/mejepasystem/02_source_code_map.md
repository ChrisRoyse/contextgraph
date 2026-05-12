# 02 — Source Code Map

Source files covered: `Cargo.toml` (workspace), every `crates/*/Cargo.toml`, every `crates/*/src/lib.rs`, every `crates/*/src/main.rs` (where applicable), `.claude/settings.json`, `scripts/cc/*.sh`.

---

## 1. Workspace layout

```
/home/cabdru/contextgraph/
├── Cargo.toml                    # workspace manifest; lists 25 member crates
├── Cargo.lock                    # locked dep graph
├── CLAUDE.md                     # operator role + reality-loop contract (§11)
├── README.md, LICENSE, ...       # bounded; under git
├── crates/                       # 25 Rust crates (all under one workspace)
├── docs/
│   ├── futurebuild/              # forward plan (27 docs + specs/ + tasks/)
│   ├── ruvectorfindings/         # historical design (master plan + specs + 16 docs)
│   ├── ccreality/                # 21-doc historical pre-pivot design (KEEP as reference)
│   └── mejepasystem/             # THIS doc set
├── config/, configs/             # governed paths, model pins, reward specs
├── scripts/
│   ├── cc/                       # Claude Code hook scripts (hook_pre_tool_use_edit.sh, etc.)
│   ├── ensure_daemon.sh          # bootstraps warm daemon + stdio proxy
│   └── systemd/                  # mejepa-daemon.service unit
├── tasks/swebench-lite/          # 300 SWE-bench Lite task manifests (JSON)
├── memory/                       # SMAP shared mind: 8 namespaces
├── .claude/                      # settings.json, agents/*, skills/*, loop.md
├── target/release/               # cargo build artifacts (reproducible; bounded by cache)
├── tmp/                          # ephemeral scratch (cleaned)
├── reference/                    # upstream-reference-only checkouts (NEVER vendor or path-dep)
├── agentdb/, ruvector/           # clones for inspiration only (clean-room rule)
├── shakespear/                   # benchmark fixtures
└── pyproject.toml                # Python harness support for swebench bridge
```

---

## 2. The 25 crates — one-line descriptions

Source: each crate's `Cargo.toml` + `src/lib.rs` top doc comment.

| # | Crate | Version | Purpose |
|---|---|---|---|
| 1 | `context-graph-benchmark` | 0.1.0 | Benchmarking suite comparing 13-embedder multi-space fingerprinting vs single-embedder RAG baseline |
| 2 | `context-graph-causal-agent` | workspace | Deterministic causal relationship scanning + E5 activation helpers (LLM backend removed in 2026-05-09 pivot) |
| 3 | `context-graph-cli` | 0.1.0 | CLI surface: 15 command groups (session/hooks/memory/topic/divergence/dynamicjepa/learning/utl/setup/warmup/watch/export/backfill-e14/blackbox/ccreality) |
| 4 | `context-graph-core` | workspace | Core domain types + traits + AST chunker (11-language tree-sitter) + DynamicJEPA + learner training + monitoring |
| 5 | `context-graph-cuda` | 0.1.0 | CUDA acceleration for RTX 5090 / Blackwell sm_120: vector ops + attention + GPU HDBSCAN |
| 6 | `context-graph-dynamicjepa` | workspace | DynamicJEPA service crate: tiny Candle model training, hash-checked artifacts, source-of-truth persistence |
| 7 | `context-graph-embeddings` | 0.1.0 | Embedding pipeline with GPU-accelerated text→embedding via Candle backend (mandatory) |
| 8 | `context-graph-graph-agent` | workspace | Deterministic graph relationship scanning + E8 activation helpers (LLM backend removed in pivot) |
| 9 | `context-graph-mcp` | 0.1.0 | MCP JSON-RPC server: 206 tools, 14-embedder semantic memory retrieval system |
| 10 | `context-graph-mejepa` | workspace | ME-JEPA Phase 2 predictor (CUDA-only), frozen target, VICReg loss, FSV harness, 18-module heal/ tree |
| 11 | `context-graph-mejepa-cf` | workspace | Canonical ME-JEPA RocksDB column family names; dependency-free to share without cycles |
| 12 | `context-graph-mejepa-corpus` | workspace | Phase 0 mutation corpus generator: 8 operators, Docker oracle bridge, repo-atomic split, CLI |
| 13 | `context-graph-mejepa-embedders` | workspace | Phase 1b embedder ensemble: SHA-256 digest verification, VRAM budget gate, retired-slot policy, 143-row routing FSV |
| 14 | `context-graph-mejepa-hygiene` | workspace | Phase 6 storage hygiene: tier transitions, quota, GC, witness Merkle compression, 3 MCP tools |
| 15 | `context-graph-mejepa-instruments` | workspace | Phase 1 instruments + 5,120-d panel: `Instrument` trait, 15 slot encoders, Panel/PanelBuilder/PanelStore, linear-probe gate |
| 16 | `context-graph-mejepa-shift-subscriber` | workspace | Phase 7 durable JSONL shift subscriber: tail, watermark, panic supervision, latency telemetry |
| 17 | `context-graph-mejepa-tct` | workspace | Phase 4b TCT constellation guard: builder, calibrator, Gτ, shrinkage, per-chunk-type localization |
| 18 | `context-graph-mejepa-train` | 0.1.0 | Phase 3 training loop: UTML L_step + sampler + AdamW + cert chain + holdout eval + LoRA + distillation |
| 19 | `context-graph-mincut` | workspace | Stoer-Wagner global minimum-cut (O(V³) dense) for repo-atomic split with min cross-set edge weight |
| 20 | `context-graph-paths` | workspace | Fail-closed durable path resolver; no `/tmp` / `$HOME` / cwd fallbacks |
| 21 | `context-graph-solver` | workspace | Clean-room graph influence + sparse solver primitives: causal_mask, cg, conformal, csr, granger, mmr, pagerank |
| 22 | `context-graph-storage` | 0.1.0 | Storage layer: RocksDB CFs (101 total), AgentDB hybrid, indexes, dynamicjepa/teleological sub-modules |
| 23 | `context-graph-test-utils` | workspace | Shared test utilities — NO MOCK DATA (real fingerprints + store factories only) |
| 24 | `context-graph-tier-compression` | workspace | 8/7/5/3-bit groupwise linear scalar quantization for tiered cold storage |
| 25 | `context-graph-witness` | workspace | Clean-room tamper-evident witness chain: SHAKE-256 73-byte chain + Merkle tree + certificates + ed25519 (feature-gated) |

---

## 3. Per-crate file inventory

### 3.1 `context-graph-benchmark`

```
src/
├── ablation.rs
├── baseline/
│   ├── mod.rs
│   ├── single_embedder.rs
│   └── single_hnsw.rs
├── bin/                          # benchmark drivers
├── causal_bench/                 # causal-eval helpers
├── config.rs
├── datasets/                     # benchmark fixture loaders
├── lib.rs
├── metrics/                      # metric computation (recall@k, precision, MRR, ...)
├── realdata/                     # real-data benchmark adapters
├── reports/                      # report writers
├── runners/                      # benchmark execution
├── scaling/                      # scale tests
├── stress_corpus.rs
├── tuning/                       # hyperparameter sweep
├── util.rs
└── validation/                   # benchmark integrity checks
```

### 3.2 `context-graph-causal-agent`

```
src/
├── activator/mod.rs              # E5 activation helpers
├── error.rs
├── lib.rs
├── scanner/mod.rs                # candidate causal-pair scanning
└── types/mod.rs                  # CausalRelationship, etc.
```

### 3.3 `context-graph-cli`

```
src/
├── bin/
│   ├── blackbox.rs               # BlackBoxFix ledger + API server (separate binary)
│   └── context-graph.rs          # symlink alias for main
├── commands/                     # 15 command groups + 150+ sub-subcommands
│   ├── ccreality/mod.rs          # record-harness-transition
│   ├── session/mod.rs            # restore/persist-identity
│   ├── hooks/                    # 7 hook entrypoints
│   ├── memory/                   # inject-context/brief, capture-memory/response
│   ├── topic.rs                  # portfolio/stability/detect
│   ├── divergence.rs             # check
│   ├── dynamicjepa.rs            # 44 ops (LARGEST file)
│   ├── learning.rs               # 7 ops
│   ├── utl.rs                    # 7 ops
│   ├── setup.rs
│   ├── warmup.rs
│   ├── watch.rs
│   ├── export_training.rs
│   ├── export_dynamicjepa.rs
│   ├── backfill_e14.rs
│   └── blackbox.rs               # 44+ ledger ops (BlackBoxFix)
├── error.rs
├── governed_edit.rs              # legacy gate (now passthrough)
├── lib.rs
├── main.rs                       # top-level Cli enum
├── mcp_client.rs                 # MCP client for shell-side use
└── mcp_helpers.rs
```

### 3.4 `context-graph-core`

```
src/
├── causal/                       # causal types + helpers
├── clustering/                   # HDBSCAN (uses cuda for GPU when enabled)
├── code/mod.rs                   # CodeStore, code-domain types
├── config/                       # config types + loaders
├── constellation/                # legacy constellation types (TCT proper lives in mejepa-tct)
├── contrastive/                  # contrastive pair mining
├── dynamicjepa/                  # DynamicJEPA core types
├── embeddings/                   # embedding primitives + traits
├── entity/mod.rs                 # E11 entity types (retired but types remain)
├── error/                        # ContextGraphError hierarchy
├── fusion/mod.rs                 # MultiSpace + Pipeline fusion strategies
├── graph/                        # graph types
├── graph_linking/                # K-NN graph linking + typed edges
├── index/                        # HNSW + secondary indexes
├── injection/                    # context injection
├── learner/mod.rs                # learner-state types (E15-E21)
├── learner_training.rs           # learner training matrix
├── learning/mod.rs               # LearningEvent
├── lib.rs                        # 13-embedder system + retrieval
├── llm_edge_validation/          # validate_graph_link (Hermes-2-Pro)
├── memory/                       # MemoryStore, ast/ (NEW 11-lang chunker), legacy ast_chunker.rs
│   ├── ast/                      # NEW 11-language chunker (Phase 1a)
│   │   ├── classify.rs
│   │   ├── chunker.rs
│   │   ├── entity_types.rs
│   │   └── lib.rs
│   ├── ast_chunker.rs            # LEGACY Rust-only (converge to ast/)
│   └── mod.rs
├── monitoring.rs                 # ops monitoring primitives
├── quantization/                 # PQ-8, scalar
├── retrieval/                    # search strategies + RRF + post-processing
├── similarity/                   # cosine + normalization (raw [-1,1] vs normalized [0,1])
├── stubs/                        # in-memory stubs
├── teleological/                 # TeleologicalFingerprint + TeleologicalMemoryStore
├── training/                     # training surfaces
├── traits/                       # core traits
├── typed_edge_export/            # typed edge training records (F1/F2)
├── types/                        # ModelId core enum (14 variants), embedder slot enum
└── weights/mod.rs                # weight profiles
```

### 3.5 `context-graph-cuda`

```
src/
├── context/                      # CUDA context init
├── error.rs
├── ffi/                          # cudarc FFI bindings
├── hdbscan/                      # GPU HDBSCAN (ARCH-GPU-05)
├── lib.rs
├── ops.rs                        # similarity, attention
├── safe/                         # safe wrappers
├── similarity.rs                 # cosine, dot-product
└── stub.rs                       # CPU stub when CUDA missing
```

### 3.6 `context-graph-dynamicjepa`

```
src/
├── artifact.rs                   # hash-checked artifact load/save
├── config.rs
├── lib.rs
├── mi_audit.rs                   # pairwise MI auditor
├── model/mod.rs                  # tiny Candle DynamicJEPA model
├── service.rs                    # training/inference service
└── transfer.rs                   # cross-domain transfer (counter_world → gridworld)
```

### 3.7 `context-graph-embeddings`

```
src/
├── adapters/                     # per-model adapters
├── batch/                        # batch processor
├── bin/train_causal.rs           # E5 causal training (retired but binary stays)
├── cache/                        # embedding cache (key = (model, chunk_sha))
├── config/                       # 14-model production() config
├── error/
├── global_provider.rs            # GlobalProvider singleton
├── gpu/                          # CUDA helpers + VRAM allocator
├── learner/mod.rs                # E15-E21 wrappers
├── lib.rs
├── models/mod.rs                 # E1-E14 model implementations
├── provider/                     # EmbedderProvider
├── pruning/                      # legacy
├── quantization/                 # quantized weight storage
├── storage/                      # weight blob storage
├── training/                     # LoRA + AdamW + InfoNCE + multitask + distillation
├── traits/mod.rs                 # EmbedderForward, PretrainedEmbedderForward
├── types/mod.rs
└── warm/                         # warm-up sequence
```

### 3.8 `context-graph-graph-agent`

```
src/
├── activator/mod.rs              # E8 activation helpers
├── error.rs
├── lib.rs
├── scanner/mod.rs                # candidate graph-edge scanning
└── types/mod.rs                  # GraphEdge, etc.
```

### 3.9 `context-graph-mcp`

```
src/
├── adapters/                     # storage adapters
├── daemon.rs                     # supervisor (tokio JoinSet, supervised tasks, exponential backoff, flap detection)
├── daemon_validate.rs            # D-root validator
├── handlers/
│   └── tools/
│       ├── dispatch.rs           # the central tool dispatch table (206 tools)
│       ├── names.rs              # canonical tool name strings
│       ├── helpers.rs            # shared helpers (component_variance_f32, etc.)
│       ├── reality_loop/         # ccreality MCP-tool sub-tree
│       │   ├── alter.rs          # harness_open_window, harness_apply_line_window_edit, harness_run_command, harness_git_*, harness_verify_state
│       │   ├── optimizer.rs      # optimizer_record_decision/recommendation/harness_transition
│       │   ├── bandit.rs         # optimizer_bandit_select/record_reward/state
│       │   ├── recommendations.rs # optimizer_recall_recommendations
│       │   ├── influence.rs      # optimizer_compute_influence
│       │   ├── witness_chain.rs  # optimizer_witness_chain_verify/diff/repair_legacy
│       │   ├── shift_log.rs      # reality_shift_log, reality_shift_compare_to_my_view
│       │   ├── reality.rs        # reality_latest_root, reality_attempt_summary, reality_official_report, etc.
│       │   └── path_policy.rs    # gutted to "reject empty/control-char paths only" (2026-05-07)
│       ├── mejepa_tools.rs       # mejepa_verify, mejepa_predict_latest, mejepa_*
│       ├── mejepa_phase7_tools.rs # observe_shift, subscriber_status, capture_audit
│       ├── graph_link_tools.rs   # get_typed_edges, traverse_graph, get_unified_neighbors
│       └── ...                   # one file per tool group
├── lib.rs
├── main.rs                       # foreground / daemon entrypoint, --daemon-server-only flag
├── monitoring.rs
├── protocol.rs                   # JSON-RPC 2.0 wire format
├── server/                       # server loop + connection handling
├── tools/                        # tool schemas
└── weights.rs
```

### 3.10 `context-graph-mejepa`

```
src/
├── bin/
│   ├── mejepa-heal-drill.rs      # synthetic drift drill
│   └── mejepa-verify.rs          # CLI verify entry
├── calibration.rs                # ConformalCalibration loader/saver
├── calibration_types.rs
├── cli.rs                        # internal CLI for mejepa subcommands
├── compiler.rs                   # MeJepaCompiler::{compile, verify, observe, compile_with_panel}
├── config.rs
├── conformal.rs                  # SplitConformal predictor
├── data_models.rs                # types
├── degraded.rs                   # degraded-mode handling
├── error.rs
├── eval/                         # Phase 8 EvalRunner
│   ├── runner.rs                 # run_holdout_eval
│   ├── metrics.rs                # Pearson correlation, conformal health, OOD AUC, Gτ rate, Bayesian shrinkage, ship_gate_failures
│   ├── queue.rs                  # ActiveLearningQueueState
│   ├── graph.rs                  # PatchSimilarityGraph
│   ├── report.rs                 # weekly report writer
│   ├── store.rs                  # RocksDbEvalStore (9 CFs)
│   └── types.rs                  # EvalReport, AuxHeadDistillationSummary, StateTransferDiagnostic
├── evidence.rs
├── fixtures.rs
├── frozen_target.rs              # FrozenTargetAdapter (reads panel[t=2] from CF_MEJEPA_PANELS)
├── fsv_writer.rs                 # FSV evidence writer
├── gates.rs                      # ship gate evaluation
├── grad_hook.rs                  # gradient hook for non-finite detection
├── heal/                         # 18-module Phase 5 self-healing tree
│   ├── drift.rs
│   ├── ewc.rs                    # Fisher matrix, EWC++
│   ├── promote.rs                # A/B/C promotion gate
│   ├── full_retrain.rs
│   ├── integrity.rs              # witness chain integrity check
│   ├── lora_refresh.rs
│   ├── calibration.rs
│   ├── distill.rs
│   ├── regulate.rs               # dormant-unit re-init hook
│   ├── pipeline.rs
│   ├── drill.rs                  # synthetic drift drill
│   ├── store.rs
│   ├── cf.rs                     # heal-specific CFs
│   ├── fsv.rs
│   ├── scheduler.rs              # run_self_optimization_scheduler (Phase A skeleton)
│   └── scheduler_state.rs        # SchedulerState
├── lib.rs
├── loss/                         # legacy loss surface (current canonical in mejepa-train)
├── ood.rs                        # OOD detector
├── oracle_head.rs                # Linear<5120, num_tests + 1>
├── panel_source.rs               # PanelSource trait
├── predictor/                    # 6-layer transformer trunk
├── provenance.rs                 # Provenance struct
├── store.rs
├── types.rs                      # PredictionId, FeedbackId, PanelId, WitnessHash, AgentId, SurpriseSeverity, FeedbackKind, FailureModeClass, DdaSignals, ExemplarRef, SurpriseEvent, ActualOutcome, EmbedderViolationSignature
├── verify_cli.rs
├── verify_cli_tests.rs
└── vram.rs                       # VRAM accounting + nvidia-smi shell-out
```

### 3.11 `context-graph-mejepa-cf`

```
src/
└── lib.rs                        # ONLY canonical CF name constants; no deps; no cycles
```

CF names (subset, see [`04_database_schema.md`](04_database_schema.md) for full list):
`CF_MEJEPA_PANELS`, `CF_MEJEPA_PANEL_META`, `CF_MEJEPA_WEIGHT_BLOBS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_MUTATION_CORPUS`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_AGENT_FEEDBACK`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_CONSTELLATION`, `CF_MEJEPA_FISHER_SNAPSHOTS`, `CF_MEJEPA_DRIFT_WINDOW`, `CF_MEJEPA_DRIFT_HISTORY`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DISTILL_STEPS`, `CF_MEJEPA_ACTIVE_POINTERS`, `CF_MEJEPA_EVAL_REPORTS`, `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`, `CF_MEJEPA_PATCH_GRAPH`, `CF_MEJEPA_CORPUS_PROVENANCE`, `CF_MEJEPA_EMBEDDING_REGISTRY`, etc.

### 3.12 `context-graph-mejepa-corpus`

```
src/
├── bin/mejepa-corpus.rs          # CLI: generate, verify, stats, prepare-images, cleanup
├── compile_error.rs              # operator #8
├── delete_test_call.rs           # operator #5
├── generate.rs                   # corpus generation orchestration
├── generic_language_ops.rs       # cross-language helpers
├── known_good.rs                 # operator #1
├── lib.rs
├── off_by_one.rs                 # operator #3
├── oracle.rs                     # OracleVerdict types
├── over_engineer.rs              # operator #7
├── patch_mutation.rs             # patch mutation core
├── prng.rs                       # SplitMix64 deterministic PRNG
├── python_lex.rs                 # triple-quote / f-string-aware Python masker
├── source_patch.rs               # source-of-truth patch wrapper
├── source_patch_support.rs       # git subprocess (PER-SUBPROCESS TIMEOUT REQUIRED — see blocker 2026-05-11)
├── split.rs                      # train/calib/holdout split (Stoer-Wagner + LPT bin-pack)
├── split_tests.rs
├── subtle_flip.rs                # operator #2
├── swap_variable.rs              # operator #4
├── swebench.rs                   # Docker oracle bridge: run_swebench_lite_oracle()
├── tests.rs
├── util.rs
└── wrong_file.rs                 # operator #6
```

### 3.13 `context-graph-mejepa-embedders`

```
src/
├── calibration.rs                # E17 calibration cert
├── config.rs                     # 21-slot registry
├── diagnostics.rs
├── digest.rs                     # SHA-256 manifest pins
├── embedder_id.rs                # EmbedderId enum (E1..E21)
├── ensemble.rs                   # ensemble policy
├── error.rs
├── forward.rs                    # EmbedderForward + PretrainedEmbedderForward + EmbedderBackedInstrumentHead (E1/E6/E7/E8/E10/E12/E13/E14)
├── lib.rs                        # fail-closed control surface
├── routing.rs                    # 143-row (Language × EntityType) → embedder set
├── types.rs
└── vram.rs                       # VRAM budget gate
```

### 3.14 `context-graph-mejepa-hygiene`

```
src/
├── categories.rs                 # tier classification
├── entry.rs                      # storage entry types
├── error.rs
├── gc.rs                         # gc_run_nightly + GcReport
├── lib.rs
├── mcp.rs                        # mcp_gc_run, mcp_quota_status, mcp_witness_compress
├── ops.rs                        # exports: gc_run_nightly, quota_check_and_evict, heal_detect_drift, heal_retrain_if_needed, witness_compress_old_segments, witness_verify_integrity, tier_demote, tier_promote
├── quota.rs                      # quota_status + quota_check_and_evict + 115 GB hard cap
├── quota_types.rs
├── reports.rs
├── storage.rs                    # open_hygiene_rocksdb
├── tier.rs                       # tier_for_score, decayed_score, score_after_read, encode_for_tier, decode_from_tier, tier_demote, tier_promote
├── tier_frozen.rs                # 1-bit frozen tier
├── tier_ops.rs                   # tier_demote_all, tier_promote_all
└── witness_compress.rs           # witness_compress_old_segments, verify_witness_integrity, audit_verify_old_entry
```

### 3.15 `context-graph-mejepa-instruments`

```
src/
├── e_code.rs                     # code-semantic slots (E_AST, E_Diff via projection)
├── e_embedder_heads.rs           # EmbedderBackedInstrumentHead
├── e_oracle.rs                   # E_Oracle (128-d: pass/fail/skip/error fractions + 10-class exception one-hot)
├── e_reasoning.rs                # E_Reasoning (384-d via E17)
├── e_runtime.rs                  # E_Runtime (256-d: walltime + RSS + coverage Δ)
├── e_scalars.rs                  # Scalars (128-d: BFS depth, blame age, churn, complexity, coverage Δ, repo health)
├── e_static_analysis.rs          # E_StaticAnalysis (256-d)
├── e_text.rs                     # text-semantic slots (E_Test, E_Problem, E_CommitMsg via projection)
├── e_trace.rs                    # E_Trace (512-d)
├── e_witness.rs                  # E_Witness (256-d: sequence transformer over 73-byte chain entries)
├── features.rs
├── frozen_hook.rs                # frozen instrument verification hook
├── lib.rs                        # Phase 1 instruments + 5,120-d panel
├── linear_probe.rs               # linear-probe gate (validates panel composition)
├── materialize.rs                # panel materialization
├── panel_json.rs                 # JSON validator + try_from invariant gate
├── panel_store.rs                # PanelStore (CF_MEJEPA_PANELS)
└── tests.rs
```

### 3.16 `context-graph-mejepa-shift-subscriber`

```
src/
├── cache.rs                      # InstrumentCache (key = (slot, file_sha))
├── error.rs
├── lib.rs                        # Phase 7 durable JSONL subscriber
├── models.rs                     # Shift, SlotKey, LatencySnapshot, LiveSubscriberStatus, CaptureAuditBundle
├── runner.rs                     # ShiftSubscriber::{run_until_idle, run_until_shutdown}
├── shift_transform.rs            # shift_to_inference
├── subscriber.rs                 # ShiftSubscriber::open
├── tail.rs                       # ShiftLogTail (poll_next_line @ 250ms)
└── watermark.rs                  # WatermarkWriter (read_all, write)
```

### 3.17 `context-graph-mejepa-tct`

```
src/
├── builder.rs                    # constellation construction over Phase 0 + cross-language stratification
├── calibrator.rs                 # per-embedder threshold τ_m calibration (≤5% FP on KnownGood)
├── constellation.rs              # TctConstellation type
├── error.rs
├── freshness.rs                  # max-age policy (default 90 days)
├── gtau.rs                       # Gτ check + gtau_check_chunks (chunk-level localization)
├── hinge.rs                      # L_constellation_centroid hinge loss
├── hinge_types.rs
├── inspect.rs                    # mejepa_constellation_inspect MCP backing
├── lib.rs                        # fail-closed Phase 4b guard
├── panel_slots.rs                # per-slot centroid routing
├── rate.rs
├── rate_aggregator.rs
├── shrinkage.rs                  # Bayesian shrinkage on cells N<50
├── shrinkage_engine.rs
├── store.rs                      # CF_MEJEPA_CONSTELLATION
├── tests.rs
├── types.rs
└── verdict.rs                    # Verdict::GuardRejected + violating_embedders + violating_chunks
```

### 3.18 `context-graph-mejepa-train`

```
src/
├── aux/mod.rs                    # aux-head distillation
├── bin/mejepa-train.rs           # training driver
├── cert/                         # TrainingCertificate hash chain
├── checkpoint.rs                 # checkpoint save/load
├── cli/mod.rs
├── config.rs
├── distill/mod.rs                # distillation training
├── error.rs
├── eval/                         # holdout eval (legacy; canonical in mejepa::eval)
├── learning_signal/              # UTML L_step computation
│   ├── pairwise_mi_matrix.rs     # PairwiseMiAuditor (REQ-UTML-06)
│   ├── continual_backprop.rs     # DormantUnitDetector (REQ-UTML-07)
│   ├── state_transfer.rs         # state-transfer T (REQ-UTML-08)
│   ├── delta_p.rs                # compute_delta_p
│   ├── delta_k.rs                # compute_delta_k
│   ├── delta_omega.rs            # compute_delta_omega
│   ├── delta_xi.rs               # compute_delta_xi (target_collapse == 0 enforced)
│   └── l_step.rs                 # compute_l_step
├── lib.rs
├── lora_wiring.rs                # LoRA adapter wiring
├── loss/                         # vicreg.rs + constellation.rs + per-head losses
├── optim/mod.rs                  # AdamW + cosine LR
├── sampler/                      # BatchSampler (sampling_weight = base × L_step × (1+5·op) × (1+2·surprise) × decay^age)
└── trainer/mod.rs                # Trainer orchestrator
```

### 3.19 `context-graph-mincut`

```
src/
├── lib.rs                        # Stoer-Wagner O(V³) global min-cut
└── tests.rs                      # paper Fig 1 reproduces textbook mincut=4.0
```

### 3.20 `context-graph-paths`

```
src/
└── lib.rs                        # durable path resolver; no /tmp / $HOME / cwd fallbacks
```

### 3.21 `context-graph-solver`

```
src/
├── causal_mask.rs                # MaskStrategy {Strict, TimeWindow{w}, CausalWithSelf}, apply_mask_to_scores, retrocausal_safety_check
├── causal_mask_tests.rs
├── cg.rs                         # conjugate gradient
├── conformal.rs                  # split conformal predictor
├── csr.rs                        # CSR sparse matrix
├── error.rs
├── granger.rs                    # Granger F-test (OLS + Lentz incomplete beta + Lanczos lgamma)
├── granger_tests.rs
├── lib.rs
├── mmr.rs                        # MMR top-k diversified retrieval
└── pagerank.rs                   # PageRank
```

### 3.22 `context-graph-storage`

```
src/
├── code/                         # code-store
├── column_families.rs            # ALL 101 CF declarations
├── dynamicjepa/                  # DynamicJEPA CFs (dj_*)
├── error.rs
├── graph_edges/                  # typed edge index
├── indexes.rs                    # secondary indexes
├── lib.rs
└── teleological/                 # CF_TELEOLOGICAL_*, CF_TRAINING_RECORDS, CF_LEARNING_EVENTS, CF_LEARNER_*, CF_CONSTELLATIONS, CF_CONTRASTIVE_PAIRS, CF_TYPED_EDGE_RECORDS
```

### 3.23 `context-graph-test-utils`

```
src/
├── fingerprints.rs               # real fingerprint generators
├── lib.rs
└── stores.rs                     # store factory helpers
```

### 3.24 `context-graph-tier-compression`

```
src/
├── lib.rs                        # 8/7/5/3-bit groupwise linear scalar quantization + 22-byte magic+version header
└── tests.rs                      # FSV: cosine 0.99999/0.99997/0.99960/0.99192 on 768-d Gaussian
```

### 3.25 `context-graph-witness`

```
src/
├── cert.rs                       # Stage 0, Stage F, Merkle certificates
├── cert_merkle.rs                # Merkle tree
├── cert_tests.rs
├── lib.rs                        # 73-byte SHAKE-256 chain entries
└── sign.rs                       # ed25519 (feature-gated)
```

---

## 4. Module dependency graph (high-level)

Direction: `crate_a → crate_b` means a depends on b.

```
context-graph-mcp
  ├─→ context-graph-cli (sometimes; for blackbox dispatch)
  ├─→ context-graph-core (TeleologicalMemoryStore, traits)
  ├─→ context-graph-storage (RocksDB access)
  ├─→ context-graph-embeddings (E1-E14)
  ├─→ context-graph-mejepa (compiler, eval, heal)
  ├─→ context-graph-mejepa-tct (constellation_inspect)
  ├─→ context-graph-mejepa-hygiene (gc, quota, witness_compress)
  ├─→ context-graph-mejepa-shift-subscriber (Phase 7 tools)
  ├─→ context-graph-mejepa-cf (canonical names)
  ├─→ context-graph-witness (chain verify/diff/repair)
  └─→ context-graph-paths (D-root resolver)

context-graph-mejepa
  ├─→ context-graph-mejepa-train (Trainer, cert, sampler)
  ├─→ context-graph-mejepa-instruments (Panel, instruments)
  ├─→ context-graph-mejepa-tct (Gτ guard, GuardRejected)
  ├─→ context-graph-mejepa-embedders (routing, forward)
  ├─→ context-graph-mejepa-cf
  ├─→ context-graph-mejepa-hygiene (CF storage opens)
  ├─→ context-graph-embeddings (E1-E14 backbones)
  ├─→ context-graph-solver (mmr, conformal)
  ├─→ context-graph-witness (witness_hash on every persisted)
  ├─→ context-graph-storage (RocksDB)
  └─→ context-graph-cuda (predictor forward)

context-graph-mejepa-train
  ├─→ context-graph-mejepa-instruments (panel construction)
  ├─→ context-graph-mejepa-embedders (routing)
  ├─→ context-graph-mejepa-tct (constellation loss)
  ├─→ context-graph-mejepa-cf
  ├─→ context-graph-witness (training cert chain)
  └─→ context-graph-storage

context-graph-mejepa-shift-subscriber
  ├─→ context-graph-mejepa (compiler::observe)
  ├─→ context-graph-mejepa-instruments (panel build)
  ├─→ context-graph-mejepa-embedders
  ├─→ context-graph-mejepa-cf (CF_MEJEPA_SHIFT_WATERMARK)
  └─→ context-graph-storage

context-graph-mejepa-corpus
  ├─→ context-graph-mejepa-cf (CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS)
  ├─→ context-graph-mincut (Stoer-Wagner for split)
  ├─→ context-graph-paths (D-corpus root enforcement)
  ├─→ context-graph-witness (corpus provenance)
  └─→ context-graph-storage

context-graph-mejepa-tct
  ├─→ context-graph-mejepa-embedders
  ├─→ context-graph-mejepa-cf (CF_MEJEPA_CONSTELLATION)
  ├─→ context-graph-witness
  └─→ context-graph-storage

context-graph-mejepa-hygiene
  ├─→ context-graph-tier-compression
  ├─→ context-graph-mejepa-cf
  ├─→ context-graph-witness (witness_compress)
  ├─→ context-graph-paths
  └─→ context-graph-storage

context-graph-mejepa-instruments
  ├─→ context-graph-mejepa-embedders (EmbedderBackedInstrumentHead)
  ├─→ context-graph-mejepa-cf
  └─→ context-graph-witness

context-graph-cli
  ├─→ context-graph-mcp (mcp_client)
  ├─→ context-graph-core
  ├─→ context-graph-embeddings (warmup)
  ├─→ context-graph-storage
  ├─→ context-graph-dynamicjepa
  └─→ context-graph-mejepa-cf

context-graph-core
  ├─→ context-graph-cuda (clustering)
  ├─→ context-graph-embeddings
  ├─→ context-graph-storage
  └─→ context-graph-witness

context-graph-embeddings
  ├─→ context-graph-cuda
  ├─→ context-graph-core (traits, types)
  └─→ context-graph-storage

context-graph-storage
  └─→ (no internal deps beyond serde/rocksdb)

context-graph-mejepa-cf
  └─→ (NO INTERNAL DEPS — dependency-free by design)

context-graph-mincut, context-graph-solver, context-graph-tier-compression, context-graph-paths, context-graph-witness, context-graph-cuda
  └─→ (foundation crates; only external deps)
```

The dependency-free `context-graph-mejepa-cf` is the keystone preventing CF-name drift across crates without creating cycles.

---

## 5. Entry-point trace (MCP daemon mode)

```
cargo run --release -p context-graph-mcp -- --daemon-server-only --d-root /mnt/d/contextgraph
   │
   ▼
crates/context-graph-mcp/src/main.rs::main()
   │
   ▼
parse args (--daemon-server-only, --d-root, --daemon-port 3100, --http-port 3101)
   │
   ▼
crates/context-graph-mcp/src/daemon_validate.rs::validate_d_drive_root()
   │   (fails closed MEJEPA_D_DRIVE_UNAVAILABLE if /mnt/d/contextgraph missing or unwritable)
   ▼
flock(~/.cache/cgreality/reality-loop-stdio.lock) — singleton enforcement
   │   (SIGTERM(2s) → SIGKILL evicts stale prior MCP)
   ▼
open RocksDB at /mnt/d/contextgraph/storage/contextgraph-rocksdb/ with all 101 CFs registered
   │
   ▼
crates/context-graph-mcp/src/daemon.rs::run_supervised_tasks([
   ShiftSubscriberTask,                # context-graph-mejepa-shift-subscriber
   SelfOptimizationSchedulerTask,      # context-graph-mejepa::heal::scheduler
   NightlyGcSchedulerTask,             # context-graph-mejepa-hygiene::gc
   WeeklyEvalSchedulerTask,            # context-graph-mejepa::eval::runner
])
   │   (tokio::JoinSet supervisor; exponential backoff; flap detection > 5 restarts/5min → MEJEPA_SCHEDULER_FLAPPING)
   ▼
serve JSON-RPC 2.0 over stdio (or daemon-port for daemon mode)
   │
   ▼
handlers/tools/dispatch.rs — 206 tools registered
```

---

## 6. Entry-point trace (CLI `context-graph-cli`)

```
context-graph-cli <subcommand>
   │
   ▼
crates/context-graph-cli/src/main.rs::main() — global flags (-v/--verbose), Cli enum
   │
   ▼
match Cli::Subcommand to crates/context-graph-cli/src/commands/<subcommand>.rs::handle()
   │
   ▼
each subcommand opens its own resources (storage if needed, MCP client if needed)
```

---

## 7. Entry-point trace (one Edit shift, end-to-end)

```
Claude Code calls Edit
   │
   ▼ (PreToolUse hook fires)
.claude/settings.json::hooks.PreToolUse  →  context-graph-cli hooks pre-tool
   │   (records pre-edit SHA via crates/context-graph-cli/src/commands/hooks/pre_tool.rs)
   ▼
Edit returns
   │
   ▼ (PostToolUse hook fires)
.claude/settings.json::hooks.PostToolUse  →  context-graph-cli hooks post-tool
   │   (records after_sha, runs cargo check, appends shift-log JSONL entry at
   │    /mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl)
   ▼
~250ms later, daemon's shift subscriber consumes the line
   │
   ▼
crates/context-graph-mejepa-shift-subscriber/src/runner.rs::run_until_idle()
   ├─→ ShiftLogTail::poll_next_line()
   ├─→ ShiftSubscriber::open() reconstructs from CF_MEJEPA_SHIFT_WATERMARK
   └─→ for each shift:
       ├─→ memory::ast::chunk_with_options(file, language, opts) → Vec<Chunk>
       ├─→ for each chunk × embedder in routing: EmbedderForward::forward(chunk)
       ├─→ compute DDA signals → CF_MEJEPA_DDA_SIGNALS
       ├─→ PanelBuilder::build() → Panel(5,120-d) → CF_MEJEPA_PANELS
       ├─→ shift_to_inference():
       │   ├─→ MeJepaCompiler::verify(panel_t-1, action, panel_t)
       │   ├─→ predictor forward → predicted_panel + per-head outputs
       │   ├─→ ConformalPredictor::wrap() → interval
       │   ├─→ Gτ guard via mejepa_tct::gtau::gtau_check_chunks()
       │   └─→ assemble RealityPrediction → CF_MEJEPA_LIVE_PREDICTIONS
       └─→ update CF_MEJEPA_SHIFT_WATERMARK
   │
   ▼ (on agent's next prompt)
.claude/settings.json::hooks.UserPromptSubmit  →  (futurebuild) prediction-injection hook
   │   reads CF_MEJEPA_LIVE_PREDICTIONS, renders markdown, injects into agent context
   ▼
agent sees the prediction; may call mejepa_record_agent_feedback if disagrees
   │
   ▼ (when tests run)
PostToolUse on Bash → test-outcome capture hook → CF_MEJEPA_ORACLE_VERDICTS
   │
   ▼ (scheduler's next 250ms tick)
crates/context-graph-mejepa/src/heal/scheduler.rs::run_self_optimization_scheduler
   ├─→ observe_tick: drain pending oracle outcomes → MeJepaCompiler::observe()
   │   ├─→ EWC++ regularized SGD step
   │   ├─→ compute ΔP/ΔK/ΔΩ/ΔΞ → L_step
   │   ├─→ write TrainingCertificate → CF_MEJEPA_TRAIN_CERTS (witness-chain bound)
   │   └─→ update BatchSampler weights
   ├─→ drift_tick: classify Soft/Hard/Catastrophic
   ├─→ active_learning_tick: drain queue
   ├─→ promote_tick: A/B/C if pending
   ├─→ continual_backprop_tick: re-init dormant units
   ├─→ constellation_freshness_tick: refit stale cells
   └─→ emergency_eviction_tick: quota check
```

---

## 8. Build / package configuration

### 8.1 Workspace manifest (`Cargo.toml`)

Lists all 25 member crates; workspace-wide profile defaults (`release` with LTO + codegen-units=1 for the predictor crates).

### 8.2 Build targets

| Target | Command | Output |
|---|---|---|
| Full workspace | `cargo build --release --workspace` | every binary; ~5-10 min on cold cache |
| MCP server | `cargo build --release -p context-graph-mcp` | `target/release/context-graph-mcp` |
| CLI | `cargo build --release -p context-graph-cli` | `target/release/context-graph-cli`, `target/release/context-graph` (symlink), `target/release/blackbox` |
| Corpus CLI | `cargo build --release -p context-graph-mejepa-corpus` | `target/release/mejepa-corpus` |
| Training driver | `cargo build --release -p context-graph-mejepa-train` | `target/release/mejepa-train` |
| Heal drill | `cargo build --release -p context-graph-mejepa` (bin) | `target/release/mejepa-heal-drill` |
| Verify CLI | `cargo build --release -p context-graph-mejepa --bin mejepa-verify` | `target/release/mejepa-verify` |

### 8.3 Feature flags

| Flag | Crate | Purpose |
|---|---|---|
| `ed25519` | `context-graph-witness` | Enable ed25519 segment signing |
| `cuda` | `context-graph-cuda` | Enable real CUDA path (default off uses stub) |
| `e11_entity` | `context-graph-mcp` | Enable E11 entity tools (retired post-2026-05-09) |
| `e5_causal` | `context-graph-mcp` | Enable E5 causal tools (retired post-2026-05-09) |

### 8.4 What gets installed

- Binaries land under `target/release/`. Operator installs via `cp ... ~/.local/bin/` or via the systemd unit's `ExecStart` pointing at the workspace target.
- Models: NOT bundled. Operator places under `/mnt/d/cabdru/contextgraph/models/<embedder>/` with SHA-pinned digests per `models_config.toml`.
- Hooks: written by `context-graph-cli setup --generate-hooks` to `.claude/settings.json` + `.claude/hooks/*.sh`.
- Systemd unit: `scripts/systemd/mejepa-daemon.service` copied to `~/.config/systemd/user/`.

### 8.5 Reproducibility

- Rust toolchain pinned: `rust-toolchain.toml` (workspace root).
- Cargo.lock committed; deterministic dep graph.
- Tree-sitter grammars version-pinned at 0.23.x.
- Witness chain entries are byte-deterministic; same code + same model versions = byte-equal predictions.
- Phase 0 corpus is fully deterministic via SplitMix64 PRNG seed.
