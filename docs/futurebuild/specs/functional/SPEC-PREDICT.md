# SPEC-PREDICT — Functional Specification

> **Level 2.** Defines WHAT the live prediction agent API must do — read-side tools for "what does ME-JEPA think?" + write-side tool for agent feedback.
> Owns: REQ-PREDICT-01..20.
> Implementation reference: [`08_LIVE_PREDICTION_AGENT_API.md`](../../08_LIVE_PREDICTION_AGENT_API.md), [`15_PREDICTION_TAXONOMY.md`](../../15_PREDICTION_TAXONOMY.md).

---

## User stories

- **As a Claude Code agent**, I want to call one MCP tool to get a structured prediction of every issue with the code I've written so I can decide what to fix.
- **As a Claude Code agent**, I want predictions injected into my context automatically so I don't have to remember to ask.
- **As a Claude Code agent**, I want to write feedback when a prediction was wrong so the model improves.
- **As an operator**, I want to ask "what would happen if I made this hypothetical edit" without applying the edit.
- **As an operator**, I want to inspect the per-prediction reasoning (which embedders, which chunks, which exemplars) when a prediction surprises me.

## Requirements

### Read-side tools

#### REQ-PREDICT-01 — `mejepa_verify` rich response

`mejepa_verify(patch, context)` SHALL return a `RealityPrediction` with all Q1–Q5 surfaces populated per `15_PREDICTION_TAXONOMY.md §1`. Empty vecs are populated (not elided). Includes verdict, confidence_interval, ood_score, all 8 Q4 head outputs, claim_graph + reconciliation, reality_impact (when shifts have applied), provenance.

#### REQ-PREDICT-02 — `mejepa_predict_latest` returns daemon-cached prediction

`mejepa_predict_latest(session_id, max_age_seconds, include_provenance)` SHALL return the most recent `RealityPrediction` from `CF_MEJEPA_LIVE_PREDICTIONS` for the session, or recompute if older than `max_age_seconds`.

#### REQ-PREDICT-03 — `mejepa_predict_what_if` MCP tool (NEW)

`mejepa_predict_what_if(hypothetical_changes, context)` SHALL run a forward pass against the hypothetical patch WITHOUT applying it to disk. Returns the same `RealityPrediction` schema PLUS a `comparison_to_current` block.

#### REQ-PREDICT-04 — `mejepa_explain_prediction` MCP tool (NEW)

`mejepa_explain_prediction(prediction_id, head, concern_index)` SHALL return per-prediction reasoning: contributing signals (embedder + chunk + cosine), saliency map, closest exemplars, counterfactual minimum edit suggestion.

#### REQ-PREDICT-05 — Multi-head extension to predictor

The `MeJepaPredictor` SHALL implement 8 new prediction heads beyond the existing panel + oracle: failure-mode, edge-case, tech-debt, dead-code (via tech-debt sub-class), perf, security, accuracy, cost, reasoning. Each head is a thin Linear projection on top of the shared transformer trunk; per-head feature projections consume the head's required DDA-signal subset.

#### REQ-PREDICT-06 — Always-on emission

Every prediction MUST emit all 10 head outputs. The agent never receives "looks fine" — it receives the full structured concern surface, even on `Verdict::Pass`.

#### REQ-PREDICT-07 — Conformal interval per head

Each Q4 head SHALL include a calibrated confidence interval per `MeJepaCompiler::compile`'s conformal predictor; per-head calibration is tracked separately so one head's miscalibration doesn't contaminate the others.

#### REQ-PREDICT-08 — TCT Gτ guard violations exposed

`RealityPrediction.guard_violations` SHALL be populated whenever the TCT Gτ guard rejects the predicted panel for any chunk × embedder pair. Includes the cosine, threshold, and deficit per violation.

#### REQ-PREDICT-09 — Closest-exemplar retrieval

`RealityPrediction.closest_exemplars` SHALL retrieve the top-N nearest known exemplars from `CF_MEJEPA_FAILURE_EXEMPLARS` via patch-similarity-graph edges. Each exemplar names the task, mutation kind, similarity score, and oracle outcome.

### Write-side tool

#### REQ-PREDICT-10 — `mejepa_record_agent_feedback` MCP tool (NEW)

`mejepa_record_agent_feedback(prediction_id, feedback_kind, agent_explanation, actual_outcome, severity, extra_structured_data)` SHALL:
1. Persist a `SurpriseEvent` to `CF_MEJEPA_AGENT_FEEDBACK` keyed by `(prediction_id, agent_id, ts)`.
2. Resolve the original prediction; compute calibration_delta.
3. Multiply sampling weight for affected chunks by `1 + 2 * severity_score`.
4. Add an entry to the active-learning queue with `kind=AgentSurprise`.
5. If severity = Catastrophic, write to `CF_MEJEPA_HEAL_REPORTS` and operator-visible alerts stream.
6. Write a witness-chain entry binding the feedback to the original prediction's witness hash.
7. Return `feedback_id`, `witness_hash`, `sampling_weight_bump_applied`, `active_learning_queue_position`, `drift_signal_contribution`, `scheduler_action_queued`.

#### REQ-PREDICT-11 — Feedback is not locally rate-limited

`mejepa_record_agent_feedback` SHALL NOT impose a local per-agent rate limit. This is a single-operator system; valid feedback must be durably recorded and invalid feedback must return an explicit fail-closed error. Flood resistance is provided by schema validation, witness-bound writes, active-learning queue readback, and operator-label priority rather than throttling the operator's own tools.

### Hook-side delivery

#### REQ-PREDICT-12 — UserPromptSubmit prediction injection (NEW)

A new hook SHALL be wired into `.claude/settings.json` under `hooks.UserPromptSubmit` that:
1. Reads `CF_MEJEPA_LIVE_PREDICTIONS` for the current session.
2. If a prediction exists newer than the last prompt OR newer than 60 seconds, renders the markdown injection block per `15_PREDICTION_TAXONOMY.md §8`.
3. Bounds the rendered output to ≤ 4 KB.
4. Injects as a `<user>` message addendum.

#### REQ-PREDICT-13 — Prediction injection truncation

When the prediction's full markdown rendering exceeds 4 KB, the hook SHALL truncate Q4 concerns to top-5 per category by descending confidence; full predictions stay queryable via `mejepa_predict_latest`.

### Schema

#### REQ-PREDICT-14 — `RealityPrediction` schema stability

`RealityPrediction` schema is additive: extending an enum or adding a field is allowed; renaming or shrinking is forbidden. Old serialized predictions remain readable.

#### REQ-PREDICT-15 — Provenance bound

Every `RealityPrediction` SHALL include `provenance { predictor_version, constellation_version, calibration_version, active_pointer }` so a future client can reproduce the prediction byte-for-byte from the persisted weights + constellation + calibration.

#### REQ-PREDICT-16 — Witness-hash bound

Every `RealityPrediction` SHALL include a `witness_hash` linking it to the parent panel and the contributing shift entries.

### Q1 / Q5 paths

#### REQ-PREDICT-17 — Agent claim graph extractor

A deterministic rule-based extractor SHALL parse the agent's response (when provided in `context.agent_response`) into an `AgentClaimGraph` with `claims: Vec<AgentClaim>` per `15 §5.1`. No LLM calls.

#### REQ-PREDICT-18 — Claim reconciliation

For each `AgentClaim`, the system SHALL emit a `ClaimReconciliation` with status in `{Confirmed, Missing, Modified, UnexpectedSideEffect, SuperficialMatch, AmbiguousRef, Unverifiable}` per `15 §5.2`, with evidence rows (file, before_sha, after_sha, line_range).

#### REQ-PREDICT-19 — Reality-impact reconciler

When shifts have applied since the prediction was made, `RealityPrediction.reality_impact` SHALL be populated per `15 §6` with observed_shifts, predicted vs observed files changed, predicted vs observed test outcomes, prediction_correctness classification.

### Acceptance criteria

#### REQ-PREDICT-20 — Round-trip schema validation

A `RealityPrediction` produced by `mejepa_verify` SHALL serialize to JSON and deserialize back to an identical struct (round-trip lossless).

## Edge cases

- **Patch is empty diff**: verdict = `Pass` (trivially), all Q4 vecs empty, claim graph empty.
- **Patch has merge conflict markers**: `Verdict::OutOfDistribution` with `predicted_failure_modes: [SyntaxError]`.
- **OOD score > 0.9 (way out of distribution)**: verdict = `OutOfDistribution`; `predicted_*` heads still populated but with widened intervals.
- **No prediction exists for `prediction_id` in `mejepa_record_agent_feedback`**: return `MEJEPA_FEEDBACK_PREDICTION_NOT_FOUND`.
- **Constellation stale (> 90 days)**: verdict optionally `Abstain` with reason; operator override via `mejepa_constellation_inspect --force-stale`.

## Non-functional requirements

- `mejepa_verify` p50 latency < 100ms (cached instruments); p99 < 500ms.
- `mejepa_predict_latest` p50 < 50ms (RocksDB read).
- `mejepa_record_agent_feedback` p50 < 20ms.
- Prediction injection rendering p50 < 50ms.
