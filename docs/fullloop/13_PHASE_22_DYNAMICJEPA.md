# Phase 22 — DynamicJEPA Integration (Surprise as Reward)

**Goal:** wire the existing DynamicJEPA subsystem (28 column families, panels/predictions/surprise/plans/audit) into the reality-loop reward signal so the optimizer can react to *latent-state surprise*, not just official `resolved_ids`.
**Effort:** 3–5 days.
**Risk:** high — touches two subsystems and adds a new reward feature category.

## Context

`docs2/contextgraphspecs/21_dynamicjepa_subsystem.md` describes a complete latent-state machinery: domain packs, instrument readings, panels, predictions, surprise events, planning traces, guard decisions, verification runs. It is plumbed but not yet *consumed* by the reality loop.

Reality loop today scores attempts via `gemmafrontend/reward_spec.verified_outcome.json`'s six components (correctness, verification_confidence, risk_penalty, incident_penalty, policy_missing_penalty, training_value). DynamicJEPA-derived signals are absent. The Phase 17 shadow framework is the right place to introduce them.

## What DynamicJEPA can offer the reality loop

| DJ artifact | Reality-loop signal |
|-------------|---------------------|
| Surprise event count per attempt | Penalty weight when latent state diverges from prediction |
| Latent panel divergence | Indicates the inner actor's edits diverged from learned latent dynamics |
| Prediction error norm | Quantifies how badly the predictor missed the post-edit state |
| Plan-trace coverage | Did the agent's tool-use match a known plan? |
| Guard decisions | Did any DJ guard fire? |

Each becomes a new reward component in candidate shadow specs.

## New reality-signal block

Each `<attempt_root>/reality-signal-packet.json` gains a `dynamicjepa_reality` block:

```json
{
  "dynamicjepa_reality": {
    "schema_version": 1,
    "domain_pack": "swe_bench_lite_inner_loop_v1",
    "panel_count": 12,
    "surprise_events": [
      {"event_id": "srp-...", "kind": "latent_state_divergence", "magnitude": 0.83, "source_of_truth": "..."}
    ],
    "prediction_error_l2": 0.42,
    "plan_trace_coverage": 0.71,
    "guard_decisions": [],
    "source_of_truth_refs": ["sqlite:.../dynamicjepa_panels", "..."]
  }
}
```

Engine writes this block by querying the DJ store after the inner actor's tool transcript completes and the validation phase ends. Block writes follow the existing read-after-write byte-equal pattern.

## New shadow reward components (Phase 17 → Phase 22)

Three candidate components proposed at Phase 22 start:

| Component id | Feature | Initial weight | Rationale |
|--------------|---------|----------------|-----------|
| `dj_surprise_penalty` | `dynamicjepa_reality.surprise_events.length` | -0.5 | Surprise = state diverged from prediction = uncertain progress |
| `dj_prediction_accuracy_bonus` | `1.0 - dynamicjepa_reality.prediction_error_l2` | +1.0 | Accurate predictions = state evolves predictably (good) |
| `dj_plan_trace_bonus` | `dynamicjepa_reality.plan_trace_coverage` | +0.5 | Tool-use that matches a known plan = grounded behavior |

These are *proposed*. Phase 17's promotion methodology decides whether any actually become production reward components.

## Inverted use: DJ guides the optimizer's intervention surface

In addition to feeding reward, DynamicJEPA's plan traces and surprise events guide which *intervention surface* the optimizer chooses:

- High surprise on prompt → consider editing the prompt template.
- High surprise on tool surface (specific tool's transitions show divergence) → consider editing tool spec.
- High surprise on reward computation → reward shadow-spec proposal.
- Low surprise overall + low resolved rate → the model itself is the bottleneck (escalate).

The `cc-reality-diagnostician` subagent gains a step that consults `dynamicjepa_reality` before classifying.

## Engine + storage changes

- New MCP tool: `mcp__cgreality__dynamicjepa_reality_for_attempt {run_id, attempt}` returns the DJ block.
- New MCP tool: `mcp__cgreality__dynamicjepa_intervention_hint {run_id, attempt}` returns a structured `{surface_class, magnitude, reasoning}` hint based on surprise sources.
- DJ projection writers in the inner-the inner actor harness (`crates/context-graph-cli/src/bin/reality-loop.rs`) emit panels at the right moments — they already do per `docs2/contextgraphspecs/22_reality_loop_subsystem.md`'s `harness_reality` block; this phase just exposes them as first-class reality-signal data.

## Operator playbook for Phase 22

1. Land Phases 13–21 first.
2. Verify DJ store is healthy: `mcp__cgreality__dynamicjepa_reality_for_attempt` against any existing attempt returns non-empty data.
3. Add the three candidate shadow components via Phase 17 tools.
4. Recompile every existing attempt's reward against each candidate.
5. Run correlation reports.
6. If correlation thresholds pass, surface promotion recommendations.

## What success looks like

The dashboard's reality_score gains a sub-component breakdown showing DJ contribution. Cases where the optimizer correctly *avoided* a speculative harness edit (because surprise was low and the model was simply struggling) become visible in the artifact-scoreboard's `discard` rationale: "DJ surprise low; intervention surface unclear; chose `no_change`."

## AI-verifiable end-state for Phase 22

```bash
mcp__cgreality__dynamicjepa_reality_for_attempt run_id=<r> attempt=<a>
# expected: response includes panel_count >= 1, prediction_error_l2 set

# at least one shadow spec includes dj_* components:
ls <run_root>/reality-optimizer/reward-shadow/candidate-specs/ | xargs -I{} jq '.components[].id' <run_root>/reality-optimizer/reward-shadow/candidate-specs/{} | grep -q '^"dj_'

# correlation report exists for it
test -s <run_root>/reality-optimizer/reward-shadow/correlation/<spec_id>-correlation-report.json
```

## Out of scope for Phase 22

- DJ training (panels improving via training run) — that's a DJ-subsystem task, not reality-loop.
- Cross-task DJ transfer (using DJ knowledge from psf__requests-2317 to predict django__django-12908). Future work.
- Real-time DJ updates during a single attempt. Updates happen at attempt-summary write time only.
