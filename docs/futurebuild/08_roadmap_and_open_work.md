# 08 — Roadmap and Open Work

**Last refreshed:** 2026-05-19.

Source: `gh issue list --state open`, recent `type:decision` closes, git log, GitHub issue comments, and archived roadmap history.

---

## 0. Doctrine Update — Binary Q1/Q2/Q5 Contract (2026-05-19, locked in)

The 2026-05-19 reframe (`01_overview.md §1.1`, `05_predictor_pipeline.md §0`) reduces the prediction contract to three binary surfaces against ground truth (Q1 bytes/AST, Q2 Docker oracle, Q5 shift log), one explanation surface (Q3), and removes Q4 entirely. Five new epics implement the new direction; one closes the old direction; one updates the paper:

| New epic | Purpose | Status |
|---|---|---|
| **#404 / TASK-PY-G-110** — Failure-mode labeling hierarchy | 4-level named-failure-mode hierarchy on top of the panel; meaning layer over the embedding outputs | OPEN p0 (planned filing 2026-05-19) |
| **#405 / TASK-PY-G-111** — Cross-panel triangulation | Panel B re-scoring with non-overlapping encoders; defends 0.95 against encoder-matched Goodhart | OPEN p0 (planned filing 2026-05-19) |
| **#406 / TASK-PY-G-112** — Online mistake-driven learning | Perceptron-family head update on every disagreement-with-reality event; `mistake_repeat_rate ≤ 0.05` convergence target | OPEN p0 (planned filing 2026-05-19) |
| **#407 / TASK-PY-G-113** — Threshold-calibration partition-disjointness audit | Every τ calibrated on partition disjoint from train + eval; 10th-percentile floor enforced | OPEN p1 (planned filing 2026-05-19) |
| **#408 / TASK-PY-G-114** — Q4 doctrine freeze + close out Q4 producer track | Closes #266 / #267 / #268 / #269 / #270 / #399 / #400 / #401 as `archived-display-only`; closes #402 as `wontfix` (ambiguity boundary); freezes Q4 CFs | CLOSED 2026-05-19; aiwonder FSV passed |
| **#409 / TASK-DOC-001** — Paper revision for binary doctrine | Rewrites PAPER §1 from "calibrated multi-head RealityPrediction" to "binary reality predictor with labeled explanation surfaces" | OPEN p2 (planned filing 2026-05-19) |
| **#413 / TASK-PY-G-117** — Deterministic auto-label discovery | Slot-preserving candidate labels, code-state constellations, failure evidence sets, usefulness metrics, and accepted label registry for the 399,777 AST chunk corpus | CLOSED 2026-05-19; FSV passed on aiwonder |
| **#414 / TASK-PY-G-118** — Skill-sequence discovery and chunk-skill reverse membership | Turns groups of TCT constellations into hierarchical skills/abilities, writes chunk -> skill memberships, and defines update/split/create/demote lifecycle behavior when predictions are wrong | CLOSED 2026-05-19; synthetic + full aiwonder materialization FSV passed, live #406 consumer integration pending |
| **#416 / TASK-PY-G-119** — Pathway exploration surface (binary-leaf butterfly-effect tree) | L3 counterfactual rollouts for decision support: top-K (3-5) most-probable future trajectories per candidate patch, chains of Q1/Q2/Q5 binary leaves only (Q4 strictly excluded), operator picks which future to realize; downstream consumer of #413 Level-0 registry, #404 hierarchical naming, #406 mistake loop, #407 calibration disjointness; precedes #335 reverse-prediction | OPEN p1 (filed 2026-05-19) |
| **#417 / TASK-PY-G-120** — Bidirectional chunk↔skill linkage capability surface | Expose the already-shipped #413 + #414 data (399,777 chunks ↔ 491 accepted labels ↔ 128 named skills ↔ 36,773 chunk-skill memberships ↔ 10,579 skill-chunk reverse rows) through MCP tools for code→skill, skill→code, set queries, refactoring blast-radius, cross-repo skill transfer, pathway-leaf-to-code attachment, skill graph, coverage-gap audit, conflict graph, and lifecycle updates when reality refutes a prediction. Read-only navigation over training substrate; no new prediction head; binary-leaf constraint preserved at prediction boundaries. | OPEN p1 (filed 2026-05-19) |

---

## 1. Current Ship Gate (active until met)

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per (mutation_category × language)
  on the 300×8 Python SWE-bench Lite corpus
  using Q2 Docker-oracle agreement as the binary verdict
  AND (NEW 2026-05-19): cross-panel oracle correlation ≥ 0.95 on Panel B
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Last holdout measurement:** `0.866667` (legacy `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`; not current production evidence until revalidated on aiwonder under #369).

**Gap to ship:** 0.083 correlation points on Panel A. Panel B has no measurement yet; #405 will produce the first.

**P0 product contract:** #397 is the guiding context. ME-JEPA is the **binary reality predictor**: it returns `Pass | Fail` for each candidate code change against the Docker oracle (Q2), with named-failure-mode + closest-exemplar explanation (Q3) on Fail. Q1 (claim-exists) and Q5 (predicted-shift-event) are additional binary predictions against bytes/AST and the shift log respectively. **Q4 ambiguous surfaces are out of scope and frozen as display-only historical data.** The MCP surface presents the binary verdict + named explanation + closest exemplars to the human/operator-agent; it does not infer hidden human intent (#358) and it does not generate code.

Once met → human review of AI-generated Python patches is statistically unjustified. Auto-trust policy gates engage (TASK-PY-G-058 / #277).

---

## 2. Open Issue Snapshot (live issue tracker is authoritative)

By phase label:

| Label | Open count | Notes |
|---|---|---|
| `phase:g` | 70 after #269 close | Ship-gate convergence work (this is the critical path) |
| `phase:migration` | 4 | Aiwonder migration / durability follow-ups |
| `phase:f-followup` | 1 | #312 hot_root on ext4 throughput (perf followup) |
| `phase:f` | 0 (closed as of 2026-05-13) | — |
| `phase:h` | 0 (closed) | — |
| No phase | 0 | — |

By priority:

| Priority | Open count | Examples |
|---|---|---|
| `priority:p0` | live tracker authoritative | #397 consequence-prediction contract, #404 hierarchy, #405 cross-panel, #406 mistake loop, #408 Q4 freeze, #385 slot-preserving constellation refactor, #372 aiwonder ZFS mount enforcement, #383 evidence-weighted issue ordering |
| `priority:p1` | 21 after #269 close | Calibration, adversarial defense, Q4 non-trivial-on-Pass, perf/cost producer re-proof, multi-file holdout, off-host backup |
| `priority:p2` | 40 | EK series tail, post-ship policies, refined sub-tasks, frozen-panel dimensionality ablation, tool-usage telemetry |
| `priority:p3` | 9 | Deferred (SWE-bench Verified subset, sample-efficiency, v2 stretch tasks, future Docker image plan) |

---

## 3. Critical Path (the issues whose resolution moves the ship-gate needle)

### 3.−1 Binary-doctrine Phase G ordering (2026-05-19 reframe — supersedes prior queue)

The slot-preserving refactor (§3.0 below) is largely shipped and architecturally correct. The next gate-moving work is the **labeling + cross-panel + mistake-loop** stack from the binary doctrine, in this evidence-weighted order:

| Rank | Epic | Why it moves the gate |
|---|---|---|
| 1 | **#408 / TASK-PY-G-114** — Q4 freeze + producer-track closeout | Frees bandwidth from Q4 expansion (#266–#270, #399–#402) so it can flow to #404/#405/#406. No code blocked, just rebanding. |
| 2 | **#404 / TASK-PY-G-110** — Failure-mode labeling hierarchy | Turns the 78-column pairwise-correlation evidence from #388 into named clusters. Required so Q3 explanation surface can land an actionable named mode on every Fail row. The labeling itself is also the primary input feature for the binary head: same-named-failure-mode neighbors are the closest exemplars the predictor needs to commit. |
| 3 | **#413 / TASK-PY-G-117** — Auto-label discovery baseline | ✅ closed 2026-05-19. Materialized the deterministic baseline label compiler and accepted registry at `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/`. Consumers now have stable label/evidence IDs instead of a flat panel projection. |
| 4 | **#414 / TASK-PY-G-118** — Skill-sequence discovery + chunk-skill reverse membership | Promotes groups of teleological constellations into hierarchical skills/abilities. Synthetic + production aiwonder FSV now prove ordered candidate rows, many-to-many chunk membership, reverse index rows, lifecycle audit rows, live-pending active-learning candidates, and fail-closed leaky/order/support/path edges. Remaining gate work is #406 live mistake-loop consumption. |
| 5 | **#407 / TASK-PY-G-113** — Threshold-calibration disjointness audit | Cheapest move likely to detect under-calibration. The 0.866667 gap may be partly because some τ values were set on a non-disjoint partition. One-day spike with a binary outcome. |
| 6 | **#405 / TASK-PY-G-111** — Cross-panel triangulation Panel B | Defends 0.95 against encoder-matched Goodhart. **Adds a hard new ship-gate requirement.** Without Panel B, even hitting 0.95 on Panel A is not ship-eligible. |
| 7 | **#406 / TASK-PY-G-112** — Online mistake-driven loop | The "first time every time" property. Converts every disagreement-with-reality into a training signal under EWC++ protection, using #413 label/evidence IDs and #414 ordered skill signatures as the stable mistake context. Long-horizon ship-defending work. |
| 7.5 | **#416 / TASK-PY-G-119** — Pathway exploration surface (binary-leaf butterfly-effect tree) | L3 counterfactual rollouts for decision support. Operator-facing reality-shaping layer that converts the binary verdict primitive into top-K (3-5) most-probable future trajectories. Consumes #413 (Level-0 registry for closest-exemplar attachment), #404 (named failure modes on Fail leaves), #406 (mistake loop credits assignment across pathway leaves), #407 (Goodhart-safe surfacing on disjoint calibration), and #417 (skill/chunk attachment per pathway leaf). Precedes #335 \`mejepa_reverse_prediction\` (the system tells the agent what patch-shape lands the chosen pathway). Not gate-blocking; ships when the gate-movers stabilize. |
| 7.6 | **#417 / TASK-PY-G-120** — Bidirectional chunk↔skill linkage capability surface | Makes the live project path explicit: chunks compile into TCT constellations, constellations activate hierarchical skills/abilities, predictions consume ordered active skill IDs, and wrong predictions update/split/create/demote/no-change the skill model. This is the runtime evidence bridge for #416 and the live-learning bridge for #406; not a new prediction head. |
| 8 | **#409 / TASK-DOC-001** — Paper revision | Aligns PAPER §1 with the binary doctrine. Not gate-blocking; defer until first cross-panel green window. |

### 3.0 Slot-preserving constellation architecture (#385 / TASK-PY-G-085)

This is now first in the queue. The code already has typed `Panel` slots, but the current predictor/head/OOD paths still include flat `PANEL_DIM` / `CONCAT_INPUT_DIM` projections and flat residual scoring. That can let one embedder or instrument surface compensate for another, which contradicts the TCT contract in `docs/PAPER.md` and `docs/alldata.md`.

Required before any new `0.95` claim:

- Predictor inputs/outputs preserve slot identity through the product path.
- Q3/Q5 heads expose slot-level evidence or attribution; flat `linear(PANEL_DIM, classes)` heads cannot be accepted as ship-gate proof by themselves.
- OOD, residual, Gtau, and calibration metrics compute per-slot/per-embedder scores first and aggregate through fail-closed conjunction or explicit violation records.
- Persisted panel rows carry per-slot metadata/provenance sufficient for independent aiwonder readback.
- DDA signals come from real active embedder outputs and pairwise relationships, not seeded placeholders.
- FSV includes adversarial panels where one slot is wrong while global flat distance would look acceptable.

Child issue split:

| Issue | Purpose | Why it matters for `0.95` |
|---|---|---|
| #386 / TASK-PY-G-086 | Slot-map `ConstellationObservation` schema | ✅ closed 2026-05-19; makes each slot/embedder independently addressable at prediction time |
| #387 / TASK-PY-G-087 | Slot-preserving late-fusion predictor | ✅ closed 2026-05-19; removes early flat fusion as the predictor's semantic bottleneck |
| #388 / TASK-PY-G-088 | Real aiwonder cross-embedder correlation tensors | Turns the completed corpus substrate into relationship features |
| #389 / TASK-PY-G-089 | Per-slot calibration, uncertainty, and OOD | Prevents weak slots from being hidden by aggregate scores |
| #390 / TASK-PY-G-090 | Consensus/contradiction intelligence heads | ✅ closed 2026-05-19; makes disagreement, redundancy, novelty, and blind spots first-class predictors |
| #391 / TASK-PY-G-091 | Slot-level attribution surfaces | ✅ closed 2026-05-19; explains which slots/evidence surfaces caused a verdict, Q4 concern, Q5 impact, fingerprint match, or active-learning escalation |
| #392 / TASK-PY-G-092 | Temporal constellation trajectory features | Tracks exact-state constellation movement across time/windows |
| #393 / TASK-PY-G-093 | Capability-discovery ablation suite | Measures which constellation features move oracle correlation |
| #398 / TASK-PY-G-097 | Rerun ablations with #388/#389 artifacts | Tests the richer tensor + per-slot calibration state before predictor changes |
| #394 / TASK-PY-G-094 | Anti-compensation FSV suite | ✅ closed 2026-05-19; prevents regressions back to flat-vector scoring |
| #395 / TASK-PY-G-095 | Real-corpus constellation smoke | Starts from the completed 391/391 aiwonder forward-cache substrate, not synthetic data |
| #396 / TASK-PY-G-096 | Constellation fine-tuning objective | Trains on per-slot vectors, pairwise tensors, contradiction/consensus features, temporal metadata, and oracle labels as separate loss components |

**#395 FSV status (2026-05-18):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/`: 7 spread shards, 12 active embedders, 2,688 packed records, 84 per-cell centroid rows, 66 pairwise relationship rows, 20 candidate failure-shape rows, tiny real-corpus training/inference smoke, and independent readback with `flat_vector_concat_used=false`.

**#396 FSV status (2026-05-18):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/`: 353 real-corpus training rows, 1,440 per-slot vector rows, 2,727 row-pair observations, 66 pairwise relationship rows, and a training certificate with separate slot, pairwise, contradiction, temporal, oracle-alignment, and anti-compensation terms. The certificate joins the 391-shard forward cache to the 300×8 corpus oracle index, proves `flat_vector_concat_used=false`, proves no loss component is only a flat `PANEL_DIM` aggregate, and exposes ablation switches that independently disable slot, pairwise, contradiction, and temporal terms. It correctly refuses promotion: the smoke objective does not beat the local baseline ladder or the historical 0.866667 context.

**#386 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-086-constellation-observation-schema-fsv/`: the Rust `ConstellationObservation` schema enumerates all 15 packed panel slots, preserves slot/embedder IDs, dimensions, offsets, filled/missing state, per-slot vector hashes, provenance, calibration cell, and pairwise relationships separately from the packed 5,120-d layout. Edge cases reject duplicate slots, wrong dimensions, non-finite panels, unknown JSON fields, and calibration-cell drift across observation/provenance/slot rows. Main FSV sha256 `9c1b6b870373472b3f8ab1fb999b6e87c62895c828337f0e2fc687c182e8ce11`.

**#387 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-087-slot-preserving-late-fusion-predictor-fsv/`: the live predictor now uses per-slot input encoders, explicit late fusion, and per-slot decoders instead of early `linear(CONCAT_INPUT_DIM, hidden_dim)` plus monolithic output projection as the semantic path. Auxiliary heads now project per-slot logits before late class fusion, so Q3/Q4-style heads can carry slot evidence forward. The no-compensation trace can be built from tensors and proves a one-bad-slot case is rejected even when aggregate distance passes. Main FSV sha256 `9f2c5380794420ca5c4a6a11ca819b731377003b0fbe806f9b2463ab8c6b8d8a`; independent readback sha256 `3a597d3a92160f549785593a86d71f73380809f8f721907cb648463c9c48db12`.

**#393 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv/`: the suite consumed #396 arrays and emitted 3 baselines plus 6 ablations. The flat-vector baseline is explicitly comparison-only (`ship_gate_eligible=false`). Every ship-gate-eligible ablation from the current #396 scalar features is demoted against the local baseline ladder: best local baseline Brier `0.008555205797287593`; best ablation `per_slot_pairwise_temporal` has ΔBrier `+0.21353286994717507` and Δcorrelation vs historical `-0.5790619486178568`. Q4, Q5, and OOD measurements are recorded as label gaps rather than inferred: Q4 producer/calibration gaps #270/#127, Q5 replay #264/#128, and OOD #389/#276/#129 remain the needed data sources.

**#388 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-088-cross-embedder-correlation-tensors-fsv/`: the tensor materializer consumed the 391-shard forward-cache substrate and wrote durable artifacts under `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/`. It produced 516 per-cell/window/slot distribution tensors, 2,838 per-cell/window upper-triangle pair tensors, 1,400 valid pair rows, and 8 full cell/windows with all 66 active pair rows. The report proves `slot_identity_preserved=true`, `flat_vector_concat_used=false`, 12 active embedder slots, declared 15 frozen instrument panel slots where applicable, and redundant/independent/mixed/contradictory/unsupported surface counts.

**#389 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-calibration-ood-fsv/`: the calibrator consumed #388 tensors and #396 training rows, wrote durable calibration state under `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`, and produced 12 global slot calibrations, 84 fail-closed per-cell slot rows, and 66 pair calibration rows. The policy proves strict conjunction, `aggregate_score_never_overrides_slot_violation=true`, `reject_missing_or_stale_calibration=true`, `flat_vector_concat_used=false`, and explicit edge cases for missing slot, stale calibration, nonfinite score, and one-bad-slot rejection when the aggregate mean would pass. The live Rust surface also persists `RealityPrediction.per_slot_ood_reasons` and exposes it through MCP explain/inspect; live FSV PASS is `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-ood-reasons-live-fsv/run-1779156050260/task_py_g_089_per_slot_ood_reasons_live_fsv.json` (sha256 `198bc651ef5075c30676e80625cd8600fe36c3bbc0e1d47ade4662537700a924`). #129 and #130 have now rerun under this guard; #130 proves the current Gτ path is still `ship_gate_countable=false` because per-cell support/calibration and compile-error substrate are incomplete.

**#390 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-090-consensus-contradiction-heads-fsv/run-20260519T124035Z/`: `RealityPrediction.constellation_intelligence` now carries slot-pair consensus, contradiction, redundancy, novelty, blind-spot, and consequence-pressure evidence; `CF_MEJEPA_CONSTELLATION_PATTERNS` persists promoted relationship patterns; `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` receives high-value disagreement rows. The run consumed #396 pairwise observations, found real e7/e9 `zero_fraction` pair AUC `0.5882968` vs own-slot baseline AUC `0.40438873` over 109 rows (lift `+0.18390805`), and proved high synthetic disagreement abstains before a false confident pass. Main FSV sha256 `b1d500f008b0a14a704c4c2f83fe8cd19349f0dfb0c78baedb35c711abc50b64`.

**#391 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-091-slot-attribution-explanation-fsv/run-20260519T132343Z/`: `RealityPrediction.slot_attributions` and `HierarchicalPredictionRecord.slot_attributions` now persist named slot/embedder/chunk attribution records with polarity, source, score, thresholds, margins, Q-head/Q5/fingerprint/active-learning links, reason, and evidence. MCP latest/explain/inspect expose compact summaries for agents while inspect returns the full stored record; ship-gate status advertises the attribution source CFs. The FSV proves 3 live prediction rows, 2 hierarchical rows, rejection/Q4/Q5/fingerprint/active-learning attribution flags true, compact MCP shape true, and validation rejects duplicate rows, rejection verdicts without violating/missing/stale slot evidence, and zero active-learning IDs. Main FSV sha256 `78ec5f364b0f6c50c020901eac42d08f965cf15e22a824f53972deb4d5d401d7`.

**#398 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/`: the ablation suite now consumes #396 arrays, #388 tensors, and #389 calibration/OOD state in one run. It emits 3 baselines plus 7 ablations, 12 per-slot contribution metrics, all 66 per-pair contribution rows, independent readback, and 8 edge cases. The report proves `flat_vector_concat_used_for_ship_gate_eligible_paths=false`, rejects missing/stale/not-global-ready calibration, and preserves #389 one-bad-slot rejection when aggregate mean would pass. Result: `no_promotion`; calibrated full constellation Brier `0.7539461538461542` is worse than the best local baseline Brier `0.008555205797287593`, and correlation is invalid/degenerate rather than lift. Main FSV sha256 `90039db274eb4e37b4fc03ed3bb115b184149c00f3cf2fc684118dff492b854e`; report sha256 `a65b320ac08208d26eaeb57eefb90e7bdd5e994d0662527edb399d7ffa06ed35`.

**#394 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-094-anti-compensation-fsv/`: the suite consumes #388 tensor dimensions and #389 slot/pair calibration, emits six adversarial fixtures, and proves corrupted/missing/stale/dimension-shifted slots, contradictory pair relationships, and temporal slot jumps all remain non-ship-gate-eligible even when aggregate normalized scores would pass. The FSV writes direct readback of prediction, OOD, Gτ, Q4, active-learning, and explanation surfaces. Main FSV sha256 `57f710b98fab31d2548bc6da8397487ac960f7cd02d9659df5335f0b017d32a1`; report sha256 `1d68f304dfc2356633596e05eacdf3ae61f89a053fd5c224a830cb9ea9477047`.

**#377 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-078-q4-trust-gate-fsv/`: the Rust Q4 trust gate now separates `q4.rawObservations` from `q4.trustedConsequences` on MCP prediction/reporting surfaces and fails closed until each Q4 head has producer rows, #127 calibration rows, and exact active-slot evidence for `e1,e2,e3,e4,e6,e7,e8,e9,e10,e12,e13,e14`. Boundary cases prove missing support, producer-without-calibration, calibration-without-slots, wrong slot names, and partial head support cannot unlock aggregate Q4 trust. A durable catalog fixture proves future producer evidence can unlock readiness through `/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json` without changing code. Main FSV sha256 `0af1726a6e104e7cb70c694c22e40a91b96dc6c323b3679f827d86617aeca30a`; independent readback sha256 `5d4b752ec5fca9eda24fe78dbee649e9a00cb20c1435e3a2f65860886f75bcaa`.

**#266/#401 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv/`: the Q4 perf producer now runs real bounded `python3` benchmark and cProfile commands, persists raw wrappers under `/zfs/archive/contextgraph/corpus/python-q4-perf-labels-v1/`, writes real `.prof` files under the FSV fixture root, and stores 260 canonical `(corpus_row_id, metric)` rows in `CF_MEJEPA_Q4_PERF_LABELS` after the #401 expansion. Expanded FSV sha256 `a29d33b25872c8ae2afc34b682206d3a7615e75fdf7e01b735eddc9cd74c1c3d`.

**#399 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-098-q4-perf-real-analyzer-fsv/`: the Q4 perf producer was re-proved from real bounded `python3` benchmark and cProfile commands, persisted raw wrappers under `/zfs/archive/contextgraph/corpus/python-q4-perf-labels-v1/task-py-g-098-1779173936485/`, wrote real `.prof` files under the #399 fixture root, and stores 60 canonical `(corpus_row_id, metric)` rows in `CF_MEJEPA_Q4_PERF_LABELS`. Independent readback uses external `ldb` to read key `task-py-g-098-1779173936485:label:test_linear`. The re-proof FSV root is separate from the shared Q4 catalog producer root, which now uses expanded #266/#401 evidence. Main FSV sha256 `69d84cf47b35f45b61dc9ee49ee1f2700538459dad6c69386d5b7551cc00df63`; independent readback sha256 `c4ac201239aef4c94af135ca31e1ca1478b6d58b0a19ecea749abd8336f70107`.

**#267/#401 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-048-q4-accuracy-label-producer-fsv/`: the Q4 accuracy producer runs real bounded `python3` metric commands, persists raw wrappers under `/zfs/archive/contextgraph/corpus/python-q4-accuracy-labels-v1/`, stores 240 producer rows in `CF_MEJEPA_Q4_ACCURACY_LABELS` after the #401 expansion, preserves same-metric/different-`source_test` rows, treats loss/error metrics as lower-is-better, and persists/reopens quarantine rows without key overwrite. Expanded FSV sha256 `36e8fa3a10ca32982eba743f2f6d2af69ad4908ccc88b2775308891a429d2f1b`.

**#268/#401 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-049-q4-cost-label-producer-fsv/`: the Q4 cost producer runs real bounded `python3` cost commands, persists raw wrappers under `/zfs/archive/contextgraph/corpus/python-q4-cost-labels-v1/`, stores 240 producer rows in `CF_MEJEPA_Q4_COST_LABELS` after the #401 expansion, and maps CI-minute/dependency-count/wheel-byte positive deltas into `PredictedCostRegression` surfaces while preserving negative deltas as improvements. Expanded FSV sha256 `3000e741177c2ebbcc9e67f2cb43fe80fd01abb392fe2fd27b43a84afa190351`.

**#400 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-099-q4-cost-real-artifact-fsv/`: the Q4 cost producer was re-proved from real bounded `python3 -m pytest` output, real `python3 -m pip wheel --no-deps --no-build-isolation` output, requirements/patch bytes adding `numpy==2.0` and `scipy==1.14`, and wheel metadata bytes. The FSV writes 31 raw cost artifacts under `/zfs/archive/contextgraph/corpus/python-q4-cost-labels-v1/`, stores 93 canonical rows in `CF_MEJEPA_Q4_COST_LABELS`, and independent readback uses external `ldb` to read two same-row/different-chunk dependency-count keys without overwrite. The re-proof remains separate; the shared Q4 catalog now uses expanded #268/#401 evidence. Main FSV sha256 `eac99f1c79c44346d99ceeacc6e96708ef9f823199706dc3d59f4c9ee09a7887`; independent readback sha256 `96b4160e6ffc721db425463d5ac71974b0311f8b751c2b65651b5fcb36565c07`.

**#269 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv/`: the Q4 reasoning producer consumes observed `agent_prose`, oracle outcome, prediction verdict, E17 affect score, and `E_Reasoning` pairwise cosines. It stores 300 producer rows in `CF_MEJEPA_Q4_REASONING_LABELS`, stores 5 producer-heldout rows in `CF_MEJEPA_HEAD_CALIBRATIONS`, and persists the centroid checkpoint under `/zfs/archive/contextgraph/state/mejepa-heads/reasoning-head-v1/`. The producer FSV is producer-only and remains historical/display-only after #408; it is not an active prediction or model-quality input. Hedging F1 and Overclaiming F1 are both `1.0`; empty prose, code-only prose, non-English prose, and harvest `<200` all fail closed into explicit classes/statuses. Main FSV sha256 `788be9c14a4f0eeb5c73bf231fe48d470d3476883f69cfbe53b5a27652aaaf6b`; independent readback sha256 `a02021559110c364e3a5ecbf82e6db046d142a44e0c312c6078705b3ec57cddc`.

**#270 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv/`: the Q4 always-on-Pass validation samples 500 real Oracle-Pass SWE-bench rows from `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json`, stores 500 pass-row annotations in `CF_MEJEPA_Q4_PASS_ANNOTATIONS`, computes `q4_pass_nontrivial_rate=0.8088888889` over 450 denominator rows, excludes 50 `KnownGoodNoConcern` rows, surfaces operator-novel concerns, renders `weekly/q4_pass_nontrivial.md`, and flags the `Q4_PASS_NONTRIVIAL_REGRESSION` edge when consecutive-window rate drop exceeds 10%. Independent readback uses external `ldb` to read canonical key `django__django-13230:known_good:48a35caf568d07edb8e27e8810f128819341b655674683e5b265be3b99b187eb:edge_case:6518cd3de1cd525ae0d8805b2250595a6888b72a8f12c107a2d466b54ab97b68`. The producer FSV records historical producer support; #408 freezes the rows display-only, not active prediction input. Main FSV sha256 `08b5442685b7f0630c8f3d1edb1a3ad78402e618e7e7f7a876a54d4422ef35a0`; independent readback sha256 `e911e07345f119392d00c0a3bb4f3fa896340b7dc0242ee1164cd2502e2fd269`.

**#127/#401 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-012-q4-confidence-calibration-fsv/`: the calibration consumer reads the closed and expanded Q4 producers, writes typed `q4cal::<head>::<class>` records to `CF_MEJEPA_HEAD_CALIBRATIONS`, validates exact active slots from `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`, and updates the shared Q4 catalog at `/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json`. Perf (`260`), accuracy (`240`), cost (`240`), reasoning (`300`), and non-trivial-on-Pass (`500`) had historical `q4HeadReady=true`; the aggregate `q4TrustDecisionEligible=true` bit is now archival because #408 freezes Q4 as display-only. Security is stale and structural heads remain excluded/fail-closed until archived. Main FSV sha256 `1b993783ff8a03f5cd94279634debcd936b7aeee7d9c78c6aa22e9db34f1260e`; independent readback sha256 `7173b90df47a961ff4ecee70972db3356cd640e8ca8d166a6e3adcc96535accb`; aggregate #401 FSV sha256 `70750e3956e1674eb100f528339c95daabb7b669246c0d906deefb45706eda2f`.

**Ordering update from #388/#389/#393/#398/#394/#387 (locked in by #383 / TASK-PY-G-084):** the slot-preserving architecture is now enforced in the live predictor path, but the current #396/#388/#389 calibrated feature family is still not promotion-worthy. The richer slot/pair calibration readout is architecturally correct and aggregate-compensation regressions are blocked, but the ablation evidence shows this path does not close the 0.083 correlation gap by itself. **The next gate-moving work, in evidence-weighted order, is:**

1. **Archive the Q4 aggregate path and keep it out of model-quality work.**
   - Q4 producer evidence — #266 TASK-PY-G-047 (perf), #267 TASK-PY-G-048 (accuracy), #268 TASK-PY-G-049 (cost), #269 TASK-PY-G-050 (reasoning), #270 TASK-PY-G-051 (non-trivial-on-Pass), #399 TASK-PY-G-098, #400 TASK-PY-G-099, and #401 TASK-PY-G-100 remain historical FSV proof.
   - Q4 calibration/trust rows — #127 and #377 remain readable for audit, but #408 freezes them display-only; they are not active predictor inputs, replay keys, or ship-gate evidence.
   - Any future "what else could go wrong" evidence must flow through Q2 binary verdict pressure, Q3 named explanation, TCT/skill labels, or explicit post-reality supervision, not a resurrected Q4 head.
2. **Gτ guard audit under the same slot-preserving discipline** — #130 TASK-PY-G-015 is closed. It reports all 8 Python cells from #388/#389 evidence and keeps the Gτ path non-countable: 6 low-pass cells, 1 missing cell (`python:compile_error`), 8 per-cell calibration fail-closed cells, 26 missing reward-signal neighborhoods.
3. **Per-head contradiction calibration** — #263 TASK-PY-G-044 is closed. Verdict assembly now consumes per-cell `CF_MEJEPA_CONTRADICTION_THRESHOLDS`, abstains on calibrated Q2/pass vs high-severity Q3 explanation/TCT consequence-pressure conflict, and fails closed on missing thresholds. Historical Q4 contradiction rows remain display-only.
4. **Goodhart / oracle-noise guards** — #374 TASK-PY-G-077 secret holdout and #376 TASK-PY-G-076 statistical-power bound are closed as guard protocols, both currently fail closed as `underpowered`. They continue to block any "≥ 0.95 × 4 windows" claim until real four-window train-visible plus secret/delayed holdout evidence exists and passes.
5. **Compositional generalization** — #379 TASK-PY-G-081 multi-file structural patch holdout.
6. **Remaining predictor-adjacent constellation surfaces** — #390/#391/#392 (and #382 frozen-panel dimensionality ablation) should follow the label/replay/calibration substrate unless a measured ablation shows they are the next bottleneck.

**Current data-location map for #396 descendants:**

| Data | Path | Notes |
|---|---|---|
| 300×8 corpus + oracle labels | `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json` | 2,400 entries; `oracle_all_passed` is the label source, joined by `(task_instance_id, mutation_category)` |
| AST chunks | `/zfs/hot/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | Provides chunk metadata and row-key fields |
| 391-shard packed forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/` | Per-shard packed manifests point to each embedder's record/index files |
| #388 cross-embedder tensors | `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/` | Durable tensor artifacts: `slot_distribution_tensors.jsonl`, `pairwise_correlation_tensors.jsonl`, `constellation_tensor_report.json`, `constellation_tensor_manifest.json` |
| #388 FSV | `/zfs/archive/contextgraph/fsv/task-py-g-088-cross-embedder-correlation-tensors-fsv/python_constellation_cross_embedder_tensors_fsv.json` | PASS evidence for TASK-PY-G-088 |
| #389 per-slot calibration/OOD | `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` | Durable calibration artifacts: `per_slot_calibration.jsonl`, `per_cell_slot_calibration.jsonl`, `per_pair_calibration.jsonl`, `ood_guard_policy.json`, `per_slot_calibration_report.json` |
| #389 FSV | `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-calibration-ood-fsv/python_per_slot_calibration_ood_fsv.json` | PASS evidence for TASK-PY-G-089; sha256 `24075e0b0cd349c132b3bf2388642e277b854aa121f0039d330487bcf302edd7` |
| #389 live OOD reason FSV | `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-ood-reasons-live-fsv/run-1779156050260/task_py_g_089_per_slot_ood_reasons_live_fsv.json` | PASS evidence that `RealityPrediction.per_slot_ood_reasons` persists through `CF_MEJEPA_LIVE_PREDICTIONS`, byte replays from RocksDB, and rejects empty embedder, missing calibration version, nonfinite score, and over-wide reason arrays without adding rows; sha256 `198bc651ef5075c30676e80625cd8600fe36c3bbc0e1d47ade4662537700a924` |
| #387 late-fusion predictor FSV | `/zfs/archive/contextgraph/fsv/task-py-g-087-slot-preserving-late-fusion-predictor-fsv/slot_preserving_late_fusion_predictor_fsv.json` | PASS evidence for TASK-PY-G-087; sha256 `9f2c5380794420ca5c4a6a11ca819b731377003b0fbe806f9b2463ab8c6b8d8a`; independent readback sha256 `3a597d3a92160f549785593a86d71f73380809f8f721907cb648463c9c48db12` |
| #396 certificate/artifacts | `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/artifacts/` | Generated arrays and certificate for TASK-PY-G-096 |
| #394 anti-compensation FSV | `/zfs/archive/contextgraph/fsv/task-py-g-094-anti-compensation-fsv/python_anti_compensation_fsv.json` | PASS evidence for TASK-PY-G-094; sha256 `57f710b98fab31d2548bc6da8397487ac960f7cd02d9659df5335f0b017d32a1`; fixture rows, active-learning rows, report, independent readback, and external edge readback under the same root |
| #398 calibrated ablation FSV | `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/python_constellation_ablation_with_calibration_fsv.json` | PASS evidence for TASK-PY-G-097; sha256 `90039db274eb4e37b4fc03ed3bb115b184149c00f3cf2fc684118dff492b854e`; result `no_promotion` |
| #398 calibrated ablation artifacts | `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/artifacts/` | Report sha256 `a65b320ac08208d26eaeb57eefb90e7bdd5e994d0662527edb399d7ffa06ed35`; metrics sha256 `dc4e3cbe7ec2f95e029030e4fc9550f8a01791267c6b6618d5482bfe4937ad11`; independent readback sha256 `8b1c0f147692179ce3e50b29a90208dcf143ba8e666f8869d588f4801799d0b6`; external edge readback sha256 `bc7fa442c8e05aa9b5574e3d2a9e95465bc8eedc01282fcb80bf3220964ce473` |
| #390 constellation intelligence FSV | `/zfs/archive/contextgraph/fsv/task-py-g-090-consensus-contradiction-heads-fsv/run-20260519T124035Z/consensus_contradiction_heads_fsv.json` | PASS evidence for TASK-PY-G-090; sha256 `b1d500f008b0a14a704c4c2f83fe8cd19349f0dfb0c78baedb35c711abc50b64`; real e7/e9 pair lift `+0.18390805` over own-slot baseline; independent readback rows: pattern `1`, active learning `1`, live prediction `1` |
| #391 slot-attribution explanation FSV | `/zfs/archive/contextgraph/fsv/task-py-g-091-slot-attribution-explanation-fsv/run-20260519T132343Z/slot_attribution_explanation_fsv.json` | PASS evidence for TASK-PY-G-091; sha256 `78ec5f364b0f6c50c020901eac42d08f965cf15e22a824f53972deb4d5d401d7`; independent readback rows: live predictions `3`, hierarchical predictions `2`; validates compact MCP summaries and full machine-readable stored attribution records |
| #276 OOD harvest FSV | `/zfs/archive/contextgraph/fsv/task-py-g-057-ood-harvest-fsv/run-20260519T022705Z/ood_harvest_fsv.json` | PASS evidence for TASK-PY-G-057; sha256 `bc3f9519f157f657fba9fea573fb2dfef4c256221f6a4ffdfec949e88b987b60`; validates harvest/calibration/active-learning CF readback |
| #129 OOD serving-gate FSV | `/zfs/archive/contextgraph/fsv/task-py-g-014-ood-gate-fsv/run-20260519T104711Z/ood_gate_fsv.json` | PASS evidence for TASK-PY-G-014; sha256 `58c46eaba6ea587468b61064cdd2ffe1e616f1a248a51a31bd264c5b272bb729`; independent readback sha256 `3d550dc98cec6957497e82a849501769fc8f1eb581dd3d146fa53a15d0092e7f`; validates strict OOD calibrator serving, empty-harvest synthetic-only fallback, cell-regression flags, missing/corrupt fail-closed behavior, cold-cell precedence, and monotonic AUC after a larger harvest |
| #130 Gτ guard audit FSV | `/zfs/archive/contextgraph/fsv/task-py-g-015-gtau-guard-audit-fsv/run-20260519T111048Z/gtau_guard_audit_fsv.json` | PASS evidence for TASK-PY-G-015; sha256 `48574d33bb567b3406515dde935267880dad95037a584726a98f4bb368cd05c1`; audit sha256 `8193e8d46771870d7fa6ef0d5425429bc38f823cd85fd9feae914e460e782cd8`; independent readback sha256 `6da01ae0d8bc3ea5adb33195f14da364d1b1a2de95cc0dbb42763f037cf7656c`; validates all 8 Python cells, slot identity, no flat-vector evidence, and explicit low-pass/missing-neighborhood remediation |
| #263 multi-head contradiction FSV | `/zfs/archive/contextgraph/fsv/task-py-g-044-multi-head-contradiction-fsv/run-20260519T114854Z/` | PASS evidence for TASK-PY-G-044; sha256 `39fac46044f4a3a8a9d43a5ff117af8c1b8b46906f186f284c89120460b1cf48`; writes/readbacks `CF_MEJEPA_CONTRADICTION_THRESHOLDS` for `python:over_engineer`, `python:subtle_flip`, and `python:wrong_file`; validates `MULTI_HEAD_CONTRADICTION`, `CONTRADICTION_THRESHOLD_MISSING`, and `SINGLE_HEAD_ONLY`; corrects the stale D19/TASK-PY-G-058 dependency to #396 rows + #279 fixtures |
| #374 secret holdout guard FSV | `/zfs/archive/contextgraph/fsv/task-py-g-077-secret-holdout-fsv/python_secret_holdout_fsv.json` | PASS protocol evidence with current fail-closed result `underpowered`; sha256 `16c1a5a68fe2c79616410ad204e7843c86f5da333b7662772fd3e0d8c814233e`; independent readback sha256 `0a429bc62b95ac25f198acd04822c81744632c628a5dc8e6b8c552fb80205e03` |
| #377 Q4 trust gate FSV | `/zfs/archive/contextgraph/fsv/task-py-g-078-q4-trust-gate-fsv/q4_trust_gate_fsv.json` | PASS evidence for TASK-PY-G-078; sha256 `0af1726a6e104e7cb70c694c22e40a91b96dc6c323b3679f827d86617aeca30a`; independent readback sha256 `5d4b752ec5fca9eda24fe78dbee649e9a00cb20c1435e3a2f65860886f75bcaa`; future producers write readiness catalog at `/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json` |
| #266/#401 Q4 perf label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv/q4_perf_expanded_harvest_fsv.json` | PASS evidence for TASK-PY-G-047 expanded by #401; 260 producer rows; sha256 `a29d33b25872c8ae2afc34b682206d3a7615e75fdf7e01b735eddc9cd74c1c3d`; raw output root `/zfs/archive/contextgraph/corpus/python-q4-perf-labels-v1/task-py-g-100-expanded/` |
| #399 Q4 perf real-analyzer re-proof FSV | `/zfs/archive/contextgraph/fsv/task-py-g-098-q4-perf-real-analyzer-fsv/q4_perf_real_analyzer_fsv.json` | PASS evidence for TASK-PY-G-098; sha256 `69d84cf47b35f45b61dc9ee49ee1f2700538459dad6c69386d5b7551cc00df63`; independent readback sha256 `c4ac201239aef4c94af135ca31e1ca1478b6d58b0a19ecea749abd8336f70107`; raw output row `/zfs/archive/contextgraph/corpus/python-q4-perf-labels-v1/task-py-g-098-1779173936485/`; direct `ldb` key `task-py-g-098-1779173936485:label:test_linear`; separate re-proof, catalog now uses expanded #266/#401 |
| #267/#401 Q4 accuracy label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-048-q4-accuracy-label-producer-fsv/q4_accuracy_expanded_harvest_fsv.json` | PASS evidence for TASK-PY-G-048 expanded by #401; 240 producer rows; sha256 `36e8fa3a10ca32982eba743f2f6d2af69ad4908ccc88b2775308891a429d2f1b`; raw output root `/zfs/archive/contextgraph/corpus/python-q4-accuracy-labels-v1/task-py-g-100-expanded/` |
| #268/#401 Q4 cost label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-049-q4-cost-label-producer-fsv/q4_cost_expanded_harvest_fsv.json` | PASS evidence for TASK-PY-G-049 expanded by #401; 240 producer rows; sha256 `3000e741177c2ebbcc9e67f2cb43fe80fd01abb392fe2fd27b43a84afa190351`; raw output root `/zfs/archive/contextgraph/corpus/python-q4-cost-labels-v1/task-py-g-100-expanded/` |
| #400 Q4 cost real-artifact re-proof FSV | `/zfs/archive/contextgraph/fsv/task-py-g-099-q4-cost-real-artifact-fsv/q4_cost_real_artifact_fsv.json` | PASS evidence for TASK-PY-G-099; sha256 `eac99f1c79c44346d99ceeacc6e96708ef9f823199706dc3d59f4c9ee09a7887`; independent readback sha256 `96b4160e6ffc721db425463d5ac71974b0311f8b751c2b65651b5fcb36565c07`; raw output rows under `/zfs/archive/contextgraph/corpus/python-q4-cost-labels-v1/task-py-g-099-1779175026127*`; separate re-proof, catalog now uses expanded #268/#401 |
| #269 Q4 reasoning label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv/q4_reasoning_label_producer_fsv.json` | PASS historical evidence for TASK-PY-G-050; sha256 `788be9c14a4f0eeb5c73bf231fe48d470d3476883f69cfbe53b5a27652aaaf6b`; independent readback sha256 `a02021559110c364e3a5ecbf82e6db046d142a44e0c312c6078705b3ec57cddc`; raw output root `/zfs/archive/contextgraph/corpus/python-q4-reasoning-labels-v1/`; checkpoint root `/zfs/archive/contextgraph/state/mejepa-heads/reasoning-head-v1/`; display-only after #408 |
| #270 Q4 non-trivial-on-Pass FSV | `/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv/q4_non_trivial_on_pass_fsv.json` | PASS historical evidence for TASK-PY-G-051; sha256 `08b5442685b7f0630c8f3d1edb1a3ad78402e618e7e7f7a876a54d4422ef35a0`; independent readback sha256 `e911e07345f119392d00c0a3bb4f3fa896340b7dc0242ee1164cd2502e2fd269`; real corpus sample `500/1267` Oracle-Pass rows; annotation CF `CF_MEJEPA_Q4_PASS_ANNOTATIONS`; weekly markdown sha256 `b56881c10e81acb70a29d6c0b2f7824b012d97b092f0a0f7abff428ca6abafc1`; direct `ldb` key `django__django-13230:known_good:48a35caf568d07edb8e27e8810f128819341b655674683e5b265be3b99b187eb:edge_case:6518cd3de1cd525ae0d8805b2250595a6888b72a8f12c107a2d466b54ab97b68`; display-only after #408 |
| #127/#401 Q4 confidence calibration FSV | `/zfs/archive/contextgraph/fsv/task-py-g-012-q4-confidence-calibration-fsv/q4_confidence_calibration_fsv.json` | PASS historical evidence for TASK-PY-G-012 after #401; sha256 `1b993783ff8a03f5cd94279634debcd936b7aeee7d9c78c6aa22e9db34f1260e`; independent readback sha256 `7173b90df47a961ff4ecee70972db3356cd640e8ca8d166a6e3adcc96535accb`; historical perf/accuracy/cost/reasoning/non-trivial-on-Pass readiness; display-only after #408 |
| #401 Q4 harvest expansion FSV | `/zfs/archive/contextgraph/fsv/task-py-g-100-q4-harvest-expansion-fsv/q4_harvest_expansion_fsv.json` | PASS historical evidence for TASK-PY-G-100; sha256 `70750e3956e1674eb100f528339c95daabb7b669246c0d906deefb45706eda2f`; direct readback sha256 `71b39669818cb7c196fd5dfe56fdc1d643e6dbfe84909f00911f54bddbff0e78`; proves historical Q4 trust-gate behavior before #408 freeze |
| #408 Q4 doctrine freeze FSV | `/zfs/archive/contextgraph/fsv/q4-doctrine-freeze-fsv/q4_doctrine_freeze_fsv.json` | PASS evidence for TASK-PY-G-114; sha256 `23408b7d09571ca123447d05b365d1ac33b200142be9ce64a75d07c163eb32c3`; independent readback sha256 `5e2f0213a36d9463bd2f630d45f1928fd3c2a80e21b1fa7e06825d80c10412d5`; proves `q4RowCountDelta=0`, `q4LossCoefficientMax=0.0`, `q4InputsToShipGate=0`, historical Q4 remains readable, new Q4 fields are empty, and the weekly dashboard fails closed on frozen Q4 CF growth plus `q4cal::*` calibration growth |

### 3.1 Q4 head training — CLOSED 2026-05-19 (ambiguity boundary)

**Per the binary doctrine (`01_overview.md §1.1`, `05_predictor_pipeline.md §16`), all Q4 surfaces are out of the prediction contract.** Q4 questions ("is this performant?", "is this maintainable?", "are there latent bugs?") do not have a free binary form against reality — they require arbitrary thresholds on continuous heuristics or ask taste judgments with no ground truth. The system does not predict ambiguous things because predicting noise corrupts the mistake-driven learning loop.

Closure status of every prior Q4 producer/calibration issue:

| Issue | New status |
|---|---|
| #266 / TASK-PY-G-047 — Q4 perf analyzer label pipeline | `archived-display-only`; 260 rows frozen in `CF_MEJEPA_Q4_PERF_LABELS` |
| #267 / TASK-PY-G-048 — Q4 accuracy analyzer label pipeline | `archived-display-only`; 240 rows frozen |
| #268 / TASK-PY-G-049 — Q4 cost analyzer label pipeline | `archived-display-only`; 240 rows frozen |
| #269 / TASK-PY-G-050 — Reasoning-class head | `archived-display-only`; 300 rows frozen |
| #270 / TASK-PY-G-051 — Non-trivial-on-Pass | `archived-display-only`; 500 rows frozen |
| #127 / TASK-PY-G-012 — Q4 confidence calibration | `archived-display-only`; Brier/ECE/τ rows frozen |
| #377 / TASK-PY-G-078 — Q4 trust gate | `archived-display-only`; gate is now a no-op pass-through |
| #399 / TASK-PY-G-098 — Q4 perf re-proof | `archived-display-only` |
| #400 / TASK-PY-G-099 — Q4 cost re-proof | `archived-display-only` |
| #401 / TASK-PY-G-100 — Q4 harvest expansion | `archived-display-only` |
| #402 / TASK-PY-G-101 — Q4 security/structural producers | `wontfix-ambiguity-boundary` (ambiguous heads not predictable; semgrep/bandit findings are heuristics not reality) |
| #118 / TASK-PY-G-003 — Per-head Q1-Q5 distillation | re-scoped to Q1/Q2/Q5 only; Q4 heads removed |
| #292 / TASK-PY-G-073 — Predicted-Works head + sub-heads | retained (Q2 sub-classifier, not Q4); already closed |

**Closure procedure completed under #408 / TASK-PY-G-114.** That epic wrote the one-time Q4 freeze FSV at `/zfs/archive/contextgraph/fsv/q4-doctrine-freeze-fsv/`, pins `Q4_LOSS_COEFFICIENT_MAX=0.0`, clamps direct reasoning/Q4 loss assembly to zero, forces `q4_inputs_to_ship_gate() == 0`, and adds the weekly Q4-row-count-delta recheck to `mejepa_weekly_eval_dashboard` including the `CF_MEJEPA_HEAD_CALIBRATIONS:q4cal::*` counter.

**What replaces the Q4 surfaces in the operator answer.** Operators who used to want "what else could go wrong" answers get them from the **named failure mode** + **closest exemplar** attached to a Fail verdict (§3.−1 row 2, epic #404). If the verdict is Pass, the operator answer is "Pass + Q1=Yes (claim landed in bytes) + closest exemplars all Pass + cross-panel Panel B confirms" — not a list of speculative ambiguous risks.

### 3.2 Q5 shift-log replay (#264 / TASK-PY-G-045)

✅ **Shipped 2026-05-19 on aiwonder.** `reality_impact.rs` now materializes durable Q5 `RealityImpactRecord` rows in `CF_MEJEPA_REALITY_IMPACT` from persisted `CF_MEJEPA_LIVE_PREDICTIONS` plus session shift logs. It classifies `Confirmed`, `Missed`, `NotYetObserved`, and `Surprise` consequences, fails closed on explicit SHA-chain gaps, unverifiable per-file SHA evidence, before/after SHA discontinuity, and out-of-order timestamps, and exposes `mcp__cgreality__mejepa_reality_impact`.

FSV evidence: `/zfs/archive/contextgraph/fsv/task-py-g-045-shift-log-replay-engine-fsv/shift_log_replay_engine_fsv.json`, sha256 `2e2116e2c0f15c2861d52798b1aefe0435b8acda50399f4a0547b51190fb0959`; independent reopened-RocksDB readback sha256 `8ef576a9353015ea1e4957b753dbe4c717cc92431fcd58824574ca159ee55df5`.

Q5 calibration consumer: #128 is shipped and writes `CF_MEJEPA_Q5_CALIBRATIONS` from these replay records.

### 3.2.1 Q5 impact calibration (#128 / TASK-PY-G-013)

✅ **Shipped 2026-05-19 on aiwonder.** `reality_impact.rs` now materializes durable `Q5CalibrationRecord` rows in `CF_MEJEPA_Q5_CALIBRATIONS` from #264 replay records in `CF_MEJEPA_REALITY_IMPACT`.

Calibration scope:
- Cells are keyed by `calibration_id`, `language`, `mutation_category`, and `side_effect_kind`.
- Metrics include true positives, false positives, false negatives, awaiting-oracle count, replay-exclusion count, support, precision, recall, F1, and surprise rate.
- `NotYetObserved` rows set `AwaitingOracle` and stay out of precision/recall denominators.
- Fail-closed replay errors such as `SHIFT_LOG_CHAIN_BROKEN` set `ReplayExcluded` and stay out of metric denominators.
- High-surprise and low-F1 cells emit retraining flags for the next model-quality loop.

FSV evidence: `/zfs/archive/contextgraph/fsv/task-py-g-013-q5-calibration-fsv/q5_calibration_fsv.json`, sha256 `042bd8ad45df924b6f607c3c2f2bcacc339ad91a11e5457bd1da52b24b22f133`; independent reopened-RocksDB readback sha256 `aa8f56ec0be0bc2e85e9232f391a2f7e3b9c013db37a5ba2091135c85df45a26`; sample sha256 `a0ce89e3a859b634677ab14222f8b5ce8875740074b621cdf9e2f04041db7731`.

### 3.3 Phase G calibration sweep (Phase G §3)

All calibration items below are now qualified by #385: their evidence must be per-slot/per-embedder constellation evidence, not a flat panel aggregate.

| Issue | Task | Purpose |
|---|---|---|
| #122 | refined 66-cell pairwise MI matrix audit (redundancy/degeneracy) | open |
| #123 | per-cell TCT signal-saturation matrix (realized/theoretical) | open |
| #124 / TASK-PY-G-009 | frozen instrument coverage audit per Python cell | open |
| #125 / TASK-PY-G-010 | Q1 claim-existence calibration at corpus scale | open |
| #126 / TASK-PY-G-011 | Q3 failure-mode calibration | open |
| #127 / TASK-PY-G-012 | Q4 confidence calibration per class with Brier/ECE/τ | ✅ closed 2026-05-19; aggregate Q4 ready after #401 for producer-backed heads |
| #128 / TASK-PY-G-013 | Q5 head calibration vs shift-log replay (consumes #264) | ✅ closed 2026-05-19; writes `CF_MEJEPA_Q5_CALIBRATIONS` |
| #263 / TASK-PY-G-044 | Multi-head contradiction calibration per cell | ✅ closed 2026-05-19; writes `CF_MEJEPA_CONTRADICTION_THRESHOLDS` |
| #121 / TASK-PY-G-006 | Conformal RCA classifier | open |
| #130 / TASK-PY-G-015 | Gτ guard audit | ✅ closed 2026-05-19; audit complete, current Gτ path non-countable until low-pass/missing neighborhoods are remediated |
| #152 / TASK-PY-G-021 | E_Trace gating | open |

**Gate for Phase G §3:** per-head Brier ≤ 0.05, ECE ≤ 0.05, conformal coverage 88–92%.

### 3.4 Phase 5 / Phase 6 closure (continuous self-improvement + agent feedback loop)

| Issue | Task | Status |
|---|---|---|
| #274 / TASK-PY-G-055 | UserPromptSubmit hook live RealityPrediction injection | ✅ closed |
| #154 / TASK-PY-G-023 | Active-learning consumer (multi-source queue: surprise / contradiction / OOD / agent / operator) | open |
| #275 / TASK-PY-G-056 | Live Claude Code session integration FSV (end-to-end) | ✅ closed 2026-05-18 via aiwonder FSV #366 |
| #276 / TASK-PY-G-057 | Live-session OOD corpus harvest | ✅ closed 2026-05-19 via aiwonder FSV |
| #131 / TASK-PY-G-016 | Feedback cooldown (prevent reward thrashing) | open |
| #163 / TASK-PY-G-024 | Feedback retrain consumer | open |
| #182 / TASK-PY-G-040 | Feedback replay-weight FSV | open |

**#276 FSV status (2026-05-19):** PASS on aiwonder. Evidence: `/zfs/archive/contextgraph/fsv/task-py-g-057-ood-harvest-fsv/run-20260519T022705Z/ood_harvest_fsv.json` (sha256 `bc3f9519f157f657fba9fea573fb2dfef4c256221f6a4ffdfec949e88b987b60`). The FSV seeds 50 live prediction rows, harvests 5 deliberate OOD rows, confirms active-learning queue weight `4.0`, persists orphan and degenerate-panel quarantines, tiers down an expired row, downweights an operator-labeled in-distribution row, flags `OOD_GATE_OVER_FLAGGING` at false-positive-rate `0.5`, and proves review rows include RocksDB CF/key/value byte anchors.

**#129 FSV status (2026-05-19):** PASS on aiwonder. Evidence: `/zfs/archive/contextgraph/fsv/task-py-g-014-ood-gate-fsv/run-20260519T104711Z/ood_gate_fsv.json` (sha256 `58c46eaba6ea587468b61064cdd2ffe1e616f1a248a51a31bd264c5b272bb729`) plus independent readback sha256 `3d550dc98cec6957497e82a849501769fc8f1eb581dd3d146fa53a15d0092e7f`. The FSV proves strict `mejepa_verify` consumption of selected `CF_MEJEPA_OOD_CALIBRATIONS`, τ_ood serving from harvested rows, normal Python vs fresh OOD behavior, missing/corrupt calibrator fail-closed via `OOD_CALIBRATOR_MISSING`, empty harvest synthetic-only calibration with `OOD_HARVEST_EMPTY`, per-cell AUC regression with `OOD_CELL_AUC_REGRESSION`, cold-cell precedence over OOD, and monotonic AUC after a larger harvest.

**#130 FSV status (2026-05-19):** PASS on aiwonder. Evidence: `/zfs/archive/contextgraph/fsv/task-py-g-015-gtau-guard-audit-fsv/run-20260519T111048Z/gtau_guard_audit_fsv.json` (sha256 `48574d33bb567b3406515dde935267880dad95037a584726a98f4bb368cd05c1`), audit report sha256 `8193e8d46771870d7fa6ef0d5425429bc38f823cd85fd9feae914e460e782cd8`, independent readback sha256 `6da01ae0d8bc3ea5adb33195f14da364d1b1a2de95cc0dbb42763f037cf7656c`. The audit consumes #388 tensors plus #389 calibration/policy state, reports all 8 Python mutation cells, proves `slotIdentityPreserved=true` and `flatVectorConcatUsed=false`, and validates empty/missing/invalid/over-limit edge cases. Current result is fail-closed for promotion: 6 low-pass cells, 1 missing substrate cell (`python:compile_error`), 8 per-cell calibration fail-closed cells, and 26 missing reward-signal neighborhoods.

**#263 FSV status (2026-05-19):** PASS on aiwonder. Evidence: `/zfs/archive/contextgraph/fsv/task-py-g-044-multi-head-contradiction-fsv/run-20260519T114854Z/multi_head_contradiction_fsv.json` (sha256 `39fac46044f4a3a8a9d43a5ff117af8c1b8b46906f186f284c89120460b1cf48`). The FSV calibrates per-cell `(tau_oracle, tau_failure_count)` thresholds for `python:over_engineer`, `python:subtle_flip`, and `python:wrong_file`, persists them to `CF_MEJEPA_CONTRADICTION_THRESHOLDS`, proves reopened-RocksDB readback, abstains on `MULTI_HEAD_CONTRADICTION`, fails closed on `CONTRADICTION_THRESHOLD_MISSING`, and skips only as `SINGLE_HEAD_ONLY` when no failure head is present. The run records the dependency correction: live tracker maps TASK-PY-G-058 to #277 post-ship auto-trust, so #263 used #396 constellation rows plus #279 adversarial fixtures.

### 3.5 Phase 7 adversarial defense + ship-gate closure (after calibration green)

| Issue | Task | Status |
|---|---|---|
| #178 | refined cell taxonomy with Active/Exempt/Uncovered tri-state | open |
| #129 / TASK-PY-G-014 | OOD detection gate calibrated on harvested + synthetic OOD | ✅ closed 2026-05-19 via aiwonder FSV |
| #120 / TASK-PY-G-005 | Catastrophic Python cell quarantine | open |
| #148 / TASK-PY-G-017 | Python leakage detector | open |
| #153 / TASK-PY-G-022 | Python oracle pass/fail audit | open |
| #165 / TASK-PY-G-027 | Python 0.95 confidence gate | open |
| #167 / TASK-PY-G-032 | Four-window Python burn-in | open |
| #172 / TASK-PY-G-034 | Python acceptance bundle | open |
| #180 / TASK-PY-G-038 | MCP RealityPrediction FSV | open |
| #171 / TASK-PY-G-033 | Fail-closed release switch | open |
| #374 / TASK-PY-G-077 | Secret holdout Goodhart guard for the 0.95 gate (`type:test-gap`) | ✅ closed 2026-05-19; protocol PASS, current result `underpowered` |
| #376 / TASK-PY-G-076 | Bound oracle noise floor + ship-gate statistical power | ✅ closed 2026-05-19; result `underpowered`, #165/#167 cannot claim `0.95 x 4 windows` yet |
| #377 / TASK-PY-G-078 | Q4 trust gate must fail closed until label producers train heads | ✅ closed 2026-05-19; FSV PASS |
| #379 / TASK-PY-G-081 | Multi-file structural patch holdout for compositional generalization | open p1 |
| #381 / TASK-PY-G-080 | Stop-hook self-verify circuit breaker | ✅ closed 2026-05-19; bounded BLOCKED artifacts and timeout breaker shipped |
| #382 / TASK-PY-G-083 | Frozen-panel dimensionality ablation | open p2 |
| #375 / TASK-PY-G-079 | Consume witness provenance in `RealityPrediction.confidence` | open p2 |
| #378 / TASK-PY-G-082 | MCP tool-usage telemetry + dead-surface audit | open p2 |

### 3.6 Phase 8 post-ship operation (engages after `≥ 0.95 × 4 windows`)

| Issue | Task |
|---|---|
| #277 / TASK-PY-G-058 | Post-ship-gate auto-trust policy |
| #173 / TASK-PY-G-035 | Post-green drift guardrails |
| #170 / TASK-PY-G-031 | Model-quality RCA dashboard |
| #168 / TASK-PY-G-029 | Live Python patch backfill |
| #164 / TASK-PY-G-026 | Checkpoint provenance |
| #162 / TASK-PY-G-025 | Eval-after-promotion hook |
| #166 / TASK-PY-G-028 | Python error taxonomy |
| #169 / TASK-PY-G-030 | Unseen Python holdout quarantine |

---

## 4. EK Series (EPIC #321 explanatory-knowledge programme) — open tail

| Issue | Status |
|---|---|
| #325 TASK-EK-004 — Mechanism-conjecture surface on RealityPrediction | open |
| #326 TASK-EK-005 — `mejepa_counterfactual_under_invariant` MCP tool | open |
| #327 TASK-EK-006 — Curiosity reward | ✅ closed 2026-05-18 |
| #328 TASK-EK-007 — Held-out failure-mode class falsification | open |
| #329 TASK-EK-008 — AlphaEvolve-pattern outer loop using RealityPrediction as fitness | open |
| #330 TASK-EK-009 — Ontology-growth audit + telemetry (system-vs-operator attribution) | open |
| #335 TASK-EK-013 — `mejepa_reverse_prediction` MCP tool — desired-reality → patch-shape constraints | open |
| #336 TASK-EK-014 — Operator-contribution surface | in-progress |
| #339 TASK-EK-016 — Chunk foundationality | in-progress |
| #340 TASK-EK-017 — Library-level foundationality | in-progress |
| #349 TASK-EK-016-ext — Embedder-foundationality | in-progress |
| #352 TASK-EK-015 — Credibility-weighted operator override | in-progress |
| #354 / #355 / #356 / #357 | TASK-EK-018 v2 stretch (NAS embedder, cross-library, self-rewriting, status reallocation) — `priority:p3` |

---

## 5. Aiwonder migration (open tail)

| Issue | Task | Status |
|---|---|---|
| #310 | META — Issue refactor (2026-05-15) | in-progress p0 |
| #366 / TASK-AIWONDER-015 | Rerun TASK-PY-G-056 live-session FSV on aiwonder | ✅ closed 2026-05-18 |
| #367 / TASK-AIWONDER-016 | Project-ingest/runtime data locality audit for arbitrary repo folders | ✅ closed 2026-05-18 |
| #368 / TASK-AIWONDER-017 | Host capability utilization audit + scheduler profile | open p1 |
| #369 / TASK-AIWONDER-018 | Revalidate post-migration Phase G FSV artifacts on aiwonder | open p0 (blocks transition of all Phase G FSV from `/mnt/d` to `/zfs/archive`) |
| #372 / TASK-AIWONDER-019 | ContextGraph roots must be mounted on aiwonder ZFS pools (paths-policy enforcement) | open p0 `status:blocked` |
| #380 / TASK-AIWONDER-020 | Off-host backup for `/zfs/archive` proof history (#296 backup posture follow-up) | open p1 |
| #384 / TASK-AIWONDER-021 | CUDA/candle rollback and reproducibility lane | open p2 |
| #295 | PERF — #199 resume launch scans full D-drive FSV root | in-progress |
| #299 / #300 / #301 / #302 / #303 / #304 / #305 | AIWONDER-003..010 install / transfer / build / launch / calibration sub-tasks | in-progress p1 |
| #306 | AIWONDER-011 SWE-bench Lite Python Docker image plan (future Q3) | p3 |

---

## 6. Active type:context pins (must read at session start per AGENTS.md)

| # | Title | Why pinned |
|---|---|---|
| #310 | META Issue refactor (2026-05-15) | Tracks Phase G execution-order map; the live source of "what to do NOW" |
| #227 | Python ME-JEPA data explosion runtime policy | Defines runtime constraints during forward cache jobs |
| #296 | aiwonder migration EPIC (closed but still load-bearing context) | Establishes the migration plan |
| #383 / TASK-PY-G-084 | Gate-active issue order must be evidence-weighted, not vision-weighted (p0) | When choosing the next task, weight by measured impact on the 0.083 correlation gap (e.g., #388/#389/#398 showed slot-preserving features alone don't close the gap → next bandwidth shifts to label producers, not more architectural refactors) |
| #397 / [PY-G] CONTEXT | ME-JEPA consequence-prediction truth-system contract (p0) | "Predicting reality" = predicting consequences of code being live; the MCP product presents calibrated consequence evidence to the operator/agent; hidden human intent is not a prediction target (locks in #358) |

---

## 7. Bugs (open, non-blocking)

| # | Title | Status |
|---|---|---|
| #295 | Resume launch scans full D-drive FSV root (perf) | in-progress |
| #307 | promote_tree_verified asserts 'previous destination archived' on second-subrun shard promotion (regression in 4cd003e1) | in-progress |
| #320 | phase_g_project_merkle_cache_fsv example fails clippy — `ProjectMerkleCache` doesn't impl `Debug` | needs-triage p1 |
| #338 | Capture agent response bodies in runtime/agent-history for Q1 claim FSV | needs-triage p2 |
| #312 | hot_root on ext4 is the throughput bottleneck — move to /hotpool ZFS | open `phase:f-followup` |

---

## 8. Deferred (not blocking ship gate)

| # | Why deferred |
|---|---|
| #149 / TASK-PY-G-018 | SWE-bench Verified subset — separate dataset; ship gate uses Lite 300×8 (priority dropped p2→p3 2026-05-15) |
| #150 / TASK-PY-G-019 | Python sample-efficiency analysis (post-ship-gate) |
| #354–#357 | TASK-EK-018 v2 stretch (NAS embedder from scratch / cross-library proposals / self-rewriting absence priors / status reallocation extension) |
| Non-Python ship-gate cells (Rust/JS/TS/Go/Java/C/Cpp/CSharp/Ruby/Php) | Active execution is Python-first (`CLAUDE.md §6.3`); non-Python deferred until Python truth system is green (archived roadmap history) |
| Phase J post-ship roadmap | Archived at `_archived/09_POST_SHIP_ROADMAP_PHASE_J_20260518.md`; inactive until the Python `>=0.95 x 4 windows` gate closes |

---

## 9. Archived Post-Ship Roadmap

Phase J is archived and inactive until the Python `>= 0.95 x 4 windows` gate is green. See `_archived/09_POST_SHIP_ROADMAP_PHASE_J_20260518.md` for the parked ideas. Current execution bandwidth stays on the 0.083 correlation gap, starting with #385 because flat-panel compensation can invalidate downstream calibration evidence.

---

## 10. How to Pick Up Work (per AGENTS.md)

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
9. Code-simplifier pass (`docs2/hardeningprompt.md`).
10. File any observed defect / pattern / discovery as a fresh issue before ending the turn.

---

## 11. What This Doc Does NOT Cover

- Day-to-day operator decisions outside the issue tracker — those happen in conversation with the operator and become `type:decision` issues when locked in.
- Hardware procurement / aiwonder admin — `phase:migration` issues track that subset.
- Speculative future modalities or business outcomes — outside the WHAT-IS scope; capture as `type:context` issues if ever revisited.
- Time estimates — Phase G has no public ETA; TASK-G-021 (per-cell convergence ETA tracker) emits internal projections used by the heal scheduler.
