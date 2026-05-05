# Cost & Model Policy

## Default model assignments

| Component | Default model | Why |
|-----------|---------------|-----|
| Main optimizer session (writer) | Opus | Architecture-level harness reasoning; one turn per trigger; cost per turn bounded |
| `cc-reality-harness-editor` subagent | Opus | The single source-editing path; needs the best reasoning |
| `cc-reality-diagnostician` subagent | Sonnet | Read-only failure classification; runs on every turn; volume justifies Sonnet |
| `cc-reality-reward-analyst` subagent | Sonnet | Read-only signal analysis; spawned only on ambiguous diagnoses |
| `cc-reality-swe-failure-triage` subagent | Sonnet | Read-only Docker/env triage; spawned only on official-eval failures |
| `cc-reality-source-of-truth-readback` subagent | Sonnet | Mechanical SHA verification; cheap context |
| Inner Gemma actor | gemma4:e4b (local Ollama) | Existing; not Anthropic-billed |

The `model` field in `.claude/settings.json` is `opus` (drives the main session). Each subagent's frontmatter overrides per the table above.

## Cost cap per turn

Target: **<$0.50 per turn**. A typical turn:
- 1 main-session reasoning call (Opus) — ~10–25 K tokens in/out.
- 1 diagnostician subagent (Sonnet) — ~5–15 K tokens.
- 0–1 reward-analyst (Sonnet) — ~3–10 K.
- 0–1 swe-triage (Sonnet) — ~3–10 K.
- 1 harness-editor (Opus) — ~5–20 K tokens IF status=changed; 0 if no_change.
- 1 readback (Sonnet) — ~2–5 K tokens IF status=changed.
- ~10–30 MCP tool calls — token-light per call but additive.

Budget at 2026 list rates: ~$0.30 per turn typical, ~$0.50 worst-case (status=changed with full critic ensemble).

## Cost-cap enforcement

Two layers:

### Layer 1 — agent-side soft cap

In `.claude/loop.md`, the iteration procedure includes a check at the end:

```
If you have already spent more than 0.50 USD on this turn (estimate from
the cumulative model usage you can see in the tool-result envelopes), record
status=blocked with reason="cost_cap_reached" and stop.
```

This is heuristic — the agent is not a perfect token counter. But it's a useful guardrail.

### Layer 2 — environment-side hard cap

The watcher script that spawns Claude Code (see [14_TRIGGER_AND_INVOCATION_POLICY](14_TRIGGER_AND_INVOCATION_POLICY.md), method E) sets:

- `--max-turns 25` (caps total model rounds in one session)
- `--max-output-tokens` (per-call cap)
- Polls `~/.cache/cgreality/cost_today.json` (a local accumulator) and refuses to spawn if today's total > $20.

The accumulator is updated by the `Stop` hook from the `total_cost_usd` field in Claude Code's session report (available when the session was started with `--output-format json`).

## When to escalate to "spend more"

Some failures are worth Opus + extended thinking (`/effort high` or `/effort max`):

- Repeated `blocked` on the same root cause across three turns.
- A failure class the diagnostician marks as `unknown` after consulting all critics.
- An operator-marked priority case (operator runs `/loop` after editing `~/.cache/cgreality/priority_marker` to set higher cap).

The operator can override the per-turn cap by passing `--effort max` to `claude -p` or by switching `model: opus` to `model: opus, effort: max` in `.claude/settings.json`.

## When to spend less

Some turns are mechanical and Sonnet is sufficient for the writer:

- Reverting a known-bad change (operator has already triaged).
- Writing a memory-only recommendation (`store_memory` for a pattern, no harness edit).
- Drift recovery (re-perceive only; no new edit).

For these, the operator can launch with `--model sonnet` instead of opus. The CLAUDE.md addendum and skill prompts work on either model.

## Per-model rate limits

Anthropic rate limits per minute:
- Opus 4.7: ~50 requests / 100 K tokens (production tier)
- Sonnet 4.6: ~50 requests / 200 K tokens
- Haiku 4.5: ~100 requests / 200 K tokens

A single ccreality turn uses ~3–8 model calls total (main + 1–4 subagents + readback). Well within limits even for high-frequency loops. The main constraint is Anthropic billing, not throughput.

## Why Opus default, not Sonnet

Earlier draft (`docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md`) suggested Sonnet first, Opus on repeated failure. Revised to Opus-first because:
1. Each turn is event-triggered, low frequency. Opus's marginal cost over Sonnet is small in absolute dollar terms.
2. The writer's job is architecture-level — the right tool spec change, the right reward feature, the right prompt directive. Sonnet sometimes proposes plausible-but-wrong edits that pass cargo check but don't move the reward signal. Opus's better reasoning more than pays for itself.
3. The shift-principle requires the writer to reason about what its own edits will mutate. Opus does this more reliably.

We may revisit this after the first 100 turns of real data. The cost ledger (`record_kind = "ccreality_optimizer_recommendation"` rows in BlackBox) lets us aggregate model-vs-status to see the actual ROI.

## Critic ensemble policy

Default ensemble: just the diagnostician. Add reward-analyst when failure_class is `reward_unavailable` or `tool_grounding_failure`. Add swe-failure-triage when failure_class is `official_eval_failure`. Always run readback when status=changed.

Maximum simultaneous critics: 3 (diagnostician + 2 specialists). More than that and the writer's context gets noisy with critic summaries.

## Parallel-agent policy

See [16_PARALLEL_AGENT_POLICY](16_PARALLEL_AGENT_POLICY.md) for the rule: one writer at a time. Critics can run in parallel (read-only).
