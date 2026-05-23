# 07 — Results, Benchmarks, and FSV Evidence History

Source: aiwonder `/zfs/archive/contextgraph/fsv/` for production evidence; git log; GitHub issue comments; per-issue RESOLVED comments.

**Purpose:** preserve the proof-of-work that future agents would otherwise lose. Every entry references a physical artifact (FSV JSON, sha256, CF) that can be re-verified.

---

## 0a. ⚠ 2026-05-22 hardening pivot — measurement validity requires re-affirmation

The 2026-05-22 5-Sherlock parallel forensic audit (#629 closeout) plus the **#633 manual full-system FSV + hardening gate** opening means **every measurement in this doc is now provisional** until manually re-verified under the manual SoT contract (`06_operations_and_fsv.md §1.9`). Gate-moving conclusions ("the predictor is at 0.7942 rank-lift", "117/117 active true-Fail rows have FailLikely coverage", "#615 candidate plan is valid") were recorded against auto-FSV harnesses; per #655, those harnesses are being retired in favour of operator-driven SoT readback as part of `crates/*/examples/*_fsv.rs` retirement.

The numbers below remain the best available evidence; they are not gate-eligible until #633 sub-tasks (#634-#651) RESOLVED/BLOCKED with manual SoT before/after captures.

**P0 audit-uncovered defects (`§2.0` below) gate any further claim of correlation-class improvement:**

- **#618** aggregate OOD over flat 5,120-byte packed buffer (doctrine §1.5ter violation)
- **#619** `forward_inverse_inner` flat `Linear(PANEL_DIM, hidden)` projections (doctrine §1.5ter violation)
- **#621** hardcoded `gtau_pass_rate: 1.0` in HoldoutReport published to weekly dashboard
- **#670** invalid forward-cache spaces `e2/e3/e4/e8` (forward-cache embedder collapse uncovered by #666)

These three doctrine violations + the embedder-collapse finding mean the `0.7942190170` rank-lift number itself rests on a substrate that contains known-broken paths. Re-validate under manual FSV before re-claiming.

---

## 0. ⚠ CRITICAL: pre-fix evidence partially invalidated

Between Phase G ramp and 2026-05-19 22:45 UTC, the inference path used SHA256-seeded synthetic vectors (`crates/context-graph-mejepa/src/compiler.rs::vectors_for` calling `deterministic_vector(slot, &digest)`) instead of real `E*Instrument` encoders. **The 0.866667 ship-gate measurement and every ablation that compared against it are INVALID** — they measured a synthetic-input pipeline, not the real predictor. See `01_overview.md §1.7` for the full forensic.

| Status | Evidence |
|---|---|
| **REAL (no rebaseline)** | AST chunks, 391-shard forward cache, #388 tensors, #389 calibration, #413 labels, #414 skills, #404 hierarchy, anti-compensation invariants |
| **INVALID (must rebaseline)** | Ship-gate 0.866667; `λ_inv` ablation; #393 / #398 ablations; "labels are the bottleneck" claim; "slot-preserving doesn't close gap" claim |

The corpus survives. The pre-fix measurement does not. The post-fix real-input rebaseline now exists, but is itself **provisional** (see §0a) until manually re-verified.

---

## 1. Current Ship-Gate Status

| Metric | Target | Latest measurement | Status |
|---|---|---|---|
| `prediction_oracle_correlation` | ≥ 0.95 (stable × 4 rolling windows × all Python mutation cells) | **0.7942190170 rank-lift calibrated** (`0.6916886568` rank-safe, `0.4843192995` raw) | Post-fix aiwonder real-input rebaseline exists. #535 code-state-constellation rank-lift improved correlation by `+0.1020384431` over #534; #536 hard-negative weighting added another `+0.0004919171` correlation, Brier `0.0936445221 → 0.0923729688`, ECE `0.0338404700 → 0.0240316074`. Gate still fails on per-cell ranking and statistical-power blockers. |
| Cross-panel oracle correlation (Panel B) | ≥ 0.95 with `\|corr_a − corr_b\| ≤ 0.05` | **Panel B active-surface parity green** on the #392 205-row active subset: `corr_A=0.9526342154`, `corr_B=0.9902423024`, `\|corr_A-corr_B\|=0.0376080871` | Comparable #405/#521 parity exists for the active #392 surface; full ship-gate still waits on #612 → #392/#611 → #150/#610 → #167. |
| `mistake_repeat_rate` | ≤ 0.05 (rolling 100-mistake window) | byte-readable via `mejepa_mistake_loop_status` | Awaits real predictor mistakes (machinery shipped #406/#523; needs post-fix predictor to generate disagreements) |

**Gap to ship:** `0.1557809830` against the rank-lift calibrated real-input Panel A measurement. Pre-fix `0.866667` is not comparable because it measured synthetic vectors.

**Metric contract update (#532).** Per-cell Pearson is not defined for zero-variance oracle cells. The active ship-gate FSV now classifies every block as `mixed`, `all_fail`, `all_pass`, or `empty`: mixed blocks use Pearson correlation; degenerate all-fail/all-pass blocks use target-class recall plus calibration (`actual_class_confidence_mean`, Brier, ECE). Each block also carries statistical-power diagnostics so a point estimate cannot be mistaken for a powered readiness claim.

**Calibration + invalid-source + rank-safety + rank-lift update (#533/#531/#534/#535/#536).** The ship-gate FSV fits deterministic calibration/rank-lift policies on the 240-row calibration split only, then evaluates the 240-row holdout. Constants are allowed only for structurally degenerate `compile_error` / `wrong_file` cells. Rank-reversing transforms are allowed only when the calibration split has mixed-class support ≥ 8 per class and raw Pearson ≤ -0.25; this prevents calibration-split overfit from flipping holdout ranking. The rank-lift head uses category-local RBF exemplar scoring over real `code_state_constellations.jsonl` slot-feature means and hashed live label counts while excluding target-side oracle/docker/test labels; #536 added calibration-selected train-only hard-negative weighting (`tau=20.0`, `fail_boost=1.5`). Latest ship-gate FSV: `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`, report sha256 `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499`, readback sha256 `9cf352a775a38df87f17e6b1b0964d0ec93b93de9d5447b6bc762294c1b67ae2`; row-trace FSV for #549/#392 remains `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T185945Z/`, report sha256 `781aa3f6527abed2add930a8a19f9eb790fdbe3c86b81f27006ae4ca48ca2506`. `compile_error` rows use raw invalid-source diagnostic panels rather than carrier modules. #535 lifted overall calibrated correlation (`0.6916886568` → `0.7937270999`); #536 lifted again to `0.7942190170`, with Brier `0.0923729688` and ECE `0.0240316074`. #536 improved one-fail cell Pearson definition/Brier, `swap_variable` Pearson/Brier, `subtle_flip` Brier, and `off_by_one` fail recall; threshold behavior still saturates several weak cells, so this is a small repair, not a gate close.

**Post-#536 calibration-disjointness revalidation (#547).** The #407 reusable auditor was closed before the current #536 real-input policy existed, so #547 revalidated the actual rank-lift path. FSV: `/zfs/archive/contextgraph/fsv/post-536-calibration-disjointness-fsv/run-20260520T181254Z/post_536_calibration_disjointness_fsv.json`, report sha256 `d90152ebd41b5e626ca8b9882dbba185d711f13aa69048e7e627fa854d4b27e8`, readback sha256 `2d58c9e3dd0c70d08202656f85596058287c77eb9a48aa1bb15010d89d19f0c7`. Result `all_passed=true`: train/calibration/holdout row-key overlaps are all zero; live feature-member overlaps are all zero; 3,000 code-state rows map to 2,400 unique corpus keys as expected; #413 label-lineage hashes match; selected `tau=20.0, fail_boost=1.5` is calibration-selected; no target-side label prefix enters live rank-lift features.

**Post-#547 mixed-cell repair diagnostic (#548).** Targeted follow-up FSV: `/zfs/archive/contextgraph/fsv/post-547-mixed-cell-ranking-repair-fsv/run-20260520T184458Z/post_547_mixed_cell_ranking_repair_fsv.json`, report sha256 `acd57aba489a73816b114af0440ae06e07f2e195a1d1f91c0003261220655551`, readback sha256 `a85d42eda42e76e4e5ce787345f934f1977e42609ba5de597a1f1cab6e8e9009`. Result `all_passed=true` with typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED`: the harness reproduces #536 exactly (`0.7942190170`, Brier `0.0923729688`, ECE `0.0240316074`), proves legal calibration-selected candidates still leave true mixed cells threshold-degenerate (`off_by_one`, `subtle_flip`, `swap_variable`), and records a forbidden holdout-selected counterfactual that would lift correlation to `0.8076422215` and Brier to `0.087037511`. The conclusion is not "no signal"; it is "current signal is not calibration-stable enough to promote." Follow-up issue #549 now owns adding legal mixed-cell signal.

**#549 mixed-cell signal fusion diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-549-mixed-cell-signal-fusion-fsv/run-20260520T191159Z/task_py_g_549_mixed_cell_signal_fusion_fsv.json`, report sha256 `b6b391e97670e1cf493f009455eb29143cc4636c540fea9250c85837ead090c1`, readback sha256 `f7c6d085fe1be4f85b70c1222a639d1d1dd57c42774867d2f2a01f91e1ab818c`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `MIXED_CELL_SIGNAL_FUSION_INSUFFICIENT`. Repo/touched-path metadata remains useful (`0.7974180579`, Brier `0.0911292881`, ECE `0.0156852305`) but insufficient. The formal 1,037-candidate calibration-selected raw/rank-safe/rank-lift score-fusion sweep finds best legal holdout correlation `0.7945368886`, Brier `0.0923306793`, ECE `0.0208097622`; `subtle_flip` remains all-fail/non-positive-correlation and `swap_variable` remains all-fail. Edge cases pass: missing row traces fail closed, mismatched row identities fail closed, and holdout-selected weight tuning is flagged forbidden. The next gate-moving evidence must be temporal/multi-chunk trajectory evidence or cross-window stability, not more static score blending.

**#392 first slice: aggregate temporal trajectory diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T192449Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `a7d763d72c11cafeabc6f1d53728cced2d97c07430b78a707b1107efd64136e3`, readback sha256 `d16bfaa402f19475b858826a1f1024f9be2982d6bce1629101862f49b1afd0cb`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `TEMPORAL_TRAJECTORY_FIRST_SLICE_INSUFFICIENT`. The feature view is 1,232 dimensions over ordered base → candidate → test_patch trajectories: per-slot deltas, cross-slot delta relationships, chunk/file movement, and live label transitions. Best calibration-selected legal candidate reaches holdout correlation `0.8008320332`, Brier `0.0897067040`, ECE `0.0253749657`; this is the first post-#536 legal surface above `0.80`, but ECE regresses and `subtle_flip` / `swap_variable` remain threshold-degenerate. Edge cases pass: ordered replay works, non-monotonic order fails closed, missing code-state rows fail closed, and target-side labels remain excluded.

**#392 second slice: file-path + skill/ability trajectory diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T195910Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `e686fd1387458814ec150939f58188522743d7c5fa968567643bf43d1dd014a1`, readback sha256 `780710bbbeb1e0c63df7013df803f17f44a603d0cca9d837d53c0fa2f7222532`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED`. The feature view is 2,177 dimensions over ordered code-state, file-path, and skill/ability trajectories using the v2 live-split label root plus #110 hierarchy RocksDB; slot identity is preserved and cross-slot relationships are scalar-stat aggregates, not a fused 5,120-d semantic vector. Best calibration-selected legal candidate reaches holdout correlation `0.8422425985`, Brier `0.0726622641`, ECE `0.0386186689`, accuracy `0.9000`, pass recall `0.8583`, and fail recall `0.9417`. This is real directional lift (`+0.0480235815` correlation and `-0.0197107047` Brier vs #536), but it is still non-promotable because ECE regresses and `swap_variable` remains threshold-degenerate. Edge cases pass: ordered replay works, non-monotonic order fails closed, missing code-state rows fail closed, missing file-path rows fail closed, hierarchy DB readback passes, holdout threshold selection is flagged forbidden, and target-side labels remain excluded.

**#392 third slice: out-of-fold probability calibration diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T201911Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `dbf9208774b714db1f72e113a4b4a3845cdd807bbd755a57e5edd3c41bd645a3`, readback sha256 `845b72c075db37ded8e918eca9b481fc4c5405c792aaa1d41005bf8c61d4e1ef`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED`. The selector now tests deterministic probability-calibration maps with out-of-fold calibration selection, not in-sample bin memorization and not holdout tuning. The selected calibration removes the selected candidate's mixed-cell threshold degeneracy, but holdout correlation falls to `0.8157011867`, Brier is `0.0843216032`, and ECE worsens to `0.0556452833`; the only remaining selected-candidate blocker is `aggregate_ece_regressed`. Conclusion: sparse bin calibration is not the missing gate mover. Next work must preserve the stronger `0.8422425985` file/skill trajectory signal while adding robust per-cell or cross-window calibration that does not overfit the 240-row calibration split.

**#392 fourth slice: affine file/skill calibration diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T203036Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `289e050be36ddd81672f1f76f01f9dd6f55f22e801d8ebe603efcf637c58d09e`, readback sha256 `2badb703d12df4a41edd7b7246367418eb5139ba8b8990702822edbf9d7dd73c`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED`. The selector now includes a fixed complexity penalty for sparse probability-bin calibrators and selects the simpler affine true-mixed calibration: base candidate `category_local_knn(k=31, fail_boost=1.5)`, scope `true_mixed_per_category`, scale `1.30`, offset `0.02`. Holdout correlation rises to `0.8497224450`, Brier improves to `0.0698330477`, accuracy to `0.9083333611`, pass recall to `0.8916666508`, fail recall to `0.9250000119`, and predicted pass/fail support is non-degenerate. The remaining blocker is only `aggregate_ece_regressed: 0.031462930`, so the next legal repair must reduce ECE below the #536 baseline `0.0240316074` without losing this affine trajectory lift.

**#392 fifth slice: hardened affine/blend diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T205039Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `a197f49ebdaefa4d00366d464f4d085033de13b67276254a46a328872bde60ae`, readback sha256 `d7632930bb83fc76d9b65832fc313d3b298fd49b3de06c7cfc81ac4232916487`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED`. This run hardens the #392 FSV contract: #536 `holdout_predictions` reproduce the baseline metrics exactly, affine calibration edge cases pass, and blocker typing only emits ECE-specific code when the selected legal candidate's blocker list is ECE-only. The selector also tests affine/rank-lift blends; the best blend (`affine_weight=0.95`, `rank_lift_weight=0.05`) reaches holdout correlation `0.8484774828`, Brier `0.0703259036`, and ECE `0.0312432162`, but does not beat the affine calibration objective. The selected legal candidate remains the affine true-mixed calibration with correlation `0.8497224450`, Brier `0.0698330477`, accuracy `0.9083333611`, and blocker `aggregate_ece_regressed: 0.031462930`.

**#392 sixth slice: logit calibration diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T205929Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `6c040688cf5e52428e69dffe0270e1605c3158d8da6403bbca1abae45239f7af`, readback sha256 `b6e26e4fa862e8e03515ec8ec96c0b906f1d27d8603d82ecf305f001dda0c037`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED`. The selector now tests monotone logit score compression/shift on calibration rows only. Best logit candidate is `category_local_knn(k=31, fail_boost=1.5)`, scope `true_mixed_per_category`, slope `1.30`, bias `0.10`; holdout correlation is `0.8488159180`, Brier `0.0703388974`, and ECE `0.0342222452`. It does not beat affine and worsens ECE, so logit calibration is ruled out as the immediate repair. The selected legal candidate remains affine with correlation `0.8497224450`, Brier `0.0698330477`, and blocker `aggregate_ece_regressed: 0.031462930`.

**#392 seventh/ninth slice: mean-bias + support-aware coverage hardening diagnostic.** Best FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T214049Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `8e3644db8b0a896632741d18d871d7affc2f98a0238785009833c608a5adeac1`, readback sha256 `40b9c60998f0bc1cdc61861dc97d6f9cca9c2a88038ffa4e54f183b58d26edf9`. Result `all_passed=true`, `promotable_candidate_found=false`, typed blocker `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED`. This run keeps split-legal mean-bias calibration over affine scores with out-of-fold calibration scoring and hardens the evidence contract: missing slot/scalar values, missing unknown skill summaries, missing file-path rows, non-monotonic trajectories, and support-shrink key gaps all fail closed. Legitimate code-state keys missing from the #110 skill hierarchy are recorded as explicit coverage gaps, not silent zeros: `1531 / 3000` expected code-state keys are covered and `1469` are uncovered; the later #551 closeout resolves those uncovered states as pass/context-only negative-control evidence, not an active coverage repair. The selected policy remains `calibration_selected_mean_bias_calibration`: affine base `category_local_knn(k=31, fail_boost=1.5)`, affine scope `true_mixed_per_category`, scale `1.30`, offset `0.02`, then global mean-bias shrinkage `0.50` with bias `-0.0039657354`. Holdout correlation is `0.8497869968`, Brier `0.0697008073`, and ECE `0.0299093761`. Support-aware prior shrinkage is not selected: it slightly improves holdout ECE to `0.0298766457` and correlation to `0.8497893214`, but worsens calibration objective (`-0.0210823119` vs mean-bias `-0.0211772919`). It remains non-promotable because ECE is still above #536's `0.0240316074`. A heavier ECE-weighted selection run (`run-20260520T211137Z`, report sha256 `785e6020e49b87f979ade9f42f0810ab43b70f0a1f93b1a65c22f3c7fe94b0c0`) was worse: correlation `0.8465248346`, Brier `0.0711186603`, ECE `0.0305833183`, so the tree keeps the standard selection weight.

**#554 / TASK-SKILL-002 usefulness-weighted ordered skill diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-skill-002-usefulness-weighted-392-fsv/run-20260520T232838Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `92d9f983c32f50cb62cb217ddbe95a7c2da3cfabf243199e194ea000f2fff0a8`, readback sha256 `2950ae4df088924397b2c39aa438e1766f7711575617a2524020034d5c99ede1`. Result `all_passed=true`, selected skill policy `current_raw_skill_surface`, typed blocker `TASK_SKILL_002_USEFULNESS_WEIGHTED_NONREGRESSING_BUT_ECE_BLOCKED`. The FSV adds no-skill, raw-skill, blanket-cap negative-control, failure-only, pass-stability-only, context-only, and usefulness-weighted combined skill ablations while preserving split disjointness and target-label exclusion. Combined usefulness weighting improves holdout correlation/Brier vs the raw skill surface (`0.8679359555` / `0.0620564893` vs `0.8497869968` / `0.0697008073`), and failure-only is even stronger (`0.8830994964` / `0.0554981381`), but both regress ECE (`0.0396374688` and `0.0643141195` vs raw `0.0299093761`). The selector therefore rejects them as non-promotable and keeps the raw surface active. This is strong evidence that ordered skill semantics help consequence prediction, but the repair target is calibration, not blanket skill expansion.

**#558 / TASK-SKILL-006 full-corpus targeted skill rematerialization + #392 rerun.** Materialization FSV: `/zfs/archive/contextgraph/fsv/task-skill-006-full-corpus-skill-rematerialization-fsv/run-1779320263984/task_py_g_118_aiwonder_skill_materialization_fsv.json`, report sha256 `27d24719ff2f3c310f1a757d9f5bf142be712c9deae0049b7cc83439cd66d6a0`. Rerun FSV: `/zfs/archive/contextgraph/fsv/task-skill-006-full-corpus-skill-rematerialization-fsv/run-20260520T233926Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `2c3c7cea2ced0afc4715bad03624401ea497cfee1260d62ce7f305f5fb11828a`, readback sha256 `07dbede6b8cd844060c94e5c31b281f748fcd08998ef5668a8d9ac9070b01829`. Result `all_passed=true`: the materializer consumed `399777` chunk rows from the v2 aiwonder label root, generated `398215` episodes, and wrote `157` skills, `9489` memberships, `4364` reverse-index rows, and `157` lifecycle audit rows. Candidate profile counts were `7839` failure skills, `29380` pass-stability skills, `47` context-negative-evidence skills, and `2124` neutral diagnostics; selected candidates were `128` failure skills plus failure coverage backstop. The #392 rerun selected `usefulness_weighted_combined` as the non-regressing active policy, lifting legal holdout correlation/Brier to `0.8679359555` / `0.0620564893` with accuracy `0.9083333611`, pass recall `0.9166666865`, and fail recall `0.9000000`. It is still blocked by aggregate ECE `0.0396374688`, so the remaining blocker is split-legal calibration, not skill materialization or blanket coverage.

**#560 / TASK-SKILL-008 split-legal ECE repair attempt.** FSV: `/zfs/archive/contextgraph/fsv/task-skill-008-split-legal-ece-repair-fsv/run-20260521T004021Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `c2505eeb6f755d362510b61d78437f364029deb000363d54715ea3238eec6eee`, readback sha256 `3b401c081af4cf415f8270ee6e1ae07752b0a8aa7d481eae95927cecf69040fa`. Result `all_passed=true`, typed blocker `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED`. The run added a calibration-selected ECE-focused mean-bias diagnostic over the #558 skill DB; it selected the same global model as the standard mean-bias path and did not reduce holdout ECE (`0.0396374688`). Support shrink remains slightly lower on ECE (`0.0396139733`) but not promotable. The typed blocker now explicitly points at the next missing signal: per-cell or per-constellation calibration and consequence-to-cause traces, not aggregate ECE weighting, support-only shrinkage, or holdout threshold tuning.

**#557 / TASK-SKILL-005 skill usefulness audit surface.** FSV: `/zfs/archive/contextgraph/fsv/task-skill-005-skill-usefulness-audit-surface-fsv/run-20260521T023736Z/task_skill_005_skill_usefulness_audit_surface_fsv.json`, report sha256 `5a701604deaa7cff8e086b5f4ed36e2a8dfc7bfe1fe3b4b4f877b7860b610036`. Result `passes=true`. Direct aiwonder `ldb` readback: `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS=6`, `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP=2258`, `CF_MEJEPA_SKILL_REVERSE_INDEX=1129`, `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT=7`. The audit surface exposes candidate kind, support/fail-pass distribution/lift, calibration-impact proxy, per-cell contribution count, live labels vs target supervision labels, source-byte representative sequences, lifecycle history, uncovered code-state keys, unknown ordered constellations, generic high-support rejections, and metric-regression rejections. It proves the five manually derived #551 patterns are visible, keeps `stable_background_import_and_impl_scaffold` as context/negative evidence rather than a failure skill, and makes the blanket expansion regression visible as rejected/demoted evidence. This does not change the ship metric; it gives operators and future agents the read surface needed to preserve targeted skill usefulness while repairing ECE.

**#551 / TASK-PY-G skill coverage gap closeout + corrected #392 rerun.** #551 is resolved as a justified negative-control finding. Direct aiwonder readback showed `3000` code states, `1531` skill-covered states, `1469` uncovered states, `1139` failure-evidence rows, and `1139 / 1139` failure-evidence rows covered; all uncovered target and non-target states were pass/context-only. The materializer fix rejects `/mnt/d` active paths, skips target/oracle labels as live skill input, and rejects overbroad high-support static patterns. Default-128 materialization FSV: `/zfs/archive/contextgraph/fsv/task-skill-009-targeted-skill-coverage-repair-fsv-default128-final/run-1779333467960/task_py_g_118_aiwonder_skill_materialization_fsv.json`, report sha256 `ba2446b603edd648a64eb327e9375a8539c251393949c1acec24f9b1acb4b20b`; direct `ldb` counts were `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS=155`, `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP=9489`, `CF_MEJEPA_SKILL_REVERSE_INDEX=4364`, `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT=155`. The corrected #392 rerun after fixing evidence-consequence double-calibration is `/zfs/archive/contextgraph/fsv/task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `9ed8830f184e0aca07a1ee47897b31b342dbdaf2db0bacc28d0a08981dfbc29c`, readback sha256 `9d8f73a794a98b84ed7a4d89e7c9c3037b94b0e5bf27fe0d3c9f94a8e436a9c3`, `all_passed=true`, and no retained `/mnt/d` references. Legal best is `calibration_selected_mean_bias_calibration`: correlation `0.8770877719`, Brier `0.0581485741`, ECE `0.0475411229`, accuracy `0.9250000119`; generalized skill-kind evidence-consequence and bin-consequence are worse (`ECE 0.0567991696` and `0.0570490398`). Conclusion: more skill coverage or broader evidence-key ladders are not the repair; #392 needs a different split-legal calibration/signal family.

**#392 evidence-semantics hardening.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-evidence-semantics-hardening-fsv-release/run-20260521T074421Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `edf4755f44ccb31a3eaafc5ad07f842e832e9cd684b5838cde53eba8715f6366`, readback sha256 `4cddf2db7c0732e3b33164b7bbfe0207e2b9dc3ecde7e3f3233c004014af8655`. Result `all_passed=true`, `edge_cases.all_passed=true`, no retained `/mnt/d` references. The harness now standardizes cause-residual features from legal fit rows, stores mean/std in the model, requires cause-bin application stats in calibration and holdout, adds direct cause-residual/cause-bin edge cases, and rejects exact `/mnt/d`, `/mnt/d/*`, `/mnt/c/*`, and non-aiwonder active roots. Legal best is `calibration_selected_cause_bin_calibration`: correlation `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`, pass recall `0.9333333373`, fail recall `0.9416666627`. Cause-bin application is real, not a key-stats no-op: calibration applied `62 / 90` scoped rows and holdout applied `46 / 90`. The accepted blocker remains `aggregate_ece_regressed: 0.040213395`, so this is the strongest legal correlation/Brier slice so far but still not promotable.

**#392 final-bin calibration diagnostic.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-392-final-bin-guarded-fsv/run-20260521T090040Z/task_py_g_392_temporal_trajectory_features_fsv.json`, report sha256 `4d3cefc3c7484854978da56fb4da0a031c4061505ebca9a6b8ac5773ee52bc87`, readback sha256 `aa9de48e9e573a1db39a69e78785bdab2ada2c88ec2449943b7e37ed8ebc4c26`. Result `all_passed=true`, `promotable_candidate_found=false`. The diagnostic adds a split-legal final probability-bin layer over cause-bin rows and an explicit final-bin selector guard: a final-bin candidate must beat cause-bin by a material calibration-objective margin and pass split non-regression checks before it can become legal best. The best final-bin model was global, `bin_count=5`, `smoothing=3.0`, `raw_weight=0.95`; it slightly improved holdout correlation/Brier to `0.8941474557` / `0.0503885597` but worsened ECE to `0.0432727709`, so it is retained with `selection_forbidden=true` (`objective_delta_below_material_margin`). The selected legal best therefore remains cause-bin calibration at correlation `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`, with blocker `aggregate_ece_regressed: 0.040213395`. Final aggregate binning is now a retained negative result, not the next repair path.

**#610 / #150 / #611 burn-in admissibility and active-point repair closeout.** #610 FSV `/zfs/archive/contextgraph/fsv/task-py-g-133-burn-in-admissibility-policy-fsv/run-20260522T123721Z/task_py_g_133_burn_in_admissibility_policy_fsv.json`, report sha256 `d766c06d8da35f5a1e55cfbc7981b464ce65a595d910ca16bfbc15c7e465658c`, reports `all_passed=true`, `burn_in_start_allowed=false`, and `TASK_PY_G_133_BURN_IN_POWER_NOT_ADMISSIBLE`. #150 FSV `/zfs/archive/contextgraph/fsv/task-py-g-019-sample-efficiency-power-fsv/run-20260522T124456Z/task_py_g_019_sample_efficiency_power_fsv.json`, report sha256 `585c54502084c6e96037d1c49df6b901a48822284a8c839c76c9542fe8513019`, reports `support_policy=blocked`; longer windows or row-support policy alone cannot start burn-in while the three active point metrics remain sub-target. #611 FSV `/zfs/archive/contextgraph/fsv/task-py-g-134-active-point-metric-repair-fsv/run-20260522T130250Z/task_py_g_134_active_point_metric_repair_fsv.json`, report sha256 `ccea46d6ac5677cfc7a9b849c9dd70322570ae281ba6a7b8a23d76e6d772704f`, readback sha256 `bf1c67b6186d6dfa7dedf21744d6d87c960523a8ff616b356862cf0353d636fc`, reports `all_passed=true`, `repair_promoted=false`, and typed blocker `TASK_PY_G_134_NO_EXISTING_LEGAL_WEAK_CELL_REPAIR`. The weak cells remain `delete_test_call` (`corr=0.6948082447`), `off_by_one` (`0.8675352335`), and `subtle_flip` (`0.8035093546`). #612 is the current P0: produce or falsify split-stable additional evidence for those cells, starting with `delete_test_call` failure-bearing train/calibration support.

**#612 / TASK-PY-G-135 data-substrate lineage audit.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-135-weak-cell-evidence-expansion-fsv/run-20260522T133526Z/task_py_g_135_data_substrate_lineage_fsv.json`, report sha256 `5ff5ab01f4c1fba1d1286dded66c2d3941d01021717257fea0c9cf950ea609ed`, readback `/zfs/archive/contextgraph/fsv/task-py-g-135-weak-cell-evidence-expansion-fsv/run-20260522T133526Z/task_py_g_135_data_substrate_lineage_readback.json`. Result `all_passed=true`, `evidence_expansion_promoted=false`, typed blockers `TASK_PY_G_135_LEGACY_ABSOLUTE_PATHS_IN_AST_CHUNKS`, `TASK_PY_G_135_AI_WONDER_CANNOT_DEREFERENCE_RETAINED_ABSOLUTE_PATHS`, `TASK_PY_G_135_LEGACY_ABSOLUTE_PATHS_IN_FORWARD_PACKED_RECORDS`, `TASK_PY_G_135_INCOMPLETE_PER_EMBEDDER_SLOT_COVERAGE`, and `TASK_PY_G_135_DELETE_TEST_CALL_MISSING_EMBEDDER_SIGNAL`. The raw AST artifact is retained on aiwonder, but all `399777 / 399777` rows still carry legacy `/mnt/d` in `absolute_path` and `dataschema`; aiwonder has no `/mnt/d`; known source-readback consumers still dereference `absolute_path`. Active v2 labels are not the path problem (`0` `/mnt/d` lines across the label root), and `index.json.patch_path` is clean (`0` absolute/mounted/parent-escape paths). Forward-cache vectors are real and slot-keyed, but coverage is incomplete: `4665772 / 4797324` expected chunk/embedder pairs exist, with `131552` quarantined pairs. Missing slot coverage is gate-relevant: `delete_test_call` has `95` rows with zero slots and `2498` rows with six slots; `off_by_one` has `70` zero-slot and `2054` six-slot rows; `subtle_flip` has `69` zero-slot and `2039` six-slot rows. Current `delete_test_call` split support is not learnable (`238 pass / 2 fail` train, `30 pass / 0 fail` calibration, `29 pass / 1 fail` holdout). Follow-up P0s: #613 source-lineage canonicalization, #614 forward-cache slot completion/missingness mask, #615 delete-test-call assertion-duty regeneration, and #616 weak-cell oracle exception reruns.

**#613 / TASK-PY-G-135A aiwonder AST source-lineage canonicalization.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-135a-aiwonder-source-lineage-canonicalization-fsv/run-20260522T140211Z/task_py_g_135a_aiwonder_source_lineage_canonicalization_fsv.json`, report sha256 `e518bdacb20281b6907bf119fae4b506298bebb4f5ed9b5c86f6299efc7bf13a`, readback sha256 `f0e05b6191fd014d37186fafc757938856cc373341658224b54dfe3d24e7315c`, canonical AST sha256 `bc13e46a6262209b617a624f451e05e1f37e9dddd0d97b624538d7aefb9d0234`. Result `all_passed=true`, `promoted=true`. The missing 300x8 source tree is now materialized on aiwonder at `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z` with `4224` files and `148346301` bytes; spot hashes match the retained legacy source bytes. Active `python_ast_chunks.jsonl` now has `399777` rows with `0` `/mnt/d` or `/mnt/c` hits in `absolute_path` and `dataschema`; the old mounted-drive values are retained only as `legacy_provenance_path` / `legacy_dataschema_path` on all rows. The canonicalizer verified per-row file hash, embedded `source_text` hash, and exact `byte_start:byte_end` source slice before promotion. `python_ast_chunks.py`, `python_static_graph_core.py`, `python_static_graph.py`, and `smoke_manifest.py` now share fail-closed source readback behavior: legacy mounted active paths are rejected, canonical aiwonder paths are hash-verified, and smoke-manifest legacy rows can only use embedded source text after sha verification. #613 removes the active `/mnt/d` source-lineage blocker; #614/#615/#616 remain the immediate data-substrate blockers before rerunning #392/#611/#150/#610.

**#614 / TASK-PY-G-135B forward-cache slot-completion mask.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-135b-forward-cache-slot-completion-fsv/run-20260522T141655Z/task_py_g_135b_forward_cache_slot_completion_fsv.json`, report sha256 `8da6c7a213aa3b442b4909e5fae4ec170b827022c819132d0f5ab38ab0334092`, readback sha256 `6f9b74927b08333bdc46688ca405e1999a535344a35b10f4876266a7a301e17f`, mask sha256 `b298087f960cea25fde4420653ffb16943bcf92708a2564d9b17077634d2b70c`. Result `all_passed=true`, `legal_missingness_mask_promoted=true`, `slot_completion_promoted=false`. Active AST rows have `0` mounted active path hits. The forward-cache index has `4665772` unique present chunk/embedder rows and `0` unknown rows; the missing `131552 / 4797324` expected chunk/embedder pairs are all quarantined (`0` unaccounted). Root cause is not live `/mnt/d` dereference anymore: old mounted paths remain historical metadata in quarantine/packed records, while the missing slot substrate is long-source quarantine (`124019` `MEJEPA_FORWARD_TOKEN_TOO_LONG`, `7524` `MEJEPA_FORWARD_INPUT_TOO_LARGE`) plus `9` GPU-OOM rows. The retained mask lives at `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl` with `399777` rows: `378444` full 12-slot rows, `20706` partial rows, and `627` zero-slot rows. The active forward-cache pair universe is `66` pairs per complete 12-slot row; the full 15-slot panel universe is `105`, but forward-cache-only data is not allowed to claim complete 105-pair constellations. Downstream #392/#611/#150/#610 reruns must join this mask and treat `missing_slots` as explicit features/guards before making complete pair claims. Weak-cell missingness remains gate-relevant: `delete_test_call` has `95` zero-slot and `2498` six-slot rows, `off_by_one` has `70` zero-slot and `2054` six-slot rows, and `subtle_flip` has `69` zero-slot and `2039` six-slot rows.

**#615 / TASK-PY-G-135C delete-test-call regeneration plan.** FSV: `/zfs/archive/contextgraph/fsv/task-py-g-135c-delete-test-call-regeneration-fsv/run-20260522T143109Z/task_py_g_135c_delete_test_call_regeneration_fsv.json`, report sha256 `c1431c11bd249b0bb51f24d1349080a678a69543b2430afcf1a2da397460d918`, readback sha256 `d07c1d5006988bd81ef709d98ac7f855f66d3516dae79e3f4ee16822baf68f40`, candidate plan sha256 `47997f6b7f0582166ca6a008fc760fddcfe6fca97e0ce342379adc8d9034b324`. Result `all_passed=true`, `regeneration_plan_materialized=true`, `regeneration_complete=false`, and typed blocker `TASK_PY_G_135C_DELETE_TEST_CALL_ZERO_CALIBRATION_FAILURE_SUPPORT`. Existing `delete_test_call` is still not learnable: the only observed failure-bearing sublabel is `production_plain_assert_deleted`, with `2` train failures, `1` holdout failure, and `0` calibration failures (`160/22/20` pass rows across train/calibration/holdout). The plan enumerates `1131` deterministic aiwonder-source candidates for Docker oracle execution: `231` `production_plain_assert_deleted`, `89` `test_self_assert_deleted`, `50` `pytest_raises_deleted`, and `761` `other_assertion_like_deleted`. Stable candidate partitions include `26` calibration production-plain candidates and `2` candidates from currently failure-bearing source files in calibration. Next action is an aiwonder Docker oracle batch over `/zfs/archive/contextgraph/state/regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1/delete_test_call_regeneration_candidates.jsonl`, then rebuild AST chunks, #614 forward mask, labels, #392, and #611 from the new oracle outcomes.

**Real-input calibrated constellation ablation rerun (#537 / #398).** The #398-style calibrated constellation ablation was rerun against the then-current #535 `0.7937270999` baseline: `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-realinput-fsv/run-20260520T170156Z/`, FSV sha256 `03096e466278f60631328207b07fe0aba6c6ffca622d21a468fa167ad33d730a`, report sha256 `4d0f8af83f5ab7ffdf6541979336c55d232dfa2fdd2c88e231cd4a7deb2a6ee4`, metrics sha256 `62c2774fc05658bb4529b608a48e2a181299057ad6125b87bf22c16151a32e30`. Result remains `no_promotion`: calibrated full constellation correlation is `null`, Brier is `0.7539461538`, best local baseline Brier is `0.0085552058`, and all ship-eligible component ablations are `demote`. This is not a gate mover.

**Real-input uncalibrated constellation ablation rerun (#537 / #393).** The #393 companion suite was rerun against the same #535 baseline while consuming only the real #396 training arrays: `/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-realinput-fsv/run-20260520T181900Z/`, FSV sha256 `c1ed1efc4aaceef210619fc290dbd09a2fa0c2732a6f3f87969237f1a8460e26`, report sha256 `d83eed24863d3ba8e7e0f62e4b7dd6bc081d103ed5a13ddf0846de613a5aeb4c`, metrics sha256 `05aa0855e5cccc2b188add0b69193f91cb0e0d5b3ec8fe36105d85685e690376`. Result remains `no_promotion`: best uncalibrated ship-eligible family is `per_slot_pairwise_temporal` with correlation `0.2876047260`, Brier `0.2220880757`, and delta `-0.5061223739` vs #535. This is not a gate mover and is farther below the #536 measurement of record.

**Real-input `lambda_inv` ablation rerun (#546 / #537).** The EK-012 ablation harness was replaced after discovering the old path used `EchoPredictor` + `CategoryMajorityOracle`. New aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-ek-012-bidirectional-ablation-realinput-fsv/run-20260520T172657Z/bidirectional_ablation_realinput_fsv.json`, report sha256 `bebb84c8b7800fd5567803a43025ee19a88182b32c77f8a4eeee4c9eb2dc2dd4`, readback sha256 `f9e24a09758a38e90c7579181dbf5956f28648ea7de86d4a1b53c517d43c5cbe`. Result: all variants regress against the #535 `0.7937270999` predecessor rank-lift baseline. Best rank-safe predictor-output variant is `lambda_inv=0.0` at `0.6704055071` (`-0.1233215928` vs #535); positive lambdas are worse (`0.25 → 0.5562775135`, `0.5 → 0.5049043894`, `1.0 → 0.5726367831`). Typed finding: `regression`; keep `lambda_inv=0.0` for ship-gate ordering.

---

## 2. Recent Closures (recent → older)

### 2.0 5-Sherlock parallel forensic audit + 2026-05-22 P0 closure wave (#629)

On 2026-05-22 a 5-Sherlock parallel forensic audit shipped ~40 P0 closures across all 22 active workspace crates, plus ~15 new findings. Closeout summary: `#629 (type:discovery)`.

**Sherlock scope partition:**

| Sherlock | Crates audited |
|---|---|
| #1 | `context-graph-mejepa` (predictor trunk, 155 src files) |
| #2 | `context-graph-mejepa-train` (UTML trainer, mistake loop, EWC/CBP, replay buffer) |
| #3 | `context-graph-mcp` + `context-graph-cli` + `context-graph-dynamicjepa` (216 MCP + 39 DynamicJEPA tools) |
| #4 | `mejepa-embedders` + `instruments` + `storage` + `corpus` + `tier-compression` + `mejepa-cf` + `core` + `embeddings` + `cuda` (data substrate) |
| #5 | `mejepa-tct` + `hygiene` + `shift-subscriber` + `witness` + `solver` + `mincut` + `paths` (verification primitives) |

**Fixes shipped in the audit (per `#629`):**

| Commit | Fix | Sherlock |
|---|---|---|
| `f1b37c57` | Tool-count drift (216 vs 214; 69 vs 67 mejepa tools) | #3 |
| `f1b37c57` | RFC 9110 §5.6.2 case-insensitivity in HTTP media-type parsing | #3 |
| `f1b37c57` | Clock corruption silently coerced to 0 in `current_unix_ms_i64()` | #3 |
| `5999b0f0` | UTF-8 lossy substitution in JSON-RPC stdio framing → fail-closed per RFC 8259 §8.1 | #3 |
| `5999b0f0` | Aiwonder source-lineage canonicalization in `swebench_lite_ops` | #4 |
| `28375702` | TCT freshness audit log writing to retired `./memory/journal/` → aiwonder durable root | #5 |
| `c02ba854` | `io::Error::other` clippy fix in `context-graph-witness/examples/cert_fsv.rs` | #5 |

**Silent-success / fake-FSV / leak bugs closed 2026-05-22:**

| Issue | Defect |
|---|---|
| #594 | MCP/CLI eval-run still persisted fixture EvalReports via indirect deterministic-compiler path |
| #595 | Operator status surfaces trusted raw `EvalReport.ship_gate_passed` instead of current gate predicate |
| #596 | Rust Trainer emitted pseudo-loss without training a predictor |
| #597 | Checkpointed-train echoed oracle labels into live predictions + EvalReports |
| #598 | Stop self-verify exited 0 when ME-JEPA RocksDB/CLI verification was unavailable |
| #599 | `store_memory` returned success after provenance/sidecar write failures |
| #600 | Auto-label materialization passed with missing or unverified slot evidence |
| #601 | ME-JEPA `dbPath` overrides could report fixture/local RocksDBs as source of truth |
| #603 | Provenance tools returned success with partial storage read failures |
| #604 | Secret-holdout guard reported `passes=true` while actual guard was blocked |
| #605 | Memory mutation tools reported success without complete readback/provenance |
| #606 | Synthetic stress eval used `ship_gate_passed`/exit-0 semantics for non-gate fixture success |
| #607 | `no_inner_actor_models` reported `readback_equal=true` without artifact readback |
| #608 | `optimizer_witness_chain_verify` returned valid success for absent `witness-chain.bin` |
| #609 | Oracle verdict CF contract split between lowercase corpus rows and canonical calibration rows |

**Data-substrate closure chain (2026-05-22):**

| Issue | Title |
|---|---|
| #150 | Python sample-efficiency / row-window power support (closed `support_policy=blocked`) |
| #372 | ContextGraph roots must be mounted on aiwonder ZFS pools (closed; `/zfs/archive` → `archive/contextgraph` ZFS, `/zfs/hot` → `hotpool/contextgraph` ZFS) |
| #496 | `soak.rs` doesn't actually soak (`soak.rs:541`) |
| #498 | `chaos.yml` looks in wrong crate for `storage_exhaustion` shim |
| #510 | CI gap — 17+ workspace crates have zero CI test coverage |
| #610 | Burn-in admissibility policy after Panel B and aiwonder root readiness (closed; `burn_in_start_allowed=false`) |
| #611 | Active point-metric repair for burn-in weak cells (closed; `repair_promoted=false`; `TASK_PY_G_134_NO_EXISTING_LEGAL_WEAK_CELL_REPAIR`) |
| #612 | Split-stable weak-cell evidence expansion for active point metrics |
| #613 | Canonicalize aiwonder AST source lineage + path-readback consumers |
| #614 | Complete-or-mask missing per-embedder forward-cache slots before 105-pair mining |
| #631 | Canonicalize or guard forward-cache packed-record lineage metadata |
| #632 | Forward-cache checkpoint/shard-plan retain legacy `/mnt/d` smoke-manifest metadata |
| #641 | Active embedder outputs, packed cache, quarantine semantics |
| #666 | P0 forward-cache embedder vector-space collapse before label discovery (closed; **led to #670 follow-up: regenerate `e2/e3/e4/e8`**) |
| #667 | Scrub geometry label root live-input policy for duplicate/degenerate spaces |
| #668 | P0 check in reproducible geometry-label atlas materializer |
| #669 | `swebench_lite_ops` CLI module invocation exited 0 without running |

**Per-cell calibrator diagnostics closed (TASK-PY-G-124..129):**

| Issue | Title |
|---|---|
| #584 | TASK-PY-G-124C per-cell named-pair selector before scorer wiring |
| #586 | TASK-PY-G-124E per-cell calibration substrate for named-pair retest |
| #587 | TASK-PY-G-126 per-cell axis-rotation calibrator diagnostic |
| #588 | TASK-PY-G-128 logit-space ECE diagnostic for abstain-tail calibration |
| #589 | TASK-PY-G-129 active-vs-abstain self-similarity audit |
| #590 | TASK-PY-G-127 entanglement-distance cluster conformal kernel |

**Geometry-label epic opened 2026-05-22 (successor to #413, in-progress):**

| Issue | Title | Status |
|---|---|---|
| #657 | TASK-PY-G-136 per-embedder vector-space label discovery bakeoff | in-progress p0 |
| #658 | TASK-PY-G-137 embedder-specific clustering contracts and metric audit | ✅ closed |
| #659 | TASK-PY-G-138 per-embedder cluster candidate materialization | ✅ closed |
| #660 | TASK-PY-G-139 cluster label naming and exemplar descriptors | ✅ closed |
| #661 | TASK-PY-G-140 cross-embedder label consensus and disagreement map | blocked behind #657 |
| #662 | TASK-PY-G-141 gate-safe promotion of geometry labels into predictor features | blocked behind #661 |
| #664 | Geometry region dictionary and per-embedder search router | blocked p1 |
| #665 | New prediction heads over per-embedder geometry labels | blocked p2 |

**Active P0 hardening sub-tasks (the #633 child queue, all `needs-triage`):** see `08_roadmap_and_open_work.md §0.4` for the 17 sub-tasks #634-#651.

### 2.1 Mistake-loop MCP tools (#523 / TASK-PY-G-112B — closed 2026-05-20)

`mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status` shipped.

FSV: `/zfs/archive/contextgraph/fsv/task-py-g-112b-mistake-loop-mcp-tools-fsv/run-20260520T113125Z/task_py_g_112b_mistake_loop_mcp_tools_fsv.json`, sha256 `9a9e50b6d6fdf8d3e4b64ab6edd992802121eb13b17663bc0f11786446411ae7`. Proves: all three tools registered; writes/reads land in RocksDB CFs; online head corrects same-panel mistake; lifecycle audit joins byte-readable; missing CFs fail closed; target-side oracle labels rejected as live inputs; invalid panel signatures leave no partial mistake/replay writes.

### 2.2 Failure-mode hierarchy (#404 / TASK-PY-G-110 — closed 2026-05-20)

8 Level-1 groups, 157 Level-2 skills, 379 Level-3 constellations, 3,169 Level-3 memberships, 536 named predictions, 12,658 total chunk-skill membership rows. Holdout: 114/114 actual Fail rows have FailLikely named-mode coverage; closest-exemplar oracle agreement 1.0 over 113/114 comparable holdout Fail rows. Post-fix active-prediction revalidation: 117/117 active predicted true-Fail rows have FailLikely named-mode coverage, while aggregate predicted-Fail coverage is 137/150 because 13 false-positive Fail verdicts have no named consequence explanation.

FSV: initial closure `/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779273783982/`, sha256 `06b3e46a9a703cf0d1c52991784726f1b1f5577df7d7488360ebc651a698b863`; post-fix revalidation `/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779376526126/task_py_g_110_failure_mode_hierarchy_fsv.json`, sha256 `77deb00b1d975bd1b83bf71a4bf48e2069eb1808202a6bf819da5e414f47d879`.

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
| AST chunk store | `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | Chunk metadata, stable row keys; active source paths canonicalized by #613 |
| AST chunk source files | `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/` | aiwonder-native source bytes for the 300x8 AST rows; 4,224 files / 148,346,301 bytes |
| Full packed embedder forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-<TS>/` | 391 completed shards; per-shard packed manifests; 12 active embedders |
| Forward-cache slot-completeness mask | `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl` | #614 output; 399,777 rows; prevents incomplete rows from being treated as complete all-pair constellations |
| Delete-test-call regeneration candidates | `/zfs/archive/contextgraph/state/regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1/delete_test_call_regeneration_candidates.jsonl` | #615 output; 1,131 deterministic candidates awaiting Docker oracle execution |
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

#383 says gate-active issue order must be evidence-weighted, not vision-weighted. **The "evidence" that drove the recent reordering was from synthetic-input ablations and is itself invalidated.** Re-rank from the `0.7942190170` real Panel A measurement; with #393/#398/#546 remeasured and not promoted, #536 only a small hard-negative weighting lift, and #547 passing disjointness, the top of the queue is #548 deeper mixed-cell ranking repair plus Panel B / anti-Goodhart validation.

### 5.4 The labeling layer is the meaning surface (#404 / #413 / #414 / #418) — substrate stands

Raw slot violations + cross-embedder correlations only become actionable when clustered into a 4-level hierarchy of named failure modes grounded by closest historical exemplars. The 491-row accepted Level-0 registry + 128 Level-2 skills + 379 Level-3 constellations + 36,773 chunk-skill memberships are all derived from the real forward cache and survive the synthetic-vector bug. Post-fix revalidation proves the hierarchy still covers true failures (117/117 active predicted true-Fail rows) but also exposes 13 false-positive Fail verdicts with no named consequence explanation. That is evidence for predictor-policy repair, not blanket skill expansion.

### 5.5 The mistake loop replaces best-of-K (#406)

Best-of-K papers over an under-trained predictor by spending compute at inference time. The online mistake loop pays the cost at *learning* time and converges to a predictor that is right on the first try. Current shipped primitive: key-local exact-panel online-head state with `protection_mode=exactPanelKeyIsolation`. Convergence target: `mistake_repeat_rate ≤ 0.05`. The machinery is shipped; needs real predictor mistakes from a real trained checkpoint to validate convergence.

### 5.6 Silent-success bugs were everywhere (2026-05-22 5-Sherlock audit)

The 5-Sherlock audit's most important finding: **return values lie**. Across 15 distinct surfaces (memory mutation, MCP eval-run, store_memory, optimizer witness verify, stop self-verify, oracle verdict CFs, etc. — see §2.0), code paths returned `success`/`passed=true`/exit-0 without actually performing the work or after partial write failures. This is the universal counter-evidence for the FSV doctrine: *"a return value is a claim, the source of truth is the verdict."* The fixes shipped; the operational implication (#655) is that auto-FSV harnesses also tend toward silent-success and must be retired in favour of manual SoT readback as the ship-gate evidence form.

### 5.7 Forward-cache embedder collapse (#666 → #670)

The Sherlock audit's substrate-side finding: **the forward cache contains invalid vector spaces in `e2`, `e3`, `e4`, `e8`**. #666 closed by acknowledging the collapse; #670 is the follow-up to regenerate-or-retire those spaces. Until #670 closes, any predictor evidence riding those embedders is suspect. The #657 per-embedder geometry-label bakeoff is the response: discover what each embedder's vector space actually contains *before* claiming it provides predictor signal.

### 5.8 Connectome topology features are diagnostic-only (#513 / #527)

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
| Post-#547 mixed-cell diagnostic (#548) | `post-547-mixed-cell-ranking-repair-fsv/run-20260520T184458Z/` | `acd57aba…` |
| #549 mixed-cell signal fusion diagnostic | `task-py-g-549-mixed-cell-signal-fusion-fsv/run-20260520T191159Z/` | `b6b391e…` |
| #392 aggregate temporal trajectory diagnostic | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T192449Z/` | `a7d763d…` |
| #392 file/skill temporal trajectory diagnostic | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T195910Z/` | `e686fd1…` |
| #392 out-of-fold probability calibration diagnostic | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T201911Z/` | `dbf92087…` |
| #392 affine file/skill calibration diagnostic | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T203036Z/` | `289e050b…` |
| #392 hardened affine/blend diagnostic | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T205039Z/` | `a197f49…` |
| #392 logit calibration diagnostic | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T205929Z/` | `6c04068…` |
| #392 mean-bias + support-aware coverage hardening diagnostic | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T214049Z/` | `8e3644d…` |
| #554 usefulness-weighted ordered skill diagnostic | `task-skill-002-usefulness-weighted-392-fsv/run-20260520T232838Z/` | `92d9f98…` |
| #558 full-corpus targeted skill rematerialization | `task-skill-006-full-corpus-skill-rematerialization-fsv/run-1779320263984/` | `27d2471…` |
| #558 #392 rerun from rematerialized skills | `task-skill-006-full-corpus-skill-rematerialization-fsv/run-20260520T233926Z/` | `2c3c7ce…` |
| #560 split-legal ECE repair diagnostic | `task-skill-008-split-legal-ece-repair-fsv/run-20260521T004021Z/` | `c2505ee…` |
| #557 skill usefulness audit surface | `task-skill-005-skill-usefulness-audit-surface-fsv/run-20260521T023736Z/` | `5a70160…` |
| #551 coverage-gap closeout materialization | `task-skill-009-targeted-skill-coverage-repair-fsv-default128-final/run-1779333467960/` | `ba2446b…` |
| #392 corrected skill-kind evidence rerun | `task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` | `9ed8830…` |
| Ship-gate row-trace rerun for #549 | `swebench-300x8-ship-gate-fsv/run-20260520T185945Z/` | `781aa3f6…` |
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
