# 08 — Roadmap and Open Work

**Last refreshed:** 2026-05-18.

Source: `gh issue list --state open`, recent `type:decision` closes, git log, GitHub issue comments, and archived roadmap history.

---

## 1. Current Ship Gate (active until met)

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per (mutation_category × language)
  on the 300×8 Python SWE-bench Lite corpus
```

**Last holdout measurement:** `0.866667` (legacy `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`; not current production evidence until revalidated on aiwonder under #369).

**Gap to ship:** 0.083 correlation points. Bridging this is the entire job of Phase G's remaining open work.

**P0 architecture correction before more metric chasing:** #385 / TASK-PY-G-085 now blocks the gate. ME-JEPA must treat the panel as a typed per-embedder/per-instrument Teleological Constellation. The 5,120-d panel is a storage layout, not a single compensable embedding space. Predictor, OOD, heads, persistence, DDA, and FSV evidence must prove that a weak slot cannot be hidden by strong values in other slots.

**P0 product contract:** #397 is the guiding context. ME-JEPA is the consequence-prediction truth system: predicting reality means predicting what happens when code is live against durable repository/test/runtime/dependency/security/perf/cost/maintenance reality. The MCP surface presents predicted consequences, confidence, and evidence to the human/operator-agent; it does not infer hidden human intent (#358) and it does not generate code.

Once met → human review of AI-generated Python patches is statistically unjustified. Auto-trust policy gates engage (TASK-PY-G-058 / #277).

---

## 2. Open Issue Snapshot (live issue tracker is authoritative)

By phase label:

| Label | Open count | Notes |
|---|---|---|
| `phase:g` | 62 | Ship-gate convergence work (this is the critical path) |
| `phase:migration` | 2 | Aiwonder migration — closing out; #368 and #306 still open |
| `phase:f-followup` | 1 | #312 hot_root on ext4 throughput (perf followup) |
| `phase:f` | 0 (closed as of 2026-05-13) | — |
| `phase:h` | 0 (closed) | — |
| No phase (cross-cutting bugs, perf) | ~2 | #295 (resume scan perf), #307 (promote_tree_verified race), #320 (clippy bug) |

By priority:

| Priority | Open count | Examples |
|---|---|---|
| `priority:p0` | 2+ | #385 slot-preserving constellation refactor, #310 META issue refactor, plus any migration blockers still labeled p0 |
| `priority:p1` | 18 | Calibration, adversarial defense, and Q4 head training |
| `priority:p2` | 37 | EK series tail, post-ship policies, refined sub-tasks |
| `priority:p3` | 9 | Deferred (SWE-bench Verified subset, sample-efficiency, v2 stretch tasks) |

---

## 3. Critical Path (the issues whose resolution moves the ship-gate needle)

### 3.0 Slot-preserving constellation architecture (#385 / TASK-PY-G-085)

This is now first in the queue. The code already has typed `Panel` slots, but the current predictor/head/OOD paths still include flat `PANEL_DIM` / `CONCAT_INPUT_DIM` projections and flat residual scoring. That can let one embedder or instrument surface compensate for another, which contradicts the TCT contract in `docs/PAPER.md` and `docs/alldata.md`.

Required before any new `0.95` claim:

- Predictor inputs/outputs preserve slot identity through the product path.
- Q3/Q4/Q5 heads expose slot-level evidence or attribution; flat `linear(PANEL_DIM, classes)` heads cannot be accepted as ship-gate proof by themselves.
- OOD, residual, Gtau, and calibration metrics compute per-slot/per-embedder scores first and aggregate through fail-closed conjunction or explicit violation records.
- Persisted panel rows carry per-slot metadata/provenance sufficient for independent aiwonder readback.
- DDA signals come from real active embedder outputs and pairwise relationships, not seeded placeholders.
- FSV includes adversarial panels where one slot is wrong while global flat distance would look acceptable.

Child issue split:

| Issue | Purpose | Why it matters for `0.95` |
|---|---|---|
| #386 / TASK-PY-G-086 | Slot-map `ConstellationObservation` schema | Makes each slot/embedder independently addressable at prediction time |
| #387 / TASK-PY-G-087 | Slot-preserving late-fusion predictor | Removes early flat fusion as the predictor's semantic bottleneck |
| #388 / TASK-PY-G-088 | Real aiwonder cross-embedder correlation tensors | Turns the completed corpus substrate into relationship features |
| #389 / TASK-PY-G-089 | Per-slot calibration, uncertainty, and OOD | Prevents weak slots from being hidden by aggregate scores |
| #390 / TASK-PY-G-090 | Consensus/contradiction intelligence heads | Makes disagreement, redundancy, novelty, and blind spots first-class predictors |
| #391 / TASK-PY-G-091 | Slot-level attribution surfaces | Explains which evidence surfaces caused a verdict |
| #392 / TASK-PY-G-092 | Temporal constellation trajectory features | Tracks exact-state constellation movement across time/windows |
| #393 / TASK-PY-G-093 | Capability-discovery ablation suite | Measures which constellation features move oracle correlation |
| #394 / TASK-PY-G-094 | Anti-compensation FSV suite | Prevents regressions back to flat-vector scoring |
| #395 / TASK-PY-G-095 | Real-corpus constellation smoke | Starts from the completed 391/391 aiwonder forward-cache substrate, not synthetic data |
| #396 / TASK-PY-G-096 | Constellation fine-tuning objective | Trains on per-slot vectors, pairwise tensors, contradiction/consensus features, temporal metadata, and oracle labels as separate loss components |

**#395 FSV status (2026-05-18):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/`: 7 spread shards, 12 active embedders, 2,688 packed records, 84 per-cell centroid rows, 66 pairwise relationship rows, 20 candidate failure-shape rows, tiny real-corpus training/inference smoke, and independent readback with `flat_vector_concat_used=false`.

**#396 FSV status (2026-05-18):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/`: 353 real-corpus training rows, 1,440 per-slot vector rows, 2,727 row-pair observations, 66 pairwise relationship rows, and a training certificate with separate slot, pairwise, contradiction, temporal, oracle-alignment, and anti-compensation terms. The certificate joins the 391-shard forward cache to the 300×8 corpus oracle index, proves `flat_vector_concat_used=false`, proves no loss component is only a flat `PANEL_DIM` aggregate, and exposes ablation switches that independently disable slot, pairwise, contradiction, and temporal terms. It correctly refuses promotion: the smoke objective does not beat the local baseline ladder or the historical 0.866667 context.

**#393 FSV status (2026-05-19):** PASS on aiwonder. Evidence under `/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv/`: the suite consumed #396 arrays and emitted 3 baselines plus 6 ablations. The flat-vector baseline is explicitly comparison-only (`ship_gate_eligible=false`). Every ship-gate-eligible ablation from the current #396 scalar features is demoted against the local baseline ladder: best local baseline Brier `0.008555205797287593`; best ablation `per_slot_pairwise_temporal` has ΔBrier `+0.21353286994717507` and Δcorrelation vs historical `-0.5790619486178568`. Q4, Q5, and OOD measurements are recorded as label gaps rather than inferred: Q4 producers #266-#269/#127, Q5 replay #264/#128, and OOD #389/#276/#129 remain the needed data sources.

**Ordering update from #393:** do not promote the current #396 scalar feature family. The next evidence-weighted constellation work should materialize richer real-corpus relationship tensors and per-slot calibration (#388/#389), then rerun ablations with those artifacts before changing the predictor path.

**Current data-location map for #396 descendants:**

| Data | Path | Notes |
|---|---|---|
| 300×8 corpus + oracle labels | `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json` | 2,400 entries; `oracle_all_passed` is the label source, joined by `(task_instance_id, mutation_category)` |
| AST chunks | `/zfs/hot/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | Provides chunk metadata and row-key fields |
| 391-shard packed forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/` | Per-shard packed manifests point to each embedder's record/index files |
| #396 certificate/artifacts | `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/artifacts/` | Generated arrays and certificate for TASK-PY-G-096 |

### 3.1 Q4 head training (Phase G §2 producers — heads exist as schema fields but lack training data pipelines)

| Issue | Task | Producer for | Status |
|---|---|---|---|
| #266 / TASK-PY-G-047 | Q4 perf analyzer label pipeline | `predicted_perf_regressions` head | open |
| #267 / TASK-PY-G-048 | Q4 accuracy analyzer label pipeline | `predicted_accuracy_degradations` head | open |
| #268 / TASK-PY-G-049 | Q4 cost analyzer label pipeline | `predicted_cost_regressions` head | open |
| #269 / TASK-PY-G-050 | Reasoning-class head training | `predicted_reasoning_class` head | open |
| #118 / TASK-PY-G-003 | Per-head Q1-Q5 distillation training step | all heads | open |
| #270 / TASK-PY-G-051 | Q4 non-trivial-on-Pass calibration | always-on Q4 validation | open |
| #292 / TASK-PY-G-073 | Predicted-Works head + sub-heads | explicit positive assertions | ✅ closed 2026-05-18 |

### 3.2 Q5 shift-log replay (#264 / TASK-PY-G-045)

`reality_impact.rs` exists as a scaffold; the shift-log replay engine that drives Q5 `RealityImpact` for production traffic is the remaining work.

### 3.3 Phase G calibration sweep (Phase G §3)

All calibration items below are now qualified by #385: their evidence must be per-slot/per-embedder constellation evidence, not a flat panel aggregate.

| Issue | Task | Purpose |
|---|---|---|
| #122 | refined 66-cell pairwise MI matrix audit (redundancy/degeneracy) | open |
| #123 | per-cell TCT signal-saturation matrix (realized/theoretical) | open |
| #124 / TASK-PY-G-009 | frozen instrument coverage audit per Python cell | open |
| #125 / TASK-PY-G-010 | Q1 claim-existence calibration at corpus scale | open |
| #126 / TASK-PY-G-011 | Q3 failure-mode calibration | open |
| #127 / TASK-PY-G-012 | Q4 confidence calibration per class with Brier/ECE/τ | open |
| #128 / TASK-PY-G-013 | Q5 head calibration vs shift-log replay (consumes #264) | open |
| #263 / TASK-PY-G-044 | Multi-head contradiction calibration per cell | open |
| #121 / TASK-PY-G-006 | Conformal RCA classifier | open |
| #130 / TASK-PY-G-015 | Gτ guard audit | open |
| #152 / TASK-PY-G-021 | E_Trace gating | open |

**Gate for Phase G §3:** per-head Brier ≤ 0.05, ECE ≤ 0.05, conformal coverage 88–92%.

### 3.4 Phase 5 / Phase 6 closure (continuous self-improvement + agent feedback loop)

| Issue | Task | Status |
|---|---|---|
| #274 / TASK-PY-G-055 | UserPromptSubmit hook live RealityPrediction injection | ✅ closed |
| #154 / TASK-PY-G-023 | Active-learning consumer (multi-source queue: surprise / contradiction / OOD / agent / operator) | open |
| #275 / TASK-PY-G-056 | Live Claude Code session integration FSV (end-to-end) | ✅ closed 2026-05-18 via aiwonder FSV #366 |
| #276 / TASK-PY-G-057 | Live-session OOD corpus harvest | open |
| #131 / TASK-PY-G-016 | Feedback cooldown (prevent reward thrashing) | open |
| #163 / TASK-PY-G-024 | Feedback retrain consumer | open |
| #182 / TASK-PY-G-040 | Feedback replay-weight FSV | open |

### 3.5 Phase 7 adversarial defense + ship-gate closure (after calibration green)

| Issue | Task | Status |
|---|---|---|
| #178 | refined cell taxonomy with Active/Exempt/Uncovered tri-state | open |
| #129 / TASK-PY-G-014 | OOD detection gate calibrated on harvested + synthetic OOD | open |
| #120 / TASK-PY-G-005 | Catastrophic Python cell quarantine | open |
| #148 / TASK-PY-G-017 | Python leakage detector | open |
| #153 / TASK-PY-G-022 | Python oracle pass/fail audit | open |
| #165 / TASK-PY-G-027 | Python 0.95 confidence gate | open |
| #167 / TASK-PY-G-032 | Four-window Python burn-in | open |
| #172 / TASK-PY-G-034 | Python acceptance bundle | open |
| #180 / TASK-PY-G-038 | MCP RealityPrediction FSV | open |
| #171 / TASK-PY-G-033 | Fail-closed release switch | open |

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
