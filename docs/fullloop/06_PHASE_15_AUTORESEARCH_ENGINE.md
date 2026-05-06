# Phase 15 — Autoresearch Engine (Champion State + Experiment Registry)

**Goal:** add the durable per-run experiment registry and champion state described in `docs/reality/05_auto_optimization_engine.md`. This is what lets the outer loop reason about its *own* progress, not just per-attempt outcomes.
**Effort:** 2–3 days.
**Risk:** low — additive, no behavior change to existing artifacts.

## Why this is necessary

After Phase 14 lands the first `resolved_ids`, the loop is *capable* but still treats every turn as independent. Without experiment tracking it cannot answer:

- "Have I made the harness better or worse over the last 50 turns?"
- "Which prompt template is the current champion?"
- "Was harness-edit X kept or discarded? Why?"
- "How many distinct candidate harness states have we tried?"

Phase 15 closes that gap.

## New artifacts

```
<run_root>/reality-optimizer/
  experiment-registry.json     ← versioned list of every harness candidate
  champion-state.json          ← best (model, task) result so far
  attempts.jsonl               ← compact one-line per attempt summary
  prompt-compiler-runs.jsonl   ← every prompt manifest hash + outcome
  reward-series.jsonl          ← scalar reward over time per (model, task)
  decisions.jsonl              ← keep/discard/repair/escalate/promote per attempt
```

All of these are written by the engine (which sees attempts), updated by the MCP server (which sees recommendations), and read by Claude Code as analyze inputs.

### `experiment-registry.json`

```json
{
  "schema_version": 1,
  "experiments": [
    {
      "experiment_id": "exp-20260505-122400-abc",
      "harness_change_summary": "added open_test_file tool spec to agent_tool_specs",
      "harness_change_recommendation_path": "file:/.../recommendation-turn-04.json",
      "harness_change_shift_ids": ["01JxxxX..."],
      "files_changed": ["crates/context-graph-cli/src/bin/reality-loop.rs"],
      "before_attempt_count": 12,
      "after_attempt_count": 0,
      "outcome": "pending | kept | discarded | escalated",
      "reasoning": "champion reward delta required ≥0.1; observed +0.27"
    }
  ]
}
```

### `champion-state.json`

```json
{
  "schema_version": 1,
  "champions": [
    {
      "model": "gemma4:e4b",
      "task": "psf__requests-2317",
      "attempt_path": "file:/.../attempt-000017",
      "official_resolved": true,
      "best_reward": 4.31,
      "patch_sha256": "sha256:...",
      "promoted_at_unix": 1714896000
    }
  ]
}
```

A champion is *promoted* (Phase 19) only if held-out + promotion split also pass. Until promotion, champions are *candidate* champions.

## New MCP tools

Add to the `cgreality` server (in `crates/context-graph-mcp/src/handlers/tools/reality_loop/`):

| Tool | Purpose | Input | Output |
|------|---------|-------|--------|
| `experiment_registry_list` | list experiments | `{ status_filter?: "pending|kept|discarded|escalated", limit?: int }` | `{ experiments: [...], source_of_truth: "file:..." }` |
| `experiment_registry_get` | full experiment record | `{ experiment_id }` | `{ experiment: {...}, source_of_truth }` |
| `experiment_registry_update_outcome` | record keep/discard/escalate | `{ experiment_id, outcome, reasoning }` | `{ updated: true, source_of_truth, shift_id }` |
| `champion_state_get` | current champion(s) | `{ model?: string, task?: string }` | `{ champions: [...], source_of_truth }` |
| `champion_state_propose` | mark a candidate champion | `{ model, task, attempt_path, reward }` | `{ proposed: true, source_of_truth, shift_id }` |

All alter operations append to the per-session shift log per `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`.

## Engine changes

`reality-loop` binary:
- After each attempt completes, append one line to `<run_root>/reality-optimizer/attempts.jsonl` with `{attempt_id, model, task, status, official_resolved, reward, prompt_template_id, prompt_manifest_sha256, patch_sha256, source_of_truth_refs}`.
- After each prompt compilation, append to `prompt-compiler-runs.jsonl`.
- After each reward compilation, append the scalar to `reward-series.jsonl`.

These are JSONL append-only with the existing read-after-write byte-equal verification path.

## `/loop` body changes

`.claude/loop.md` step 4 (Decide) now consults `champion_state_get` and `experiment_registry_list` before drafting an intervention:

> If a champion already exists for this (model, task) and the latest attempt's reward is within 5% of the champion's reward, prefer `no_change` over a speculative harness edit. The loop should not churn the harness while close to the verified champion.

> If three consecutive experiments for similar diagnoses were `discarded`, mark the new candidate as `requires human inspection` instead of running another speculative edit.

## Decision rule (engine-side)

After every attempt, the engine evaluates and writes `decisions.jsonl` row using the rule from `docs/reality/05_auto_optimization_engine.md`:

```
decision = "keep"     if reward improved AND mandatory verifiers passed
decision = "promote"  if held-out + promotion verification also passed (Phase 19)
decision = "discard"  if reward equal/worse OR repeated patch OR not useful
decision = "repair"   if failure is actionable AND attempt budget remains
decision = "escalate" if policy/credential/verifier/environment issue
```

The optimizer's recommendation MUST cite this decision in `reason` when it acts on (or chooses not to act on) it.

## AI-verifiable end-state for Phase 15

After running 5 attempts with at least one optimizer turn each:
```bash
test -s <run_root>/reality-optimizer/experiment-registry.json
test -s <run_root>/reality-optimizer/champion-state.json
[[ $(wc -l < <run_root>/reality-optimizer/attempts.jsonl) -ge 5 ]]
mcp__cgreality__experiment_registry_list status_filter=pending
mcp__cgreality__champion_state_get
```

All commands succeed and return well-formed JSON.

## Cross-references

- `docs/reality/05_auto_optimization_engine.md` — full design.
- `docs/reality/14_operational_scoreboard.md` — how scoreboard reads these artifacts (Phase 20).
- `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md` — pattern for new MCP tools.
- `docs/fullloop/04_PHASE_13_HARDENING.md` — must complete first.
