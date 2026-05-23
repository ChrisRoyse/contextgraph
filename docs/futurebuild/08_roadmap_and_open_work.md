# 08 — Roadmap and Open Work

**Source:** `gh issue list --state open`, recent `type:decision` closes, git log, GitHub issue comments.

---

## 0. Doctrine Reminder (binary contract)

Prediction contract is **three binary surfaces against ground truth** (Q1 bytes/AST, Q2 Docker oracle, Q5 shift log), **one explanation surface** (Q3 named failure mode + closest exemplar), **Q4 removed entirely** (ambiguous heads not predictable). See `01 §1.1`, `05 §0`.

---

## 0.4 P0 GATE: 2026-05-22 manual full-system FSV + hardening (#633)

**Status (2026-05-22):** 5-Sherlock parallel forensic audit (#629 closeout) over all 22 active workspace crates surfaced silent-success/fake-FSV/doctrine-violation defects across the entire product surface. Operator opened **#633 / [P0] Manual full-system FSV and hardening gate before new work** to stop all feature/gate-moving work until every product surface has been **manually** verified at real source-of-truth bytes/rows/files/services on aiwonder. Return values, green scripts, previously-passing FSV JSON are supporting evidence only — not the verdict.

**Binding contract** (must be followed for every child issue):

1. Define the Source of Truth — exact file path, RocksDB CF/key, systemd unit, Docker container, ZFS dataset, model cache path, metric/log path, or external oracle artifact.
2. Capture BEFORE — manually read the SoT and paste observed state into issue comment.
3. Execute concrete trigger with synthetic known input where possible.
4. Capture AFTER — independently re-read SoT and paste observed delta.
5. Exercise happy path **plus at least 3 edge classes**.
6. On mismatch: stop, root-cause, fix or file linked blocker, repeat manual FSV after fix.
7. No child closes from return code or generated JSON alone.

**17 child sub-tasks (all `priority:p0`, `status:needs-triage`):**

| Issue | Domain |
|---|---|
| #634 | aiwonder host, ZFS roots, GPU, Docker, MPS, env |
| #635 | aiwonder model cache + no-download embedder readiness |
| #636 | 300×8 corpus, AST chunks, forward cache, labels, skills |
| #637 | SWE-bench Docker oracle + weak-cell outcome truth |
| #638 | ME-JEPA `RealityPrediction` Q1/Q2/Q3/Q5 product path |
| #639 | MCP product tools, retired surfaces, JSON-RPC state |
| #640 | slot-preserving panels + frozen-instrument execution |
| #642 | RocksDB CF registry, data integrity, retention, restore |
| #643 | trainer, mistake loop, replay, checkpoints, Fisher/EWC |
| #644 | CLI, hooks, shift log, path discipline, no mounted-drive |
| #645 | build, tests, clippy, CI, secrets, SBOM/provenance |
| #646 | observability, evidence retention, backup, DR |
| #647 | performance, scalability, capacity, cost |
| #648 | security hardening for code + aiwonder ops |
| #649 | documentation + GitHub issue ledger truth |
| #650 | benchmark + gate-metric honesty audit |
| #651 | operational practice + RCA discipline hardening |

**Done When:** all linked P0 children have either RESOLVED with manual SoT before/after + happy + 3 edge cases, or BLOCKED with specific blocker issue and no hidden assumption. Only then does normal #615/#616 gate movement resume.

**Operational implication (#655, in-progress p0):** Rust `*_fsv.rs` examples and tests are being **retired** in favour of manual operator-driven SoT verification. New gate evidence is captured as issue comments with direct bytes-on-disk readback. See `06 §1.9`.

**Doctrine-violation P0s the audit surfaced (all open, all gate-blocking):**

| Issue | Title | Source |
|---|---|---|
| #618 / TASK-PY-G-136 | Aggregate OOD score uses flat 5,120-byte panel buffer (doctrine §1.5ter) | mejepa |
| #619 / TASK-PY-G-137 | `forward_inverse_inner` uses `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter) | mejepa + mejepa-train |
| #620 | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook never invoked from training | mejepa + mejepa-train |
| #621 / TASK-PY-G-138 | Hardcoded `gtau_pass_rate: 1.0` in HoldoutReport published to MCP weekly dashboard | mejepa-train + mcp |
| #622 | Trainer dead-code cleanup: distill + lora_wiring + aux | mejepa-train |
| #623 | `cold_start_conformal_policy` emits `alpha=0.0` (dead public API) | mejepa |
| #624 | `verdict_assembly` inline `CONTRADICTION_CALIBRATION_INVALID` not declared constant | mejepa |
| #625 | `DeterministicPredictor` publicly re-exported (synthetic-vector landmine post-#422) | mejepa |
| #626 / TASK-PY-G-139 | Hygiene retention default policy missing ~90 CFs | mejepa-hygiene |
| #627 | shift-subscriber runner integration test fails on main | mejepa-shift-subscriber |
| #628 | Doc/code drift: `MEJEPA_TCT_DIM_MISMATCH` (docs) vs `MEJEPA_INFER_DIM_MISMATCH` (emitted) | docs |
| #704/#714 | Retire `e2/e3/e4` for static AST forward-cache runs (closed evidence) | mejepa-embedders |
| #709/#715 | Add inter-embedder distinctness gate and rebuild v2 9-slot static AST forward cache | mejepa-embedders |
| #671 | Stale SWE-bench harness defaults point at missing aiwonder archive venv | scripts |
| #672 | Geometry scalar feature lineage + leakage gate for `head_projection` promotion | mejepa + mejepa-train |
| #673 | Label-quality audit ranks embedder signal vs noise | mejepa-train |
| #674 | `run-remote` wrapper can hang after remote command completes | scripts |

---

## 0.5 Historical P0 BLOCKER (superseded by #633): #392 trajectory ECE repair

Between Phase G ramp and 2026-05-19 22:45 UTC, inference path used SHA256-seeded synthetic vectors. Six P0 fixes (#422 / #424 / #426 / #437 / #438 / #441 / #452, closed 2026-05-19/20) replaced broken pipeline. **Previous `0.866667` ship-gate measurement is INVALID.** See `01 §1.7`, `07 §0`.

**Current real-input state (#536/#547/#548/#549/#392/#554/#557/#558/#560):**

Latest ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`: raw `0.4843192995`, rank-safe `0.6916886568`, rank-lift `0.7942190170`. Carries first-class #392 active/abstain diagnostics. Earlier row-trace `run-20260520T185945Z` preserves #536 metrics + raw/rank-safe/rank-lift row traces for #549/#392.

Calibration/rank-lift fitted only on 240-row calibration split; constants restricted to structurally degenerate cells; rank-reversing transforms require mixed-class support ≥ 8 per class + raw Pearson ≤ -0.25; rank-lift excludes target-side oracle/docker/test labels. #536 selected hard-negative weighting `tau=20.0, fail_boost=1.5` — small lift.

**#547 → #548 → #549 chain (all closed):** #547 proved #536 policy split-disjoint. #548 typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED`: holdout-selected thresholds can lift metrics, but legal calibration leaves true mixed cells saturated. #549 proved static repo/path metadata + calibration-selected score fusion are useful but insufficient.

**#392 trajectory chain (all ECE-blocked):**

| Slice | Best | Status |
|---|---|---|
| Aggregate trajectories | `0.8008320332` | Initial lift |
| File-path + skill/ability | `0.8422425985` | — |
| Out-of-fold probability bins | — | Regressed ECE |
| Hardened support-aware mean-bias | corr `0.8497869968`, Brier `0.0697008073`, ECE `0.0299093761`, accuracy `0.9083333611` | Exact #536 row-trace reproduction, no mixed-cell threshold degeneracy |

**#554 / #558 / #560 / #557 / #551 (all closed):**

- **#554:** targeted usefulness-weighted ordered skills increase consequence signal but ECE-sensitive.
- **#558:** rematerialized full v2 label corpus (`399777` chunks, `398215` episodes, `157` skills, `9489` memberships). `usefulness_weighted_combined`: corr `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`, accuracy `0.9083333611`, pass recall `0.9166666865`, fail recall `0.9000000`.
- **#560:** split-legal ECE-focused calibration diagnostic selected same model; support-shrink best `0.0396139733` — non-promotable.
- **#557:** usefulness audit surface (source-byte sequences, lifecycle/demotion, gap views, generic high-support rejections, blanket-expansion regression evidence) queryable without new prediction head.
- **#551:** uncovered `1469/3000` code states are pass/context-only; all `1139/1139` failure-evidence rows skill-covered. Coverage/pass-context expansion regressed #392 — not the repair.

ECE remains above #536 baseline `0.0240316074`. Next lift must repair ECE through per-cell/per-constellation consequence evidence, cross-window validation, or true-mixed feature specialization, then pass Panel B / anti-Goodhart validation. Do not spend more time on: synthetic-input rebaselines, static score blends, aggregate-only trajectory, sparse bin calibration, affine/rank-lift blending, logit calibration, ECE-overweighted selection, support-only shrinkage, blanket skill expansion, holdout threshold tunes.

**2026-05-21 #551/#392 correction:** corrected #392 rerun at `fsv/task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` (sha256 `9ed8830f184e0aca07a1ee47897b31b342dbdaf2db0bacc28d0a08981dfbc29c`) — mean-bias at corr `0.8770877719`, Brier `0.0581485741`, ECE `0.0475411229`. Evidence-semantics hardening at `fsv/task-py-g-392-evidence-semantics-hardening-fsv-release/run-20260521T074421Z/` proves standardized cause-residual features, selecting cause-bin: corr `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`. Final-bin diagnostic at `fsv/task-py-g-392-final-bin-guarded-fsv/run-20260521T090040Z/` improved corr/Brier but worsened ECE — selector-forbidden. Cause-bin remains legal best.

Latest ship-gate FSV wires #392 active/abstain as first-class gate accounting: `205/240` active rows, active coverage `0.8541666865`, corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`. Full active/abstain gate remains false: `delete_test_call`, `off_by_one`, `subtle_flip`, rolling-window, statistical-power checks fail. #392 stays P0 only through targeted active-cell/window/power repair without holdout tuning.

**2026-05-22 #610/#150/#611/#612/#613/#614/#615 data-substrate decisions:** #372 aiwonder root readiness + comparable #405/#521 Panel B parity are green; burn-in still not admissible.

| Ref | Result |
|---|---|
| **#610** | `burn_in_start_allowed=false`, `decision_code=TASK_PY_G_133_BURN_IN_POWER_NOT_ADMISSIBLE` |
| **#150** | `support_policy=blocked`: row support / longer windows alone cannot start #167; active point correlations below target for `delete_test_call`, `off_by_one`, `subtle_flip` |
| **#611** | `repair_promoted=false`, `TASK_PY_G_134_NO_EXISTING_LEGAL_WEAK_CELL_REPAIR` |
| **#612** | Raw AST `399777/399777` rows had legacy `/mnt/d`; active v2 labels scrubbed (`0` `/mnt/d`); forward cache real but `4665772/4797324` chunk/embedder pairs; `delete_test_call` only `3/300` oracle failures, zero calibration |
| **#613** | `all_passed=true`: active AST rows now `0` `/mnt/d`/`/mnt/c` hits; legacy retained only as `legacy_*` provenance |
| **#614** | `all_passed=true`, `legal_missingness_mask_promoted=true`, `slot_completion_promoted=false`. `131552` missing chunk/embedder pairs all quarantined (`0` unaccounted). Mask: 399,777 rows |
| **#615** | `all_passed=true`, `regeneration_complete=false`. `production_plain_assert_deleted` has `0` calibration failures. Plan: 1,131 deterministic candidates for Docker oracle execution |

**Historical 2026-05-22 remaining revalidation queue (superseded by §3.−1):**

1. **#615:** run deterministic `delete_test_call` regeneration oracle batch, then rebuild AST → #614 mask → labels → #392 → #611 from new outcomes.
2. **#616:** rerun + classify weak-cell oracle timeout/error rows for `off_by_one`, `subtle_flip`.
3. Rerun #392, then #611/#150/#610, after #615-#616 data repairs + all reruns join #614 completeness mask.
4. Keep #167 blocked until #610 returns `burn_in_start_allowed=true`; then start four consecutive windows on aiwonder ZFS roots.
5. ✅ #129 OOD serving gate, #130 Gτ guard audit, #404 holdout coverage, #529 synthetic-vector rebaseline, #372 aiwonder root readiness, comparable #405/#521 Panel B parity, #610 burn-in decision, #150 row/window power decision, #611 no-existing-repair falsification, #612 data-substrate audit, #613 source-lineage canonicalization, #614 forward-cache completeness mask are green/retained.

#529 is closed context. #536, #537, #547, #548, #549, #554, #557, #558, #560, #551 are closed evidence; #392 is active with 13 non-promotable/negative-control slices. **All issues in §3 below assume the real-input rebaseline, #536 rank-lift, #547 disjointness, #548 typed blocker, #549 static-signal blocker, #392 trajectory-ECE blocker, #554 skill-usefulness ECE blocker, #557 usefulness audit, #558 rematerialized-skill ECE blocker, #560 ECE-repair blocker, #551 justified-coverage-gap negative control exist unless explicitly noted.**

---

## 1. Current Ship Gate

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per mathematically-valid mixed (mutation_category × language) cell
  with all-pass/all-fail cells using #532 degenerate-cell replacement metric
  on 300×8 Python SWE-bench Lite corpus
  using Q2 Docker-oracle agreement as binary verdict
  AND cross-panel oracle correlation ≥ 0.95 on Panel B
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Last holdout measurement:** binary rank-lift `0.7942190170` from real code-state constellation labels (`0.6916886568` rank-safe, `0.4843192995` raw) in `fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`. Same FSV carries #392 active/abstain diagnostics: active corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978` on `205/240` rows — full active/abstain gate remains false. Previous `0.866667` (legacy pre-migration evidence) measured against synthetic-vector inference path and is invalidated.

**Honest gap to ship:** `0.4657` against raw Pearson. The old `0.1557809830` gap was against rank-lift, now diagnostic-only per #695, plus four-window stability + Panel B cross-checks.

**Degenerate-cell rule (#532).** `compile_error::python`, `wrong_file::python`, any future single-class cell cannot honestly use Pearson. FSV must emit cell's variance class, replacement gate metric (`all_fail_recall_confidence_brier_ece` or `all_pass_recall_confidence_brier_ece`), statistical-power diagnostics instead of bare `correlation unavailable` blocker.

**P0 product contract (#397):** ME-JEPA is the binary reality predictor. Returns `Pass | Fail` per candidate change against Docker oracle (Q2), named-failure-mode + closest-exemplar explanation (Q3) on Fail. Q1 (claim-exists) + Q5 (predicted-shift-event) are additional binary predictions. Q4 ambiguous surfaces out of scope.

Once met → human review of AI-generated Python patches is statistically unjustified. Auto-trust policy gates engage (#277).

---

## 2. Open Issue Snapshot (live tracker is authoritative)

As of 2026-05-22: **137 open** issues across workspace. Use queries in §12 for current state.

By phase:

| Label | Open count |
|---|---|
| `phase:g` | ~120 (critical path) |
| `phase:migration` | ~5 (#372 closed; tail items remain) |
| `phase:f-followup` | 1 |
| `phase:f` / `phase:h` | 0 (closed) |

By priority (excerpt):

| Priority | Open epic examples |
|---|---|
| `priority:p0` | **#633** manual full-system FSV gate (parent), #634-#651 sub-tasks, **#655** retire Rust FSV (in-progress), **#656** mejepa-heal-drill conformal-overcoverage (in-progress), **#657** TASK-PY-G-136 geometry-label bakeoff (in-progress), **#615** delete-test-call regeneration (in-progress), #616 weak-cell oracle reruns, #618/#619/#621 doctrine-violation P0s, #661/#662 geometry-label promotion (blocked behind #657), **#709/#715** v2 static-cache distinctness/rebuild, #671/#672/#673/#674 audit P0s, **#397** consequence-prediction contract (context pin), **#383** evidence-weighted ordering (context pin), #167 burn-in (blocked by #633), #165 confidence gate (blocked by #167), #310 META context |
| `priority:p1` | #630 long-source/OOM forward-cache quarantine recovery, #626 hygiene retention 90 CFs missing, #664 geometry region dictionary (blocked behind #657), #380 off-host backup, #379 multi-file holdout, #416 pathway, #417 skill linkage remaining, #511 entropy loss, #433 ability resolver replay completeness, #442 librocksdb-sys native build |
| `priority:p2` | EK series tail, post-ship policies, #382 frozen-panel dimensionality ablation, #378 tool-usage telemetry, #622 trainer dead-code cleanup, #623/#624 minor doctrine string drift, #665 new prediction heads over geometry labels |
| `priority:p3` | Deferred (SWE-bench Verified subset, v2 stretch, future Docker image plan), 8× neuroscience-reserve issues (#538-#545) |

---

## 3. Critical Path (evidence-weighted ordering)

Per pinned context #383 (LOCKED) + #712 (2026-05-23): gate-active issue order is **evidence-weighted, not vision-weighted**.

### 3.−1 Active critical path (2026-05-23 Sherlock-validated, supersedes prior §3.0)

3-Sherlock parallel audit decomposed the `0.7942190170` measurement into three structural defects (trainer/checkpoint lineage gap #683 + invalid legacy forward-cache slots + Goodhart-magnifier metric #695). Honest gap against raw Pearson = `0.4657`. Direct aiwonder readback shows the latest FSV loaded `/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json` with `optimizer_steps=64` and `weight_delta_observed=true`; #683 remains rank 1 because that path is out-of-band from the public `Trainer` contract and compiler construction must fail closed without an explicit manifest. #704/#714 retired `e2/e3/e4` for static AST; #709/#715 now own distinctness-gated v2 static-cache rebuild. New ordering:

| Rank | Issue | Action | Expected impact | Status |
|---|---|---|---|---|
| 1 | **#683** | Unify real training/checkpoint export with public trainer contract; require production `trained_checkpoint_manifest_path` at `mejepa/src/cli.rs:307-314` fail-closed | Largest; current checkpoint evidence is out-of-band from the public trainer | open p0 |
| 2 | **#709 + #715** | Add/promote inter-embedder distinctness gate, then rebuild a v2 static AST cache with active slots `e1,e6,e7,e8,e9,e10,e12,e13,e14`; `e2/e3/e4` remain retired for static AST by #704/#714 | Replaces invalid v1 cache with a clean 9-slot substrate; proves post-`ff4d4c03` `e8` is not an `e1` alias | open p0 |
| 3 | **#615** | delete_test_call Docker oracle batch over 1,131 deterministic candidates → rebuild AST → #614 mask → labels → #392 → #611 | Closes `0.69 → ≥0.85` weak-cell ceiling | in-progress p0 |
| 4 | **#618 + #619** | Per-slot OOD aggregation at `compiler.rs:502`; slot-preserving panel projections at `predictor/mod.rs:136-147` | Doctrine §1.5ter repair; removes anti-compensation cheat path | open p0 |
| 5 | **#695** | Stop using rank-lift as gate metric; use raw Pearson + isotonic/Platt | Reveals honest gap; ECE-safe | open p1 |
| 6 | **#685/#687/#688/#690/#684/#689/#699** | Eliminate hardcoded passing telemetry and pseudo-cert confidence: `forward_logits` zeros; `detach_loras` no-op; hardcoded `EpochSummary`; `retrain_triggered:true`; `per_slot_ood_reasons` copied from guard violations; `ExactPanelKeyIsolation` LUT inert; `calibrated_confidence_multiplier` from pseudo-certs | Prevents Goodhart on future mistake-loop wiring | open p1 |
| 7 | **#633 + #655 + #656 + #657 and children** | Manual full-system FSV/hardening gate, auto-FSV retirement, heal-drill truthfulness, geometry-label bakeoff | Evidence integrity and substrate prerequisites | mixed |
| Done | **#704/#714/#670/#691/#707** | Static temporal retirement, invalid-space policy, hook fail-visible behavior, strict OOD fail-closed behavior | Closed evidence; retained as prerequisites, not active blockers | closed |
| 8 | **#392 / #511 / #405 / #379** | Trajectory ECE / entropy aux loss / Panel B / multi-file holdout | **Blocked behind active ranks 1-7** | blocked |

**#633 manual full-system FSV gate** stays active as parallel work; children #634-#651 still required. **#655** Rust `*_fsv.rs` retirement → manual SoT readback (`06 §1.9`).

### 3.0 Pre-2026-05-23 critical path (historical — superseded)

The ordering below was the working critical path through 2026-05-22, prior to the 3-Sherlock audit that exposed the trainer-not-training defect. Retained for proof-of-work continuity. Items 0 through 9 below describe the substrate work that **survives the audit** and remains valid; items 10 onward (#615/#616/#392/#511/#405/#379) are now rank 3-9 in §3.−1 above.

| Rank | Epic | Status | Note |
|---|---|---|---|
| **0** | **REBASELINE + RANK-SAFE CALIBRATION** | ✅ landed #531/#534 | Valid raw `0.4843192995`; rank-safe `0.6916886568` |
| **0.5** | **#535** mixed-cell rank lift from real labels | ✅ landed 2026-05-20 | Per #695, rank-lift transform is a Goodhart magnifier — measures exemplar similarity, not predictor |
| 1 | **#537** real-input ablation revalidation | ✅ complete 2026-05-20 | #393/#398/EK-012 reruns measured untrained-predictor head — results invalidated by #683 |
| 1.5 | **#536** repair remaining weak cells after #535 | ✅ complete 2026-05-20 | Same caveat as #535 — rank-lift Goodhart (#695) |
| 2-9 | **#404 / #413 / #414 / #418 / #406+#523 / #547 / #548 / #549** | ✅ closed | Structural substrate work — all survives 2026-05-23 audit |
| 10+ | #615 / #616 / #392 / #511 / #405 / #379 | see §3.−1 above | |

### 3.1 Slot-preserving constellation architecture (#385 parent — substantially shipped)

Closed children: #386 (schema), #387 (late-fusion predictor), #388 (cross-embedder tensors), #389 (per-slot calibration + OOD), #390 (consensus/contradiction heads), #391 (slot attribution), #393 (ablation suite), #394 (anti-compensation), #395 (real-corpus smoke), #396 (training objective), #398 (calibrated rerun), #441 (trained checkpoint export), #452 (native active-embedder adapter).

Remaining open: #392 (temporal trajectory features), #382 (frozen-panel dimensionality ablation).

**Current finding:** slot-preserving architecture is correct and remains required representation contract, but pre-fix numerical conclusions are invalid because they measured synthetic-input panels. Real-input #393/#398/EK-012 reruns are now negative evidence; do not promote another constellation-wide feature family from these results.

### 3.2 Q4 head training — CLOSED (ambiguity boundary)

Per binary doctrine (`01 §1.6`), all Q4 surfaces out of prediction contract. Q4 questions do not have free binary form against reality.

Closure status:

| Issue | Status |
|---|---|
| #266 perf, #267 accuracy, #268 cost, #269 reasoning, #270 non-trivial-on-Pass, #399/#400/#401 | `archived-display-only` |
| #127 confidence calibration, #377 trust gate | `archived-display-only` |
| #402 security/structural producers | `wontfix-ambiguity-boundary` |
| #118 per-head Q1-Q5 distillation | re-scoped to Q1/Q2/Q5 only; Q4 heads removed |

`CF_MEJEPA_Q4_*` CFs remain readable for forensic continuity; not trained, not consumed, not ship-gate-relevant. Weekly delta-zero recheck via `mejepa_weekly_eval_dashboard.q4Freeze`.

**What replaces Q4 in operator answers.** Operators who used to want "what else could go wrong" answers get them from **named failure mode** + **closest exemplar** attached to Fail verdicts. Pass: "Pass + Q1=Yes (claim landed in bytes) + closest exemplars all Pass + cross-panel Panel B confirms" — not speculative ambiguous risks.

### 3.3 Phase G calibration sweep (open p1/p2)

All items below qualified by #385: evidence must be per-slot/per-embedder constellation, not flat panel aggregate.

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

All blocked behind #633 hardening gate.

| Issue | Task | Status |
|---|---|---|
| #120 | Catastrophic Python cell quarantine | blocked p3 |
| #148 | Python leakage detector | blocked p3 |
| #153 | Python oracle pass/fail audit | blocked p3 |
| #612 | Split-stable weak-cell evidence expansion | closed (data-substrate audit chain) |
| #150 | Python sample-efficiency / row-window power | closed (support_policy=blocked) |
| #165 | Python 0.95 confidence gate | blocked behind #167 |
| #167 | Four-window Python burn-in | blocked behind #633 + #610 → reopens if #610 returns `burn_in_start_allowed=true` |
| #172 | Python acceptance bundle | blocked p3 |
| #180 | MCP `RealityPrediction` FSV | blocked p3 |
| #171 | Fail-closed release switch | blocked p3 |
| #379 | Multi-file structural patch holdout | blocked p3 |
| #382 | Frozen-panel dimensionality ablation | blocked p3 |
| #375 | Consume witness provenance in `RealityPrediction.confidence` | blocked p3 |
| #378 | MCP tool-usage telemetry + dead-surface audit | blocked p3 |

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

Read-only MCP surfaces shipped 2026-05-19/21: `mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`, `mejepa_chunk_as_star`, `mejepa_constellation_membership`, `mejepa_skill_impact`, `mejepa_skill_graph_inspect`, `mejepa_skill_conflict_graph`, `mejepa_skill_browse`.

#557 extends with skill usefulness evidence: candidate kind, support/fail-pass/lift, genericity, per-cell contribution, live-safe vs target supervision labels, source sequences, lifecycle, uncovered code-state keys, unknown ordered constellations, generic high-support rejections, metric-regression rejections. Proves `stable_background_import_and_impl_scaffold` belongs in context/negative evidence, exposes blanket-expansion regression.

Remaining open: cross-repo skill transfer + pathway-leaf attachment.

Constraint: skills are deterministic groupings of accepted Level-0 labels. Read-only navigation over training substrate; no new prediction surface; binary-leaf constraint preserved at prediction boundaries.

---

## 5. Information-Physics Items (peer-reviewed framing)

Four issues filed against `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`:

| Issue | Status | Source principle |
|---|---|---|
| #511 / TASK-IP-001 — Information-entropy auxiliary loss | OPEN p1; **on the road to 0.95** | Vopson 2nd law of infodynamics (AIP Advances 12, 075310, 2022) |
| #512 / TASK-IP-002 — Self-dual ECC over 15-slot panel | OPEN p1; hygiene | Gates' self-dual ECC in supersymmetry (Doran et al. 2008) |
| #513 / TASK-IP-003 — Connectome rich-club analysis | ✅ CLOSED 2026-05-20 as **demoted negative finding** | FlyWire connectome (Nature 634, 2024-10-02) |
| #514 / TASK-IP-005 — Eon-style cold-load behavioural FSV | OPEN p1; safety net | Eon Systems fly demonstration (March 2026) |

#513 tested and demoted: #526 fixed split reproducibility (`7/8` cells at Jaccard ≥ 0.8), but #527 proved stable topology not promotion-worthy — modularity above null `1/8`, full finite-edge coverage `0`, diagnostic correlation regressed `-0.583`. Topology harness kept diagnostic-only.

---

## 6. Post-0.95 Creativity Engine (autonomous bounded growth)

**Architectural framing.** ME-JEPA is *engineering* half (verifies binary claims against reality). *Creativity* half is **autonomous embedder synthesis** with two oracles gating every proposal:

1. **Novelty oracle** — cross-correlation: candidate orthogonal to existing N? Reject if max correlation > τ_novelty (~0.7).
2. **Utility oracle** — ablation: improves per-cell oracle correlation by ≥ δ_utility (~0.005)?

Only candidates passing BOTH promoted. Rejected candidates write `CF_MEJEPA_EMBEDDER_PROPOSAL_AUDIT` row with typed reasons.

**Bounded by reality.** Orthogonal informative axes ContextGraph admits is bounded by intrinsic dimensionality of corpus. For code N* ≈ 50-500. System discovers own N* because novelty oracle rejects every candidate once corpus fully resolved.

**Five issues (all post-0.95 — none are gate-movers):**

| Rank | Epic | Why | Priority |
|---|---|---|---|
| C-1 | **#517** Mistake-driven embedder demand signal | Closes loop between unresolvable mistakes and embedder synthesis | p1 |
| C-2 | **#516** Novelty + Utility oracle | Two-test gate converts unbounded growth into bounded automated growth | p2 (p1 immediately post-0.95) |
| C-3 | **#518** Embedder retirement audit + Pareto frontier | Vopson 2nd-law applied to embedder set; dominated embedders retire with grace period | p1 |
| C-4 | **#519** Domain-routed sparse panel | Universal ~30 slots + per-domain ~15 extension; 3-5× storage saving | p1 |
| C-5 | **#520** Episodic-to-semantic consolidation | Settled chunks → `(cluster_id, residual)`; 99% storage reduction in high-clustering regime | p2 |

**Sequencing:** C-1 first (demand rows). C-2 (validates proposals). C-3 (closes loop with pruning). C-4 (multi-language scaling). C-5 (steady-state).

**Composition.** Together with v2-stretch creativity tasks (#329 AlphaEvolve outer loop, #335 reverse-prediction, #354-357 EK v2 series), forms **self-resolving observer**: system grows panel until corpus resolved, prunes dominated embedders, specializes per domain, compresses settled clusters. Bounded by reality, not ambition. Storage bounded; search precision maximal.

**Why none are gate-movers for 0.95.** Current `0.7942190170 → 0.95` gap is function of remaining cell-specific ranking weakness and unproven cross-panel generalization, not embedder count or storage architecture. Targeted cell repair + real-input ablation reruns + cross-panel + calibration + entropy loss + mistake loop closes gap. Creativity engine takes over after 0.95.

**NOT in scope.** No new ambiguous-head prediction surfaces. No consciousness / sentience / phenomenology framing. No simulation-hacking / reality-bending claims. Bounded creativity within falsifiable substrate.

---

## 6.9 Neuroscience-Inspired Reserve (filed 2026-05-20, DO NOT START until current queue concludes)

**Status:** all 8 issues filed at `priority:p3`, `status:needs-triage`, with explicit `⚠ DO NOT START — Neuroscience-reserve contingency` markers.

**Activation rule:** *contingency plans* for if current Phase G queue + creativity engine fail to close remaining ship-gate gap. **NOT to be started until:**

1. Current queue concludes (#529 rebaseline → #535 rank lift → #536 weak-cell repair → #547 calibration disjointness → #548 typed blocker → #549 static-signal blocker → #392 temporal features → #511 entropy loss → #405 Panel B → #379 multi-file holdout).
2. Post-queue gap analysis concludes remaining gap not closable with current architecture + corpus + label substrate.
3. Operator explicitly re-prioritizes any (or all) from `p3` based on what gap analysis identifies.

If gate closes via current queue, these issues stay in `p3` reserve indefinitely. If gate does not close, become candidate lever set for next architectural sweep.

### Tier 1 — high-impact, doctrine-compatible, addresses current gap directly

| Issue | Title | Neuroscience source |
|---|---|---|
| **#538 / TASK-PY-NEURO-001** | Predictive-coding head: top-down per-slot prediction + bottom-up residual | Rao & Ballard 1999 (*Nature Neuroscience*); Friston free-energy |
| **#539 / TASK-PY-NEURO-002** | Hippocampal schema-prioritized replay consolidation (incl. reverse replay for pathway credit) | Mattar & Daw 2018 (*Nature Neuroscience*); Liu et al. 2019 (*Cell*) |
| **#540 / TASK-PY-NEURO-003** | Hierarchical predictive coding tied to #404 / #413 / #414 hierarchy | Felleman & Van Essen 1991 (*Cerebral Cortex*); Bastos et al. 2012 (*Neuron*) |
| **#541 / TASK-PY-NEURO-004** | Mixed-selectivity cross-slot binding layer (replaces #390's linear pairwise statistics) | Rigotti et al. 2013 (*Nature*); Fusi et al. 2016 (*Current Opinion in Neurobiology*) |

### Tier 2 — promising but more speculative; design-influence on existing epics

| Issue | Title | Composes with |
|---|---|---|
| **#542 / TASK-PY-NEURO-005** | Population-dynamics low-dim trajectories for #416 (Shenoy/Churchland/Cunningham) | #416 pathway; #392 temporal |
| **#543 / TASK-PY-NEURO-006** | Sparse distributed representations per slot (Hawkins/Numenta; Olshausen & Field 1996) | #519 domain-routed sparse panel; #389 OOD |
| **#544 / TASK-PY-NEURO-007** | Synchrony-based temporal binding for #392 (Singer & Engel; Fries 2005) | #392 temporal; #390 constellation intelligence |
| **#545 / TASK-PY-NEURO-008** | Idle-time generative rollouts (default-mode network analog) | #416, #517, #406, #520 |

### Doctrinal compatibility check (all 8 verified before filing)

- ✅ Slot identity invariant preserved (no cross-embedder dim compare)
- ✅ Anti-compensation invariant (#394) preserved (strong slots cannot silence weak)
- ✅ Binary contract preserved (final verdict still Q2 Pass/Fail; Q3 explanation-only; Q4 frozen)
- ✅ Ambiguity boundary preserved (no ambiguous heads introduced)
- ✅ No inner-LLM assumption introduced
- ✅ No synthetic-input shortcut (all extensions ride real instrument substrate)

### Honest risk note

Neuroscience-inspired architectures have mixed history in ML. Predictive coding (Tier 1A/1C) has strongest empirical track record; mixed selectivity (Tier 1D) is most theoretically grounded; replay consolidation (Tier 1B) is most directly tied to an existing problem (#406 convergence). Tier-2 items are more speculative. None is a guarantee — documented option set, not a commitment.

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

**Major milestone (2026-05-22):** **#372 CLOSED** — ContextGraph roots mounted on aiwonder ZFS pools (`/zfs/archive/contextgraph` → `archive/contextgraph` ZFS, `/zfs/hot/contextgraph` → `hotpool/contextgraph` ZFS). Aiwonder root-readiness blocker satisfied. Legacy mounted-drive paths (`/mnt/d`, `/mnt/c`) forbidden in active code/FSV; CI lint `scripts/ci/check_no_c_drive_paths.sh` + #612 → #613 → #614 → #615 → #631 → #632 → #641 → #666 audit chain enforce behaviorally.

| Issue | Task | Status |
|---|---|---|
| #310 | META Issue refactor | in-progress p0 |
| #372 | ContextGraph roots must be mounted on aiwonder ZFS pools | ✅ closed 2026-05-22 |
| #299-#305 | AIWONDER-003..010 install / transfer / build / launch / calibration | all closed with #372 |
| #295 | PERF — resume launch scans full aiwonder FSV root | in-progress |
| #368 | Host capability utilization audit + scheduler profile | blocked p3 (behind #633/#634) |
| #380 | Off-host backup for `/zfs/archive` proof history | blocked p3 |
| #384 | CUDA/candle rollback and reproducibility lane | blocked p3 |
| #306 | SWE-bench Lite Python Docker image plan (future / Q3) | blocked p3 |
| #312 | hot_root on ext4 throughput bottleneck — superseded by #372 | satisfied; close after #634 confirms no separate scheduler-profile work remains |
| #442 | Track aiwonder librocksdb-sys cstdint native build requirement | blocked p1 |
| #617 | WSL `/mnt/c` capacity snapshots remain in aiwonder FSV scripts | needs-triage p2 |
| #671 | Stale SWE-bench harness defaults point at missing aiwonder archive venv | needs-triage p0 |
| #674 | `run-remote` wrapper can hang after remote command completes | needs-triage p0 |

---

## 9. Active `type:context` Pins (must read at session start)

| # | Title | Why pinned |
|---|---|---|
| **#712** | **Sherlock-validated critical path (2026-05-23)** | **Authoritative ordering.** Decomposes `0.7942` into trainer/checkpoint lineage gap + invalid legacy forward-cache slots + Goodhart metric. New ranks supersede prior §3.0; post-#704/#714 active substrate work is #709/#715. |
| #633 | [P0] Manual full-system FSV and hardening gate | Active in parallel (rank 8 of #712); children #634-#651 still required |
| #310 | META Issue refactor | Phase G execution-order map; now subordinate to #712 |
| #383 | Gate-active issue order must be evidence-weighted (p0) | Evidence per #712 |
| #397 | ME-JEPA consequence-prediction truth-system contract (p0) | Binary contract intact post-Sherlock |
| #227 | Python ME-JEPA data explosion runtime policy | Forward-cache job constraints |
| #296 | aiwonder migration EPIC | Migration plan (closed but load-bearing) |
| #629 | 5-Sherlock 2026-05-22 audit closeout (`type:discovery`) | Indexes audit findings + fixes + new issues |

---

## 10. Bugs (open)

### 10.1 P0 bugs (must repair before #392/#511/#405 resume)

| # | Title | Source |
|---|---|---|
| #618 | Aggregate OOD score uses flat 5,120-byte panel buffer (doctrine §1.5ter) | 5-Sherlock |
| #619 | `forward_inverse_inner` uses `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter) | 5-Sherlock |
| #621 | Hardcoded `gtau_pass_rate: 1.0` published to MCP weekly dashboard | 5-Sherlock |
| #709 | Add inter-embedder distinctness gate before cache promotion | post-#704/#714 |
| #715 | Rebuild static AST forward cache with 9-slot policy and post-fix E8 | post-#704/#714 |
| #671 | Stale SWE-bench harness defaults point at missing aiwonder archive venv | post-#372 |
| #672 | Geometry scalar feature lineage + leakage gate for `head_projection` promotion | post-#657 |
| #673 | Label-quality audit ranks embedder signal vs noise | post-#657 |
| #674 | `run-remote` wrapper can hang after remote command completes | post-#615 |
| #656 | mejepa-heal-drill fails before complete readback on conformal overcoverage | in-progress |
| #616 | Rerun and classify weak-cell oracle timeout/error rows | open |

### 10.2 P1/P2 bugs

| # | Title | Status |
|---|---|---|
| #620 | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook never invoked from training | needs-triage p2 |
| #622 | Trainer dead-code cleanup batch (distill + lora_wiring + aux) | needs-triage p2 |
| #623 | `cold_start_conformal_policy` invalid `alpha=0.0` dead public API | needs-triage p3 |
| #624 | `verdict_assembly` inline `CONTRADICTION_CALIBRATION_INVALID` not declared constant | needs-triage p3 |
| #625 | `DeterministicPredictor` publicly re-exported (synthetic-vector landmine post-#422) | needs-triage p2 |
| #626 | Hygiene retention default policy missing ~90 CFs (`config/retention_policy.toml` stale) | needs-triage p1 |
| #627 | shift-subscriber runner integration test fails on main | needs-triage p2 |
| #628 | Doc/code error-string drift: `MEJEPA_TCT_DIM_MISMATCH` vs `MEJEPA_INFER_DIM_MISMATCH` | needs-triage p3 |
| #630 | Recover long-source/OOM forward-cache quarantines after legal mask | needs-triage p1 |
| #663 | Resolve duplicate TASK-PY-G keys after geometry-label issue creation | needs-triage p2 |
| #307 | promote_tree_verified asserts on second-subrun shard promotion | in-progress |
| #320 | phase_g_project_merkle_cache_fsv clippy — `ProjectMerkleCache` Debug missing | needs-triage p1 |
| #338 | Capture agent response bodies in runtime/agent-history for Q1 claim FSV | blocked p3 |
| #429 | DynamicJEPA Phase 8 FSV intermittently aborts in native record-surprise path | blocked p3 |
| #433 | P1 ability resolver evidence integrity and replay key completeness | blocked p3 |
| #436 | context-graph-mejepa example clippy gate has pre-existing warnings | blocked p3 |
| #453 | Residual Sherlock/investigation findings after first cleanup pass | needs-triage p1 |
| #522 | verification_state test: predicted SubtleFlip failure no longer escalates | needs-triage p1 |

---

## 11. Deferred (not blocking ship gate)

| # | Why deferred |
|---|---|
| #149 | SWE-bench Verified subset — separate dataset; ship gate uses Lite 300×8 |
| #354-#357 | TASK-EK-018 v2 stretch (NAS from scratch / cross-library / self-rewriting / status reallocation) |
| Non-Python ship-gate cells (Rust/JS/TS/Go/Java/C/Cpp/CSharp/Ruby/Php) | Active execution Python-first; non-Python deferred until Python truth system green |

---

## 12. How to Pick Up Work (per AGENTS.md)

1. Run six-issue queries from `AGENTS.md`:
   ```bash
   gh issue list --state open --label "type:context" --json number,title,body
   gh issue list --state open --label "status:in-progress" --json number,title,assignees
   gh issue list --state open --label "status:blocked" --json number,title
   gh issue list --state open --label "source:agent" --search "no:assignee" --json number,title,labels,updatedAt
   gh issue list --state closed --label "type:decision" --limit 20
   gh issue list --state closed --label "type:discovery,type:pattern" --search "<kw>" --limit 20
   ```
2. Read `README.md` for current doc map, then `01 §1-2`, `07 §1`, this roadmap.
3. Find highest-priority open issue you can usefully unblock that does NOT have an in-progress assignee.
4. CLAIM (assign self + flip `status:needs-triage` → `status:in-progress` + post CLAIM comment with files-you-will-touch + ETA).
5. Audit-the-task before writing code (`06 §1.6`).
6. Work; comment at every milestone.
7. Produce FSV evidence per `06 §1.5`.
8. RESOLVED / PAUSE / BLOCKED issue closeout.
9. Code-simplifier pass.
10. File any observed defect / pattern / discovery as fresh issue before ending turn.

---

## 13. What This Doc Does NOT Cover

- Day-to-day operator decisions outside the issue tracker — those happen in conversation with operator and become `type:decision` issues when locked in.
- Hardware procurement / aiwonder admin — `phase:migration` issues track that subset.
- Time estimates — Phase G has no public ETA; per-cell convergence ETA tracker emits internal projections used by heal scheduler.
