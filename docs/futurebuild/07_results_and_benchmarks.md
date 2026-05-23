# 07 — Results, Benchmarks, and FSV Evidence History

**Source:** aiwonder `/zfs/archive/contextgraph/fsv/` for production evidence; git log; GitHub issue comments; per-issue RESOLVED comments.

**Purpose:** preserve proof-of-work that future agents would otherwise lose. Every entry references a physical artifact (FSV JSON, sha256, CF) that can be re-verified.

---

## 0a. ⚠ 2026-05-23 3-Sherlock audit — measurement decomposed

The `0.7942190170` rank-lift number is a composite Goodhart-magnifier, not a predictor evaluation. 3-Sherlock parallel forensic audit (reports `docs/SHERLOCK_{PREDICTOR,SUBSTRATE,INFRA}_FINDINGS_2026-05-22.md`) decomposes it:

| Layer | Defect | Issue |
|---|---|---|
| Predictor head | Public `Trainer::run_full_training` hard-coded `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE` at `mejepa-train/src/trainer/mod.rs:31-34, 409-422`; latest aiwonder FSV loaded an out-of-band task-py-g-127 checkpoint (`optimizer_steps=64`, `weight_delta_observed=true`) but that path is not countable through `trained_predictor` / `ship_gate_countable_training`, and compiler construction must fail closed without an explicit manifest | #683 |
| Substrate (`e2/e3/e4`) | `chunk.time` wall-clock injection at `python_forward_cache_fsv.rs:2308` identical for all 399,777 chunks → constant sinusoidal-PE output (1 unique vector per slot) | #704, #708 |
| Substrate (`e8`) | `GraphModel::embed_batch()` missing `project_source_vector` before commit `ff4d4c03` (2026-05-22); prod corpus generated 7d earlier → byte-identical to `e1` | #704, #670 |
| Gate metric | Rank-lift = category-local RBF exemplar similarity fitted on calibration split; lifts raw `0.4843 → 0.7942` without improving Brier/ECE; measures exemplar similarity, not predictor | #695 |
| Active-subset diagnostic | `python_calibration_active_gate.py:322-378` selects per-cell `coverage` + `margin_threshold` to maximize gate pass; "active" = high `\|score-0.5\|` = easy rows; `0.953` corr is selection bias | #692 |

**Honest gap:** `0.95 − 0.4843 = 0.4657` against raw Pearson.

**29 new issues filed (#683-#711).** Critical-path ordering replaced by #712 (pinned context). Every #392 / #511 / #549 / #554 / #558 / #560 / #393 / #398 / #536 measurement in §1-§2 is a perturbation of (checkpoint-loaded but trainer-orphaned predictor) ∘ (4/12 dead slots) ∘ (rank-lift Goodhart). Five #392 trajectory repairs hit ECE walls because of these defects, not proven calibration limits.

## 0b. ⚠ 2026-05-22 hardening pivot — still active

5-Sherlock parallel forensic audit (#629 closeout) + **#633 manual full-system FSV + hardening gate** opening. Manual SoT contract (`06 §1.9`) is binding for closure evidence; per #655, auto-FSV `*_fsv.rs` harnesses are being retired.

P0 audit-uncovered defects (carried into 2026-05-23 critical path):

| Issue | Defect |
|---|---|
| #618 | Aggregate OOD over flat 5,120-byte buffer (doctrine §1.5ter) |
| #619 | `forward_inverse_inner` flat `Linear(PANEL_DIM, hidden)` projections (doctrine §1.5ter) |
| #621 | Hardcoded `gtau_pass_rate: 1.0` to weekly dashboard |
| #670 | Invalid forward-cache spaces `e2/e3/e4/e8` |

---

## 0. ⚠ CRITICAL: pre-fix evidence partially invalidated

Between Phase G ramp and 2026-05-19 22:45 UTC, inference path used SHA256-seeded synthetic vectors (`crates/context-graph-mejepa/src/compiler.rs::vectors_for` calling `deterministic_vector(slot, &digest)`) instead of real `E*Instrument` encoders. **The 0.866667 ship-gate measurement and every ablation comparing against it are INVALID** — they measured a synthetic-input pipeline, not the real predictor. See `01 §1.7`.

| Status | Evidence |
|---|---|
| **REAL (no rebaseline)** | AST chunks, 391-shard forward cache, #388 tensors, #389 calibration, #413 labels, #414 skills, #404 hierarchy, anti-compensation invariants |
| **INVALID (must rebaseline)** | Ship-gate 0.866667; `λ_inv` ablation; #393 / #398 ablations; "labels are the bottleneck" claim; "slot-preserving doesn't close gap" claim |

The corpus survives. The pre-fix measurement does not. Post-fix real-input rebaseline now exists, but is itself **provisional** (see §0a) until manually re-verified.

---

## 1. Current Ship-Gate Status

| Metric | Target | Latest measurement | Status |
|---|---|---|---|
| `prediction_oracle_correlation` (raw Pearson) | ≥ 0.95 stable × 4 rolling windows × all Python mutation cells | **`0.4843192995`** | **Honest model-output baseline. Gap = `0.4657`.** Training/checkpoint lineage not countable through public trainer (#683); 4/12 embedder slots dead (#670, #704); rank-lift composite metric not gate-eligible (#695). |
| `prediction_oracle_correlation` (rank-safe) | (diagnostic only) | `0.6916886568` | Monotonic recalibration. |
| `prediction_oracle_correlation` (rank-lift) | (NOT gate-eligible per #695) | `0.7942190170` | Category-local RBF exemplar similarity over calibration split. Lifts raw `+0.31` without improving Brier/ECE. Measures exemplar similarity, not predictor. #535/#536 lift came from this transform, not from the model. |
| Cross-panel oracle correlation (Panel B) | ≥ 0.95 with `\|corr_a − corr_b\| ≤ 0.05` | Panel B active-surface parity on #392 205-row subset: `corr_A=0.9526342154`, `corr_B=0.9902423024`, `\|Δ\|=0.0376080871` | Comparable #405/#521 only on active subset (selection bias #692). Full Panel B blocked behind #683 + #670. |
| `mistake_repeat_rate` | ≤ 0.05 (rolling 100-mistake window) | byte-readable via `mejepa_mistake_loop_status` | Mistake-loop LUT inert because `protection_mode=ExactPanelKeyIsolation` keyed on exact panel-signature hash (#689). Production panels almost never collide. |

**Honest gap to ship:** `0.4657` against raw Pearson. The previously-claimed `0.1558` gap was against rank-lift, which is not the predictor. Pre-fix `0.866667` (synthetic-vector evidence) remains invalid.

**Metric contract update (#532).** Per-cell Pearson not defined for zero-variance oracle cells. Active ship-gate FSV classifies every block as `mixed`, `all_fail`, `all_pass`, or `empty`: mixed → Pearson; degenerate → target-class recall + calibration (`actual_class_confidence_mean`, Brier, ECE). Each block carries statistical-power diagnostics.

**Calibration + invalid-source + rank-safety + rank-lift update (#533/#531/#534/#535/#536).** Ship-gate FSV fits deterministic calibration/rank-lift policies on 240-row calibration split only, then evaluates 240-row holdout. Constants allowed only for structurally degenerate `compile_error` / `wrong_file` cells. Rank-reversing transforms only when calibration has mixed-class support ≥ 8 per class + raw Pearson ≤ -0.25. Rank-lift uses category-local RBF exemplar scoring over real `code_state_constellations.jsonl` slot-feature means + hashed live label counts while excluding target-side oracle/docker/test labels; #536 added calibration-selected train-only hard-negative weighting (`tau=20.0`, `fail_boost=1.5`).

Latest ship-gate FSV: `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`, report sha256 `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499`, readback sha256 `9cf352a775a38df87f17e6b1b0964d0ec93b93de9d5447b6bc762294c1b67ae2`. Row-trace FSV for #549/#392: `run-20260520T185945Z/`, report sha256 `781aa3f6527abed2add930a8a19f9eb790fdbe3c86b81f27006ae4ca48ca2506`. `compile_error` uses raw invalid-source diagnostic panels. #535 lifted overall calibrated correlation `0.6916886568 → 0.7937270999`; #536 lifted to `0.7942190170`, Brier `0.0923729688`, ECE `0.0240316074`. #536 improved one-fail cell Pearson definition/Brier, `swap_variable` Pearson/Brier, `subtle_flip` Brier, `off_by_one` fail recall; threshold behavior still saturates several weak cells.

### 1.1 Post-#536 evidence chain (closed unless noted)

| Ref | FSV | Key result |
|---|---|---|
| **#547** post-#536 calibration-disjointness revalidation | `fsv/post-536-calibration-disjointness-fsv/run-20260520T181254Z/` — report sha256 `d90152ebd41b5e626ca8b9882dbba185d711f13aa69048e7e627fa854d4b27e8`, readback sha256 `2d58c9e3dd0c70d08202656f85596058287c77eb9a48aa1bb15010d89d19f0c7` | `all_passed=true`: train/calibration/holdout row-key overlaps all zero; live feature-member overlaps all zero; 3,000 code-state rows map to 2,400 unique corpus keys; #413 label-lineage hashes match; selected `tau=20.0, fail_boost=1.5` is calibration-selected. |
| **#548** post-#547 mixed-cell repair diagnostic | `fsv/post-547-mixed-cell-ranking-repair-fsv/run-20260520T184458Z/` — report sha256 `acd57aba489a73816b114af0440ae06e07f2e195a1d1f91c0003261220655551`, readback sha256 `a85d42eda42e76e4e5ce787345f934f1977e42609ba5de597a1f1cab6e8e9009` | `all_passed=true`, typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED`. Reproduces #536 exactly. Legal calibration leaves `off_by_one`, `subtle_flip`, `swap_variable` threshold-degenerate. Forbidden holdout-selected counterfactual would lift correlation to `0.8076422215`. |
| **#549** mixed-cell signal fusion | `fsv/task-py-g-549-mixed-cell-signal-fusion-fsv/run-20260520T191159Z/` — report sha256 `b6b391e97670e1cf493f009455eb29143cc4636c540fea9250c85837ead090c1`, readback sha256 `f7c6d085fe1be4f85b70c1222a639d1d1dd57c42774867d2f2a01f91e1ab818c` | Repo/touched-path metadata useful (`0.7974180579`, Brier `0.0911292881`, ECE `0.0156852305`) but insufficient. 1,037-candidate score-fusion sweep best legal holdout corr `0.7945368886`. Typed blocker `MIXED_CELL_SIGNAL_FUSION_INSUFFICIENT`. |

### 1.2 #392 trajectory chain (ECE-blocked)

All under `fsv/task-py-g-392-temporal-trajectory-features-fsv/` unless noted. All `all_passed=true`, `promotable_candidate_found=false`.

| Slice | Run | Best holdout corr | Brier | ECE | Notes |
|---|---|---|---|---|---|
| 1st aggregate trajectories | `run-20260520T192449Z/` (sha256 `a7d763d72c11cafeabc6f1d53728cced2d97c07430b78a707b1107efd64136e3`) | `0.8008320332` | `0.0897067040` | `0.0253749657` (regresses) | First post-#536 legal surface above 0.80; `TEMPORAL_TRAJECTORY_FIRST_SLICE_INSUFFICIENT` |
| 2nd file-path + skill/ability | `run-20260520T195910Z/` (sha256 `e686fd1387458814ec150939f58188522743d7c5fa968567643bf43d1dd014a1`) | `0.8422425985` | `0.0726622641` | `0.0386186689` | `+0.0480235815` corr vs #536; `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED` |
| 3rd out-of-fold probability | `run-20260520T201911Z/` (sha256 `dbf9208774b714db1f72e113a4b4a3845cdd807bbd755a57e5edd3c41bd645a3`) | `0.8157011867` | `0.0843216032` | `0.0556452833` | Sparse bin calibration not the gate mover |
| 4th affine file/skill | `run-20260520T203036Z/` (sha256 `289e050be36ddd81672f1f76f01f9dd6f55f22e801d8ebe603efcf637c58d09e`) | `0.8497224450` | `0.0698330477` | `0.0314629300` | Selected affine true-mixed calibration (k=31, scale=1.30, offset=0.02); accuracy `0.9083333611`, pass recall `0.8916666508`, fail recall `0.9250000119` |
| 5th hardened affine/blend | `run-20260520T205039Z/` (sha256 `a197f49ebdaefa4d00366d464f4d085033de13b67276254a46a328872bde60ae`) | `0.8484774828` (blend) | `0.0703259036` | `0.0312432162` | Best blend (`affine 0.95, rank_lift 0.05`) loses to affine baseline |
| 6th logit calibration | `run-20260520T205929Z/` (sha256 `6c040688cf5e52428e69dffe0270e1605c3158d8da6403bbca1abae45239f7af`) | `0.8488159180` | `0.0703388974` | `0.0342222452` | Logit ruled out — worsens ECE |
| 7th/9th mean-bias + support-aware coverage | `run-20260520T214049Z/` (sha256 `8e3644db8b0a896632741d18d871d7affc2f98a0238785009833c608a5adeac1`, readback `40b9c60998f0bc1cdc61861dc97d6f9cca9c2a88038ffa4e54f183b58d26edf9`) | `0.8497869968` | `0.0697008073` | `0.0299093761` | Selected `calibration_selected_mean_bias_calibration`. Skill coverage explicit: `1531/3000` covered, `1469` uncovered. Support-aware shrinkage slightly worse on calibration objective. |

### 1.3 #554/#557/#558/#560/#551/#392 evidence chain (2026-05-21 correction)

| Ref | FSV | Result |
|---|---|---|
| **#554** usefulness-weighted ordered skill diagnostic | `fsv/task-skill-002-usefulness-weighted-392-fsv/run-20260520T232838Z/` — sha256 `92d9f983c32f50cb62cb217ddbe95a7c2da3cfabf243199e194ea000f2fff0a8`, readback `2950ae4df088924397b2c39aa438e1766f7711575617a2524020034d5c99ede1` | Combined: corr `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`. Failure-only stronger (corr `0.8830994964`, Brier `0.0554981381`, ECE `0.0643141195`). Both ECE-regress, rejected; raw surface kept active. |
| **#558** TASK-SKILL-006 full-corpus skill rematerialization + #392 rerun | Materialization: `fsv/task-skill-006-full-corpus-skill-rematerialization-fsv/run-1779320263984/` — sha256 `27d24719ff2f3c310f1a757d9f5bf142be712c9deae0049b7cc83439cd66d6a0`. Rerun: `run-20260520T233926Z/` — sha256 `2c3c7cea2ced0afc4715bad03624401ea497cfee1260d62ce7f305f5fb11828a`, readback `07dbede6b8cd844060c94e5c31b281f748fcd08998ef5668a8d9ac9070b01829` | `399777` chunks → `398215` episodes → `157` skills, `9489` memberships, `4364` reverse, `157` lifecycle audit. Selected `usefulness_weighted_combined`: corr `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`, accuracy `0.9083333611`. ECE-blocked. |
| **#560** TASK-SKILL-008 split-legal ECE repair | `fsv/task-skill-008-split-legal-ece-repair-fsv/run-20260521T004021Z/` — sha256 `c2505eeb6f755d362510b61d78437f364029deb000363d54715ea3238eec6eee`, readback `3b401c081af4cf415f8270ae07752b0a8aa7d481eae95927cecf69040fa` | ECE-focused mean-bias selects same model. Holdout ECE unchanged `0.0396374688`; support-shrink slightly lower `0.0396139733` not promotable. Typed blocker points at per-cell/per-constellation calibration + consequence-to-cause traces. |
| **#557** TASK-SKILL-005 skill usefulness audit surface | `fsv/task-skill-005-skill-usefulness-audit-surface-fsv/run-20260521T023736Z/` — sha256 `5a701604deaa7cff8e086b5f4ed36e2a8dfc7bfe1fe3b4b4f877b7860b610036` | `passes=true`. Direct `ldb`: `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS=6`, `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP=2258`, `CF_MEJEPA_SKILL_REVERSE_INDEX=1129`, `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT=7`. Audit surface exposes kind, support/fail-pass/lift, calibration-impact, per-cell contribution, live vs target labels, source-byte sequences, lifecycle, uncovered code-state keys, unknown constellations, rejected/demoted. |
| **#551** TASK-PY-G skill coverage gap closeout + corrected #392 | Materialization: `fsv/task-skill-009-targeted-skill-coverage-repair-fsv-default128-final/run-1779333467960/` — sha256 `ba2446b603edd648a64eb327e9375a8539c251393949c1acec24f9b1acb4b20b`. Direct `ldb`: `LEVEL2_SKILLS=155`, `CHUNK_SKILL_MEMBERSHIP=9489`, `SKILL_REVERSE_INDEX=4364`, `SKILL_LIFECYCLE_AUDIT=155`. Corrected #392 rerun: `fsv/task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` — sha256 `9ed8830f184e0aca07a1ee47897b31b342dbdaf2db0bacc28d0a08981dfbc29c`, readback `9d8f73a794a98b84ed7a4d89e7c9c3037b94b0e5bf27fe0d3c9f94a8e436a9c3` | `1469/3000` uncovered code states are pass/context-only; pass/context expansion regresses #392 → not the repair. Legal best `calibration_selected_mean_bias_calibration`: corr `0.8770877719`, Brier `0.0581485741`, ECE `0.0475411229`, accuracy `0.9250000119`. |
| **#392 evidence-semantics hardening** | `fsv/task-py-g-392-evidence-semantics-hardening-fsv-release/run-20260521T074421Z/` — sha256 `edf4755f44ccb31a3eaafc5ad07f842e832e9cd684b5838cde53eba8715f6366`, readback `4cddf2db7c0732e3b33164b7bbfe0207e2b9dc3ecde7e3f3233c004014af8655` | Standardizes cause-residual features; cause-bin application real (`62/90` calibration, `46/90` holdout). Legal best `calibration_selected_cause_bin_calibration`: corr `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`, pass recall `0.9333333373`, fail recall `0.9416666627`. Blocker `aggregate_ece_regressed: 0.040213395`. |
| **#392 final-bin diagnostic** | `fsv/task-py-g-392-final-bin-guarded-fsv/run-20260521T090040Z/` — sha256 `4d3cefc3c7484854978da56fb4da0a031c4061505ebca9a6b8ac5773ee52bc87`, readback `aa9de48e9e573a1db39a69e78785bdab2ada2c88ec2449943b7e37ed8ebc4c26` | Final-bin (global, `bin_count=5`, `smoothing=3.0`, `raw_weight=0.95`): corr `0.8941474557`, Brier `0.0503885597`, ECE `0.0432727709`. Worsens ECE, `selection_forbidden=true`. Cause-bin remains selected. |

### 1.4 Burn-in admissibility + active-point repair chain (2026-05-22)

| Ref | FSV | Result |
|---|---|---|
| **#610** burn-in admissibility | `fsv/task-py-g-133-burn-in-admissibility-policy-fsv/run-20260522T123721Z/` — sha256 `d766c06d8da35f5a1e55cfbc7981b464ce65a595d910ca16bfbc15c7e465658c` | `all_passed=true`, `burn_in_start_allowed=false`, `TASK_PY_G_133_BURN_IN_POWER_NOT_ADMISSIBLE` |
| **#150** sample-efficiency power | `fsv/task-py-g-019-sample-efficiency-power-fsv/run-20260522T124456Z/` — sha256 `585c54502084c6e96037d1c49df6b901a48822284a8c839c76c9542fe8513019` | `support_policy=blocked`; longer windows or row support alone cannot start burn-in while three active point metrics remain sub-target |
| **#611** active-point metric repair | `fsv/task-py-g-134-active-point-metric-repair-fsv/run-20260522T130250Z/` — sha256 `ccea46d6ac5677cfc7a9b849c9dd70322570ae281ba6a7b8a23d76e6d772704f`, readback `bf1c67b6186d6dfa7dedf21744d6d87c960523a8ff616b356862cf0353d636fc` | `all_passed=true`, `repair_promoted=false`, `TASK_PY_G_134_NO_EXISTING_LEGAL_WEAK_CELL_REPAIR`. Weak cells: `delete_test_call` (corr `0.6948082447`), `off_by_one` (`0.8675352335`), `subtle_flip` (`0.8035093546`). |

### 1.5 Data-substrate lineage audit chain (#612 → #613 → #614 → #615, 2026-05-22)

| Ref | FSV | Result |
|---|---|---|
| **#612** TASK-PY-G-135 data-substrate lineage audit | `fsv/task-py-g-135-weak-cell-evidence-expansion-fsv/run-20260522T133526Z/` — sha256 `5ff5ab01f4c1fba1d1286dded66c2d3941d01021717257fea0c9cf950ea609ed` | Typed blockers: `LEGACY_ABSOLUTE_PATHS_IN_AST_CHUNKS`, `AI_WONDER_CANNOT_DEREFERENCE_RETAINED_ABSOLUTE_PATHS`, `LEGACY_ABSOLUTE_PATHS_IN_FORWARD_PACKED_RECORDS`, `INCOMPLETE_PER_EMBEDDER_SLOT_COVERAGE`, `DELETE_TEST_CALL_MISSING_EMBEDDER_SIGNAL`. AST: `399777/399777` rows had legacy `/mnt/d`. Forward cache: `4665772/4797324` chunk/embedder pairs, `131552` quarantined. Slot coverage zero-slot counts: `delete_test_call=95`, `off_by_one=70`, `subtle_flip=69`. `delete_test_call` split support not learnable (`238 pass / 2 fail` train, `30 pass / 0 fail` calibration, `29 pass / 1 fail` holdout). |
| **#613** TASK-PY-G-135A aiwonder AST source-lineage canonicalization | `fsv/task-py-g-135a-aiwonder-source-lineage-canonicalization-fsv/run-20260522T140211Z/` — sha256 `e518bdacb20281b6907bf119fae4b506298bebb4f5ed9b5c86f6299efc7bf13a`, readback `f0e05b6191fd014d37186fafc757938856cc373341658224b54dfe3d24e7315c`, canonical AST sha256 `bc13e46a6262209b617a624f451e05e1f37e9dddd0d97b624538d7aefb9d0234` | `all_passed=true`, `promoted=true`. Source tree materialized: `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/` — 4,224 files, 148,346,301 bytes. Active rows now `0` `/mnt/d` or `/mnt/c` in `absolute_path`/`dataschema`; legacy values retained only as `legacy_*` provenance. Fail-closed source readback shipped to `python_ast_chunks.py`, `python_static_graph_core.py`, `python_static_graph.py`, `smoke_manifest.py`. |
| **#614** TASK-PY-G-135B forward-cache slot-completion mask | `fsv/task-py-g-135b-forward-cache-slot-completion-fsv/run-20260522T141655Z/` — sha256 `8da6c7a213aa3b442b4909e5fae4ec170b827022c819132d0f5ab38ab0334092`, readback `6f9b74927b08333bdc46688ca405e1999a535344a35b10f4876266a7a301e17f`, mask sha256 `b298087f960cea25fde4420653ffb16943bcf92708a2564d9b17077634d2b70c` | `all_passed=true`, `legal_missingness_mask_promoted=true`, `slot_completion_promoted=false`. `0` mounted active path hits. `4665772` present chunk/embedder rows; `131552` quarantined (`124019` `MEJEPA_FORWARD_TOKEN_TOO_LONG`, `7524` `MEJEPA_FORWARD_INPUT_TOO_LARGE`, `9` GPU-OOM); `0` unaccounted. Mask at `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl`: 399,777 rows (378,444 full 12-slot + 20,706 partial + 627 zero-slot). Active pair universe is 66 pairs per complete 12-slot row; 15-slot panel universe is 105 — forward-cache-only data NOT allowed to claim complete 105-pair constellations. |
| **#615** TASK-PY-G-135C delete-test-call regeneration plan | `fsv/task-py-g-135c-delete-test-call-regeneration-fsv/run-20260522T143109Z/` — sha256 `c1431c11bd249b0bb51f24d1349080a678a69543b2430afcf1a2da397460d918`, readback `d07c1d5006988bd81ef709d98ac7f855f66d3516dae79e3f4ee16822baf68f40`, plan sha256 `47997f6b7f0582166ca6a008fc760fddcfe6fca97e0ce342379adc8d9034b324` | `all_passed=true`, `regeneration_plan_materialized=true`, `regeneration_complete=false`, `TASK_PY_G_135C_DELETE_TEST_CALL_ZERO_CALIBRATION_FAILURE_SUPPORT`. Only observed failure-bearing sublabel `production_plain_assert_deleted`: 2 train fail, 1 holdout fail, 0 calibration fail. Plan enumerates 1,131 deterministic candidates: 231 `production_plain_assert_deleted`, 89 `test_self_assert_deleted`, 50 `pytest_raises_deleted`, 761 `other_assertion_like_deleted`. Stable: 26 calibration production-plain + 2 from currently failure-bearing files in calibration. Next: aiwonder Docker oracle batch over plan, rebuild AST → #614 mask → labels → #392 → #611. |

### 1.6 Real-input ablation reruns (#537/#398/#393/#546)

| Ref | FSV | Result |
|---|---|---|
| **#537/#398** calibrated constellation rerun | `fsv/task-py-g-097-constellation-ablation-with-calibration-realinput-fsv/run-20260520T170156Z/` — FSV sha256 `03096e466278f60631328207b07fe0aba6c6ffca622d21a468fa167ad33d730a`, report sha256 `4d0f8af83f5ab7ffdf6541979336c55d232dfa2fdd2c88e231cd4a7deb2a6ee4`, metrics sha256 `62c2774fc05658bb4529b608a48e2a181299057ad6125b87bf22c16151a32e30` | `no_promotion`: full constellation corr `null`, Brier `0.7539461538`, best local baseline Brier `0.0085552058`. Not a gate mover. |
| **#537/#393** uncalibrated constellation rerun | `fsv/task-py-g-093-constellation-ablation-suite-realinput-fsv/run-20260520T181900Z/` — FSV sha256 `c1ed1efc4aaceef210619fc290dbd09a2fa0c2732a6f3f87969237f1a8460e26`, report sha256 `d83eed24863d3ba8e7e0f62e4b7dd6bc081d103ed5a13ddf0846de613a5aeb4c`, metrics sha256 `05aa0855e5cccc2b188add0b69193f91cb0e0d5b3ec8fe36105d85685e690376` | `no_promotion`: best `per_slot_pairwise_temporal` corr `0.2876047260`, Brier `0.2220880757`, delta `-0.5061223739` vs #535. |
| **#546/#537** `λ_inv` bidirectional ablation | `fsv/task-ek-012-bidirectional-ablation-realinput-fsv/run-20260520T172657Z/` — sha256 `bebb84c8b7800fd5567803a43025ee19a88182b32c77f8a4eeee4c9eb2dc2dd4`, readback `f9e24a09758a38e90c7579181dbf5956f28648ea7de86d4a1b53c517d43c5cbe` | `regression`. Best `lambda_inv=0.0` at `0.6704055071`; positive lambdas worse (`0.25 → 0.5562775135`, `0.5 → 0.5049043894`, `1.0 → 0.5726367831`). Keep `lambda_inv=0.0`. |

---

## 2. Recent Closures (recent → older)

### 2.0 5-Sherlock parallel forensic audit + 2026-05-22 P0 closure wave (#629)

2026-05-22: 5-Sherlock parallel forensic audit shipped ~40 P0 closures across all 22 active workspace crates + ~15 new findings. Closeout: `#629 (type:discovery)`.

**Sherlock scope partition:**

| Sherlock | Crates audited |
|---|---|
| #1 | `context-graph-mejepa` (predictor trunk, 155 src files) |
| #2 | `context-graph-mejepa-train` (UTML trainer, mistake loop, EWC/CBP, replay buffer) |
| #3 | `context-graph-mcp` + `context-graph-cli` + `context-graph-dynamicjepa` (216 MCP + 39 DynamicJEPA tools) |
| #4 | `mejepa-embedders` + `instruments` + `storage` + `corpus` + `tier-compression` + `mejepa-cf` + `core` + `embeddings` + `cuda` (data substrate) |
| #5 | `mejepa-tct` + `hygiene` + `shift-subscriber` + `witness` + `solver` + `mincut` + `paths` (verification primitives) |

**Fixes shipped:**

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
| #594 | MCP/CLI eval-run persisted fixture EvalReports via indirect deterministic-compiler path |
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
| #611 | Active point-metric repair for burn-in weak cells (closed; `repair_promoted=false`) |
| #612 | Split-stable weak-cell evidence expansion for active point metrics |
| #613 | Canonicalize aiwonder AST source lineage + path-readback consumers |
| #614 | Complete-or-mask missing per-embedder forward-cache slots before 105-pair mining |
| #631 | Canonicalize or guard forward-cache packed-record lineage metadata |
| #632 | Forward-cache checkpoint/shard-plan retain legacy `/mnt/d` smoke-manifest metadata |
| #641 | Active embedder outputs, packed cache, quarantine semantics |
| #666 | P0 forward-cache embedder vector-space collapse before label discovery (closed; **led to #670: regenerate `e2/e3/e4/e8`**) |
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

**Active P0 hardening sub-tasks (#633 child queue, all `needs-triage`):** see `08 §0.4` for #634-#651.

### 2.1 Mistake-loop MCP tools (#523 / TASK-PY-G-112B — closed 2026-05-20)

`mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status` shipped.

FSV: `fsv/task-py-g-112b-mistake-loop-mcp-tools-fsv/run-20260520T113125Z/`, sha256 `9a9e50b6d6fdf8d3e4b64ab6edd992802121eb13b17663bc0f11786446411ae7`. Proves: all three tools registered; writes/reads land in RocksDB CFs; online head corrects same-panel mistake; lifecycle audit joins byte-readable; missing CFs fail closed; target-side oracle labels rejected as live inputs; invalid panel signatures leave no partial mistake/replay writes.

### 2.2 Failure-mode hierarchy (#404 / TASK-PY-G-110 — closed 2026-05-20)

8 Level-1, 157 Level-2, 379 Level-3, 3,169 Level-3 memberships, 536 named predictions, 12,658 total chunk-skill membership rows. Holdout: 114/114 actual Fail rows have FailLikely; closest-exemplar oracle agreement 1.0 over 113/114. Post-fix active: 117/117 active true-Fail rows have FailLikely; aggregate predicted-Fail coverage 137/150 — 13 false-positives have no named consequence explanation.

FSV: initial `fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv/run-1779273783982/`, sha256 `06b3e46a9a703cf0d1c52991784726f1b1f5577df7d7488360ebc651a698b863`; post-fix `run-1779376526126/`, sha256 `77deb00b1d975bd1b83bf71a4bf48e2069eb1808202a6bf819da5e414f47d879`.

### 2.3 Online mistake-driven loop (#406 / TASK-PY-G-112 — closed 2026-05-20)

Key-local exact-panel online-head state. `protection_mode=exactPanelKeyIsolation`, `claims_fisher_ewc_protection=false`. Convergence verified: inject wrong prediction → second presentation yields corrected verdict. Unrelated-control delta ±0.02.

### 2.4 Native active-embedder constellation adapter (#452 / TASK-PY-G-128 — closed 2026-05-20)

Preserves per-embedder vector spaces + ragged coverage masks instead of flattening. Unblocks #441 trained checkpoint export FSV.

### 2.5 Auto-label discovery + skill substrate (#413 / #414 — closed 2026-05-19)

**#413 / TASK-PY-G-117:** deterministic Level-0 label compiler (`python_auto_label_discovery.py`, 1,641 lines, no inner LLM). Full-corpus aiwonder FSV PASS: 399,777 chunk labels, 491 accepted label registry, 3,000 code-state constellations, 4,224 file-path constellations, 1,139 failure evidence sets, 549 usefulness metrics. FSV manifest sha256 `3cd0e3c6…`.

**#414 / TASK-PY-G-118:** 128 Level-2 skills, 36,773 chunk-skill memberships, 10,579 reverse-index rows, 128 lifecycle audit. FSV: `fsv/task-py-g-118-skill-sequence-discovery-fsv/run-1779213075188/`, sha256 `0df1437ae5e0542b356777b29df700b521b83b054113697ca280ae3d242d3dee`.

### 2.6 Runtime hierarchical ability resolver (#418 / TASK-PY-G-121 — closed 2026-05-19)

Resolves all many-to-many chunk memberships; preserves higher-ability IDs and membership keys; passes them into prediction/replay; creates candidate ability when no safe existing ability explains a refuted prediction.

FSV: `fsv/task-py-g-121-runtime-ability-resolver-fsv/run-20260519T203036Z/`.

### 2.7 First chunk-skill linkage MCP slice (#417 first slice — shipped 2026-05-19)

`mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`. Read-only over #414 CFs.

FSV: `fsv/task-py-g-120-skill-linkage-surface-fsv/run-20260519Tcodex1Z/`.

### 2.8 Q4 doctrine freeze (#408 / TASK-PY-G-114 — closed 2026-05-19)

`Q4_DOCTRINE_FREEZE_ACTIVE = true` in `q4_trust_gate.rs`. Pins `Q4_LOSS_COEFFICIENT_MAX = 0.0`, clamps reasoning/Q4 loss to zero, forces `q4_inputs_to_ship_gate() == 0`, adds weekly Q4-row-count-delta recheck.

FSV: `fsv/q4-doctrine-freeze-fsv/q4_doctrine_freeze_fsv.json`, sha256 `23408b7d…`. `q4RowCountDelta=0`, `q4LossCoefficientMax=0.0`, `q4InputsToShipGate=0`, historical Q4 readable, new Q4 fields empty, weekly dashboard fails closed on frozen CF growth.

### 2.9 Slot-preserving constellation architecture (#385/#386/#387/#388/#389/#390/#391/#393/#394/#395/#396/#398 — closed 2026-05-19) — partially invalidated

**⚠ Architectural / invariant work (#386 schema, #387 late-fusion, #388 tensors, #389 calibration, #391 attribution, #394 anti-compensation) SURVIVES.** Structural properties hold regardless of input signal quality.

**❌ Numerical ablation results (#393 / #398) are INVALID.** Measured slot-preserving predictor against synthetic-input panels. The "best ablation `per_slot_pairwise_temporal` ΔBrier `+0.21353…` Δcorrelation `-0.5790…`" finding is meaningless — predictor was fitting hash noise. Same for `no_promotion` recommendation (calibrated full-constellation Brier `0.7539…` vs best local `0.00855…`).

**❌ "Slot-preserving doesn't close the 0.083 gap by itself" framing is INVALID** — conclusion from synthetic-input measurements. Real-input Panel A baseline is now `0.7942190170`; #398-style calibrated rerun and EK-012 `λ_inv` rerun are complete and negative, but do not validate pre-fix conclusion.

Architectural correction from flat-panel projection to slot-preserving array operations. All children closed; remaining open #392 (temporal trajectory features) is follow-up signal surface.

Key results:
- **#387 slot-preserving late-fusion predictor:** live predictor uses per-slot encoders + late fusion + per-slot decoders instead of `linear(CONCAT_INPUT_DIM, hidden_dim)`. No-compensation trace proves one bad slot cannot be hidden by aggregate distance.
- **#388 cross-embedder correlation tensors:** 516 per-cell/window/slot distribution tensors, 2,838 per-cell/window upper-triangle pair tensors, 1,400 valid pair rows, 8 full cell/windows with all 66 active pair rows.
- **#389 per-slot calibration + OOD:** 12 global slot calibrations, 84 fail-closed per-cell slot rows, 66 pair calibrations, strict-conjunction OOD policy. `RealityPrediction.per_slot_ood_reasons` persists through `CF_MEJEPA_LIVE_PREDICTIONS`.
- **#390 constellation intelligence:** real e7/e9 `zero_fraction` pair AUC `0.5882968` vs own-slot baseline `0.40438873` over 109 rows, lift `+0.18390805`.
- **#393 / #398 ablations:** both real-input reruns measured against #535 Panel A baseline (`0.7937270999`), remain `no_promotion`. #536 moved measurement of record to `0.7942190170`. Best #393 uncalibrated family `per_slot_pairwise_temporal` at `0.2876047260`; #398 calibrated full constellation non-promotable. EK-012 `lambda_inv` remeasured by #546; best rank-safe `lambda_inv=0.0` at `0.6704055071`.
- **#394 anti-compensation FSV:** corrupted/missing/stale/dimension-shifted slots, contradictory pair relationships, temporal jumps remain non-ship-gate-eligible even when aggregate normalized scores would pass.

### 2.10 OOD harvest + serving gate (#276 / #129 — closed 2026-05-19)

`CF_MEJEPA_OOD_HARVEST` + `CF_MEJEPA_OOD_CALIBRATIONS` + 4× active-learning projection + `OOD_GATE_OVER_FLAGGING` / `OOD_CELL_AUC_REGRESSION` / `OOD_HARVEST_EMPTY` / `OOD_CALIBRATOR_MISSING` / `OOD_SCORE_ABOVE_THRESHOLD`.

### 2.11 Gτ guard audit (#130 — closed 2026-05-19)

All 8 Python cells audited. Audit PASS but ship eligibility remains fail-closed: 6 low-pass cells, 1 missing substrate cell (`python:compile_error`), 8 per-cell calibration fail-closed cells, 26 missing reward-signal neighborhoods.

### 2.12 Multi-head contradiction calibration (#263 — closed 2026-05-19)

Per-cell `(tau_oracle, tau_failure_count)` thresholds. Verdict assembly abstains on calibrated Q2/pass vs high-severity Q3 conflict. Fails closed on missing thresholds. Records `SINGLE_HEAD_ONLY` when contradiction impossible.

### 2.13 Live-session integration (#275 / #366 — closed 2026-05-18)

5 real SWE-bench session traces. PASS: `allPassed=true`, 9 trace rows, 12 live prediction rows, 1 active-learning row, 9 runtime JSONL files. FSV: `fsv/task-py-g-056-live-session-fsv/run-20260518T212128Z/`, sha256 `4fe5d4af…`.

### 2.14 Q5 shift-log replay + calibration (#264 / #128 — closed 2026-05-19)

`reality_impact.rs` materializes `RealityImpactRecord` from `CF_MEJEPA_LIVE_PREDICTIONS` + session shift logs. Classifies `Confirmed` / `Missed` / `NotYetObserved` / `Surprise`. Fails closed on SHA-chain gaps, unverifiable per-file SHA, before/after discontinuity, out-of-order timestamps. MCP: `mejepa_reality_impact`.

Q5 calibration writes `CF_MEJEPA_Q5_CALIBRATIONS` from replay records. Cells keyed by `(calibration_id, language, mutation_category, side_effect_kind)`. Computes TP/FP/FN, awaiting-oracle, support, precision/recall/F1, surprise rate.

---

## 3. Aiwonder Production Data Map

Active Python ship-gate data:

| Artifact | Path | Role |
|---|---|---|
| 300×8 SWE-bench Lite Python corpus + oracle labels | `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-<TS>/index.json` | 2,400 entries; `oracle_all_passed` is label source |
| AST chunk store | `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | Chunk metadata, stable row keys; active source paths canonicalized by #613 |
| AST chunk source files | `/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/` | aiwonder-native source bytes; 4,224 files / 148,346,301 bytes |
| Full packed embedder forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-<TS>/` | 391 completed shards; per-shard packed manifests; 12 active embedders |
| Forward-cache slot-completeness mask | `/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl` | #614 output; 399,777 rows; prevents incomplete rows from being treated as complete all-pair constellations |
| Delete-test-call regeneration candidates | `/zfs/archive/contextgraph/state/regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1/delete_test_call_regeneration_candidates.jsonl` | #615 output; 1,131 deterministic candidates awaiting Docker oracle |
| Cross-embedder constellation tensor store | `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/` | #388: slot/pairwise tensors |
| Per-slot calibration and OOD guard | `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` | #389: 12 global + 84 per-cell + 66 pair + OOD policy |
| Level-0 label registry | `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/` | #413: 7 JSONL artifacts + manifest |
| Q4 frozen baseline | `/zfs/archive/contextgraph/state/q4-freeze/q4_frozen_cf_counts.json` | #408: weekly delta-zero recheck |

---

## 4. Phase Status Summary

### 4.1 Phases A-F: code/FSV shipped

All Phase A-D implementation issues (#3-#19 umbrella #15) CLOSED. Phase E self-optimization policy shipped. Phase F production operations + observability + runbook + DR + API shipped. Independent audit verified 35/35 items present.

### 4.2 Phase G: in flight — model-quality convergence

Open work targeting 0.95 ship gate. See `08`.

### 4.3 Phase H: complete — inner-actor LLM retirement

`feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`. The previously documented Phase H `scripts/ci/phase_h_*.sh` absence-checks are missing on disk; #693 tracks the doc/control gap for Qwen / Hermes / Ollama / llama.cpp absence checks.

---

## 5. Key Recent Architectural Discoveries

### 5.1 Synthetic-vector contamination invalidated every pre-fix correlation finding

**Headline discovery.** Between Phase G ramp and 2026-05-19 22:45 UTC, the inference path bypassed real frozen instruments and filled panel slots with SHA256-seeded synthetic vectors. The predictor was learning a hash-derived prior, not real code semantics. Same number `0.8666666746139526` verbatim appeared across every single ablation (`λ_inv ∈ {0, 0.25, 0.5, 1.0}`) precisely because the input never changed — only downstream compute did, and a deterministic-input system converges to a deterministic-output fixed point.

Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452) document + fix this. Corpus + label substrate survive (real Candle CUDA forward generated them); only inference-path measurements contaminated.

### 5.2 Slot-preserving refactor is doctrinally correct (status update)

Architectural correction from flat-panel projection (#385) to slot-preserving array operations (#386-#398) IS doctrinally correct: cross-embedder dim compares structurally forbidden, anti-compensation enforced, per-slot calibration + OOD reasons persist. **But the "slot-preserving doesn't close the 0.083 gap by itself" claim is INVALID** — drawn from #398's calibrated ablation rerun against synthetic-input panels. Real-input #393, #398, #546 reruns are now negative evidence — queue ordering should move to targeted weak-cell repair rather than another constellation-wide promotion attempt.

### 5.3 Evidence-weighted ordering (#383, pinned context) — recheck required

#383 says gate-active issue order must be evidence-weighted, not vision-weighted. **Evidence that drove recent reordering was from synthetic-input ablations and is itself invalidated.** Re-rank from `0.7942190170` real Panel A measurement; with #393/#398/#546 remeasured + not promoted, #536 only a small hard-negative weighting lift, #547 passing disjointness, the top of queue is #548 deeper mixed-cell ranking repair + Panel B / anti-Goodhart validation.

### 5.4 The labeling layer is the meaning surface (#404 / #413 / #414 / #418) — substrate stands

Raw slot violations + cross-embedder correlations only become actionable when clustered into 4-level hierarchy of named failure modes grounded by closest historical exemplars. The 491-row accepted Level-0 registry + 128 Level-2 skills + 379 Level-3 constellations + 36,773 chunk-skill memberships are all derived from real forward cache and survive the synthetic-vector bug. Post-fix revalidation proves the hierarchy still covers true failures (117/117 active predicted true-Fail rows) but also exposes 13 false-positive Fail verdicts with no named consequence explanation. Predictor-policy repair, not blanket skill expansion.

### 5.5 The mistake loop replaces best-of-K (#406)

Best-of-K papers over an under-trained predictor by spending compute at inference time. Online mistake loop pays cost at *learning* time and converges to predictor right on first try. Current shipped primitive: key-local exact-panel online-head state with `protection_mode=exactPanelKeyIsolation`. Convergence target: `mistake_repeat_rate ≤ 0.05`. Machinery shipped; needs real predictor mistakes from real trained checkpoint to validate convergence.

### 5.6 Silent-success bugs were everywhere (2026-05-22 5-Sherlock audit)

Most important audit finding: **return values lie**. Across 15 distinct surfaces (memory mutation, MCP eval-run, store_memory, optimizer witness verify, stop self-verify, oracle verdict CFs, etc. — see §2.0), code paths returned `success`/`passed=true`/exit-0 without performing work or after partial write failures. Universal counter-evidence for FSV doctrine: *"a return value is a claim, the source of truth is the verdict."* Fixes shipped; operational implication (#655): auto-FSV harnesses also tend toward silent-success and must be retired in favour of manual SoT readback.

### 5.7 Forward-cache embedder collapse (#666 → #670)

Sherlock substrate-side finding: **forward cache contains invalid vector spaces in `e2`, `e3`, `e4`, `e8`**. #666 closed acknowledging collapse; #670 is follow-up to regenerate-or-retire. Until #670 closes, any predictor evidence riding those embedders is suspect. #657 per-embedder geometry-label bakeoff is the response: discover what each embedder's vector space actually contains *before* claiming it provides predictor signal.

### 5.8 Connectome topology features are diagnostic-only (#513 / #527)

FlyWire-style rich-club analysis tested but demoted: split-stable evidence improved (`cellsSplitReproducibleAt0_8=7/8`) but stable feature not gate-moving (modularity above null `1/8`, full finite-edge coverage `0`, diagnostic correlation regressed `-0.583`). Topology harness kept diagnostic-only; not promoted.

---

## 6. FSV Evidence Catalog (active production)

Selected key recent artifacts. Full index: every closed Phase G issue documents specific test pass count + FSV path + sha256 in its RESOLVED comment.

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
| #549 mixed-cell signal fusion | `task-py-g-549-mixed-cell-signal-fusion-fsv/run-20260520T191159Z/` | `b6b391e…` |
| #392 aggregate trajectory | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T192449Z/` | `a7d763d…` |
| #392 file/skill trajectory | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T195910Z/` | `e686fd1…` |
| #392 out-of-fold probability | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T201911Z/` | `dbf92087…` |
| #392 affine file/skill calibration | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T203036Z/` | `289e050b…` |
| #392 hardened affine/blend | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T205039Z/` | `a197f49…` |
| #392 logit calibration | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T205929Z/` | `6c04068…` |
| #392 mean-bias + support-aware | `task-py-g-392-temporal-trajectory-features-fsv/run-20260520T214049Z/` | `8e3644d…` |
| #554 usefulness-weighted skill | `task-skill-002-usefulness-weighted-392-fsv/run-20260520T232838Z/` | `92d9f98…` |
| #558 full-corpus skill rematerialization | `task-skill-006-full-corpus-skill-rematerialization-fsv/run-1779320263984/` | `27d2471…` |
| #558 #392 rerun from rematerialized skills | `task-skill-006-full-corpus-skill-rematerialization-fsv/run-20260520T233926Z/` | `2c3c7ce…` |
| #560 split-legal ECE repair | `task-skill-008-split-legal-ece-repair-fsv/run-20260521T004021Z/` | `c2505ee…` |
| #557 skill usefulness audit | `task-skill-005-skill-usefulness-audit-surface-fsv/run-20260521T023736Z/` | `5a70160…` |
| #551 coverage-gap closeout materialization | `task-skill-009-targeted-skill-coverage-repair-fsv-default128-final/run-1779333467960/` | `ba2446b…` |
| #392 corrected skill-kind evidence | `task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` | `9ed8830…` |
| Ship-gate row-trace for #549 | `swebench-300x8-ship-gate-fsv/run-20260520T185945Z/` | `781aa3f6…` |
| Stop-hook circuit breaker (#381) | `task-py-g-080-stop-hook-circuit-breaker-fsv/run-20260519T031222Z-457165/` | `20b75ce3…` |
| Patch similarity (#247) | `task-py-g-042-patch-similarity-exemplar-fsv/run-1779075407903/` | `177cc3a7…` |
| Secret/delayed holdout guard (#374) | `task-py-g-077-secret-holdout-fsv/` (result `underpowered`) | `16c1a5a6…` |
| Oracle-noise statistical power (#376) | `task-py-g-076-oracle-noise-power-fsv/` (result `underpowered`) | `1324dab0…` |

Frozen Q4 producer/calibration FSVs (display-only after #408): #266 perf labels (260), #267 accuracy labels (240), #268 cost labels (240), #269 reasoning labels (300), #270 non-trivial-on-Pass (500), #127 calibration, #377 trust gate, #399 perf re-proof, #400 cost re-proof, #401 harvest expansion.

---

## 6.5 Neuroscience-Inspired Reserve (filed 2026-05-20, DO NOT START until current queue concludes)

No results yet — all 8 issues (#538-#545) at `priority:p3` with `⚠ DO NOT START — Neuroscience-reserve contingency` markers. Activation depends on post-current-queue gap analysis. **If** activated and shipped, expected FSV shape:

| Issue | Expected proof artifact | Ablation comparison |
|---|---|---|
| #538 predictive-coding head | `task_py_neuro_001_predictive_coding_head_fsv.json` with per-slot predictions, residuals, slot-identity-preserved=true | Real-input baseline vs predictor-with-PC-head; corr + Brier deltas |
| #539 schema-prioritized replay | `task_py_neuro_002_replay_consolidation_fsv.json` with schema-grouped batch evidence, reverse-replay credit attribution | `mistake_repeat_rate` across schema clusters vs uniform-prior baseline |
| #540 hierarchical predictive coding | `task_py_neuro_003_hierarchical_predictive_fsv.json` with per-level error attribution | Hierarchy as backward-prediction vs hierarchy-as-aggregator-only |
| #541 mixed-selectivity binding | `task_py_neuro_004_mixed_selectivity_binding_fsv.json` with basis-function attribution + slot-identity preserved | Mixed-selectivity vs current #390 linear pairwise statistics |
| #542 / #543 / #544 / #545 | Design-influence; no separate FSV; additions to existing planned epic FSVs (#416 / #519 / #392 / #517) | Per-host-epic ablations |

See `08 §6.9` for activation rule and `05 §18.5` for pipeline-integration sketch.

---

## 7. What This Doc Does NOT Cover

- Per-test timing or flake history (not centrally tracked beyond TASK-G-019 oracle flakiness quarantine).
- Pre-2026-05 detailed phase A-F FSV inventory — see closed issue RESOLVED comments.
- Operator personal hardware setup (`~/.config/mejepa/daemon.toml`, GPU driver versions, CUDA toolkit install).
- Off-aiwonder operator backup procedures — operator responsibility.
