# Phase 20 — Operational Scoreboard / Dashboard

**Goal:** make the optimizer's progress legible. Without a dashboard, every recommendation is a needle in a haystack of artifact JSON.
**Effort:** 2–4 days.
**Risk:** low — read-only on the durable artifact tree.

## Three scoreboards (per `docs/reality/14_operational_scoreboard.md`)

### Level 1 — Attempt scoreboard (`<run_root>/reality-scoreboard/attempt-scoreboard.jsonl`)

One row per attempt across all (model, task, attempt) tuples.

Columns: `suite_id, split, model, instance_id, attempt, patch_sha256, patch_parse_status, patch_applied_official, official_resolved, failed_fail_to_pass_count, failed_pass_to_pass_count, reward, loss, incident_count, agent_tool_rounds_total, agent_tool_result_count, agent_tool_names_used, agent_tool_transcript_sha256, prompt_template_id, source_of_truth_refs`.

### Level 2 — Model-task scoreboard (`<run_root>/reality-scoreboard/model-task-scoreboard.json`)

One row per (model, task) pair.

Columns: `best_attempt, resolved_attempt, attempts_to_resolve, best_reward, reward_delta, failure_mode_initial, failure_mode_final, repeated_patch_count`.

### Level 3 — Loop artifact scoreboard (`<run_root>/reality-scoreboard/artifact-scoreboard.json`)

One row per outer-loop artifact candidate (i.e. per `experiment-registry.json` entry).

Columns: `artifact_kind, artifact_version, artifact_sha256, baseline_version, suite, resolved_delta, reward_delta, incident_delta, prompt_size_delta, decision (keep/discard/escalate/promote)`.

## Reality score (single aggregate)

```
reality_score =
    4.0 * official_resolve_rate
  + 1.0 * patch_apply_rate
  + 1.0 * fail_to_pass_reduction_rate
  + 1.0 * reward_improvement_rate
  - 1.5 * pass_to_pass_regression_rate
  - 1.0 * repeated_patch_rate
  - 1.0 * incident_rate
```

Computed across the active runtime root and per (model, task) and per harness-state. Persisted in `<run_root>/reality-scoreboard/reality-scoreboard.md` as a Markdown table the operator reads at-a-glance.

## Source

A new dashboard is added to `gemmafrontend/`:

```
gemmafrontend/
  scoreboard/
    server.py           ← FastAPI + Jinja templates
    templates/
      attempt_table.html
      model_task_table.html
      artifact_table.html
      reality_score.html
    static/
      reality-score.css
```

Endpoints:
- `GET /scoreboard/` — main view, all three levels in collapsible sections.
- `GET /scoreboard/attempts.json` — JSON of attempt scoreboard.
- `GET /scoreboard/champion` — current champion-state.json contents.
- `GET /scoreboard/recommendations` — list of every `claude-code-optimizer/recommendation-*.json` in active runtime, paginated.
- `GET /scoreboard/shift-log/<session_id>` — shift log for a session.

The server reads from disk only. No write paths. No GPU dependencies. Runs on `:8788` (separate from existing `gemmafrontend/server.py` on `:8787`).

## Engine changes

`reality-loop` after each attempt:
1. Append a row to `attempt-scoreboard.jsonl`.
2. Recompute `model-task-scoreboard.json` (full rewrite is fine; the file stays small).
3. Recompute `reality-scoreboard.md`.

The artifact scoreboard updates whenever `experiment_registry_update_outcome` is called (Phase 15).

## New MCP tools

| Tool | Purpose |
|------|---------|
| `scoreboard_summary` | summarized counts + reality_score for a run |
| `scoreboard_get_attempt_row` | lookup by attempt path |
| `scoreboard_render_markdown` | regenerate `reality-scoreboard.md` |

The dashboard server uses these tools internally, not direct file reads, so policy stays consistent.

## What "good progress" looks like on the dashboard

After a sustained `/loop` run:
- Attempt scoreboard rows accumulate; `official_resolved` count grows.
- Model-task scoreboard shows attempts-to-resolve declining for instances that have resolved.
- Artifact scoreboard shows experiments transitioning from `pending` to `kept` or `discarded`.
- Reality score trends upward (despite occasional regressions, which are normal).

## What "bad progress" looks like

- Reward improving but `official_resolve_rate` flat → reward hacking (Phase 17 shadow-spec correlation analysis flags this).
- Same prompt template reused across many failed attempts → optimizer is not actually editing things.
- Many `repair` decisions but no `keep` → loop is churning, not learning.

The dashboard surfaces all three patterns automatically with red/yellow/green indicators on the reality_score panel.

## AI-verifiable end-state for Phase 20

```bash
test -s <run_root>/reality-scoreboard/attempt-scoreboard.jsonl
test -s <run_root>/reality-scoreboard/model-task-scoreboard.json
test -s <run_root>/reality-scoreboard/artifact-scoreboard.json
test -s <run_root>/reality-scoreboard/reality-scoreboard.md

curl -fsS http://localhost:8788/scoreboard/   # 200 OK
curl -fsS http://localhost:8788/scoreboard/attempts.json | jq 'length' # >= 1
mcp__cgreality__scoreboard_summary
```

## Out of scope

- Real-time WebSocket pushes (operator hits refresh).
- External BI integration.
- Investor-facing summary export — that's `gemmafrontend/server.py`'s `build_investor_category_pack` tool, untouched.
