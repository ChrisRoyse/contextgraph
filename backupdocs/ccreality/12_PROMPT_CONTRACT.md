# Prompt Contract — what every optimizer turn receives

The optimizer turn sees three concentric layers of prompt context:

1. **System layer** (constant across turns): Claude Code's default system prompt + the project's `CLAUDE.md` (which now includes the ccreality role addendum from [10_CLAUDE_MD_PROJECT_MEMORY](10_CLAUDE_MD_PROJECT_MEMORY.md)) + the `.claude/loop.md` body when `/loop` fires.
2. **Drip layer** (per-turn, hook-injected): SessionStart on the first prompt, UserPromptSubmit on every subsequent prompt, PreCompact on compactions. See [02_REALITY_DRIP_PROTOCOL](02_REALITY_DRIP_PROTOCOL.md).
3. **Tool-output layer** (interactive within the turn): every MCP tool call returns a payload that includes `source_of_truth` paths, shift_ids, and SHAs. The agent reasons against these.

This document specifies the exact text shapes for each layer.

## System layer

### `CLAUDE.md` (verbatim block)

See [10_CLAUDE_MD_PROJECT_MEMORY](10_CLAUDE_MD_PROJECT_MEMORY.md) for the addendum. The block is fenced with `---` markers and appended to the existing project `CLAUDE.md`.

### `.claude/loop.md` (verbatim)

See [11_LOOP_PROMPT](11_LOOP_PROMPT.md) for the body. Loaded by Claude Code when `/loop` is invoked.

### Default Claude Code system prompt (untouched)

Comes from Claude Code itself. We do not override; we append via `--append-system-prompt` in headless mode if needed:

```
--append-system-prompt "$(cat .claude/ccreality_append.md)"
```

`.claude/ccreality_append.md` is identical to the addendum block in `CLAUDE.md`, used only when running headless without auto-loaded CLAUDE.md (which should not happen because we never use `--bare`).

## Drip layer

Each hook emits its own `additionalContext` block. These appear in the agent's transcript as messages tagged with the hook event name. The shapes are specified in [07_HOOKS_SPEC](07_HOOKS_SPEC.md). Recap of what the agent sees:

### From SessionStart (one-time)
```
Active reality-loop runtime root: /tmp/contextgraph-rust-reality-loop/run-2026-05-05-...
Target SWE-bench instance: django__django-12345
Success predicate: official SWE-bench report places target instance in resolved_ids
Run status: in_progress. Attempts so far: 3.
ContextGraph MCP tools available under prefix mcp__cgreality__. Call mcp__cgreality__reality_latest_root first to confirm the active root, then mcp__cgreality__reality_attempt_summary with compact: true to inspect the most recent attempt unless full transcript readback is required.
Reality-boundary rules (deterministic deny list also applies):
- Never edit .env files
- Never edit swebench/ official evaluator
- Never edit tests/oracle_** benchmark fixtures
- Never edit crates/*/src/reward_specs/** or verifiers/**
- Never claim SWE-bench success unless official resolved_ids contains the target instance
```

### From UserPromptSubmit (every turn)
```
Latest attempt: 3 at /tmp/.../attempt-3
Trigger decision: {"status":"triggered","should_run":true,"policy":"failure_or_regression","reasons":["official_swe_not_resolved","repeated_patch_detected"]}. Source of truth: file:/tmp/.../attempt-3/trigger-decision.json
Reality signal compact: {"failure_digest":{"class":"patch_apply_failure","first_error_location":"auth.py:42"},"official_progress":{"fail_to_pass":{"failure_count":1}},"patch_signals":{"repeated_patch":true,"patch_sha256":"sha256:..."},"score_reality":{"reward":{"clipped":0.12}},"reinjection_directive":"Re-read problem-reality oracle paths before drafting next patch."}. Source of truth: file:/tmp/.../attempt-3/reality-signal-packet.json
Last 3 reality shifts in this session: [{"shift_id":"01J...","tool_name":"harness_apply_line_window_edit","subject":{"type":"file","path":"crates/context-graph-cli/src/bin/reality-loop.rs"},"delta":{"lines_added":12,"lines_removed":4},"verification":{"cargo_check":"PASS"}}]. Source of truth: file:/tmp/.../cgreality-shift-log/<session>.jsonl
```

### From PreToolUse Edit (per edit)
```
Pre-edit reality: file=crates/context-graph-cli/src/bin/reality-loop.rs current_sha=sha256:.... Governed-path policy: allow.
```

### From PostToolUse Edit (per successful edit)
```
Post-edit reality: file=crates/context-graph-cli/src/bin/reality-loop.rs new_sha=sha256:.... Diff stat: 1 file changed, 12 insertions(+), 4 deletions(-). cargo check -p context-graph-cli --bin reality-loop: PASS. harness_transition: file:/tmp/.../attempt-3/harness-transitions/round-04.json
```

### From PreCompact (per compaction)
Same payload as SessionStart, prefixed with `(post-compaction reality re-injection)`.

## Tool-output layer

### Pattern: every analyze tool returns `source_of_truth`

Example `mcp__cgreality__reality_attempt_summary` response:
```json
{
  "attempt": { ... },
  "model_response": { ... },
  "blackbox_verifier": { ... },
  "score": { ... },
  "reward": { ... },
  "ledger_counts": { ... },
  "source_of_truth": "file:/tmp/.../attempt-3/attempt-summary.json"
}
```

### Pattern: every alter tool returns shift_id + before/after

Example `mcp__cgreality__harness_apply_line_window_edit` response:
```json
{
  "status": "ok",
  "path": "crates/context-graph-cli/src/bin/reality-loop.rs",
  "start_line": 5126,
  "end_line": 5180,
  "before_sha256": "sha256:abc...",
  "after_sha256": "sha256:def...",
  "lines_added": 12,
  "lines_removed": 4,
  "git_diff_path": "file:/tmp/.../attempt-3/claude-code-optimizer/diffs/<turn>-<unix>.diff",
  "harness_transition_path": "file:/tmp/.../attempt-3/harness-transitions/round-04.json",
  "shift_id": "01J9...",
  "source_of_truth": "file:/tmp/.../attempt-3/harness-transitions/round-04.json"
}
```

The agent uses `shift_id` to fetch the exact record back via `reality_shift_log` if needed, and includes it in the recommendation's `shift_log_excerpt`.

## What the agent must never receive

- Raw model output from another LLM (we don't proxy DeepSeek/OpenAI).
- Unstructured stderr from external processes (we capture stderr_tail bounded to ~4 KB; never dump unbounded output).
- Inlined large files (>32 KB). We always return pointers and let the agent fetch via `reality_replay_artifact` if it needs more.
- Any `mcp__<other-server>__*` tool — only `cgreality` is exposed to the optimizer (per `permissions.allow` in [06_SETTINGS_JSON_SPEC](06_SETTINGS_JSON_SPEC.md)).

## What the agent must always produce

The terminal artifact of every turn is `mcp__cgreality__optimizer_record_recommendation`, conforming to [13_STRUCTURED_OUTPUT_SCHEMA](13_STRUCTURED_OUTPUT_SCHEMA.md). The Stop hook enforces this.

The artifact includes:
- `source_files_changed`: every file the agent edited.
- `source_of_truth_readbacks`: file:... paths the agent claims to have actually re-read after editing.
- `shift_log_excerpt`: the shift_ids of the mutations this turn produced. Lets future turns replay.
- `verification_commands`: the exact commands the agent ran (or had a subagent run).
- `next_inner_prompt_delta`: the one-line directive to feed to the next inner-the inner actor attempt.
- `created_at_unix` and `source_of_truth`: schema-shaped placeholders only. The MCP server stamps the actual write timestamp and file path.

## Prompt-injection defense (recap)

Every drip injection is phrased as a factual statement, never as an imperative addressed to Claude. The hook scripts use `additionalContext` (Claude Code's structured channel) instead of raw stdout to make the source clear. If a drip ever embeds user-controlled text (e.g. an SWE-bench problem statement), it must be quoted/fenced or summarized — never injected raw — to prevent inner-prompt confusion.

This is most relevant when the SWE-bench problem statement contains text like "the function should ignore comments starting with #" — that text could be misread as an instruction to Claude. The reality-signal-packet builder (in the Rust engine) already escapes such content; the MCP tools and hook scripts MUST preserve that escaping.
