# 01 — System Overview

**Last refreshed:** 2026-05-19. **Status:** Phase G in flight (ship gate `prediction_oracle_correlation ≥ 0.95 × 4 rolling windows` still open; latest holdout 0.866667). Phase A–F + H code/FSV-shipped. 2026-05-19 architectural verdict: the slot-preserving constellation refactor (#385/#386-#398) is correct but does not close the 0.083 gap by itself; #383 (evidence-weighted ordering) now redirects next bandwidth toward Q4/Q5 label producers + OOD calibration before further predictor changes.

Source files covered: `Cargo.toml`, `CLAUDE.md`, `AGENTS.md`, `crates/context-graph-mcp/src/lib.rs`, `crates/context-graph-mejepa/src/lib.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`, `crates/context-graph-mejepa-instruments/src/lib.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs`, `crates/context-graph-storage/src/lib.rs`, `crates/context-graph-mcp/src/tools/definitions/mod.rs`.

---

## 1. What This System Is

**ME-JEPA-Code** is a **binary reality predictor**. AI coding agents (Claude Code / Codex / Cursor) call it via a local MCP JSON-RPC server. For any candidate code change it returns:

```
verdict ∈ { Pass, Fail }    // "Will this change exist in working reality? Yes or No."
+ named_failure_mode         // (when Fail) the labeled cluster from the failure-mode hierarchy (§1.2)
+ closest_exemplars          // top-K historical patches from the 300×8 corpus that look like this one
+ violating_slots            // per-slot OOD reasons + cross-embedder contradiction evidence (#389/#390)
+ mistake_id (when wrong)    // ground-truth observation lands in CF_MEJEPA_MISTAKE_LOG and feeds online learning (§1.3)
```

There is one prediction per candidate. There is no best-of-K. There is no Abstain at the product surface. The system must be right the first time; when it is wrong, the mistake is captured and the predictor adapts.

The five Q-questions are no longer parallel predictions. They split into **binary predictions** (Q1, Q2, Q5) and **explanation-only surfaces** (Q3) — and **Q4 is out of the prediction contract entirely**.

| Q | Status under the strict binary contract |
|---|---|
| **Q1** | **Binary prediction.** Hard input feature + downstream check: did the AI's claim physically land in the bytes/AST? Reality is the bytes on disk + AST; the check is byte-equality + AST diff. Free binary form. |
| **Q2** | **The binary predictor itself.** Output is `Verdict::Pass\|Fail`. Reality is the Docker oracle (`swebench.harness.run_evaluation` per-instance). Free binary form. |
| **Q3** | **Explanation surface only.** Named failure mode + closest exemplar attached to a `Fail` verdict. Does NOT make its own prediction; it labels the binary verdict that Q2 already made, using the §1.2 hierarchy + case-based exemplar retrieval. |
| **Q4** | **OUT OF SCOPE.** Removed from the prediction contract 2026-05-19. Q4 questions ("is this performant?", "is this maintainable?", "is this idiomatic?", "are there latent bugs?") do not have a free binary form against reality — they require arbitrary thresholds, depend on workload/hardware/baseline assumptions, or ask about ambiguous taste judgments. ME-JEPA does not predict ambiguous things. Existing Q4 producers (perf/accuracy/cost/reasoning/non-trivial-on-Pass) are **frozen as historical display data**; not trained against, not ship-gate-relevant, not consumed by the predictor. The heads remain in the schema for backward read-only compatibility and will be marked `deprecated_ambiguous` in the next schema version. |
| **Q5** | **Binary prediction.** Reality is the durable shift log (`/zfs/archive/contextgraph/runtime/shift-log.jsonl`). For each predicted side effect: did the corresponding shift-log event occur? Yes/no per leaf event. Falsifications enter the online-learning mistake loop (`reality_impact.rs`, `CF_MEJEPA_MISTAKE_LOG`). |

**Why Q4 is dropped, not thresholded.** A continuous quantity (perf wall-clock, mutation survival rate, complexity score) only becomes binary via an arbitrary threshold ("perf regressed by ≥10%"). The threshold is not reality — it is a heuristic. ME-JEPA predicts reality, not heuristics. Heuristic-against-reality predictions can be neither right nor wrong in a sense that drives the mistake loop; there is no ground truth that says "10% is the correct threshold." Without ground truth there is no learning signal. Better to predict zero ambiguous things than to predict noise.

This is a meaningful tightening from earlier roadmap framings. Previous Q4 producer work (#266 / #267 / #268 / #269 / #270 / #399 / #400 / #401, plus #402) remains shipped FSV evidence but is reclassified as **completed-and-archived**; new Phase G bandwidth does NOT spend on Q4 expansion. See `08_roadmap_and_open_work.md §3.1` for the closure of the Q4 producer track and the redirection of bandwidth.

**Ship-gate predicate:** `prediction_oracle_correlation ≥ 0.95` stable across `(mutation_category × language)` on four consecutive rolling windows of the 300×8 SWE-bench Lite Python corpus, **measured on Panel A *and* re-confirmed on the non-overlapping Panel B** (§1.4 cross-panel triangulation; new requirement 2026-05-19).

**Product boundary.** LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, and docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`.

**Consequence-prediction contract (#397).** In this project, "predicting reality" means predicting the consequences of code being live: whether the change exists, whether it works, why it would fail, what secondary risks it creates, and how it changes durable repository/test/runtime/dependency reality over time. ME-JEPA presents those predicted consequences, confidence, and evidence to the human operator or the operator's AI agent so they can decide whether to proceed, repair, or choose a different route. The model observes explicit task text, agent claims, edits, AST deltas, frozen instruments, oracle outcomes, and verification feedback; it must not infer or classify hidden human intent (#358).

**Live skill/ability surface (#417).** When ME-JEPA compiles a live project, the path is: project files -> Python AST chunks -> slot-preserving TCT constellations -> deterministic live-observable labels -> many-to-many chunk-skill memberships -> `PredictionLabelContext.active_skill_ids` -> consequence prediction. A chunk can participate in multiple Level-1 groups, Level-2 skills, and Level-3/higher abilities; a skill can span many chunks/files/ordered steps. Those memberships are prediction evidence, not human-intent inference. If reality refutes the prediction, the mistake loop must update the involved skill/ability model: update an underweighted skill, split a mixed skill, create a new candidate skill, demote an unstable skill, or record no-change with evidence.

### 1.1 Binary-reality contract (2026-05-19 reframe)

The five questions (Q1–Q5) are **explanation surfaces** for one **binary prediction**:

```
verdict ∈ { Pass, Fail }   ⇔   "Does this candidate code change exist in working reality?"
```

Pass means the AI's claimed change is materially correct against bytes/AST/test/runtime reality. Fail means it is not. There is no soft-confidence ranking, no `Abstain`, no best-of-K hedging at the product surface. The system must be **correct on the first try**; if it is wrong, the mistake becomes a labeled training row and the predictor adapts so the same mistake is not made twice. This is the property that lets the ship gate be a single number (`prediction_oracle_correlation ≥ 0.95`) rather than a calibrated-coverage statistic.

The Q-surfaces split into **binary predictions** (Q1/Q2/Q5), an **explanation surface** (Q3), and **out-of-scope** (Q4):

| Surface | Binary against reality? | Role |
|---|---|---|
| Q1 (`claim_graph`) | Yes — bytes/AST against disk | Binary prediction + input feature to Q2 |
| Q2 (`oracle head` + `Verdict::Pass\|Fail`) | Yes — Docker oracle | **The product-facing binary prediction** |
| Q3 (`predicted_failure_modes` + `closest_exemplars`) | No — labels not predictions | Named explanation attached to Fail; powered by failure-mode hierarchy (§1.2) + case-based exemplars (§1.3) |
| Q4 (always-on heads) | **No — ambiguous by nature** | **Out of scope.** Existing heads frozen as display-only historical data. Not trained. Not ship-gate-relevant. See §1.2 below for the ambiguity argument. |
| Q5 (`reality_impact`) | Yes — shift-log against shift-log | Binary prediction + mistake-detection surface; falsifications enter the online-learning mistake loop |

The legacy `Verdict::{OutOfDistribution, Abstain, GuardRejected}` states do not disappear from the code, but they are **internal states**, not product outputs. An internal `Abstain` or `OOD` is a signal that the predictor lacks sufficient evidence to be correct on the first try — the system MUST treat that as a present-tense failure to be eliminated, not as an acceptable hedge. The path out of an internal Abstain is online learning, not "try K candidates and pick the best."

### 1.5 The butterfly-effect pathway surface (binary-leaf-only consequence exploration)

The binary-prediction contract (§1.1) is the per-leaf primitive. What the system *delivers to the operator* on top of that primitive is a small set of **most-probable futures** — concrete branching pathways that a candidate code change opens up in durable reality, each pathway being a conjunction of binary leaves. The operator picks which future they want; the system shows them what code-shape lands them there.

**Why this is what the system is actually doing.** Each prediction surface is a binary leaf against ground truth (Q1: did the claim land in bytes? Q2: does the Docker oracle pass? Q5: did this specific predicted shift-event occur?). These leaves compose temporally and causally: Q1's outcome conditions Q2's prior; Q2's outcome conditions Q5's prior over downstream shift events; and Q5 events condition further Q5 events as the patch lives in the codebase over subsequent sessions. The full *tree* of possible reality trajectories from a single candidate patch is exponentially large, but **most branches collapse to negligible probability**. The most-probable branches form a small handful of pathways the operator can usefully reason about.

This is the established L3 capability in the JEPA world-model rubric (arXiv 2511.16333): L1 temporal prediction → L2 action-conditioned prediction → **L3 counterfactual rollouts for decision support** → L4 planning/control. ME-JEPA's Q1/Q2/Q5 stack already does L1 + L2; the pathway surface is the L3 instance.

**Strict binary-leaf rule (the ambiguity boundary applies here too).** Every node in the pathway tree must be a binary leaf against ground truth — Q1/Q2/Q5 only. No ambiguous heads (Q4 perf / accuracy / cost / latent-bug / tech-debt / idiomaticity) enter the tree as nodes. The reason is the same reason Q4 is out of the prediction contract: an ambiguous leaf cannot be falsified by reality, so a pathway through an ambiguous leaf cannot be probability-weighted in a way that the mistake loop can clean up. Whether the pathway surface ever expands to ambiguous leaves is an open question; current scope is binary only.

**Construction (planned, #416 / TASK-PY-G-119 — Pathway exploration surface).**

1. **Seed:** the candidate patch (post-Q1 binary verdict on claim-exists).
2. **Tree expansion:** beam search over downstream binary leaves. Each leaf is one of:
   - `Q2.oracle_pass(patch) → {Yes, No}` — immediate test outcome
   - `Q5.shift_event(event_id) → {Yes, No}` — predicted downstream consequence (predicted file-byte change, predicted test re-run outcome, predicted downstream API call shift, etc.)
   Each node's children are conditioned on the path so far. Each edge carries the predictor's binary probability + the calibration confidence.
3. **Pruning:** beam width is bounded; low-probability subtrees (`P(path) < ε` for ε ≈ 0.01) are pruned. Inspired by Diffusion Scenario Tree (arXiv 2509.14832) — `top-K` retention by cumulative path probability, branch probabilities normalized to sum to 1 at each level. Inspired by SWE-Search / CodePilot / Narrative Studio MCTS patterns — UCB1 for selection, rollout depth bounded, backprop-of-value up the path.
4. **Top-K surfacing:** return the 3-5 most-probable terminal-state pathways. Each pathway carries:
   - The full chain of binary leaves with their per-leaf probabilities
   - The joint pathway probability (product of conditional leaf probabilities, calibrated by the conformal interval)
   - The closest historical exemplar from the 300×8 corpus that traced a similar pathway (powered by the 491-row accepted label registry from #413)
   - The named failure mode for any `Fail` leaf in the pathway (powered by #404 once Level-1/2/3 hierarchical naming lands on top of #413's registry)
   - The active skill / higher-ability IDs and source chunk memberships that explain each leaf when available (powered by #414/#417)
5. **Operator decision:** the operator reads the 3-5 pathways, picks the one they want reality to take. The system then shows them what code-shape *gets them to that pathway* — by reverse-engineering the patch-shape constraints that satisfy the chosen pathway's binary leaves (the §10.2(IX) / TASK-EK-013 / #335 `mejepa_reverse_prediction` tool, when it ships).

**What this is, in standard language.** The pathway surface is the **Pearl Rung-3 counterfactual rollout** instantiated on binary leaves over the durable reality substrate. It is the *simulation-in-the-loop* pattern from arXiv 2603.11677 ("Control without foresight is like driving at night with no headlights"). It is structurally equivalent to **quantitative patch impact analysis** (arXiv 2605.13885) plus **sensitivity analysis for chaotic systems** (the SciML shadowing-methods treatment of the butterfly effect) — measure of how a small action cascades through a complex system, with the cascade decomposed into a small set of named most-probable trajectories rather than a single point forecast.

**Why this matters operationally.** Without the pathway surface, the operator sees one binary verdict per patch (Pass / Fail) and decides yes-or-no on a single candidate. With the pathway surface, the operator sees the *shape of the futures that candidate opens* — what propagates cleanly, what breaks downstream, what triggers re-runs, what compounds over sessions. This converts the system from a verifier of one candidate into **decision-support for reality-shaping**: the operator can choose which of the 3-5 most-probable futures they want, then ask the agent to write the code-shape that lands them there.

**What this is NOT.** The pathway surface is not generative — it does not synthesize new pathways from imagination. Every leaf in every pathway is a binary prediction made by the predictor, against the same ground-truth substrate (bytes / Docker / shift log) that ground-truths the per-leaf training. Every pathway is byte-replayable: every leaf's predicted outcome eventually meets reality, and the mistake loop (§1.3 / #406) updates the predictor on disagreements. The pathway tree is a *projection* of what the predictor already knows about binary leaves, not a free-form imagination tool.

**Ambiguous-pathway question is open and explicitly deferred.** Whether the pathway surface can ever expand to ambiguous leaves (e.g. "this pathway probably degrades performance by ≥10%") is unanswered. The current ambiguity-boundary rule says no: ambiguous leaves don't have free binary form against reality, so a pathway through one cannot be probability-weighted by the mistake loop. Whether some richer formulation can include them is future work; for now the pathway surface is binary-leaf-only. Tracked under #416 / TASK-PY-G-119 with explicit `ambiguous_leaves_in_pathway == 0` FSV assertion.

### 1.5bis The bidirectional chunk↔skill linkage (#417 / TASK-PY-G-120 — data shipped, capability surface planned)

The substrate chain is **AST chunks → constellations → accepted labels → skills**, and the linkage at every step is bidirectional. The data was shipped end-to-end on 2026-05-19 across #413 (Level-0 labels) and #414 (Level-2 skills + memberships). What's pending is the operator-facing **capability surface** that exposes the linkage. The capability surface is filed as #417 / TASK-PY-G-120.

**Shipped substrate (do not redo):**

| Layer | Artifact | Rows |
|---|---|---|
| AST chunks | `python_ast_chunks.jsonl` | 399,777 |
| Level-0 labels per chunk | `chunk_constellation_labels.jsonl` (#413) | 399,777 |
| Accepted Level-0 label registry | `accepted_label_registry.jsonl` (#413) | 491 |
| Level-2 named skills | `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` (#414) | 128 |
| Chunk → skill memberships | `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP` (#414) | 36,773 |
| Skill → chunk reverse index | `CF_MEJEPA_SKILL_REVERSE_INDEX` (#414) | 10,579 |
| Skill lifecycle audit | `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT` (#414) | 128 |

**The insight.** Every higher-level skill is a composition of accepted Level-0 labels which are deterministic functions of specific AST chunks' multi-embedder constellations. Therefore every named skill traces back to the exact source bytes that compose it, and every source byte can be queried for which named skills it participates in. The data exists; the MCP exposure does not.

**Twelve capabilities this unlocks (per #417):**

1. **Skill → code (forward).** `mejepa_skill_to_code(skill_id)` returns the literal source bytes of the top-K member chunks with file/line/commit provenance. "Show me the code that implements skill X."
2. **Code → skill (reverse).** `mejepa_code_to_skill(chunk_id)` returns all skills this chunk participates in.
3. **Skill-conditioned code search.** Boolean set-query: "find code that exhibits skills A AND B but NOT C." Pure intersection/union/exclusion over the existing reverse index.
4. **Refactoring blast-radius via skill graph.** Modify chunk C → walk memberships → walk reverse index → transitive co-member chunks. GitNexus / code-graph-analysis pattern instantiated on skill graph instead of call graph. Composes with TASK-EK-016 chunk-foundationality for risk weighting.
5. **Cross-repository skill transfer.** When ingesting a new project, run the #413 deterministic compiler over its AST chunks; chunks that match existing accepted Level-0 signatures automatically participate in existing skills. The gskill / SkillX automatic-skill-onboarding pattern, but data-driven over deterministic signatures.
6. **Pathway-leaf-to-code linkage.** #416 pathway Fail leaves attach the specific historical chunks that exemplify the mode. "This pathway breaks because the candidate's pattern matches these chunks (byte-level provenance) that have historically failed."
7. **Skill-graph causal reasoning.** Skill S1's chunks call into skill S2's chunks → S1 depends on S2. The chunk call graph projected onto the skill graph gives a causal layer above the call graph.
8. **Operator-facing skill ontology browser.** MCP surface over the 128 named skills + 36,773 memberships + 128 lifecycle audit rows, with source-byte rendering.
9. **Skill provenance for compliance.** Every accepted skill traces to specific AST chunks with file/line/commit provenance. "This code embodies these N capabilities" is a byte-falsifiable claim.
10. **Active-learning skill gaps.** Chunks with 0 memberships are candidates for new skill discovery; coverage map drives the active-learning queue.
11. **Skill-conditioned generation context.** When the operator picks a pathway (#416) and asks the agent to write the code that lands it (#335 `mejepa_reverse_prediction`), retrieve top-K historical chunks of the relevant skills as in-context examples. Case-based reasoning, not free-form synthesis.
12. **Cross-skill conflict graph.** Skills that never co-fire across the shipped memberships are auto-derived as mutually exclusive. "These two capabilities cannot coexist in the same code state."

**The constraint that stays.** Skills are deterministic groupings of accepted Level-0 labels. They do NOT introduce ambiguous heads. Any consumer of skill linkage that feeds back into a prediction surface (e.g. #416 pathway leaves) MUST still use Q1/Q2/Q5 binary leaves only at the prediction boundary. The skill linkage is a **navigation surface over the predictor's training substrate**, not a new prediction surface.

### 1.5ter Array-of-vectors constellation rule (LOCKED IN, 2026-05-19)

The panel is an array of 15 per-embedder vectors. This is the structural rule the predictor, the Gτ guard, the OOD scorer, the calibrator, every downstream head, every FSV harness, and every doc MUST honour. The slot-preserving refactor (#385 / #386 / #387 / #388 / #389 / #390 / #394) shipped this rule into the code; archive-grade framings that called the panel a "5,120-d joint embedding" or "fused panel" are doctrinally retired.

**What an entry in the panel is.** Each `InstrumentSlot` carries one vector produced by one specific frozen embedder/instrument: `E_AST` produces a tree-sitter AST vector, `E_TypeGraph` produces a type-graph attention vector, `E1`-class slots produce e5-large-v2 semantic vectors, `E12` produces ColBERT late-interaction tokens, `E_Trace` produces an execution-trace transformer vector, `E_Oracle` produces a binary-outcome one-hot, etc. Each vector space is **distinct and unrelated to the others**. Dimension `k` of `E_AST` has no relationship to dimension `k` of `E1` — they are different coordinate systems.

**What you CAN do (valid operations):**

| Operation | Example | Valid because |
|---|---|---|
| Same-embedder dim-wise compare | `cosine(panel_A.E_AST, panel_B.E_AST)` | Both vectors are in the same E_AST space |
| Same-embedder centroid + threshold | `cos(panel.E_AST, centroid.E_AST) >= τ_E_AST` (the Gτ guard) | Centroid is built from same-embedder samples |
| Same-embedder vector arithmetic | `panel_A.E_AST - panel_B.E_AST` (residual within the same space) | Same coordinate system |
| Array-vs-array per-slot map | 15-element vector of `[cosine(panel_A.slot_i, panel_B.slot_i) for i in slots]` | Each comparison is same-embedder; the resulting vector lives in slot-index space, NOT in a fused embedding space |
| Strict-conjunction gate over the map | `forall slot: cos(A.slot, B.slot) >= τ_slot` | Per-slot threshold per embedder; same-embedder cosines combined by logical AND, not by averaging or projection |
| Per-pair similarity matrix between two arrays | 15×15 matrix where cell `(i,j)` is `cosine(panel_A.slot_i, panel_B.slot_j)` ONLY for `i==j` (off-diagonal is undefined / typed-zero / never compared) | Diagonal cells are same-embedder cosines; off-diagonal is structurally undefined |
| Cross-slot SCALAR pairwise relationship within one array | `MI(slot_a_distribution, slot_b_distribution)`, `corr(slot_a, slot_b)`, `agreement_score(slot_a, slot_b)`, `disagreement_score(slot_a, slot_b)` | These are scalar statistics over the two slot distributions; they do NOT align dimensions across the slots, they compute a single number summarizing the relationship |
| Matrix-style operations on the typed structured array | Per-slot weighted aggregation `sum_i w_i * f_i(slot_i)`, slot-set selection / masking via `filled_mask`, masked matrix projections through 15 dedicated linear projections (one per slot), slot-preserving late fusion | Each operation respects slot identity; one linear projection per slot, with the result combined via late fusion, not by reading the packed buffer as one homogeneous vector |
| Array-level aggregate scalar statistics | `min_margin = min over slots of (cosine - threshold)`, `slots_violating_count`, `worst_slot_id` | Aggregates *over* same-embedder comparisons, not *across* embedder spaces |

**What you CANNOT do (forbidden operations):**

| Forbidden | Why |
|---|---|
| `cosine(panel.E_AST_subvector, panel.E1_subvector)` (even if padded to same length) | Different coordinate systems; the dimensions don't refer to comparable axes |
| `linear(5120, hidden_dim)` over the packed buffer | Treats all 5,120 bytes as one homogeneous embedding; silently learns meaningless cross-embedder compensations |
| `residual_norm_sq(predicted.data, target.data)` over the packed 5,120 buffer | Must decompose to per-slot residuals first; per-slot residuals are then aggregated by fail-closed conjunction, never averaged |
| `flat_attention(packed_buffer)` (self-attention over all 5,120 elements as if they were one sequence) | Attention treats positions as exchangeable within a coordinate system; mixing E_AST positions with E1 positions in one attention head is the same error as flat linear projection |
| Padding shorter slots to a common length and then comparing dim-wise | The pad changes the vector length but does not move the vectors into the same coordinate system; the comparison is still meaningless |
| Any operation that allows a strong slot's value to compensate for a violation in a weak slot | The anti-compensation FSV (#394) catches this; strict per-slot conjunction enforces it |

**Why the 5,120 number exists at all.** It is the contiguous byte layout for serialization: the sum of per-slot dimensions, stored as one f32 buffer with per-slot `(offset, dim)` extents in `InstrumentSlot::layout()`. RocksDB persists `Panel` as a JSON envelope containing this buffer + the `filled_mask` bitmap. On every read, the buffer is sliced back into 15 per-slot vectors before any operation. The 5,120 number is for transport; it is never a semantic input.

**Code-level enforcement (already shipped).** `crates/context-graph-mejepa-tct/src/gtau.rs:14` iterates `for embedder in EmbedderId::all()` and computes `cosine_similarity(slot, centroid)` per embedder only. `cosine_similarity` at `gtau.rs:90` errors with `MEJEPA_TCT_DIM_MISMATCH` on any vector-length mismatch (architectural prevention of accidental cross-embedder comparison). `slot_identity_preserved=true` and `flat_vector_concat_used=false` are FSV-asserted at #386 / #387 / #388 / #389 / #390 / #394 / #398. The slot-preserving late-fusion predictor (#387) replaced the prior flat-projection predictor and the FSV proved the swap.

**Where to look in code:**
- Storage layout: `crates/context-graph-mejepa-instruments/src/lib.rs:158-199` (`InstrumentSlot::layout()`)
- Gτ guard (the per-embedder cosine implementation): `crates/context-graph-mejepa-tct/src/gtau.rs:14`
- Dim-mismatch fail-closed: `crates/context-graph-mejepa-tct/src/gtau.rs:90`
- Slot-preserving predictor: `crates/context-graph-mejepa/src/predictor/` (per-slot input encoders + late fusion)
- Anti-compensation FSV: `crates/context-graph-mejepa/examples/python_anti_compensation_fsv.rs` (#394)

### 1.6 The ambiguity boundary (locked-in rule)

ME-JEPA only predicts questions that have a *free binary form against reality*. Free means: the binary verdict comes from observing reality directly, not from imposing an arbitrary threshold on a continuous heuristic. The system does not predict ambiguous things, because:

1. **Ambiguous questions have no ground truth.** "Is this performant?" depends on workload, hardware, baseline, and acceptable margin — there is no objective answer reality renders. Without ground truth, there is no learning signal, and the prediction is unfalsifiable.
2. **Thresholded continuous proxies aren't reality, they're heuristics.** "perf regression ≥ 10%" is a binary heuristic, not a binary fact about reality. Predicting whether mutmut/semgrep/vulture/hyperfine returns a finding predicts what those tools say, not whether the code is good. Predicting heuristics introduces noise that can't be cleaned up via the mistake loop because the heuristic itself is the source of disagreement.
3. **Predicting noise corrupts the mistake-driven loop.** The whole online-adaptation property in §1.3 depends on every mistake row being a real disagreement with reality. Mistakes against heuristic thresholds train the predictor to match the heuristic, not reality. Better to predict zero ambiguous things than to predict noise.

What this excludes from the prediction contract:

| Excluded surface | Why |
|---|---|
| Performance regressions | Continuous metric; threshold is arbitrary; depends on workload |
| Maintainability / tech debt | Taste judgment; no ground truth |
| Idiomatic-ness | Style preference; no ground truth |
| Latent bugs | Cannot enumerate; mutmut mutation survival is a heuristic, not a bug |
| Code redundancy / dead code | Depends on future use; vulture / clone-detection are heuristics |
| Edge-case coverage | Cannot enumerate edge cases by definition |
| Cost (CI minutes, dependency growth) | Continuous; threshold is arbitrary |
| Reasoning quality | Ambiguous; "good reasoning" has no oracle |

What stays in the prediction contract:

| Kept surface | Reality that renders the binary verdict |
|---|---|
| Q1 claim-exists | The bytes / AST on disk after the patch |
| Q2 oracle pass | The Docker oracle's `report.json` (FAIL_TO_PASS / PASS_TO_PASS / etc.) |
| Q5 specific shift events | The durable shift-log JSONL on aiwonder |

Q4 heads remain in the schema and the CFs (`CF_MEJEPA_Q4_*`) as **frozen historical display data**, readable by `mejepa_explain_prediction` for forensic continuity, but not trained, not calibrated as a prerequisite for ship, and not consumed by the predictor's loss. New Phase G bandwidth spends nowhere on Q4 expansion. See `08_roadmap_and_open_work.md §3.1` for the Q4 producer track closure.

### 1.2 The labeling layer (Level 0 SHIPPED 2026-05-19 via #413 / TASK-PY-G-117)

Passing the 300×8 SWE-bench Lite corpus through 13 retrieval embedders (E1–E13) + 15 frozen instrument slots (E_AST … Scalars) produced a large corpus of vectors, pairwise correlations (#388), and per-slot/per-cell calibration state (#389). The 2026-05-19 doctrine reframe identified the **labeling layer** as the missing meaning surface that turns those numbers into named rules. The Level 0 (baseline auto-discovery) implementation **shipped** on 2026-05-19 via TASK-PY-G-117 / #413 with full-corpus FSV PASS.

**What shipped (full-corpus aiwonder run, 2026-05-19):**

- **Implementation:** `scripts/swebench_lite_ops/python_auto_label_discovery.py` (1,641 lines, deterministic Python, no inner LLM).
- **CLI entrypoint:** `python-auto-label-discovery` (registered through `scripts/swebench_lite_ops/cli.py`).
- **Output root:** `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/`.
- **FSV:** `/zfs/archive/contextgraph/fsv/task-py-g-117-auto-label-discovery-usefulness-fsv/python_auto_label_discovery_fsv.json` (`passes=true`; manifest sha256 `3cd0e3c6…`, report sha256 `3b3e4b0b…`).
- **Full-corpus counts:** 399,777 chunk constellation labels · 4,665,772 slot forward records consumed · 25,289,552 pair relationship labels · 3,106,565 group labels · 3,000 code-state constellations · 4,224 file-path constellations · 1,139 failure evidence sets · 549 usefulness metrics · 491 accepted label registry rows · 12/12 manual-review reproductions matched.
- **Architecture commitments enforced in FSV:** `slot_identity_preserved=true`, `flat_vector_concat_used=false`, no active `/mnt/d` reads/writes.
- **Leakage policy:** target-side labels (oracle/docker/test-phase outcomes, code-state outcomes) are emitted via `leakage_policy:target_side_only` — supervision/reward inputs to the trainer only, **never** live predictor inputs. Live inputs are slot-local observations, pair/group relationships, repository facts, and durable non-target evidence.

**Three artifact levels (all materialized, with stable JSONL hashes per the #413 RESOLVED comment):**

| Level | Object | What's materialized |
|---|---|---|
| **0 — Chunk** | Per-chunk slot-local + pair + group labels | `chunk_constellation_labels.jsonl` (sha256 `4ad438ff…`); 399,777 rows. Powered by a deterministic label compiler that emits AST/source surfaces, source-site relation (exact-mutation / changed-file / wrong-file / missing-target / test-patch / no-compact-source-site), mutation mechanism micro-labels, per-slot scalar/region labels (`norm_l2`, `std`, `abs_mean`, `entropy_proxy`, `zero_fraction`), pair relationship labels, and group signatures across `temporal_triad` / `semantic_content_panel` / `sparse_lexical_panel` / `sparse_hdc_bridge` slot sets. |
| **1 — File / path** | File-level constellations | `file_path_constellations.jsonl`; 4,224 rows. Slot-local aggregate distributions, pair/group relationship counts, AST surface counts, mutation/source-site evidence, partition metadata. |
| **2 — Code state** | `(task_instance_id, mutation_category, workspace_state)` constellations | `code_state_constellations.jsonl` (sha256 `582cc0de…`); 3,000 rows. Code-state outcome labels (`code_state_outcome:working_as_intended` vs `code_state_outcome:failed_or_problematic`) are **target-side leakage-policy:target_side_only**. |

**Three localization modes** distinguished deterministically for failed/problematic code states (`failure_evidence_sets.jsonl`, sha256 `747f8298…`; 1,139 rows):

- `localization:single_point_candidate` (612 cases) — one chunk/source-site dominates the deterministic evidence
- `localization:multi_point_evidence_set` (1,479 cases) — several chunks jointly explain the failure risk
- `localization:diffuse_code_state_risk` (18 cases) — whole code state is failed/problematic but no compact chunk set is currently justified
- `localization:working_state_no_failure_localization` (891 cases) — no failure to localize

**Usefulness scoring** (`label_usefulness_metrics.jsonl`, sha256 `923a618f…`; 549 metrics) scores every candidate label on support count + minimum-support rejection, pass/fail lift + precision/recall against oracle/Docker outcomes, train/calibration/holdout stability, closest-exemplar oracle agreement, incremental predictive contribution over simpler labels, degenerate/leaky-label rejection, and typed uncertainty for unknown/unsupported/low-power labels. Only labels that survive every gate enter the `accepted_label_registry.jsonl` (sha256 `3846bef3…`; 491 accepted rows).

**Learning bridge manifest** (`learning_bridge_manifest.json`, sha256 `5fc7e11b…`) maps accepted label IDs into the **existing** UTML/replay machinery — no new training loop required:
- `delta_p.per_chunk_values` consumes per-chunk Level-0 labels
- `delta_xi.constellation_violation_rate` consumes constellation-pattern labels (`CF_MEJEPA_CONSTELLATION_PATTERNS`)
- `ReplayBufferRow.cell_id` keyed by `(mutation_category × language)` cell from Level-2 code-state constellations
- Active-learning queue (`CF_MEJEPA_ACTIVE_LEARNING_QUEUE`) consumes unknown/OOD reasons
- Ontology-growth audit (`CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT`) consumes label promotion/rejection events

**What's next (NOT shipped — open work):**

| Layer | Status | Tracked under |
|---|---|---|
| Level 0 chunk labels + Level 1 file constellations + Level 2 code-state constellations | ✅ SHIPPED 2026-05-19 | #413 closed |
| Hierarchical naming on top of the 491-label registry (failure-mode skills + behavioral constellations à la ClipCannon's Action-Unit → Group → Skill → Constellation hierarchy) | Open — uses the published SWE-bench taxonomies (Aleithan et al. 2025, Yang et al. 2024, SWE-Bench Pro 2025) as the operator-curated seed vocabulary on top of the deterministic Level-0 registry | #404 / TASK-PY-G-110 |
| Docker oracle outcome ontology (deterministic test-phase vector labels: `FAIL_TO_PASS`, `PASS_TO_PASS`, `FAIL_TO_FAIL`, `PASS_TO_FAIL` + `docker_resolved_clean`, `target_fail_to_pass_tests_failed`, etc.) | Open — feeds Level-2 code-state outcome supervision | #411 / TASK-PY-G-116 |
| Online mistake-driven consumption of the accepted registry | Open — uses stable label/evidence IDs from #413 as the mistake context | #406 / TASK-PY-G-112 |
| MCP tools wrapping the CLI for operator-side label inspection | Open — `mejepa_list_failure_modes`, `mejepa_label_failure_cluster`, `mejepa_explain_named_failure_mode`, `mejepa_failure_mode_coverage_audit` | #404 / TASK-PY-G-110 |

A binary prediction becomes actionable when the predictor returns `Fail + named_failure_mode + closest_exemplar`. The agent that consumes the prediction can act on a named mode the way a senior reviewer would; the operator can audit the prediction the way they would audit a named bug class. **The deterministic label substrate that powers this is now full-corpus shipped; the hierarchical naming + MCP surface on top is what #404 still owns.**

### 1.3 Online mistake-driven adaptation (the "isn't wrong in the future" property)

The predictor's head over the panel is a margin-based binary classifier wrapped in an **online-mistake-driven update loop** (Rosenblatt 1957 perceptron lineage, with second-order/kernel variants from Cesa-Bianchi & Conconi 2005 and Orabona 2008 for stability):

1. Predictor returns `Pass | Fail` for a candidate patch.
2. Reality (Docker oracle + PostToolUse capture + shift-log replay) returns the ground truth.
3. If prediction matches reality → no update; the panel + slot calibration was already discriminative enough.
4. If prediction disagrees → **mistake row** lands in `CF_MEJEPA_MISTAKE_LOG`. The mistake row carries the panel, the slot trace, the named failure mode the predictor *thought* applied, the named failure mode the closest exemplar suggests, and the ground-truth outcome. The trainer consumes the mistake row under EWC++ catastrophic-forgetting protection (already in #TASK-PY-G-065) plus prioritized replay weight (#TASK-PY-G-068).
5. Convergence target: for the same `(mutation_category × language × named_failure_mode)` cell, the predictor must not repeat the mistake on the next prediction. The mistake log is the proof.

This replaces the multi-shot best-of-K pattern and the calibrated-coverage Abstain pattern with a **one-shot + adapt** discipline. The agent never sees multiple candidate verdicts; it sees one binary answer and a structured explanation, both of which are subject to falsification by the next ground-truth observation.

### 1.4 Cross-panel triangulation (the Goodhart guard)

The predictor is trained against the 15-slot instrument panel. The Gτ guard is evaluated against the same 15-slot panel. That is **encoder-matched** by construction and creates a Goodhart risk: hitting 0.95 on this panel does not prove the prediction is right against reality; it could prove that the predictor learned the panel.

A second, deliberately non-overlapping panel (Panel B) is materialized for hold-out triangulation:

- E_AST: tree-sitter → CodeBERT AST encoder
- E_CFG: attention encoder → Pythia program-graph encoder
- E_DataFlow: attention encoder → joern-driven data-flow encoder
- E_Code (E7-equivalent): Qodo-Embed-1 → Voyage-code-2
- E_Test/E_Problem/E_CommitMsg (e5 reuse): e5-large-v2 → bge-large-en-v1.5

When Panel A clears the gate, Panel B re-scores the same 30-instance holdout (and 240-instance train-visible window). If Panel B oracle correlation stays within 0.05 of Panel A, the gate is genuinely external. If Panel B drops by ≥0.10, the predictor was Goodharting Panel A, and the result is fail-closed for ship.

This is the analogue of PAPER §6.3's named-future-work item "cross-encoder triangulation with ECAPA-TDNN" — except where PAPER scoped it as future work for voice cloning, ME-JEPA can do it now because the predictor is already architecturally separated from the panel.

---

## 2. Process / Port / Tech Architecture

| Process | Port / IPC | Technology | Purpose |
|---|---|---|---|
| `context-graph-mcp` server (`--mode stdio` or `--mode reality-loop`) | stdio JSON-RPC (default) or HTTP transport | Rust, tokio, async-trait | 196-tool MCP surface; daemon mode supervises long-lived state |
| `context-graph-cli` | CLI invocation | Rust | Operator commands: `mejepa observe`, `mejepa project-ingest`, `mejepa compression-progress`, `mejepa active-learning`, etc. |
| ME-JEPA training daemon | embedded in CLI/binary | candle (CUDA 13.2), RocksDB | UTML training loop; reads `CF_MEJEPA_REPLAY_BUFFER`; writes `CF_MEJEPA_TRAIN_CERTS` |
| Shift-log subscriber | tokio task | tokio, async streams | Tails JSONL shift log → materializes panels (`crates/context-graph-mejepa-shift-subscriber`) |
| Heal scheduler | embedded thread | tokio | Drift detection, Fisher/EWC, distill, promotion (`crates/context-graph-mejepa/src/heal/`) |
| Single RocksDB (per runtime root) | local disk | RocksDB ≥ 8.x | 200+ column families (51 base + ~120 ME-JEPA + 19 teleological + 13 quantized + 5 code + 2 causal). See `03_storage_schema.md`. |
| GPU runtime | CUDA 13.2 on RTX 5090 Blackwell | cudarc 0.19.x via candle 0.10.2 | Embedder forward passes, predictor training |

**No HTTP from action paths.** `WebSearch`/`WebFetch`/`mcp__exa__*` are read-only research tools. The MCP server is the only network surface for runtime work.

**No inner LLM.** Local generative inference (Qwen / Hermes / Ollama / llama.cpp) was retired 2026-05-09. `feature = "llm"` triggers `compile_error!` (`crates/context-graph-mcp/src/lib.rs:13`).

**No intent synthesis.** North-star / intent modeling is retired by ADR #358. Observable goals and agent claims may be stored as evidence in the reality chain; hidden human intention is not a prediction target.

---

## 3. Technology Stack

| Layer | Technology | Version constraint | Where pinned |
|---|---|---|---|
| Language | Rust | edition 2021, MSRV 1.95 | `Cargo.toml:64-66` |
| Async runtime | tokio | 1.35 (`features = ["full"]`) | `Cargo.toml:72` |
| Serialization | serde, serde_json, bincode 1.3, toml 0.8 | — | `Cargo.toml:76-79` |
| Error | thiserror, anyhow | — | `Cargo.toml:82-83` |
| Telemetry | tracing, tracing-subscriber (json/env-filter) | — | `Cargo.toml:86-87` |
| Config | `config` crate | 0.14 | `Cargo.toml:90` |
| IDs / time | uuid (v4/v5/serde), chrono (serde) | uuid 1.6, chrono 0.4 | `Cargo.toml:93-94` |
| ML runtime | candle-core, candle-nn (CUDA) | 0.10.2 (cudarc 0.19.x, CUDA 13.2) | `Cargo.toml:101-104` |
| Storage | RocksDB | via `rocksdb` crate (see per-crate manifests) | `crates/context-graph-storage/Cargo.toml` |
| Crypto / hashing | ed25519, sha2, blake3 | — | `crates/context-graph-witness/Cargo.toml` |
| Vector search | usearch HNSW (patch-similarity), hnsw_rs (embedder indexes) | — | per-crate |
| Build profiles | `release` (thin LTO, 16 CGUs), `release-max` (1 CGU), `dev` (opt 0, line-info debug, incremental) | — | `Cargo.toml:112-128` |
| License | PolyForm-Noncommercial-1.0.0 | — | `Cargo.toml:67` |

---

## 4. Workspace Members (25 active default + 1 excluded)

`Cargo.toml:6-32` lists all members; `default-members` (lines 36-61) excludes `context-graph-benchmark` (87K lines, build explicitly with `-p`). Full per-crate detail in `02_source_map.md`.

| Crate | Role | LOC (src lib.rs only) |
|---|---|---|
| `context-graph-mcp` | MCP JSON-RPC server + 196 tools | 29 (lib) + 136K (main.rs) |
| `context-graph-cli` | Operator CLI, helpers | 14 (lib) + 230 (main.rs) |
| `context-graph-core` | Shared domain types | — |
| `context-graph-storage` | RocksDB wrapper, teleological/code/causal CFs | 190 |
| `context-graph-witness` | Tamper-evident chain primitives (ed25519, Merkle) | 225 |
| `context-graph-solver` | Causal masking, MMR, conformal predictor | 31 |
| `context-graph-mincut` | Clean-room Stoer-Wagner global min-cut | 333 |
| `context-graph-paths` | Fail-closed durable path resolver | 614 |
| `context-graph-tier-compression` | 8/7/5/3-bit groupwise scalar quantization | 516 |
| `context-graph-embeddings` | E1-E14 embedding pipeline | 135 |
| `context-graph-cuda` | CUDA helpers | — |
| `context-graph-mejepa` | **Predictor trunk** (compiler, conformal, OOD, claim graph, patch similarity, project-ingest, ...) | 175 (lib); 73 module files |
| `context-graph-mejepa-cf` | Canonical column-family name registry (zero-dep) | 819 |
| `context-graph-mejepa-corpus` | 8× mutation operators + SWE-bench bridge | 822 |
| `context-graph-mejepa-instruments` | 15-slot typed panel storage layout + frozen instruments | 595 |
| `context-graph-mejepa-embedders` | 21 embedder slots, dynamic registry, calibration | 63 |
| `context-graph-mejepa-train` | UTML trainer, replay buffer, certificates | 51 |
| `context-graph-mejepa-tct` | Teleological constellations + Gτ guard | 49 |
| `context-graph-mejepa-hygiene` | Tier transitions, quota, GC, witness compress | 37 |
| `context-graph-mejepa-shift-subscriber` | JSONL shift-log tail → panel materialization | 26 |
| `context-graph-dynamicjepa` | 5090jepa MCP surface (39 tools) | — |
| `context-graph-causal-agent` | Deterministic causal extraction | — |
| `context-graph-graph-agent` | Graph extraction / discovery | 66 |
| `context-graph-test-utils` | Shared test fixtures (NO MOCK DATA) | 23 |
| `context-graph-benchmark` *(excluded from default build)* | 87K-line benchmarking suite | — |

---

## 5. Entry Points

| Binary / command | Crate | Purpose |
|---|---|---|
| `context-graph-mcp` | `context-graph-mcp` | MCP server. `--mode stdio` (default) or `--mode reality-loop` (lightweight subset). Multi-agent observability via `mejepa_daemon_status`. |
| `context-graph-cli` | `context-graph-cli` | Operator CLI. Subcommands include `mejepa observe`, `mejepa project-ingest`, `mejepa project-report`, `mejepa compression-progress`, `mejepa active-learning {list|label|dismiss}`, `mejepa stop-self-verify`, `mejepa eval synthetic-stress`. |
| `mejepa` | `context-graph-mejepa` `[[bin]]` | Standalone predictor harness. |
| `mejepa-heal-drill` | `context-graph-mejepa` `[[bin]]` | Heal scheduler drill. |
| `mejepa-train` | `context-graph-mejepa-train` `[[bin]]` | Trainer entry. |
| `train-causal` | `context-graph-embeddings` `[[bin]]` | Causal embedder training. |
| 50+ FSV examples (`crates/*/examples/*_fsv.rs`) | per crate | Per-task FSV harnesses. See `07_results_and_benchmarks.md` for the full index. |

---

## 6. Directory Layout (what exists at runtime)

```
crates/                                — Rust workspace (1567 .rs src files, 189 example files, 258 test files)
docs/futurebuild/                       — this doc suite (8 condensed docs as of 2026-05-18)
docs/futurebuild/_archived/             — original 31 planning docs (read-only history)
docs/futurebuild/specs/FSV-PROTOCOL.md  — mandatory verification contract (still authoritative)
docs2/AICodingAgentSuperPrompt.md       — master doctrine (universal protocol)
docs2/hardeningprompt.md                — 14-axis hardening deep-dive
scripts/cc/                             — Claude Code hook scripts (PostToolUse, Stop, UserPromptSubmit)
scripts/ci/                             — CI lint (e.g. check_no_c_drive_paths.sh)
scripts/swebench_lite_ops/              — Python redaction + SWE-bench Lite helpers
tasks/swebench-lite/                    — task manifest JSON (workstreams)
config/                                 — governed paths, model pins
.claude/                                — settings.json, agents/, loop.md (1 line; see specs)
.github/workflows/, .github/labels.yml  — CI + label set (memory = Issues)
memory.archive/                         — read-only legacy SAMP/SMAP (526 files, do not write)

/zfs/archive/contextgraph/              — **aiwonder production durable SoT** (see 03_storage_schema.md §5)
  corpus/                               —   production corpus, panel cache, fingerprint registry
  fsv/                                  —   per-task FSV evidence (post-migration target)
  runtime/                              —   shift log, test outcomes, scheduler events
  projects/<project_id>/                —   reality-compiler per-project cache, manifest, report
  state/                                —   durable runtime state surviving restarts
  exports/                              —   weekly eval reports, operator exports

/zfs/hot/contextgraph/                  — **aiwonder hot state**
  repo/                                 —   git checkout, release binaries
  models/                               —   embedder model weights
  corpus/                               —   stable hot inputs
  runtime/                              —   active scheduler logs / hot run state

/mnt/d/contextgraph/                    — **legacy/local SoT** (327 FSV dirs as of 2026-05-18)
                                          historical for production unless an issue explicitly
                                          scopes a local-only smoke (#369 tracks revalidation)
```

---

## 7. Storage Tier Classification

| Tier | What | Where | Regenerable? |
|---|---|---|---|
| **Sacred** (never edit) | Operator `.env`/`.env.*`; `swebench/**`; `tests/oracle_**/**`; `crates/*/src/reward_specs/**`; `crates/*/src/verifiers/**`; `crates/context-graph-benchmark/datasets/swe_bench_*/**`; `*.pem`, `*.key`, `**/credentials.json` | various | No |
| **Durable production** | RocksDB CFs (51+ teleological/quantized/code/causal + 120+ ME-JEPA); FSV evidence; project-ingest manifests; weekly exports | `/zfs/archive/contextgraph/` on aiwonder | Source-recoverable (corpus) or replay-from-shift-log (predictions). Many FSV artifacts are irreplaceable single events. |
| **Hot** | Active checkpoints, model weights, hot run state | `/zfs/hot/contextgraph/` on aiwonder | Yes (re-download / rebuild) |
| **Ephemeral** | `/tmp/contextgraph-*` scratch | `/tmp/` | Yes |
| **Legacy/local** | Pre-migration FSV + corpus + shift-log | `/mnt/d/contextgraph/` | Frozen as historical evidence; #369 reclassifies |

---

## 8. Exception Hierarchy (fail-closed structured errors)

Every crate defines its own `Error` enum with `SCREAMING_SNAKE_CASE` code via `.code()`. Truncated catalog:

| Crate | Top-level error type | Sample error codes | Where defined |
|---|---|---|---|
| `context-graph-mejepa-instruments` | `InstrumentError` (`InvalidInput`, `NumericalInvariant`, `FrozenViolation`, `Store`) | `MEJEPA_INSTRUMENTS_INVALID_INPUT`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`, `MEJEPA_INSTR_FROZEN_VIOLATION`, `MEJEPA_INSTRUMENTS_STORE_ERROR` | `crates/.../src/lib.rs:62-105` |
| `context-graph-mejepa-corpus` | `MutationError` (`InvalidInput`, `NoMutationSite`, `OperatorFailed`, `LeakageDetected`, `SubprocessTimeout`) | `MEJEPA_CORPUS_INVALID_INPUT`, `MEJEPA_CORPUS_NO_MUTATION_SITE`, `MEJEPA_CORPUS_OPERATOR_FAILED`, `MEJEPA_CORPUS_LEAKAGE_DETECTED`, `MEJEPA_CORPUS_SUBPROCESS_TIMEOUT` | `crates/.../src/lib.rs:111-160` |
| `context-graph-mejepa` | `PredictorError`, `MejepaInferError`, `LossError`, `NanSource` | (see `crates/context-graph-mejepa/src/error.rs`) | `error.rs` |
| `context-graph-mejepa-train` | `TrainerError`, `TrainerErrorCode`, `UtmlError`, `UtmlErrorCode` | (see `error.rs`) | `crates/.../src/error.rs` |
| `context-graph-mejepa-tct` | `Tct*Error` | constellation freshness, calibration cell support, Gτ guard | `error.rs` |
| `context-graph-mejepa-hygiene` | hygiene `Error` | tier transitions, quota overflow, GC | `error.rs` |
| `context-graph-mejepa-shift-subscriber` | `SubscriberError` | watermark, decode, tail | `error.rs` |
| `context-graph-storage` | `StorageError`, `QuantizedStorageError`, `TeleologicalStoreError`, `CodeStorageError`, `GraphEdgeStorageError` | open / CF missing / decode failure | `error.rs` |
| `context-graph-mcp` (server/handlers) | retired tools → `CCREALITY_ENGINE_RETIRED` fail-closed | — | `deprecation.rs` |

**Fail-closed contract.** Missing CFs, missing files, missing models, missing env vars, malformed inputs MUST return a structured error with `code()`. No silent defaults; no permissive fallbacks. Doctrine § 1 / FSV-PROTOCOL § 3.

---

## 9. Major Subsystems (one paragraph each)

**MCP server (`context-graph-mcp`).** JSON-RPC 2.0 stdio (default) or HTTP transport. 196 tool definitions exported by `tools/definitions/mod.rs::get_tool_definitions()`: 49 are `mejepa_*` (Phase 4/5/6/8/F/G/EK predictor + heal + observe + ship-gate dashboard). Also: 39 `dynamicjepa_*` (5090jepa surface), 14 learning, 18 UTL learner-state, 6 constellation, 6 entity, 10 embedder, 5 core memory, 4 each for topic/file_watcher/sequence/causal/graph_link/contrastive, 3 each for typed_edges/provenance, plus singletons. Retired `reality_*` tools fail closed with `CCREALITY_ENGINE_RETIRED`. Reality-loop mode (`--mode reality-loop`) exposes the post-pivot Claude-Code-outer-loop subset only. Server enforces multi-agent isolation via path-policy + edit-lock (see `06_operations_and_fsv.md`).

**Predictor (`context-graph-mejepa`).** CUDA-only production predictor. 73 modules covering claim graph (Q1), patch-similarity HNSW exemplars (Q3), Q4 security/perf/cost/accuracy heads, Q5 shift-log replay impact, multi-head VICReg + Huber loss, conformal calibration, OOD detection, claim-vs-reality reconciliation, project-ingest reality compiler, synthetic-stress evaluator, adversarial corpus, dynamic embedder registration, foundationality (PageRank on chunk/library/embedder dep graphs), label transfer audit, operator override surface. `compile()` and `compile_with_fingerprint_observation()` are the canonical Q1-Q5 entry points.

**Frozen instruments (`context-graph-mejepa-instruments`).** A `Panel` is an **array of 15 per-embedder vectors** (a teleological constellation), one vector per slot, each vector living in its own embedder's space: `E_AST (384)`, `E_CFG (256)`, `E_DataFlow (256)`, `E_TypeGraph (256)`, `E_Test (384)`, `E_Trace (512)`, `E_Diff (256)`, `E_Witness (256)`, `E_Oracle (128)`, `E_Problem (1024)`, `E_CommitMsg (384)`, `E_StaticAnalysis (256)`, `E_Runtime (256)`, `E_Reasoning (384)`, `Scalars (128)`. The 5,120 f32 contiguous buffer + per-slot byte offsets is the serialization format; **the semantic unit is the array of 15 per-embedder vectors, NOT a 5,120-d fused embedding**. Cross-embedder dimension-wise comparison is forbidden (E1's dim-3 is not comparable to E_AST's dim-3 — those numbers live in unrelated coordinate systems). Valid operations: same-embedder dim-wise compare (E1↔E1, E_AST↔E_AST, etc.); array-vs-array operations on the structured array (per-slot cosine map, similarity matrices, matrix-style projections that respect slot identity, masked aggregation); cross-slot SCALAR pairwise relationship metrics within the same array (MI, correlation, agreement). `Panel`/`PanelBuilder` enforce fail-closed dim+finite+double-fill validation. Instrument trait carries zero trainable params (structural collapse-immunity invariant `ΔΞ.target_collapse = 0`).

**Embedders (`context-graph-mejepa-embedders`).** 21 slots (E5 retired). 12 active content embedders (8 pretrained + 4 algorithmic). Frozen-by-design weights: E1, E6, E7, E8, E10, E12, E13, E14. Dynamic-registration scaffolding (`dynamic_registry.rs`) + VRAM budget enforcement (`vram.rs`) + falsification gate + freeze audit + adversarial-injection mitigation. SHA-256 manifest pins for every pretrained weight. CUDA-required for production; CPU construction rejected.

**Corpus generator (`context-graph-mejepa-corpus`).** 8 mutation categories × 11 languages = 88 generic operators (Python uses the parser-backed operators; other 10 languages use `generic_language_ops`). Deterministic SplitMix64 PRNG. Categories: `KnownGood`/`SubtleFlip`/`OffByOne`/`SwapVariable`/`DeleteTestCall`/`WrongFile`/`OverEngineer`/`CompileError`. SWE-bench Lite bridge (`swebench.rs`) wraps `python -m swebench.harness.run_evaluation` per-instance, parses `report.json`, converts `FAIL_TO_PASS`/`PASS_TO_PASS`/`FAIL_TO_FAIL`/`PASS_TO_FAIL` → `OracleVerdict`. Split modes: `InstanceAtomic` (default, 240/30/30 on Lite) and `RepoAtomic` (195/77/28, repo-bounded). Provenance via SHA256 hash of sorted `(task_id, category, patch_sha256, oracle_verdict_sha256)` rows.

**Trainer (`context-graph-mejepa-train`).** UTML v1.0 implementation. `compute_l_step` evaluates `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. EWC++ (`task-py-g-065`), continual backprop (Dohare 2024, `task-py-g-066`), prioritized replay buffer over `CF_MEJEPA_REPLAY_BUFFER` keyed by surprise + per-cell coverage gap, inverse-map head (TASK-EK-012), per-fingerprint Fisher / dormancy (TASK-FP-008), compression-progress reporter (TASK-EK-001) producing CDL_bits weekly. Outputs `TrainingCertificate` rows in `CF_MEJEPA_TRAIN_CERTS`.

**TCT (`context-graph-mejepa-tct`).** Teleological constellations + Gτ guard. Cell key = `(mutation_category, language, additional_dims)`. Per-cell calibration, freshness audit (`freshness_policy.rs`), constellation refresh scheduler (`CF_MEJEPA_CONSTELLATION_REFRESH_LOG`). `Gτ` rejects predictions with insufficient cell support. Refresh-report renderer emits human-readable triage.

**Hygiene (`context-graph-mejepa-hygiene`).** Tier transitions (hot → warm → frozen), quota enforcement, GC reports, witness-chain Merkle compression. MCP surfaces: `mejepa_quota_status`, `mejepa_gc_run`, `mejepa_witness_compress`.

**Shift subscriber (`context-graph-mejepa-shift-subscriber`).** Tokio task tailing JSONL shift log (`/zfs/archive/contextgraph/runtime/shift-log.jsonl` in production, `/mnt/d/...` historical). Decodes `ShiftEntry` → calls `shift_to_inference()` → materializes `Panel` → writes `CF_MEJEPA_PANELS` + `CF_MEJEPA_SHIFT_WATERMARK`. Watermark write is the durability gate.

**Storage (`context-graph-storage`).** RocksDB wrapper. 19 teleological CFs (fingerprints, e1_matryoshka_128, e12_late_interaction, e13_splade_inverted, e6_sparse_inverted, content, source_metadata, file_index, topic_portfolio, entity_provenance, audit_log, audit_by_target, merge_history, importance_history, tool_call_index, consolidation_recommendations, embedding_registry, custom_weight_profiles, topic_profiles). 13 quantized embedder CFs (`emb_0..emb_12`). 5 code CFs (entities, e7 embeddings, file/name/signature indexes). 2 causal CFs (relationships, by_source). Per-CF `Options` with column-specific compaction. HNSW configs per embedder; inverted index for sparse embedders.

**CLI (`context-graph-cli`).** Operator CLI. Helpers in `governed_edit.rs` (path-policy edits), `mcp_client.rs` (call MCP from CLI scripts), `mcp_helpers.rs`. `exit_code_for_error()` maps known structured errors to deterministic exit codes for CI/scripts; `is_corruption_indicator()` triggers harder failure modes.

**Witness (`context-graph-witness`).** Tamper-evident chain primitives. ed25519 segment signing (feature `ed25519`), Merkle aggregation, Stage 0 + Stage F + Merkle certificates with full metadata binding (corpus_id, policy_version, query_id, head_id, latency_ms, trajectory_count).

---

## 10. What This System Is NOT

- Not a generative inference engine — no inner LLM. The product is the *verifier*, not a code-writer.
- Not a benchmark harness — `context-graph-benchmark` exists separately and is intentionally not in the default build.
- Not a hosted service — runs locally on operator hardware (aiwonder for production durable state).
- Not a Python codebase — Python only shows up as the *corpus subject* (SWE-bench Lite) and in tool wrappers (`scripts/swebench_lite_ops/*`).
- Not cross-language at ship-gate time — current ship gate is Python-only (300×8 Python corpus). Non-Python `Language::*` variants exist in the corpus generator but are deferred from the current gate.
- Not HTTP-callable from action paths — the MCP server is the only network surface for runtime work.

---

## Cross-references

- Per-crate source map + dependency graph → `02_source_map.md`
- Every RocksDB column family with key/value format → `03_storage_schema.md`
- Every MCP tool with parameters and returns → `04_mcp_tools.md`
- Embedder slots / instruments / heads / TCT / corpus / heal / training → `05_predictor_pipeline.md`
- FSV protocol, runbook, DR, observability, test suite → `06_operations_and_fsv.md`
- FSV evidence history, ship-gate progress, ablations → `07_results_and_benchmarks.md`
- Current open work, Phase G/I/J, deferred items → `08_roadmap_and_open_work.md`
- Doctrine (universal protocol) → `docs2/AICodingAgentSuperPrompt.md`
- Project addendum (invariants, deny-list) → `CLAUDE.md`
- Per-turn agent contract → `AGENTS.md`
