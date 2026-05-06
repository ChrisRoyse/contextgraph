# Phase 13 — Hardening

**Goal:** clean every small bug surfaced by the cutover smoke + Phase 12 production attempt before iterating on capability.
**Effort:** <1 day.
**Risk:** low — bugs are localized.

## Tasks

### T13.1 — Fix SessionStart hook attempt-summary parser

Symptom (observed this session): `SessionStart` injected the line "Latest attempt summary read failed. Source of truth: file:/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/smoke-1778022681/cgreality-smoke-phase09-layer5/smoke-attempt-1/attempt-summary.json. Remediation: inspect malformed attempt-summary.json before relying on session-start context."

Reality readback: that file is well-formed JSON (verified with `cat` and via `mcp__cgreality__reality_attempt_summary` returning the same data structurally). The hook script's parser appears to expect a key/shape it doesn't find in synthetic-smoke summaries.

Fix:
- Inspect `scripts/cc/hook_session_start.sh` and any helper called by it.
- For synthetic-smoke summaries (record_kind `ccreality_smoke_attempt_summary`), the hook should accept the smaller shape and emit a normal "Active reality-loop runtime root: …" message without the "read failed" warning.
- Add a hook unit test under `scripts/cc/tests/hooks/test_session_start.sh` that exercises a synthetic smoke and asserts no "read failed" string in the output.

End-state: `bash scripts/cc/tests/hooks/test_session_start.sh` PASSES against a synthetic smoke runtime.

### T13.2 — `mcp__cgreality__reality_harness_transitions` accepts integer attempt

Symptom: when called with `{run_id, attempt: 1}` (integer), the tool returned `CCREALITY_ARG_MISSING_OR_NOT_U64` indicating it received the value as a string.

Fix:
- Inspect the JSON-RPC arg coercion in `crates/context-graph-mcp/src/handlers/tools/reality_loop/` analyze module.
- Coerce `attempt` from JSON number → u64 directly (don't pass through string).
- Add unit test in `cargo test -p context-graph-mcp --lib reality_loop` that calls the tool with both integer and string forms and expects success.

End-state: `mcp__cgreality__reality_harness_transitions {run_id: <real>, attempt: 1}` returns the transitions list without error.

### T13.3 — `/loop` body invokes `reality_shift_compare_to_my_view` at start

Symptom: the `/loop` iteration procedure documented in `docs/ccreality/11_LOOP_PROMPT.md` describes drift detection ("If you see external drift…") but does not name the call. Coverage is therefore conditional on the model remembering to call it.

Fix:
- Add an explicit step 2.5 in `.claude/loop.md`: "Call `mcp__cgreality__reality_shift_compare_to_my_view {session_id, files: [governed_subset]}` against the recently-touched files in this session. If any `external_drift_detected` is reported, switch the recommendation status to `blocked` with reason naming the drifted files."
- Add a smoke layer (`scripts/cc/tests/smoke.sh` Layer 5) that mutates a governed file from outside the session, then asserts the next /loop step produces a `blocked` recommendation citing drift.

End-state: smoke run includes the new drift layer and passes.

### T13.4 — Recommendation `cost_estimate_usd` budget warning

Symptom: schema accepts `cost_estimate_usd` but no per-day rollup exists.

Fix:
- Add `mcp__cgreality__optimizer_record_recommendation` server logic that, after writing the recommendation, sums `cost_estimate_usd` across all recommendations in the active runtime root for today (UTC). If total > $20 USD (configurable via env `CGREALITY_DAILY_COST_CAP_USD`), append a `cost_cap_warning` field to the response (does not block writing). When total > 1.5× cap, return `CCREALITY_COST_CAP_REACHED` (blocking) and require operator override.

End-state: a contrived `claude --print` run that records 25 recommendations with `cost_estimate_usd: 1.0` each blocks recommendation #21+ until the env var is set higher.

### T13.5 — Cleanup stale shift-log files

Symptom: `<runtime_root>/cgreality-shift-log/` accumulates per-session JSONL files indefinitely. After 30 days of runs this is a minor disk-space concern.

Fix:
- Add a maintenance MCP tool `mcp__cgreality__shift_log_compact` that gzips files older than `--age 30d` (default).
- Operator-runnable; not auto-fired.

End-state: tool exists, manual invocation rotates a stale file and leaves recent ones intact.

### T13.6 — Quiet "no_change" recommendation churn

Symptom: against the active smoke, two `optimizer_record_recommendation` calls were made for the same attempt (turn 01 was overwritten). Recommendations should be append-only per (attempt, turn_number).

Fix:
- Server logic: if `(attempt, turn_number)` already has a recommendation file, increment turn_number until a free slot is found, OR return `CCREALITY_RECOMMENDATION_TURN_COLLISION` if `turn_number` was explicitly set.
- Update `.claude/loop.md` so each /loop fires uses `turn_number = max_existing + 1`.

End-state: two consecutive /loop calls for the same attempt produce `recommendation-turn-01.json` and `recommendation-turn-02.json` — never overwriting.

## AI-verifiable end-state for Phase 13

Run:
```bash
bash scripts/cc/tests/smoke.sh   # must include the drift-detection layer
cargo test -p context-graph-mcp --lib reality_loop --quiet
bash scripts/cc/tests/hooks/test_session_start.sh
```

All three must exit 0. The smoke report JSON must show `drift_detection: PASS`.

## Out of scope for Phase 13

- New capability (saved for Phase 14).
- Cross-model wiring (Phase 16).
- Reward changes (Phase 17).
- Anything that touches verifier or oracle code (forever sacred).
