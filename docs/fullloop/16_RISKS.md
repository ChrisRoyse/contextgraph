# Risks and Mitigations

The forward plan introduces new ways the loop can fail. Each risk has an owner (where to look) and a mitigation already designed in.

## R1 — Reward hacking

**Risk:** the optimizer edits the harness in a way that increases reward without increasing real `resolved_ids`.

**Mitigations:**
- Production reward spec is in the deny list; only shadow specs can be edited.
- Promotion requires correlation with `official_resolved` (Phase 17).
- Promotion split must show no regression in resolved counts (Phase 19).
- Any single component contributing > 50% of a shadow spec's gain is rejected as a hacking signal.
- Champion is `candidate` until promoted; promotion requires human approval of the reward-analyst's `promote-recommendation.md`.

**Detection:** dashboard reality_score "reward up but resolved_rate flat" pattern (Phase 20). Auto-flag.

## R2 — Cost runaway

**Risk:** /loop spawns more subagents per turn than expected; daily API spend exceeds budget.

**Mitigations:**
- Per-recommendation cost ceiling enforced by Phase 13 task T13.4.
- Daily cap enforced by `cgreality-tick.sh` (Phase 21).
- Operator dashboard surfaces cost-summary on the same page as reality_score.
- One-writer rule (one harness-editor subagent per turn) bounds parallelism.

**Detection:** daily summary cron emits an alert at 75% of cap.

## R3 — Stale state edits (mitigated by Phase 0; verified continuously)

**Risk:** the optimizer edits a file based on stale SHA; another writer (operator, watcher, simultaneous /loop) mutated it first.

**Mitigations:**
- `harness_open_window` returns SHA; `harness_apply_line_window_edit` requires `observed_sha256` back.
- `reality_shift_compare_to_my_view` is invoked at /loop start (Phase 13 T13.3).
- Concurrency lock at `~/.cache/cgreality/active_runtime_root.lock` prevents two sessions writing.

**Detection:** mismatch returns `HARNESS_EDIT_SHA_MISMATCH` immediately; recommendation is `blocked` with drift detail.

## R4 — Inner-model regression (the loop made the inner actor worse)

**Risk:** an "improvement" actually narrows the inner actor's behavior — fewer parsable patches, more retries, lower reward.

**Mitigations:**
- `experiment-registry.json` tracks every harness candidate's keep/discard verdict (Phase 15).
- Champion-state.json holds the previous best result; new candidate must beat it.
- Promotion gate (Phase 19) catches regressions on holdout/promotion splits.
- Operator can revert any kept experiment per the runbook procedure.

**Detection:** reward-series.jsonl shows a step-down that aligns with a `kept` experiment's start.

## R5 — Cross-model contamination

**Risk:** raw Qwen output leaks into the inner actor's prompt, polluting it with unverified claims.

**Mitigations:**
- Cross-model memory namespace stores only verified failure facts (Phase 16).
- Engine-enforced rule: a memory is created only after official Docker readback proves the failure.
- Prompt compiler quality gate fails if memory text contains the gold patch or solution-leakage markers (existing check; extended in Phase 16).

**Detection:** prompt-quality status `hidden_solution_leak_absent` regression.

## R6 — Drift on long-lived runtime root

**Risk:** runtime root accumulates millions of artifacts; disk fills; index lookups slow.

**Mitigations:**
- `mcp__cgreality__shift_log_compact` (Phase 13 T13.5) rotates stale files.
- Operator can roll runtime root to a fresh path via `~/.cache/cgreality/active_runtime_root` reassignment.
- BlackBox ledger is per-task, not per-root; it stays bounded.

**Detection:** dashboard shows runtime-root size at top.

## R7 — `Stop` hook indefinite block

**Risk:** the Stop hook refuses to allow turn end because the model didn't call `optimizer_record_recommendation`. Session hangs.

**Mitigations:**
- `.claude/loop.md` step 7 is "Record" — the model is explicitly told this is mandatory.
- The `cc-reality-harness-editor` subagent's contract includes "the parent must record the recommendation; you are not the parent."
- Session timeout in Claude Code default settings is finite.

**Detection:** session transcript ends without a `recommendation-*.json` file → manual recovery (operator forces `optimizer_record_recommendation` with `status: error, reason: stop_hook_recovery`).

## R8 — Subagent contention / deadlock

**Risk:** `cc-reality-harness-editor` is spawned twice in the same turn; both try to write the same file.

**Mitigations:**
- `docs/ccreality/16_PARALLEL_AGENT_POLICY.md` declares one writer rule.
- `.claude/loop.md` reads "Cap each turn at one `cc-reality-harness-editor` invocation."
- Future: explicit lock file per harness-editor invocation (low priority; not yet observed).

**Detection:** post-hoc; check `harness_transitions/` for two entries with overlapping timestamps from different tool_use_ids.

## R9 — DynamicJEPA reward miscalibration (post-Phase 22)

**Risk:** `dj_surprise_penalty` weight is too high; the optimizer becomes overly conservative.

**Mitigations:**
- DJ components enter via Phase 17 shadow framework; never directly into production.
- Initial weights are deliberately small (-0.5 / +1.0 / +0.5); promotion requires correlation evidence.

**Detection:** post-promotion correlation re-check after 30 attempts; revert via Phase 17 tools if regression.

## R10 — Operator scope drift

**Risk:** operator starts editing harness manually mid-/loop, breaking the optimizer's mental model.

**Mitigations:**
- Reality-shift principle: every external edit triggers `external_drift_detected` next turn.
- Runbook says: pause /loop before manual edits, restart after.
- Recommendation schema's `drift_detected.files` field captures any drift the optimizer encounters.

**Detection:** `external_drift_detected` artifact + operator self-discipline.

## R11 — Schema drift

**Risk:** the recommendation schema or governed-paths schema gets edited inconsistently across the codebase.

**Mitigations:**
- Both schemas live in `config/` and are read at MCP-server startup.
- The recommendation schema is hot-reloadable per Phase 5.
- Schema validation is mandatory in `optimizer_record_recommendation`.

**Detection:** any `CCREALITY_RECOMMENDATION_SCHEMA_INVALID` error → fix schema, re-run.

## R12 — Loss of trust in `resolved_ids`

**Risk:** SWE-bench Lite official harness changes its report structure; `resolved_ids` field renamed; loop falsely shows "no resolved" forever.

**Mitigations:**
- `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` already raises when classification is missing.
- Runtime measurement contract pins the harness command + dataset.
- Operator runs the official-eval gold-path readback (`/tmp/contextgraph-official-report-cwd-smoke/`) on schedule to confirm structure.

**Detection:** missing classification → typed Rust error, not silent miss.

## What is NOT a risk we mitigate

- "The optimizer makes a non-optimal harness change." That's the *purpose* of the loop — it must be free to try, fail, and learn. The mitigation is the experiment registry, not preventing the change.
- "An attempt fails." Failure is the loop's input. Phase 12 already proved the system handles this correctly.
- "the inner actor can't solve a hard task." Out of scope for the meta-loop; covered by curriculum (Phase 19).
