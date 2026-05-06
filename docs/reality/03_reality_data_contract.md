# Reality Data Contract

This contract defines the records ContextGraph must capture so a model can be reprompted, benchmarked, trained, and governed from verified outcomes.

## Required Identifiers

Every record must carry:

- `run_id`
- `attempt_id`
- `model_id`
- `task_id`
- `trace_id`
- `tool_call_id` when a tool is involved
- `source_of_truth`
- `created_at`
- `payload_sha256`
- `record_sha256` when persisted to BlackBoxFix

## Model IO

```json
{
  "record_kind": "model_io",
  "run_id": "run_...",
  "attempt_id": "attempt_001",
  "model": {
    "provider": "contextgraph_rust_gemma_coding_harness",
    "name": "gemma4:e4b",
    "params": {
      "temperature": 0,
      "max_new_tokens": 4096
    }
  },
  "input": {
    "prompt_sha256": "sha256:...",
    "redacted_preview": "Repair SWE-bench issue ...",
    "raw_ref": null
  },
  "output": {
    "output_sha256": "sha256:...",
    "redacted_preview": "{\"patch\":\"diff --git ...\"}",
    "raw_ref": null
  },
  "token_usage": {
    "prompt_tokens": 0,
    "completion_tokens": 0
  }
}
```

Raw prompt/output text should not be copied into every export. Use hashes and redacted previews by default, with encrypted object refs only when governance permits.

## Action Proposal

```json
{
  "record_kind": "action_proposal",
  "run_id": "run_...",
  "attempt_id": "attempt_001",
  "tool_name": "apply_candidate_patch",
  "arguments_sha256": "sha256:...",
  "argument_status": "schema_valid",
  "expected_persisted_delta": {
    "git.diff": "applies",
    "tests.FAIL_TO_PASS": "passed"
  }
}
```

## Policy Decision

```json
{
  "record_kind": "policy_decision",
  "status": "allowed",
  "mode": "bounded_autonomous",
  "capability_id": "contextgraph.swe_lite.apply_and_test",
  "risk_tier": "high",
  "mutating": true,
  "verifier_coverage": "present"
}
```

If policy cannot be evaluated, the action must not run.

## Execution Result

```json
{
  "record_kind": "execution_result",
  "status": "completed",
  "command": ["git", "apply", "candidate.patch"],
  "exit_code": 0,
  "stdout_sha256": "sha256:...",
  "stderr_sha256": "sha256:...",
  "elapsed_ms": 120
}
```

Execution result is not proof of success.

## Source-of-Truth Readback

```json
{
  "record_kind": "readback_snapshot",
  "source_of_truth": "file:/tmp/run/tasks/x/attempt-001/validation.json",
  "actual_readback": {
    "validation_status": "failed",
    "error_code": "SWE_FAIL_TO_PASS_TESTS_FAILED",
    "failed_tests": ["test_requests.py::TestTimeout::test_encoded_methods"],
    "artifact_sha256": "sha256:..."
  }
}
```

This is the record that determines reality.

## Verifier Result

```json
{
  "record_kind": "verifier_result",
  "status": "failed",
  "verifier_id": "cmd_exit_zero_v1",
  "expected_persisted_delta": {},
  "actual_readback": {},
  "score": {
    "correctness": 0,
    "risk": 1,
    "confidence": 0.99
  }
}
```

## Reward Compilation

```json
{
  "record_kind": "reward_compilation",
  "reward": {
    "raw": -3.515,
    "clipped": -3.515,
    "loss": 3.515,
    "class": "negative"
  },
  "components": [
    {
      "id": "incident_penalty",
      "feature": "incident_present",
      "value": 1,
      "weight": -3,
      "contribution": -3
    }
  ]
}
```

`loss = -reward` is expected for optimization systems that minimize loss and maximize reward.

## Prompt Compiler Input

```json
{
  "attempt": 2,
  "previous_attempt": {
    "patch_sha256": "sha256:...",
    "status": "official_unresolved",
    "failed_tests": ["..."],
    "incident_ids": ["..."],
    "reward": -3.515
  },
  "repository_context": {
    "source_of_truth": "git:/tmp/workspace#base_commit",
    "files": []
  },
  "instructions": {
    "do_not_repeat_patch_sha256": ["sha256:..."],
    "success_condition": "official_report.resolved_ids contains instance_id"
  }
}
```

## Training Episode

```json
{
  "record_kind": "training_episode",
  "sample_kind": "verified_repair_attempt",
  "input": {
    "prompt_sha256": "sha256:...",
    "evidence_context_sha256": "sha256:..."
  },
  "action": {
    "patch_sha256": "sha256:..."
  },
  "outcome": {
    "verifier_status": "failed",
    "official_resolved": false,
    "reward": -3.515
  },
  "preference_label": {
    "preferred_over": []
  }
}
```

## Non-Negotiable Rules

- No mock source-of-truth rows.
- No success from return values alone.
- No skipped verifiers counted as success.
- No raw secrets in default exports.
- No reward compilation without a source scorecard.
- No promotion when source-of-truth coverage is incomplete.
