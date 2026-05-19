# Investigation 2 — Tests, Examples & Benchmarks Dead Code

**Investigator:** Sherlock Holmes Agent #2
**Date:** 2026-05-19
**Scope:** `crates/*/tests/**`, `crates/*/examples/**`, repo-root `tests/`, `tasks/`, `crates/context-graph-benchmark/**`. Crate `src/`, top-level `scripts/`, top-level `docs/` excluded (parallel investigators).

**Verdict-grade summary.** Two surfaces dominate the dead-code mass: (1) the 26-file 16,772-LOC `blackbox_phase*_fsv` test suite in `crates/context-graph-cli/tests/` exercising an enterprise SaaS scaffold (HTTP/k8s/terraform/OTEL/SDK ingest/dashboard/scorecard/release gate/pilot package/risk register/ticketing/release/privacy retention) that has zero references in `docs/futurebuild/`, no open or closed issues mentioning "blackbox", and last activity ~2 weeks ago; (2) ~146 examples that hardcode `/mnt/d/contextgraph/...` FSV roots with no env override and no `/zfs/archive` fallback, all of which require revalidation per closed-decision #363 (aiwonder-durable-SoT) and tracked under #369. Q4 examples (9 files) are correctly frozen post-#408 and should be kept as forensic continuity. Tests using `Hermes-2-Pro` strings are validator-version labels in storage round-trip tests, not active inner-LLM calls. Several `examples/*.rs` files in `context-graph-core` are scratch smokes with zero assertions and produce no FSV evidence file.

---

## §1 Method

LSU compliance — sources examined in this order BEFORE forming a verdict:

1. `docs/futurebuild/README.md` (binary doctrine reframe header, current ship-gate status)
2. `docs/futurebuild/01_overview.md` (read by line range only — file > 25k tokens)
3. `docs/futurebuild/02_source_map.md §2,§3,§9` (crate inventory, mejepa modules, example partial inventory)
4. `docs/futurebuild/07_results_and_benchmarks.md §1, §2.1-§2.13, §3, §4` (shipped FSV evidence index, phase status, ADRs)
5. `docs/futurebuild/08_roadmap_and_open_work.md §0, §1, §3.-1` (binary doctrine, new epics, evidence-weighted ordering)
6. `docs/futurebuild/specs/FSV-PROTOCOL.md §1-§6` (no-mock rule, SoT readback, evidence shape)
7. `CLAUDE.md` (project addendum, hard "never"s)
8. Live tree inventory: 130 integration test files (crate-level), 218 example files, 7 benches files, 300 task manifests.
9. Per-file `git log` for last touch date on suspicious surfaces.
10. `gh issue list --search` for surface aliveness (blackbox: zero hits open or closed).

Inventory by crate:
| Crate | examples files | declared `[[example]]` |
|---|---|---|
| context-graph-cli | 7 | 0 |
| context-graph-core | 13 | 1 |
| context-graph-embeddings | 4 | 0 |
| context-graph-mcp | 17 | 3 |
| context-graph-mejepa | 89 | 56 |
| context-graph-mejepa-corpus | 8 | 0 |
| context-graph-mejepa-embedders | 7 | 0 |
| context-graph-mejepa-hygiene | 5 | 5 |
| context-graph-mejepa-instruments | 15 | 0 |
| context-graph-mejepa-shift-subscriber | 1 | 1 |
| context-graph-mejepa-tct | 2 | 2 |
| context-graph-mejepa-train | 35 | 28 |
| context-graph-mincut/paths/solver/storage/tier-compression/witness | 12 | <5 |

Note on declarations: Cargo auto-discovers any `.rs` file directly in `examples/`. `[[example]]` declarations exist for `required-features` / `path` overrides; absence does NOT imply an example is dead. Only files in subdirectories (e.g. `examples/support/`) need `#[path]` re-export to compile, and all such files verified to be referenced (`task_py_g_04N_q4_*_fsv_support.rs` referenced from main examples via `#[path = "support/..."]`).

---

## §2 Removal Candidates — HIGH confidence

These have all three: (a) no reference in current futurebuild docs, (b) no open or closed GitHub issue, (c) live tree shows independent product surface that is not part of ME-JEPA-Code.

| File | Type | LOC | Reason | Evidence | Suggested action |
|---|---|---|---|---|---|
| `crates/context-graph-cli/tests/blackbox_phase1_fsv.rs` | test | 508 | "BlackBoxFix" SaaS-ledger SQLite scaffold; not in ME-JEPA-Code product surface | `grep -ri blackbox docs/futurebuild/` → 0 hits in current docs; only `_archived/14_NEW_LOOP_MD.md`. `gh issue list --search blackbox` → 0 open/closed issues. Last commit `1cf15ab3 2026-05-12`. | Pending operator review of `crates/context-graph-cli/src/commands/blackbox.rs` (35,604 LOC; Investigator #1 owns); delete tests when bin retired |
| `crates/context-graph-cli/tests/blackbox_phase2_fsv.rs` | test | 935 | Same scaffold; SQLite ledger redaction + hash chain | Same as above | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase3_fsv.rs` | test | 898 | Same scaffold | Closed issue #176 (`blackbox_phase3_fsv GitHub curl gets 403`) acknowledged fragility | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase4_fsv.rs` | test | 461 | Same scaffold | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase5_export_fsv.rs` | test | 451 | Same scaffold | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase6_mcp_policy_fsv.rs` | test | 523 | MCP policy variant of blackbox scaffold | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase7_dashboard_fsv.rs` | test | 626 | Enterprise dashboard surface | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase8_enterprise_fsv.rs` | test | 569 | "Enterprise" surface | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase9_gitlab_ci_fsv.rs` | test | 657 | GitLab CI integration; not ME-JEPA | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase10_scanner_fsv.rs` | test | 679 | SAST scanner integration | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase11_github_issue_evidence_fsv.rs` | test | 693 | GitHub Issue connector | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase12_scorecard_fsv.rs` | test | 457 | Scorecard surface | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase13_privacy_retention_fsv.rs` | test | 650 | Privacy/retention surface | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase14_release_gate_fsv.rs` | test | 844 | Release gate surface; references `reality_loop_v1` string only | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase15_pilot_package_fsv.rs` | test | 910 | Pilot package surface | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase16_risk_register_fsv.rs` | test | 947 | Risk register surface | Last commit `ec647fee` (with phase17) | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase17_http_server_fsv.rs` | test | 383 | HTTP server / axum / tenant API surface | Imports axum router, reqwest client; not ME-JEPA | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase18_first_wedge_demo_fsv.rs` | test | 800 | "First wedge demo" surface | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase19_fsv_protocol_fsv.rs` | test | 618 | FSV protocol surface (separate from project FSV-PROTOCOL) | Same | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase20_k8s_deployment_fsv.rs` | test | 534 | Kubernetes deployment surface | Not ME-JEPA | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase21_terraform_cloud_fsv.rs` | test | 668 | Terraform Cloud surface | Not ME-JEPA | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase22_ticketing_fsv.rs` | test | 567 | Ticketing connector | Not ME-JEPA | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase23_otel_export_fsv.rs` | test | 539 | OpenTelemetry export | Not ME-JEPA | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_phase24_sdk_ingest_fsv.rs` | test | 430 | CloudEvents SDK ingest | Last commit `a9c00ef8`; not ME-JEPA | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_reward_compiler_fsv.rs` | test | 750 | Domain-pack reward compiler | Imports BlackBox surface | Delete with phase1 |
| `crates/context-graph-cli/tests/blackbox_cross_phase_api_surface_hardening_fsv.rs` | test | ~700 | Cross-phase API hardening | Imports BlackBox surface | Delete with phase1 |
| **TOTAL BlackBox tests** | | **~16,772 LOC** | | | |

| File | Type | LOC | Reason | Evidence | Suggested action |
|---|---|---|---|---|---|
| `crates/context-graph-core/examples/tree_sitter_test.rs` | example | 25 | Manual scratch smoke; no assertions, no FSV emitted | `grep -c assert!\\|panic! ...` → 0; `println!`-only output | Delete |
| `crates/context-graph-core/examples/fsv_line_numbers.rs` | example | 172 | `println!`-only manual smoke; no assertions, no FSV file write | 0 assertion macros | Delete |
| `crates/context-graph-core/examples/fsv_source_metadata.rs` | example | 88 | `println!`-only manual smoke; no assertions, no FSV file write | 0 assertion macros | Delete |
| `crates/context-graph-benchmark/src/bin/e11_entity_bench.rs` | bench bin | ~600 | E11 (KEPLER) embedder DISABLED in production per memory (Audit #9: e10_active_vector with E11 weight-gated off; TASK-EK-018F TransE shipped 2026-05-19 as E11 replacement) | Last commit `2838efe4 2026-04-27` (pre-binary-doctrine reframe); E11 weight-gated off in MultiSpace | Pending: confirm with operator that E11 retired benchmarks not part of regression suite |
| `crates/context-graph-benchmark/src/bin/e10_modifier_tuning.rs` | bin (not declared `[[bin]]`) | ~? | "Phase 6: Grid search over intent→context and context→intent modifiers" — intent modeling RETIRED per closed `type:decision` #358 (2026-05-18) | Module header says `intent-context benchmark pairs (from Phase 3 data)`; #358 retires `mejepa_elicit_intent`; #397 lock-rule says no hidden-intent inference | Delete |

---

## §3 Removal Candidates — MEDIUM confidence

These are stale by indirect evidence (retired phase, retired surface, no recent commits) but may still serve as proof-of-work artifacts in 07_results_and_benchmarks.md.

| File | Type | Reason | Evidence | Suggested action |
|---|---|---|---|---|
| `crates/context-graph-storage/tests/llm_validation_integration.rs` | test | Round-trips `LLMEdgeValidation` storage type carrying `validator_version: "hermes-2-pro-mistral-7b-q5-km@2026-04"` string. Hermes was retired 2026-05-09 (Phase H). Tests storage roundtrip only — no actual LLM call. | File has hardcoded `hermes-2-pro-mistral-7b-q5-km@2026-04` validator-version strings; underlying `CF_TYPED_EDGE_VALIDATIONS` + `LLMEdgeValidation` type still in `context-graph-core/src/llm_edge_validation.rs` (Investigator #1 territory) | Mark stale; remove when Investigator #1 deletes the source type |
| `crates/context-graph-storage/tests/typed_edge_export_integration.rs` | test | Same Hermes validator-version string at line 55 | Same | Mark stale together with M1 |
| `crates/context-graph-storage/tests/full_14_embedder_fsv.rs` | test | Uses E11_DIM constant from retired embedder | Per memory: `E11 disabled per memory (TASK-EK-018F TransE replacement)`; constants still exist for storage roundtrip | Keep as storage-shape test until schema bump; flag for update when E11 dim removed |
| `crates/context-graph-core/examples/legacy_ast_chunker_parity_fsv.rs` | example | Explicit `#![allow(deprecated)]` — tests a deprecated bridge | First line: `// This FSV intentionally verifies the deprecated bridge.` | Delete when deprecated AST-chunker bridge in `context-graph-core` is removed; not before |
| `crates/context-graph-benchmark/src/bin/causal_accuracy_bench.rs` | bin (not declared) | Causal-discovery is no longer a product surface after binary doctrine reframe | Not declared in `[[bin]]`; auto-compiles only because no `autobins=false`; not referenced in 07 §4 benchmarks list | Operator review whether causal benchmarks still inform ship-gate |
| `crates/context-graph-benchmark/src/bin/causal_embedding_bench.rs` | bin (not declared) | Same | Same | Same |
| `crates/context-graph-benchmark/src/bin/causal_provenance_bench.rs` | bin (not declared) | Same | Same | Same |
| `crates/context-graph-benchmark/src/bin/causal_realdata_bench.rs` | bin (declared `[[bin]]`) | Causal-realdata benchmark; named `causal-realdata-bench` | Declared; might still be live | Verify with operator |
| `crates/context-graph-benchmark/src/bin/e2_manual_verification.rs` | bin (not declared) | Verifies temporal-boost decay; pre-binary-doctrine, E2 weight-gated to 0.0 by default in MultiSpace | Module header: "The boost is only applied when temporal_weight > 0 (defaults to 0.0)" | Delete if E2 boost path retired; operator review |
| `crates/context-graph-benchmark/src/bin/embedder_impact_bench.rs` + `embedder_impact_realdata_bench.rs` | bins | Pre-binary-doctrine impact benchmarks | Not in 07 §4 benchmark list | Operator review |
| `crates/context-graph-benchmark/src/bin/weight_profile_bench.rs` | bin (not declared) | Weight-profile experimentation; pre-binary-doctrine | Last commit ancient | Operator review |
| `crates/context-graph-benchmark/src/bin/sparse_bench.rs` | bin (declared `[[bin]]`) | Sparse embedder benchmarks; pre-binary-doctrine | Declared | Verify |
| `crates/context-graph-benchmark/src/bin/multimodal_bench.rs` + `multimodal_realdata_bench.rs` | bins | Multimodal benchmarks; binary doctrine is text-Python only | Not in 07 §4 | Delete or gate behind feature flag |
| `crates/context-graph-cli/tests/dynamicjepa_research_smoke_fsv.rs` | test | Closed issue #81 acknowledged fragility (requires `/tmp/onet_probe/db_30_2_text.zip`); test gated by `ONET_TEXT_ZIP` env at commit `c53964ae` | Issue #81 closed but file persists | Verify gating is sufficient; keep as integration test if so |

---

## §4 Removal Candidates — LOW confidence (needs operator review)

These have specific concerns but may still serve a purpose.

| File | Type | Reason | Suggested action |
|---|---|---|---|
| `crates/context-graph-core/examples/divergence_fsv_test.rs` (450 LOC, 46 assertions, 76 println) | example | Heavy `println!` debug output but has 46 real assertions; verifies `DivergenceReport` retrieval surface | Operator review: is divergence still a live product surface? Memory mentions divergence in audit history but not current binary doctrine |
| `crates/context-graph-core/examples/embedder_routing_fsv.rs` (104 LOC, 0 assertions) | example | Has custom `require()` failure mechanism (not `assert!`); writes `/zfs/archive/contextgraph/fsv/contextgraph-mejepa-embedders-fsv/routing_coverage.json` | Keep; verify routing surface still active |
| 25 `phase_f_*_fsv.rs` examples across crates | examples | Per 07 §2.6 Phase F "all implementation issues CLOSED as of 2026-05-13" — proof-of-work artifacts no longer re-run | Keep as forensic continuity; mark "frozen — Phase F" |
| 6 `phase_b_*` / `phase_c_*` / `phase_d_*` / `phase_e_*` examples | examples | Per 07 §2.1-§2.5 Phase A-E "code/FSV shipped"; proof artifacts | Keep as forensic continuity |
| `crates/context-graph-mejepa-corpus/examples/multilang_mutation_fsv.rs` | example | Tests non-Python mutation operators; non-Python languages "deferred from ship gate" per `CLAUDE.md §6.3` | Keep; non-Python ops still in code per `generic_language_ops.rs` |
| `crates/context-graph-storage/tests/learning_events_fsv.rs` | test | `learning_events` surface; uses `CF_LEARNING_EVENTS` not in current futurebuild docs index | Verify Learning-as-UTL is still a live product surface |
| `crates/context-graph-storage/tests/utl_learner_state_fsv.rs` + `utl_learner_embedders_fsv.rs` | tests | UTL learner state; not in current futurebuild but `commands/utl.rs` exists in CLI | Verify aliveness |

---

## §5 Q4 frozen tests/examples (post-#408) — KEEP but mark frozen

Per `08_roadmap_and_open_work.md §0` #408 CLOSED 2026-05-19: "Q4 doctrine freeze + close out Q4 producer track" with aiwonder FSV passed. These files are forensic continuity; no new producer writes after 2026-05-19. CFs remain readable.

| File | Status |
|---|---|
| `crates/context-graph-mejepa-train/examples/task_py_g_012_q4_confidence_calibration_fsv.rs` | FROZEN (was PASS in 07 §1 line 68) |
| `crates/context-graph-mejepa-train/examples/task_py_g_047_q4_perf_label_fsv.rs` + `support/task_py_g_047_q4_perf_label_fsv_support.rs` | FROZEN (was PASS, #266) |
| `crates/context-graph-mejepa-train/examples/task_py_g_048_q4_accuracy_label_fsv.rs` + `support/task_py_g_048_q4_accuracy_label_fsv_support.rs` | FROZEN (was PASS, #267) |
| `crates/context-graph-mejepa-train/examples/task_py_g_049_q4_cost_label_fsv.rs` + `support/task_py_g_049_q4_cost_label_fsv_support.rs` | FROZEN (was PASS, #268) |
| `crates/context-graph-mejepa-train/examples/task_py_g_050_q4_reasoning_label_fsv.rs` + `support/task_py_g_050_q4_reasoning_label_fsv_support.rs` | FROZEN (was PASS, #269) |
| `crates/context-graph-mejepa-train/examples/task_py_g_051_q4_non_trivial_on_pass_fsv.rs` | FROZEN (was PASS, #270) |
| `crates/context-graph-mejepa-train/examples/task_py_g_100_q4_harvest_expansion_fsv.rs` | FROZEN (was PASS, #401) |
| `crates/context-graph-mejepa-train/examples/task_py_g_114_q4_doctrine_freeze_fsv.rs` | LIVE — this IS the freeze proof FSV |
| `crates/context-graph-mejepa-train/examples/task_rwd_204_q4_security_fsv.rs` | FROZEN (was structural-head excluded under #402) |
| `crates/context-graph-mejepa/examples/task_py_g_078_q4_trust_gate_fsv.rs` | FROZEN (was PASS, #377) |
| `crates/context-graph-mejepa/examples/task_py_g_044_multi_head_contradiction_fsv.rs` | KEEP LIVE (#263 PASS; uses Q4-aware contradiction but the multi-head contradiction surface remains active even with Q4 frozen) |
| `crates/context-graph-mejepa/examples/task_py_g_090_consensus_contradiction_heads_fsv.rs` | KEEP LIVE (#390 PASS; q4_risk_pressure is one input but consensus/contradiction is a Q3 surface) |
| `crates/context-graph-mejepa/examples/phase_g_cold_start_verdict_fsv.rs` | KEEP LIVE (#TASK-PY-G-061 PASS; Q4 abstain note checks but cold-cell policy is Q2/Q3) |

Suggested action: tag all FROZEN files with a top-of-file comment `// FROZEN 2026-05-19 per #408 — forensic continuity only; do not re-run, do not modify` and move to a `q4_archived/` subdirectory of `examples/` when codebase is ready for the schema bump that drops Q4 CFs (tracked under `phase:cleanup` per `02_source_map.md §12.4`).

---

## §6 Phase H absence-check tests — MUST KEEP

Per scope rule: "HARNESSES that verify reality-loop / inner-LLM is ABSENT — these are STILL LIVE proof and must be KEPT".

Search for Rust-level Phase H tests: **NONE FOUND**. All Phase H absence-checks live as shell scripts under `scripts/ci/phase_h_*` (Investigator #3 territory):

- `scripts/ci/phase_h_no_inner_actor_models_fsv.sh`
- `scripts/ci/phase_h_reality_loop_binary_absence_fsv.sh`
- `scripts/ci/phase_h_retired_mcp_tools_fsv.sh`
- `scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh`
- `scripts/ci/phase_h_ccreality_engine_retired_gate_fsv.sh`
- `scripts/ci/phase_h_claude_inner_loop_reference_fsv.sh`
- `scripts/ci/phase_h_agent_roster_retirement_fsv.sh`
- `scripts/ci/phase_h_deletion_list_audit_fsv.sh`
- `scripts/ci/phase_h_smap_superseded_decisions_fsv.sh`

Within my scope, no test or example uses or invokes the retired stack (Hermes/Qwen3.6/Ollama/llama.cpp/DeepSeek/OpenAI). The two `hermes-2-pro` matches are validator-version metadata strings in storage round-trip tests (§3 row M1/M2).

---

## §7 Mock data violations (FSV-PROTOCOL §3 violations)

Search across `crates/*/tests/` and `crates/*/examples/`:

| Pattern | Hits | Notes |
|---|---|---|
| `MockEmbedder`, `mock_embedder`, `FakeEmbedder`, `StubEmbedder` | 0 | Clean |
| `MOCKED`, `todo!()`, `unimplemented!()` | 0 | Clean |
| `mocked`, "mock data", "fake data" (case-insensitive content match) | scattered comments | Mostly NEGATIVE assertions ("no mocks", "no fake data") |
| `assert!(true)` | 0 | Clean |

One file exists specifically to **detect** fake data: `crates/context-graph-embeddings/tests/no_fake_data_test.rs` (AP-007 compliance; detects fake pointer `0x7f80_0000_0000`, sin-wave outputs, VRAM delta mismatch, all-zeros outputs). This is the OPPOSITE of a violation — it's a guard. **Live and load-bearing.**

**No FSV-PROTOCOL §3 violations found in tests/examples within scope.** The corpus respects the no-mock rule.

---

## §8 Orphan examples (no auto-discovery)

Cargo auto-discovers any `.rs` file directly in `examples/` (no `autoexamples=false` anywhere in the workspace). The only files that need an explicit `#[path]` re-export are in subdirectories:

- `crates/context-graph-mejepa-train/examples/support/task_py_g_047_q4_perf_label_fsv_support.rs` → referenced from `task_py_g_047_q4_perf_label_fsv.rs` via `#[path = "support/..."]`
- `crates/context-graph-mejepa-train/examples/support/task_py_g_048_q4_accuracy_label_fsv_support.rs` → referenced from `task_py_g_048_q4_accuracy_label_fsv.rs`
- `crates/context-graph-mejepa-train/examples/support/task_py_g_049_q4_cost_label_fsv_support.rs` → referenced from `task_py_g_049_q4_cost_label_fsv.rs`
- `crates/context-graph-mejepa-train/examples/support/task_py_g_050_q4_reasoning_label_fsv_support.rs` → referenced from `task_py_g_050_q4_reasoning_label_fsv.rs`

All four support files are properly wired. **Zero true orphan examples found.**

Same check for `crates/context-graph-benchmark/src/bin/` — no `autobins=false`, so all 32 files auto-compile even when only ~20 are declared `[[bin]]`. The undeclared ones still compile but use defaulted name based on file (kebab-case from snake_case). Not orphans by Cargo's definition.

---

## §9 Stale `/mnt/d/` paths in tests/examples (#369 revalidation needed)

**146 files** hardcode `/mnt/d/contextgraph/...` paths with NEITHER `env::var` override NOR `/zfs/archive` fallback. Per closed `type:decision` #363 (2026-05-15, "aiwonder is durable SoT for post-migration ContextGraph Phase G") and tracked under #369, these need revalidation.

Pattern check: current/canonical pattern (e.g. `task_py_g_091_slot_attribution_explanation_fsv.rs`):
```rust
const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-091-slot-attribution-explanation-fsv";
let fsv_root = std::env::var("CG_TASK_PY_G_091_FSV_ROOT")
    .unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string());
```

Stale pattern (e.g. `task_py_g_065_ewc_fsv.rs`):
```rust
const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-065-ewc-fsv";
// no env override anywhere; can't run on aiwonder
```

The 146 files break down (selected high-confidence stale; FULL list available via the inventory below):

| Crate | Count | Worst offenders (assertions hardcoded to `/mnt/d/`) |
|---|---|---|
| `context-graph-mejepa` | 47 | `task_py_g_065_ewc_fsv.rs`, `task_py_g_066_continual_backprop_fsv.rs`, `task_py_g_067_novel_pattern_fsv.rs`, `task_py_g_068_replay_buffer_fsv.rs`, `task_py_g_069_posttooluse_capture_fsv.rs` (all PASS in 07 §1 with `/zfs/archive` evidence — example file mismatch with shipped evidence) |
| `context-graph-mejepa-train` | 16 | `train_smoke_fsv.rs`, `train_full_fsv.rs`, `train_holdout_eval_fsv.rs`, `swebench_300x8_ship_gate_fsv.rs` (the literal ship-gate gate that 0.866667 measured against — hardcodes `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/`; matches the 07 §1 legacy reference; #369 to revalidate) |
| `context-graph-mejepa-instruments` | 12 | All `task_rwd_*` instruments examples |
| `context-graph-mcp` | 16 | `phase_f_e2e_happy_path_fsv.rs`, `daemon_supervision_fsv.rs`, `phase_g_*` shipped tasks |
| `context-graph-cli` | 7 | `phase_f_active_learning_cli_fsv.rs`, `phase_g_oracle_flakiness_quarantine_fsv.rs` |
| `context-graph-core` | 9 | `ast_chunker_fsv.rs`, `swebench_ast_fsv.rs`, `python_ast_atomic_fsv.rs`, `legacy_ast_chunker_parity_fsv.rs`, `swebench_python_ast_corpus_fsv.rs` |
| `context-graph-embeddings` | 4 | `bert_family_true_batch_fsv.rs`, `colbert_true_batch_fsv.rs`, `qwen2_code_true_batch_fsv.rs`, `splade_true_batch_fsv.rs` (`/mnt/d/cabdru/contextgraph/models` default) |
| `context-graph-mejepa-corpus` | 7 | `mejepa_corpus_fsv.rs`, `oracle_fsv.rs`, `split_fsv.rs`, `swebench_oracle_fsv.rs`, `phase_g_targeted_mutation_expansion_fsv.rs` |
| `context-graph-mejepa-embedders` | 7 | All examples |
| `context-graph-mejepa-hygiene` | 5 | All examples; declared `[[example]]` |
| Other | 16 | `mincut_fsv.rs`, `ed25519_fsv.rs`, `cert_fsv.rs`, etc. |

Specific contradiction caught: `crates/context-graph-mejepa/tests/load.rs` (lines 36, 40) asserts `fsv_root.starts_with("/zfs/archive/contextgraph/fsv/phase-f-load-test-fsv/runs")` but the underlying `tests/e2e/load.rs` line 187 returns `PathBuf::from("/mnt/d/contextgraph/fsv")` and lines 869, 872 contain `path.starts_with("/mnt/d/contextgraph/fsv/")`. **This test cannot pass as currently written.** Worth investigating whether `load.rs` ever ran green post-#363.

**Action:** Per #369, each of the 146 files should:
1. Add `CG_<TASK>_FSV_ROOT` env var override.
2. Change `DEFAULT_FSV_ROOT` to `/zfs/archive/contextgraph/fsv/<task>-fsv`.
3. Revalidate on aiwonder and update `07_results_and_benchmarks.md` to point to the new run.

---

## §10 Brittle/always-pass tests

Tests with no assertions or vacuous assertions:

| File | Assertions | Notes |
|---|---|---|
| `crates/context-graph-core/examples/tree_sitter_test.rs` | 0 | `println!`-only; 25 LOC; pure smoke (also in §2 HIGH) |
| `crates/context-graph-core/examples/fsv_line_numbers.rs` | 0 | `println!`-only; 172 LOC; named `_fsv_` but emits no FSV (also in §2 HIGH) |
| `crates/context-graph-core/examples/fsv_source_metadata.rs` | 0 | `println!`-only; 88 LOC; named `fsv_` but emits no FSV (also in §2 HIGH) |
| `crates/context-graph-core/examples/embedder_routing_fsv.rs` | 0 native `assert!`; uses custom `require()` | Custom failure macro is functionally equivalent to assert; not brittle |
| `crates/context-graph-mejepa/tests/load.rs` | 8 assertions but contradicting underlying file | Asserts `/zfs/archive/...` but `tests/e2e/load.rs` hardcodes `/mnt/d/...` — cannot pass on either filesystem unless one or the other is wrong (also in §9) |

**No `assert!(true)` instances found.** No tests with zero verification logic that are gating completion.

---

## §11 `tasks/` and `tests/e2e/` summary

### `tasks/swebench-lite/`

300 SWE-bench Lite Python instance manifests (`{repo}__{repo}-{NNN}.json`). Each contains `instance_id`, `repo`, `base_commit`, `problem_statement`, `fail_to_pass`, `pass_to_pass_count`, `official_test_patch_sha256`, `oracle_patch_sha256`, `source_of_truth: "huggingface:princeton-nlp/SWE-bench_Lite:test:<instance_id>"`. Referenced by:
- `crates/context-graph-mejepa-corpus/src/swebench.rs` (live; product surface)
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs`, `split_fsv.rs`
- `crates/context-graph-mejepa-instruments/examples/ast_embedder_smoke_fsv.rs`, `phase1_extended_fsv.rs`
- `scripts/swebench_lite_ops/commands.py`, `core.py`

**Verdict:** LIVE. Per `01_overview.md` and `07 §1.1` these 300 manifests are the canonical Python ship-gate corpus dimension. Path discipline (CLAUDE.md §7) says I cannot propose deletion. Keep all 300.

### `tests/e2e/` (repo root)

13 files: `adversarial_corpus.rs`, `catastrophic_event.rs`, `chaos_failpoints.rs`, `happy_path.rs`, `harness.rs`, `heal_ticker_telemetry.rs`, `load.rs`, `multi_agent.rs`, `pause_active_learning.rs`, `pause_consumer.rs`, `restart_safety.rs`, `soak.rs`, `storage_exhaustion.rs`.

Each is wired into Cargo via a shim in `crates/<crate>/tests/`:
- `crates/context-graph-mejepa/tests/e2e.rs` re-exports `happy_path`, `restart_safety`, `multi_agent`.
- `crates/context-graph-mejepa/tests/multi_agent.rs`, `soak.rs`, `load.rs`, `heal_ticker_telemetry.rs`, `adversarial_corpus.rs`, `chaos_failpoints.rs`, `catastrophic_event.rs`, `pause_consumer.rs`, `pause_active_learning.rs` each `#[path]`-re-export the root file.
- `crates/context-graph-mejepa-hygiene/tests/storage_exhaustion.rs` re-exports `storage_exhaustion.rs`.
- `crates/context-graph-mcp/tests/phase_f_e2e_happy_path.rs` re-exports `harness.rs`.

**Verdict:** LIVE. All 13 files are reached by Cargo test discovery via shim integration tests. No orphans. However, `load.rs` has the `/mnt/d/` vs `/zfs/archive/` contradiction noted in §9 + §10.

### `tests/fixtures/`

30 adversarial diff fixtures across 3 categories: `embedder_targeted_*.diff` (10), `knowngood_lookalike_*.diff` (10), `buggy_passing_*.diff` (10). These feed TASK-PY-G-060 / TASK-FP-009 adversarial corpus, which is **PASS** per 07 §1 line 20 ("30/30 adversarial rows persist in `CF_MEJEPA_ADVERSARIAL_CORPUS`"). **LIVE, KEEP ALL.**

---

## §12 `context-graph-benchmark` crate verdict

87K LOC; excluded from default build per `Cargo.toml:33-61`. Build explicitly with `cargo build -p context-graph-benchmark`.

**Cargo.toml inventory** (live observation; `02_source_map.md §8 / §2 row 25` is stale — it lists 4 benchmark `[[bin]]` names that do not exist: `maxsim_bench`, `causal_relationships_bench`, `hybrid_effectiveness_bench`, `e11_quality_bench`):

Actual declared `[[bench]]` targets (2):
- `comparative_suite` → `benches/comparative_suite.rs`
- `scaling_analysis` → `benches/scaling_analysis.rs`

Actual declared `[[bin]]` targets (32):
`benchmark-report, realdata-bench, gpu-bench, embedder-stress, hf-bench, temporal-bench, causal-realdata-bench, temporal-realdata-bench, sparse-bench, graph-bench, multimodal-bench, multimodal-realdata-bench, unified-realdata-bench, graph-realdata-bench, validation-realdata-bench, e4-hybrid-session-bench, e1-semantic-bench, e11-entity-bench, e7-bench, e7-realdata-bench, embedder-impact-bench, embedder-impact-realdata-bench, weight-profile-bench, code-fingerprint-bench, causal-accuracy-bench, causal-embedding-bench, causal-provenance-bench, disk-space-bench, graph-linking-bench, graph-linking-real-bench, spawn-blocking-bench, e2-manual-verification`

Auto-discovered (not declared, but in `src/bin/`):
- `e10_modifier_tuning.rs` — intent-modeling tuning; intent retired per #358 → **REMOVE** (§2 HIGH).

Per-bench verdict:

| Bin | Status | Reason |
|---|---|---|
| `benchmark-report` | KEEP | Reporter for benchmarks |
| `gpu-bench`, `hf-bench`, `temporal-bench`, `sparse-bench`, `graph-bench`, `realdata-bench`, `unified-realdata-bench` | KEEP | Still benchmark the embedder pipeline; binary doctrine doesn't retire embedders themselves |
| `e1-semantic-bench`, `e7-bench`, `e7-realdata-bench`, `e4-hybrid-session-bench` | KEEP | Active embedders (E1, E4, E7) |
| `e2-manual-verification` | LOW (§4) | E2 weight-gated to 0.0 by default; pre-binary-doctrine; operator review |
| **`e10_modifier_tuning`** (auto-discovered) | **REMOVE** (§2) | Intent-modeling retired per #358 |
| **`e11-entity-bench`** | **REMOVE** (§2) | E11 disabled in production; TASK-EK-018F TransE shipped 2026-05-19 as replacement |
| `causal-realdata-bench` (declared) + `causal-accuracy-bench`, `causal-embedding-bench`, `causal-provenance-bench` (auto-discovered) | MEDIUM (§3) | Causal-discovery is not in binary doctrine product surface |
| `embedder-impact-bench`, `embedder-impact-realdata-bench`, `weight-profile-bench` | MEDIUM (§3) | Pre-binary-doctrine optimization; not in 07 §4 |
| `graph-linking-bench`, `graph-linking-real-bench` | KEEP | Graph linking is a live MCP surface (#417 chunk-skill linkage) |
| `multimodal-bench`, `multimodal-realdata-bench` | MEDIUM (§3) | Binary doctrine is text-Python only; multimodal is out of scope |
| `validation-realdata-bench` | KEEP if used by ship-gate; verify |
| `code-fingerprint-bench` | KEEP (related to TASK-FP fingerprint catalog) |
| `disk-space-bench`, `spawn-blocking-bench` | KEEP (infrastructure tests) |
| `temporal-realdata-bench` | KEEP if E2 boost active; LOW otherwise |
| `embedder-stress` | KEEP (load testing) |
| `comparative_suite`, `scaling_analysis` (benches) | KEEP unless retired |

**Overall verdict for the crate:** Retain it as-is (excluded from default build) but **clean up the dead bins** (e10/e11/intent-tuning and possibly causal/multimodal/weight-profile suites). The 87K LOC headline is misleading: most lines are in `src/realdata/`, `src/causal_bench/`, `src/ablation.rs`, `src/datasets/`, `src/metrics/`, `src/baseline/` (shared library code reused by the bins). Until the source-side dead-code review (Investigator #1) deletes the supporting library code, deleting individual bins gains relatively little.

---

## §13 Suggested removal sequence

Order matters to keep the build green at every step.

1. **Phase H absence-check verification.** Confirm no Rust test mentions the retired inner stack. **DONE** — none in scope.
2. **Step 1 — easy wins (no source changes).** Delete the 3 zero-assertion `examples/*.rs` in `context-graph-core` (`tree_sitter_test.rs`, `fsv_line_numbers.rs`, `fsv_source_metadata.rs`). Run `cargo build --examples -p context-graph-core` → expect green.
3. **Step 2 — `e10_modifier_tuning`.** Delete `crates/context-graph-benchmark/src/bin/e10_modifier_tuning.rs`. Confirm `cargo build -p context-graph-benchmark` still green (intent-related). It's not declared `[[bin]]`, but auto-discovered; deletion safe.
4. **Step 3 — pre-#363 #369 batch revalidation (large but mechanical).** For the 146 `/mnt/d/`-only example files, add env-override pattern (`CG_<TASK>_FSV_ROOT`) defaulting to `/zfs/archive/contextgraph/fsv/<task>-fsv`. Revalidate one at a time on aiwonder and update 07 §1. **Do NOT delete files** — they're the running FSV harness for shipped tasks.
5. **Step 4 — fix `tests/e2e/load.rs` contradiction.** Either (a) update `tests/e2e/load.rs` to use `/zfs/archive/contextgraph/fsv/phase-f-load-test-fsv` matching `crates/context-graph-mejepa/tests/load.rs:36-40`, or (b) revert the assertion in `crates/context-graph-mejepa/tests/load.rs`. Decision belongs to operator.
6. **Step 5 — Q4 freeze tagging.** Add top-of-file `// FROZEN 2026-05-19 per #408 — forensic continuity only` to the 9 Q4 example files in §5. No deletion. Optional: move into `examples/q4_archived/` subdirectory with `#[path]` re-exports (similar to support files) when codebase ready for the schema bump.
7. **Step 6 — BlackBoxFix retirement (largest mass).** Sequence:
   1. Operator decision: is the `blackbox` `[[bin]]` retained as a separate sub-product, or fully retired?
   2. If retired: Investigator #1 deletes `crates/context-graph-cli/src/commands/blackbox.rs` (35,604 LOC), `crates/context-graph-cli/src/bin/blackbox.rs`, and the `[[bin]] name = "blackbox"` declaration in `crates/context-graph-cli/Cargo.toml:23-25`. Then THIS investigation deletes 26 test files (~16,772 LOC).
   3. Build verification: `cargo build --release -p context-graph-cli` green; `cargo test -p context-graph-cli` green.
8. **Step 7 — Hermes string cleanup.** After Investigator #1 deletes `crates/context-graph-core/src/llm_edge_validation/` and `CF_TYPED_EDGE_VALIDATIONS`, delete `crates/context-graph-storage/tests/llm_validation_integration.rs` and remove the Hermes string lines from `crates/context-graph-storage/tests/typed_edge_export_integration.rs`.
9. **Step 8 — benchmark `[[bin]]` cleanup.** Remove `e11-entity-bench`, optionally causal/multimodal/weight-profile/embedder-impact bins if operator confirms they don't gate ship.
10. **Step 9 — deprecated bridge.** When `context-graph-core/src/memory/ast` deprecated bridge is removed (Investigator #1), delete `crates/context-graph-core/examples/legacy_ast_chunker_parity_fsv.rs`.

Estimated net deletion potential at conservative end (steps 1, 2, 5 tagging only, 7, 9): ~17,500 LOC dead tests + ~3 dead examples + 1 dead intent bench bin. At aggressive end (also retiring causal/multimodal benchmarks and the BlackBox `[[bin]]`): ~52,500 LOC.

---

## §14 What I did NOT investigate

Per scope:
- **Crate `src/` runtime code** — Investigator #1 handles `crates/*/src/`. References in this report (e.g. `commands/blackbox.rs` 35,604 LOC, `llm_edge_validation/`, deprecated AST-chunker bridge, E11 source code, `e10_modifier_tuning` source) are surfaced as DEPENDENCY for sequencing but NOT classified for removal here.
- **`scripts/`** — Investigator #3. Phase H shell scripts (`scripts/ci/phase_h_*.sh`), `scripts/swebench_lite_ops/python_test_intent.py` (retired per #358), and the `scripts/swebench_lite_ops/cli.py` `python-auto-label-discovery` subcommand surface are all theirs.
- **`docs/`, `docs2/`, top-level Markdown** — Investigator #3. The `02_source_map.md §2 row 25 / §8` "4 benchmark bin names" mismatch (the named bins don't exist) is documented above for context.
- **`.claude/`, `.github/`, `config/`, `Cargo.toml`, `Cargo.lock`** — Investigator #3.
- **`/mnt/d/contextgraph/`, `/zfs/archive/contextgraph/`, `/zfs/hot/contextgraph/`** — runtime/durable directories outside the repo working tree; not in any investigator's scope.
- **`agentdb/`** — upstream reference-only clone (per `CLAUDE.md §6.7`); never modify.
- **`tests/oracle_*/**`, `swebench/**`** — don't exist in the current tree but called out as don't-touch in `CLAUDE.md §7`; respected.
- **`*.pem`, `*.key`, `credentials.json`** — none found in scope; respected.

End of report.
