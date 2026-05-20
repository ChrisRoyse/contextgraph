# 02 — Source Code Map

**Last refreshed:** 2026-05-19.

Source files covered: `Cargo.toml`, every `crates/*/Cargo.toml`, every `crates/*/src/lib.rs`, every `crates/*/src/main.rs`, every `[[bin]]` and `[[example]]` declaration.

Totals (2026-05-19, via `find crates -type f -path '*/src/*' -name '*.rs'`, `find crates -type f -path '*/examples/*' -name '*.rs'`, and `find crates -type f -path '*/tests/*' -name '*.rs'`): **1,600** src/lib `.rs` files · **210** examples · **258** test files.

---

## 1. Workspace Manifest

`Cargo.toml:6-30` lists 22 active workspace members. `default-members` mirrors the member list; there is no excluded benchmark crate in the live workspace.

Workspace package: `version = "0.1.0"`, `edition = "2021"`, `rust-version = "1.95"`, `license = "PolyForm-Noncommercial-1.0.0"`.

Workspace deps (`Cargo.toml:70-110`): tokio 1.35, serde 1.0, serde_json 1.0, bincode 1.3, toml 0.8, thiserror 1.0, anyhow 1.0, tracing 0.1 + tracing-subscriber 0.3 (json/env-filter), config 0.14, uuid 1.6 (v4/v5/serde), chrono 0.4 (serde), rand 0.8, rand_distr 0.4, candle-core 0.10.2 (cuda), candle-nn 0.10.2 (cuda).

Build profiles (`Cargo.toml:112-128`):
- `release` — `lto = "thin"`, `codegen-units = 16`, `strip = true`
- `release-max` — inherits `release`, `codegen-units = 1` (final-throughput builds only)
- `dev` — `opt-level = 0`, `debug = 1` (line info only), `incremental = true`, `split-debuginfo = "unpacked"`; external deps `debug = false`

---

## 2. Per-Crate Inventory

| # | Crate | Cargo description | Purpose | Key submodules |
|---|---|---|---|---|
| 1 | `context-graph-mcp` | "MCP JSON-RPC server for Context Graph" | Stdio + HTTP JSON-RPC, 201 tools, daemon, deprecation gate | `adapters/`, `daemon.rs`, `daemon_validate.rs`, `deprecation.rs`, `handlers/`, `health_probe.rs`, `monitoring.rs`, `protocol.rs`, `server/`, `telemetry/`, `tools/`, `weights.rs` |
| 2 | `context-graph-cli` | (no description) | Operator CLI + helpers | `commands/`, `error.rs`, `governed_edit.rs`, `mcp_client.rs`, `mcp_helpers.rs` |
| 3 | `context-graph-core` | "Core domain types, traits, and business logic for Context Graph" | Shared types: `CodeEntity`, `CodeLanguage`, etc. | per `types/` |
| 4 | `context-graph-storage` | "Storage layer for Context Graph with RocksDB backend" | RocksDB wrapper + 19 teleological + 13 quantized + 5 code + 2 causal CFs | `code/`, `column_families.rs`, `dynamicjepa/`, `graph_edges/`, `indexes.rs`, `teleological/` |
| 5 | `context-graph-witness` | "Clean-room tamper-evident witness-chain primitives for ContextGraph" | ed25519, Merkle, Stage 0/F/Merkle certs | `cert.rs`, `granger.rs`, `causal_mask.rs` |
| 6 | `context-graph-solver` | (n/a) | Causal masking, MMR, conformal predictor | `causal_mask.rs`, `granger.rs` |
| 7 | `context-graph-mincut` | "Clean-room global minimum-cut primitives for ContextGraph" | Stoer-Wagner global min-cut | (single-file lib) |
| 8 | `context-graph-paths` | "Fail-closed durable path resolver for ContextGraph" | Resolves runtime/durable roots; rejects non-aiwonder prod targets (per c4fd94a0) | (single-file lib) |
| 9 | `context-graph-tier-compression` | (n/a) | 8/7/5/3-bit groupwise scalar quantization with magic+version header | (single-file lib) |
| 10 | `context-graph-embeddings` | "Embedding pipeline for Context Graph with local model support" | E1-E14 pipeline, model loaders | `train-causal` bin |
| 11 | `context-graph-cuda` | (n/a) | CUDA helper kernels | — |
| 12 | `context-graph-mejepa` | "ME-JEPA-Code Phase 2 predictor, frozen target, VICReg loss, and FSV harness" | **Predictor trunk** (73 modules) | see §3 |
| 13 | `context-graph-mejepa-cf` | (zero-dep CF name registry) | Canonical RocksDB CF name constants (~120 ME-JEPA CFs in groups) | (single-file lib) |
| 14 | `context-graph-mejepa-corpus` | (n/a) | 8× mutation operators + SWE-bench bridge + split planner | see §4 |
| 15 | `context-graph-mejepa-instruments` | "Clean-room ME-JEPA-Code Phase 1 instruments + array-of-vectors panel scaffolding for ContextGraph" | 15 frozen instrument slots; `Panel` is an array of 15 per-embedder vectors (a teleological constellation), one vector per slot; per-slot byte offsets serialize into a 5,120 f32 contiguous buffer for transport only. Cross-embedder dim-wise comparison forbidden; same-embedder cosine + array-level operations + scalar pairwise cross-slot relationships only. | see §5 |
| 16 | `context-graph-mejepa-embedders` | (n/a) | 21 embedder slots, dynamic registry, calibration | `cache*.rs`, `calibration.rs`, `config.rs`, `digest.rs`, `dynamic_registry.rs`, `embedder_id.rs`, `ensemble.rs`, `forward.rs`, `routing.rs`, `types.rs`, `vram.rs` |
| 17 | `context-graph-mejepa-train` | "ME-JEPA Phase 3 training loop, certificate chain, holdout evaluation, and FSV" | UTML trainer, EWC, CBP, replay buffer, certificates, compression-progress | see §6 |
| 18 | `context-graph-mejepa-tct` | (n/a) | TCT constellations + Gτ guard + freshness | `builder.rs`, `calibrator.rs`, `constellation.rs`, `freshness*.rs`, `gtau.rs`, `guard_audit.rs`, `hinge*.rs`, `refresh_report.rs`, `shrinkage*.rs`, `store.rs`, `verdict.rs` |
| 19 | `context-graph-mejepa-hygiene` | "ME-JEPA Phase 6 storage hygiene, quota, GC, and witness compression" | Tier transitions, quota, GC, witness Merkle compress | `categories.rs`, `gc.rs`, `mcp.rs`, `quota.rs`, `reports.rs`, `retention.rs`, `storage.rs`, `tier*.rs`, `witness_compress.rs` |
| 20 | `context-graph-mejepa-shift-subscriber` | (n/a) | JSONL shift-log tail + watermark + cache → panels | `cache.rs`, `models.rs`, `runner.rs`, `shift_transform.rs`, `status.rs`, `subscriber.rs`, `tail.rs`, `watermark.rs` |
| 21 | `context-graph-dynamicjepa` | (n/a) | 5090jepa MCP surface (39 tools) — adapter shipping, plan trace, surprise log | (multiple modules) |
| 22 | `context-graph-test-utils` | "Shared test utilities for context-graph workspace — NO MOCK DATA" | Fixtures, deterministic helpers (no mocks for completion gates per FSV-PROTOCOL §3) | — |

**Note on §1 workspace member count.** As of 2026-05-20 cleanup the workspace has **22 active members**. The retired crates `context-graph-causal-agent`, `context-graph-graph-agent`, and the pre-pivot `context-graph-benchmark` suite were deleted after the ME-JEPA binary predictor mission left them with no ship-gate role.

---

## 3. `context-graph-mejepa` Modules (73 files; trunk of the predictor)

`crates/context-graph-mejepa/src/lib.rs:6-86` (Phase 2 — predictor + Phase 4 inference + everything bolted on since).

| Module | Purpose |
|---|---|
| `compiler.rs` (67K, biggest) | `MeJepaCompiler::compile` / `compile_with_fingerprint_observation` — main Q1-Q5 entry. Reads panel, runs predictor, applies conformal + Gτ guard + Cold-cell metric, assembles `RealityPrediction` |
| `predictor/` | `MeJepaPredictor`, transformer layers, `ArchitectureSummary` |
| `verdict_assembly.rs` | Stitches heads → `Verdict { Pass / Fail / OutOfDistribution / Abstain / GuardRejected }` |
| `claim_graph.rs` (~30K) | Q1 claim parser + AST/bytes-on-disk reconciliation → `CF_MEJEPA_CLAIM_RECONCILIATION` |
| `patch_similarity.rs` (~28K) | Q3 closest exemplars: usearch HNSW over L2-normalized `(E_AST, E_CFG, E_DataFlow, E_TypeGraph)` signatures → `CF_MEJEPA_PATCH_SIMILARITY_META` |
| `head_projection.rs` | Per-head DDA slice projection (TASK-PY-G-043) → `CF_MEJEPA_HEAD_PROJECTION_REGISTRY` |
| `contradiction.rs` | TASK-PY-G-044 per-cell multi-head contradiction thresholds and fail-closed detector → `CF_MEJEPA_CONTRADICTION_THRESHOLDS` |
| `failure_fingerprint.rs` (~75K) | First-class failure-shape fingerprint catalog (TASK-FP-001..010) → 8 fingerprint CFs |
| `adversarial_corpus.rs` | 30-case adversarial Python corpus (TASK-PY-G-060 / TASK-FP-009) → `CF_MEJEPA_ADVERSARIAL_CORPUS` |
| `synthetic_stress.rs` + `synthetic_stress_{cases,corpus,eval,store}.rs` | TASK-PY-G-071 synthetic Python stress eval → `CF_MEJEPA_SYNTHETIC_STRESS_RESULTS` |
| `project_ingest.rs` + `project_cache.rs` + `project_report.rs` + `project_stress.rs` | TASK-PY-G-062/063/064/072 reality compiler |
| `live_session_trace.rs` | TASK-PY-G-056 end-to-end live Claude Code session trace → `CF_MEJEPA_LIVE_SESSION_TRACES` |
| `reality_impact.rs` | Q5 shift-log replay |
| `claim_graph.rs`, `prediction_replay.rs`, `prediction_surfaces.rs` | Q1 surface + replay surfaces |
| `dynamic_embedder.rs` + `dynamic_embedder_freeze.rs` + `dynamic_embedder_vram.rs` | TASK-EK-018A/G/I dynamic-embedder runtime + freeze + VRAM budget |
| `embedder_falsification.rs` | TASK-EK-018C falsification gate |
| `embedder_proposal.rs` (~34K) | TASK-EK-018B absence-detection priority queue |
| `embedder_foundationality.rs` + `library_foundationality.rs` + `chunk_foundationality.rs` (~90K) | TASK-EK-016 / TASK-EK-016-ext / TASK-EK-017 PageRank on chunk/library/embedder dependency graphs |
| `entity_kge_embedder.rs` (~40K) | TASK-EK-018F TransE on corpus entity graph (the E11 replacement) |
| `algorithmic_embedder_synthesis.rs` + `learned_head_synthesis.rs` | TASK-EK-018D/E synthesis loops |
| `adversarial_embedder_injection.rs` | TASK-EK-018J adversarial injection mitigation |
| `instrument_proposal.rs` | TASK-EK-002 propose instrument |
| `mincut_panel.rs` (~48K) | TASK-EK-003 Stoer-Wagner on embedder/constellation graph |
| `pairwise_mi.rs` | TASK-PY-G-007 pairwise MI matrix audit |
| `compiler.rs` `ColdCellMetric` | TASK-PY-G-061 cold-cell verdict policy (Abstain when n_supporting<50) |
| `objective_safety.rs` | TASK-FP-104 hardwired objective + safety constraints |
| `candidate_ranking.rs` | TASK-FP-105 counterfactual repair ranking |
| `latent_search.rs` | TASK-FP-103 latent-action best-of-K search |
| `hierarchical.rs` | TASK-FP-102 durable file/function/AST/chunk prediction hierarchy → `CF_MEJEPA_HIERARCHICAL_PREDICTIONS` |
| `cross_language_label_gating.rs` | Phase F cross-language label gate |
| `system_cost.rs` (~23K) | Phase F system-cost computation |
| `toolchain_detect.rs` | Phase F toolchain availability gate |
| `degraded.rs` | Degraded-mode fallback |
| `conformal.rs` + `calibration.rs` + `calibration_types.rs` + `ood.rs` | Split-conformal predictor + per-head calibration + OOD residual score math |
| `ood_harvest.rs` + `ood_harvest/` | TASK-PY-G-057 / #276 live-session OOD harvest pipeline plus TASK-PY-G-014 / #129 serving calibrator → `CF_MEJEPA_OOD_HARVEST`, selected `CF_MEJEPA_OOD_CALIBRATIONS`, 4× active-learning projection, orphan quarantine, `OOD_GATE_OVER_FLAGGING`, `OOD_HARVEST_EMPTY`, `OOD_CELL_AUC_REGRESSION`, `OOD_CALIBRATOR_MISSING`, strict `mejepa_verify` fail-closed gate |
| `frozen_target.rs` + `grad_hook.rs` | Frozen-target adapter + gradient leak guard (`MEJEPA_INSTRUMENT_GRADIENT_LEAK`) |
| `evidence.rs` + `fsv_writer.rs` | FSV evidence builders |
| `operator_override.rs` + `operator_contribution.rs` (~29K) | REQ-PREDICT-12 / TASK-EK-014 operator override + contribution surface |
| `park_list.rs` | TASK-FLYWHEEL-007 fail-closed retry park list → `CF_MEJEPA_PARK_LIST` |
| `pause_state.rs` | Phase F pause consumer |
| `reward_signal_audit.rs` (~32K) | TASK-RWD audit reward signals; fail-closed `CF_MEJEPA_SIGNAL_DROP_LOG` |
| `sampler_reward.rs` (~20K) | TASK-PY-G-053 online observe reward → `CF_MEJEPA_SAMPLER_REWARDS` |
| `secret_redaction.rs` | TASK-PY-G-059 wrapper around `scripts/swebench_lite_ops/python_secret_redaction.py` |
| `label_transfer_audit.rs` | TASK-PREDICT-LABEL-012 → `CF_MEJEPA_LABEL_TRANSFER_DECISIONS` |
| `heal/` | Heal scheduler (5 modules) |
| `eval/` | Eval reports / dashboards |
| `loss/` | VICReg loss + Huber + inverse-map loss |
| `cli.rs` | CLI subcommands belonging to mejepa crate (also exposed via `context-graph-cli`) |
| `verify_cli.rs` | `mejepa verify` (System 2 deterministic harness; gated by `CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY`) |
| `types.rs` (114K, largest module) | `RealityPrediction`, `Verdict`, `RealityImpact`, `OracleScores`, all DTOs |
| `store.rs` | `MejepaStore` — RocksDB handle / opener |

---

## 4. `context-graph-mejepa-corpus` Modules

| Module | Purpose |
|---|---|
| `compile_error.rs` | Mutation op #8: Python syntax error |
| `delete_test_call.rs` | Mutation op #5: delete one assertion call |
| `generate.rs` | Corpus generator + SwebenchDockerOracle wrapper + report |
| `generic_language_ops.rs` | The 10 non-Python languages (deferred from current ship gate) |
| `known_good.rs` | Mutation op #1: echo input unchanged |
| `multilang_fixture_corpus.rs` | Multi-language fixture corpus |
| `off_by_one.rs` | Mutation op #3: ±1 to one numeric literal |
| `oracle.rs` | `OracleStore` — RocksDB corpus + verdict store; `iter_corpus_rows` |
| `over_engineer.rs` | Mutation op #7: append unused helper function |
| `patch_mutation.rs` | Apply patch text to base source |
| `prng.rs` | `SplitMix64` deterministic site selection |
| `python_lex.rs` | Triple-quote / f-string-aware Python lex mask (real bug found: `f"..." `string interior was being mutated) |
| `source_patch.rs` + `source_patch_support.rs` | Source-backed patch builder (replaces in-diff-line dependency) |
| `split.rs` + `split_tests.rs` | `SplitMode { InstanceAtomic (default), RepoAtomic }`; LPT pre-sort + lowest-current/target-ratio bin-pack; `assert_no_test_patch_leakage` |
| `subtle_flip.rs` | Mutation op #2: invert one boolean operator |
| `swap_variable.rs` | Mutation op #4: rename one local to another in-scope name |
| `swebench.rs` | `run_swebench_lite_oracle()` — `python -m swebench.harness.run_evaluation` wrapper, `FAIL_TO_PASS`/etc → `OracleVerdict`, Docker-name-safe `run_id` validator |
| `timed_subprocess.rs` | Subprocess timeout (kills hung Docker / git) |
| `util.rs` + `wrong_file.rs` | Helpers + Mutation op #6 |
| `tests.rs` | 81 unit tests (per memory note 2026-05-09e) |
| `bin/` | Corpus generator binaries |

---

## 5. `context-graph-mejepa-instruments` Modules

`crates/context-graph-mejepa-instruments/src/lib.rs:33-58` declares pub modules. Largest active modules: `e_code.rs`, `q4_security_labels.rs`, and the per-Q4 frozen label satellites. The unconsumed TASK-RWD producer modules were removed from the active crate surface on 2026-05-19 after the investigation cleanup; their historical proof remains in git history and closed issue evidence, not in compiled production code.

| Module | Instrument slot served | Dim | Notes |
|---|---|---|---|
| `e_oracle.rs` | `EOracle` | 128 | Pass/fail/skip/error fractions + total + has_exception + 10-class one-hot |
| `e_witness.rs` | `EWitness` | 256 | Witness-chain entry sequence encoder; `CANONICAL_WITNESS_FORMAT_VERSION` |
| `e_static_analysis.rs` | `EStaticAnalysis` | 256 | Linter / typecheck / complexity aggregates |
| `e_code.rs` | `EAst`, `ECfg`, `EDataFlow`, `ETypeGraph`, `EDiff` | 384+256+256+256+256 | Code-side encoders |
| `e_text.rs` | `ETest`, `EProblem`, `ECommitMsg` | 384+1024+384 | E1-reuse text encoders |
| `e_trace.rs` | `ETrace` | 512 | Execution-trace sequence transformer |
| `e_runtime.rs` | `ERuntime` | 256 | Wall-clock / RSS / coverage / network aggregates |
| `e_reasoning.rs` | `EReasoning` | 384 | Reasoning / reflexion trace encoder |
| `e_scalars.rs` | `Scalars` | 128 | BFS depth, blame age, churn, coverage Δ, repo-health |
| `q4_security_labels.rs` | (TASK-RWD-204 / TASK-PY-G-046) | — | Bandit + Semgrep label pipeline |
| `panel_json.rs` + `panel_store.rs` | — | — | Panel envelope (`PANEL_JSON_SCHEMA_VERSION`), RocksDB `PanelStore` |
| `materialize.rs` | — | — | `materialize_panel`, `vectors_from_panel`, `TimeStep` |
| `frozen_hook.rs` | — | — | `FrozenGuard`, `FrozenSnapshot`, `hash_f32s` — enforces zero-grad on frozen targets |
| `tests.rs` | — | — | 27+ unit tests |

Retired 2026-05-19 investigation cleanup: `e_coverage.rs`, `e_cyclomatic.rs`, `e_diff_topology.rs`, `e_embedder_heads.rs`, `composite_reward_signals.rs`, `static_analysis_reward.rs`, `telemetry_signals.rs`, `type_graph_signals.rs`, `live_session_signals.rs`, `linear_probe.rs`, and `stdlib_registry.rs`. These were unconsumed producer/helper surfaces after the Q4 freeze and binary Q1/Q2/Q5 contract. Keep `e_static_analysis.rs`, `e_code.rs`, `e_runtime.rs`, and the frozen `q4_*` label readers until the schema-version cleanup that drops deprecated-readable Q4 CFs.

**Planned (#512 / TASK-IP-002):** `ecc.rs` — self-dual doubly-even error-correcting code over the 15-slot occupancy bitmap. Defines `PARITY_CHECK_MATRIX`, `Syndrome`, `RecoveryRadius`. `PanelBuilder::build()` extended to call `panel.syndrome()` and fail-closed on `PANEL_ECC_UNCORRECTABLE`. Gates-style formal anti-compensation replacing the heuristic FSV in `python_anti_compensation_fsv.rs` (#394).

---

## 6. `context-graph-mejepa-train` Modules

`crates/context-graph-mejepa-train/src/lib.rs:11-52`.

| Module | Purpose |
|---|---|
| `aux/` | Auxiliary signals (Fisher snapshots, dormancy histories) |
| `bin/` | `mejepa-train` binary |
| `cert/` + `cert.rs` | `TrainingCertificate` → `CF_MEJEPA_TRAIN_CERTS` |
| `checkpoint.rs` | Checkpoint write/read |
| `cli/` | Train CLI subcommands |
| `compression_progress.rs` | TASK-EK-001 CPΦ reporter: conditional description length bits / window |
| `config.rs` | `TrainingConfig`, `LossCoefficients` (incl. `inverse_map_coefficient` per TASK-EK-012) |
| `dda.rs` | DDA feature computation (data-density-aware) |
| `distill/` | Distillation steps → `CF_MEJEPA_DISTILL_STEPS` |
| `error.rs` | `TrainerError`, `TrainerErrorCode` |
| `eval/` | Holdout evaluation, swebench-300x8 ship-gate eval |
| `labels.rs` (~29K) | Label assembly across all instrument outputs |
| `learning_signal.rs` (~33K) + `learning_signal/` | UTML `compute_l_step` — `ΔP` × `ΔK` × `ΔΩ` × `ΔΞ` components, `compute_per_head_learning_signal` |
| `lora_wiring.rs` | LoRA adapter wiring (cell weight transforms) |
| `loss/` | VICReg loss + Huber + inverse-map loss (`InverseMapLossBreakdown`); planned `loss/entropy.rs` for #511 / TASK-IP-001 (Kraskov-style differential entropy estimator on the late-fused latent — Vopson 2nd-law-of-infodynamics minimization) |
| `optim/` | Optimizer state |
| `replay_buffer.rs` (~20K) | TASK-RWD-210 / TASK-PY-G-068 surprise-prioritized M(t) replay buffer over `CF_MEJEPA_REPLAY_BUFFER`; `BatchSampler::next_replay_batch` |
| `sampler/` | Batch sampling strategies (uniform / replay / curiosity / operator-override) |
| `trainer/` + `trainer/InverseMapForward` | Trainer loop |

**FSV examples** declared in `Cargo.toml`: `train_smoke_fsv`, `train_cert_chain_fsv`, `train_holdout_eval_fsv`, `train_full_fsv`, `train_edge_cases_fsv`, `utml_l_step_fsv`, `swebench_300x8_ship_gate_fsv`, `operator_override_sampler_fsv`, `task_py_g_053_online_observe_fsv`, `task_ek_015_credibility_override_fsv`, `label_transfer_consumer_fsv`, `phase_g_adversarial_retrain_fsv`, `task_fp_101_sigreg_fsv`, `task_rwd_210_replay_buffer_fsv`, `task_ek_012_bidirectional_corpus_fsv`, `task_ek_012_bidirectional_ablation_fsv`, `task_py_g_007_pairwise_mi_fsv`.

---

## 7. Workspace Dependency Graph (internal only)

```
context-graph-mcp ─┬─> context-graph-mejepa
                   ├─> context-graph-mejepa-cf
                   ├─> context-graph-mejepa-train
                   ├─> context-graph-mejepa-tct
                   ├─> context-graph-mejepa-hygiene
                   ├─> context-graph-mejepa-shift-subscriber
                   ├─> context-graph-mejepa-instruments
                   ├─> context-graph-mejepa-corpus
                   ├─> context-graph-mejepa-embedders
                   ├─> context-graph-dynamicjepa
                   ├─> context-graph-storage ──> context-graph-core
                   ├─> context-graph-embeddings ──> context-graph-core, context-graph-cuda
                   ├─> context-graph-paths
                   └─> context-graph-witness ──> (ed25519, sha2, blake3)

context-graph-cli ─┬─> context-graph-mejepa (+ all of mcp's deps via re-export)
                   ├─> context-graph-mcp (client side)
                   └─> context-graph-paths

context-graph-mejepa ─┬─> context-graph-mejepa-cf
                      ├─> context-graph-mejepa-instruments ──> context-graph-mincut, context-graph-storage
                      ├─> context-graph-mejepa-embedders
                      ├─> context-graph-mejepa-corpus ──> context-graph-mejepa-instruments
                      ├─> context-graph-mejepa-tct ──> context-graph-storage
                      ├─> context-graph-mejepa-train ──> context-graph-mejepa-cf
                      ├─> context-graph-mejepa-hygiene ──> context-graph-mejepa-cf
                      ├─> context-graph-mejepa-shift-subscriber ──> context-graph-mejepa-instruments, context-graph-mejepa-cf
                      ├─> context-graph-mincut
                      ├─> context-graph-solver
                      ├─> context-graph-witness
                      ├─> context-graph-tier-compression
                      └─> context-graph-storage ──> context-graph-core

context-graph-mejepa-cf  — zero internal deps (intentional, prevents cycles)
context-graph-paths       — zero internal deps
context-graph-mincut      — zero internal deps
context-graph-solver      — zero internal deps
context-graph-tier-compression — zero internal deps
context-graph-witness     — minimal external deps (ed25519, sha2, blake3); no internal
context-graph-test-utils  — zero internal deps
```

**Acyclic check.** No internal cycles; `mejepa-cf` is intentionally dependency-free so MCP / hygiene / train / runtime crates can share CF name constants without circular dependency.

---

## 8. Binary Targets

| Binary | Crate | Entry | Purpose |
|---|---|---|---|
| `context-graph-mcp` | `context-graph-mcp` | `crates/context-graph-mcp/src/main.rs` (136K LOC) | MCP server (`--mode stdio` default, `--mode reality-loop` lightweight subset) |
| `context-graph-cli` | `context-graph-cli` | `crates/context-graph-cli/src/main.rs` | Operator CLI (primary) |
| `context-graph` | `context-graph-cli` `[[bin]]` | `crates/context-graph-cli/src/bin/context-graph.rs` | Operator CLI (short alias) |
| `mejepa` | `context-graph-mejepa` `[[bin]]` | `crates/context-graph-mejepa/src/bin/mejepa.rs` | Standalone predictor harness |
| `mejepa-heal-drill` | `context-graph-mejepa` `[[bin]]` | `crates/context-graph-mejepa/src/bin/mejepa_heal_drill.rs` | Heal scheduler drill |
| `mejepa-train` | `context-graph-mejepa-train` `[[bin]]` | `crates/context-graph-mejepa-train/src/bin/mejepa_train.rs` | Trainer driver |
| `train-causal` | `context-graph-embeddings` `[[bin]]` | (per Cargo.toml) | Causal embedder training |

---

## 9. Examples (210 example files, mostly FSV harnesses — partial inventory; full mapping in `07_results_and_benchmarks.md`)

Per-crate `[[example]]` declarations from each `Cargo.toml`:

| Crate | Notable examples (FSV harnesses, name → matching closed issue / task) |
|---|---|
| `context-graph-mcp` | `phase_f_gpu_wsl_fsv`, `task_py_g_063_project_ingest_fsv`, `task_py_g_064_project_report_fsv` |
| `context-graph-mejepa` | `mejepa-heal-drill`, `inference_fsv`, `phase5_self_healing_fsv`, `phase8_eval_fsv`, `utml_009_per_cell_state_transfer_fsv`, `phase_f_mi_heatmap_fsv`, `phase_f_per_head_calibration_fsv`, `continual_backprop_regulation_fsv`, `phase_f_toolchain_availability_fsv`, `phase_f_cross_language_label_gating_fsv`, `phase_f_system_cost_fsv`, `phase_f_adversarial_corpus_fsv`, `phase_f_pause_consumer_fsv`, `phase_f_pause_active_learning_fsv`, `phase_f_restart_safety_fsv`, `phase_f_multi_agent_fsv`, `phase_g_project_merkle_cache_fsv` (per recent commit), `task_py_g_056_live_session_fsv` (per recent commit), `task_py_g_015_gtau_guard_audit_fsv` (#130), `task_py_g_044_multi_head_contradiction_fsv` (#263) |
| `context-graph-mejepa-train` | (17 FSV examples) `train_*_fsv`, `utml_l_step_fsv`, `swebench_300x8_ship_gate_fsv`, `operator_override_sampler_fsv`, `task_py_g_053_online_observe_fsv`, `task_ek_015_credibility_override_fsv`, `label_transfer_consumer_fsv`, `phase_g_adversarial_retrain_fsv`, `task_fp_101_sigreg_fsv`, `task_rwd_210_replay_buffer_fsv`, `task_ek_012_bidirectional_corpus_fsv`, `task_ek_012_bidirectional_ablation_fsv`, `task_py_g_007_pairwise_mi_fsv` |
| `context-graph-mejepa-hygiene` | `merkle_compression_fsv`, `quota_overflow_fsv`, `gc_stress_fsv`, `phase_f_storage_exhaustion_fsv`, `phase_f_feedback_override_retention_fsv` |
| `context-graph-mejepa-shift-subscriber` | `shift_subscriber_fsv` |
| `context-graph-witness` | `ed25519_fsv` |

Full FSV evidence index (327 directories on `/mnt/d/contextgraph/fsv/`) → `07_results_and_benchmarks.md`.

---

## 10. CI Configuration

- `.github/workflows/` — GitHub Free plan budget: `runs-on: ubuntu-latest` only, `timeout-minutes: ≤30` default / `≤60` cap, Actions pinned to SHA, cron weekly for SAST / daily for stale (doctrine §3.20).
- `.github/labels.yml` + `scripts/gh-labels-bootstrap.sh` — label set (memory = Issues).
- `scripts/ci/check_no_c_drive_paths.sh` — lint blocking C-drive paths from being committed in code/FSV files.

---

## 11. What Is NOT Included

- No vendored upstream code. Clean-room only (per `CLAUDE.md §6.7`). Never vendor / path-dep / npm-depend on `ruvnet/agentdb` or `ruvnet/ruvector`.
- No backwards-compatibility migrations. Pre-production tree; wipe DB on schema change; fail fast with structured errors (`CLAUDE.md §6.6`).
- No inner-LLM code. Removed 2026-05-09; `feature = "llm"` triggers `compile_error!`.
- No HTTP from action paths. `WebSearch`/`WebFetch`/`mcp__exa__*` are read-only research tools only.
- No `.env` / `.env.*` files committed.

---

## 12. Modules for the Q1/Q2/Q5 binary contract + labeling + cross-panel + mistake loop

Source files supporting the 2026-05-19 binary doctrine (see `01_overview.md §1.1`, `05_predictor_pipeline.md §0`). Some are **shipped** (#413), some are **planned** (#404 / #405 / #406 / #407). All are **additive** to existing crates — no new top-level crate.

### 12.1 Failure-mode labeling hierarchy

**Level 0 (deterministic label compiler) — SHIPPED via #413 / TASK-PY-G-117 on 2026-05-19:**

| Module | Status | Role |
|---|---|---|
| `scripts/swebench_lite_ops/python_auto_label_discovery.py` (1,641 lines) | ✅ SHIPPED 2026-05-19 | Deterministic full-corpus label compiler. Emits chunk constellation labels, file-path + code-state constellations, failure evidence sets, usefulness metrics, accepted label registry, and the learning bridge manifest. No inner LLM. Slot identity preserved. Imports `python_constellation_training_objective` (normalize_category), `python_real_corpus_constellation_smoke` (forward-cache iterators + scalar feature extraction), `smoke_intelligence_matrices` (JSONL writers + row keys). |
| CLI entrypoint `python-auto-label-discovery` | ✅ SHIPPED 2026-05-19 | Registered through `scripts/swebench_lite_ops/cli.py`; the operator-facing surface for running the compiler against the aiwonder corpus. |
| FSV: `/zfs/archive/contextgraph/fsv/task-py-g-117-auto-label-discovery-usefulness-fsv/python_auto_label_discovery_fsv.json` | ✅ PASS (full corpus) | manifest sha256 `3cd0e3c6…`; full 399,777 chunks + 4,665,772 forward records + 491 accepted registry rows; 12/12 manual review reproductions matched |

**Level 1/2/3 hierarchical naming (consumes Level-0 registry) — open under #404 / TASK-PY-G-110:**

| Planned module | Crate | Role |
|---|---|---|
| `src/failure_mode_hierarchy.rs` | `context-graph-mejepa` | 4-level hierarchy types (`FailureModeGroupId`, `FailureModeSkillName`, `FailureModeConstellationName`, `NamedFailureMode`). Reads the 491-row accepted label registry from #413; aggregates Level-0 labels by co-occurrence into Level-1 groups, named via published SWE-bench taxonomies (Aleithan et al. 2025, Yang et al. 2024, SWE-Bench Pro 2025). |
| `src/failure_mode_clusters.rs` | `context-graph-mejepa` | Aggregation over the deterministic Level-0 label IDs (NOT a new K-means clustering pass — the K-means equivalent already lives in `python_auto_label_discovery.py` as the slot-region label emitter). |
| `src/failure_mode_vocabulary.rs` | `context-graph-mejepa` | Operator-curated failure-mode vocabulary seeded from the published taxonomies + our 8-mutation operator set + adversarial corpus. Names attach to Level-1 groups already discovered by #413. |
| `src/failure_mode_consumer.rs` | `context-graph-mejepa` | Reads aggregated Level-1/2/3 names, writes `CF_MEJEPA_FAILURE_MODE_LEVEL1/2/3`, exposes lookup for `compiler.rs::compile`. |
| `examples/task_py_g_110_failure_mode_hierarchy_fsv.rs` | `context-graph-mejepa` | FSV harness: every shipped Fail prediction in the holdout receives at least one named mode; operator-rename idempotence; named-mode → closest-exemplar oracle agreement ≥ 0.85. |

### 12.2 Cross-panel triangulation (epic #405 / TASK-PY-G-111)

| Planned module | Crate | Role |
|---|---|---|
| `src/cross_panel.rs` | `context-graph-mejepa` | Cross-panel orchestrator: schedules Panel B evaluation on gate-relevant predictions, writes `CF_MEJEPA_CROSS_PANEL_AGREEMENT`, raises `CROSS_PANEL_GOODHART_DETECTED` |
| `src/panel_b/` (new submodule) | `context-graph-mejepa-instruments` | Panel B instrument variants: `e_ast_codebert.rs` (CodeBERT), `e_cfg_pythia.rs` (Pythia program-graph), `e_dataflow_joern.rs` (joern), `e_code_voyage.rs` (Voyage-code-2), `e_text_bge.rs` (bge-large-en-v1.5). Deterministic slots (Trace/Witness/Oracle/StaticAnalysis/Runtime/Reasoning/Scalars) are shared from Panel A. |
| `examples/task_py_g_111_cross_panel_triangulation_fsv.rs` | `context-graph-mejepa` | FSV harness: both panels on same 30-instance holdout, per-instance Panel-A + Panel-B verdict in `CF_MEJEPA_CROSS_PANEL_AGREEMENT`, `CROSS_PANEL_GOODHART_DETECTED` fires on injected divergence |

### 12.3 Online mistake-driven learning (epic #406 / TASK-PY-G-112)

| Planned module | Crate | Role |
|---|---|---|
| `src/mistake_log.rs` | `context-graph-mejepa` | `MistakeRow` type, writer, secondary-index by `cell_key`, byte-replay readback for `CF_MEJEPA_MISTAKE_LOG` |
| `src/mistake_loop.rs` | `context-graph-mejepa` | Reads ground-truth verdicts from oracle / PostToolUse / shift-log; computes mistake / no-mistake; writes mistake row + boosts `CF_MEJEPA_REPLAY_BUFFER` priority 8× |
| `src/mistake_consumer.rs` | `context-graph-mejepa-train` | Trainer-side: consumes mistake rows under EWC++ Fisher protection (existing TASK-PY-G-065 path), applies Perceptron / Second-Order-Perceptron / Projectron update rule to the binary head, persists `CF_MEJEPA_ONLINE_HEAD_STATE` |
| `src/online_perceptron.rs` | `context-graph-mejepa-train` | Kernel-Perceptron family update; Cesa-Bianchi & Conconi 2005 mistake bound; Orabona 2008 Projectron memory-bounded support set; finite mistake bound guarantee |
| `examples/task_py_g_112_mistake_loop_convergence_fsv.rs` | `context-graph-mejepa-train` | FSV harness: inject deliberate wrong prediction → mistake row written → next presentation of same panel signature yields corrected verdict → `mistake_repeat_rate ≤ 0.05` |

### 12.4 Q4 freeze (deprecation, no new module)

Q4 is removed from the prediction contract (see `01_overview.md §1.1` and `05_predictor_pipeline.md §16`). No new Q4 modules are planned. Existing `q4_*_labels.rs` modules remain compiled and the CFs remain readable for forensic continuity; they are frozen — no new producer writes after 2026-05-19. Future cleanup work (deleting frozen Q4 modules + dropping CFs in a schema-version bump) tracked under `phase:cleanup`.

### 12.5 Dependency graph addendum

```
context-graph-mejepa (predictor trunk)
  + failure_mode_hierarchy.rs ─┐
  + failure_mode_clusters.rs   ├─> reads CF_MEJEPA_CONSTELLATION_PATTERNS (#390), CF_MEJEPA_PATCH_SIMILARITY_META; writes CF_MEJEPA_FAILURE_MODE_LEVEL1/2/3 + CF_MEJEPA_NAMED_PREDICTIONS
  + failure_mode_vocabulary.rs ┘
  + cross_panel.rs ────────────> reads CF_MEJEPA_LIVE_PREDICTIONS; writes CF_MEJEPA_CROSS_PANEL_AGREEMENT, CF_MEJEPA_PANEL_B_OBSERVATIONS
  + mistake_log.rs ────────────> reads CF_MEJEPA_ORACLE_VERDICTS + CF_MEJEPA_PREDICTION_VERIFICATIONS + CF_MEJEPA_REALITY_IMPACT; writes CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_MISTAKE_REPLAY_PRIORITY
  + mistake_loop.rs ───────────> bridges mistake_log → replay_buffer (boosts CF_MEJEPA_REPLAY_BUFFER priority 8×)

context-graph-mejepa-instruments
  + panel_b/ ──────────────────> 5 new Panel B encoders (CodeBERT / Pythia / joern / Voyage-code-2 / bge)

context-graph-mejepa-train
  + mistake_consumer.rs ───────> consumes CF_MEJEPA_MISTAKE_LOG under EWC++ Fisher protection
  + online_perceptron.rs ──────> Perceptron / Second-Order-Perceptron / Projectron head update; CF_MEJEPA_ONLINE_HEAD_STATE
```

No new internal dependency cycles; the planned modules attach to existing crates and read/write only via `context-graph-mejepa-cf` constants and `context-graph-storage` openers.
