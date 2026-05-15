# 12 — Future Capabilities (What the Corpus Unlocks)

This document covers the **speculative-but-plausible** capabilities the labeled corpus + 12-embedder cross-attention + 15-instrument panel will enable, beyond the core Q1-Q5 prediction. Some are direct consequences of the data we're producing; some require additional engineering. None require the LLM. All are grounded in the durable-reality substrate the seed corpus is currently building.

This list is operator-authored, not exhaustive. Once the corpus lands and we can actually see the reward field, more capabilities will surface. The point is: pass/fail prediction is the **floor**, not the ceiling.

---

## 1. Failure-shape fingerprints

**Claim:** Patches that look unrelated *lexically* may share **failure geometry** across embedders.

**Mechanism:** When E1 (semantic) says two chunks are far apart but E7 (code) + E_DataFlow + E_TypeGraph all cluster them together in an embedding subspace that historically failed oracle, that subspace is a **failure-shape fingerprint**. Examples:
- Bad API assumptions (call signature mismatch, wrong return type expected)
- Stale import paths (module renamed, import not updated)
- Type contract mismatch (None reaches non-Optional, dict reaches list-typed param)
- Missing branch coverage (added a code path with no test)
- State mutation in the wrong layer (mutating shared state from a function that shouldn't)
- Exception swallowing (`except: pass`)
- Bad boundary handling (missing `if` for empty/zero/null edge case)

**Output:** ME-JEPA learns *"this patch shape tends to fail reality even before tests run"* and surfaces the fingerprint name + the nearest historical exemplar.

**Status:** Reachable from current corpus + 12 embedders + frozen E_DataFlow / E_TypeGraph instruments once Phase G trains them.

---

## 2. Claim-to-disk existence prediction (Q1, deeper)

**Claim:** For agent claims like *"I added validation to X"*, the system can predict whether the claimed behavior **actually exists in the bytes** vs the edit only touching adjacent surface area.

**Mechanism:** Compare the structured-claim atomic assertion `(verb=added, target=validation, location=X)` against:
- AST diff (typed structural delta in `X`)
- File-level witness chain (did `X`'s SHA change?)
- Context embeddings of `X` pre-edit and post-edit
- Reachability: is the claimed behavior in the new control flow?

**Output:** Per-claim verification status (verified / refuted / adjacent) with reasoning.

**Status:** Q1 head, design done, implementation Phase G+. The structured-claim parser is the open work.

---

## 3. Oracle outcome prediction before execution

**Claim:** With SWE-bench labels + generated mutation outcomes, the predictor maps `(before code, patch, context)` to the **frozen-instrument target panel**: test pass/fail, parse health, import health, call-graph deltas, dependency touch points, warning profiles, affected symbols.

**Mechanism:** This is the **core Q2 head**, calibrated against `E_Oracle`. The Phase G training run is exactly this objective. ME-JEPA emits a calibrated probability of oracle Pass with a conformal interval.

**Output:** `Verdict::Pass | Fail | OutOfDistribution | Abstain | GuardRejected` + per-test predicted outcomes + per-instrument predicted deltas.

**Status:** Currently 0.867 oracle correlation; gating on Phase G to reach 0.95.

---

## 4. Hidden reward from embedder disagreement

**Claim:** The strongest signal may be when **embedders disagree**.

**Mechanism:** For each chunk, compute disagreement across the 66 unordered embedder pairs:
- E1 (semantic) says "high similarity to centroid"
- E8 (graph) says "structurally distant"
- E6 (sparse) sees "API token drift"
- E2/E3/E4 (temporal) sees "pattern emerged after a dependency-era boundary"

When the embedders disagree on a chunk's cell membership, the disagreement vector becomes a **high-latent-risk signal**: *"This patch looks semantically similar to passing patches but structurally is different in ways our other embedders catch."*

**Output:** Per-chunk disagreement score; surfaced as `Verdict::Abstain` triggers or as an explicit `embedder_disagreement_severity` field in `RealityPrediction`.

**Status:** Reachable directly from the forward-cache once cells materialize. The 66 pairwise cosines are part of the 155-signals-per-chunk reward field.

---

## 5. Repository-specific invariants

**Claim:** The corpus can reveal **project-level laws** that don't generalize across projects but are sharp within a project:
- Which files tend to co-change (the "this PR usually touches both `models/` and `migrations/`" pattern)
- Which call paths are fragile (functions where 80% of past edits broke the test suite)
- Which test files validate which source paths (`tests/test_X.py` is the ground truth for `src/X.py`)
- Which AST shapes correlate with regression (e.g., `try/except` around `requests.get` in django repo correlates with future test flakes)
- Which import/module edges break often (e.g., `from .util import *` is a regression magnet)

**Mechanism:** Per-project cells. The cell key includes `project_id`, so the predictor learns per-project signatures. Cross-project transfer happens via the embedders' pretrained semantic component.

**Output:** Per-project invariant index; surfaced in Q5 (Reality Impact) as "this patch violates a learned invariant of repo X."

**Status:** Reachable after Phase G corpus training. Per-project cell stratification is already in the design.

---

## 6. Patch blast-radius prediction

**Claim:** Multi-embedder correlations can answer **"what else could go wrong?"** by identifying chunks whose embeddings shift in relation to the patch even though they weren't edited. Surfaces hidden affected areas: tests, dependency callers, config paths, serialization contracts, performance-sensitive loops, security boundaries.

**Mechanism:** For each chunk in the codebase (not just the patched chunks):
1. Compute its post-edit joint embedding (counterfactual: how would this chunk's vectors change if the patch's typed-delta propagated to it?)
2. Compare to pre-edit embedding.
3. Rank by shift magnitude.
4. Surface top-K affected chunks as blast radius.

**Output:** Per-patch ranked list of affected chunks, each tagged with affected dimension (test / config / serialization / perf / security).

**Status:** Q5 head head; reachable after Phase G + static-reality-graph integration.

---

## 7. Counterfactual repair ranking

**Claim:** Once the target instruments are stable, ME-JEPA can **score candidate fixes before execution**: *"this edit is more likely to satisfy the oracle and preserve invariants than that edit."*

**Mechanism:** The agent emits N candidate patches. For each:
1. Run the full 12-embedder forward.
2. Materialize the predicted post-edit panel.
3. Run the predictor head → get `(predicted_oracle_pass, confidence, OOD_score)`.
4. Rank the N candidates by `predicted_oracle_pass × (1 - OOD_score)`.
5. Return the ranking to the agent or operator.

This becomes **feedback to coding agents without relying on the LLM's self-assessment**.

**Output:** Per-candidate-patch score; the agent picks the top-ranked patch to apply.

**Status:** Already partially exposed via `mejepa_predict_what_if` MCP tool. Full counterfactual ranking is post-ship-gate work.

---

## 8. Calibration of uncertainty

**Claim:** The system can **learn when it does not know**.

**Mechanism:** Out-of-distribution detection. If:
- The joint embedding falls in a low-density region of the training distribution
- Cell membership is weak (no centroid within Gτ threshold)
- Historical patches with similar fingerprints split between Pass and Fail (high entropy)
- Conformal interval too wide

Then ME-JEPA emits `Verdict::Abstain` + a `missing_evidence` field describing what specifically is unprecedented about this patch.

**Output:** Honest "I don't know" verdict with structured reasoning about why, instead of overclaiming.

**Status:** Conformal calibration + OOD score are part of the Q2 head. Phase G includes per-cell OOD AUC ≥ 0.90 as an auxiliary gate.

---

## 9. Teleological constellations (intent-linked chains)

**Claim:** Define these as the full chain:

```
operator goal
   → agent claim
      → edit event
         → AST delta
            → embedded chunk constellation
               → frozen instruments
                  → oracle outcome
                     → feedback
```

The reward signal is not just pass/fail; it is the **entire observed chain** of whether the edit moved durable reality toward the intended purpose.

**Mechanism:** Per-chunk, per-edit-event traversal of the chain. Each arrow is a deterministic transformation. The predictor learns the joint distribution over the chain, not just the marginal P(oracle_pass | chunk).

**Output:** Q5 head's `goal_alignment` score; the chain itself surfaced in `mejepa_explain_prediction`.

**Status:** Core to the architecture; fully realized after Phase G + operator-goal embedding from session UserPromptSubmit events.

---

## 10. Review-replacement primitives

**Claim:** Not *"replace human review"* immediately, but build the primitives that make review **statistically reducible**:
- Existence verification (Q1)
- Work prediction (Q2)
- Failure-reason prediction (Q3)
- Edge-risk prediction (Q4)
- Impact prediction (Q5)
- Confidence calibration tied to observed oracle outcomes

**Mechanism:** Each primitive is its own head; together they cover what a senior engineer mentally checks during code review.

**Output:** When all five primitives are calibrated above their per-cell thresholds, **review becomes statistically reducible**: the operator only reads the patches where one or more primitives are uncertain.

**Status:** This is the design goal. The 0.95 ship-gate is the operational definition.

---

## Beyond the user's list — what else the corpus enables

### 11. Reverse-direction prediction (why did the agent emit this patch?)

The same predictor that predicts oracle outcome from patch can be inverted to predict patch from goal. Useful for:
- Operator-side "what would ME-JEPA suggest as a fix?" tool
- Agent self-critique: agent emits patch, calls `mejepa_predict_what_if` for a counterfactual patch, compares
- Adversarial generation: produce patches that the predictor thinks will pass but reality says fail (active-learning data)

### 12. Per-agent fingerprinting

The corpus + live data builds a profile per AI agent: *"Claude Code's `subtle_flip` rate is 2%, Cursor's is 5%, Aider's is 1%."* Operators can pick agents per task. Agents can be measured for honesty (rate of fluent-but-refuted claims).

### 13. Drift detection across model versions

When the LLM upgrades (e.g., Claude 4.7 → Claude 5.0), the agent's distribution of patches shifts. ME-JEPA's drift detector catches it: cells whose recent live predictions diverge from historical baseline get flagged. This is the operator's signal that the upstream model changed behavior.

### 14. Code-specific embedding training

Right now the embedders are frozen pretrained. The corpus is large enough (~4.9 M vectors) that selective fine-tuning of specific embedders on the failure-mode contrastive pairs is feasible. Not in scope for Phase G, but plausible post-ship.

### 15. Per-language transfer learning

Once Python ships, the cross-lingual embedders (E14 BGE-M3 especially) carry signal to Rust, TypeScript, etc. The per-language ship gate need not re-run from scratch; it inherits Python's reward field where applicable.

### 16. Active counterfactual search

Given a `Verdict::Fail` patch, search the embedding neighborhood for the nearest `Verdict::Pass` patch. Emit it as a suggested fix. This is "auto-repair" without the LLM. Bounded by: the suggested patch must come from the corpus, not be hallucinated.

### 17. Repository-specific Gτ thresholds

The Gτ guard threshold is per-cell. Some repos (django, sympy) have high intra-cell variance because they touch a broad surface area; their Gτ is permissive. Others (a small library with tight invariants) have low intra-cell variance; their Gτ is strict. The system can learn the right per-repo permissiveness.

### 18. Operator-attention scoring

Track which patches the operator override versus which they leave alone. Train a meta-model that predicts operator attention. The agent can then ask: *"is the operator likely to override this prediction?"* and react accordingly (rebase, ask for clarification, raise confidence threshold).

### 19. Goal drift detection

The operator's stated goal at the start of a session (extracted from UserPromptSubmit events) defines a reward direction. As the session progresses, ME-JEPA tracks whether the agent's edits are moving toward or away from that direction. Goal drift becomes a Q5 sub-head.

### 20. Cost / latency / VRAM regression prediction

Each embedder's forward time is measured. Each instrument's computation cost is measured. Patches that increase predicted runtime cost across the pipeline get flagged in Q4's `cost_regression` sub-head. Not theoretical — the telemetry data already exists; just needs to be wired into a prediction head.

---

## The unifying principle

**Embeddings alone will not prove reality.** The labels and frozen instruments are what keep the system grounded. The multi-embedder layer gives ME-JEPA richer predictive coordinates; the durable-reality instruments supply the target truth that prevents collapse and prevents the model from learning vibes.

Each future capability above is one of two things:
1. A new head on top of the existing 12-embedder × 15-instrument substrate (cheap to add once substrate is built)
2. An expansion of the corpus or instrument set (slower; new FSV evidence required)

Both classes are tractable. The system is genuinely *built to be extended*: the architecture admits new heads without re-training the predictor backbone, and admits new instruments without breaking the collapse-immunity proof (as long as the new instruments are frozen, deterministic, and registered in the panel slot enum).

---

## What we'll know once the corpus lands

We're speculating right now because we can't see the reward field yet. Once the forward-cache run finishes on aiwonder (~20 hours from now), we will be able to see:

1. **Empirical embedder agreement / disagreement distributions per cell.** This will tell us which capabilities above are immediately reachable vs which need more data.
2. **Actual per-cell variance.** Some cells will be tight; some will be sprawling. Tight cells are easy; sprawling cells may need sub-division.
3. **Real Gτ threshold values per cell.** Calibration not theory.
4. **The 462-or-155 signal density per chunk.** We've been estimating; we'll measure.
5. **Which embedders dominate which cells.** Routing strategy.
6. **The OOD distribution.** What does "out of distribution" actually look like for code embeddings on this corpus?

Most of these capabilities are *consequences* of building the corpus. The corpus is what produces the answers, not the speculation.

---

## TL;DR

Pass/fail prediction is the floor. With the substrate the corpus produces, the system can also predict: failure shapes, claim-existence, blast radius, repository invariants, counterfactual repairs, operator attention, goal drift, and a dozen other capabilities. Most are cheap once the substrate exists. The substrate is being built right now.
