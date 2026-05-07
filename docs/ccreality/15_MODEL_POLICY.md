# Model Policy

## Default model assignments

| Component | Default model | Why |
|-----------|---------------|-----|
| Main optimizer session (writer) | Opus | Architecture-level harness reasoning; the structured-recommendation quality is sensitive to reasoning depth |
| `cc-reality-harness-editor` subagent | Opus | The single source-editing path; needs the best reasoning |
| `cc-reality-diagnostician` subagent | Sonnet | Read-only failure classification; runs on every turn |
| `cc-reality-reward-analyst` subagent | Sonnet | Read-only signal analysis; spawned only on ambiguous diagnoses |
| `cc-reality-swe-failure-triage` subagent | Sonnet | Read-only Docker/env triage; spawned only on official-eval failures |
| `cc-reality-source-of-truth-readback` subagent | Sonnet | Mechanical SHA verification; cheap context |
| Inner actor | qwen3.6:27b GGUF loaded by the Rust harness | Existing; not Anthropic-billed |

The `model` field in `.claude/settings.json` is `opus` (drives the main session). Each subagent's frontmatter overrides per the table above.

## Use the reasoning depth the work warrants

The system runs locally on the operator's hardware. Use as much reasoning depth and as many subagent calls as the work warrants. `blocked` is for real obstacles (drift, MCP disconnect, schema rejection, build failure), not budget.

## Critic ensemble policy

Default ensemble: just the diagnostician. Add reward-analyst when failure_class is `reward_unavailable` or `tool_grounding_failure`. Add swe-failure-triage when failure_class is `official_eval_failure`. Always run readback when status=changed.

Maximum simultaneous critics: 3 (diagnostician + 2 specialists). More than that and the writer's context gets noisy with critic summaries.

## Parallel-agent policy

See [16_PARALLEL_AGENT_POLICY](16_PARALLEL_AGENT_POLICY.md) for the rule: one writer at a time. Critics can run in parallel (read-only).

## Why Opus default, not Sonnet

Earlier draft (`docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md`) suggested Sonnet first, Opus on repeated failure. Revised to Opus-first because:
1. The writer's job is architecture-level — the right tool spec change, the right reward feature, the right prompt directive. Sonnet sometimes proposes plausible-but-wrong edits that pass cargo check but don't move the reward signal. Opus's better reasoning more than pays for itself in solution quality.
2. The shift-principle requires the writer to reason about what its own edits will mutate. Opus does this more reliably.
3. The system runs locally; there is no third-party billing pressure to push toward smaller models.
