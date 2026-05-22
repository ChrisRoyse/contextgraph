# 08 — Roadmap and Open Work

Source: `gh issue list --state open`, recent `type:decision` closes, git log, GitHub issue comments.

---

## 0. Doctrine Reminder (binary contract)

The prediction contract is **three binary surfaces against ground truth** (Q1 bytes/AST, Q2 Docker oracle, Q5 shift log), **one explanation surface** (Q3 named failure mode + closest exemplar), and **Q4 removed entirely** (ambiguous heads not predictable). See `01_overview.md §1.1` and `05_predictor_pipeline.md §0`.

---

## 0.5. P0 BLOCKER: Ship-gate measurement requires deeper ranking repair after first weak-cell lift

Between Phase G ramp and 2026-05-19 22:45 UTC, the inference path used SHA256-seeded synthetic vectors instead of calling real `E*Instrument` encoders. Six P0 fixes (#422 / #424 / #426 / #437 / #438 / #441 / #452, all closed 2026-05-19/20) replaced the broken pipeline. **The previous `0.866667` ship-gate measurement is INVALID** — it was the deterministic fixed-point output of a hash-derived predictor, not a real measurement of model quality on real code semantics. See `01_overview.md §1.7` and `07_results_and_benchmarks.md §0` for forensic detail.

**Current real-input state (#536/#547/#548/#549/#392/#554/#557/#558/#560):** aiwonder FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` reports raw holdout `prediction_oracle_correlation = 0.4843192995`, rank-safe calibrated holdout `0.6916886568`, and real code-state-constellation rank-lift holdout `0.7942190170`; it also carries first-class #392 active/abstain diagnostics. Earlier row-trace run `run-20260520T185945Z` preserves #536 metrics and provides the raw/rank-safe/rank-lift row traces consumed by #549/#392. Calibration/rank-lift are fitted only on the 240-row calibration split, constants are restricted to structurally degenerate cells, rank-reversing transforms require mixed-class support ≥ 8 per class plus raw Pearson ≤ -0.25, and rank-lift excludes target-side oracle/docker/test labels. #536 selected train-only hard-negative weighting (`tau=20.0`, `fail_boost=1.5`) and improved correlation/Brier/ECE over #535, but the lift is small. #547 proved the #536 policy is split-disjoint and not holdout-selected. #548 proved typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED`: holdout-selected thresholds can improve the metric, but legal calibration-selected candidates leave true mixed cells saturated. #549 proved static repo/path metadata and calibration-selected raw/rank-safe/rank-lift score fusion are useful but insufficient. #392 aggregate trajectories first lifted to `0.8008320332`; file-path plus skill/ability trajectories lifted to `0.8422425985`; sparse out-of-fold probability bins regressed ECE. The hardened #392 support-aware slice at `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T214049Z/` selected global mean-bias at correlation `0.8497869968`, Brier `0.0697008073`, ECE `0.0299093761`, and accuracy `0.9083333611`. #554 proved targeted usefulness-weighted ordered skills increase consequence signal but remain ECE-sensitive. #558 then rematerialized the full v2 label corpus (`399777` chunk rows, `398215` episodes, `157` skills, `9489` memberships) and reran #392 from the new DB. Before the later #551/#392 correction, the strongest #558 trajectory candidate was `usefulness_weighted_combined`: correlation `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`, accuracy `0.9083333611`, pass recall `0.9166666865`, fail recall `0.9000000`. #560 added a split-legal ECE-focused calibration diagnostic and proved it selects the same model and stays ECE-blocked (`0.0396374688`; support-shrink best `0.0396139733`). #557 then added the usefulness audit surface needed to keep the next repair targeted: source-byte representative sequences, lifecycle/demotion history, gap views, generic high-support rejections, and blanket-expansion regression evidence are now queryable without creating a new prediction head. This is a real lift, but it is still not promotable because ECE remains above the #536 baseline `0.0240316074`. The next lift must repair ECE through per-cell/per-constellation consequence evidence, cross-window validation, or true-mixed feature specialization, then pass Panel B / anti-Goodhart validation; do not spend more time on synthetic-input rebaselines, static score blends, aggregate-only trajectory, sparse bin calibration, affine/rank-lift blending, logit calibration, ECE-overweighted selection, support-only shrinkage, blanket skill expansion, or holdout threshold tunes.

**2026-05-21 #551/#392 correction:** #551 is no longer an open P0 coverage repair. The uncovered `1469 / 3000` code states are pass/context-only; all `1139 / 1139` failure-evidence rows are skill-covered, and coverage-cap/pass-context expansion regressed #392. The corrected #392 rerun after removing evidence-consequence double-calibration is `/zfs/archive/contextgraph/fsv/task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` (`all_passed=true`, report sha256 `9ed8830f184e0aca07a1ee47897b31b342dbdaf2db0bacc28d0a08981dfbc29c`). Legal best was mean-bias at correlation `0.8770877719`, Brier `0.0581485741`, ECE `0.0475411229`; evidence-consequence and bin-consequence were worse after correction. The later evidence-semantics hardening run `/zfs/archive/contextgraph/fsv/task-py-g-392-evidence-semantics-hardening-fsv-release/run-20260521T074421Z/` proves standardized cause-residual features and nonzero cause-bin application, selecting cause-bin calibration at correlation `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`. The final-bin diagnostic run `/zfs/archive/contextgraph/fsv/task-py-g-392-final-bin-guarded-fsv/run-20260521T090040Z/` slightly improves correlation/Brier but worsens ECE and is selector-forbidden, so the legal best remains cause-bin. The latest ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` wires #392 active/abstain into first-class gate accounting. It validates the diagnostic (`205 / 240` active rows, active coverage `0.8541666865`, corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`) but full active/abstain gate remains false because `delete_test_call`, `off_by_one`, `subtle_flip`, rolling-window, and statistical-power checks fail. The #392 path stays P0 only through targeted active-cell/window/power repair without holdout tuning.

**2026-05-22 #610/#150/#611/#612/#613/#614 data-substrate decisions:** #372 aiwonder root readiness and comparable #405/#521 Panel B parity are green, but burn-in is still not admissible. The retained #610 FSV `/zfs/archive/contextgraph/fsv/task-py-g-133-burn-in-admissibility-policy-fsv/run-20260522T123721Z/task_py_g_133_burn_in_admissibility_policy_fsv.json` (`sha256 d766c06d8da35f5a1e55cfbc7981b464ce65a595d910ca16bfbc15c7e465658c`) reports `all_passed=true`, `burn_in_start_allowed=false`, and `decision_code=TASK_PY_G_133_BURN_IN_POWER_NOT_ADMISSIBLE`. The #150 follow-up FSV `/zfs/archive/contextgraph/fsv/task-py-g-019-sample-efficiency-power-fsv/run-20260522T124456Z/task_py_g_019_sample_efficiency_power_fsv.json` (`sha256 585c54502084c6e96037d1c49df6b901a48822284a8c839c76c9542fe8513019`) reports `support_policy=blocked`: row support / longer windows alone cannot start #167 because active point correlations are below target for `delete_test_call`, `off_by_one`, and `subtle_flip`. #611 then proved no existing legal repair is promotable from the current aiwonder artifacts: `/zfs/archive/contextgraph/fsv/task-py-g-134-active-point-metric-repair-fsv/run-20260522T130250Z/task_py_g_134_active_point_metric_repair_fsv.json` (`sha256 ccea46d6ac5677cfc7a9b849c9dd70322570ae281ba6a7b8a23d76e6d772704f`) reports `all_passed=true`, `repair_promoted=false`, and typed blocker `TASK_PY_G_134_NO_EXISTING_LEGAL_WEAK_CELL_REPAIR`. #612 pivoted to the data substrate and produced `/zfs/archive/contextgraph/fsv/task-py-g-135-weak-cell-evidence-expansion-fsv/run-20260522T133526Z/task_py_g_135_data_substrate_lineage_fsv.json` (`sha256 5ff5ab01f4c1fba1d1286dded66c2d3941d01021717257fea0c9cf950ea609ed`): raw AST chunk rows had `399777 / 399777` legacy `/mnt/d` `absolute_path` + `dataschema` values, aiwonder had no `/mnt/d`, path-dereference consumers still existed, active v2 labels were scrubbed (`0` `/mnt/d` lines), the forward cache had real vectors but only `4665772 / 4797324` expected chunk/embedder records, and `delete_test_call` had only `3 / 300` oracle failures with zero calibration failures. #613 resolved the active source-lineage half: `/zfs/archive/contextgraph/fsv/task-py-g-135a-aiwonder-source-lineage-canonicalization-fsv/run-20260522T140211Z/task_py_g_135a_aiwonder_source_lineage_canonicalization_fsv.json` (`sha256 e518bdacb20281b6907bf119fae4b506298bebb4f5ed9b5c86f6299efc7bf13a`) reports `all_passed=true`; active AST rows now have `0` `/mnt/d` or `/mnt/c` hits in `absolute_path`/`dataschema`, with legacy values retained only as `legacy_*` provenance. #614 then produced the legal slot-completeness mask: `/zfs/archive/contextgraph/fsv/task-py-g-135b-forward-cache-slot-completion-fsv/run-20260522T141655Z/task_py_g_135b_forward_cache_slot_completion_fsv.json` (`sha256 8da6c7a213aa3b442b4909e5fae4ec170b827022c819132d0f5ab38ab0334092`) reports `all_passed=true`, `legal_missingness_mask_promoted=true`, and `slot_completion_promoted=false`. All `131552` missing chunk/embedder pairs are quarantined (`0` unaccounted); the mask is `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl` (`399777` rows). #615/#616 now remain as the immediate P0 data-substrate queue; downstream reruns may proceed only if they join the #614 mask and preserve missingness features/guards.

**Remaining revalidation queue:**

1. #615: regenerate `delete_test_call` as assertion-duty subfamilies with split-stable train/calibration failures.
2. #616: rerun and classify weak-cell oracle timeout/error rows for `off_by_one` and `subtle_flip`.
3. Rerun #392, then #611/#150/#610, after the relevant #615-#616 data repairs exist and all reruns join the #614 completeness mask.
4. Keep #167 blocked until #610 returns `burn_in_start_allowed=true`; then start four consecutive windows on the aiwonder ZFS roots.
5. ✅ #129 OOD serving gate, #130 Gτ guard audit, #404 holdout coverage, #529 synthetic-vector rebaseline, #372 aiwonder root readiness, comparable #405/#521 Panel B parity, #610 burn-in decision, #150 row/window power decision, #611 no-existing-repair falsification, #612 data-substrate audit, #613 source-lineage canonicalization, and #614 forward-cache completeness mask are green/retained for the current queue.

#529 is now a closed context: the synthetic-vector rebaseline and downstream #129/#130/#404 revalidations all have post-fix evidence. #536, #537, #547, #548, #549, #554, #557, #558, #560, and #551 are closed evidence; #392 is active with thirteen non-promotable/negative-control slices: aggregate trajectory, file/skill trajectory lift, out-of-fold probability calibration, affine file/skill calibration, affine/rank-lift blending, logit calibration, mean-bias calibration, ECE-overweighted selection, support-aware coverage hardening, usefulness-weighted skill policy ablation, full-corpus targeted skill rematerialization, ECE-focused mean-bias repair, and corrected skill-kind evidence-consequence/bin-consequence repair. **All issues listed in §3 below assume the real-input rebaseline, #536 rank-lift measurement, #547 disjointness proof, #548 typed blocker, #549 static-signal blocker, #392 trajectory-ECE blocker, #554 skill-usefulness ECE blocker, #557 usefulness audit surface, #558 rematerialized-skill ECE blocker, #560 ECE-repair blocker, and #551 justified-coverage-gap negative control exist unless explicitly noted otherwise.**

---

## 1. Current Ship Gate

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per mathematically-valid mixed (mutation_category × language) cell
  with all-pass/all-fail cells using the #532 degenerate-cell replacement metric
  on the 300×8 Python SWE-bench Lite corpus
  using Q2 Docker-oracle agreement as the binary verdict
  AND cross-panel oracle correlation ≥ 0.95 on Panel B
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Last holdout measurement:** binary rank-lift remains `0.7942190170` from real code-state constellation labels (`0.6916886568` rank-safe calibrated, `0.4843192995` raw) in `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`. That same FSV carries #392 active/abstain diagnostics: active corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978` on `205 / 240` holdout rows, but full active/abstain gate remains false. Previous `0.866667` figure (from legacy pre-migration evidence) was measured against the synthetic-vector inference path and is invalidated.

**Gap to ship:** `0.1557809830` against the rank-lift calibrated real-input Panel A measurement, plus four-window stability and Panel B cross-checks.

**Degenerate-cell rule (#532).** `compile_error::python`, `wrong_file::python`, or any future single-class cell cannot honestly use Pearson correlation. The FSV must emit the cell's variance class, the replacement gate metric (`all_fail_recall_confidence_brier_ece` or `all_pass_recall_confidence_brier_ece`), and statistical-power diagnostics instead of a bare `correlation unavailable` blocker.

**P0 product contract (#397):** ME-JEPA is the binary reality predictor. It returns `Pass | Fail` for each candidate code change against the Docker oracle (Q2), with named-failure-mode + closest-exemplar explanation (Q3) on Fail. Q1 (claim-exists) and Q5 (predicted-shift-event) are additional binary predictions. Q4 ambiguous surfaces are out of scope.

Once met → human review of AI-generated Python patches is statistically unjustified. Auto-trust policy gates engage (#277).

---

## 2. Open Issue Snapshot (live tracker is authoritative)

By phase:

| Label | Open count |
|---|---|
| `phase:g` | ~70 (this is the critical path) |
| `phase:migration` | 4 |
| `phase:f-followup` | 1 |
| `phase:f` / `phase:h` | 0 (closed) |

By priority:

| Priority | Examples |
|---|---|
| `priority:p0` | #615 delete-test-call regeneration, #616 oracle exception reruns, #612 parent data-substrate audit, #397 consequence-prediction contract, #383 evidence-weighted ordering, #167 burn-in (blocked by #610), #165 confidence gate (blocked by #167) |
| `priority:p1` | #630 long-source/OOM forward-cache quarantine recovery, #379 multi-file holdout, #416 pathway, #417 skill linkage remaining, #511 entropy loss, #380 off-host backup, #433 ability resolver replay completeness |
| `priority:p2` | EK series tail, post-ship policies, #382 frozen-panel dimensionality ablation, #378 tool-usage telemetry |
| `priority:p3` | Deferred (SWE-bench Verified subset, v2 stretch tasks, future Docker image plan) |

---

## 3. Critical Path (evidence-weighted ordering)

Per pinned context #383 (LOCKED): gate-active issue order is **evidence-weighted, not vision-weighted**.

### 3.0 The road to 0.95 (post-rebaseline ordering)

| Rank | Epic | Status | Why it moves the gate |
|---|---|---|---|
| **0** | **REBASELINE + RANK-SAFE CALIBRATION** — run `swebench_300x8_ship_gate_fsv` against real-instruments inference + real trained checkpoint | ✅ landed #531/#534 | Valid Panel A rank-safe measurement exists: `0.6916886568`. |
| **0.5** | **#535** mixed-cell rank lift from real labels | ✅ landed 2026-05-20 | First valid Panel A rank-lift measurement: `0.7937270999`, Brier `0.0936445221`, ECE `0.0338404700`. |
| 1 | **#537** real-input ablation revalidation | ✅ complete 2026-05-20 | #393 uncalibrated, #398 calibrated, and EK-012 `λ_inv` reruns are all not gate movers. Best #393 family: `0.2876047260`; best `λ_inv`: `0.6704055071`; all are below the #535 predecessor baseline and the #536 measurement of record. |
| 1.5 | **#536** repair remaining weak cells after #535 | ✅ complete 2026-05-20 | Selected hard-negative rank-lift weighting (`tau=20.0`, `fail_boost=1.5`); measurement now `0.7942190170`, Brier `0.0923729688`, ECE `0.0240316074`. Small lift only; not a gate close. |
| 2 | **#404** failure-mode hierarchy | ✅ closed + post-fix revalidated 2026-05-21 | Substrate REAL. Actual Fail coverage remains 114/114; post-fix predicted true-Fail coverage is 117/117. Aggregate predicted-Fail coverage is 137/150 because 13 false-positive Fail verdicts have no named consequence explanation; repair in #392 predictor policy/feature selection, not blanket skill expansion. |
| 3 | **#413** auto-label discovery | ✅ closed 2026-05-19 | 491-row accepted Level-0 registry — REAL (deterministic over real forward cache) |
| 4 | **#414** skill substrate | ✅ closed 2026-05-19 | 128 Level-2 skills, 36,773 chunk memberships, 10,579 reverse rows — REAL (derived from #413) |
| 5 | **#418** runtime hierarchical ability resolver | ✅ closed 2026-05-19 | Pure resolver logic — REAL |
| 6 | **#406 / #523** online mistake loop + MCP | ✅ closed 2026-05-20 | Machinery shipped; needs real predictor mistakes to validate `mistake_repeat_rate ≤ 0.05` convergence |
| 7 | **#547** post-#536 calibration-disjointness revalidation | ✅ complete 2026-05-20 | Proves #536 selected `tau=20.0, fail_boost=1.5` on calibration only; train/calibration/holdout row and feature-member overlaps are zero; #413 label-lineage hashes stable; target-side labels excluded. |
| 8 | **#548** post-#547 mixed-cell ranking diagnostic | ✅ complete 2026-05-20 | Proved `MIXED_CELL_CALIBRATION_UNDERPOWERED`: current surface has signal but legal calibration cannot select the holdout-working threshold. |
| 9 | **#549** calibration-stable mixed-cell signal fusion | ✅ complete 2026-05-20 | Static metadata and score fusion are useful but insufficient; typed blocker `MIXED_CELL_SIGNAL_FUSION_INSUFFICIENT`. |
| 10 | **#392 / #554 / #557 / #558 / #560 / #551** temporal constellation trajectory features + skill usefulness repair | OPEN p0, #551/#557/#560 complete | #551 proved the uncovered skill-state set is pass/context-only and not missing failure coverage; blanket pass/context expansion regresses. Latest #392 evidence-semantics hardening selects cause-bin calibration at correlation `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`, with cause-bin applied to nonzero calibration/holdout scoped rows. Final-bin calibration was tested and rejected because ECE worsened. Next: lower ECE below the #536 baseline without coverage expansion, wider evidence-key ladders, aggregate bins, or holdout tuning. |
| 11 | **#511** information-entropy auxiliary loss | OPEN p1 | Only gate-moving if re-justified against `0.7942190170` and shown to improve the true mixed cells under split-stable selection. |
| 12 | **#405** cross-panel triangulation Panel B | OPEN p0, blocked | Even hitting 0.95 on Panel A is not ship-eligible without Panel B. Defends 0.95 against encoder-matched Goodhart. |
| 13 | **#379** multi-file structural patch holdout | OPEN p1 | Compositional generalization |
| 14 | **#409** paper revision | OPEN p2 | Aligns PAPER §1 with binary doctrine + adds synthetic-vector remediation section. Not gate-blocking. |

### 3.1 Slot-preserving constellation architecture (#385 parent — substantially shipped)

The slot-preserving refactor is largely shipped and architecturally correct. Closed children: #386 (schema), #387 (late-fusion predictor), #388 (cross-embedder tensors), #389 (per-slot calibration + OOD), #390 (consensus/contradiction heads), #391 (slot attribution), #393 (ablation suite), #394 (anti-compensation), #395 (real-corpus smoke), #396 (training objective), #398 (calibrated rerun), #441 (trained checkpoint export), #452 (native active-embedder adapter).

Remaining open children: #392 (temporal trajectory features), #382 (frozen-panel dimensionality ablation).

**Current finding:** the slot-preserving architecture is correct and remains the required representation contract, but the pre-fix numerical conclusions are invalid because they measured synthetic-input panels. The real-input #393 uncalibrated, #398 calibrated, and EK-012 `λ_inv` reruns are now negative evidence, not promotion evidence; do not promote another constellation-wide feature family from these results.

### 3.2 Q4 head training — CLOSED (ambiguity boundary)

Per binary doctrine (`01_overview.md §1.6`), all Q4 surfaces are out of the prediction contract. Q4 questions do not have a free binary form against reality — they require arbitrary thresholds on continuous heuristics or ask taste judgments with no ground truth.

Closure status of all Q4 producer/calibration issues:

| Issue | Status |
|---|---|
| #266 perf, #267 accuracy, #268 cost, #269 reasoning, #270 non-trivial-on-Pass, #399 perf re-proof, #400 cost re-proof, #401 harvest expansion | `archived-display-only` |
| #127 confidence calibration, #377 trust gate | `archived-display-only` |
| #402 security/structural producers | `wontfix-ambiguity-boundary` |
| #118 per-head Q1-Q5 distillation | re-scoped to Q1/Q2/Q5 only; Q4 heads removed |

`CF_MEJEPA_Q4_*` CFs remain readable for forensic continuity; not trained, not consumed by predictor, not ship-gate-relevant. Weekly delta-zero recheck via `mejepa_weekly_eval_dashboard.q4Freeze`.

**What replaces Q4 in operator answers.** Operators who used to want "what else could go wrong" answers now get them from the **named failure mode** + **closest exemplar** attached to Fail verdicts. If Pass: operator answer is "Pass + Q1=Yes (claim landed in bytes) + closest exemplars all Pass + cross-panel Panel B confirms" — not a list of speculative ambiguous risks.

### 3.3 Phase G calibration sweep (open p1/p2)

All calibration items below are qualified by #385: their evidence must be per-slot/per-embedder constellation evidence, not flat panel aggregate.

| Issue | Task | Status |
|---|---|---|
| #122 | refined 66-cell pairwise MI matrix audit | open |
| #123 | per-cell TCT signal-saturation matrix | open |
| #124 | frozen instrument coverage audit per Python cell | open |
| #125 | Q1 claim-existence calibration at corpus scale | open |
| #126 | Q3 failure-mode calibration | open |
| #121 | Conformal RCA classifier | open |
| #152 | E_Trace gating | open |

**Gate for Phase G §3:** per-head Brier ≤ 0.05, ECE ≤ 0.05, conformal coverage 88-92%.

### 3.4 Continuous self-improvement + agent feedback loop (open p1)

| Issue | Task | Status |
|---|---|---|
| #154 | Active-learning consumer (multi-source queue) | open |
| #131 | Feedback cooldown (prevent reward thrashing) | open |
| #163 | Feedback retrain consumer | open |
| #182 | Feedback replay-weight FSV | open |

### 3.5 Adversarial defense + ship-gate closure (after calibration green)

| Issue | Task | Status |
|---|---|---|
| #178 | Refined cell taxonomy (Active/Exempt/Uncovered tri-state) | open |
| #120 | Catastrophic Python cell quarantine | open |
| #148 | Python leakage detector | open |
| #153 | Python oracle pass/fail audit | open |
| #612 | Split-stable weak-cell evidence expansion for active point metrics | open p0 |
| #150 | Python sample-efficiency / row-window power support | closed (#150 support_policy=blocked) |
| #165 | Python 0.95 confidence gate | open (blocked by #167) |
| #167 | Four-window Python burn-in | open (blocked by #612/#610) |
| #172 | Python acceptance bundle | open |
| #180 | MCP RealityPrediction FSV | open |
| #171 | Fail-closed release switch | open |
| #379 | Multi-file structural patch holdout | open p1 |
| #382 | Frozen-panel dimensionality ablation | open p2 |
| #375 | Consume witness provenance in `RealityPrediction.confidence` | open p2 |
| #378 | MCP tool-usage telemetry + dead-surface audit | open p2 |

### 3.6 Post-ship operation (engages after `≥ 0.95 × 4 windows`)

| Issue | Task |
|---|---|
| #277 | Post-ship-gate auto-trust policy |
| #173 | Post-green drift guardrails |
| #170 | Model-quality RCA dashboard |
| #168 | Live Python patch backfill |
| #164 | Checkpoint provenance |
| #162 | Eval-after-promotion hook |
| #166 | Python error taxonomy |
| #169 | Unseen Python holdout quarantine |

---

## 4. Operator-Facing Surfaces (above the binary primitive)

### 4.1 Pathway exploration surface (#416 / TASK-PY-G-119, OPEN p1)

L3 counterfactual rollouts for decision support — top-K (3-5) most-probable future trajectories per candidate patch as chains of Q1/Q2/Q5 binary leaves only. **Q4 strictly excluded** (`ambiguous_leaves_in_pathway == 0` FSV assertion).

Consumes: #413 Level-0 registry (closest-exemplar attachment), #404 named failure modes (Fail leaves), #406 mistake loop (credits across pathway leaves), #407/#547 calibration disjointness (Goodhart-safe surfacing), #417 skill/chunk attachment per leaf.

Precedes: #335 `mejepa_reverse_prediction` — operator picks pathway, system tells agent what patch-shape lands it.

Not gate-blocking; ships when gate-movers stabilize.

### 4.2 Bidirectional chunk↔skill linkage (#417 / TASK-PY-G-120, OPEN p1)

Read-only MCP surfaces shipped 2026-05-19/21: `mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`, `mejepa_chunk_as_star`, `mejepa_constellation_membership`, `mejepa_skill_impact`, `mejepa_skill_graph_inspect`, `mejepa_skill_conflict_graph`, and `mejepa_skill_browse`.

#557 extends the browse/audit surface with skill usefulness evidence: candidate kind, support/fail-pass distribution/lift, genericity, per-cell contribution count, live-safe labels vs target supervision labels, representative source sequences, lifecycle history, uncovered code-state keys, unknown ordered constellations, generic high-support rejections, and metric-regression rejections. It proves `stable_background_import_and_impl_scaffold` belongs in context/negative evidence, not failure-skill promotion, and exposes the blanket-expansion regression.

Remaining open: cross-repo skill transfer and pathway-leaf attachment.

Constraint: skills are deterministic groupings of accepted Level-0 labels. Read-only navigation over training substrate; no new prediction surface; binary-leaf constraint preserved at prediction boundaries.

---

## 5. Information-Physics Items (peer-reviewed framing)

Four issues filed against `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`. Formalize what the project already embodies but does not name:

| Issue | Status | Source principle |
|---|---|---|
| #511 / TASK-IP-001 — Information-entropy auxiliary loss | OPEN p1; **on the road to 0.95** | Vopson 2nd law of infodynamics (AIP Advances 12, 075310, 2022) |
| #512 / TASK-IP-002 — Self-dual ECC over 15-slot panel | OPEN p1; hygiene | Gates' self-dual ECC in supersymmetry (Doran et al. 2008) |
| #513 / TASK-IP-003 — Connectome rich-club analysis | ✅ CLOSED 2026-05-20 as **demoted negative finding** | FlyWire connectome (Nature 634, 2024-10-02) |
| #514 / TASK-IP-005 — Eon-style cold-load behavioural FSV | OPEN p1; safety net | Eon Systems fly demonstration (March 2026) |

#513 was tested and demoted: #526 fixed split reproducibility (`7/8` cells at Jaccard ≥ 0.8), but #527 proved the stable topology is not promotion-worthy — modularity above null `1/8`, full finite-edge coverage `0`, diagnostic correlation regressed by `-0.583`. Topology harness kept diagnostic-only.

---

## 6. Post-0.95 Creativity Engine (autonomous bounded growth)

**The architectural framing.** ME-JEPA is the *engineering* half (verifies binary claims against reality). The *creativity* half is **autonomous embedder synthesis** with two oracles gating every proposal:

1. **Novelty oracle** — cross-correlation test: candidate orthogonal to existing N? Reject if max correlation > τ_novelty (~0.7).
2. **Utility oracle** — ablation test: improves per-cell oracle correlation by ≥ δ_utility (~0.005)?

Only candidates passing BOTH are promoted. Rejected candidates write a `CF_MEJEPA_EMBEDDER_PROPOSAL_AUDIT` row with typed reasons.

**The non-obvious property.** Orthogonal informative axes ContextGraph admits is **bounded by reality** — by intrinsic dimensionality of the corpus. For code N* ≈ 50-500. The system **discovers its own N*** because the novelty oracle rejects every candidate once the corpus is fully resolved.

**Five issues (all post-0.95 — none are gate-movers):**

| Rank | Epic | Why | Priority |
|---|---|---|---|
| C-1 | **#517** Mistake-driven embedder demand signal | Closes loop between unresolvable mistakes and embedder synthesis | p1 |
| C-2 | **#516** Novelty + Utility oracle | Two-test gate converts unbounded growth into bounded automated growth | p2 (p1 immediately post-0.95) |
| C-3 | **#518** Embedder retirement audit + Pareto frontier | Vopson 2nd-law applied to embedder set; dominated embedders retire with grace period | p1 |
| C-4 | **#519** Domain-routed sparse panel | Universal ~30 slots + per-domain ~15 extension; 3-5× storage saving | p1 |
| C-5 | **#520** Episodic-to-semantic consolidation | Settled chunks → `(cluster_id, residual)`; 99% storage reduction in high-clustering regime | p2 |

**Sequencing:** ship C-1 first (demand rows). Then C-2 (validates proposals). Then C-3 (closes loop with pruning). Then C-4 (multi-language scaling). Then C-5 (steady-state).

**Composition.** Together with v2-stretch creativity tasks (#329 AlphaEvolve outer loop, #335 reverse-prediction, #354-357 EK v2 series), these form a **self-resolving observer**: system grows panel until corpus is resolved, prunes dominated embedders, specializes per domain, compresses settled clusters. Bounded by reality, not ambition. Storage bounded; search precision maximal.

**Why none are gate-movers for 0.95.** The current `0.7942190170 → 0.95` gap is a function of remaining cell-specific ranking weakness and unproven cross-panel generalization, not embedder count or storage architecture. The targeted cell repair + real-input ablation reruns + cross-panel + calibration + entropy loss + mistake loop closes the gap. The creativity engine takes over after 0.95 is hit.

**NOT in scope.** No new ambiguous-head prediction surfaces. No consciousness / sentience / phenomenology framing. No simulation-hacking / reality-bending capability claims. The creativity engine is bounded creativity within a falsifiable substrate.

---

## 6.9. Neuroscience-Inspired Reserve (filed 2026-05-20, DO NOT START until current queue concludes)

**Status:** all eight issues filed at `priority:p3`, `status:needs-triage`, with explicit `⚠ DO NOT START — Neuroscience-reserve contingency` markers in their bodies.

**Activation rule:** these are *contingency plans* for if the current Phase G queue + creativity engine fail to close the remaining ship-gate gap. **They are NOT to be started until:**

1. The current queue concludes (#529 rebaseline → #535 rank lift → #536 weak-cell repair → #547 calibration disjointness → #548 typed blocker → #549 static-signal blocker → #392 temporal features → #511 entropy loss → #405 Panel B → #379 multi-file holdout).
2. A post-queue gap analysis has been run and concludes the remaining gap is not closable with the current architecture + corpus + label substrate.
3. The operator explicitly re-prioritizes any of these (or all) from `p3` based on what gap analysis identifies as needed.

If the gate closes via the current queue, these issues stay in `p3` reserve indefinitely. If the gate does not close, they become the candidate lever set for the next architectural sweep.

### Tier 1 — high-impact, doctrine-compatible, addresses the current gap directly

| Issue | Title | Neuroscience source |
|---|---|---|
| **#538 / TASK-PY-NEURO-001** | Predictive-coding head: top-down per-slot prediction + bottom-up residual | Rao & Ballard 1999 (*Nature Neuroscience*); Friston free-energy principle |
| **#539 / TASK-PY-NEURO-002** | Hippocampal schema-prioritized replay consolidation (incl. reverse replay for pathway credit assignment) | Mattar & Daw 2018 (*Nature Neuroscience*); Liu et al. 2019 (*Cell*) |
| **#540 / TASK-PY-NEURO-003** | Hierarchical predictive coding tied to the #404 / #413 / #414 label hierarchy | Felleman & Van Essen 1991 (*Cerebral Cortex*); Bastos et al. 2012 (*Neuron*) |
| **#541 / TASK-PY-NEURO-004** | Mixed-selectivity cross-slot binding layer (replaces #390's linear pairwise statistics) | Rigotti et al. 2013 (*Nature*); Fusi et al. 2016 (*Current Opinion in Neurobiology*) |

### Tier 2 — promising but more speculative; design-influence on existing epics

| Issue | Title | Composes with |
|---|---|---|
| **#542 / TASK-PY-NEURO-005** | Population-dynamics low-dim trajectories for #416 pathway surface (Shenoy/Churchland/Cunningham) | #416 pathway exploration; #392 temporal trajectories |
| **#543 / TASK-PY-NEURO-006** | Sparse distributed representations per slot (Hawkins/Numenta; Olshausen & Field 1996) | #519 domain-routed sparse panel; #389 OOD |
| **#544 / TASK-PY-NEURO-007** | Synchrony-based temporal binding for #392 trajectory features (Singer & Engel; Fries 2005) | #392 temporal features; #390 constellation intelligence |
| **#545 / TASK-PY-NEURO-008** | Idle-time generative rollouts for proactive active learning (default-mode network analog) | #416, #517, #406, #520 |

### Doctrinal compatibility check (all eight verified before filing)

- ✅ Slot identity invariant preserved (no cross-embedder dim-wise compare anywhere)
- ✅ Anti-compensation invariant (#394) preserved (strong slots cannot silence weak slots)
- ✅ Binary contract preserved (final verdict still Q2 Pass/Fail; Q3 explanation-only; Q4 frozen)
- ✅ Ambiguity boundary preserved (no ambiguous heads introduced)
- ✅ No inner-LLM assumption introduced
- ✅ No synthetic-input shortcut (all extensions ride the real instrument substrate)

### Honest risk note

Neuroscience-inspired architectures have a mixed history in ML. Predictive coding (Tier 1A/1C) has the strongest empirical track record; mixed selectivity (Tier 1D) is the most theoretically grounded; replay consolidation (Tier 1B) is the most directly tied to an existing problem (#406 convergence). The Tier-2 items are more speculative. None is a guarantee — they are a documented option set, not a commitment.

---

## 7. EK Series (Explanatory Knowledge programme #321) — open tail

| Issue | Status |
|---|---|
| #325 Mechanism-conjecture surface | open p2 |
| #326 `mejepa_counterfactual_under_invariant` MCP tool | open p2 |
| #328 Held-out failure-mode class falsification | open p2 |
| #329 AlphaEvolve-pattern outer loop using `RealityPrediction` as fitness | open p2 |
| #330 Ontology-growth audit + telemetry | open p2 |
| #335 `mejepa_reverse_prediction` — desired-reality → patch-shape constraints | open p2 |
| #336 Operator-contribution surface | in-progress |
| #339 Chunk foundationality | in-progress |
| #340 Library-level foundationality | in-progress |
| #349 Embedder-foundationality | in-progress |
| #352 Credibility-weighted operator override | in-progress |
| #354-#357 v2 stretch (NAS embedder / cross-library / self-rewriting / status reallocation) | p3 |

---

## 8. Aiwonder Migration (open tail)

| Issue | Task | Status |
|---|---|---|
| #310 | META Issue refactor | in-progress p0 |
| #372 | ContextGraph roots must be mounted on aiwonder ZFS pools (paths-policy enforcement) | closed 2026-05-22; `/zfs/archive/contextgraph` -> `archive/contextgraph` ZFS and `/zfs/hot/contextgraph` -> `hotpool/contextgraph` ZFS |
| #368 | Host capability utilization audit + scheduler profile | open p1 |
| #380 | Off-host backup for `/zfs/archive` proof history | open p1 |
| #384 | CUDA/candle rollback and reproducibility lane | open p2 |
| #295 | PERF — resume launch scans full aiwonder FSV root | in-progress |
| #299-#305 | AIWONDER-003..010 install / transfer / build / launch / calibration sub-tasks | in-progress p1 |
| #306 | SWE-bench Lite Python Docker image plan (future / Q3) | p3 |
| #312 | hot_root on ext4 throughput bottleneck — move to /hotpool ZFS | satisfied by #372 root migration; close after confirming no separate scheduler-profile work remains |
| #442 | Track aiwonder librocksdb-sys cstdint native build requirement | open p1 |

---

## 9. Active `type:context` Pins (must read at session start)

| # | Title | Why pinned |
|---|---|---|
| #310 | META Issue refactor | Tracks Phase G execution-order map; the live source of "what to do NOW" |
| #227 | Python ME-JEPA data explosion runtime policy | Defines runtime constraints during forward cache jobs |
| #296 | aiwonder migration EPIC (closed but still load-bearing context) | Establishes the migration plan |
| #383 / TASK-PY-G-084 | Gate-active issue order must be evidence-weighted, not vision-weighted (p0) | When choosing next task, weight by measured impact on the current post-fix gate gap and active/abstain blockers, not the invalid pre-fix `0.083` gap |
| #397 / [PY-G] CONTEXT | ME-JEPA consequence-prediction truth-system contract (p0) | "Predicting reality" = predicting consequences of code being live; MCP presents calibrated consequence evidence; hidden human intent is not a prediction target |

---

## 10. Bugs (open, non-blocking)

| # | Title | Status |
|---|---|---|
| #295 | Resume launch scans full aiwonder FSV root (perf) | in-progress |
| #307 | promote_tree_verified asserts on second-subrun shard promotion | in-progress |
| #320 | phase_g_project_merkle_cache_fsv clippy — `ProjectMerkleCache` Debug missing | needs-triage p1 |
| #338 | Capture agent response bodies in runtime/agent-history for Q1 claim FSV | needs-triage p2 |
| #429 | DynamicJEPA Phase 8 FSV intermittently aborts in native record-surprise path | needs-triage p2 |
| #433 | P1 ability resolver evidence integrity and replay key completeness | needs-triage p1 |
| #436 | context-graph-mejepa example clippy gate has pre-existing warnings | needs-triage p2 |
| #453 | Residual Sherlock/investigation findings after first cleanup pass | needs-triage p1 |
| #522 | verification_state test: predicted SubtleFlip failure no longer escalates | needs-triage p1 |

---

## 11. Deferred (not blocking ship gate)

| # | Why deferred |
|---|---|
| #149 | SWE-bench Verified subset — separate dataset; ship gate uses Lite 300×8 |
| #354-#357 | TASK-EK-018 v2 stretch (NAS from scratch / cross-library / self-rewriting / status reallocation) |
| Non-Python ship-gate cells (Rust/JS/TS/Go/Java/C/Cpp/CSharp/Ruby/Php) | Active execution is Python-first; non-Python deferred until Python truth system is green |

---

## 12. How to Pick Up Work (per AGENTS.md)

1. Run the six-issue queries from `AGENTS.md`:
   ```bash
   gh issue list --state open --label "type:context" --json number,title,body
   gh issue list --state open --label "status:in-progress" --json number,title,assignees
   gh issue list --state open --label "status:blocked" --json number,title
   gh issue list --state open --label "source:agent" --search "no:assignee" --json number,title,labels,updatedAt
   gh issue list --state closed --label "type:decision" --limit 20
   gh issue list --state closed --label "type:discovery,type:pattern" --search "<kw>" --limit 20
   ```
2. Read `README.md` for the current doc map, then `01_overview.md §1-2`, `07_results_and_benchmarks.md §1`, and this roadmap.
3. Find the highest-priority open issue you can usefully unblock that does NOT have an in-progress assignee.
4. CLAIM (assign self + flip `status:needs-triage` → `status:in-progress` + post CLAIM comment with files-you-will-touch + ETA).
5. Audit-the-task before writing code (`06_operations_and_fsv.md §1.6`).
6. Work; comment at every milestone.
7. Produce FSV evidence per `06_operations_and_fsv.md §1.5`.
8. RESOLVED / PAUSE / BLOCKED issue closeout.
9. Code-simplifier pass.
10. File any observed defect / pattern / discovery as a fresh issue before ending the turn.

---

## 13. What This Doc Does NOT Cover

- Day-to-day operator decisions outside the issue tracker — those happen in conversation with the operator and become `type:decision` issues when locked in.
- Hardware procurement / aiwonder admin — `phase:migration` issues track that subset.
- Time estimates — Phase G has no public ETA; per-cell convergence ETA tracker emits internal projections used by the heal scheduler.
