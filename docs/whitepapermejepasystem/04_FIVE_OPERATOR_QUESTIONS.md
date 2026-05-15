# 04 — The Five Operator Questions

Every `RealityPrediction` ME-JEPA emits answers five specific questions. They are not arbitrary — they correspond to the five distinct reality channels an AI-generated patch can fail in. Each question has its own head, its own training target, and its own conformal calibration. The five together are what the operator gets back when they query `mejepa_predict_latest`.

This document is the user-facing contract. If you understand nothing else about ME-JEPA, understand this.

---

## Q1 — Does what the AI claimed EXIST on disk?

### The failure mode it addresses

Agents lie about what they did. Not deliberately — they hallucinate. *"I added input validation to `parse_user_input()`"* sometimes means the agent added `# TODO: validate input` and considered the job done. *"I refactored the database layer"* sometimes means the agent renamed two functions and left the third one as-is. The lie is always plausible at the level of language and always falsifiable at the level of bytes.

### What the Q1 head returns

A **claim graph** reconciled against the actual SHA chain of the bytes on disk.

```rust
pub struct ClaimReconciliation {
    pub claims: Vec<AgentClaim>,
    pub verified: Vec<ClaimVerification>,  // claims that match the AST diff
    pub refuted: Vec<ClaimRefutation>,     // claims with no matching AST node
    pub adjacent: Vec<ClaimAdjacency>,     // claims that touched nearby code but missed the target
    pub overall_existence_score: f32,      // [0, 1]
}
```

### How it works

1. Parse the agent's natural-language claim with a structured extractor (deterministic rule set + small parser, NOT an LLM) into atomic assertions: `(verb, target, qualifier)` tuples. Example: *"I added input validation to parse_user_input"* → `(added, function:parse_user_input, validation)`.
2. Compute the AST diff between pre-edit and post-edit. Extract the typed delta: `(NodeKind::FunctionDef, name='parse_user_input', body_delta=[...])`.
3. For each atomic claim, find the AST delta that should-have-happened. Compare.
4. Emit `verified` / `refuted` / `adjacent` per claim. Score the whole reconciliation.

### Training target

The `E_AST` instrument and the `E_Witness` instrument together: AST diff + SHA chain. Both deterministic. When the predictor learns to map (claim, pre-edit_panel) → (post-edit_panel.E_AST), the Q1 head learns to predict whether the claim's structural delta will show up in the bytes.

### What the operator sees

In the rendered Markdown injection:

```
**Claim reconciliation:** "I added input validation" — REFUTED (no AST node matching `validation` 
verb in the post-edit diff). Adjacent edit found: function name change at parse_user_input:42. 
**Predicted reality impact:** the claim does not exist on disk.
```

### Why this is hard

The atomic-claim extractor must be deterministic — no LLM. A rule-based parser over the agent's claim is brittle but auditable. The training signal is whether the rule-based extractor's atom matches the AST delta's typed delta. Over the corpus, the extractor's recall converges on patterns that show up in real agent claims.

---

## Q2 — Does it WORK?

### The failure mode it addresses

Agent says *"the tests pass."* Tests pass locally; CI fails. Agent's local environment is wrong. Or the agent ran the tests but didn't notice the regression. Or the agent never ran the tests but said they did.

### What the Q2 head returns

A **calibrated verdict** + a **conformal interval** + per-test predicted outcomes.

```rust
pub enum Verdict {
    Pass,              // p(oracle_pass) ≥ 0.95 with conformal interval inside [0, 1]
    Fail,              // p(oracle_pass) ≤ 0.05
    OutOfDistribution, // OOD score ≥ 0.9; predictor not confident this case is in distribution
    Abstain,           // calibrated interval too wide; predictor doesn't know
    GuardRejected,     // TCT Gτ guard fired; cell-membership conflict
}

pub struct WorkPrediction {
    pub verdict: Verdict,
    pub predicted_oracle_pass: f32,
    pub calibrated_confidence: f32,
    pub conformal_interval_low: f32,
    pub conformal_interval_high: f32,
    pub ood_score: f32,
    pub predicted_test_flips: Vec<TestFlip>,  // tests likely to change pass/fail state
    pub closest_exemplars: Vec<ExemplarChunk>,
}
```

### How it works

1. Run the 12 embedders on the post-edit chunks.
2. Predictor head emits a raw "would oracle say Pass?" score.
3. Conformal calibrator converts the score → calibrated probability with interval.
4. Out-of-distribution detector checks whether the joint embedding falls within the training distribution; if not, emit `OutOfDistribution`.
5. TCT Gτ guard checks per-cell membership; if conflict, emit `GuardRejected`.
6. Otherwise compare calibrated probability to thresholds: `≥ 0.95 → Pass`, `≤ 0.05 → Fail`, else `Abstain`.

### Training target

The `E_Oracle` instrument: real Docker SWE-bench oracle outcomes. Each (task, mutation) cell has known FAIL_TO_PASS, PASS_TO_PASS, FAIL_TO_FAIL, PASS_TO_FAIL sets from running the per-task Docker oracle. That gives unambiguous Pass/Fail labels for 300 × 8 = 2,400 cells.

For the on-corpus training: the predictor learns `f(panel) → p(oracle_pass = True)`. For live predictions: same predictor, applied to (post-edit_panel) for the live edit.

### What the operator sees

```
**Verdict:** Fail | oracle-pass=0.07 | confidence=0.88 | ood=0.12 | outcomes=Fail
**Predicted test flips:** test_parse_user_input_rejects_unicode (was PASS, will FAIL)
**Closest exemplars:** psf__requests-2317#subtle_flip (oracle=Fail, similarity=0.81)
```

### Why this is the core question

Q2 is what gets you to 0.95 oracle correlation. The other four questions are auxiliary — they explain Q2 or scope its uncertainty. Q2 is what makes the system a verifier.

---

## Q3 — WHY would it fail?

### The failure mode it addresses

Q2 says `Fail` with confidence 0.88. The operator's next question is *"why?"* If ME-JEPA can't answer that, the operator has to read the code themselves — and the verifier didn't save them work.

### What the Q3 head returns

A **ranked list of predicted failure modes**, each with a confidence and a pointer to the violating embedder/instrument.

```rust
pub struct FailureExplanation {
    pub predicted_failure_modes: Vec<RankedFailureMode>,
    pub predicted_test_flips: Vec<TestFlip>,
    pub violating_embedders: Vec<EmbedderViolation>,
    pub violating_instruments: Vec<InstrumentViolation>,
    pub closest_exemplars: Vec<ExemplarChunk>,
}

pub struct RankedFailureMode {
    pub description: String,           // e.g. "TypeError on None argument"
    pub probability: f32,
    pub source_instrument: InstrumentSlot,
    pub nearest_failing_exemplar: ChunkId,
}
```

### How it works

For each chunk in the patch:
1. Embed across 12 embedders.
2. Find the K nearest chunks in `CF_MEJEPA_TCT_CELLS` whose oracle outcome was `Fail`.
3. Cluster the failure modes of those nearest neighbors: TypeError, AssertionError, ImportError, AttributeError, off-by-one, missing return, exception swallowing, ...
4. Rank by how strongly the patch's embedding lines up with each cluster's centroid.
5. Surface which embedder is most strongly pulling toward each failure mode (the "violating embedder").
6. Surface which instrument disagrees with the predictor's `Pass` hypothesis (the "violating instrument").

### Training target

The `E_Oracle` instrument's exception type one-hot (10 classes: TypeError, AssertionError, ImportError, AttributeError, ValueError, KeyError, IndexError, NameError, SyntaxError, Other). Combined with the per-failure-mode cluster centroids built from the historical corpus.

### What the operator sees

```
**Predicted failure modes:**
  1. TypeError on None argument (p=0.71, nearest exemplar: django__django-12286#subtle_flip)
  2. AssertionError on edge-case input (p=0.42, nearest exemplar: sympy__sympy-13895#off_by_one)
  3. KeyError on missing config key (p=0.18)
**Violating embedders:** E7 (code) strongly indicates control-flow mutation in the touched function
**Violating instruments:** E_TypeGraph reports None reaches a non-Optional parameter at parse_user_input:42
```

---

## Q4 — WHAT ELSE could go wrong?

### The failure mode it addresses

Q2 said `Pass`. The patch will probably ship. But the patch has hidden costs the test suite doesn't measure: edge cases the tests don't exercise, latent bugs introduced by the new code, technical debt added (new code paths that nobody will clean up), dead code that the patch leaves behind, redundancy with existing functions, performance regressions, security-relevant deltas, accuracy regressions in ML pipelines, cost regressions (cloud bills, infra usage).

This is the question that catches the *quiet* failures — the ones that pass review and ship and surface six months later.

### What the Q4 head returns

**Always-on, one sub-head per concern.** Even when Q2 verdict is `Pass`, every Q4 sub-head fires.

```rust
pub struct WhatElsePredictions {
    pub predicted_edge_cases_not_covered_by_tests: Vec<EdgeCase>,
    pub predicted_latent_bugs: Vec<LatentBug>,
    pub predicted_tech_debt_added: Vec<TechDebtItem>,
    pub predicted_dead_code: Vec<DeadCodeBlock>,
    pub predicted_redundancy: Vec<RedundancyPair>,
    pub predicted_perf_regression: Option<PerfRegression>,
    pub predicted_security_regression: Option<SecurityRegression>,
    pub predicted_accuracy_regression: Option<AccuracyRegression>,
    pub predicted_cost_regression: Option<CostRegression>,
}
```

Each `*Regression` is `Option<...>` because they may be absent. The list-typed fields are always emitted (possibly empty).

### How it works

Each sub-head has its own training target:

| Sub-head | Training target |
|---|---|
| Edge cases | Chunks similar to the patch where the historical corpus has a known edge-case (witnessed by oracle exception type or by an explicit edge-case annotation in test files) |
| Latent bugs | Chunks where the historical corpus's predicted-Pass labeled outcome was wrong (refuted) |
| Tech debt | Static metrics: cyclomatic complexity delta, file length delta, depth-of-nesting delta, count of TODO/FIXME comments added |
| Dead code | Static reachability: did the patch create a function/branch/import that no caller references? |
| Redundancy | Embedding similarity of the new code to existing code in the same project ≥ 0.92 |
| Perf | Per-symbol call-frequency profile × big-O signature shift detected by the static graph |
| Security | Patterns from the security-relevant instrument: introduces I/O on unsanitized input, removes auth check, adds privilege escalation, exposes secret |
| Accuracy | For ML code paths: probability-class output divergence from baseline |
| Cost | Calls to flagged expensive APIs / increased compute per inference / increased storage allocation |

### Training target

A mix of:
- `E_Oracle` (test outcomes) for *some* sub-heads
- Static-graph features for `dead_code` and `redundancy`
- Static metrics for `tech_debt`
- Specialized labels from corpus annotation for `perf`, `security`, `accuracy`, `cost` (these are sparser; some sub-heads are weaker until corpus expands)

### What the operator sees

```
**Predicted edge cases not covered by tests:** 
  - empty input list (p=0.65)
  - unicode normalisation (p=0.42)
  - subprocess failure (p=0.18)
**Predicted latent bugs:** unbounded retry loop in error handler
**Predicted tech debt added:** cyclomatic complexity +4 on parse_user_input (now 14, threshold 10)
**Predicted dead code:** function `legacy_parse_v1` no longer reachable
**Predicted perf regression:** None
**Predicted security regression:** None
**Predicted accuracy regression:** None
**Predicted cost regression:** None
```

### Why "always on"

Per the futurebuild vision: **the system predicts every issue it can predict, every time it predicts at all**. Q2 verdict `Pass` does not mean "stop predicting." The patch is still moving reality, and reality might be moving in a direction the operator cares about. Q4 surfaces that direction even when Q2 is green.

---

## Q5 — How does it IMPACT REALITY?

### The failure mode it addresses

Q1-Q4 are predictions about the immediate patch. Q5 is the **downstream cascade**: how does this edit ripple through the rest of the codebase, the test suite, the call graph, the deployment? An edit to one function changes the behavior of every caller. A change in a config file changes every consumer. An edit that looks isolated may have a blast radius of 50 other files.

### What the Q5 head returns

A **`RealityImpact`** structure: the predicted shift log of the codebase post-application, reconciled against the predicted side-effects.

```rust
pub struct RealityImpact {
    pub blast_radius: Vec<AffectedFile>,
    pub affected_call_paths: Vec<CallPath>,
    pub affected_tests: Vec<TestImpact>,
    pub affected_config_paths: Vec<ConfigImpact>,
    pub affected_serialization_contracts: Vec<SerializationImpact>,
    pub affected_perf_sensitive_loops: Vec<PerfHotPath>,
    pub affected_security_boundaries: Vec<SecurityBoundary>,
    pub goal_alignment: GoalAlignment,
}

pub struct GoalAlignment {
    pub operator_goal: Option<String>,       // from session state or user prompt
    pub alignment_score: f32,                // [-1, 1] — does this patch move toward the goal?
    pub conflicting_invariants: Vec<Invariant>,
}
```

### How it works

The Q5 head consumes:
1. The post-edit panel from the frozen instruments.
2. The shift log of past edits in the session (where was the agent going?).
3. The static reality graph (file/symbol/call/import edges).
4. The patch's joint embedding.

It outputs the predicted impact by:
1. Walking the static reality graph from the touched symbols → collecting all reachable callers/callees within K hops.
2. Embedding each of those reachable chunks.
3. Computing the embedding shift each would experience if the patch's typed delta propagated to them (counterfactual).
4. Ranking by predicted shift magnitude.
5. Cross-referencing against the operator's stated goal (extracted from session UserPromptSubmit events) to compute `alignment_score`.

### Training target

The shift log itself, replayed: when the corpus has a sequence (patch_t1, patch_t2, ..., patch_tn) over a single repo, the predictor learns to predict patch_tk from patch_t<k. The reality-impact head predicts which files patch_tk *would* have edited from patch_t<k's static graph reachability.

For the test-impact sub-head, the target is the actual `FAIL_TO_PASS` / `PASS_TO_FAIL` sets from SWE-bench oracle outcomes — which tests historically flipped under similar patches.

### What the operator sees

```
**Reality impact:**
  Blast radius: 12 files affected
  Affected tests: test_parse_user_input.py (3 tests, all currently PASS — predicted PASS), 
                  test_integration_user_flow.py (1 test, currently PASS — predicted FAIL)
  Affected call paths: handlers.user.create_user → models.user.validate
  Affected config paths: none
  Affected serialization contracts: UserInputModel (one field's validator changed)
  Affected perf-sensitive loops: none
  Affected security boundaries: input validation point now stricter
  Goal alignment: 0.82 (operator goal: "harden user-input handling against malformed payloads")
  Conflicting invariants: none
```

---

## How the five questions compose

The five heads are not independent classifiers. They share the joint embedding and consume the same panel; they differ in which heads of the predictor MLP they project through and which instruments they're calibrated against.

**Calibration is per-question.** Q2's `Verdict::Pass` confidence is calibrated against `E_Oracle`. Q4's `predicted_perf_regression` confidence is calibrated against a perf-specific instrument. The conformal residual distributions are per-head.

**Abstain composes too.** If Q2 abstains, the predictor doesn't have a confident answer. Q3/Q4/Q5 may still emit results, but with elevated uncertainty flags. The downstream UI treats an abstained Q2 as a softer "look at this" signal rather than a hard fail.

**Operator overrides flow back to all heads.** When the operator calls `mejepa_record_agent_feedback(prediction_id, observed_outcome)`, the verification record is keyed by `prediction_id`, which links to all five heads' outputs. The replay buffer pulls the entry at 6× weight; the next retrain updates all relevant heads.

## Five questions = five reality channels

| Channel | Reality fact | Instrument | Head |
|---|---|---|---|
| **Existence** | Did the bytes change in the way the claim says? | E_AST + E_Witness | Q1 |
| **Behavior** | Does the Docker oracle exit 0? | E_Oracle | Q2 |
| **Mechanism** | If it fails, what specific exception fires? | E_Oracle (exception type) | Q3 |
| **Hidden cost** | Is there structural / static-quality / perf / security / cost damage that tests can't see? | E_DataFlow + E_TypeGraph + static metrics + E_DDA | Q4 |
| **Ripple** | What downstream files / tests / configs are affected? | Static reality graph + shift log replay | Q5 |

Each channel is independently auditable. Each head is independently improvable. The operator sees them together because together they are what "reviewing AI-generated code" actually decomposes into.
