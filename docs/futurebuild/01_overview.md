# 01 — System Overview

**Status:** Phase G in flight. **Critical path replaced 2026-05-23** by 3-Sherlock audit (#712 pinned). Public trainer/checkpoint lineage not countable (#683); the legacy forward cache contained invalid slots, with `e2/e3/e4` now retired for static AST by #704/#714 and v2 9-slot rebuild tracked by #709/#715; rank-lift not gate-eligible (#695). Honest gap against raw Pearson = `0.4657`. **#633 hardening gate still active** (children #634-#651). Phases A-F complete; Phase H compile-error guard is real, but the historically claimed 9 absence-check scripts are tracked by #693. #392/#511/#405/#379 are blocked behind the new #712 chain. #372 aiwonder ZFS root migration CLOSED 2026-05-22.

See `README.md ⚠ 2026-05-23 Sherlock audit` (new critical path), `08 §3.−1` (replaces §3.0), `07 §2.0` (5-Sherlock closure index), `07 §2.0a` (3-Sherlock 2026-05-23 audit), `06 §1.9` (manual-SoT contract).

**Source files:** `Cargo.toml`, `CLAUDE.md`, `AGENTS.md`, `crates/context-graph-mcp/src/lib.rs`, `crates/context-graph-mejepa/src/lib.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`, `crates/context-graph-mejepa-instruments/src/lib.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs`, `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-mcp/src/tools/definitions/mod.rs`.

---

## 1. What This System Is

**ME-JEPA-Code** is a **binary reality predictor**. AI coding agents (Claude Code / Codex / Cursor) call it via local MCP JSON-RPC. For any candidate code change:

```
verdict ∈ { Pass, Fail }    // "Will this change exist in working reality? Yes/No."
+ named_failure_mode         // (Fail) labeled cluster from failure-mode hierarchy (§1.2)
+ closest_exemplars          // top-K historical patches from 300×8 corpus
+ violating_slots            // per-slot OOD reasons + cross-embedder contradiction evidence
+ mistake_id (when wrong)    // ground truth lands in CF_MEJEPA_MISTAKE_LOG → online learning
```

One prediction per candidate. No best-of-K. No `Abstain` at product surface. Must be right first time; when wrong, mistake captured + predictor adapts.

The five Q-questions:

| Q | Status |
|---|---|
| **Q1** | **Binary prediction.** Did the AI's claim physically land in bytes/AST? Reality = bytes on disk + AST diff. |
| **Q2** | **The predictor itself.** Output `Verdict::Pass\|Fail`. Reality = Docker oracle (`swebench.harness.run_evaluation`). |
| **Q3** | **Explanation only.** Named failure mode + closest exemplar attached to `Fail` verdict. Labels Q2; no own prediction. |
| **Q5** | **Binary prediction.** Per predicted side effect: did corresponding shift-log event occur? Reality = `/zfs/archive/contextgraph/runtime/shift-log.jsonl`. |
| **Q4** | **Removed (#408 closed 2026-05-19).** Ambiguous heads (perf, accuracy, cost, reasoning) have no free binary form against reality. Frozen CFs remain readable; producer track closed. |

**Why Q4 dropped, not thresholded.** Continuous quantities (perf wall-clock, mutation survival rate) only become binary via arbitrary thresholds. Threshold is heuristic, not reality. Mistake-loop convergence requires real disagreement with reality.

**Ship-gate predicate:**

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per mathematically-valid (mutation_category × language) mixed cell
  with degenerate all-pass/all-fail cells gated by #532 replacement contract
  on 300×8 SWE-bench Lite Python corpus
  using Q2 Docker-oracle agreement
  AND cross-panel oracle correlation ≥ 0.95 on Panel B (#405)
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Degenerate-cell metric contract (#532).** Pearson used only when oracle target has both Pass and Fail rows. Single-class cells (`compile_error::python`, `wrong_file::python`) have zero oracle variance → FSV uses target-class recall ≥ 0.95, actual-class confidence ≥ 0.95, Brier ≤ 0.05, ECE ≤ 0.05 + power diagnostics (Fisher-z CI mixed cells; Wilson lower bound degenerate recall). Never emit bare "correlation unavailable" blocker without replacement metric.

### 1.1 Current measurement (2026-05-23 Sherlock decomposition)

Latest ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`:

| Metric | Value | Semantic |
|---|---|---|
| Raw holdout `prediction_oracle_correlation` | `0.4843192995` | **Honest predictor output** |
| Rank-safe calibrated holdout | `0.6916886568` | Monotonic recalibration of raw |
| Rank-lift holdout | `0.7942190170` | **Not gate-eligible**: category-local RBF exemplar similarity model fitted on calibration split; lifts raw `+0.31` without improving Brier/ECE; measures exemplar similarity, not predictor (#695) |
| Brier | `0.0923729688` | Calibration quality (rank-lift transform does not reduce) |
| ECE | `0.0240316074` | Expected calibration error |
| Report sha256 | `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499` | — |

**Three structural defects under the measurement (3-Sherlock audit 2026-05-23, see #712):**

| Layer | Defect | Issue |
|---|---|---|
| Predictor head | Public `Trainer::run_full_training` is hard-coded `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE`; latest aiwonder FSV loaded `/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json` (`optimizer_steps=64`, `weight_delta_observed=true`), but that training/export path is out-of-band from the trainer contract and compiler construction must fail closed without an explicit manifest | #683 |
| Substrate | Legacy v1 forward cache had `e2/e3/e4` collapsed to 1 unique vector each and `e8` byte-identical to `e1`. #704/#714 retire `e2/e3/e4` for static AST; #709/#715 must prove active-slot distinctness and rebuild a v2 9-slot static cache with post-`ff4d4c03` `e8`. | #704, #714, #709, #715 |
| Gate metric | Rank-lift exemplar similarity displaces real predictor output as the gate measurement; the +0.31 lift does nothing for Brier/ECE because exemplar similarity is not a predictor | #695 |

**Substrate-side carries also:** #618 flat-buffer OOD path + #619 flat panel projections (both doctrine §1.5ter violations).

**Honest gap:** `0.95 − 0.4843 = 0.4657` against raw Pearson. The `0.1558` gap against rank-lift is the gap against a composite metric that doesn't measure the predictor.

Earlier row-trace `run-20260520T185945Z` preserves #536 metrics + raw/rank-safe/rank-lift row traces for #549/#392. Calibration uses only the 240-row calibration split; constants limited to structurally degenerate cells; rank-reversing transforms blocked unless calibration has mixed-class support ≥ 8 per class + raw Pearson ≤ -0.25; rank-lift excludes target-side oracle/docker/test labels.

**#536/#547/#548/#549 evidence chain (closed):** #547 proved selected path is split-disjoint. #548 typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED` — holdout-only thresholds lift correlation `0.8076422215` and Brier `0.087037511`, but forbidden. #549 proved live-safe repo/touched-path metadata + calibration-selected raw/rank-safe/rank-lift score fusion is insufficient.

**#392 trajectory chain (ECE-blocked):**

| Slice | Holdout corr | Brier | ECE | Status |
|---|---|---|---|---|
| Aggregate trajectories | `0.8008320332` | — | — | Sparse out-of-fold bins regressed ECE |
| File-path + skill/ability | `0.8422425985` | — | — | — |
| Support-aware mean-bias | `0.8497869968` | `0.0697008073` | `0.0299093761` | Skill coverage explicit: `1531/3000` covered, `1469` uncovered (pass/context-only per #551) |
| Run `run-20260520T214049Z` accuracy | `0.9083333611` | — | — | Exact #536 row-trace reproduction, no mixed-cell threshold degeneracy |

**#554 / #557 / #558 / #560 (closed):** #554 — usefulness-weighted skills carry stronger consequence signal but inactive against old artifact. #558 — rematerialized full v2 label corpus (`399777` chunk rows, `398215` episodes, `157` skills, `9489` memberships), `usefulness_weighted_combined` selected pre-correction at corr `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`, accuracy `0.9083333611`, pass recall `0.9166666865`, fail recall `0.9000000`; report sha256 `2c3c7cea2ced0afc4715bad03624401ea497cfee1260d62ce7f305f5fb11828a`. #560 — split-legal ECE-focused mean-bias repair against #558 DB reproduced same legal best (same metrics; report sha256 `c2505eeb6f755d362510b61d78437f364029deb000363d54715ea3238eec6eee`). #557 — skill usefulness audit surface (source bytes, lifecycle/demotion, generic high-support rejections, blanket-expansion regressions).

Non-promotable: ECE above #536 baseline `0.0240316074`; typed blockers `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED` + `TASK_SKILL_002_USEFULNESS_WEIGHTED_NONREGRESSING_BUT_ECE_BLOCKED` accepted.

### 1.1.1 #551 closeout correction + #392 evidence-semantics hardening (2026-05-21)

#551 resolved as **justified negative-control finding**, not active coverage repair. Aiwonder readback: all `1139/1139` failure-evidence rows skill-covered; uncovered `1469/3000` code states are pass/context-only. Pass/context expansion regresses #392 → not the repair.

| Run | Path | Selected | Corr | Brier | ECE | Accuracy |
|---|---|---|---|---|---|---|
| #392 evidence-fixed-base | `fsv/task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` | `calibration_selected_mean_bias_calibration` | `0.8770877719` | `0.0581485741` | `0.0475411229` | `0.9250000119` |
| #392 evidence-semantics-hardening | `fsv/task-py-g-392-evidence-semantics-hardening-fsv-release/run-20260521T074421Z/` | `calibration_selected_cause_bin_calibration` | `0.8939641118` | `0.0504463837` | `0.0402133949` | `0.9375` |
| #392 final-bin-guarded | `fsv/task-py-g-392-final-bin-guarded-fsv/run-20260521T090040Z/` | (retained, `selection_forbidden=true`) | `0.8941474557` | `0.0503885597` | `0.0432727709` (worse) | — |

Cause-bin remains selected (final-bin improved corr/Brier but worsened ECE).

Latest ship-gate FSV folds #392 active/abstain as an easy-row diagnostic: `205/240` active rows, active corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`. This is not a hidden gate pass: per-cell active metric + rolling-window + statistical-power blockers remain, and #392 is blocked behind the #712 clean-substrate/trainer chain before renewed gate movement.

**Product boundary.** LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`.

**Consequence-prediction contract (#397).** Model observes explicit task text, agent claims, edits, AST deltas, frozen instruments, oracle outcomes, verification feedback. Does NOT infer hidden human intent. Predicted consequences + evidence presented to operator/agent to decide proceed/repair/reroute.

**Live skill/ability surface.** Project files → Python AST chunks → slot-preserving TCT constellations → deterministic live-observable labels → many-to-many chunk-skill memberships → `PredictionLabelContext.active_skill_ids` → consequence prediction. Chunks participate in multiple Level-1/2/3 groups. Memberships are prediction evidence, not intent inference. Future refutation-driven label updates must be verified through the clean #709/#715 substrate and #633 manual SoT gate before being treated as ship evidence.

---

### 1.2 The labeling layer (meaning surface)

Raw slot violations + cross-embedder correlations clustered into **4-level hierarchy of named failure modes**. Product returns named mode that applies, grounded by closest historical exemplar.

**Shipped substrate (do not redo):**

| Layer | Artifact | Rows |
|---|---|---|
| AST chunks | `python_ast_chunks.jsonl` | 399,777 |
| Level-0 labels per chunk (#413) | `chunk_constellation_labels.jsonl` | 399,777 |
| Accepted Level-0 label registry (#413) | `accepted_label_registry.jsonl` | 491 |
| Level-2 named skills (#414) | `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` | 128 |
| Chunk → skill memberships (#414) | `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` | 36,773 |
| Skill → chunk reverse (#414) | `CF_MEJEPA_SKILL_REVERSE_INDEX` | 10,579 |
| Level-1/2/3 hierarchy + named predictions (#404) | `CF_MEJEPA_FAILURE_MODE_LEVEL{1,2,3}_*`, `CF_MEJEPA_NAMED_PREDICTIONS` | 8 groups / 157 skills / 379 constellations / 536 named predictions |

**Holdout proof (#404):** 114/114 actual Fail rows have FailLikely named-mode coverage; closest-exemplar oracle agreement is 1.0 over 113/114 comparable rows. Post-fix active-prediction view: 117/117 active predicted true-Fail rows resolve to FailLikely named modes; aggregate predicted-Fail coverage is 137/150 — 13 false-positive Fail verdicts have no named consequence explanation. Predictor-policy/feature-selection blocker, not #404 hierarchy-substrate failure.

**Live-input safety:** target-side labels (oracle/Docker/test outcomes) emitted with `leakage_policy:target_side_only`. Supervision/reward inputs only — **never** live predictor inputs.

---

### 1.3 Online mistake-driven adaptation

Predictor's head over the panel is wrapped in **online-mistake-driven update loop** (#406, #523, shipped):

1. Predictor returns `Pass | Fail` for candidate patch.
2. Reality (Docker oracle + PostToolUse capture + shift-log replay) returns ground truth.
3. Match → no update.
4. Disagree → **mistake row** in `CF_MEJEPA_MISTAKE_LOG` with full context.
5. Trainer consumes via key-local exact-panel online-head state (`CF_MEJEPA_ONLINE_HEAD_STATE`). Same panel signature returns corrected verdict next time.
6. **Convergence target:** `mistake_repeat_rate ≤ 0.05` over rolling 100-mistake window.

Current protection mode: `exactPanelKeyIsolation`. Unrelated-control delta ≤ 0.02. True Fisher/EWC NOT claimed without durable Fisher rows (#524 guards boundary).

MCP surface (shipped 2026-05-20, #523): `mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status`.

---

### 1.4 Cross-panel triangulation (Goodhart guard)

Predictor trained against 15-slot Panel A. Gτ guard evaluated against same Panel A. Encoder-matched → 0.95 on Panel A could mean the predictor learned the panel.

Panel B (non-overlapping encoders) re-scores every gate-relevant patch:

| Slot | Panel A | Panel B |
|---|---|---|
| AST | tree-sitter → 384-d | CodeBERT → 384-d |
| CFG | attention encoder → 256-d | Pythia program-graph → 256-d |
| Data-flow | def-use attention → 256-d | joern → 256-d |
| Type-graph | tree-sitter type → 256-d | pytype JSON → 256-d |
| Code-semantic | Qodo-Embed-1 → 1536-d | Voyage-code-2 → 1536-d |
| Test/Problem/CommitMsg | e5-large-v2 | bge-large-en-v1.5 |
| Trace/Witness/Oracle/StaticAnalysis/Runtime/Reasoning/Scalars | unchanged | unchanged (deterministic) |

Fail-closed: Panel A ≥ 0.95 AND Panel B < 0.85 → `CROSS_PANEL_GOODHART_DETECTED`. Tracked by **#405 (open p0, blocked)**.

---

### 1.5 Butterfly-effect pathway surface (planned, #416)

Small set of **most-probable futures** — concrete branching pathways from candidate patch, each a conjunction of binary leaves.

**Strict rule: every node is a Q1/Q2/Q5 binary leaf against ground truth.** No ambiguous leaves (Q4, taste judgments).

**Construction:**
1. Seed: candidate patch (post-Q1 binary verdict).
2. Beam search over downstream binary leaves; each leaf = `Q2.oracle_pass | Q5.shift_event`.
3. Beam-width bounded; low-prob subtrees pruned (`P(path) < ε ≈ 0.01`).
4. Surface top 3-5 terminal-state pathways: leaf chain + joint probability + closest exemplar (#413) + named failure mode for Fail leaves (#404) + active skill IDs (#414).
5. Operator picks one. When #335 ships, system reverse-engineers patch-shape constraints that land that pathway.

**Pearl Rung-3 counterfactual rollout** on binary leaves over durable reality substrate. `L3` in JEPA world-model rubric. Converts system from verifier of one candidate into decision-support for reality-shaping.

**Not generative.** Every leaf is a binary prediction the predictor commits on. Every pathway is byte-replayable. Mistake loop credits assignments across pathway leaves on disagreement.

---

### 1.5bis Bidirectional chunk↔skill linkage (#417, shipped read surfaces)

Substrate chain `AST chunks → constellations → accepted labels → skills` is bidirectional. Read-only MCP surfaces shipped 2026-05-19/21:

| Tool | Purpose |
|---|---|
| `mejepa_skill_to_code(skill_id)` | Forward: source bytes of member chunks |
| `mejepa_code_to_skill(chunk_id)` | Reverse: skills this chunk participates in |
| `mejepa_skill_set_query` | Boolean set query over memberships |
| `mejepa_skill_coverage_audit` | Coverage gap audit |
| `mejepa_chunk_as_star` | Chunk as slot-preserving constellation/star |
| `mejepa_constellation_membership` | Constellation membership + neighboring labels |
| `mejepa_skill_impact` | Consequence/blast-radius for a skill |
| `mejepa_skill_graph_inspect` | Ordered chunk/skill graph inspection |
| `mejepa_skill_conflict_graph` | Skill conflict + disagreement graph |
| `mejepa_skill_browse` | Browse named skills with support + lifecycle |

#557 adds usefulness audit: candidate kind, support/fail-pass distribution, lift, genericity, per-cell contribution, source-byte representative sequences, lifecycle, uncovered code-state keys, unknown ordered constellations, rejected/demoted patterns. Keeps stable background scaffolding as context/negative evidence, not promoted failure skill.

Remaining (open in #417): cross-repo skill transfer + pathway-leaf attachment.

Constraint: skills are deterministic groupings of accepted Level-0 labels. NO ambiguous heads. Navigation surface over predictor's training substrate, not new prediction surface.

Runtime resolver (#418, shipped): resolves every live-input-safe membership per chunk; carries memberships into `PredictionLabelContext`; on refutation writes lifecycle audit (update/split/create/demote/no-change).

---

### 1.5ter Array-of-vectors constellation rule (LOCKED IN)

Panel is **array of 15 per-embedder vectors**. NOT a 5,120-d fused embedding. Contiguous 5,120 f32 buffer is byte-layout for serialization only.

**Valid:**
- Same-embedder dim-wise compare: `cosine(panel_A.E_AST, panel_B.E_AST)` ✓
- Array-vs-array per-slot map (vector of 15 same-embedder cosines)
- Strict-conjunction gate over the map
- Per-pair similarity matrix (diagonal only — off-diagonal undefined)
- Cross-slot scalar pairwise: `MI(slot_a, slot_b)`, `correlation(slot_a, slot_b)`
- Matrix-style ops respecting slot identity (per-slot weighted aggregation, masked projections)

**Forbidden:**
- `cosine(panel.E_AST, panel.E1)` (different coordinate systems) ✗
- `linear(5120, hidden)` over packed buffer (silent cross-embedder compensation) ✗
- `residual_norm_sq` over packed 5,120 buffer ✗
- Any operation where strong slot compensates for weak slot

**Code-level enforcement.** `crates/context-graph-mejepa-tct/src/gtau.rs:14` iterates `for embedder in EmbedderId::all()`; `cosine_similarity` at `gtau.rs:90` errors `MEJEPA_TCT_DIM_MISMATCH` on length mismatch. Slot-preserving late-fusion predictor (#387) replaced flat-projection predictor.

---

### 1.5quart Stars as Vectors — celestial-cartography model

Teleological-constellation primitive named after star constellations; analogy formal. See `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`.

| Astronomy | ContextGraph |
|---|---|
| One star = vector in ~100-dim observational space (Gaia DR3) | One AST chunk = array of 15 per-embedder vectors |
| Same-spectrum-band cosine valid | Same-embedder cosine valid |
| Cross-band dim-wise compare = nonsense | Cross-embedder dim-wise compare forbidden |
| Constellations perceived from viewing angle | Constellations perceived from `(mutation_category × language × cell)` |
| Big Dipper = 2D projection of 7 high-dim stars | Code-state constellation = signature over panel array |
| Star foundation model predicts missing parameters | ME-JEPA predicts missing reality outcomes |

**Key:** the constellation isn't in the data, it's in the predictor. The 15-slot panel for a chunk exists independently of any labeling. Named failure modes, skills, code-state constellations are the predictor's perceived patterns. Re-train → constellations change shape. The 0.866-vs-0.95 gap was bottlenecked by what the predictor learned to perceive, not by raw embeddings.

Operator tools (planned, extending #417): `mejepa_chunk_as_star`, `mejepa_constellation_membership`.

**Information-physics framing (peer-reviewed):**

1. **Wheeler "it from bit" (1989)** = binary Q1/Q2/Q5 contract.
2. **Vopson 2nd law of infodynamics (AIP Advances 12, 075310, 2022)** = entropy-decreasing operations (tier compression, slot-preserving fusion). Formalized as trainer loss in #511.
3. **Gates' self-dual ECC in supersymmetry (Doran et al. 2008)** = formal version of anti-compensation. Filed as #512.
4. **FlyWire whole-brain connectome (Nature 634, 2024)** = #388 cross-embedder correlation tensor. #513 tested + demoted as diagnostic-only.
5. **Eon Systems cold-load (March 2026)** = behavior encoded in weights, not transient training state. Filed as #514.

---

### 1.5quint Creativity engine (post-0.95 — autonomous bounded growth)

ME-JEPA is the *engineering* half: verifies binary claims against reality. The *creativity* half — **autonomous embedder synthesis** — proposes new measurement axes. Two oracles gate every proposal:

1. **Novelty oracle** — cross-correlation: candidate orthogonal to existing N? Reject if max correlation > τ_novelty (~0.7).
2. **Utility oracle** — ablation: improves per-cell oracle correlation by ≥ δ_utility (~0.005)?

**Bounded by reality.** Orthogonal informative axes ContextGraph admits is bounded by intrinsic dimensionality of corpus. For code N* ≈ 50-500. System discovers its own N* because novelty oracle rejects every candidate once corpus is fully resolved.

**Five issues (all post-0.95):**

| Issue | Purpose | Priority |
|---|---|---|
| #517 / TASK-CR-002 | Mistake-driven embedder demand signal | p1 |
| #516 / TASK-CR-001 | Novelty + Utility oracle | p2 (p1 immediately post-0.95) |
| #518 / TASK-CR-003 | Embedder retirement audit + Pareto frontier | p1 |
| #519 / TASK-CR-004 | Domain-routed sparse panel | p1 |
| #520 / TASK-CR-005 | Episodic-to-semantic consolidation | p2 |

Together with v2-stretch (#329, #335, #354-357) → **self-resolving observer**.

---

### 1.5sext Neuroscience-inspired reserve (filed 2026-05-20, DO NOT START until current queue concludes)

8 contingency issues (#538-#545) at `priority:p3` with explicit `⚠ DO NOT START` markers. Reassess only after post-queue gap analysis.

**Tier 1 (high-impact, doctrine-compatible, addresses gap directly):**
- #538 predictive-coding head + top-down per-slot prediction + bottom-up residual (Rao & Ballard 1999 / Friston free-energy)
- #539 hippocampal schema-prioritized replay + reverse replay for pathway credit assignment (Mattar & Daw 2018 / Liu et al. 2019)
- #540 hierarchical predictive coding tied to #404 hierarchy (Felleman & Van Essen 1991 / Bastos et al. 2012)
- #541 mixed-selectivity cross-slot binding (Rigotti et al. 2013 / Fusi et al. 2016)

**Tier 2 (design-influence on existing epics):**
- #542 population-dynamics low-dim trajectories for #416 (Shenoy/Churchland/Cunningham)
- #543 sparse distributed representations per slot (Hawkins/Numenta)
- #544 synchrony-based temporal binding for #392 (Singer & Engel / Fries 2005)
- #545 idle-time generative rollouts (default-mode network analog)

All 8 verified doctrine-compatible: slot identity, anti-compensation, binary contract, ambiguity boundary preserved; no inner LLM; no synthetic-input shortcuts. See `08 §6.9`.

---

### 1.6 The ambiguity boundary (locked-in rule)

ME-JEPA only predicts questions with **free binary form against reality**. Free = verdict from observing reality directly, not from imposing arbitrary thresholds on continuous heuristics.

**Excluded from prediction contract:** perf regressions, maintainability, idiomaticity, latent bugs, code redundancy, edge-case coverage, cost, reasoning quality.

**Kept in prediction contract:**

| Surface | Reality |
|---|---|
| Q1 claim-exists | Bytes / AST on disk |
| Q2 oracle pass | Docker oracle `report.json` |
| Q5 shift events | Durable shift-log JSONL |

Q4 heads remain in schema + CFs (`CF_MEJEPA_Q4_*`) as **frozen historical display data**, readable but not trained, calibrated, or consumed.

---

### 1.6bis 2026-05-23 3-Sherlock audit — measurement decomposed, critical path replaced

3-Sherlock parallel forensic audit dispatched 2026-05-23 (reports `docs/SHERLOCK_{PREDICTOR,SUBSTRATE,INFRA}_FINDINGS_2026-05-22.md`) filed 29 new issues (#683-#711), followed by #714/#715 after the static-AST temporal decision. Headline: `0.7942190170` rank-lift decomposes into trainer/checkpoint lineage gap + invalid legacy forward-cache slots + Goodhart-magnifier metric. New pinned context **#712** carries the new ordered critical path:

```
#683 (trainer) → #709+#715 (v2 9-slot cache) → #615 (delete_test_call) →
#618+#619 (doctrine) → #695 (drop rank-lift) →
#685/#687/#688/#690/#684/#689/#699 (kill hardcoded/no-op telemetry) →
resume #392/#511/#405/#379
```

Each rank with file:line evidence + expected impact in #712 body. Boundary preserved: binary contract, slot identity, Q4 freeze, ambiguity boundary, reality-boundary deny-list all intact.

### 1.6ter 2026-05-22 hardening pivot — still active

#629 5-Sherlock audit surfaced ~15 P0 silent-success defects (`07 §2.0`): code returning `success`/`passed=true`/exit-0 without performing work. Examples: `optimizer_witness_chain_verify` returning success for absent `witness-chain.bin` (#608); `store_memory` returning success after sidecar write failure (#599); Rust Trainer emitting pseudo-loss without training a predictor (#596); checkpointed-train echoing oracle labels into live predictions (#597); MCP/CLI eval-run persisting fixture EvalReports via deterministic compiler (#594).

Per **#655** (retire Rust FSV examples → manual SoT guidance, in-progress p0), auto-FSV `*_fsv.rs` harnesses are being retired. Any ship-gate-relevant evidence must come from manual operator-driven SoT readback per `06 §1.9`.

**P0 doctrine/substrate defects (open, gate-blocking):** #683, #618, #619, #621, #709, #715 (see README warning). #670/#704/#714 are closed prerequisite evidence.

**Substrate audit chain (data-substrate completion, 2026-05-22):** #612 → #613 (active source-lineage canonicalized; `0` `/mnt/d` hits) → #614 (forward-cache slot-completeness mask: 378,444 full 12-slot rows + 20,706 partial + 627 zero-slot) → #615 (delete-test-call regeneration: 1,131 deterministic candidates, in-progress) → #616 (weak-cell oracle reruns, open) → #631/#632/#641/#666/#667/#668 (closed).

**New geometry-label epic (#413 successor, in-progress):** #657 TASK-PY-G-136 per-embedder vector-space label discovery bakeoff. Static AST label discovery is now scoped to the clean 9-slot policy (`e1,e6,e7,e8,e9,e10,e12,e13,e14`) after #704/#714; `e2/e3/e4` are temporal/live-data candidates only unless a future issue provides semantic time evidence. Each eligible embedder gets its own deterministic geometry-label map via Leiden-on-kNN / HDBSCAN / spherical k-means / UMAP-diagnostic bakeoff. Downstream: #661 cross-embedder consensus -> #662 gate-safe promotion -> #664 router -> #665 prediction heads.

---

### 1.7 Synthetic-vector remediation (2026-05-19/20) — measurement caveat

Between Phase G ramp and 2026-05-19 22:45 UTC, **inference path** in `crates/context-graph-mejepa/src/compiler.rs::vectors_for` filled instrument-panel slots with SHA256-seeded synthetic vectors via `deterministic_vector(slot, &digest)` instead of real frozen `E*Instrument` encoders. Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452) document + fix this.

**Pre-fix end-to-end inference:**

```
patch + context → SHA256(task_id ++ problem_text ++ patch_sha ++ commit_msg ++ time_step)
                → digest[32]
                → deterministic_vector(slot, &digest)   ← sine-wave from hash
                → Panel (structurally valid; per-slot hashed nonsense)
                → MeJepaPredictor::new()                ← untrained random weights
                → "verdict"
                → promote_best wrote MEJEPA_BEST_STUB_V1 text to best.safetensors
                → ship-gate claimed "0.866667 prediction_oracle_correlation"
```

Deterministic for same `(task_id, patch_sha)`. Every ablation moved compute, never input signal → every variant converged to same `0.8666666746139526` fixed point.

**Code-level fixes (shipped):**

| Commit | Fix |
|---|---|
| `e2c51669` | `vectors_for` → `build_inference_panel_inputs` → real `E*Instrument` encoders (`compiler.rs:1410-1418`) |
| `5330629e` | Project cache fails closed on fake embeddings; no more `deterministic_vector_blob` |
| `a0849ac3` | Project ingest wired to active embedder forward providers |
| `398e5216` | Multi-array provider honors active model registry |
| `db83923a` | `MEJEPA_BEST_STUB_V1` text-stub checkpoint rejected at load (`predictor_checkpoint.rs:147-158, 239-250`); manifest-verified trained checkpoint pipeline added |
| `d068d2ed` | Trainer consumes native active-embedder ragged constellations (#452) |
| `f982f29d` | Runtime fail-closed if cannot load verified trained checkpoint |

**Survived structurally (retained evidence; clean ship substrate still requires #709/#715):**

| Artifact | Why real |
|---|---|
| 399,777 AST chunks (`python_ast_chunks.jsonl`) | Deterministic AST chunker over real SWE-bench Lite Python source |
| 391-shard packed forward cache | Real Candle CUDA forward over real chunks, `precision_class: "candle_real_forward"`, ~24h GPU, SHA-pinned safetensors, fail-closed guards at `crates/context-graph-mejepa-embedders/src/forward.rs:122,207,349,422`; retained as historical evidence only until #709/#715 rebuilds v2 because legacy static AST slots `e2/e3/e4` collapsed and legacy `e8` aliased to `e1` |
| #388 cross-embedder correlation tensors | Deterministic derivation from real forward cache |
| #389 per-slot calibration + OOD policy | Built from #388 + real training rows |
| #413 Level-0 label compiler (491 accepted, 399,777 chunk labels) | Deterministic compiler over real chunks + v1 cache; retained for lineage, recompute on #709/#715 v2 cache before gate claims |
| #414 skill substrate (128 / 36,773 / 10,579) | Derived from #413; retained for lineage, recompute/validate after v2 labels |
| #404 hierarchy (8 / 157 / 379 / 536 named predictions) | Derived from #413 + #414; retained for lineage, not a clean-v2 label proof |
| Anti-compensation FSV (#394) | Invariant; doesn't depend on input quality |
| Runtime ability resolver (#418) | Pure resolver logic |
| Mistake-loop machinery (#406 / #523) | Storage + MCP wrappers; needs real predictor mistakes to validate convergence |

**Invalidated (must rebaseline):**

| Artifact | Why invalid |
|---|---|
| Ship-gate `0.866667` | Measured against synthetic-input inference |
| `λ_inv ∈ {0, 0.25, 0.5, 1.0}` ablation (all `0.8666666746139526`) | Identical because input deterministic-from-hash |
| #393 capability-discovery + #398 calibrated reruns | Real-input reruns exist, remain `no_promotion` |
| "Slot-preserving doesn't close the 0.083 gap" claim | Real-input #393/#398/#546 reruns are negative evidence, not pre-fix validation |
| "Gap is a function of labels, not embedders/storage" claim | Invalid as broad causal claim |
| #129 OOD serving gate AUC | Revalidated post-fix in `fsv/task-py-g-014-ood-gate-fsv/run-20260521T145048Z/` |
| #130 Gτ guard audit pass/fail counts | Revalidated post-fix in `fsv/task-py-g-015-gtau-guard-audit-fsv/run-20260521T142921Z/`; fail-closed/non-countable |

**Rebaseline action plan (current P0):**

1. ✅ `swebench_300x8_ship_gate_fsv` end-to-end on aiwonder with real instruments and an out-of-band trained checkpoint. Current: `fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`; not public-Trainer-countable until #683 closes.
2. ✅ `λ_inv` bidirectional ablation real inputs. Current: `fsv/task-ek-012-bidirectional-ablation-realinput-fsv/run-20260520T172657Z/`.
3. ✅ #393 / #398 against real-input. Current: `fsv/task-py-g-093-constellation-ablation-suite-realinput-fsv/run-20260520T181900Z/` + `fsv/task-py-g-097-constellation-ablation-with-calibration-realinput-fsv/run-20260520T170156Z/`.
4. ✅ Out-of-band trained-weight manifest: `trained_weight_sha256 != initial_weight_sha256`; public Trainer countability remains #683.
5. ✅ Repair first weak-cell slice from #535/#536. #548/#549 proved remaining static mixed-cell signal calibration-underpowered; #392/#558/#560 file/skill trajectories stronger but ECE-blocked. Next: per-cell/per-constellation ECE repair, `swap_variable` skill specialization, consequence-to-cause traces, Panel B validation.
6. ✅ Revalidate post-#536 calibration disjointness (#547): `tau=20.0, fail_boost=1.5` calibration-selected, row/feature-member overlaps zero, #413 label-lineage hashes match, target-side labels excluded.

Tracked under #529; gate-active critical path is now `08 §3.−1`, with #392/#511/#405/#379 blocked behind #683, #709/#715, #615, #618/#619, #695, no-op telemetry cleanup, and #633/#655/#656/#657 evidence gates.

---

## 2. Process / Port / Tech Architecture

| Process | Port / IPC | Technology | Purpose |
|---|---|---|---|
| `context-graph-mcp` server | stdio JSON-RPC (default) or HTTP | Rust, tokio, async-trait | 216-tool MCP surface; daemon mode |
| `context-graph-cli` | CLI invocation | Rust | `mejepa observe`, `mejepa project-ingest`, `mejepa compression-progress`, `mejepa active-learning`, etc. |
| ME-JEPA training daemon | embedded in CLI/binary | candle (CUDA 13.2), RocksDB | UTML training loop; reads `CF_MEJEPA_REPLAY_BUFFER`; writes `CF_MEJEPA_TRAIN_CERTS` |
| Shift-log subscriber | tokio task | tokio | Tails JSONL shift log → materializes panels |
| Heal scheduler | embedded thread | tokio | Drift detection, Fisher/EWC, distill, promotion |
| RocksDB (per runtime root) | local disk | RocksDB ≥ 8.x | 200+ column families |
| GPU runtime | CUDA 13.2 on RTX 5090 Blackwell | cudarc 0.19.x via candle 0.10.2 | Embedder forward passes, predictor training |

**Constraints:**
- **No HTTP from action paths.** MCP server is only network surface.
- **No inner LLM.** Local generative inference (Qwen / Hermes / Ollama / llama.cpp) compile-locked off — `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`.
- **No intent synthesis.** Observable goals + agent claims are evidence; hidden human intent not a prediction target.

---

## 3. Technology Stack

| Layer | Technology | Version |
|---|---|---|
| Language | Rust | edition 2021, MSRV 1.95 |
| Async runtime | tokio | 1.35 |
| Serialization | serde, serde_json, bincode 1.3, toml 0.8 | — |
| Error | thiserror, anyhow | — |
| Telemetry | tracing, tracing-subscriber (json/env-filter) | — |
| ML runtime | candle-core, candle-nn (CUDA) | 0.10.2 (cudarc 0.19.x, CUDA 13.2) |
| Storage | RocksDB | per-crate manifests |
| Crypto / hashing | ed25519, sha2, blake3 | — |
| Vector search | usearch HNSW, hnsw_rs | — |
| License | PolyForm-Noncommercial-1.0.0 | — |

Build profiles (`Cargo.toml:112-128`): `release` (thin LTO, 16 CGUs, strip), `release-max` (1 CGU), `dev` (opt-0, line-info debug, incremental).

---

## 4. Workspace Members (22 active crates)

| Crate | Role |
|---|---|
| `context-graph-mcp` | MCP JSON-RPC server + 216 tools (69 `mejepa_*`) |
| `context-graph-cli` | Operator CLI + helpers (`context-graph` short alias) |
| `context-graph-core` | Shared domain types |
| `context-graph-storage` | RocksDB wrapper + teleological/code/causal CFs |
| `context-graph-witness` | ed25519 + Merkle + Stage 0/F certificates |
| `context-graph-solver` | Causal masking, MMR, conformal predictor |
| `context-graph-mincut` | Clean-room Stoer-Wagner global min-cut |
| `context-graph-paths` | Fail-closed durable path resolver (aiwonder enforced) |
| `context-graph-tier-compression` | 8/7/5/3-bit groupwise scalar quantization |
| `context-graph-embeddings` | E1-E14 embedding pipeline |
| `context-graph-cuda` | CUDA helpers |
| **`context-graph-mejepa`** | **Predictor trunk** (73 modules) |
| `context-graph-mejepa-cf` | Canonical CF name registry (zero-dep) |
| `context-graph-mejepa-corpus` | 8× mutation operators + SWE-bench bridge |
| `context-graph-mejepa-instruments` | 15-slot panel + frozen instruments |
| `context-graph-mejepa-embedders` | 21 embedder slots, dynamic registry, calibration |
| `context-graph-mejepa-train` | UTML trainer, replay buffer, mistake loop, certificates |
| `context-graph-mejepa-tct` | Teleological constellations + Gτ guard |
| `context-graph-mejepa-hygiene` | Tier transitions, quota, GC, witness compress |
| `context-graph-mejepa-shift-subscriber` | JSONL shift-log tail → panel materialization |
| `context-graph-dynamicjepa` | 5090jepa MCP surface (39 tools) |
| `context-graph-test-utils` | Deterministic-by-shape helpers for storage/serialization tests only; NOT ME-JEPA gate evidence |

Full per-crate detail in `02_source_map.md`.

---

## 5. Entry Points

| Binary | Purpose |
|---|---|
| `context-graph-mcp` | MCP server. `--mode stdio` (default) or `--mode reality-loop` (subset). Multi-agent observability via `mejepa_daemon_status`. |
| `context-graph-cli` / `context-graph` | Operator CLI. `mejepa observe`, `mejepa project-ingest`, `mejepa project-report`, `mejepa compression-progress`, `mejepa active-learning {list\|label\|dismiss}`. |
| `mejepa` | Standalone predictor harness. |
| `mejepa-heal-drill` | Heal scheduler drill. |
| `mejepa-train` | Trainer entry. |
| `train-causal` | Causal embedder training. |
| 210+ FSV examples (`crates/*/examples/*_fsv.rs`) | Per-task FSV harnesses. See `07_results_and_benchmarks.md`. |

---

## 6. Directory Layout

```
crates/                                — Rust workspace (22 active crates)
docs/futurebuild/                       — this doc suite
docs/futurebuild/specs/FSV-PROTOCOL.md  — mandatory verification contract
docs2/AICodingAgentSuperPrompt.md       — master doctrine
docs2/hardeningprompt.md                — 14-axis hardening deep-dive
scripts/cc/                             — Claude Code hook scripts
scripts/ci/                             — CI lint (e.g. check_no_c_drive_paths.sh)
scripts/swebench_lite_ops/              — Python redaction + SWE-bench Lite helpers
tasks/swebench-lite/                    — task manifest JSON
config/                                 — governed paths, model pins
.claude/                                — settings.json, agents/, loop.md
.github/workflows/, .github/labels.yml  — CI + label set (memory = Issues)

# aiwonder production durable SoT
/zfs/archive/contextgraph/
  corpus/                               — production corpus, panel cache, fingerprint registry
  fsv/                                  — per-task FSV evidence (post-migration target)
  runtime/                              — shift log, test outcomes, scheduler events
  projects/<project_id>/                — reality-compiler per-project cache, manifest, report
  state/                                — durable runtime state (calibration, labels, q4-trust-gate)
  exports/                              — weekly eval reports, operator exports

# aiwonder hot state
/zfs/hot/contextgraph/
  repo/                                 — git checkout, release binaries
  models/                               — embedder model weights
  corpus/                               — stable hot inputs (chunks.jsonl)
  runtime/                              — active scheduler logs / hot run state
```

---

## 7. Storage Tier Classification

| Tier | What | Where | Regenerable? |
|---|---|---|---|
| **Sacred** (never edit) | `.env`/`.env.*`; `swebench/**`; `tests/oracle_**/**`; `crates/*/src/reward_specs/**`; `crates/*/src/verifiers/**`; `*.pem`, `*.key`, `**/credentials.json` | various | No |
| **Durable production** | RocksDB CFs (~200); FSV evidence; project-ingest manifests; weekly exports | `/zfs/archive/contextgraph/` | Source-recoverable (corpus) or replay-from-shift-log (predictions). Many FSV artifacts irreplaceable single events. |
| **Hot** | Active checkpoints, model weights, hot run state | `/zfs/hot/contextgraph/` | Yes (re-download / rebuild) |
| **Ephemeral** | `/tmp/contextgraph-*` scratch | `/tmp/` | Yes |

Legacy mounted-drive roots (`/mnt/d/contextgraph/`) — **provenance only**, not active data roots.

---

## 8. Exception Hierarchy (fail-closed structured errors)

Every crate has `Error` enum with `SCREAMING_SNAKE_CASE` code via `.code()`:

| Crate | Sample codes |
|---|---|
| `context-graph-mejepa-instruments` | `MEJEPA_INSTRUMENTS_INVALID_INPUT`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`, `MEJEPA_INSTR_FROZEN_VIOLATION` |
| `context-graph-mejepa-corpus` | `MEJEPA_CORPUS_INVALID_INPUT`, `MEJEPA_CORPUS_NO_MUTATION_SITE`, `MEJEPA_CORPUS_LEAKAGE_DETECTED`, `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT` |
| `context-graph-mejepa` | `MEJEPA_INSTRUMENT_GRADIENT_LEAK`, `OOD_CALIBRATOR_MISSING`, `OOD_SCORE_ABOVE_THRESHOLD`, `MULTI_HEAD_CONTRADICTION`, `CONTRADICTION_THRESHOLD_MISSING`, `CROSS_PANEL_GOODHART_DETECTED`, `PATHWAY_AMBIGUOUS_LEAF_REJECTED` |
| `context-graph-mejepa-tct` | `MEJEPA_TCT_DIM_MISMATCH` |
| `context-graph-mcp` | `CCREALITY_ENGINE_RETIRED` (retired tool callers fail closed) |

**Fail-closed contract.** Missing CFs/files/models/env vars/malformed inputs MUST return structured error with `.code()`. No silent defaults.

---

## 9. Major Subsystems

**MCP server (`context-graph-mcp`).** JSON-RPC 2.0 stdio (default) or HTTP. 216 tools (69 `mejepa_*` product surface + learning, UTL, constellation, entity, embedder, graph, sequence, file watcher, training, provenance, daemon). Retired `reality_*` tools fail closed with `CCREALITY_ENGINE_RETIRED`. Enforces multi-agent isolation via path-policy + edit-lock.

**Predictor (`context-graph-mejepa`).** CUDA-only production. 73 modules: claim graph (Q1), patch-similarity HNSW exemplars (Q3), Q5 shift-log replay impact, multi-head VICReg + Huber loss, conformal calibration, OOD detection, project-ingest reality compiler, synthetic-stress evaluator, adversarial corpus, dynamic embedder registration, foundationality (PageRank), operator override. `compile()` + `compile_with_fingerprint_observation()` are canonical Q1-Q5 entry points.

**Frozen instruments (`context-graph-mejepa-instruments`).** `Panel` is array of 15 per-embedder vectors: `E_AST (384)`, `E_CFG (256)`, `E_DataFlow (256)`, `E_TypeGraph (256)`, `E_Test (384)`, `E_Trace (512)`, `E_Diff (256)`, `E_Witness (256)`, `E_Oracle (128)`, `E_Problem (1024)`, `E_CommitMsg (384)`, `E_StaticAnalysis (256)`, `E_Runtime (256)`, `E_Reasoning (384)`, `Scalars (128)`. 5,120 f32 contiguous buffer is byte layout only. Cross-embedder dim-wise compare forbidden. Instruments carry zero trainable params (`ΔΞ.target_collapse = 0`).

**Embedders (`context-graph-mejepa-embedders`).** 21 slots; 12 active content (8 pretrained + 4 algorithmic). Frozen-by-design: E1, E6, E7, E8, E10, E12, E13, E14. Dynamic-registration with VRAM budget + falsification + freeze + adversarial-injection mitigation. SHA-256 manifest pins. CUDA-required.

**Corpus generator (`context-graph-mejepa-corpus`).** 8 mutation × 11 languages = 88 generic operators (Python first-class via parser-backed; other 10 deferred). Categories: `KnownGood`/`SubtleFlip`/`OffByOne`/`SwapVariable`/`DeleteTestCall`/`WrongFile`/`OverEngineer`/`CompileError`. SWE-bench Lite bridge wraps `python -m swebench.harness.run_evaluation` per-instance. Split modes: `InstanceAtomic` (default, 240/30/30) + `RepoAtomic`.

**Trainer (`context-graph-mejepa-train`).** UTML v1.0. `compute_l_step` evaluates `L_step = ΔP × ΔK × ΔΩ × ΔΞ`, but public `Trainer::run_full_training` is not countable ship training until #683 unifies it with the real checkpoint/export path. EWC++ Fisher, continual backprop (Dohare 2024), and prioritized replay remain intended trainer architecture, not gate-countable evidence until #683/#689/#593/#699/#643 close. **Mistake loop consumer (#406/#523):** shipped storage/MCP plus exact-panel online-head replay; general mistake-driven learning and durable Fisher/EWC protection are not claimed until the Trainer contract and telemetry surfaces are repaired.

**TCT (`context-graph-mejepa-tct`).** Teleological constellations + Gτ guard. Cell key = `(mutation_category, language, additional_dims)`. Per-cell calibration, freshness audit, refresh scheduler. Gτ rejects predictions with insufficient cell support.

**Hygiene (`context-graph-mejepa-hygiene`).** Tier transitions (hot→warm→frozen), quota enforcement, GC, witness-chain Merkle compression. MCP: `mejepa_quota_status`, `mejepa_gc_run`, `mejepa_witness_compress`.

**Shift subscriber.** Tokio task tailing `/zfs/archive/contextgraph/runtime/shift-log.jsonl`. Decodes `ShiftEntry` → `shift_to_inference()` → materializes `Panel` → writes `CF_MEJEPA_PANELS` + `CF_MEJEPA_SHIFT_WATERMARK`. Watermark is durability gate.

**Storage (`context-graph-storage`).** RocksDB wrapper. ~200 CFs (51 base + 120 ME-JEPA + 19 teleological + 13 quantized + 5 code + 2 causal). HNSW per embedder; inverted index for sparse embedders.

**Witness (`context-graph-witness`).** Tamper-evident chain. ed25519 segment signing, Merkle aggregation, Stage 0 + Stage F + Merkle certificates with full metadata binding.

---

## 10. What This System Is NOT

- Not a generative inference engine — no inner LLM. Product is verifier, not code-writer.
- Not a benchmark harness — pre-pivot benchmark crate retired.
- Not a hosted service — runs locally on operator hardware (aiwonder for production).
- Not a Python codebase — Python only shows up as corpus subject (SWE-bench Lite) + tool wrappers.
- Not cross-language at ship-gate time — current gate Python-only (300×8).
- Not HTTP-callable from action paths.

---

## Cross-references

| Topic | Doc |
|---|---|
| Per-crate source map + dependency graph | `02_source_map.md` |
| Every RocksDB CF with key/value format | `03_storage_schema.md` |
| Every MCP tool with parameters and returns | `04_mcp_tools.md` |
| Embedder slots / instruments / heads / TCT / corpus / heal / mistake loop / training | `05_predictor_pipeline.md` |
| FSV protocol, runbook, DR, observability, test suite | `06_operations_and_fsv.md` |
| FSV evidence history, ship-gate progress, ablations | `07_results_and_benchmarks.md` |
| Current open work, critical path, post-ship plans | `08_roadmap_and_open_work.md` |
| Doctrine (universal protocol) | `docs2/AICodingAgentSuperPrompt.md` |
| Project addendum (invariants, deny-list) | `CLAUDE.md` |
| Per-turn agent contract | `AGENTS.md` |
