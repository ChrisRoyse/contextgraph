# Phase 17 — Reward Shadow Optimization

**Goal:** let the optimizer iterate on reward features without ever altering the production reward spec, by running candidate specs in shadow mode against the same attempts.
**Effort:** 2–3 days.
**Risk:** medium — methodology around correlation thresholds requires care.

## Why shadow

`gemmafrontend/reward_spec.verified_outcome.json` is the single source of truth for reward and is in the deny list (under `crates/*/src/reward_specs/**` for the engine-internal copy, plus the gemmafrontend root copy is operationally treated as immutable). Editing it without verification risks reward hacking — the loop could optimize itself by re-weighting toward easy-to-satisfy components.

Shadow mode lets the `cc-reality-reward-analyst` subagent propose new components, attach them to a *candidate* spec, and have the engine re-score every existing attempt against it. Promotion only happens after correlation with verified outcomes is rigorously checked.

## Artifact layout

```
<run_root>/reality-optimizer/reward-shadow/
  candidate-specs/
    <spec-id>.json                     ← full reward spec, schema-valid
    <spec-id>.proposal.md              ← reward-analyst's reasoning
  recompiled/
    <attempt-path-hash>/
      shadow-reward-<spec-id>.json     ← scalar + per-component breakdown
  correlation/
    <spec-id>-correlation-report.json  ← Pearson + Spearman vs official_resolved
```

Each shadow spec has the same schema as the production spec. The proposal Markdown explains *why* the components were chosen.

## Engine changes

`reality-loop attempt` and `reality-loop verify` both gain a hidden `--shadow-reward-specs` flag (CSV of paths). When set:

- After the production reward is computed, the engine recompiles reward against each shadow spec.
- Shadow rewards are written to `recompiled/<attempt-path-hash>/shadow-reward-<spec-id>.json`.
- The production reward path remains the unique success oracle for any decision-making.

## New MCP tools

| Tool | Purpose |
|------|---------|
| `reward_shadow_propose` | persist a candidate spec + proposal | `{ spec, proposal_markdown }` |
| `reward_shadow_recompile_all` | re-score every existing attempt against a candidate | `{ spec_id }` |
| `reward_shadow_correlation_report` | compute Pearson/Spearman vs `official_resolved` | `{ spec_id, holdout_attempts? }` |
| `reward_shadow_promote_recommendation` | record the reward-analyst's promotion recommendation (reviewer must approve manually) | `{ spec_id, justification }` |

`reward_shadow_propose` is an alter tool and appends to the shift log.

## Methodology — when can a shadow spec be promoted?

A shadow spec `<spec-id>` is *promotion-recommendation-eligible* iff:

1. ≥ 30 attempts in the run have official Docker readback.
2. Pearson correlation of shadow scalar with `official_resolved` exceeds production spec's by ≥ 0.10.
3. Spearman correlation also exceeds production by ≥ 0.05 (rank robustness).
4. Per-component contribution analysis shows no single component contributes > 50% of the gain (no "one-feature-takes-all" hacking).
5. On a holdout attempt set (specified by Phase 19), the same gain is reproduced (within ±0.05).

Even when all five hold, *only the operator promotes*. The MCP tool refuses to write to the production reward spec path. It writes a `promote-recommendation.md` for human review.

## Reward-analyst subagent's expanded role

`cc-reality-reward-analyst` (Sonnet, read-only) is upgraded with these capabilities:

- Read every attempt's reality-signal packet and current production reward output.
- Propose a candidate component (e.g., "add `tool_grounding_quality` with weight +1.0").
- Generate the candidate spec JSON via `reward_shadow_propose`.
- Trigger `reward_shadow_recompile_all`.
- Compute and persist a correlation report.
- Recommend promotion (or not) via `reward_shadow_promote_recommendation`.

It never proposes a component that re-weights or removes an existing component without explicit operator instruction.

## What the analyst should *not* do

- Add components that depend on data not durable on disk (model self-confidence, etc.).
- Add components that correlate with `official_resolved` only because they are *part of* the resolved-prediction pipeline (label leakage).
- Propose a candidate spec that produces score > current production for >70% of unresolved attempts (very likely a hacking signal).

The reward-analyst's proposal Markdown must affirmatively address each of these for every proposal.

## AI-verifiable end-state for Phase 17

```bash
mcp__cgreality__reward_shadow_propose spec='{...}' proposal_markdown='...'
mcp__cgreality__reward_shadow_recompile_all spec_id=<id>
mcp__cgreality__reward_shadow_correlation_report spec_id=<id>
test -s <run_root>/reality-optimizer/reward-shadow/correlation/<id>-correlation-report.json

# verify the production reward spec was NEVER touched:
git diff -- gemmafrontend/reward_spec.verified_outcome.json   # empty
```

## Cross-references

- `docs/reality/06_governance_and_reward_hacking_controls.md` — full governance design.
- `docs/reality/05_auto_optimization_engine.md` — reward-tuning experiment type.
- Phase 19 (curriculum) provides the holdout split needed for step 5 above.
