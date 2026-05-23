# 02 — Source Code Map

## ⚠ 2026-05-22 hardening pivot — audit-uncovered defects (all open P0/P1)

| Issue | File / Module | Defect |
|---|---|---|
| #618 | `crates/context-graph-mejepa/src/compiler.rs` aggregate OOD path | Reads flat 5,120-byte panel buffer (doctrine §1.5ter violation) |
| #619 | `crates/context-graph-mejepa-train/src/loss/inverse.rs::forward_inverse_inner` | `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter violation) |
| #620 | `crates/context-graph-mejepa-train/` | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook never invoked from training |
| #621 | `crates/context-graph-mejepa-train/` HoldoutReport publisher | Hardcoded `gtau_pass_rate: 1.0` published to MCP weekly dashboard |
| #622 | `crates/context-graph-mejepa-train/src/{distill,lora_wiring,aux}.rs` | Trainer dead-code batch |
| #623 | `crates/context-graph-mejepa/src/conformal.rs::cold_start_conformal_policy` | Emits `alpha=0.0` (invalid input to `calibrate_tau`) — dead public API |
| #624 | `crates/context-graph-mejepa/src/verdict_assembly.rs` | Inline `CONTRADICTION_CALIBRATION_INVALID` not a declared constant |
| #625 | `crates/context-graph-mejepa/src/lib.rs` | `DeterministicPredictor` publicly re-exported (synthetic-vector landmine post-#422) |
| #626 | `config/retention_policy.toml` + `crates/context-graph-mejepa-hygiene/` | Default policy missing ~90 CFs |
| #627 | `crates/context-graph-mejepa-shift-subscriber/src/runner.rs` | `run_until_idle_consumes_jsonl` test fails on main (missing TCT centroid scaffold) |
| #628 | `crates/context-graph-mejepa-tct/src/gtau.rs` vs docs | Error string drift: docs `MEJEPA_TCT_DIM_MISMATCH` vs emitted `MEJEPA_INFER_DIM_MISMATCH` |
| #670 | `crates/context-graph-mejepa-embedders/` `e2/e3/e4/e8` adapters | Forward-cache embedder vector-space collapse — regenerate or retire |

**Operational change (#655, in-progress p0):** the `crates/*/examples/*_fsv.rs` ecosystem is being retired in favour of manual operator-driven SoT readback (per `06 §1.9`). The 210 example FSVs in §9 remain for forensic reference.

**Geometry-label epic (#657 / TASK-PY-G-136, in-progress):** per-embedder vector-space label discovery via Leiden-on-kNN / HDBSCAN / spherical k-means / UMAP-diagnostic bakeoff. Planned modules: `crates/context-graph-mejepa-train/src/geometry_label_discovery/` + `crates/context-graph-mejepa/src/predictor/geometry_label_router.rs` (#664) + `crates/context-graph-mejepa/src/predictor/geometry_label_heads.rs` (#665). Output: `/zfs/archive/contextgraph/state/labels/task-py-g-136-per-embedder-geometry-labels-v1/`.

---

## ⚠ Synthetic-vector remediation (2026-05-19/20) — fixed-module index

| Module | Pre-fix | Fix |
|---|---|---|
| `crates/context-graph-mejepa/src/compiler.rs::vectors_for` | SHA256-seeded synthetic vectors | Replaced by `build_inference_panel_inputs` calling real `E*Instrument` encoders (commit `e2c51669`) |
| `crates/context-graph-mejepa/src/project_cache.rs` | Wrote `deterministic_vector_blob` as embeddings | Fails closed (commit `5330629e`) |
| `crates/context-graph-mejepa/src/predictor_checkpoint.rs` | Accepted `MEJEPA_BEST_STUB_V1` text-stubs | Rejects at load + export (`:147-158, 239-250`) |
| `crates/context-graph-mejepa-train` | No real predictor weight optimization | Now owns real `MeJepaPredictor` weights; exports manifest-verified checkpoints (commit `db83923a`) |

**`crates/context-graph-mejepa-embedders/` forward-cache pipeline was unaffected** — fail-closed guards in `src/forward.rs:122,207,349,422` ("no deterministic vector fallback is permitted") prevented contamination.

See `01 §1.7` for forensic and `08 §0.5` for rebaseline blocker.

**Source files:** `Cargo.toml`, every `crates/*/Cargo.toml`, every `crates/*/src/lib.rs`, every `crates/*/src/main.rs`, every `[[bin]]` and `[[example]]` declaration.

**Totals:** 1,600 src/lib `.rs` files · 210 examples · 258 test files.

---

## 1. Workspace Manifest

`Cargo.toml:6-30` lists 22 active workspace members. Workspace package: `version = "0.1.0"`, `edition = "2021"`, `rust-version = "1.95"`, `license = "PolyForm-Noncommercial-1.0.0"`.

Workspace deps: tokio 1.35, serde 1.0, serde_json 1.0, bincode 1.3, toml 0.8, thiserror 1.0, anyhow 1.0, tracing 0.1 + tracing-subscriber 0.3, config 0.14, uuid 1.6, chrono 0.4, rand 0.8, rand_distr 0.4, candle-core 0.10.2 (cuda), candle-nn 0.10.2 (cuda).

Build profiles: `release` (thin LTO, 16 CGUs, strip), `release-max` (1 CGU), `dev` (opt 0, line-info debug, incremental).

---

## 2. Per-Crate Inventory

| # | Crate | Purpose | Key submodules |
|---|---|---|---|
| 1 | `context-graph-mcp` | MCP JSON-RPC stdio + HTTP, 216 tools, daemon, deprecation gate | `adapters/`, `daemon.rs`, `daemon_validate.rs`, `deprecation.rs`, `handlers/`, `health_probe.rs`, `monitoring.rs`, `protocol.rs`, `server/`, `telemetry/`, `tools/`, `weights.rs` |
| 2 | `context-graph-cli` | Operator CLI + helpers | `commands/`, `error.rs`, `governed_edit.rs`, `mcp_client.rs`, `mcp_helpers.rs` |
| 3 | `context-graph-core` | Shared domain types (`CodeEntity`, `CodeLanguage`, etc.) | `types/` |
| 4 | `context-graph-storage` | RocksDB wrapper + 19 teleological + 13 quantized + 5 code + 2 causal CFs | `code/`, `column_families.rs`, `dynamicjepa/`, `graph_edges/`, `indexes.rs`, `teleological/` |
| 5 | `context-graph-witness` | ed25519, Merkle, Stage 0/F certs | `cert.rs`, `granger.rs`, `causal_mask.rs` |
| 6 | `context-graph-solver` | Causal masking, MMR, conformal predictor | `causal_mask.rs`, `granger.rs` |
| 7 | `context-graph-mincut` | Clean-room Stoer-Wagner global min-cut | (single-file lib) |
| 8 | `context-graph-paths` | Fail-closed durable path resolver; rejects non-aiwonder prod targets | (single-file lib) |
| 9 | `context-graph-tier-compression` | 8/7/5/3-bit groupwise scalar quantization | (single-file lib) |
| 10 | `context-graph-embeddings` | E1-E14 pipeline, model loaders | `train-causal` bin |
| 11 | `context-graph-cuda` | CUDA helper kernels | — |
| 12 | **`context-graph-mejepa`** | **Predictor trunk** (73 modules) | see §3 |
| 13 | `context-graph-mejepa-cf` | Canonical CF name registry (~120 ME-JEPA CFs; zero internal deps) | (single-file lib) |
| 14 | `context-graph-mejepa-corpus` | 8× mutation operators + SWE-bench bridge + split planner | see §4 |
| 15 | `context-graph-mejepa-instruments` | 15 frozen instrument slots; `Panel` is array of 15 per-embedder vectors | see §5 |
| 16 | `context-graph-mejepa-embedders` | 21 embedder slots, dynamic registry, calibration | `cache*.rs`, `calibration.rs`, `dynamic_registry.rs`, `embedder_id.rs`, `forward.rs`, `routing.rs`, `vram.rs` |
| 17 | `context-graph-mejepa-train` | UTML trainer, EWC, CBP, replay buffer, certificates, mistake loop | see §6 |
| 18 | `context-graph-mejepa-tct` | TCT constellations + Gτ guard + freshness | `builder.rs`, `calibrator.rs`, `constellation.rs`, `freshness*.rs`, `gtau.rs`, `guard_audit.rs`, `store.rs`, `verdict.rs` |
| 19 | `context-graph-mejepa-hygiene` | Tier transitions, quota, GC, witness compress | `categories.rs`, `gc.rs`, `quota.rs`, `retention.rs`, `tier*.rs`, `witness_compress.rs` |
| 20 | `context-graph-mejepa-shift-subscriber` | JSONL shift-log tail + watermark + cache → panels | `cache.rs`, `runner.rs`, `shift_transform.rs`, `subscriber.rs`, `tail.rs`, `watermark.rs` |
| 21 | `context-graph-dynamicjepa` | 5090jepa MCP surface (39 tools) | (multiple modules) |
| 22 | `context-graph-test-utils` | Deterministic-by-shape, pseudo-random-by-content helpers for storage/serialization/shape tests only; not ME-JEPA gate evidence | — |

---

## 3. `context-graph-mejepa` Modules (73 files; predictor trunk)

| Module | Purpose |
|---|---|
| `compiler.rs` (67K) | `MeJepaCompiler::compile` / `compile_with_fingerprint_observation` — main Q1-Q5 entry. Reads panel, runs predictor, applies conformal + Gτ + Cold-cell metric, assembles `RealityPrediction` |
| `predictor/` | `MeJepaPredictor` (slot-preserving per-slot encoders + late fusion), `TransformerLayer`, `ArchitectureSummary` |
| `verdict_assembly.rs` | Stitches heads → `Verdict { Pass / Fail / OutOfDistribution / Abstain / GuardRejected }`; product surface returns Pass/Fail only |
| `claim_graph.rs` (~30K) | Q1 claim parser + AST/bytes-on-disk reconciliation → `CF_MEJEPA_CLAIM_RECONCILIATION` |
| `patch_similarity.rs` (~28K) | Q3 closest exemplars: usearch HNSW over `(E_AST, E_CFG, E_DataFlow, E_TypeGraph)` signatures |
| `head_projection.rs` | Per-head DDA slice projection |
| `contradiction.rs` | Per-cell multi-head contradiction thresholds, fail-closed detector |
| `failure_fingerprint.rs` (~75K) | First-class failure-shape fingerprint catalog |
| `adversarial_corpus.rs` | 30-case adversarial Python corpus |
| `synthetic_stress*.rs` | 30-case synthetic Python stress eval |
| `project_ingest.rs` + `project_cache.rs` + `project_report.rs` + `project_stress.rs` | Per-project reality compiler |
| `live_session_trace.rs` | End-to-end live Claude Code session trace |
| `reality_impact.rs` | Q5 shift-log replay; `RealityImpactRecord` rows |
| `prediction_replay.rs` + `prediction_surfaces.rs` | Q1 surface + replay surfaces |
| `dynamic_embedder*.rs` + `embedder_falsification.rs` + `embedder_proposal.rs` + `adversarial_embedder_injection.rs` | Dynamic-embedder runtime + freeze + falsification + VRAM budget + injection mitigation |
| `chunk_foundationality.rs` (~90K) + `library_foundationality.rs` + `embedder_foundationality.rs` | PageRank on chunk/library/embedder dependency graphs |
| `entity_kge_embedder.rs` (~40K) | TransE on corpus entity graph |
| `algorithmic_embedder_synthesis.rs` + `learned_head_synthesis.rs` | Synthesis loops |
| `instrument_proposal.rs` | Propose instrument from absence-detection |
| `mincut_panel.rs` (~48K) | Stoer-Wagner on embedder/constellation graph |
| `pairwise_mi.rs` | Pairwise MI matrix audit |
| `latent_search.rs` | Latent-action best-of-K (debug/training-time only) |
| `candidate_ranking.rs` | Counterfactual repair ranking |
| `objective_safety.rs` | Hardwired objective + safety constraints |
| `hierarchical.rs` | Durable file/function/AST/chunk prediction hierarchy → `CF_MEJEPA_HIERARCHICAL_PREDICTIONS` |
| `cross_language_label_gating.rs` | Phase F cross-language label gate |
| `system_cost.rs` (~23K) + `toolchain_detect.rs` + `degraded.rs` + `pause_state.rs` | Phase F operations |
| `conformal.rs` + `calibration*.rs` + `ood.rs` | Split-conformal predictor + per-head calibration + OOD residual score math |
| `ood_harvest.rs` + `ood_harvest/` | Live-session OOD harvest + serving calibrator |
| `frozen_target.rs` + `grad_hook.rs` | Frozen-target adapter + gradient leak guard (`MEJEPA_INSTRUMENT_GRADIENT_LEAK`) |
| `evidence.rs` + `fsv_writer.rs` | FSV evidence builders |
| `operator_override.rs` + `operator_contribution.rs` (~29K) | Operator override + contribution surface |
| `park_list.rs` | Fail-closed retry park list → `CF_MEJEPA_PARK_LIST` |
| `reward_signal_audit.rs` (~32K) + `sampler_reward.rs` (~20K) | Reward-signal audit + online observe reward |
| `secret_redaction.rs` | Wrapper around `scripts/swebench_lite_ops/python_secret_redaction.py` |
| `label_transfer_audit.rs` | Durable label transfer audit |
| `heal/` | Heal scheduler (5 modules) |
| `eval/` | Eval reports / dashboards |
| `loss/` | VICReg loss + Huber + inverse-map loss |
| `cli.rs` + `verify_cli.rs` | CLI subcommands |
| `types.rs` (114K, largest) | `RealityPrediction`, `Verdict`, `RealityImpact`, `OracleScores`, all DTOs |
| `store.rs` | `MejepaStore` — RocksDB handle |

**Planned:**
- `pathway/{tree,beam_search,leaf_predictor,exemplar_attach,skill_attach,store}.rs` (#416) — binary-leaf butterfly-effect pathway tree
- `cross_panel.rs` + `panel_b/` submodule (#405) — Panel B Goodhart guard
- `embedder_growth.rs` (#516) + `embedder_retirement.rs` (#518) — post-0.95 creativity engine

---

## 4. `context-graph-mejepa-corpus` Modules

| Module | Purpose |
|---|---|
| `compile_error.rs` | Mutation op #8: Python syntax error |
| `delete_test_call.rs` | Mutation op #5: delete one assertion call |
| `generate.rs` | Corpus generator + `SwebenchDockerOracle` wrapper |
| `generic_language_ops.rs` | 10 non-Python languages (deferred from current ship gate) |
| `known_good.rs` | Mutation op #1: echo input unchanged |
| `multilang_fixture_corpus.rs` | Multi-language fixture corpus |
| `off_by_one.rs` | Mutation op #3: ±1 to one numeric literal |
| `oracle.rs` | `OracleStore` — RocksDB corpus + verdict store |
| `over_engineer.rs` | Mutation op #7: append unused helper |
| `patch_mutation.rs` | Apply patch text to base source |
| `prng.rs` | `SplitMix64` deterministic site selection |
| `python_lex.rs` | Triple-quote / f-string-aware Python lex mask |
| `source_patch.rs` + `source_patch_support.rs` | Source-backed patch builder |
| `split.rs` + `split_tests.rs` | `SplitMode { InstanceAtomic (default), RepoAtomic }`; LPT pre-sort + bin-pack; `assert_no_test_patch_leakage` |
| `subtle_flip.rs` | Mutation op #2: invert one boolean operator |
| `swap_variable.rs` | Mutation op #4: rename one local |
| `swebench.rs` | `run_swebench_lite_oracle()` — `python -m swebench.harness.run_evaluation` wrapper |
| `timed_subprocess.rs` | Subprocess timeout |
| `util.rs` + `wrong_file.rs` | Helpers + Mutation op #6 |
| `tests.rs` | 81+ unit tests |
| `bin/` | Corpus generator binaries |

---

## 5. `context-graph-mejepa-instruments` Modules

`Panel` is array of 15 per-embedder vectors. 5,120 f32 contiguous buffer is byte layout only. Cross-embedder dim-wise compare forbidden.

| Module | Slot served | Dim | Notes |
|---|---|---|---|
| `e_oracle.rs` | `EOracle` | 128 | Pass/fail/skip/error fractions + total + has_exception + 10-class one-hot |
| `e_witness.rs` | `EWitness` | 256 | Witness-chain entry sequence encoder |
| `e_static_analysis.rs` | `EStaticAnalysis` | 256 | Linter / typecheck / complexity aggregates |
| `e_code.rs` | `EAst`, `ECfg`, `EDataFlow`, `ETypeGraph`, `EDiff` | 384+256+256+256+256 | Code-side encoders |
| `e_text.rs` | `ETest`, `EProblem`, `ECommitMsg` | 384+1024+384 | E1-reuse text encoders |
| `e_trace.rs` | `ETrace` | 512 | Execution-trace sequence transformer |
| `e_runtime.rs` | `ERuntime` | 256 | Wall-clock / RSS / coverage / network |
| `e_reasoning.rs` | `EReasoning` | 384 | Reasoning / reflexion trace |
| `e_scalars.rs` | `Scalars` | 128 | BFS depth, blame age, churn, coverage Δ, repo-health |
| `q4_security_labels.rs` | (frozen display-only) | — | Bandit + Semgrep label pipeline; Q4 read-only after ambiguity-boundary freeze |
| `panel_json.rs` + `panel_store.rs` | — | — | Panel envelope (`PANEL_JSON_SCHEMA_VERSION`), RocksDB `PanelStore` |
| `materialize.rs` | — | — | `materialize_panel`, `vectors_from_panel`, `TimeStep` |
| `frozen_hook.rs` | — | — | `FrozenGuard`, `FrozenSnapshot`, `hash_f32s` — enforces zero-grad on frozen targets |
| `tests.rs` | — | — | 27+ unit tests |

**Planned:**
- `ecc.rs` (#512) — self-dual doubly-even ECC over 15-slot occupancy bitmap. Gates-style formal anti-compensation; `PARITY_CHECK_MATRIX`, `Syndrome`, `RecoveryRadius`.
- `domain_routing.rs` (#519) — domain detector + per-domain panel manifest + sparse fill routing (post-0.95).

Q4 producer modules and unconsumed TASK-RWD signals retired; only frozen display-only label readers remain.

---

## 6. `context-graph-mejepa-train` Modules

| Module | Purpose |
|---|---|
| `aux/` | Auxiliary signals (Fisher snapshots, dormancy histories) |
| `bin/` | `mejepa-train` binary |
| `cert/` + `cert.rs` | `TrainingCertificate` → `CF_MEJEPA_TRAIN_CERTS` |
| `checkpoint.rs` | Checkpoint write/read |
| `cli/` | Train CLI subcommands |
| `compression_progress.rs` | CPΦ reporter: conditional description length bits / window |
| `config.rs` | `TrainingConfig`, `LossCoefficients` |
| `dda.rs` | DDA feature computation |
| `distill/` | Distillation steps → `CF_MEJEPA_DISTILL_STEPS` |
| `eval/` | Holdout evaluation, swebench-300x8 ship-gate eval |
| `labels.rs` (~29K) | Label assembly across instrument outputs |
| `learning_signal.rs` (~33K) + `learning_signal/` | UTML `compute_l_step` — `ΔP × ΔK × ΔΩ × ΔΞ` |
| `lora_wiring.rs` | LoRA adapter wiring |
| `loss/` | VICReg + Huber + inverse-map loss |
| `optim/` | Optimizer state |
| `replay_buffer.rs` (~20K) | Surprise-prioritized M(t) replay; label/skill-aware priority rows |
| `sampler/` | Batch sampling strategies (uniform / replay / curiosity / operator-override) |
| `trainer/` | Trainer loop |
| **`mistake_log.rs` + `online_head_state.rs` + `online_head_state_keys.rs` + `online_head_state_support.rs`** | **Mistake loop (#406, shipped):** `MistakeLogRow`, byte-replay readback; `protection_mode=exactPanelKeyIsolation` |
| `ability_resolver_refutation.rs` + `ability_resolver_types.rs` + `ability_resolver_storage.rs` | Live consumer bridge: ability refutations write mistake + replay + lifecycle audit + online-head rows |
| `skill_sequence_discovery.rs` + `skill_sequence_types.rs` + `chunk_skill_membership.rs` + `skill_validation.rs` | Skill substrate (#414, shipped) |
| `failure_mode_hierarchy.rs` + `skill_corpus_materialization.rs` | Level-1/2/3 hierarchy + named prediction rows (#404, shipped) |

**Planned:**
- `loss/entropy.rs` (#511) — Kraskov-style differential entropy estimator on late-fused latent; Vopson 2nd-law minimization
- `embedder_demand.rs` (#517) — mistake-loop residual-signature emission when no slot can explain disagreement (post-0.95)

**FSV examples:** `train_smoke_fsv`, `train_cert_chain_fsv`, `train_holdout_eval_fsv`, `train_full_fsv`, `train_edge_cases_fsv`, `utml_l_step_fsv`, `swebench_300x8_ship_gate_fsv`, `operator_override_sampler_fsv`, `task_py_g_053_online_observe_fsv`, `task_ek_015_credibility_override_fsv`, `label_transfer_consumer_fsv`, `phase_g_adversarial_retrain_fsv`, `task_fp_101_sigreg_fsv`, `task_rwd_210_replay_buffer_fsv`, `task_ek_012_bidirectional_corpus_fsv`, `task_ek_012_bidirectional_ablation_fsv`, `task_py_g_007_pairwise_mi_fsv`, `task_py_g_110_failure_mode_hierarchy_fsv`, `task_py_g_112_mistake_loop_convergence_fsv`, `task_py_g_118_skill_sequence_discovery_fsv`, `task_py_g_118_aiwonder_skill_materialization_fsv`.

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

context-graph-cli ─┬─> context-graph-mejepa
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
                      └─> context-graph-storage

# Zero internal deps (intentional, prevents cycles)
context-graph-mejepa-cf, context-graph-paths, context-graph-mincut, context-graph-solver,
context-graph-tier-compression, context-graph-witness, context-graph-test-utils
```

**Acyclic check.** No internal cycles; `mejepa-cf` is intentionally dependency-free so MCP / hygiene / train / runtime crates can share CF name constants.

---

## 8. Binary Targets

| Binary | Crate | Entry | Purpose |
|---|---|---|---|
| `context-graph-mcp` | `context-graph-mcp` | `src/main.rs` (136K LOC) | MCP server (`--mode stdio` default, `--mode reality-loop` subset) |
| `context-graph-cli` | `context-graph-cli` | `src/main.rs` | Operator CLI (primary) |
| `context-graph` | `context-graph-cli` `[[bin]]` | `src/bin/context-graph.rs` | Operator CLI (short alias) |
| `mejepa` | `context-graph-mejepa` `[[bin]]` | `src/bin/mejepa.rs` | Standalone predictor harness |
| `mejepa-heal-drill` | `context-graph-mejepa` `[[bin]]` | `src/bin/mejepa_heal_drill.rs` | Heal scheduler drill |
| `mejepa-train` | `context-graph-mejepa-train` `[[bin]]` | `src/bin/mejepa_train.rs` | Trainer driver |
| `train-causal` | `context-graph-embeddings` `[[bin]]` | per Cargo.toml | Causal embedder training |

---

## 9. Examples (210 example files — mostly FSV harnesses)

Per-crate `[[example]]` declarations include:

- `context-graph-mcp` — `phase_f_gpu_wsl_fsv`, `task_py_g_063_project_ingest_fsv`, `task_py_g_064_project_report_fsv`, `task_py_g_112b_mistake_loop_mcp_tools_fsv`
- `context-graph-mejepa` — `inference_fsv`, `phase5_self_healing_fsv`, `phase8_eval_fsv`, `phase_f_*_fsv` (~15 harnesses), `task_py_g_015_gtau_guard_audit_fsv`, `task_py_g_044_multi_head_contradiction_fsv`, `task_py_g_056_live_session_fsv`, `task_py_g_086_constellation_observation_schema_fsv`, `task_py_g_087_slot_preserving_late_fusion_predictor_fsv`, `task_py_g_088_cross_embedder_correlation_tensors_fsv`, `task_py_g_089_per_slot_calibration_ood_fsv`, `task_py_g_090_consensus_contradiction_heads_fsv`, `task_py_g_091_slot_attribution_explanation_fsv`, `task_py_g_094_anti_compensation_fsv`, `task_py_g_117_auto_label_discovery_fsv`, `task_py_g_121_runtime_ability_resolver_fsv`, `q4_doctrine_freeze_fsv`
- `context-graph-mejepa-train` — `task_py_g_110_failure_mode_hierarchy_fsv`, `task_py_g_112_mistake_loop_convergence_fsv`, `task_py_g_118_skill_sequence_discovery_fsv`, `task_py_g_118_aiwonder_skill_materialization_fsv`, 17+ trainer FSVs
- `context-graph-mejepa-hygiene` — `merkle_compression_fsv`, `quota_overflow_fsv`, `gc_stress_fsv`
- `context-graph-mejepa-shift-subscriber` — `shift_subscriber_fsv`
- `context-graph-witness` — `ed25519_fsv`

Full FSV evidence index → `07_results_and_benchmarks.md`.

---

## 10. CI Configuration

- `.github/workflows/` — GitHub Free plan budget: `runs-on: ubuntu-latest` only, `timeout-minutes: ≤30` default / `≤60` cap, Actions pinned to SHA, cron weekly for SAST / daily for stale.
- `.github/labels.yml` + `scripts/gh-labels-bootstrap.sh` — label set (memory = Issues).
- `scripts/ci/check_no_c_drive_paths.sh` — blocks C-drive paths in code/FSV files.
- `scripts/ci/phase_g_*.sh` — operator-invoked Phase G audit harnesses. Phase H's previously documented `phase_h_*.sh` absence-check suite is missing; #693 tracks whether to create it or remove the historical claim.

---

## 10.5 Neuroscience-Inspired Reserve — Planned Modules (DO NOT START until current queue concludes)

8 contingency issues (#538-#545) at `priority:p3` with `⚠ DO NOT START` markers. See `08 §6.9`.

**Tier 1 planned modules:**

| Issue | Planned crate / module path | Tied existing crate |
|---|---|---|
| #538 Predictive-coding head | `crates/context-graph-mejepa/src/predictor/top_down_prediction.rs` + `crates/context-graph-mejepa-train/src/loss/predictive_coding.rs` | `context-graph-mejepa`, `context-graph-mejepa-train` |
| #539 Schema-prioritized replay | `crates/context-graph-mejepa-train/src/replay_scheduler.rs` + `reverse_replay.rs` + `generative_replay.rs` | `context-graph-mejepa-train` (extends `replay_buffer.rs`) |
| #540 Hierarchical predictive coding | `crates/context-graph-mejepa/src/hierarchical_prediction.rs` + `crates/context-graph-mejepa-train/src/loss/hierarchical_predictive.rs` | consumes shipped #404 / #414 substrate |
| #541 Mixed-selectivity binding | `crates/context-graph-mejepa/src/predictor/mixed_selectivity_binding.rs` + `binding_basis_audit.rs` | extends `predictor/` and #390 surface |

**Tier 2 design influence (no separate modules):**
- #542 → #416 `pathway/` design (manifold trajectories option)
- #543 → #519 sparse-panel design + per-instrument top-k gates
- #544 → #392 temporal-feature design + #390 synchrony scoring
- #545 → background tokio task composing with #416 / #517 / #406 / #520

All 8 verified doctrine-compatible (slot identity, anti-compensation, binary contract, ambiguity boundary, no inner LLM, no synthetic shortcuts) before filing.

---

## 11. What Is NOT Included

- No vendored upstream code. Clean-room only (`CLAUDE.md §6.7`). Never vendor / path-dep / npm-depend on `ruvnet/agentdb` or `ruvnet/ruvector`.
- No backwards-compatibility migrations. Pre-production tree; wipe DB on schema change; fail fast with structured errors.
- No inner-LLM code. `feature = "llm"` triggers `compile_error!`.
- No HTTP from action paths. `WebSearch`/`WebFetch`/`mcp__exa__*` are read-only research tools only.
- No `.env` / `.env.*` files committed.

---

## 12. New CFs for Q1/Q2/Q5 binary contract + labeling + cross-panel + mistake loop

All shipped artifacts are additive to existing crates — no new top-level crate.

### 12.1 Failure-mode labeling (#404, #413, #414, shipped)

| Layer | Module | Status |
|---|---|---|
| Level 0 deterministic label compiler | `scripts/swebench_lite_ops/python_auto_label_discovery.py` (1,641 lines, deterministic Python, no inner LLM); CLI `python-auto-label-discovery` | ✅ SHIPPED (#413) |
| Level 1/2/3 hierarchy + named predictions | `src/failure_mode_hierarchy.rs`, `src/skill_corpus_materialization.rs` in `mejepa-train` | ✅ SHIPPED (#404) |
| Skill substrate (memberships + reverse + lifecycle) | `src/skill_sequence_discovery.rs`, `src/chunk_skill_membership.rs` in `mejepa-train` | ✅ SHIPPED (#414) |
| Runtime hierarchical ability resolver | `src/ability_resolver_*.rs` in `mejepa-train` | ✅ SHIPPED (#418) |

### 12.2 Cross-panel triangulation (#405, planned, blocked p0)

| Planned module | Crate | Role |
|---|---|---|
| `src/cross_panel.rs` | `context-graph-mejepa` | Schedules Panel B evaluation, writes `CF_MEJEPA_CROSS_PANEL_AGREEMENT`, raises `CROSS_PANEL_GOODHART_DETECTED` |
| `src/panel_b/` submodule | `context-graph-mejepa-instruments` | Panel B variants: `e_ast_codebert.rs`, `e_cfg_pythia.rs`, `e_dataflow_joern.rs`, `e_code_voyage.rs`, `e_text_bge.rs` |
| `examples/task_py_g_111_cross_panel_triangulation_fsv.rs` | `context-graph-mejepa` | FSV harness |

### 12.3 Online mistake-driven learning (#406 / #523, shipped)

| Module | Crate | Role |
|---|---|---|
| `src/mistake_log.rs` | `mejepa-train` | `MistakeLogRow`, writer, validation, byte-replay readback for `CF_MEJEPA_MISTAKE_LOG` |
| `src/online_head_state*.rs` (3 files) | `mejepa-train` | Key-local exact-panel online-head state; `protection_mode=exactPanelKeyIsolation` |
| `src/ability_resolver_refutation*.rs` | `mejepa-train` | Live consumer bridge |
| `src/handlers/tools/mejepa_mistake_loop_tools.rs` | `mejepa-mcp` | MCP wrappers: `mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status` |

### 12.4 Q4 freeze (deprecation, no new module)

Q4 removed from prediction contract. No new Q4 modules planned. Existing `q4_*_labels.rs` modules remain compiled and CFs remain readable for forensic continuity; frozen — no new producer writes.

### 12.5 Dependency graph addendum

```
context-graph-mejepa (predictor trunk)
  + failure_mode_hierarchy.rs ─> writes CF_MEJEPA_FAILURE_MODE_LEVEL{1,2,3}_* + CF_MEJEPA_NAMED_PREDICTIONS
  + cross_panel.rs (planned)   ─> writes CF_MEJEPA_CROSS_PANEL_AGREEMENT, CF_MEJEPA_PANEL_B_OBSERVATIONS
  + mistake_log.rs             ─> writes CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_MISTAKE_REPLAY_PRIORITY
  + online_head_state.rs       ─> writes CF_MEJEPA_ONLINE_HEAD_STATE
  + ability_resolver_refutation -> live refutation consumer writes lifecycle + mistake + replay + online-head rows

context-graph-mejepa-instruments
  + panel_b/ (planned)         ─> 5 new Panel B encoders
```

No new internal dependency cycles; planned modules attach to existing crates and read/write only via `context-graph-mejepa-cf` constants and `context-graph-storage` openers.
