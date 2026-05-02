# Training And Eval Corpus

BlackBoxFix should turn verified agent operations into high-quality training and
eval data. The key is that each row includes the expected delta and the actual
readback, not just a transcript.

## Relationship To Existing Exporter

The existing DynamicJEPA bundle exporter already uses the right shape:

- `state`;
- `action`;
- `expected_persisted_delta`;
- `actual_readback`;
- `verifier_status`;
- `error_code`;
- `remediation`.

BlackBoxFix generalizes that pattern from DynamicJEPA bundles to arbitrary agent
runs and operational connectors.

## Episode Row Contract

```json
{
  "schema_version": 1,
  "record_kind": "verified_agent_episode",
  "episode_id": "run_123:step_4:tool_2",
  "source": {
    "tenant_id": "tenant_...",
    "run_id": "run_123",
    "step_id": "step_4",
    "tool_call_id": "tool_2",
    "evidence_packet_id": "ev_..."
  },
  "state": {
    "task": "upgrade vulnerable dependency",
    "pre_state": {},
    "policy_context": {},
    "available_tools": []
  },
  "action": {
    "tool_name": "github_create_pull_request",
    "tool_input_redacted": {},
    "model_decision_redacted": {}
  },
  "expected_persisted_delta": {},
  "actual_readback": {},
  "verifier_status": {
    "status": "passed",
    "score": {}
  },
  "error_code": null,
  "remediation": null
}
```

## Data Splits

The exporter should support:

- `passed`: correct actions.
- `failed`: outcome mismatch examples.
- `blocked`: governance and precondition failures.
- `incident`: high-risk failure packets.
- `edge_case`: manual FSV edge-case rows.

Do not discard failures. Failures are high-value training data for tool-use
planning, verifier-aware reasoning, and recovery.

## Training Uses

### Process Supervision

Train models to prefer actions that lead to verified state, not just plausible
text. The model sees task state, action, expected delta, actual readback, and
verifier result.

### Tool-Use Fine-Tuning

Teach exact tool schema use, precondition checking, and remediation behavior.

### Eval Generation

Turn each failure class into an eval:

- prompt injection attempts;
- unauthorized tool invocation;
- missing readback permission;
- wrong source-of-truth state;
- incomplete remediation;
- policy-denied action.

### Recovery Training

Use failed and blocked episodes to train the next action:

```json
{
  "state": {
    "verifier_status": "failed",
    "error_code": "CI_REQUIRED_CHECK_FAILED"
  },
  "expected_next_action": {
    "tool_name": "ci_get_test_artifact",
    "reason": "inspect failing test before editing again"
  }
}
```

## Export Modes

- `episodes.jsonl`: main training/eval rows.
- `manifest.json`: counts, hashes, source ledger ids, schema versions.
- `evidence/`: optional redacted evidence packet files.
- `evals/`: generated regression eval fixtures.

## Export Acceptance

The exporter must:

- read from the physical ledger, not in-memory return values;
- reject broken hash chains;
- reject missing verifier results for mutating actions;
- parse every JSONL row after writing;
- hash the output file;
- write a manifest;
- verify the manifest points to the exact ledger range exported.

## High-Value Dataset Types

- agent action -> verifier result;
- failed action -> remediation;
- task -> required source-of-truth checks;
- tool schema -> valid invocation;
- policy context -> allowed or denied tool use;
- before/after state -> expected delta;
- incident packet -> root cause label.

## Long-Term Moat

A corpus of verified operational episodes is difficult to copy because it comes
from real tool use, real source-of-truth readback, and real failures. This can
become the data layer for agents that understand consequences.

