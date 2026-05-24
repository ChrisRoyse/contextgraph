# 07 — Results, Benchmarks, and FSV Evidence History

**Source:** aiwonder `/zfs/archive/contextgraph/fsv/` for production evidence; git log; GitHub issue comments; per-issue RESOLVED comments.

**Purpose:** preserve proof-of-work that future agents would otherwise lose. Every entry references a physical artifact (FSV JSON, sha256, CF) that can be re-verified.

---

## 1. Current ship-gate status

| Metric | Target | Latest measurement |
|---|---|---|
| `prediction_oracle_correlation` (raw Pearson) | ≥ 0.95 stable × 4 rolling windows × all Python mutation cells | **`0.4843192995`** |
| Brier | ≤ 0.05 | `0.0923729688` |
| ECE | ≤ 0.05 | `0.0240316074` |
| Cross-panel oracle correlation (Panel B) | ≥ 0.95 with `\|corr_a − corr_b\| ≤ 0.05` | Blocked behind #683 + clean v2 static cache (#709/#715) |
| `mistake_repeat_rate` | ≤ 0.05 (rolling 100-mistake window) | Byte-readable via `mejepa_mistake_loop_status`; mistake-loop LUT inert in production (panels rarely collide on exact signature hash) |

**Honest gap to ship:** `0.4657` against raw Pearson.

**Latest ship-gate FSV:** `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`, report sha256 `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499`, readback sha256 `9cf352a775a38df87f17e6b1b0964d0ec93b93de9d5447b6bc762294c1b67ae2`.

### 1.1 Degenerate-cell metric contract (#532)

Pearson is undefined for zero-variance oracle cells. The ship-gate FSV classifies every block as `mixed`, `all_fail`, `all_pass`, or `empty`:

| Block class | Metric |
|---|---|
| mixed | Pearson + Fisher-z confidence interval |
| all_fail / all_pass / single-class | target-class recall ≥ 0.95, actual-class confidence ≥ 0.95, Brier ≤ 0.05, ECE ≤ 0.05, Wilson lower bound on recall |
| empty | not gate-eligible |

Each block carries statistical-power diagnostics. Never emit a bare "correlation unavailable" blocker without a replacement metric.

### 1.2 Calibration + invalid-source + rank-safety policy (#533/#531/#534/#535/#536)

Ship-gate FSV fits deterministic calibration/rank-lift policies on the 240-row calibration split only, then evaluates the 240-row holdout. Constants are allowed only for structurally degenerate `compile_error` / `wrong_file` cells. Rank-reversing transforms only when calibration has mixed-class support ≥ 8 per class + raw Pearson ≤ -0.25.

`compile_error` uses raw invalid-source diagnostic panels. Rank-lift uses category-local RBF exemplar scoring over real `code_state_constellations.jsonl` slot-feature means + hashed live label counts while excluding target-side oracle/docker/test labels.

**Rank-lift transform (`0.4843 → 0.7942` on holdout) is diagnostic-only per #695** — it is a category-local exemplar-similarity composite, not the predictor's score; it does not improve Brier/ECE. Honest gap is against raw Pearson.

### 1.3 Active/abstain diagnostic (#392, not a gate pass)

Latest ship-gate FSV folds #392 active/abstain as an easy-row diagnostic on `205/240` rows: active corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`. **Not a hidden gate pass** — per-cell active metric + rolling-window + statistical-power blockers remain. #392 is blocked behind the #712 clean-substrate/trainer chain before renewed gate movement.

---

## 2. Phase status summary

### 2.1 Phases A–F: shipped

All Phase A-D implementation issues (#3-#19 umbrella #15) CLOSED. Phase E self-optimization policy shipped. Phase F production operations + observability + runbook + DR + API shipped. Independent audit verified 35/35 items present.

### 2.2 Phase G: in flight — model-quality convergence

Open work targeting the 0.95 ship gate. Authoritative ordering in `08 §3` per pinned context #712.

### 2.3 Phase H: complete — inner-actor LLM retirement

`feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`. The previously documented Phase H `scripts/ci/phase_h_*.sh` absence-check scripts are missing on disk; #693 tracks the doc/control gap for Qwen / Hermes / Ollama / llama.cpp absence checks.

---

## 3. Key shipped milestones

### 3.1 Mistake-loop MCP tools (#523 / TASK-PY-G-112B)

`mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status` shipped. All three tools registered; writes/reads land in RocksDB CFs; online head corrects same-panel mistake; lifecycle audit joins byte-readable; missing CFs fail closed; target-side oracle labels rejected as live inputs; invalid panel signatures leave no partial mistake/replay writes.

FSV: `fsv/task-py-g-112b-mistake-loop-mcp-tools-fsv/run-20260520T113125Z/`, sha256 `9a9e50b6…`.

### 3.2 Online mistake-driven loop (#406 / TASK-PY-G-112)

Key-local exact-panel online-head state. `protection_mode=exactPanelKeyIsolation`, `claims_fisher_ewc_protection=false`. Convergence verified: inject wrong prediction → second presentation yields corrected verdict. Unrelated-control delta ±0.02.

### 3.3 Failure-mode hierarchy (#404 / TASK-PY-G-110)

8 Level-1, 157 Level-2, 379 Level-3, 3,169 Level-3 memberships, 536 named predictions, 12,658 total chunk-skill membership rows. Holdout: 114/114 actual Fail rows have FailLikely; closest-exemplar oracle agreement 1.0 over 113/114.

FSV: `fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779376526126/`, sha256 `77deb00b…`.

### 3.4 Auto-label discovery + skill substrate (#413 / #414)

**#413 / TASK-PY-G-117:** deterministic Level-0 label compiler (`python_auto_label_discovery.py`, 1,641 lines, no inner LLM). Full-corpus aiwonder FSV PASS: 399,777 chunk labels, 491 accepted label registry, 3,000 code-state constellations, 4,224 file-path constellations, 1,139 failure evidence sets, 549 usefulness metrics. FSV manifest sha256 `3cd0e3c6…`.

**#414 / TASK-PY-G-118:** 128 Level-2 skills, 36,773 chunk-skill memberships, 10,579 reverse-index rows, 128 lifecycle audit. FSV: `fsv/task-py-g-118-skill-sequence-discovery-fsv/run-1779213075188/`, sha256 `0df1437a…`.

### 3.5 Runtime hierarchical ability resolver (#418 / TASK-PY-G-121)

Resolves all many-to-many chunk memberships; preserves higher-ability IDs and membership keys; passes them into prediction/replay; creates candidate ability when no safe existing ability explains a refuted prediction.

FSV: `fsv/task-py-g-121-runtime-ability-resolver-fsv/run-20260519T203036Z/`.

### 3.6 Chunk-skill linkage MCP surface (#417 first slice)

`mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`, `mejepa_chunk_as_star`, `mejepa_constellation_membership`, `mejepa_skill_impact`, `mejepa_skill_graph_inspect`, `mejepa_skill_conflict_graph`, `mejepa_skill_browse`. Read-only over #414 CFs.

### 3.7 Q4 doctrine freeze (#408 / TASK-PY-G-114)

`Q4_DOCTRINE_FREEZE_ACTIVE = true` in `q4_trust_gate.rs`. Pins `Q4_LOSS_COEFFICIENT_MAX = 0.0`, clamps reasoning/Q4 loss to zero, forces `q4_inputs_to_ship_gate() == 0`, adds weekly Q4-row-count-delta recheck.

FSV: `fsv/q4-doctrine-freeze-fsv/q4_doctrine_freeze_fsv.json`, sha256 `23408b7d…`. `q4RowCountDelta=0`, `q4LossCoefficientMax=0.0`, `q4InputsToShipGate=0`.

### 3.8 Slot-preserving constellation architecture (#385 parent — substantially shipped)

Closed children: #386 (schema), #387 (late-fusion predictor), #388 (cross-embedder tensors), #389 (per-slot calibration + OOD), #390 (consensus/contradiction heads), #391 (slot attribution), #394 (anti-compensation), #395 (real-corpus smoke), #396 (training objective), #452 (native active-embedder adapter).

Architectural invariant: predictor consumes panel as array of per-embedder vectors via per-slot encoders + late fusion + per-slot decoders. Cross-embedder dim compares structurally forbidden. Anti-compensation enforced.

Key shipped results:
- **#387 slot-preserving late-fusion predictor.** Live predictor uses per-slot encoders + late fusion + per-slot decoders instead of `linear(CONCAT_INPUT_DIM, hidden_dim)`. No-compensation trace proves one bad slot cannot be hidden by aggregate distance.
- **#388 cross-embedder correlation tensors.** 516 per-cell/window/slot distribution tensors, 2,838 per-cell/window upper-triangle pair tensors, 1,400 valid pair rows, 8 full cell/windows with all 66 active pair rows.
- **#389 per-slot calibration + OOD.** 12 global slot calibrations, 84 fail-closed per-cell slot rows, 66 pair calibrations, strict-conjunction OOD policy. `RealityPrediction.per_slot_ood_reasons` persists through `CF_MEJEPA_LIVE_PREDICTIONS`.
- **#390 constellation intelligence.** Real e7/e9 `zero_fraction` pair AUC `0.5882968` vs own-slot baseline `0.40438873` over 109 rows, lift `+0.18390805`.
- **#394 anti-compensation FSV.** Corrupted/missing/stale/dimension-shifted slots, contradictory pair relationships, temporal jumps remain non-ship-gate-eligible even when aggregate normalized scores would pass.

### 3.9 OOD harvest + serving gate (#276 / #129)

`CF_MEJEPA_OOD_HARVEST` + `CF_MEJEPA_OOD_CALIBRATIONS` + 4× active-learning projection + `OOD_GATE_OVER_FLAGGING` / `OOD_CELL_AUC_REGRESSION` / `OOD_HARVEST_EMPTY` / `OOD_CALIBRATOR_MISSING` / `OOD_SCORE_ABOVE_THRESHOLD`.

FSV: `fsv/task-py-g-014-ood-gate-fsv/run-20260521T145048Z/`, sha256 `d7f35e57…`.

### 3.10 Gτ guard audit (#130)

All 8 Python cells audited. Audit PASS but ship eligibility remains fail-closed: 6 low-pass cells, 1 missing substrate cell (`python:compile_error`), 8 per-cell calibration fail-closed cells, 26 missing reward-signal neighborhoods. `flatVectorConcatUsed=false`.

FSV: `fsv/task-py-g-015-gtau-guard-audit-fsv/run-20260521T142921Z/`, sha256 `169f6cc7…`.

### 3.11 Multi-head contradiction calibration (#263)

Per-cell `(tau_oracle, tau_failure_count)` thresholds. Verdict assembly abstains on calibrated Q2/pass vs high-severity Q3 conflict. Fails closed on missing thresholds. Records `SINGLE_HEAD_ONLY` when contradiction is impossible.

### 3.12 Live-session integration (#275 / #366)

5 real SWE-bench session traces. PASS: `allPassed=true`, 9 trace rows, 12 live prediction rows, 1 active-learning row, 9 runtime JSONL files. FSV: `fsv/task-py-g-056-live-session-fsv/run-20260518T212128Z/`, sha256 `4fe5d4af…`.

### 3.13 Q5 shift-log replay + calibration (#264 / #128)

`reality_impact.rs` materializes `RealityImpactRecord` from `CF_MEJEPA_LIVE_PREDICTIONS` + session shift logs. Classifies `Confirmed` / `Missed` / `NotYetObserved` / `Surprise`. Fails closed on SHA-chain gaps, unverifiable per-file SHA, before/after discontinuity, out-of-order timestamps. MCP: `mejepa_reality_impact`.

Q5 calibration writes `CF_MEJEPA_Q5_CALIBRATIONS` from replay records. Cells keyed by `(calibration_id, language, mutation_category, side_effect_kind)`. Computes TP/FP/FN, awaiting-oracle, support, precision/recall/F1, surprise rate.

### 3.14 Data-substrate audit chain (2026-05-22)

| Ref | Result |
|---|---|
| **#612** TASK-PY-G-135 data-substrate lineage audit | Typed blockers: `LEGACY_ABSOLUTE_PATHS_IN_AST_CHUNKS`, `INCOMPLETE_PER_EMBEDDER_SLOT_COVERAGE`, `DELETE_TEST_CALL_MISSING_EMBEDDER_SIGNAL` |
| **#613** TASK-PY-G-135A aiwonder AST source-lineage canonicalization | `promoted=true`. Source tree materialized: `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/` — 4,224 files, 148,346,301 bytes. Active rows now `0` `/mnt/d` or `/mnt/c` in `absolute_path`. Canonical AST sha256 `bc13e46a6262209b617a624f451e05e1f37e9dddd0d97b624538d7aefb9d0234` |
| **#614** TASK-PY-G-135B forward-cache slot-completion mask | `legal_missingness_mask_promoted=true`. Mask at `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl`: 399,777 rows (378,444 full 12-slot + 20,706 partial + 627 zero-slot). 131,552 missing chunk/embedder pairs all quarantined (`0` unaccounted). Active pair universe is 66 pairs per complete 12-slot row; 15-slot panel universe is 105 |
| **#615** TASK-PY-G-135C delete-test-call regeneration plan (in-progress) | Plan at `/zfs/archive/contextgraph/state/regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1/delete_test_call_regeneration_candidates.jsonl`: 1,131 deterministic candidates awaiting Docker oracle execution |

### 3.15 Aiwonder ZFS root migration (#372 — CLOSED 2026-05-22)

`/zfs/archive/contextgraph` → `archive/contextgraph` ZFS; `/zfs/hot/contextgraph` → `hotpool/contextgraph` ZFS. Legacy mounted-drive paths (`/mnt/d`, `/mnt/c`) forbidden in active code/FSV; enforced behaviorally by `scripts/ci/check_no_c_drive_paths.sh` + #612 → #613 → #614 → #615 → #631 → #632 → #641 → #666 audit chain.

### 3.15bis Trainer / checkpoint lineage unified (#683, CLOSED 2026-05-23)

Public Rust `Trainer::run_full_training` previously hard-coded `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE`. Production CLI checkpoint loading depended on optional `trained_checkpoint_manifest_path` — fresh `VarMap` predictor could be constructed if absent.

Closed 2026-05-23 via commits `77abb11a` (added countable predictor training path to public Trainer) + `dd6ad14e` (trained-checkpoint verifier CLI) + `fcf34053` (training certificates made countability-explicit). Production now fail-closed if `trained_checkpoint_manifest_path` absent. Rank 1 of #712 critical path discharged.

### 3.15ter Causal-embedder track filed (#744 / #745 / #746, 2026-05-23 / 2026-05-24)

Operator-directed follow-up to BLINDSPOT-AUDIT-2026-05-23 (#739). Three new issues at `priority:p1`, all aiwonder-only execution per `CLAUDE.md §6.7`:

| Issue | Title | Status | Pre-requisites |
|---|---|---|---|
| **#744** | TASK-CR-006 — Causal auto-labeling MVP on CommitPackFT | needs-triage | None (uses legacy v1 cache directly) |
| **#745** | TASK-CR-007 — In-slot causal embedder replacement (E5/E11/E14) | blocked | #744 PASS + #746 + #715 + #516 + #518 + #680 + #732 |
| **#746** | TASK-CR-008 — Low-signal embedder audit | blocked | #715 (closed: #683) |

In-slot replacement strategy: E5/E11 slots are empty (retired/disabled); E14 is "reserved." Repopulating these slots' encoders with causal embedders (C_TemporalCommit / C_CommonsenseCausal / C_BugFix) does not require schema change or #733 incremental cache as hard prerequisite. Only the per-slot forward cache re-bakes (~6-8h GPU on aiwonder RTX 5090 per slot, sequential).

Expected FSV artifacts (pending PHASE 1 execution):
- `/zfs/archive/contextgraph/fsv/task-cr-006-causal-auto-labeling-mvp-fsv/run-<TS>Z/` — cross-cluster transition tensor + asymmetric AUC matrix.
- `/zfs/archive/contextgraph/fsv/task-cr-007-in-slot-causal-replacement-fsv/<slot>/run-<TS>Z/` — per-slot replacement audit + promotion evidence + revert evidence (if applicable).
- `/zfs/archive/contextgraph/fsv/task-cr-008-low-signal-embedder-audit-fsv/{phase-1-ablation,phase-2-attribution,phase-3-live-variance}/` — per-phase audit evidence.
- `/zfs/archive/contextgraph/state/embedder-signal-audit/task-cr-008-signal-ranking-v1/embedder_replacement_priority.jsonl` — authoritative ranking.

### 3.16 Geometry-label epic opened (#657, in-progress)

Per-embedder vector-space label discovery via Leiden-on-kNN / HDBSCAN / spherical k-means / UMAP-diagnostic bakeoff. Static AST label discovery is scoped to the clean 9-slot policy (`e1,e6,e7,e8,e9,e10,e12,e13,e14`) after #704/#714 retired temporal slots `e2/e3/e4` for static AST. Downstream: #661 cross-embedder consensus → #662 gate-safe promotion → #664 router → #665 prediction heads.

---

## 4. FSV evidence catalog

Selected production artifacts. Full index: every closed Phase G issue documents specific test pass count + FSV path + sha256 in its RESOLVED comment.

| FSV | Path | sha256 |
|---|---|---|
| Ship-gate (latest) | `swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` | `8a018eaf…` |
| Mistake-loop MCP tools (#523) | `task-py-g-112b-mistake-loop-mcp-tools-fsv/run-20260520T113125Z/` | `9a9e50b6…` |
| Failure-mode hierarchy v2 (#404) | `task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779376526126/` | `77deb00b…` |
| Skill-sequence aiwonder materialization (#414) | `task-py-g-118-skill-sequence-discovery-fsv/run-1779213075188/` | `0df1437a…` |
| Auto-label discovery (#413) | `task-py-g-117-auto-label-discovery-usefulness-fsv/` | manifest `3cd0e3c6…` |
| Q4 doctrine freeze (#408) | `q4-doctrine-freeze-fsv/q4_doctrine_freeze_fsv.json` | `23408b7d…` |
| Cross-embedder tensors (#388) | `task-py-g-088-cross-embedder-correlation-tensors-fsv/` | `a547af92…` |
| Per-slot calibration + OOD (#389) | `task-py-g-089-per-slot-calibration-ood-fsv/` | `24075e0b…` |
| Slot-preserving late-fusion predictor (#387) | `task-py-g-087-slot-preserving-late-fusion-predictor-fsv/` | `9f2c5380…` |
| Anti-compensation FSV (#394) | `task-py-g-094-anti-compensation-fsv/` | `57f710b9…` |
| Constellation intelligence (#390) | `task-py-g-090-consensus-contradiction-heads-fsv/run-20260519T124035Z/` | `b1d500f0…` |
| Slot attribution (#391) | `task-py-g-091-slot-attribution-explanation-fsv/run-20260519T132343Z/` | `78ec5f36…` |
| Multi-head contradiction (#263) | `task-py-g-044-multi-head-contradiction-fsv/run-20260519T114854Z/` | `39fac460…` |
| OOD harvest (#276) | `task-py-g-057-ood-harvest-fsv/run-20260519T022705Z/` | `bc3f9519…` |
| OOD serving gate (#129) | `task-py-g-014-ood-gate-fsv/run-20260521T145048Z/` | `d7f35e57…` |
| Gτ guard audit (#130) | `task-py-g-015-gtau-guard-audit-fsv/run-20260521T142921Z/` | `169f6cc7…` |
| Q5 shift-log replay (#264) | `task-py-g-045-shift-log-replay-engine-fsv/` | `2e2116e2…` |
| Q5 calibration (#128) | `task-py-g-013-q5-calibration-fsv/` | `042bd8ad…` |
| Live-session integration (#275 / #366) | `task-py-g-056-live-session-fsv/run-20260518T212128Z/` | `4fe5d4af…` |
| Post-#547 mixed-cell diagnostic (#548) | `post-547-mixed-cell-ranking-repair-fsv/run-20260520T184458Z/` | `acd57aba…` |
| #549 mixed-cell signal fusion | `task-py-g-549-mixed-cell-signal-fusion-fsv/run-20260520T191159Z/` | `b6b391e…` |
| Post-#536 calibration disjointness (#547) | `post-536-calibration-disjointness-fsv/run-20260520T181254Z/` | `d90152eb…` |
| AST source-lineage canonicalization (#613) | `task-py-g-135a-aiwonder-source-lineage-canonicalization-fsv/run-20260522T140211Z/` | `e518bdac…` |
| Forward-cache slot-completion mask (#614) | `task-py-g-135b-forward-cache-slot-completion-fsv/run-20260522T141655Z/` | `8da6c7a2…` |
| Delete-test-call regeneration plan (#615) | `task-py-g-135c-delete-test-call-regeneration-fsv/run-20260522T143109Z/` | `c1431c11…` |
| Stop-hook circuit breaker (#381) | `task-py-g-080-stop-hook-circuit-breaker-fsv/run-20260519T031222Z-457165/` | `20b75ce3…` |
| Patch similarity (#247) | `task-py-g-042-patch-similarity-exemplar-fsv/run-1779075407903/` | `177cc3a7…` |

Frozen Q4 producer/calibration FSVs (display-only after #408): #266 perf labels (260), #267 accuracy labels (240), #268 cost labels (240), #269 reasoning labels (300), #270 non-trivial-on-Pass (500), #127 calibration, #377 trust gate, #399 perf re-proof, #400 cost re-proof, #401 harvest expansion.

---

## 5. Aiwonder production data map

Active Python ship-gate data:

| Artifact | Path | Role |
|---|---|---|
| 300×8 SWE-bench Lite Python corpus + oracle labels | `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-<TS>/index.json` | 2,400 entries; `oracle_all_passed` is label source |
| AST chunk store | `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | 399,777 chunks; stable row keys; active source paths canonicalized by #613 |
| AST chunk source files | `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/` | aiwonder-native source bytes; 4,224 files / 148,346,301 bytes |
| Legacy v1 forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-<TS>/` | 391 completed shards; retained as evidence only |
| Planned v2 static AST forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v2-static-ast-<TS>/` | #715 output; 9 active static slots (`e1,e6,e7,e8,e9,e10,e12,e13,e14`); promotion requires #709 distinctness proof |
| Forward-cache slot-completeness mask | `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl` | #614 output; 399,777 rows; prevents incomplete rows from being treated as complete all-pair constellations |
| Delete-test-call regeneration candidates | `/zfs/archive/contextgraph/state/regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1/delete_test_call_regeneration_candidates.jsonl` | #615 output; 1,131 deterministic candidates awaiting Docker oracle |
| Cross-embedder constellation tensor store | `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/` | #388: slot/pairwise tensors |
| Per-slot calibration and OOD guard | `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` | #389: 12 global + 84 per-cell + 66 pair + OOD policy |
| Level-0 label registry | `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/` | #413: 7 JSONL artifacts + manifest |
| Geometry-label root (#657, planned) | `/zfs/archive/contextgraph/state/labels/task-py-g-136-per-embedder-geometry-labels-v1/` | per-embedder geometry-label maps after #657 bakeoff |
| Q4 frozen baseline | `/zfs/archive/contextgraph/state/q4-freeze/q4_frozen_cf_counts.json` | #408: weekly delta-zero recheck |

---

## 6. Neuroscience-inspired reserve (DO NOT START until current queue concludes)

No results yet — all 8 issues (#538-#545) at `priority:p3` with `⚠ DO NOT START — Neuroscience-reserve contingency` markers. Activation depends on post-current-queue gap analysis. **If** activated and shipped, expected FSV shape:

| Issue | Expected proof artifact | Ablation comparison |
|---|---|---|
| #538 predictive-coding head | `task_py_neuro_001_predictive_coding_head_fsv.json` with per-slot predictions, residuals, slot-identity-preserved=true | Real-input baseline vs predictor-with-PC-head; corr + Brier deltas |
| #539 schema-prioritized replay | `task_py_neuro_002_replay_consolidation_fsv.json` with schema-grouped batch evidence, reverse-replay credit attribution | `mistake_repeat_rate` across schema clusters vs uniform-prior baseline |
| #540 hierarchical predictive coding | `task_py_neuro_003_hierarchical_predictive_fsv.json` with per-level error attribution | Hierarchy as backward-prediction vs hierarchy-as-aggregator-only |
| #541 mixed-selectivity binding | `task_py_neuro_004_mixed_selectivity_binding_fsv.json` with basis-function attribution + slot-identity preserved | Mixed-selectivity vs current #390 linear pairwise statistics |
| #542 / #543 / #544 / #545 | Design-influence; no separate FSV; additions to existing planned epic FSVs (#416 / #519 / #392 / #517) | Per-host-epic ablations |

See `08 §6` for activation rule and `05 §18.5` for pipeline-integration sketch.

---

## 7. What this doc does NOT cover

- Per-test timing or flake history (not centrally tracked beyond TASK-G-019 oracle flakiness quarantine).
- Pre-2026-05 detailed phase A-F FSV inventory — see closed issue RESOLVED comments.
- Operator personal hardware setup (`~/.config/mejepa/daemon.toml`, GPU driver versions, CUDA toolkit install).
- Off-aiwonder operator backup procedures — operator responsibility.
