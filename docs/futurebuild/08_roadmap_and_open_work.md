# 08 — Roadmap and Open Work

Source: `gh issue list --state open`, recent `type:decision` closes, git log, GitHub issue comments.

---

## 0. Doctrine Reminder (binary contract)

The prediction contract is **three binary surfaces against ground truth** (Q1 bytes/AST, Q2 Docker oracle, Q5 shift log), **one explanation surface** (Q3 named failure mode + closest exemplar), and **Q4 removed entirely** (ambiguous heads not predictable). See `01_overview.md §1.1` and `05_predictor_pipeline.md §0`.

---

## 0.5. P0 BLOCKER: Ship-gate measurement requires targeted cell repair after rank-lift

Between Phase G ramp and 2026-05-19 22:45 UTC, the inference path used SHA256-seeded synthetic vectors instead of calling real `E*Instrument` encoders. Six P0 fixes (#422 / #424 / #426 / #437 / #438 / #441 / #452, all closed 2026-05-19/20) replaced the broken pipeline. **The previous `0.866667` ship-gate measurement is INVALID** — it was the deterministic fixed-point output of a hash-derived predictor, not a real measurement of model quality on real code semantics. See `01_overview.md §1.7` and `07_results_and_benchmarks.md §0` for forensic detail.

**Current real-input state (#535):** aiwonder FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T164924Z/` reports raw holdout `prediction_oracle_correlation = 0.4843192995`, rank-safe calibrated holdout `0.6916886568`, and real code-state-constellation rank-lift holdout `0.7937270999`. Calibration/rank-lift are fitted only on the 240-row calibration split, constants are restricted to structurally degenerate cells, rank-reversing transforms require mixed-class support ≥ 8 per class plus raw Pearson ≤ -0.25, and rank-lift excludes target-side oracle/docker/test labels. The next lift is targeted repair for remaining weak cells plus cross-panel Goodhart defense (#405), not another synthetic-input rebaseline.

**Remaining revalidation queue:**

1. Finish the uncalibrated #393 companion evidence under #537. The #398-style calibrated rerun and EK-012 `λ_inv` rerun are complete and not gate movers.
2. Repair remaining weak cells from the #535 report (#536): `off_by_one`, `subtle_flip`, `swap_variable`, plus one-fail `delete_test_call` / `known_good` / `over_engineer` cells whose current predictions saturate and leave Pearson undefined.
3. Re-validate #129 OOD serving gate + #130 Gτ guard audit + #404 holdout coverage against real-input predictions.

Tracked under #529 plus #536/#537. **All issues listed in §3 below assume the real-input rebaseline and #535 rank-lift exist unless explicitly noted otherwise.**

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

**Last holdout measurement:** `0.7937270999` rank-lift calibrated from real code-state constellation labels (`0.6916886568` rank-safe calibrated, `0.4843192995` raw) from `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T164924Z/`. Previous `0.866667` figure (from legacy pre-migration evidence) was measured against the synthetic-vector inference path and is invalidated.

**Gap to ship:** `0.1562729001` against the rank-lift calibrated real-input Panel A measurement, plus four-window stability and Panel B cross-checks.

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
| `priority:p0` | #397 consequence-prediction contract, #405 Panel B (blocked), #383 evidence-weighted ordering, #372 aiwonder ZFS mount, #521 Panel B model-backed encoders (blocked), #528 no_mnt_d audit hardening |
| `priority:p1` | #407 calibration disjointness, #379 multi-file holdout, #416 pathway, #417 skill linkage remaining, #511 entropy loss, #392 temporal trajectory, #380 off-host backup, #433 ability resolver replay completeness |
| `priority:p2` | EK series tail, post-ship policies, #382 frozen-panel dimensionality ablation, #378 tool-usage telemetry |
| `priority:p3` | Deferred (SWE-bench Verified subset, v2 stretch tasks, future Docker image plan) |

---

## 3. Critical Path (evidence-weighted ordering)

Per pinned context #383 (LOCKED): gate-active issue order is **evidence-weighted, not vision-weighted**.

### 3.0 The road to 0.95 (post-rebaseline ordering)

| Rank | Epic | Status | Why it moves the gate |
|---|---|---|---|
| **0** | **REBASELINE + RANK-SAFE CALIBRATION** — run `swebench_300x8_ship_gate_fsv` against real-instruments inference + real trained checkpoint | ✅ landed #531/#534 | Valid Panel A rank-safe measurement exists: `0.6916886568`. |
| **0.5** | **#535** mixed-cell rank lift from real labels | ✅ landed 2026-05-20 | Valid Panel A rank-lift measurement exists: `0.7937270999`, Brier `0.0936445221`, ECE `0.0338404700`. |
| 1 | **#537** finish remaining real-input ablation revalidation | OPEN p0; #398-style rerun done; #546 real-input `λ_inv` rerun done | Pre-fix ablations measured nothing meaningful. Reruns so far are not gate movers: calibrated full constellation remains `no_promotion`, and best `λ_inv` variant is `0.0` at `0.6704055071` rank-safe (`-0.1233215928` vs #535 rank-lift baseline). |
| 1.5 | **#536** repair remaining weak cells after #535 | OPEN p0 | Directly targets `off_by_one`, `subtle_flip`, `swap_variable`, and one-fail saturated cells exposed by the latest measurement. |
| 2 | **#404** failure-mode hierarchy | ✅ closed 2026-05-20 | Substrate REAL (deterministic over real corpus); holdout measurements need re-validation against post-fix predictions |
| 3 | **#413** auto-label discovery | ✅ closed 2026-05-19 | 491-row accepted Level-0 registry — REAL (deterministic over real forward cache) |
| 4 | **#414** skill substrate | ✅ closed 2026-05-19 | 128 Level-2 skills, 36,773 chunk memberships, 10,579 reverse rows — REAL (derived from #413) |
| 5 | **#418** runtime hierarchical ability resolver | ✅ closed 2026-05-19 | Pure resolver logic — REAL |
| 6 | **#406 / #523** online mistake loop + MCP | ✅ closed 2026-05-20 | Machinery shipped; needs real predictor mistakes to validate `mistake_repeat_rate ≤ 0.05` convergence |
| 7 | **#407** threshold-calibration partition-disjointness audit | OPEN p1 | Needed now that real calibration exists; must prove no holdout leakage or calibration Goodhart. |
| 8 | **#511** information-entropy auxiliary loss | OPEN p1 | Re-justify against the `0.7937270999` real baseline before training. |
| 9 | **#405** cross-panel triangulation Panel B | OPEN p0, blocked | Even hitting 0.95 on Panel A is not ship-eligible without Panel B. Defends 0.95 against encoder-matched Goodhart. |
| 10 | **#379** multi-file structural patch holdout | OPEN p1 | Compositional generalization |
| 11 | **#392** temporal constellation trajectory features | OPEN p1 | Exact-state movement across windows |
| 12 | **#409** paper revision | OPEN p2 | Aligns PAPER §1 with binary doctrine + adds synthetic-vector remediation section. Not gate-blocking. |

### 3.1 Slot-preserving constellation architecture (#385 parent — substantially shipped)

The slot-preserving refactor is largely shipped and architecturally correct. Closed children: #386 (schema), #387 (late-fusion predictor), #388 (cross-embedder tensors), #389 (per-slot calibration + OOD), #390 (consensus/contradiction heads), #391 (slot attribution), #393 (ablation suite), #394 (anti-compensation), #395 (real-corpus smoke), #396 (training objective), #398 (calibrated rerun), #441 (trained checkpoint export), #452 (native active-embedder adapter).

Remaining open children: #392 (temporal trajectory features), #382 (frozen-panel dimensionality ablation).

**Current finding:** the slot-preserving architecture is correct and remains the required representation contract, but the pre-fix numerical conclusions are invalid because they measured synthetic-input panels. The real-input #398-style calibrated rerun and EK-012 `λ_inv` rerun are now negative evidence, not promotion evidence; finish the remaining #393 companion evidence before using ablations for queue ordering.

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
| #165 | Python 0.95 confidence gate | open (gated by #376/#374 underpowered) |
| #167 | Four-window Python burn-in | open |
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

Consumes: #413 Level-0 registry (closest-exemplar attachment), #404 named failure modes (Fail leaves), #406 mistake loop (credits across pathway leaves), #407 calibration disjointness (Goodhart-safe surfacing), #417 skill/chunk attachment per leaf.

Precedes: #335 `mejepa_reverse_prediction` — operator picks pathway, system tells agent what patch-shape lands it.

Not gate-blocking; ships when gate-movers stabilize.

### 4.2 Bidirectional chunk↔skill linkage (#417 / TASK-PY-G-120, OPEN p1)

First read-only MCP slice shipped 2026-05-19 (`mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`).

Remaining open: skill impact/blast-radius, skill graph inspect, cross-repo skill transfer, skill conflict graph, skill browse, pathway-leaf attachment, stars-as-vectors slice (`mejepa_chunk_as_star`, `mejepa_constellation_membership`).

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

**Why none are gate-movers for 0.95.** The current `0.7937270999 → 0.95` gap is a function of remaining cell-specific ranking weakness and unproven cross-panel generalization, not embedder count or storage architecture. The targeted cell repair + real-input ablation reruns + cross-panel + calibration + entropy loss + mistake loop closes the gap. The creativity engine takes over after 0.95 is hit.

**NOT in scope.** No new ambiguous-head prediction surfaces. No consciousness / sentience / phenomenology framing. No simulation-hacking / reality-bending capability claims. The creativity engine is bounded creativity within a falsifiable substrate.

---

## 6.9. Neuroscience-Inspired Reserve (filed 2026-05-20, DO NOT START until current queue concludes)

**Status:** all eight issues filed at `priority:p3`, `status:needs-triage`, with explicit `⚠ DO NOT START — Neuroscience-reserve contingency` markers in their bodies.

**Activation rule:** these are *contingency plans* for if the current Phase G queue + creativity engine fail to close the remaining ship-gate gap. **They are NOT to be started until:**

1. The current queue concludes (#529 rebaseline → #535 rank lift → #536 weak-cell repair → #407 calibration disjointness → #511 entropy loss → #405 Panel B → #379 multi-file holdout → #392 temporal features).
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
| #372 | ContextGraph roots must be mounted on aiwonder ZFS pools (paths-policy enforcement) | open p0 `status:blocked` |
| #368 | Host capability utilization audit + scheduler profile | open p1 |
| #380 | Off-host backup for `/zfs/archive` proof history | open p1 |
| #384 | CUDA/candle rollback and reproducibility lane | open p2 |
| #295 | PERF — resume launch scans full aiwonder FSV root | in-progress |
| #299-#305 | AIWONDER-003..010 install / transfer / build / launch / calibration sub-tasks | in-progress p1 |
| #306 | SWE-bench Lite Python Docker image plan (future / Q3) | p3 |
| #312 | hot_root on ext4 throughput bottleneck — move to /hotpool ZFS | open `phase:f-followup` |
| #442 | Track aiwonder librocksdb-sys cstdint native build requirement | open p1 |
| #451 | Clean stale legacy /mnt/d prose from older open issue bodies | open p2 |
| #528 | Harden no_mnt_d audit semantics and comment scanning | open p0 in-progress |

---

## 9. Active `type:context` Pins (must read at session start)

| # | Title | Why pinned |
|---|---|---|
| #310 | META Issue refactor | Tracks Phase G execution-order map; the live source of "what to do NOW" |
| #227 | Python ME-JEPA data explosion runtime policy | Defines runtime constraints during forward cache jobs |
| #296 | aiwonder migration EPIC (closed but still load-bearing context) | Establishes the migration plan |
| #383 / TASK-PY-G-084 | Gate-active issue order must be evidence-weighted, not vision-weighted (p0) | When choosing next task, weight by measured impact on the 0.083 correlation gap |
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
| #150 | Python sample-efficiency analysis (post-ship-gate) |
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
