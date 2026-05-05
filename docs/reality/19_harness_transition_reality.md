# Harness Transition Reality

This document records the implementation that makes the coding-agent harness the inner reality loop.

## Problem

Before this change, ContextGraph persisted the model tool request and tool result, then BlackBoxFix verified those artifacts. That proved the tool boundary existed, but it did not make the harness step itself a first-class trainable state transition.

The missing reality was:

```text
before harness state + model tool action -> observed tool result -> outcome label + learning signal
```

Without that transition record, DynamicJEPA, Learning-as-UTL, and constellation machinery could only consume coarse attempt-level summaries. The agent loop could tell Gemma that an attempt failed, but not cleanly explain which harness behaviors moved the loop toward or away from verified success.

## Implementation

Source: `crates/context-graph-cli/src/bin/reality-loop.rs`.

For every governed tool call, the Rust reality loop now writes:

```text
<attempt_root>/agent-tools/round-NN-II-<tool>/harness-transition.json
```

The transition artifact has `record_kind=contextgraph_harness_transition` and includes:

- attempt, round, tool index, tool call id, tool name, and protocol;
- before state: repository HEAD, repository status, executed tool-call count, response source-of-truth;
- action: request source-of-truth, request SHA-256, arguments SHA-256;
- observation: result source-of-truth, result SHA-256, status, error code, observed paths, changed files, candidate patch ref, patch SHA;
- outcome: useful/neutral/harmful label, utility delta, correction-required flag, reuse-observed flag;
- DynamicJEPA projection: deterministic state id, action id, outcome id, transition id, tool family, surprise hint;
- Learning-as-UTL signal: delta_e, surprise, coherence, consolidation, transfer;
- constellation signal: tool-family selectors and failure/success pattern tags;
- BlackBoxFix boundary refs linking request, result, and transition artifacts.

The loop writes the JSON with `write_json_checked`, then immediately reads it back. If the readback differs, the run fails closed with a typed Rust error.

## BlackBoxFix Verification

The per-tool BlackBoxFix verifier now checks all three artifacts:

```text
requested-tool-call.json
tool-result.json
harness-transition.json
```

The verifier command fails unless:

- the requested tool-call file exists and is non-empty;
- the tool-result file exists, is non-empty, and contains `contextgraph_agent_tool_result`;
- the harness-transition file exists, is non-empty, and contains `contextgraph_harness_transition`.

That means the evidence ledger no longer proves only that a tool result was stored. It proves the tool boundary and the measured state transition were both materialized.

## Reality Signal Packet

Each attempt's `reality-signal-packet.json` now includes `harness_reality`.

`harness_reality` is computed by reading every transition artifact referenced by `agent-tool-transcript.json`. It fails closed if an executed tool call is missing a transition ref, if a transition has the wrong record kind, or if the transition count does not equal the executed tool-call count.

The summary includes:

- total transition rows;
- transition coverage;
- counts by tool, outcome label, and tool family;
- summed utility delta;
- correction-required count;
- DynamicJEPA projection counts;
- Learning projection counts;
- Constellation selector counts;
- transition artifact source-of-truth refs and SHA-256 hashes.

## Mapping To Subsystems

DynamicJEPA consumes the transition shape:

```text
state_id + action_id -> outcome_id
```

The harness transition also identifies surprise candidates when tool execution fails or returns blocked/failed status.

Learning-as-UTL consumes:

```text
outcome label + utility_delta + correction_required + reuse_observed
```

It also receives deterministic signal families: delta_e, surprise, coherence, consolidation, and transfer.

Constellation consumes tool-family selectors:

```text
tool_family:repository_navigation
tool_family:workspace_mutation
tool_family:verification
error_code:<typed failure>
status:<ok|blocked|failed>
```

That lets the loop cluster successful and failed harness behaviors, not only final SWE-bench results.

## No Success Claim

Harness transitions are intermediate reward signals. They never replace the official SWE-bench success source of truth.

Official success remains:

```text
official SWE-bench report resolved_ids contains task.instance_id
```

Harness transition reality improves the next prompt and future training samples. It does not let the system claim the SWE task is solved.

## Full State Verification Contract

To verify this feature manually:

1. Run a real Gemma attempt through `target/debug/reality-loop`.
2. Open `agent-tool-transcript.json`.
3. For each `tool_results[].harness_transition`, read the referenced file.
4. Verify `record_kind=contextgraph_harness_transition`.
5. Verify the BlackBoxFix ledger has one tool-call/evidence/verifier set for each governed tool boundary.
6. Open `reality-signal-packet.json`.
7. Verify `harness_reality.transitions_total == agent-tool-transcript.executed_tool_calls`.
8. Verify `harness_reality.transition_reward_signals` is present and the source-of-truth refs point to real files.

This is full state verification for the harness-as-reality-loop claim.
