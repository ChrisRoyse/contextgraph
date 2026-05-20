# 07 — Results, Benchmarks, and FSV Evidence History

Source: aiwonder `/zfs/archive/contextgraph/fsv/` for production evidence; git log; GitHub issue comments; per-issue RESOLVED comments.

**Purpose:** preserve the proof-of-work that future agents would otherwise lose. Every entry references a physical artifact (FSV JSON, sha256, CF) that can be re-verified.

---

## 0. ⚠ CRITICAL: pre-fix evidence partially invalidated

Between Phase G ramp and 2026-05-19 22:45 UTC, the inference path used SHA256-seeded synthetic vectors (`crates/context-graph-mejepa/src/compiler.rs::vectors_for` calling `deterministic_vector(slot, &digest)`) instead of real `E*Instrument` encoders. **The 0.866667 ship-gate measurement and every ablation that compared against it are INVALID** — they measured a synthetic-input pipeline, not the real predictor. See `01_overview.md §1.7` for the full forensic.

| Status | Evidence |
|---|---|
| **REAL (no rebaseline)** | AST chunks, 391-shard forward cache, #388 tensors, #389 calibration, #413 labels, #414 skills, #404 hierarchy, anti-compensation invariants |
| **INVALID (must rebaseline)** | Ship-gate 0.866667; `λ_inv` ablation; #393 / #398 ablations; "labels are the bottleneck" claim; "slot-preserving doesn't close gap" claim |

The corpus survives. The pre-fix measurement does not. The post-fix real-input rebaseline now exists, and Phase G is working from that number.

---

## 1. Current Ship-Gate Status

| Metric | Target | Latest measurement | Status |
|---|---|---|---|
| `prediction_oracle_correlation` | ≥ 0.95 (stable × 4 rolling windows × all Python mutation cells) | **0.7942190170 rank-lift calibrated** (`0.6916886568` rank-safe, `0.4843192995` raw) | Post-fix aiwonder real-input rebaseline exists. #535 code-state-constellation rank-lift improved correlation by `+0.1020384431` over #534; #536 hard-negative weighting added another `+0.0004919171` correlation, Brier `0.0936445221 → 0.0923729688`, ECE `0.0338404700 → 0.0240316074`. Gate still fails on per-cell ranking and statistical-power blockers. |
| Cross-panel oracle correlation (Panel B) | ≥ 0.95 with `\|corr_a − corr_b\| ≤ 0.05` | no measurement yet | #405 produces the first (after Panel A rebaseline lands) |
| `mistake_repeat_rate` | ≤ 0.05 (rolling 100-mistake window) | byte-readable via `mejepa_mistake_loop_status` | Awaits real predictor mistakes (machinery shipped #406/#523; needs post-fix predictor to generate disagreements) |

**Gap to ship:** `0.1557809830` against the rank-lift calibrated real-input Panel A measurement. Pre-fix `0.866667` is not comparable because it measured synthetic vectors.

**Metric contract update (#532).** Per-cell Pearson is not defined for zero-variance oracle cells. The active ship-gate FSV now classifies every block as `mixed`, `all_fail`, `all_pass`, or `empty`: mixed blocks use Pearson correlation; degenerate all-fail/all-pass blocks use target-class recall plus calibration (`actual_class_confidence_mean`, Brier, ECE). Each block also carries statistical-power diagnostics so a point estimate cannot be mistaken for a powered readiness claim.

**Calibration + invalid-source + rank-safety + rank-lift update (#533/#531/#534/#535/#536).** The ship-gate FSV fits deterministic calibration/rank-lift policies on the 240-row calibration split only, then evaluates the 240-row holdout. Constants are allowed only for structurally degenerate `compile_error` / `wrong_file` cells. Rank-reversing transforms are allowed only when the calibration split has mixed-class support ≥ 8 per class and raw Pearson ≤ -0.25; this prevents calibration-split overfit from flipping holdout ranking. The rank-lift head uses category-local RBF exemplar scoring over real `code_state_constellations.jsonl` slot-feature means and hashed live label counts while excluding target-side oracle/docker/test labels; #536 added calibration-selected train-only hard-negative weighting (`tau=20.0`, `fail_boost=1.5`). Latest FSV: `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T175714Z/`, report sha256 `3162179b93011b8d9f51865fb3535f2b713b178199216f8e42dd43b45da102ce`, readback sha256 `731912054a9e95ecb23ab8a05a73d1bf4de7a04bca6f215bba7d68aca581d19b`. `compile_error` rows use raw invalid-source diagnostic panels rather than carrier modules. #535 lifted overall calibrated correlation (`0.6916886568` → `0.7937270999`); #536 lifted again to `0.7942190170`, with Brier `0.0923729688` and ECE `0.0240316074`. #536 improved one-fail cell Pearson definition/Brier, `swap_variable` Pearson/Brier, `subtle_flip` Brier, and `off_by_one` fail recall; threshold behavior still saturates several weak cells, so this is a small repair, not a gate close.

**Real-input calibrated constellation ablation rerun (#537 / #398).** The #398-style calibrated constellation ablation was rerun against the then-current #535 `0.7937270999` baseline: `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-realinput-fsv/run-20260520T170156Z/`, FSV sha256 `03096e466278f60631328207b07fe0aba6c6ffca622d21a468fa167ad33d730a`, report sha256 `4d0f8af83f5ab7ffdf6541979336c55d232dfa2fdd2c88e231cd4a7deb2a6ee4`, metrics sha256 `62c2774fc05658bb4529b608a48e2a181299057ad6125b87bf22c16151a32e30`. Result remains `no_promotion`: calibrated full constellation correlation is `null`, Brier is `0.7539461538`, best local baseline Brier is `0.0085552058`, and all ship-eligible component ablations are `demote`. This is not a gate mover.

**Real-input uncalibrated constellation ablation rerun (#537 / #393).** The #393 companion suite was rerun against the same #535 baseline while consuming only the real #396 training arrays: `/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-realinput-fsv/run-20260520T181900Z/`, FSV sha256 `c1ed1efc4aaceef210619fc290dbd09a2fa0c2732a6f3f87969237f1a8460e26`, report sha256 `d83eed24863d3ba8e7e0f62e4b7dd6bc081d103ed5a13ddf0846de613a5aeb4c`, metrics sha256 `05aa0855e5cccc2b188add0b69193f91cb0e0d5b3ec8fe36105d85685e690376`. Result remains `no_promotion`: best uncalibrated ship-eligible family is `per_slot_pairwise_temporal` with correlation `0.2876047260`, Brier `0.2220880757`, and delta `-0.5061223739` vs #535. This is not a gate mover and is farther below the #536 measurement of record.

**Real-input `lambda_inv` ablation rerun (#546 / #537).** The EK-012 ablation harness was replaced after discovering the old path used `EchoPredictor` + `CategoryMajorityOracle`. New aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-ek-012-bidirectional-ablation-realinput-fsv/run-20260520T172657Z/bidirectional_ablation_realinput_fsv.json`, report sha256 `bebb84c8b7800fd5567803a43025ee19a88182b32c77f8a4eeee4c9eb2dc2dd4`, readback sha256 `f9e24a09758a38e90c7579181dbf5956f28648ea7de86d4a1b53c517d43c5cbe`. Result: all variants regress against the #535 `0.7937270999` predecessor rank-lift baseline. Best rank-safe predictor-output variant is `lambda_inv=0.0` at `0.6704055071` (`-0.1233215928` vs #535); positive lambdas are worse (`0.25 → 0.5562775135`, `0.5 → 0.5049043894`, `1.0 → 0.5726367831`). Typed finding: `regression`; keep `lambda_inv=0.0` for ship-gate ordering.

---

## 2. Recent Closures (recent → older)

### 2.1 Mistake-loop MCP tools (#523 / TASK-PY-G-112B — closed 2026-05-20)

`mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status` shipped.

FSV: `/zfs/archive/contextgraph/fsv/task-py-g-112b-mistake-loop-mcp-tools-fsv/run-20260520T113125Z/task_py_g_112b_mistake_loop_mcp_tools_fsv.json`, sha256 `9a9e50b6d6fdf8d3e4b64ab6edd992802121eb13b17663bc0f11786446411ae7`. Proves: all three tools registered; writes/reads land in RocksDB CFs; online head corrects same-panel mistake; lifecycle audit joins byte-readable; missing CFs fail closed; target-side oracle labels rejected as live inputs; invalid panel signatures leave no partial mistake/replay writes; `/mnt/d` not used for active I/O.

### 2.2 Failure-mode hierarchy (#404 / TASK-PY-G-110 — closed 2026-05-20)

8 Level-1 groups, 157 Level-2 skills, 379 Level-3 constellations, 3,169 Level-3 memberships, 536 named predictions, 12,658 total chunk-skill membership rows. Holdout: 114/114 Fail rows have FailLikely named-mode coverage; closest-exemplar oracle agreement 1.0 over 113/114 comparable holdout Fail rows.

FSV: `/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779273783982/`, sha256 `06b3e46a9a703cf0d1c52991784726f1b1f5577df7d7488360ebc651a698b863`.

### 2.3 Online mistake-driven loop (#406 / TASK-PY-G-112 — closed 2026-05-20)

Key-local exact-panel online-head state. Reports `protection_mode=exactPanelKeyIsolation`, `claims_fisher_ewc_protection=false`. Convergence verified: inject deliberate wrong prediction → second presentation of same panel signature yields corrected verdict. Unrelated-control delta within ±0.02.

### 2.4 Native active-embedder constellation adapter (#452 / TASK-PY-G-128 — closed 2026-05-20)

Preserves per-embedder vector spaces and ragged coverage masks instead of flattening active vectors. Unblocks #441 trained checkpoint export FSV.

### 2.5 Auto-label discovery + skill substrate (#413 / #414 — closed 2026-05-19)

**#413 / TASK-PY-G-117:** deterministic Level-0 label compiler (`python_auto_label_discovery.py`, 1,641 lines, no inner LLM). Full-corpus aiwonder FSV PASS: 399,777 chunk labels, 491 accepted label registry rows, 3,000 code-state constellations, 4,224 file-path constellations, 1,139 failure evidence sets, 549 usefulness metrics. FSV manifest sha256 `3cd0e3c6…`.

**#414 / TASK-PY-G-118:** 128 Level-2 skills, 36,773 chunk-skill memberships, 10,579 reverse-index rows, 128 lifecycle audit rows. Production aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-py-g-118-skill-sequence-discovery-fsv/run-1779213075188/`, sha256 `0df1437ae5e0542b356777b29df700b521b83b054113697ca280ae3d242d3dee`.

### 2.6 Runtime hierarchical ability resolver (#418 / TASK-PY-G-121 — closed 2026-05-19)

Resolves all many-to-many chunk memberships; preserves higher-ability IDs and membership keys; passes them into prediction/replay; creates candidate ability when no safe existing ability explains a refuted prediction.

FSV: `/zfs/archive/contextgraph/fsv/task-py-g-121-runtime-ability-resolver-fsv/run-20260519T203036Z/`.

### 2.7 First chunk-skill linkage MCP slice (#417 first slice — shipped 2026-05-19)

`mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`. Read-only over #414 CFs.

FSV: `/zfs/archive/contextgraph/fsv/task-py-g-120-skill-linkage-surface-fsv/run-20260519Tcodex1Z/`.

### 2.8 Q4 doctrine freeze (#408 / TASK-PY-G-114 — closed 2026-05-19)

`Q4_DOCTRINE_FREEZE_ACTIVE = true` in `q4_trust_gate.rs`. Pins `Q4_LOSS_COEFFICIENT_MAX = 0.0`, clamps reasoning/Q4 loss assembly to zero, forces `q4_inputs_to_ship_gate() == 0`, adds weekly Q4-row-count-delta recheck.

FSV: `/zfs/archive/contextgraph/fsv/q4-doctrine-freeze-fsv/q4_doctrine_freeze_fsv.json`, sha256 `23408b7d…`. Proves: `q4RowCountDelta=0`, `q4LossCoefficientMax=0.0`, `q4InputsToShipGate=0`, historical Q4 remains readable, new Q4 fields empty, weekly dashboard fails closed on frozen CF growth.

### 2.9 Slot-preserving constellation architecture (#385/#386/#387/#388/#389/#390/#391/#393/#394/#395/#396/#398 — closed 2026-05-19) — partially invalidated

**⚠ The architectural / invariant work (#386 schema, #387 late-fusion structure, #388 tensor materialization, #389 calibration state, #391 attribution surfaces, #394 anti-compensation invariant) SURVIVES.** Those proofs are about structural properties — they hold regardless of input signal quality.

**❌ The numerical ablation results (#393 / #398) are INVALID.** They measured the slot-preserving predictor against synthetic-input panels coming out of `compiler.rs::vectors_for` → `deterministic_vector`. The "best ablation `per_slot_pairwise_temporal` ΔBrier `+0.21353…` Δcorrelation `-0.5790…`" finding is meaningless — the predictor was fitting hash noise, not real cross-embedder relationships. Same for the `no_promotion` recommendation (calibrated full-constellation Brier `0.7539…` vs best local baseline `0.00855…`).

**❌ The framing "slot-preserving architecture is correct but doesn't close the 0.083 gap by itself" is also INVALID.** That conclusion came from synthetic-input measurements. The real-input Panel A baseline is now `0.7942190170`; the #398-style calibrated rerun and EK-012 `λ_inv` rerun are complete and negative, but they do not validate the old pre-fix conclusion.

Architectural correction from flat-panel projection to slot-preserving array operations. All children closed; remaining open child #392 (temporal trajectory features) is a follow-up signal surface.

Key results:
- **#387 slot-preserving late-fusion predictor:** live predictor uses per-slot encoders + late fusion + per-slot decoders instead of `linear(CONCAT_INPUT_DIM, hidden_dim)`. No-compensation trace proves one bad slot cannot be hidden by aggregate distance.
- **#388 cross-embedder correlation tensors:** 516 per-cell/window/slot distribution tensors, 2,838 per-cell/window upper-triangle pair tensors, 1,400 valid pair rows, 8 full cell/windows with all 66 active pair rows.
- **#389 per-slot calibration + OOD:** 12 global slot calibrations, 84 fail-closed per-cell slot rows, 66 pair calibrations, strict-conjunction OOD policy. `RealityPrediction.per_slot_ood_reasons` persists through `CF_MEJEPA_LIVE_PREDICTIONS`.
- **#390 constellation intelligence:** found real e7/e9 `zero_fraction` pair AUC `0.5882968` vs own-slot baseline `0.40438873` over 109 rows, lift `+0.18390805`.
- **#393 / #398 ablations:** both real-input reruns were measured against the #535 Panel A baseline (`0.7937270999`) and remain `no_promotion`; #536 has since moved the measurement of record to `0.7942190170`. Best #393 uncalibrated family is `per_slot_pairwise_temporal` at `0.2876047260`; #398 calibrated full constellation remains non-promotable. EK-012 `lambda_inv` is also remeasured by #546; best rank-safe variant is `lambda_inv=0.0` at `0.6704055071`, so positive `lambda_inv` is not a gate mover.
- **#394 anti-compensation FSV:** corrupted/missing/stale/dimension-shifted slots, contradictory pair relationships, and temporal jumps remain non-ship-gate-eligible even when aggregate normalized scores would pass.

### 2.10 OOD harvest + serving gate (#276 / #129 — closed 2026-05-19)

`CF_MEJEPA_OOD_HARVEST` + `CF_MEJEPA_OOD_CALIBRATIONS` + 4× active-learning projection + `OOD_GATE_OVER_FLAGGING` / `OOD_CELL_AUC_REGRESSION` / `OOD_HARVEST_EMPTY` / `OOD_CALIBRATOR_MISSING` / `OOD_SCORE_ABOVE_THRESHOLD`.

### 2.11 Gτ guard audit (#130 — closed 2026-05-19)

All 8 Python cells audited. Result: audit PASS but ship eligibility remains fail-closed: 6 low-pass cells, 1 missing substrate cell (`python:compile_error`), 8 per-cell calibration fail-closed cells, 26 missing reward-signal neighborhoods.

### 2.12 Multi-head contradiction calibration (#263 — closed 2026-05-19)

Per-cell `(tau_oracle, tau_failure_count)` thresholds. Verdict assembly abstains on calibrated Q2/pass vs high-severity Q3 conflict. Fails closed on missing thresholds. Records `SINGLE_HEAD_ONLY` when contradiction is impossible.

### 2.13 Live-session integration (#275 / #366 — closed 2026-05-18)

5 real SWE-bench session traces. PASS: `allPassed=true`, 9 trace rows, 12 live prediction rows, 1 active-learning row, 9 runtime JSONL files. FSV: `/zfs/archive/contextgraph/fsv/task-py-g-056-live-session-fsv/run-20260518T212128Z/live_session_fsv.json`, sha256 `4fe5d4af…`.

### 2.14 Q5 shift-log replay + calibration (#264 / #128 — closed 2026-05-19)

`reality_impact.rs` materializes `RealityImpactRecord` from `CF_MEJEPA_LIVE_PREDICTIONS` + session shift logs. Classifies `Confirmed` / `Missed` / `NotYetObserved` / `Surprise`. Fails closed on SHA-chain gaps, unverifiable per-file SHA, before/after discontinuity, out-of-order timestamps. MCP: `mejepa_reality_impact`.

Q5 calibration writes `CF_MEJEPA_Q5_CALIBRATIONS` from replay records. Cells keyed by `(calibration_id, language, mutation_category, side_effect_kind)`. Computes TP/FP/FN, awaiting-oracle, support, precision/recall/F1, surprise rate.

---

## 3. Aiwonder Production Data Map

Active Python ship-gate data lives in distinct aiwonder artifacts:

| Artifact | Path | Role |
|---|---|---|
| 300×8 SWE-bench Lite Python corpus + oracle labels | `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-<TS>/index.json` | 2,400 entries; `oracle_all_passed` is label source |
| AST chunk store | `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | Chunk metadata, stable row keys |
| Full packed embedder forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-<TS>/` | 391 completed shards; per-shard packed manifests; 12 active embedders |
| Cross-embedder constellation tensor store | `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/` | #388 output: slot/pairwise tensors |
| Per-slot calibration and OOD guard | `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` | #389 output: 12 global + 84 per-cell + 66 pair + OOD policy |
| Level-0 label registry | `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/` | #413: 7 JSONL artifacts + manifest |
| Q4 frozen baseline | `/zfs/archive/contextgraph/state/q4-freeze/q4_frozen_cf_counts.json` | #408: weekly delta-zero recheck |

---

## 4. Phase Status Summary

### 4.1 Phases A-F: code/FSV shipped

All Phase A-D implementation issues (#3-#19 umbrella #15) CLOSED. Phase E self-optimization policy shipped. Phase F production operations + observability + runbook + DR + API shipped. Independent audit verified 35/35 items present.

### 4.2 Phase G: in flight — model-quality convergence

Open work targeting the 0.95 ship gate. See `08_roadmap_and_open_work.md`.

### 4.3 Phase H: complete — inner-actor LLM retirement

`feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`. Phase H absence-checks at `scripts/ci/phase_h_*.sh` assert deletion of Qwen / Hermes / Ollama / llama.cpp generative inference.

---

## 5. Key Recent Architectural Discoveries

### 5.1 Synthetic-vector contamination invalidated every pre-fix correlation finding

**This is the headline discovery.** Between Phase G ramp and 2026-05-19 22:45 UTC, the inference path bypassed the real frozen instruments and filled panel slots with SHA256-seeded synthetic vectors. The predictor was learning a hash-derived prior, not real code semantics. The same number 0.8666666746139526 verbatim appeared across every single ablation (`λ_inv ∈ {0, 0.25, 0.5, 1.0}`) precisely because the input never changed — only downstream compute did, and a deterministic-input system converges to a deterministic-output fixed point.

Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452) document and fix this. The corpus + label substrate survive (real Candle CUDA forward generated them); only the inference-path measurements were contaminated.

### 5.2 Slot-preserving refactor is doctrinally correct (status update)

The architectural correction from flat-panel projection (#385) to slot-preserving array operations (#386-#398) IS doctrinally correct: cross-embedder dim-wise compares are now structurally forbidden, anti-compensation is enforced, per-slot calibration + OOD reasons persist. **But the previous claim that "slot-preserving architecture doesn't close the 0.083 gap by itself" is INVALID** — that conclusion was drawn from #398's calibrated ablation rerun, which ran the slot-preserving predictor against synthetic-input panels. The real-input #393, #398, and #546 reruns are now negative evidence, so queue ordering should move to targeted weak-cell repair rather than another constellation-wide promotion attempt.

### 5.3 Evidence-weighted ordering (#383, pinned context) — recheck required

#383 says gate-active issue order must be evidence-weighted, not vision-weighted. **The "evidence" that drove the recent reordering was from synthetic-input ablations and is itself invalidated.** Re-rank from the `0.7942190170` real Panel A measurement; with #393/#398/#546 remeasured and not promoted, and #536 only a small hard-negative weighting lift, the top of the queue is deeper calibration-disjointness / anti-Goodhart proof plus mixed-cell ranking repair.

### 5.4 The labeling layer is the meaning surface (#404 / #413 / #414 / #418) — substrate stands

Raw slot violations + cross-embedder correlations only become actionable when clustered into a 4-level hierarchy of named failure modes grounded by closest historical exemplars. The 491-row accepted Level-0 registry + 128 Level-2 skills + 379 Level-3 constellations + 36,773 chunk-skill memberships are all derived from the real forward cache and survive the synthetic-vector bug. The HOLDOUT measurements (114/114 named-mode coverage, 1.0 closest-exemplar agreement) were against pre-fix prediction rows and need re-validation on post-fix predictions.

### 5.5 The mistake loop replaces best-of-K (#406)

Best-of-K papers over an under-trained predictor by spending compute at inference time. The online mistake loop pays the cost at *learning* time and converges to a predictor that is right on the first try. Current shipped primitive: key-local exact-panel online-head state with `protection_mode=exactPanelKeyIsolation`. Convergence target: `mistake_repeat_rate ≤ 0.05`. The machinery is shipped; needs real predictor mistakes from a real trained checkpoint to validate convergence.

### 5.5 Connectome topology features are diagnostic-only (#513 / #527)

FlyWire-style rich-club analysis tested but demoted: split-stable evidence improved (`cellsSplitReproducibleAt0_8=7/8`) but the stable feature is not gate-moving (modularity above null `1/8`, full finite-edge coverage `0`, diagnostic correlation regressed by `-0.583`). Topology harness kept diagnostic-only; not promoted as constellation surface.

---

## 6. FSV Evidence Catalog (active production)

Selected key recent artifacts. Full index: every closed Phase G issue documents its specific test pass count + FSV path + sha256 in its RESOLVED comment.

| FSV | Path | sha256 |
|---|---|---|
| Mistake-loop MCP tools (#523) | `task-py-g-112b-mistake-loop-mcp-tools-fsv/run-20260520T113125Z/` | `9a9e50b6…` |
| Failure-mode hierarchy v2 (#404) | `task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779273783982/` | `06b3e46a…` |
| Skill-sequence aiwonder materialization (#414) | `task-py-g-118-skill-sequence-discovery-fsv/run-1779213075188/` | `0df1437a…` |
| Auto-label discovery (#413) | `task-py-g-117-auto-label-discovery-usefulness-fsv/` | manifest `3cd0e3c6…` |
| Q4 doctrine freeze (#408) | `q4-doctrine-freeze-fsv/q4_doctrine_freeze_fsv.json` | `23408b7d…` |
| Cross-embedder tensors (#388) | `task-py-g-088-cross-embedder-correlation-tensors-fsv/` | `a547af92…` |
| Per-slot calibration + OOD (#389) | `task-py-g-089-per-slot-calibration-ood-fsv/` | `24075e0b…` |
| Slot-preserving late-fusion predictor (#387) | `task-py-g-087-slot-preserving-late-fusion-predictor-fsv/` | `9f2c5380…` |
| Calibrated ablation rerun (#398, `no_promotion`) | `task-py-g-097-constellation-ablation-with-calibration-fsv/` | `90039db2…` |
| Anti-compensation FSV (#394) | `task-py-g-094-anti-compensation-fsv/` | `57f710b9…` |
| Constellation intelligence (#390) | `task-py-g-090-consensus-contradiction-heads-fsv/run-20260519T124035Z/` | `b1d500f0…` |
| Slot attribution (#391) | `task-py-g-091-slot-attribution-explanation-fsv/run-20260519T132343Z/` | `78ec5f36…` |
| Multi-head contradiction (#263) | `task-py-g-044-multi-head-contradiction-fsv/run-20260519T114854Z/` | `39fac460…` |
| OOD harvest (#276) | `task-py-g-057-ood-harvest-fsv/run-20260519T022705Z/` | `bc3f9519…` |
| OOD serving gate (#129) | `task-py-g-014-ood-gate-fsv/run-20260519T104711Z/` | `58c46eab…` |
| Gτ guard audit (#130) | `task-py-g-015-gtau-guard-audit-fsv/run-20260519T111048Z/` | `48574d33…` |
| Q5 shift-log replay (#264) | `task-py-g-045-shift-log-replay-engine-fsv/` | `2e2116e2…` |
| Q5 calibration (#128) | `task-py-g-013-q5-calibration-fsv/` | `042bd8ad…` |
| Live-session integration (#275 / #366) | `task-py-g-056-live-session-fsv/run-20260518T212128Z/` | `4fe5d4af…` |
| Stop-hook circuit breaker (#381) | `task-py-g-080-stop-hook-circuit-breaker-fsv/run-20260519T031222Z-457165/` | `20b75ce3…` |
| Patch similarity (#247) | `task-py-g-042-patch-similarity-exemplar-fsv/run-1779075407903/` | `177cc3a7…` |
| Secret/delayed holdout guard (#374) | `task-py-g-077-secret-holdout-fsv/` (result `underpowered`) | `16c1a5a6…` |
| Oracle-noise statistical power (#376) | `task-py-g-076-oracle-noise-power-fsv/` (result `underpowered`) | `1324dab0…` |

Frozen Q4 producer/calibration FSVs (display-only after #408): #266 perf labels (260 rows), #267 accuracy labels (240), #268 cost labels (240), #269 reasoning labels (300), #270 non-trivial-on-Pass (500), #127 calibration, #377 trust gate, #399 perf re-proof, #400 cost re-proof, #401 harvest expansion.

---

## 6.5 Neuroscience-Inspired Reserve (filed 2026-05-20, DO NOT START until current queue concludes)

No results yet — all eight neuroscience-reserve issues (#538-#545) are filed at `priority:p3` with explicit `⚠ DO NOT START — Neuroscience-reserve contingency` markers. Activation depends on post-current-queue gap analysis identifying need. **If** activated and shipped, expected FSV evidence shape:

| Issue | Expected proof artifact | Expected ablation comparison |
|---|---|---|
| #538 predictive-coding head | `task_py_neuro_001_predictive_coding_head_fsv.json` with per-slot predictions, residuals, slot-identity-preserved=true | Real-input baseline vs predictor-with-PC-head; persist correlation + Brier deltas |
| #539 schema-prioritized replay | `task_py_neuro_002_replay_consolidation_fsv.json` with schema-grouped batch evidence, reverse-replay credit attribution | `mistake_repeat_rate` across schema clusters vs uniform-prior baseline |
| #540 hierarchical predictive coding | `task_py_neuro_003_hierarchical_predictive_fsv.json` with per-level error attribution | Hierarchy as backward-prediction vs hierarchy-as-aggregator-only |
| #541 mixed-selectivity binding | `task_py_neuro_004_mixed_selectivity_binding_fsv.json` with basis-function attribution + slot-identity preserved | Mixed-selectivity vs current #390 linear pairwise statistics |
| #542 / #543 / #544 / #545 | Design-influence; no separate FSV; manifest as additions to existing planned epic FSVs (#416 / #519 / #392 / #517) | Per-host-epic ablations |

See `08_roadmap_and_open_work.md §6.9` for activation rule and `05_predictor_pipeline.md §18.5` for pipeline-integration sketch.

---

## 7. What This Doc Does NOT Cover

- Per-test timing or flake history (not centrally tracked beyond TASK-G-019 oracle flakiness quarantine).
- Pre-2026-05 detailed phase A-F FSV inventory — see closed issue RESOLVED comments.
- Operator personal hardware setup (`~/.config/mejepa/daemon.toml`, GPU driver versions, CUDA toolkit install).
- Off-aiwonder operator backup procedures — operator responsibility.
