# 08 — Live Prediction Agent API

> **Purpose.** Specify the MCP tool surface AI agents call to ask "what does ME-JEPA think about my code, what's wrong with it, and what should I worry about?", plus the feedback path for agents to confirm or correct predictions. Owned by [`SPEC-PREDICT.md`](specs/functional/SPEC-PREDICT.md) (functional) and [`TECH-PREDICT.md`](specs/technical/TECH-PREDICT.md) (technical).
>
> **The slogan:** **the agent calls one tool, gets the full Q1–Q5 prediction surface, and can write back surprise events with one tool.** Two reads, one write — minimal API friction, maximal reality coupling.

---

## 1. Read-side tools (agent asks for predictions)

### 1.1 `mejepa_verify` (already shipped)

Agent passes a candidate patch + context, gets a complete `RealityPrediction` synchronously.

```jsonc
// Request
{
  "tool": "mcp__cgreality__mejepa_verify",
  "input": {
    "patch": {
      "kind": "unified_diff",                 // or "file_changes" with explicit file/old/new triples
      "content": "diff --git ..."
    },
    "context": {
      "session_id": "agent-session-01HXY...",
      "task_id": "astropy__astropy-12907",    // optional; if known, used for corpus-similarity
      "repo_root": "/path/to/repo",
      "agent_response": "I've fixed the bug by ..."  // optional; enables Q1 claim graph
    },
    "include_provenance": true,               // include witness_hash + chunk-level signals
    "include_explanations": true              // include human-readable explanation strings
  }
}
```

```jsonc
// Response (abbreviated; full schema in 15_PREDICTION_TAXONOMY.md §1)
{
  "prediction_id": "pred_01HXYZ...",
  "witness_hash": "shake256:73:...",
  "verdict": "Fail",
  "confidence_interval": { "lower": 0.04, "upper": 0.12, "method": "SplitConformal", "coverage_target": 0.90, "empirical_coverage": 0.91 },
  "ood_score": 0.18,
  "predicted_failure_modes": [ ... ],         // ranked
  "predicted_failed_tests": [ ... ],
  "guard_violations": [ ... ],
  "closest_exemplars": [ ... ],
  "predicted_edge_cases": [ ... ],
  "predicted_latent_bugs": [ ... ],
  "predicted_tech_debt_added": [ ... ],
  "predicted_dead_code": [ ... ],
  "predicted_redundant_code": [ ... ],
  "predicted_perf_regressions": [ ... ],
  "predicted_security_concerns": [ ... ],
  "predicted_accuracy_degradations": [ ... ],
  "predicted_cost_regressions": [ ... ],
  "predicted_reasoning_class": "MostlyCorrect",
  "claim_graph": { ... },
  "claim_reconciliation": [ ... ],
  "reality_impact": null,                     // populated only after shifts have applied
  "provenance": {
    "predictor_version": "v0.4.2-bf16-cuda13.2",
    "constellation_version": "v0.18.7",
    "calibration_version": "2026-05-09T12:00:00Z",
    "active_pointer": "weights:sha256:..."
  }
}
```

**Errors (fail-closed):**
- `MEJEPA_VERIFY_INVALID_PATCH` — patch could not be parsed as unified diff or file_changes
- `MEJEPA_VERIFY_REPO_UNAVAILABLE` — repo_root does not exist or is not a git repo
- `MEJEPA_VERIFY_OUT_OF_DISTRIBUTION_HARD` — OOD score exceeds even-the-prediction-is-not-trustworthy threshold; verdict still returned but flagged
- `MEJEPA_VERIFY_GUARD_REJECTED_HARD` — every chunk fails Gτ (predictor has degenerated entirely; alert operator)

### 1.2 `mejepa_predict_latest` (already shipped)

Agent asks "what's the latest prediction for my session, based on shifts the daemon has already observed?" Returns the most recent `RealityPrediction` from `CF_MEJEPA_LIVE_PREDICTIONS` keyed by `session_id`.

```jsonc
// Request
{
  "tool": "mcp__cgreality__mejepa_predict_latest",
  "input": {
    "session_id": "agent-session-01HXY...",
    "max_age_seconds": 60,                    // optional; if last prediction is older than this, recompute
    "include_provenance": false
  }
}
```

```jsonc
// Response
{
  "prediction": { ... },                       // full RealityPrediction or null if no shifts observed yet
  "shift_count_since_last_prediction": 3,
  "subscriber_lag_ms": 240
}
```

This is the path for **passive** predictions — the agent doesn't have to construct a patch; the daemon has already observed the shifts.

### 1.3 `mejepa_predict_what_if` (NEW)

Agent asks "if I were to make this hypothetical edit, what would ME-JEPA predict?" without actually applying the edit.

```jsonc
// Request
{
  "tool": "mcp__cgreality__mejepa_predict_what_if",
  "input": {
    "hypothetical_changes": [
      { "file": "src/foo.py", "old_string": "...", "new_string": "..." }
    ],
    "context": { ... }
  }
}
```

```jsonc
// Response
{
  "prediction": { ... },                       // RealityPrediction with verdict, all heads, etc.
  "comparison_to_current": {
    "delta_oracle_pass_confidence": -0.3,
    "delta_predicted_failure_count": +2,
    "delta_perf_regression_axis_count": 0,
    "verdict_change": "Pass → Fail"
  }
}
```

**Use case:** Claude Code is considering two refactor approaches; it asks ME-JEPA to predict reality for each before committing. The hypothetical doesn't touch disk; it's purely a forward pass through the predictor.

### 1.4 `mejepa_explain_prediction` (NEW)

Agent asks for the *reasoning* behind a specific prediction — which embedders, which chunks, which exemplars contributed most.

```jsonc
// Request
{
  "tool": "mcp__cgreality__mejepa_explain_prediction",
  "input": {
    "prediction_id": "pred_01HXYZ...",
    "head": "failure_mode",                    // or "edge_case" / "tech_debt" / etc.
    "concern_index": 0                          // index into the head's vec
  }
}
```

```jsonc
// Response
{
  "concern": { ... },                          // the original concern
  "contributing_signals": [
    { "embedder": "E_TypeGraph", "chunk": "chunk_123", "cosine_to_centroid": 0.61, "tau_m": 0.72, "weight": 0.34 },
    { "embedder": "E_DataFlow", "chunk": "chunk_123", "cosine_to_centroid": 0.58, "tau_m": 0.70, "weight": 0.27 },
    ...
  ],
  "saliency_map": {                            // attention weights from the failure-mode head
    "chunk_123": 0.78,
    "chunk_124": 0.15,
    "chunk_125": 0.07
  },
  "closest_exemplars": [
    { "task_id": "psf__requests-2317", "similarity_score": 0.91, "diff_summary": "...", "oracle_outcome": "Fail" }
  ],
  "counterfactual_minimum_edit": {              // what's the smallest change that would flip this concern?
    "chunk": "chunk_123",
    "suggested_edit_class": "swap_left_right_indexing",
    "predicted_post_edit_concern_severity": "Low"
  }
}
```

**Use case:** the agent wants to understand *why* ME-JEPA predicted what it predicted, so it can either act on the prediction or argue with it.

### 1.5 `mejepa_constellation_inspect` (already shipped — extension)

Already-shipped tool. Extended to allow per-chunk inspection: "for this chunk, show me the 5 nearest centroids by per-embedder cosine."

---

## 2. Write-side tool (agent records feedback)

### 2.1 `mejepa_record_agent_feedback` (NEW — the operator's missing piece)

The operator-mandated closed-loop: when the agent gets a prediction and acts on it, it can confirm, contest, or augment via this tool.

```jsonc
// Request
{
  "tool": "mcp__cgreality__mejepa_record_agent_feedback",
  "input": {
    "prediction_id": "pred_01HXYZ...",
    "feedback_kind": "Surprise",              // Confirmed | Surprise | Omission | Calibration
    "agent_explanation": "ME-JEPA predicted that test_separability_compound would fail, but I traced it carefully and the actual failure was in a different test (test_compound_left_only). The predictor confused the two test names which both touch the CompoundModel walker.",
    "actual_outcome": {
      "kind": "test_outcome",
      "test_id": "tests/test_separability.py::test_compound_left_only",
      "outcome": "Fail",
      "error_log": "AssertionError: ..."
    },
    "severity": "Medium",
    "extra_structured_data": {
      "predicted_test_id": "tests/test_separability.py::test_separability_compound",
      "actual_test_id": "tests/test_separability.py::test_compound_left_only",
      "agent_diagnosis_class": "TestNameConfusion"
    }
  }
}
```

```jsonc
// Response
{
  "feedback_id": "feedback_01HXYZ...",
  "witness_hash": "shake256:73:...",          // bound to original prediction's witness_hash
  "sampling_weight_bump_applied": 1.6,         // 1 + 2*0.3 (Medium = 0.3)
  "active_learning_queue_position": 7,
  "drift_signal_contribution": "soft",         // categorical effect on rolling drift window
  "scheduler_action_queued": "fast_track_observe"
}
```

**Behavior:**
1. Persist a `SurpriseEvent` to `CF_MEJEPA_AGENT_FEEDBACK` keyed by `(prediction_id, agent_id, ts)`.
2. Resolve the original prediction; compute `expected_correctness_at_confidence(prediction.confidence) - observed_outcome_correctness` → `calibration_delta`.
3. Multiply sampling weight for the contributing chunks by `1 + 2 * severity_score`.
4. Add an entry to the active-learning queue with `kind=AgentSurprise` and high priority.
5. If `severity == Catastrophic`, also write a high-severity entry to `CF_MEJEPA_HEAL_REPORTS` and the operator-visible alerts stream.
6. Write a witness-chain entry binding the feedback to the original prediction's witness hash.
7. Return the feedback record + queue position.

**Errors:**
- `MEJEPA_FEEDBACK_PREDICTION_NOT_FOUND` — `prediction_id` does not exist or is older than the retention window
- `MEJEPA_FEEDBACK_INVALID_OUTCOME_SHAPE` — `actual_outcome` has the wrong schema for its `kind`
- `MEJEPA_FEEDBACK_INVALID_PAYLOAD` — payload is malformed, missing required fields, or references an unsupported feedback kind/severity

There is no local per-agent feedback rate limit. This is a single-operator system; invalid or abusive payloads fail closed through validation, durable error logging, and source-of-truth readback rather than silent throttling.

### 2.2 Feedback kinds in detail

| Kind | Semantics | Sampling weight bump | Active-learning priority |
|---|---|---|---|
| `Confirmed` | "the prediction was right; I acted on it and reality agreed" | 1.1× (small reinforcement) | low |
| `Surprise` | "the prediction was wrong in a meaningful way" | `1 + 2 * severity_score` | high |
| `Omission` | "ME-JEPA missed an issue I found independently — here's what" | `1.5 + severity_score` | high |
| `Calibration` | "the prediction was right but the confidence was miscalibrated (over- or under-confident)" | 1.2× | medium |

The agent can add free text in `agent_explanation` plus structured fields in `extra_structured_data`. The structured fields are head-specific schemas (e.g., `test_outcome`, `static_analysis_finding`, `perf_measurement`).

---

## 3. The prediction-injection hook (the missing piece on the read path)

Today the hooks layer captures shifts and runs cargo check + git diff on writes. **It does not inject ME-JEPA predictions into the next agent prompt.** This is the closing of the read loop the operator implicitly requires.

`TASK-PREDICT-001-prediction-injection-hook.md` adds a UserPromptSubmit hook handler that:

1. Reads `CF_MEJEPA_LIVE_PREDICTIONS` for the current session.
2. If a prediction exists and is < 60 seconds old (or has been emitted since the last prompt), renders the markdown block per `15_PREDICTION_TAXONOMY.md §8`.
3. Bounds the rendered output to ≤ 4 KB (per the existing tool-result cap from turn 79).
4. Injects the markdown block as a `<user>` message addendum that Claude Code reads on its next thinking step.

Implementation: a new shell script `scripts/cc/hook_user_prompt_submit_inject_prediction.sh` wired into `.claude/settings.json` under `hooks.UserPromptSubmit`. It calls a small `mcp__cgreality__mejepa_predict_latest` request and renders the response.

**This is the most operationally-impactful single task in the futurebuild plan**: until predictions are injected, the agent has to remember to call `mejepa_verify` manually, which it often won't. With injection, the agent gets the prediction whether it asked or not.

---

## 4. Operator-side tools (the active-learning queue)

Already shipped:
- `mejepa_active_learning_queue` — operator surfaces top-N most uncertain predictions for label
- `mejepa_eval_run` — operator triggers a holdout-eval run (or it runs on a weekly cron)
- `mejepa_ship_gate_status` — current ≥0.95 ship-gate status per (category × language)
- `mejepa_eval_build_graph` — patch-similarity graph builder

Extension:
- `TASK-OBS-002-active-learning-cli.md` adds a small CLI under `cargo run --release -p context-graph-cli mejepa active-learning` that wraps `mejepa_active_learning_queue` for easier operator use.

---

## 5. Daemon health tools (for the loop's per-turn check)

The single-loop refactor (per [`06_REALITY_LOOP_REFACTOR.md §9`](06_REALITY_LOOP_REFACTOR.md)) requires the loop to surface daemon health every turn. The tools:

- `mejepa_subscriber_status` (already shipped) — shift subscriber lag, latency p50/p99, last-panic info
- `mejepa_heal_status` (already shipped) — current drift level, pending interventions, last A/B/C result
- `mejepa_quota_status` (already shipped) — disk usage vs 115 GB hard cap
- `mejepa_vram_budget_report` (already shipped) — VRAM usage vs budget

Wrapping these in a single `mejepa_daemon_status` aggregator tool would be convenient (`TASK-OBS-003-daemon-status-aggregator.md`), but not strictly necessary.

---

## 6. End-to-end agent interaction example

```
[Agent UserPromptSubmit] -- user asks "fix bug X"

  > Hook fires: reads CF_MEJEPA_LIVE_PREDICTIONS for session — no prediction yet
    (no shifts in this session); injects nothing.

[Agent reads, plans, calls Edit on src/foo.py]
  > PreToolUse hook records pre_sha=abc...
  > Edit returns
  > PostToolUse hook records after_sha=def..., shift-log line appended

  ~250ms later, daemon's shift subscriber consumes the line:
  - chunks src/foo.py
  - runs neural forwards on the affected chunks
  - computes DDA signals
  - builds Panel
  - calls verify(panel, action, context) — RealityPrediction persisted to CF_MEJEPA_LIVE_PREDICTIONS

[Agent calls Edit again on src/bar.py]
  > Same path; another prediction stacked.

[Agent UserPromptSubmit] -- user asks "did you fix it?"

  > Hook fires: reads CF_MEJEPA_LIVE_PREDICTIONS — finds 2 recent predictions
    for this session. Renders the latest as markdown:
    ---
    Reality prediction (chunks affected since last prompt: 4):
    Verdict: Fail (oracle pass confidence 0.07 [0.04, 0.12])
    Predicted failure modes:
      1. src/bar.py:34 — WrongAlgorithm — predicted FAIL of test_X (0.93)
    Predicted edge cases not covered:
      - EmptyInput at src/foo.py:12
    Predicted tech debt: 1 new TODO without owner
    Reasoning class: MostlyCorrect
    To respond to a wrong prediction: call mejepa_record_agent_feedback with prediction_id=pred_xyz
    ---

[Agent reads the prediction, decides ME-JEPA is wrong about test_X — that test
 doesn't exist in this repo, ME-JEPA confused it with another]

[Agent calls mejepa_record_agent_feedback]
  prediction_id = pred_xyz
  feedback_kind = Surprise
  severity = Medium
  agent_explanation = "test_X doesn't exist; predictor confused it with test_Y"
  actual_outcome = { kind: NoTest, ... }

  > Surprise event persisted to CF_MEJEPA_AGENT_FEEDBACK
  > Active-learning queue entry created (high priority)
  > Sampling weight for src/bar.py chunk_xyz bumped 1.6×
  > Witness chain: feedback bound to prediction; both immutable

[Agent runs pytest]
  > Bash hook captures the exit + the parsed test results
  > TestOutcomeEvent persisted to CF_MEJEPA_ORACLE_VERDICTS
  > Self-optim scheduler picks it up on next observe_tick (≤250ms)
  > observe(panel, action, panel_after_test, oracle=Pass) called
  > Training cert written; ΔP/ΔK/ΔΩ/ΔΞ logged

[Agent UserPromptSubmit] -- user asks for a summary
  > Hook injects updated prediction. Verdict now: Pass (oracle confidence 0.94)
  > Plus the lingering Q4 concerns: tech debt + dead code + perf neutral

[Agent reports "Done; tests pass; ME-JEPA predicts Pass at 0.94 confidence;
 noted tech debt items I'll address in follow-up"]
```

This is the **target experience**. Today, steps marked "daemon" / "Hook injects updated prediction" / `mejepa_record_agent_feedback` don't fire because the daemon mode and the injection hook and the feedback tool don't exist yet. The futurebuild plan delivers all three.

---

## 7. Backwards compatibility & versioning

- The existing `mejepa_verify` and `mejepa_predict_latest` tools are EXTENDED (new optional fields) rather than replaced. Old callers see the same response shape with new fields they can ignore.
- `Verdict` enum extension is additive. New variants don't change existing behavior; old serialized values still deserialize.
- `RealityPrediction` is versioned via the `provenance.predictor_version` field; clients that need to cross-reference predictions across model versions can use this as the version key.
- The `provenance` block lets a future client reproduce the prediction byte-for-byte from the persisted weights + constellation + calibration; this is the "audit substrate" closure.

---

## 8. Rate limiting & resource discipline

| Tool | Limit | Reason |
|---|---|---|
| `mejepa_verify` | unbounded for read; CUDA queue serializes | predictor forward is CUDA-bound; serialization is at the GPU |
| `mejepa_predict_latest` | unbounded | pure RocksDB read |
| `mejepa_predict_what_if` | unbounded | same forward path as verify |
| `mejepa_explain_prediction` | unbounded | RocksDB read + lightweight rerank |
| `mejepa_record_agent_feedback` | 100 / hour / agent_id | anti-DoS; agents shouldn't be feedback-spamming |
| `mejepa_constellation_inspect` | unbounded | RocksDB read |

The CUDA queue is shared between training (during periodic full-retrain) and inference (verify calls). The scheduler's promote_tick coordinates: during full-retrain, inference latency may rise; the predictor has a small priority-queue bypass for inference requests during retrain so end-user latency stays bounded.

---

## 9. What this API spec commits us to

1. `TASK-PREDICT-001-prediction-injection-hook.md` — the UserPromptSubmit injector
2. `TASK-PREDICT-002-record-agent-feedback-tool.md` — the new MCP write tool + CF + sampling-weight wiring
3. `TASK-PREDICT-003-predict-what-if-tool.md` — the hypothetical prediction tool
4. `TASK-PREDICT-004-explain-prediction-tool.md` — the per-prediction reasoning tool
5. `TASK-PREDICT-005-multi-head-stack.md` — extend the predictor with the multi-head taxonomy from `03_ARCHITECTURE_BLUEPRINT.md §3`
6. `TASK-PREDICT-006-rich-reality-prediction-schema.md` — extend `RealityPrediction` to the full Q1–Q5 schema in `15_PREDICTION_TAXONOMY.md §1`
7. `TASK-PREDICT-007-agent-claim-graph-extractor.md` — deterministic rule-based extractor for Q1
8. `TASK-PREDICT-008-reality-impact-reconciler.md` — Q5 reconciliation between predicted & observed shifts
9. `TASK-PREDICT-009-active-learning-agent-surprise-kind.md` — extend `ActiveLearningKind` enum

All listed in [`13_TASK_INDEX.md`](13_TASK_INDEX.md).
