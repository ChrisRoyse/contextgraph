# 08 — Live Prediction Agent API

> **Purpose.** Specify the MCP tool surface AI agents call to ask "what does ME-JEPA think about my code, what's wrong with it, and what should I worry about?", plus the feedback path for agents to confirm or correct predictions. Owned by [`SPEC-PREDICT.md`](specs/functional/SPEC-PREDICT.md) (functional) and [`TECH-PREDICT.md`](specs/technical/TECH-PREDICT.md) (technical).
>
> **The slogan:** **the agent calls one tool, gets the full Q1–Q5 prediction surface, and can write back surprise events with one tool.** Two reads, one write — minimal API friction, maximal reality coupling.

**Status snapshot (2026-05-12):** Phase A/B code now ships `mejepa_verify`, `mejepa_predict_latest`, `mejepa_predict_what_if`, `mejepa_explain_prediction`, and `mejepa_record_agent_feedback`. Phase B FSV evidence lives at `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`; the live persisted readback used `CF_MEJEPA_LIVE_PREDICTIONS` prediction id `78e5bc08a3e79d8d47487dc9efd7bd7f`. Remaining work is production UX polish for rendering every Q1-Q5 field, full exemplar/counterfactual explanation enrichment, and trained per-head quality gates.

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

### 1.3 `mejepa_predict_what_if` (shipped Phase B, 2026-05-12)

Agent asks "if I were to make this hypothetical edit, what would ME-JEPA predict?" without actually applying the edit.

```jsonc
// Request
{
  "tool": "mcp__cgreality__mejepa_predict_what_if",
  "input": {
    "patch": {
      "kind": "unified_diff",
      "content": "diff --git ..."
    },
    "context": { ... },
    "compareToPredictionId": "78e5bc08a3e79d8d47487dc9efd7bd7f" // optional
  }
}
```

```jsonc
// Response
{
  "prediction": { ... },                       // RealityPrediction with verdict, all heads, etc.
  "compareToCurrent": { ... },                  // null unless compareToPredictionId is supplied
  "diskShaBefore": { ... },
  "diskShaAfter": { ... },
  "sourceOfTruth": {
    "dbPath": "/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-rocksdb",
    "repositoryRoot": "/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/repo",
    "persistedPrediction": false
  }
}
```

**Use case:** Claude Code is considering two refactor approaches; it asks ME-JEPA to predict reality for each before committing. The what-if path must prove disk SHAs are unchanged and must not write a live prediction row.

### 1.4 `mejepa_explain_prediction` (shipped Phase B, 2026-05-12)

Agent asks for the *reasoning* behind a specific prediction — which embedders, which chunks, which exemplars contributed most.

```jsonc
// Request
{
  "tool": "mcp__cgreality__mejepa_explain_prediction",
  "input": {
    "predictionId": "78e5bc08a3e79d8d47487dc9efd7bd7f"
  }
}
```

```jsonc
// Response
{
  "predictionId": "78e5bc08a3e79d8d47487dc9efd7bd7f",
  "verdict": "Fail",
  "saliencyByChunk": {
    "src/phase_b_sample.py#0": 0.8709677419354839,
    "requirements.txt#1": 0.12903225806451613
  },
  "topFailureModes": [ ... ],
  "edgeCases": [ ... ],
  "techDebt": [ ... ],
  "securityConcerns": [ ... ],
  "agentClaimGraph": { ... },
  "realityImpact": null,
  "sourceOfTruth": {
    "dbPath": "/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-rocksdb",
    "cf": "CF_MEJEPA_LIVE_PREDICTIONS"
  }
}
```

**Use case:** the agent wants to inspect the persisted prediction surface and saliency before acting on it. Closest-exemplar and counterfactual-minimum-edit enrichment remain future work once `CF_MEJEPA_FAILURE_EXEMPLARS` is production-populated.

### 1.5 `mejepa_constellation_inspect` (already shipped — extension)

Already-shipped tool. Extended to allow per-chunk inspection: "for this chunk, show me the 5 nearest centroids by per-embedder cosine."

---

## 2. Write-side tool (agent records feedback)

### 2.1 `mejepa_record_agent_feedback` (shipped Phase A, 2026-05-12)

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

## 3. The prediction-injection hook

The hooks layer captures shifts, runs cargo check + git diff on writes, and now has a UserPromptSubmit path for injecting the latest ME-JEPA prediction into the next agent prompt. This closes the read loop the operator required.

`TASK-PREDICT-001-prediction-injection-hook.md` adds a UserPromptSubmit hook handler that:

1. Reads `CF_MEJEPA_LIVE_PREDICTIONS` for the current session.
2. If a prediction exists and is < 60 seconds old (or has been emitted since the last prompt), renders the markdown block per `15_PREDICTION_TAXONOMY.md §8`.
3. Bounds the rendered output to ≤ 4 KB (per the existing tool-result cap from turn 79).
4. Injects the markdown block as a `<user>` message addendum that Claude Code reads on its next thinking step.

Implementation: a new shell script `scripts/cc/hook_user_prompt_submit_inject_prediction.sh` wired into `.claude/settings.json` under `hooks.UserPromptSubmit`. It calls a small `mcp__cgreality__mejepa_predict_latest` request and renders the response.

2026-05-17 hardening (#274): the hook now writes per-session injection source-of-truth rows under `runtime/agent-history/<session>-injections.jsonl`, injects the cold-start observing block for empty prediction lists, emits `MEJEPA_INJECT_MCP_UNREACHABLE` when transport fails, fails closed on malformed/tool-error responses, and escapes adversarial markdown patterns (`system:`, `>>>`, template delimiters, triple backticks, and secrets). The renderer surfaces verdict, confidence, top failure modes and edge cases, Q4 concerns by category, claim reconciliation, closest exemplars, and Abstain reasons under the 4 KB budget. FSV: `/mnt/d/contextgraph/fsv/task-py-g-055-prediction-injection-fsv/hook-run-1779060791-4146443/prediction_injection_hook_fsv.json` sha256 `cebbf132d5652859d73e584cd16c3a2084fcae2f8a7e3465d4c66328c27245c1`; MCP CF readback FSV: `/mnt/d/contextgraph/fsv/task-py-g-055-prediction-injection-fsv/mcp-readback-run-1779060540122-4131605/mcp_predict_latest_fsv.json` sha256 `c8785b9ffb4bafe9f21ae6d87821a48768eacb50b1cafa3db8fc0866eaf217f3`. Fresh-host daemon enablement proof remains separate from the renderer/hook contract.

---

## 4. Operator-side tools (the active-learning queue)

Already shipped:
- `mejepa_active_learning_queue` — operator surfaces top-N most uncertain predictions for label
- `mejepa_eval_run` — operator triggers a holdout-eval run (or it runs on a weekly cron)
- `mejepa_ship_gate_status` — current ≥0.95 ship-gate status per Python mutation category
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

This is now the intended production experience. The daemon mode, injection hook, feedback tool, what-if tool, and explain tool exist; the remaining risk is operational burn-in and quality gates, not missing first-class API surface.

---

## 7. API versioning

- The existing `mejepa_verify` and `mejepa_predict_latest` tools are extended rather than replaced where possible.
- Schema changes must be explicit and fail closed when a required field, enum, or binary RocksDB value cannot be decoded. Do not silently default old predictions into a new shape.
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
| `mejepa_record_agent_feedback` | unbounded; validation fail-closed | single-operator system; bad payloads should fail with durable diagnostics, not disappear under a throttle |
| `mejepa_constellation_inspect` | unbounded | RocksDB read |

The CUDA queue is shared between training (during periodic full-retrain) and inference (verify calls). The scheduler's promote_tick coordinates: during full-retrain, inference latency may rise; the predictor has a small priority-queue bypass for inference requests during retrain so end-user latency stays bounded.

---

## 9. What this API spec commits us to

1. `TASK-PREDICT-001-prediction-injection-hook.md` — complete; #274 renderer/hook hardening FSV passed 2026-05-17.
2. `TASK-PREDICT-002-record-agent-feedback-tool.md` — complete.
3. `TASK-PREDICT-003-predict-what-if-tool.md` — complete.
4. `TASK-PREDICT-004-explain-prediction-tool.md` — complete for persisted prediction explanations; exemplar/counterfactual enrichment remains.
5. `TASK-PREDICT-005-multi-head-stack.md` — code_complete; production DDA/neural feature consumption and trained holdout gates remain.
6. `TASK-PREDICT-006-rich-reality-prediction-schema.md` — complete.
7. `TASK-PREDICT-007-agent-claim-graph-extractor.md` — complete.
8. `TASK-PREDICT-008-reality-impact-reconciler.md` — complete.
9. `TASK-PREDICT-009-active-learning-agent-surprise-kind.md` — complete.

All listed in [`13_TASK_INDEX.md`](13_TASK_INDEX.md).
