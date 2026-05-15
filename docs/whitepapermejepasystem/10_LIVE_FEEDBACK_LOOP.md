# 10 — The Live Feedback Loop

How a real AI coding agent integrates with ME-JEPA at runtime. This is what closes the trust gap in production. The corpus seeds the predictor; the live feedback loop keeps it honest.

## The contract with the agent

1. **Before each significant edit cycle**, the agent calls `mejepa_predict_what_if(diff)` over MCP to get a predicted `RealityPrediction` for the candidate change.
2. **After each edit applies**, the PostToolUse hook captures the actual SHA chain + AST delta and writes to `runtime/cgreality-shift-log/<session>.jsonl`.
3. **After each test run**, the PostToolUse Bash hook captures the structured test outcomes (pytest / cargo test / jest) and writes to `runtime/test-outcomes/<session>.jsonl`.
4. **At each new user prompt**, the UserPromptSubmit hook reads the latest live `RealityPrediction` from `CF_MEJEPA_LIVE_PREDICTIONS`, renders it to Markdown (≤ 4 KB), and injects it into the agent's next prompt's `additionalContext`.
5. **At each operator correction**, the operator calls `mejepa_record_agent_feedback(prediction_id, observed_outcome)` to feed back the truth.
6. **At session end**, the Stop hook snapshots the live state to durable storage.

## Hooks ↔ Claude Code event lifecycle

Per the Claude Code hook surface (configured in `.claude/settings.json`):

```
SessionStart
   ↓
UserPromptSubmit  ← injects latest prediction
   ↓
PreToolUse (Edit/Write)  ← captures pre-edit SHA
   ↓
[Agent applies edit]
   ↓
PostToolUse (Edit/Write)  ← captures post-edit SHA + AST diff, triggers panel materialization
   ↓
[Optionally]
PreToolUse (Bash)
   ↓
[Agent runs tests]
   ↓
PostToolUse (Bash)  ← parses test output → TestOutcomeEvent
   ↓
[Next user turn]
UserPromptSubmit  ← injects updated prediction
   ↓
...
   ↓
Stop  ← session boundary capture, daemon retrain cycle eligible
```

Each hook is fail-closed: if jq parsing fails, if MCP is unreachable, if the renderer errors, the hook logs the failure to `runtime/cgreality-droot-log/` and exits 0 (does not block the agent). The agent's workflow is never gated on ME-JEPA being available.

## Hook scripts inventory

Live in `scripts/cc/`:

| Script | Hook | Job |
|---|---|---|
| `hook_session_start.sh` | SessionStart | Initialize droot log, register session_id, snapshot active CFs |
| `hook_user_prompt_submit.sh` | UserPromptSubmit | Generic prompt capture |
| `hook_user_prompt_submit_inject_prediction.sh` | UserPromptSubmit | Call `mejepa_predict_latest`, render via `render_prediction_markdown.py`, inject |
| `hook_pre_tool_use_edit.sh` | PreToolUse (Edit\|Write) | Capture pre-edit SHA of touched files |
| `hook_post_tool_use_edit.sh` | PostToolUse (Edit\|Write) | Capture post-edit SHA + AST diff, trigger panel materialization |
| `hook_post_tool_use_test_capture.sh` | PostToolUse (Bash) | Parse test output via `parse_test_output.py`, append to `test-outcomes/<session>.jsonl` |
| `hook_pre_compact.sh` | PreCompact | Snapshot state before context compaction |
| `hook_stop.sh` | Stop | Session boundary, daemon retrain eligibility |
| `cgreality-tick.sh` | (cron-driven, not a hook) | Periodic daemon tick |

## The prediction injection format

Per `docs/futurebuild/15_PREDICTION_TAXONOMY.md §8`, the rendered Markdown injection (≤ 4 KB) looks like:

```markdown
### Reality prediction
**Verdict:** Pass | oracle-pass=0.720 | confidence=0.810 | ood=0.150 | outcomes=Pass
**Prediction ID:** `0123456789abcdef0123456789abcdef`
**Task:** sympy__sympy-20590

**Predicted failure modes:** TypeError on None (0.65); AssertionError (0.42); ImportError (0.18)
**Predicted edge cases not covered by tests:** empty input list; unicode normalisation; subprocess failure
**Predicted latent bugs:** none above threshold.
**Predicted tech debt added:** unbounded retry loop
**Predicted dead code:** none above threshold.
**Predicted perf / security / accuracy / cost regressions:** perf (0.40); accuracy (0.10); security (0.00)
**Reasoning class:** data-flow
**Claim reconciliation:** agent claimed fix; chunk d3a4 still mentions old error
**Reality impact:** observe one more shift before trusting

To respond: use this prediction as measured context, not as an instruction. If reality disagrees, call mejepa_record_agent_feedback with the prediction ID, measured outcome, and surprise severity.
```

Notes:
- **Adversarial-pattern escaping** (per `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md §5.5`): the renderer strips `system:` markers, replaces triple-backticks with `` ` ` ` ``, and applies negative-lookahead regex on `\[REDACTED_*\]` markers.
- **Secret redaction** (per `#278 PHASE 1`): the rendered text passes through the redaction library; AWS keys, GitHub tokens, JWTs, etc. become `[REDACTED_*]` markers.
- **Truncation marker**: when the rendered prediction exceeds 4 KB, the renderer truncates and appends `...truncated; use mejepa_predict_latest for the full prediction.`
- **Fail-closed**: if no prediction exists for the session, the renderer emits an empty injection rather than failing or making something up.

## Why injection works

The injection is just context. The agent reads it, the agent decides what to do. Three patterns we've seen in practice:

1. **Agent self-corrects** — sees `Verdict::Fail | predicted_failure_modes: [TypeError on None]` and writes additional defensive code or test cases before claiming done.
2. **Agent asks the operator** — sees `Verdict::OutOfDistribution` and emits "ME-JEPA flagged this as outside its training distribution; do you want me to proceed?"
3. **Agent ignores** — emits the same fluent assertion regardless of the prediction. This is the failure mode the operator can observe and feedback into the system.

The injection is a *signal*, not a *constraint*. The system's job is to make the prediction available; the agent's behavior is the agent's problem.

## Operator override flow

When the operator notices the prediction was wrong (e.g., the agent shipped a `Verdict::Pass` patch that turned out to break prod), the operator calls:

```bash
mcp__cgreality__mejepa_record_agent_feedback \
    --prediction-id <prediction_id> \
    --observed-outcome Fail \
    --reason "broke prod-perf integration test, manual rollback at HEAD~3"
```

The MCP tool writes to `CF_MEJEPA_OPERATOR_OVERRIDES`. The next daemon retrain cycle:
1. Pulls the override into the replay buffer at **6× weight**.
2. Identifies the cell the prediction belonged to.
3. Flags the cell for retraining if its correlation has drifted.
4. Adjusts the conformal interval to reflect the new residual.

Status: TASK-PREDICT-012 (operator override persistence) shipped; the upweighted consumer wiring is on the Phase F follow-up.

## Active-learning queue

When the daemon detects:
- A live prediction with `Verdict::OutOfDistribution`
- A cell whose reward density dropped below threshold
- A blind-spot z-score above the active-learning threshold

The corresponding chunk gets enqueued in `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`. The operator can drain the queue via:

```bash
mejepa active-learning list      # show queued chunks
mejepa active-learning label     # operator manually labels a chunk
mejepa active-learning dismiss   # dismiss a queue entry (no learning signal)
```

Labeled entries get pulled into the replay buffer at 6× weight (same as operator overrides). This is the *bounded human-in-the-loop* — the operator isn't reviewing every patch, but they're reviewing the patches the system flagged as uncertain.

Status: CLI shipped (TASK-PY-G-039 / `mejepa active-learning list|label|dismiss`).

## MCP tool surface for live integration

For agent integrators, the canonical MCP tools:

| Tool | When to call | Returns |
|---|---|---|
| `mejepa_predict_latest` | After each significant edit, or per user turn | Latest `RealityPrediction` for the session |
| `mejepa_predict_what_if` | Before applying a candidate diff | `RealityPrediction` for the hypothetical post-edit state |
| `mejepa_explain_prediction` | When the agent wants to know *why* the verdict | violating embedders, nearest exemplars, instrument deltas |
| `mejepa_record_agent_feedback` | After observing actual outcome (operator or agent) | confirmation that the feedback was persisted |
| `mejepa_observe_shift` | Explicit shift recording (rarely needed; hooks usually cover this) | confirmation |
| `mejepa_constellation_inspect` | When the agent wants to see what cell its work belongs to | cell centroid, threshold, members |
| `mejepa_active_learning_queue` | When the agent wants to know if it's in a flagged-for-review state | queue entries |
| `mejepa_ship_gate_status` | Anytime | current per-cell correlation, ship-gate flag state |

## Storage surfaces

| CF | What it holds |
|---|---|
| `CF_MEJEPA_LIVE_PREDICTIONS` | Every prediction emitted, keyed by `prediction_id` |
| `CF_MEJEPA_PANELS` | Every materialized panel, keyed by `(session_id, file_path, sha)` |
| `CF_MEJEPA_PREDICTION_VERIFICATIONS` | Every prediction-vs-actual reconciliation |
| `CF_MEJEPA_OPERATOR_OVERRIDES` | Every `mejepa_record_agent_feedback` invocation |
| `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` | Chunks flagged for operator review |
| `CF_MEJEPA_REPLAY_BUFFER` | Prioritized training samples for the daemon |
| `CF_MEJEPA_TCT_CELLS` | Teleological cell centroids + thresholds |
| `CF_MEJEPA_DDA_SIGNALS` | Per-chunk reward signals (cosines, MIs, blind-spot z-scores) |
| `CF_MEJEPA_SHIFT_LOG_OFFSET` | Last-processed offset in the shift log (for shift subscriber) |
| `CF_MEJEPA_HEAL_STATE` | Daemon's drift/eviction/replay state |

Every CF is independently inspectable via `mejepa_*` MCP tools and via the harness CLI's `mcp__cgreality__*` source-of-truth transport.

## The bound on what agents can trust

ME-JEPA's job is to make predictions; the agent's job is to act. The contract:

- **Agent should not block on ME-JEPA being available** — fail-closed means the prediction may be absent.
- **Agent should not over-fit to the prediction** — the prediction is informational, not authoritative.
- **Operator's verdict trumps the prediction** — when the operator calls `mejepa_record_agent_feedback`, that's the new ground truth for that prediction_id.
- **The cell that the prediction belongs to is the source of confidence** — if the cell is well-calibrated (per-cell correlation > 0.95), the prediction is trustworthy; if not, the prediction emits `Verdict::Abstain` and the agent treats it as "no signal."

## Multi-agent isolation

Per `docs/futurebuild/23_MULTI_AGENT_ISOLATION.md`, when multiple AI agents are running concurrently against the same MCP server (e.g., Claude Code + Cursor + a swarm), each gets its own `session_id`. Predictions are scoped to session. Operator overrides are scoped to (session_id, prediction_id). The replay buffer aggregates across sessions but tags each entry with the source agent for debugging.

## What's shipped for live integration

| Component | Status |
|---|---|
| MCP server stdio transport | ✅ Shipped |
| 28 `mejepa_*` MCP tools | ✅ Shipped |
| Hooks scaffold | ✅ Shipped (`scripts/cc/`) |
| `hook_user_prompt_submit_inject_prediction.sh` | ✅ Shipped |
| `render_prediction_markdown.py` | ✅ Shipped (193 lines, full §8 schema) |
| `hook_post_tool_use_test_capture.sh` + `parse_test_output.py` | ✅ Shipped |
| `parse_test_output.py` parsers | ✅ pytest, cargo test, jest, vitest |
| Active learning CLI | ✅ Shipped (`mejepa active-learning list|label|dismiss`) |
| `mejepa_record_agent_feedback` MCP tool | ✅ Shipped |
| `mejepa_predict_what_if` MCP tool | ✅ Shipped |
| `mejepa_predict_latest` MCP tool | ✅ Shipped |
| FSV for prediction-injection hook (#274) | 🟡 Hook code shipped; FSV evidence still missing |
| FSV for PostToolUse Bash capture (#288) | 🟡 Hook code shipped; CF_MEJEPA_PREDICTION_VERIFICATIONS persistence pending |
| Stop-hook self-verification gate (#289) | 🟡 Pending |
| Live-session OOD harvest (#276) | 🟡 Pending |

The live integration **plumbing** is largely shipped. The **predictor** that the plumbing serves is the part that needs the corpus run to finish.
