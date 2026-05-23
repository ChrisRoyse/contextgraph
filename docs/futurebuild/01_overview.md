# 01 — System Overview

**Status:** Phase G in flight, but **gate-moving work is suspended as of 2026-05-22**. The operator opened **#633 / [P0] Manual full-system FSV and hardening gate before new work** following a 5-Sherlock parallel forensic audit (#629 closeout) that surfaced silent-success / fake-FSV / doctrine-violation defects across the entire product surface. Until the 17 child sub-tasks (#634-#651) RESOLVED/BLOCKED with manual SoT readback evidence, no #392 / #511 / #405 / #379 work is admissible. Aiwonder ZFS root migration **#372 CLOSED** (`/zfs/archive/contextgraph` → `archive/contextgraph` ZFS, `/zfs/hot/contextgraph` → `hotpool/contextgraph` ZFS). The post-fix `0.7942190170` rank-lift measurement (and every claim built on it) is provisional pending manual revalidation. Phases A–F + H code/FSV-shipped.

See `README.md ⚠ 2026-05-22 hardening pivot` and `08_roadmap_and_open_work.md §0.4` for the active gate, `06_operations_and_fsv.md §1.9` for the manual-SoT contract that replaces auto-FSV, and `07_results_and_benchmarks.md §2.0` for the 5-Sherlock closure index.

Source files: `Cargo.toml`, `CLAUDE.md`, `AGENTS.md`, `crates/context-graph-mcp/src/lib.rs`, `crates/context-graph-mejepa/src/lib.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`, `crates/context-graph-mejepa-instruments/src/lib.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs`, `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-mcp/src/tools/definitions/mod.rs`.

---

## 1. What This System Is

**ME-JEPA-Code** is a **binary reality predictor**. AI coding agents (Claude Code / Codex / Cursor) call it via a local MCP JSON-RPC server. For any candidate code change it returns:

```
verdict ∈ { Pass, Fail }    // "Will this change exist in working reality? Yes or No."
+ named_failure_mode         // (when Fail) labeled cluster from the failure-mode hierarchy (§1.2)
+ closest_exemplars          // top-K historical patches from the 300×8 corpus
+ violating_slots            // per-slot OOD reasons + cross-embedder contradiction evidence
+ mistake_id (when wrong)    // ground-truth observation lands in CF_MEJEPA_MISTAKE_LOG and feeds online learning
```

There is one prediction per candidate. There is no best-of-K. There is no `Abstain` at the product surface. The system must be right the first time; when wrong, the mistake is captured and the predictor adapts.

The five Q-questions are **binary predictions** (Q1, Q2, Q5), an **explanation surface** (Q3), and **out-of-scope** (Q4):

| Q | Status |
|---|---|
| **Q1** | **Binary prediction.** Did the AI's claim physically land in the bytes/AST? Reality = bytes on disk + AST diff. |
| **Q2** | **The binary predictor itself.** Output is `Verdict::Pass\|Fail`. Reality = Docker oracle (`swebench.harness.run_evaluation`). |
| **Q3** | **Explanation surface only.** Named failure mode + closest exemplar attached to a `Fail` verdict. Labels Q2; does not make its own prediction. |
| **Q5** | **Binary prediction.** For each predicted side effect: did the corresponding shift-log event occur? Reality = `/zfs/archive/contextgraph/runtime/shift-log.jsonl`. |

**Why Q4 is dropped, not thresholded.** Continuous quantities (perf wall-clock, mutation survival rate) only become binary via arbitrary thresholds. The threshold is heuristic, not reality. Mistake-loop convergence requires real disagreement with reality, not disagreement with a heuristic.

**Ship-gate predicate:**

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per mathematically-valid (mutation_category × language) mixed cell
  with degenerate all-pass/all-fail cells gated by the #532 replacement contract
  on the 300×8 SWE-bench Lite Python corpus
  using Q2 Docker-oracle agreement as the binary verdict
  AND cross-panel oracle correlation ≥ 0.95 on Panel B (#405)
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Degenerate-cell metric contract (#532).** Pearson correlation is used only when the oracle target has both Pass and Fail rows. Single-class cells such as `compile_error::python` and `wrong_file::python` have zero oracle variance, so the FSV gates them with target-class recall ≥ 0.95, actual-class confidence ≥ 0.95, Brier ≤ 0.05, ECE ≤ 0.05, and explicit power diagnostics (Fisher-z CI for mixed cells; Wilson lower bound for degenerate recall). The report must never emit a bare "correlation unavailable" blocker without this replacement metric.

**Current real-input measurement (#536/#547/#548/#549/#392/#554/#558/#560 — provisional, requires #633 re-verification).** Latest aiwonder ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` reports raw holdout `prediction_oracle_correlation = 0.4843192995`, rank-safe calibrated holdout `0.6916886568`, and real code-state-constellation rank-lift holdout `0.7942190170` with Brier `0.0923729688` and ECE `0.0240316074`; report sha256 `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499`. **This number itself rides on a substrate with known-broken paths** (#618 flat-buffer OOD, #619 flat panel projections, #670 invalid `e2/e3/e4/e8` spaces) and was produced by auto-FSV harnesses that #655 is retiring; treat as the best available evidence, not as gate-eligible until manually re-verified under #650. This preserves the #536 binary baseline while adding first-class #392 active/abstain diagnostics. Earlier row-trace run `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T185945Z/` preserves #536 metrics and adds row-level calibration/holdout raw, rank-safe, and rank-lift prediction traces for #549. Calibration uses only the 240-row calibration split, constants are limited to structurally degenerate cells, rank-reversing transforms are blocked unless calibration evidence has enough mixed-class support and strong negative Pearson evidence, and rank-lift excludes target-side oracle/docker/test labels. #547 proved the selected path is split-disjoint. #548 proved `MIXED_CELL_CALIBRATION_UNDERPOWERED`: holdout-only thresholds can lift correlation (`0.8076422215`) and Brier (`0.087037511`), but are forbidden. #549 then proved live-safe repo/touched-path metadata and calibration-selected raw/rank-safe/rank-lift score fusion are still insufficient. #392 aggregate trajectories first lifted to `0.8008320332`; file-path plus skill/ability trajectories lifted to `0.8422425985`; sparse out-of-fold probability bins regressed ECE. The hardened support-aware slice at `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T214049Z/` selected a split-legal global mean-bias correction over the affine file/skill surface: holdout correlation `0.8497869968`, Brier `0.0697008073`, ECE `0.0299093761`, accuracy `0.9083333611`, pass recall `0.8916666508`, fail recall `0.9250000119`, exact #536 row-trace reproduction, and no mixed-cell threshold degeneracy. #554 then proved targeted skill usefulness carries stronger consequence signal but was ECE-blocked against the old artifact. #558 rematerialized the full v2 label corpus (`399777` chunk rows, `398215` episodes, `157` skills, `9489` memberships, materialization sha256 `27d24719ff2f3c310f1a757d9f5bf142be712c9deae0049b7cc83439cd66d6a0`) and reran #392 with that DB. Before the later #551/#392 correction, the selected #392 policy was `usefulness_weighted_combined`: holdout correlation `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`, accuracy `0.9083333611`, pass recall `0.9166666865`, fail recall `0.9000000`, trajectory report sha256 `2c3c7cea2ced0afc4715bad03624401ea497cfee1260d62ce7f305f5fb11828a`. #560 added a split-legal ECE-focused calibration diagnostic and retained report sha256 `c2505eeb6f755d362510b61d78437f364029deb000363d54715ea3238eec6eee`; it selected the same model and did not reduce ECE. This was the strongest split-legal trajectory signal before the later #551/#392 correction, but it remains non-promotable because aggregate ECE remains above the #536 baseline `0.0240316074`; typed blockers `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED` and `TASK_SKILL_002_USEFULNESS_WEIGHTED_NONREGRESSING_BUT_ECE_BLOCKED` remain accepted. The FSV still records skill coverage explicitly: `1531 / 3000` expected code-state keys are covered and `1469` are uncovered; #551 later resolves that gap as pass/context-only, not missing failure coverage. The next work must reduce ECE with per-cell/per-constellation consequence-to-cause evidence, cross-window validation, or true-mixed features, without sparse bin overfit, score blending, monotone logit calibration, ECE-overweighted selection, support-only shrinkage, blanket skill expansion, or holdout threshold tuning. `compile_error` rows use raw invalid-source diagnostic panels instead of the carrier workaround. The remaining ship gap is robust calibration plus Panel B cross-check, not the invalid pre-fix `0.866667` synthetic-vector number.

**#551 closeout correction + #392 evidence-semantics hardening.** #551 is no longer an active missing-coverage blocker. Aiwonder readback against the label root, failure-evidence rows, and skill reverse index showed all failure evidence is covered (`1139 / 1139`); the uncovered `1469 / 3000` code states are pass/context-only. Pass/context expansion by support cap regressed #392, and the corrected #392 rerun at `/zfs/archive/contextgraph/fsv/task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` shows skill-kind evidence-consequence is worse once the double-calibration bug is removed. The #392 evidence-semantics run at `/zfs/archive/contextgraph/fsv/task-py-g-392-evidence-semantics-hardening-fsv-release/run-20260521T074421Z/` standardizes cause-residual features, proves cause-bin calibration actually applies to rows, broadens active path rejection, and selects cause-bin calibration: correlation `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`, report sha256 `edf4755f44ccb31a3eaafc5ad07f842e832e9cd684b5838cde53eba8715f6366`. The follow-on final-bin diagnostic at `/zfs/archive/contextgraph/fsv/task-py-g-392-final-bin-guarded-fsv/run-20260521T090040Z/` is retained as a negative result with explicit selection guard: final-bin slightly improved correlation/Brier (`0.8941474557` / `0.0503885597`) but worsened ECE (`0.0432727709`) and is `selection_forbidden=true`, so cause-bin remains legal best. The latest ship-gate FSV at `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` integrates #392 active/abstain as a diagnostic surface: active rows `205 / 240`, active coverage `0.8541666865`, corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`. It is not a full gate pass because per-cell active metrics, rolling windows, and statistical-power checks still fail. The next repair is targeted active-cell/window/power repair, not coverage expansion, final aggregate binning, or holdout tuning.

**Product boundary.** LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, and docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`.

**Consequence-prediction contract (#397).** The model observes explicit task text, agent claims, edits, AST deltas, frozen instruments, oracle outcomes, and verification feedback. It does NOT infer or classify hidden human intent. Predicted consequences + evidence are presented to the operator/agent so they decide whether to proceed, repair, or pick a different route.

**Live skill/ability surface.** When ME-JEPA compiles a live project: project files → Python AST chunks → slot-preserving TCT constellations → deterministic live-observable labels → many-to-many chunk-skill memberships → `PredictionLabelContext.active_skill_ids` → consequence prediction. A chunk can participate in multiple Level-1 groups, Level-2 skills, and Level-3/higher abilities. Memberships are prediction evidence, not intent inference. On refutation, the mistake loop updates/splits/creates/demotes the involved skill/ability.

---

### 1.2 The labeling layer (the meaning surface)

Raw slot violations + cross-embedder correlations are clustered into a **4-level hierarchy of named failure modes**. The product returns the named mode that applies, grounded by the closest historical exemplar.

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

Holdout proof (#404): 114/114 actual Fail rows have FailLikely named-mode coverage; closest-exemplar oracle agreement is 1.0 over 113/114 comparable holdout Fail rows. Post-fix revalidation against `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` also shows 117/117 active predicted true-Fail rows resolve to FailLikely named modes, while aggregate predicted-Fail coverage is 137/150 because 13 false-positive Fail verdicts have no named consequence explanation. That is a predictor-policy/feature-selection blocker, not a #404 hierarchy-substrate failure.

Live-input safety: target-side labels (oracle/Docker/test outcomes) are emitted with `leakage_policy:target_side_only`. They are supervision/reward inputs only — **never** live predictor inputs.

---

### 1.3 Online mistake-driven adaptation

The predictor's head over the panel is wrapped in an **online-mistake-driven update loop** (#406, #523, shipped):

1. Predictor returns `Pass | Fail` for a candidate patch.
2. Reality (Docker oracle + PostToolUse capture + shift-log replay) returns ground truth.
3. Match → no update.
4. Disagree → **mistake row** in `CF_MEJEPA_MISTAKE_LOG` with full context (panel, slot trace, named failure mode predicted vs ground-truth, accepted label IDs, active skill IDs).
5. Trainer consumes via key-local exact-panel online-head state (`CF_MEJEPA_ONLINE_HEAD_STATE`). Same panel signature returns the corrected verdict on next presentation.
6. **Convergence target:** `mistake_repeat_rate ≤ 0.05` over rolling 100-mistake window.

Current protection mode: `exactPanelKeyIsolation`. Unrelated-control delta ≤ 0.02 measured. True Fisher/EWC protection is not claimed without durable Fisher rows (#524 guards the boundary).

MCP surface (shipped 2026-05-20, #523): `mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status`.

---

### 1.4 Cross-panel triangulation (the Goodhart guard)

The predictor is trained against the 15-slot Panel A. The Gτ guard is evaluated against the same Panel A. Encoder-matched by construction → 0.95 on Panel A could mean the predictor learned the panel.

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

Ship-fail-closed: Panel A ≥ 0.95 AND Panel B < 0.85 → `CROSS_PANEL_GOODHART_DETECTED`. Tracked by **#405 (open p0, blocked)**.

---

### 1.5 The butterfly-effect pathway surface (planned, #416)

What the system delivers on top of the binary primitive: a small set of **most-probable futures** — concrete branching pathways from a single candidate patch, each pathway a conjunction of binary leaves.

**Strict rule: every node is a Q1/Q2/Q5 binary leaf against ground truth.** No ambiguous leaves (Q4, taste judgments) enter the tree.

**Construction (planned, #416 / TASK-PY-G-119):**

1. Seed: candidate patch (post-Q1 binary verdict).
2. Beam search over downstream binary leaves; each leaf = `Q2.oracle_pass | Q5.shift_event`.
3. Beam-width bounded; low-probability subtrees pruned (`P(path) < ε ≈ 0.01`).
4. Surface top 3-5 terminal-state pathways: leaf chain + joint probability + closest historical exemplar (from #413 registry) + named failure mode for any Fail leaf (from #404 hierarchy) + active skill IDs (from #414).
5. Operator picks one. System (when #335 ships) reverse-engineers patch-shape constraints that land that pathway.

This is the **Pearl Rung-3 counterfactual rollout** on binary leaves over the durable reality substrate. It is `L3` in the JEPA world-model rubric. It converts the system from a verifier of one candidate into decision-support for reality-shaping.

**Not generative.** Every leaf is a binary prediction the predictor commits on. Every pathway is byte-replayable. The mistake loop credits assignments across pathway leaves on disagreement.

---

### 1.5bis Bidirectional chunk↔skill linkage (#417, shipped read surfaces)

The substrate chain — `AST chunks → constellations → accepted labels → skills` — is bidirectional at every step. Read-only MCP surfaces shipped 2026-05-19/21:

| Tool | Purpose |
|---|---|
| `mejepa_skill_to_code(skill_id)` | Forward: source bytes of member chunks |
| `mejepa_code_to_skill(chunk_id)` | Reverse: skills this chunk participates in |
| `mejepa_skill_set_query` | Boolean set query over memberships |
| `mejepa_skill_coverage_audit` | Coverage gap audit |
| `mejepa_chunk_as_star` | Chunk as slot-preserving constellation/star |
| `mejepa_constellation_membership` | Constellation membership and neighboring labels |
| `mejepa_skill_impact` | Consequence/blast-radius read surface for a skill |
| `mejepa_skill_graph_inspect` | Ordered chunk/skill graph inspection |
| `mejepa_skill_conflict_graph` | Skill conflict and disagreement graph |
| `mejepa_skill_browse` | Browse named skills with support and lifecycle summaries |

#557 adds the usefulness audit view behind this surface: candidate kind, support/fail-pass distribution, lift, genericity, per-cell contribution, source-byte representative sequences, lifecycle history, uncovered code-state keys, unknown ordered constellations, and rejected/demoted patterns. It explicitly keeps stable background scaffolding as context/negative evidence rather than promoting it as a failure skill.

Remaining (open in #417): cross-repo skill transfer and pathway-leaf attachment.

The constraint: skills are deterministic groupings of accepted Level-0 labels. They do NOT introduce ambiguous heads. Skill linkage is a **navigation surface over the predictor's training substrate**, not a new prediction surface.

Runtime resolver (#418, shipped): resolves every live-input-safe membership for each chunk; carries memberships into `PredictionLabelContext`; on refutation writes lifecycle audit (update/split/create/demote/no-change).

---

### 1.5ter Array-of-vectors constellation rule (LOCKED IN)

The panel is an **array of 15 per-embedder vectors**. NOT a 5,120-d fused embedding. The contiguous 5,120 f32 buffer is byte-layout for serialization only.

**Valid operations:**
- Same-embedder dim-wise compare: `cosine(panel_A.E_AST, panel_B.E_AST)` ✓
- Array-vs-array per-slot map (vector of 15 same-embedder cosines)
- Strict-conjunction gate over the map
- Per-pair similarity matrix (diagonal only — off-diagonal undefined)
- Cross-slot scalar pairwise: MI(slot_a, slot_b), correlation(slot_a, slot_b)
- Matrix-style ops respecting slot identity (per-slot weighted aggregation, masked projections)

**Forbidden:**
- `cosine(panel.E_AST, panel.E1)` (different coordinate systems) ✗
- `linear(5120, hidden)` over packed buffer (silent cross-embedder compensation) ✗
- `residual_norm_sq` over packed 5,120 buffer ✗
- Any operation where strong slot compensates for weak slot

**Code-level enforcement (shipped).** `crates/context-graph-mejepa-tct/src/gtau.rs:14` iterates `for embedder in EmbedderId::all()`; `cosine_similarity` at `gtau.rs:90` errors with `MEJEPA_TCT_DIM_MISMATCH` on length mismatch. Slot-preserving late-fusion predictor (#387) replaced the flat-projection predictor.

---

### 1.5quart Stars as Vectors — celestial-cartography model

The teleological-constellation primitive is named after star constellations and the analogy is formal. See `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`.

| Astronomy | ContextGraph |
|---|---|
| One star = vector in ~100-dim observational space (Gaia DR3) | One AST chunk = array of 15 per-embedder vectors |
| Same-spectrum-band cosine valid | Same-embedder cosine valid |
| Cross-band dim-wise compare = nonsense | Cross-embedder dim-wise compare forbidden |
| Constellations perceived from viewing angle | Constellations perceived from `(mutation_category × language × cell)` |
| Big Dipper = 2D projection of 7 high-dim stars | Code-state constellation = signature over panel array |
| Star foundation model predicts missing parameters | ME-JEPA predicts missing reality outcomes |

**Key implication: the constellation isn't in the data, it's in the predictor.** The 15-slot panel for a given chunk exists independently of any constellation labeling. The named failure modes, skills, and code-state constellations are the *predictor's perceived patterns*. Re-train → constellations change shape. The 0.866 vs 0.95 gap is bottlenecked by what the predictor learned to perceive, not by raw embeddings.

Operator-facing tools (planned, extending #417): `mejepa_chunk_as_star`, `mejepa_constellation_membership`.

**Information-physics framing (peer-reviewed):**

1. **Wheeler "it from bit" (1989)** = the binary Q1/Q2/Q5 contract.
2. **Vopson 2nd law of infodynamics (AIP Advances 12, 075310, 2022)** = entropy-decreasing operations (tier compression, slot-preserving fusion). Formalized as trainer loss in #511.
3. **Gates' self-dual ECC in supersymmetry (Doran et al. 2008)** = formal version of anti-compensation. Filed as #512.
4. **FlyWire whole-brain connectome (Nature 634, 2024)** = #388 cross-embedder correlation tensor. #513 was tested and demoted as diagnostic-only after split-stable evidence failed modularity/coverage.
5. **Eon Systems cold-load (March 2026)** = behavior encoded in weights, not transient training state. Filed as #514.

---

### 1.5quint The creativity engine (post-0.95 — autonomous bounded growth)

ME-JEPA is the *engineering* half: verifies binary claims against reality. The *creativity* half — **autonomous embedder synthesis** — proposes new measurement axes. Integration is autonomous because two oracles gate every proposal:

1. **Novelty oracle** — cross-correlation test: is candidate orthogonal to existing N? Reject if max correlation > τ_novelty (~0.7).
2. **Utility oracle** — ablation test: does adding it improve per-cell oracle correlation by ≥ δ_utility (~0.005)?

**The non-obvious property.** The number of orthogonal informative axes ContextGraph admits is **bounded by reality** — the intrinsic dimensionality of the corpus. For code N* ≈ 50-500. The system **discovers its own N*** because the novelty oracle rejects every candidate once the corpus is fully resolved.

**Five issues (all post-0.95):**

| Issue | What it does | Priority |
|---|---|---|
| #517 / TASK-CR-002 | Mistake-driven embedder demand signal | p1 |
| #516 / TASK-CR-001 | Novelty + Utility oracle | p2 (p1 immediately post-0.95) |
| #518 / TASK-CR-003 | Embedder retirement audit + Pareto frontier | p1 |
| #519 / TASK-CR-004 | Domain-routed sparse panel | p1 |
| #520 / TASK-CR-005 | Episodic-to-semantic consolidation | p2 |

Together with v2-stretch (#329, #335, #354-357) these form a **self-resolving observer**: system grows panel until corpus is resolved, prunes dominated embedders, specializes per domain, compresses settled clusters. Bounded by reality, not ambition.

---

### 1.6bis 2026-05-22 hardening pivot — measurement caveat (read before §1.7)

On 2026-05-22 the operator opened **#633 / [P0] Manual full-system FSV and hardening gate** following a 5-Sherlock parallel forensic audit (#629). The audit surfaced ~15 P0 silent-success defects (`07 §2.0`): code returning `success`/`passed=true`/exit-0 without performing the work. Examples: `optimizer_witness_chain_verify` returning success for absent `witness-chain.bin` (#608), `store_memory` returning success after sidecar write failure (#599), the Rust Trainer emitting pseudo-loss without training a predictor (#596), checkpointed-train echoing oracle labels into live predictions (#597), MCP/CLI eval-run persisting fixture EvalReports via deterministic compiler (#594).

**Operational implication.** Per **#655** (retire Rust FSV examples → manual SoT guidance, in-progress p0), auto-FSV `*_fsv.rs` harnesses are being retired. Any ship-gate-relevant evidence must come from manual operator-driven SoT readback per `06_operations_and_fsv.md §1.9`.

**P0 doctrine-violation defects (open, all gate-blocking):**

- **#618** aggregate OOD score uses flat 5,120-byte panel buffer (doctrine §1.5ter violation)
- **#619** `forward_inverse_inner` uses `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter violation)
- **#621** hardcoded `gtau_pass_rate: 1.0` in HoldoutReport published to MCP weekly dashboard
- **#670** invalid forward-cache spaces `e2/e3/e4/e8` (forward-cache embedder collapse uncovered by #666)

**Substrate audit chain (data-substrate completion, 2026-05-22):** #612 → #613 (active source-lineage canonicalized; `0` `/mnt/d` hits) → #614 (forward-cache slot-completeness mask shipped: 378,444 full 12-slot rows + 20,706 partial + 627 zero-slot) → #615 (delete-test-call regeneration plan: 1,131 deterministic candidates, in-progress) → #616 (weak-cell oracle reruns, open) → #631/#632/#641/#666/#667/#668 (closed).

**New geometry-label epic (the #413 successor, in-progress):** #657 TASK-PY-G-136 per-embedder vector-space label discovery bakeoff. Each active embedder (`e1,e2,e3,e4,e6,e7,e8,e9,e10,e12,e13,e14`) gets its own deterministic geometry-label map via Leiden-on-kNN / HDBSCAN / spherical k-means / UMAP-diagnostic bakeoff. Downstream: #661 cross-embedder consensus → #662 gate-safe promotion → #664 router → #665 prediction heads.

---

### 1.7 Synthetic-vector remediation (2026-05-19 / 20) — measurement caveat

Between Phase G ramp and 2026-05-19 22:45 UTC, the **inference path** in `crates/context-graph-mejepa/src/compiler.rs::vectors_for` filled instrument-panel slots with SHA256-seeded synthetic vectors via `deterministic_vector(slot, &digest)` instead of calling the real frozen `E*Instrument` encoders. Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452, all closed 2026-05-19/20) document and fix this contamination.

**What was actually doing.** The pre-fix end-to-end inference pipeline was:

```
patch + context → SHA256(task_id ++ problem_text ++ patch_sha ++ commit_msg ++ time_step)
                → digest[32]
                → deterministic_vector(slot, &digest)   ← sine-wave from hash
                → Panel (looks structurally valid; per-slot is hashed nonsense)
                → MeJepaPredictor::new()                ← untrained random weights
                → "verdict"
                → promote_best wrote MEJEPA_BEST_STUB_V1 text to best.safetensors
                → ship-gate claimed "0.866667 prediction_oracle_correlation"
```

The result was deterministic for the same `(task_id, patch_sha)`. Every ablation moved compute and never moved input signal, so every variant converged to the same 0.8666666746139526 fixed point.

**Code-level fixes (shipped).**

| Commit | Fix |
|---|---|
| `e2c51669` | `vectors_for` replaced with `build_inference_panel_inputs` → real `EAstInstrument` / `ECfgInstrument` / `EDataFlowInstrument` / `ETypeGraphInstrument` / `EDiffInstrument` / `ETestInstrument` / `EProblemInstrument` / `ECommitMsgInstrument` / `EWitnessInstrument` / `EOracleInstrument` / `ETraceInstrument` / `EStaticAnalysisInstrument` / `ERuntimeInstrument` / `EReasoningInstrument` / `ScalarsInstrument` (`crates/context-graph-mejepa/src/compiler.rs:1410-1418`) |
| `5330629e` | Project cache fails closed on fake embeddings; no more `deterministic_vector_blob` |
| `a0849ac3` | Project ingest wired to active embedder forward providers |
| `398e5216` | Multi-array provider honors active model registry |
| `db83923a` | `MEJEPA_BEST_STUB_V1` text-stub checkpoint rejected at load (`crates/context-graph-mejepa/src/predictor_checkpoint.rs:147-158, 239-250`); manifest-verified trained checkpoint pipeline added |
| `d068d2ed` | Trainer consumes native active-embedder ragged constellations (#452) |
| `f982f29d` | Runtime fail-closed if it cannot load a verified trained checkpoint |

**What survived (real, no rebaseline needed).**

| Artifact | Why real |
|---|---|
| 399,777 AST chunks (`python_ast_chunks.jsonl`) | Deterministic AST chunker over real SWE-bench Lite Python source |
| 391-shard packed forward cache | Generated by `crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs` — real Candle CUDA forward, `precision_class: "candle_real_forward"`, `~24h` GPU run, SHA-pinned safetensors via `models_config.toml`, fail-closed guards at `crates/context-graph-mejepa-embedders/src/forward.rs:122,207,349,422` saying `"no deterministic vector fallback is permitted"`. Wall-clock + VRAM signature alone prove real model inference. |
| #388 cross-embedder correlation tensors | Deterministic derivation from real forward cache |
| #389 per-slot calibration + OOD policy | Built from #388 + real training rows |
| #413 Level-0 label compiler (491 accepted, 399,777 chunk labels) | Deterministic compiler over real chunks + real cache |
| #414 skill substrate (128 / 36,773 / 10,579) | Derived from #413 |
| #404 hierarchy (8 / 157 / 379 / 536 named predictions) | Derived from #413 + #414 |
| Anti-compensation FSV (#394) | Invariant; doesn't depend on input quality |
| Runtime ability resolver (#418) | Pure resolver logic |
| Mistake-loop machinery (#406 / #523) | Storage + MCP wrappers; needs real predictor mistakes to validate convergence |

**What is invalidated (must rebaseline).**

| Artifact | Why invalid |
|---|---|
| Ship-gate `prediction_oracle_correlation = 0.866667` | Measured against synthetic-input inference pipeline |
| `λ_inv ∈ {0, 0.25, 0.5, 1.0}` bidirectional ablation (all 0.8666666746139526) | Verbatim identical because input was deterministic-from-hash |
| #393 capability-discovery ablation suite | Original measurement invalid; real-input rerun now exists and remains `no_promotion` |
| #398 calibrated constellation ablation rerun (Brier 0.7539) | Original measurement invalid; real-input rerun now exists and remains `no_promotion` |
| "Slot-preserving architecture doesn't close the 0.083 gap" claim | Original claim invalid; real-input #393/#398/#546 reruns are negative evidence but do not validate the pre-fix 0.083 framing |
| "Gap is a function of labels, not embedder count" claim | Invalid as a broad causal claim; real-input constellation/`λ_inv` ablations did not identify those surfaces as gate movers |
| #129 OOD serving gate AUC measurements | Revalidated post-fix in `/zfs/archive/contextgraph/fsv/task-py-g-014-ood-gate-fsv/run-20260521T145048Z/` |
| #130 Gτ guard audit pass/fail counts | Revalidated post-fix in `/zfs/archive/contextgraph/fsv/task-py-g-015-gtau-guard-audit-fsv/run-20260521T142921Z/`; remains fail-closed/non-countable |

**Rebaseline action plan (current P0 gate work).**

1. ✅ Run `swebench_300x8_ship_gate_fsv` end-to-end on aiwonder with real-instruments inference path + real trained checkpoint (from #441's now-shipped trainer pipeline + #127 / TASK-PY-G-127 trained-checkpoint export FSV). Current measurement: `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`.
2. ✅ Re-run `λ_inv` bidirectional ablation with real inputs. Current measurement: `/zfs/archive/contextgraph/fsv/task-ek-012-bidirectional-ablation-realinput-fsv/run-20260520T172657Z/`.
3. ✅ Re-run #393 / #398 ablation suites against real-input inference path. Current measurements: `/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-realinput-fsv/run-20260520T181900Z/` and `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-realinput-fsv/run-20260520T170156Z/`.
4. ✅ Verify trained-weight manifest: `trained_weight_sha256 != initial_weight_sha256` (proves real training happened, not stub copy).
5. ✅ Repair first weak-cell slice from #535/#536 (`delete_test_call`, `known_good`, `over_engineer`, `swap_variable` Brier/correlation; `subtle_flip` Brier; `off_by_one` fail recall). #548/#549 proved the remaining static mixed-cell signal is calibration-underpowered; #392/#558/#560 proved file/skill trajectories are a stronger but still ECE-blocked signal. Next gate work is per-cell/per-constellation ECE repair, `swap_variable` skill specialization, consequence-to-cause traces, and Panel B validation before any 0.95 claim.
6. ✅ Revalidate post-#536 calibration disjointness (#547): selected `tau=20.0, fail_boost=1.5` was calibration-selected, row/feature-member overlaps are zero, #413 label-lineage hashes match, and target-side labels are excluded from live rank-lift features.

Tracked under #529; gate-active critical path in `08_roadmap_and_open_work.md §3.0` now puts #392 trajectory calibration repair before #511 / #405.

### 1.5sext Neuroscience-inspired reserve (filed 2026-05-20, DO NOT START until current queue concludes)

Eight contingency issues (#538-#545) document neuroscience-inspired architectural enhancements that would activate if the current Phase G queue + creativity engine fail to close the remaining ship-gate gap. All filed at `priority:p3` with explicit `⚠ DO NOT START` markers. Reassess only after post-queue gap analysis.

**Tier 1** (high-impact, doctrine-compatible, addresses gap directly): predictive-coding head with top-down per-slot prediction + bottom-up residual (#538, Rao & Ballard 1999 / Friston free-energy), hippocampal schema-prioritized replay with reverse replay for pathway credit assignment (#539, Mattar & Daw 2018 / Liu et al. 2019), hierarchical predictive coding tied to the #404 label hierarchy (#540, Felleman & Van Essen 1991 / Bastos et al. 2012), mixed-selectivity cross-slot binding layer (#541, Rigotti et al. 2013 / Fusi et al. 2016).

**Tier 2** (more speculative; design-influence on existing epics): population-dynamics low-dim trajectories for #416 pathway surface (#542, Shenoy/Churchland/Cunningham), sparse distributed representations per slot (#543, Hawkins/Numenta), synchrony-based temporal binding for #392 (#544, Singer & Engel / Fries 2005), idle-time generative rollouts (#545, default-mode network analog).

All eight verified doctrine-compatible before filing: slot identity preserved, anti-compensation invariant preserved, binary contract preserved, ambiguity boundary preserved, no inner LLM, no synthetic-input shortcuts. See `08_roadmap_and_open_work.md §6.9` for full index + activation rule + risk note.

### 1.6 The ambiguity boundary (locked-in rule)

ME-JEPA only predicts questions with a **free binary form against reality**. Free = verdict comes from observing reality directly, not from imposing arbitrary thresholds on continuous heuristics.

**Excluded from prediction contract:** perf regressions, maintainability, idiomaticity, latent bugs, code redundancy, edge-case coverage, cost, reasoning quality.

**Kept in prediction contract:**

| Surface | Reality |
|---|---|
| Q1 claim-exists | Bytes / AST on disk |
| Q2 oracle pass | Docker oracle `report.json` |
| Q5 shift events | Durable shift-log JSONL |

Q4 heads remain in schema and CFs (`CF_MEJEPA_Q4_*`) as **frozen historical display data**, readable but not trained, calibrated, or consumed.

---

## 2. Process / Port / Tech Architecture

| Process | Port / IPC | Technology | Purpose |
|---|---|---|---|
| `context-graph-mcp` server | stdio JSON-RPC (default) or HTTP transport | Rust, tokio, async-trait | 204-tool MCP surface; daemon mode |
| `context-graph-cli` | CLI invocation | Rust | `mejepa observe`, `mejepa project-ingest`, `mejepa compression-progress`, `mejepa active-learning`, etc. |
| ME-JEPA training daemon | embedded in CLI/binary | candle (CUDA 13.2), RocksDB | UTML training loop; reads `CF_MEJEPA_REPLAY_BUFFER`; writes `CF_MEJEPA_TRAIN_CERTS` |
| Shift-log subscriber | tokio task | tokio | Tails JSONL shift log → materializes panels |
| Heal scheduler | embedded thread | tokio | Drift detection, Fisher/EWC, distill, promotion |
| RocksDB (per runtime root) | local disk | RocksDB ≥ 8.x | 200+ column families |
| GPU runtime | CUDA 13.2 on RTX 5090 Blackwell | cudarc 0.19.x via candle 0.10.2 | Embedder forward passes, predictor training |

**Constraints:**
- **No HTTP from action paths.** MCP server is the only network surface.
- **No inner LLM.** Local generative inference (Qwen / Hermes / Ollama / llama.cpp) is compile-locked off — `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`.
- **No intent synthesis.** Observable goals and agent claims are evidence; hidden human intent is not a prediction target.

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
| `context-graph-mcp` | MCP JSON-RPC server + 204 tools (54 `mejepa_*`) |
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
| `context-graph-test-utils` | Deterministic-by-shape, pseudo-random-by-content helpers for storage/serialization/shape tests only; not ME-JEPA gate evidence |

Full per-crate detail in `02_source_map.md`.

---

## 5. Entry Points

| Binary | Purpose |
|---|---|
| `context-graph-mcp` | MCP server. `--mode stdio` (default) or `--mode reality-loop` (lightweight subset). Multi-agent observability via `mejepa_daemon_status`. |
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
| **Durable production** | RocksDB CFs (~200); FSV evidence; project-ingest manifests; weekly exports | `/zfs/archive/contextgraph/` on aiwonder | Source-recoverable (corpus) or replay-from-shift-log (predictions). Many FSV artifacts are irreplaceable single events. |
| **Hot** | Active checkpoints, model weights, hot run state | `/zfs/hot/contextgraph/` on aiwonder | Yes (re-download / rebuild) |
| **Ephemeral** | `/tmp/contextgraph-*` scratch | `/tmp/` | Yes |

Legacy mounted-drive roots such as `/mnt/d/contextgraph/` are **provenance only** — not active data roots.
---

## 8. Exception Hierarchy (fail-closed structured errors)

Every crate defines its own `Error` enum with `SCREAMING_SNAKE_CASE` code via `.code()`. Sample:

| Crate | Sample error codes |
|---|---|
| `context-graph-mejepa-instruments` | `MEJEPA_INSTRUMENTS_INVALID_INPUT`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`, `MEJEPA_INSTR_FROZEN_VIOLATION` |
| `context-graph-mejepa-corpus` | `MEJEPA_CORPUS_INVALID_INPUT`, `MEJEPA_CORPUS_NO_MUTATION_SITE`, `MEJEPA_CORPUS_LEAKAGE_DETECTED`, `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT` |
| `context-graph-mejepa` | `MEJEPA_INSTRUMENT_GRADIENT_LEAK`, `OOD_CALIBRATOR_MISSING`, `OOD_SCORE_ABOVE_THRESHOLD`, `MULTI_HEAD_CONTRADICTION`, `CONTRADICTION_THRESHOLD_MISSING`, `CROSS_PANEL_GOODHART_DETECTED`, `PATHWAY_AMBIGUOUS_LEAF_REJECTED` |
| `context-graph-mejepa-tct` | `MEJEPA_TCT_DIM_MISMATCH` |
| `context-graph-mcp` | `CCREALITY_ENGINE_RETIRED` (retired tool callers fail closed) |

**Fail-closed contract.** Missing CFs, missing files, missing models, missing env vars, malformed inputs MUST return a structured error with `code()`. No silent defaults. (Doctrine § 1 / FSV-PROTOCOL § 3.)

---

## 9. Major Subsystems (one paragraph each)

**MCP server (`context-graph-mcp`).** JSON-RPC 2.0 stdio (default) or HTTP transport. 204 tools (54 `mejepa_*` product surface, plus learning, UTL, constellation, entity, embedder, graph, sequence, file watcher, training, provenance, daemon, etc.). Retired `reality_*` tools fail closed with `CCREALITY_ENGINE_RETIRED`. Server enforces multi-agent isolation via path-policy + edit-lock.

**Predictor (`context-graph-mejepa`).** CUDA-only production predictor. 73 modules covering claim graph (Q1), patch-similarity HNSW exemplars (Q3), Q5 shift-log replay impact, multi-head VICReg + Huber loss, conformal calibration, OOD detection, project-ingest reality compiler, synthetic-stress evaluator, adversarial corpus, dynamic embedder registration, foundationality (PageRank), operator override. `compile()` and `compile_with_fingerprint_observation()` are the canonical Q1-Q5 entry points.

**Frozen instruments (`context-graph-mejepa-instruments`).** A `Panel` is an array of 15 per-embedder vectors: `E_AST (384)`, `E_CFG (256)`, `E_DataFlow (256)`, `E_TypeGraph (256)`, `E_Test (384)`, `E_Trace (512)`, `E_Diff (256)`, `E_Witness (256)`, `E_Oracle (128)`, `E_Problem (1024)`, `E_CommitMsg (384)`, `E_StaticAnalysis (256)`, `E_Runtime (256)`, `E_Reasoning (384)`, `Scalars (128)`. The 5,120 f32 contiguous buffer is byte layout only. Cross-embedder dim-wise compare forbidden. Instruments carry zero trainable params (`ΔΞ.target_collapse = 0`).

**Embedders (`context-graph-mejepa-embedders`).** 21 slots. 12 active content embedders (8 pretrained + 4 algorithmic). Frozen-by-design: E1, E6, E7, E8, E10, E12, E13, E14. Dynamic-registration with VRAM budget + falsification + freeze + adversarial-injection mitigation. SHA-256 manifest pins for every pretrained weight. CUDA-required.

**Corpus generator (`context-graph-mejepa-corpus`).** 8 mutation categories × 11 languages = 88 generic operators (Python first-class via parser-backed operators; other 10 deferred). Categories: `KnownGood`/`SubtleFlip`/`OffByOne`/`SwapVariable`/`DeleteTestCall`/`WrongFile`/`OverEngineer`/`CompileError`. SWE-bench Lite bridge wraps `python -m swebench.harness.run_evaluation` per-instance. Split modes: `InstanceAtomic` (default, 240/30/30) and `RepoAtomic`.

**Trainer (`context-graph-mejepa-train`).** UTML v1.0 implementation. `compute_l_step` evaluates `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. EWC++ Fisher, continual backprop (Dohare 2024), prioritized replay buffer over `CF_MEJEPA_REPLAY_BUFFER`. **Mistake loop consumer (#406, shipped):** consumes `CF_MEJEPA_MISTAKE_LOG`, applies exact-panel online-head updates with measured unrelated-control isolation, writes byte-readable `mistake_repeat_rate` metric. Skill-sequence discovery (#414, shipped) + failure-mode hierarchy (#404, shipped) feed the labeling layer.

**TCT (`context-graph-mejepa-tct`).** Teleological constellations + Gτ guard. Cell key = `(mutation_category, language, additional_dims)`. Per-cell calibration, freshness audit, refresh scheduler. Gτ rejects predictions with insufficient cell support.

**Hygiene (`context-graph-mejepa-hygiene`).** Tier transitions (hot → warm → frozen), quota enforcement, GC, witness-chain Merkle compression. MCP surfaces: `mejepa_quota_status`, `mejepa_gc_run`, `mejepa_witness_compress`.

**Shift subscriber.** Tokio task tailing `/zfs/archive/contextgraph/runtime/shift-log.jsonl`. Decodes `ShiftEntry` → `shift_to_inference()` → materializes `Panel` → writes `CF_MEJEPA_PANELS` + `CF_MEJEPA_SHIFT_WATERMARK`. Watermark is the durability gate.

**Storage (`context-graph-storage`).** RocksDB wrapper. ~200 CFs (51 base + 120 ME-JEPA + 19 teleological + 13 quantized + 5 code + 2 causal). HNSW configs per embedder; inverted index for sparse embedders.

**Witness (`context-graph-witness`).** Tamper-evident chain primitives. ed25519 segment signing, Merkle aggregation, Stage 0 + Stage F + Merkle certificates with full metadata binding.

---

## 10. What This System Is NOT

- Not a generative inference engine — no inner LLM. The product is the verifier, not a code-writer.
- Not a benchmark harness — pre-pivot benchmark crate retired.
- Not a hosted service — runs locally on operator hardware (aiwonder for production).
- Not a Python codebase — Python only shows up as the corpus subject (SWE-bench Lite) and in tool wrappers.
- Not cross-language at ship-gate time — current gate is Python-only (300×8 Python corpus).
- Not HTTP-callable from action paths.

---

## Cross-references

- Per-crate source map + dependency graph → `02_source_map.md`
- Every RocksDB column family with key/value format → `03_storage_schema.md`
- Every MCP tool with parameters and returns → `04_mcp_tools.md`
- Embedder slots / instruments / heads / TCT / corpus / heal / mistake loop / training → `05_predictor_pipeline.md`
- FSV protocol, runbook, DR, observability, test suite → `06_operations_and_fsv.md`
- FSV evidence history, ship-gate progress, ablations → `07_results_and_benchmarks.md`
- Current open work, critical path, post-ship plans → `08_roadmap_and_open_work.md`
- Doctrine (universal protocol) → `docs2/AICodingAgentSuperPrompt.md`
- Project addendum (invariants, deny-list) → `CLAUDE.md`
- Per-turn agent contract → `AGENTS.md`
