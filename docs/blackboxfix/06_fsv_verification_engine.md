# FSV Verification Engine

The verifier is the core product. It converts "the agent said it worked" into
"the source of truth proves it worked" or a typed failure.

## Full-State Verification Protocol

Every mutating action follows this protocol:

1. Define the source of truth.
2. Print and persist the before-state.
3. Execute the trigger action.
4. Read the source of truth through an independent read path.
5. Decode and normalize the observed state.
6. Compare expected persisted delta to actual readback.
7. Emit `VerifierResult`.
8. Persist an `EvidencePacket`.
9. Export the action as training/eval data only with verifier status attached.

## Source Of Truth

The source of truth is the durable system where the action's final state lives.
Examples:

- Git object database for commits;
- GitHub API for pull request state;
- CI provider API for workflow status;
- database tables for application records;
- Kubernetes API for cluster objects;
- Terraform state plus cloud API for infrastructure;
- ticketing API for issue state.

Return values, logs, stdout, model text, and tool summaries can be evidence, but
they are not the source of truth unless the workflow's durable state is actually
that log/file.

## Verifier Registry

Verifiers are versioned specs:

```json
{
  "verifier_id": "github_pr_open_v1",
  "source_kind": "github",
  "input_schema": "schemas/github_pr_open_input_v1.json",
  "expected_delta_schema": "schemas/github_pr_open_delta_v1.json",
  "readback_schema": "schemas/github_pr_readback_v1.json",
  "comparison": "exact_fields",
  "required_fields": ["repo", "pr_number", "head_ref", "state"],
  "edge_cases": [
    "missing_pr",
    "wrong_head_ref",
    "closed_pr"
  ]
}
```

## Status Semantics

- `passed`: readback proves the expected delta.
- `failed`: readback completed and contradicts the expected delta.
- `blocked`: verifier cannot run because a required precondition failed.
- `not_applicable`: action is read-only or intentionally has no persisted delta.

`blocked` must include a concrete error code:

- `CONNECTOR_PERMISSION_DENIED`
- `SOURCE_OF_TRUTH_UNAVAILABLE`
- `READBACK_SCHEMA_DRIFT`
- `VERIFIER_SPEC_MISSING`
- `REDACTION_POLICY_VIOLATION`
- `EVIDENCE_WRITE_FAILED`

## Comparison Modes

Exact field match:

- expected value must equal readback value.

Set membership:

- expected item must exist in source-of-truth collection.

Count delta:

- after count minus before count must equal expected delta.

Hash match:

- artifact hash must equal expected hash.

Predicate:

- verifier-defined predicate over normalized readback.

Temporal invariant:

- a condition must hold within a bounded window, with all polling attempts
  recorded.

## Edge-Case Requirement

Every mutating verifier must define and test at least three edge cases. Example
for `github_pr_open_v1`:

- PR missing: expected `GITHUB_PR_NOT_FOUND`.
- Wrong branch: expected `GITHUB_PR_HEAD_REF_MISMATCH`.
- Closed PR: expected `GITHUB_PR_STATE_MISMATCH`.

For each edge case, the evidence must show:

- before-state;
- action attempted;
- readback state;
- typed error;
- remediation.

## Evidence Packet

```json
{
  "evidence_packet_id": "ev_...",
  "run_id": "run_...",
  "tool_call_id": "tool_...",
  "verifier_result_id": "verify_...",
  "source_of_truth": [
    {
      "kind": "github",
      "locator": "github:org/service/pull/123",
      "read_at": "2026-05-02T00:00:15Z",
      "state_hash": "sha256:..."
    }
  ],
  "before_state_hash": "sha256:...",
  "after_state_hash": "sha256:...",
  "verdict": "passed"
}
```

## Robust Logging

Every verifier failure should answer:

- Which source of truth failed?
- Which connector operation failed?
- Which field was expected?
- What did readback observe?
- Was this a policy block, execution failure, or outcome mismatch?
- What action should a human or agent take next?

## Training Signal

Verifier output becomes supervision:

- `passed` examples teach correct tool use.
- `failed` examples teach outcome mismatch detection.
- `blocked` examples teach missing preconditions and governance limits.
- Remediation fields teach recovery actions.

