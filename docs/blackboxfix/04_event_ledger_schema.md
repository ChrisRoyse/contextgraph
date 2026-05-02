# Event Ledger Schema

The ledger is the durable record of agent behavior and verifier evidence. It
must be append-only, schema-versioned, tenant-scoped, and hash-chained.

## Record Families

- `AgentRun`: one task or session.
- `AgentStep`: one reasoning/action step inside a run.
- `ModelDecision`: model output, tool choice, confidence, and policy context.
- `ToolCall`: validated invocation request and result envelope.
- `ExpectedDelta`: the durable state change the action is expected to produce.
- `ReadbackSnapshot`: independent source-of-truth state after execution.
- `VerifierResult`: comparison of expected delta and readback.
- `EvidencePacket`: exportable proof bundle.
- `PolicyDecision`: authorization and approval result.
- `IncidentPacket`: high-severity failure bundle.
- `EpisodeRecord`: training/eval export row.

## Common Record Envelope

```json
{
  "schema_version": 1,
  "record_id": "bbx_rec_...",
  "record_kind": "tool_call",
  "tenant_id": "tenant_...",
  "run_id": "run_...",
  "step_id": "step_...",
  "created_at": "2026-05-02T00:00:00Z",
  "actor": {
    "actor_kind": "ai_agent",
    "agent_id": "agent_...",
    "model_id": "provider/model",
    "human_approver_id": null
  },
  "hash": {
    "payload_sha256": "...",
    "previous_record_sha256": "...",
    "run_chain_sha256": "..."
  },
  "redaction": {
    "policy_id": "redact_v1",
    "redacted_fields": ["tool_input.token"],
    "secret_digest_fields": ["tool_input.api_key"]
  }
}
```

## AgentRun

```json
{
  "record_kind": "agent_run",
  "run_id": "run_...",
  "task": {
    "task_kind": "dependency_remediation",
    "requested_outcome": "upgrade vulnerable package and prove scanner passes",
    "risk_tier": "high"
  },
  "runtime": {
    "framework": "mcp",
    "adapter_version": "0.1.0"
  },
  "status": "completed_verified",
  "summary": {
    "steps_total": 8,
    "tool_calls_total": 5,
    "verifier_passed": 4,
    "verifier_failed": 0,
    "verifier_blocked": 0
  }
}
```

## ToolCall

```json
{
  "record_kind": "tool_call",
  "tool_call_id": "tool_...",
  "tool_name": "github_create_pull_request",
  "capability_id": "github.pull_request.write",
  "input_digest": "sha256:...",
  "input_redacted": {
    "repo": "org/service",
    "branch": "agent/fix-cve-2026-0001",
    "title": "Fix CVE-2026-0001"
  },
  "authorization": {
    "policy_decision_id": "policy_...",
    "mode": "supervised",
    "approved_by": "user_..."
  },
  "execution": {
    "started_at": "2026-05-02T00:00:00Z",
    "finished_at": "2026-05-02T00:00:10Z",
    "status": "ok",
    "external_request_id": "github-request-id"
  }
}
```

## ExpectedDelta

```json
{
  "record_kind": "expected_delta",
  "tool_call_id": "tool_...",
  "source_of_truth": [
    {
      "kind": "git",
      "locator": "github:org/service",
      "readback_spec_id": "git_commit_exists_v1"
    },
    {
      "kind": "github",
      "locator": "github:org/service/pull/123",
      "readback_spec_id": "github_pr_open_v1"
    }
  ],
  "delta": {
    "branch_exists": true,
    "commit_count_delta": 1,
    "pull_request_state": "open"
  }
}
```

## ReadbackSnapshot

```json
{
  "record_kind": "readback_snapshot",
  "readback_id": "readback_...",
  "tool_call_id": "tool_...",
  "source_of_truth": {
    "kind": "github",
    "locator": "github:org/service/pull/123"
  },
  "read_path": {
    "connector_id": "github_v1",
    "operation": "get_pull_request",
    "credential_key_id": "key_..."
  },
  "observed_state": {
    "state": "open",
    "head_ref": "agent/fix-cve-2026-0001",
    "commits": 1
  },
  "read_at": "2026-05-02T00:00:15Z"
}
```

## VerifierResult

```json
{
  "record_kind": "verifier_result",
  "verifier_result_id": "verify_...",
  "tool_call_id": "tool_...",
  "verifier_id": "github_pr_open_v1",
  "status": "passed",
  "expected_persisted_delta": {
    "pull_request_state": "open"
  },
  "actual_readback": {
    "pull_request_state": "open"
  },
  "score": {
    "correctness": 1.0,
    "risk": 0.18,
    "confidence": 0.99
  },
  "error_code": null,
  "remediation": null
}
```

## Typed Failure Example

```json
{
  "record_kind": "verifier_result",
  "status": "failed",
  "error_code": "GITHUB_PR_STATE_MISMATCH",
  "error_message": "expected PR state open, read closed",
  "field_path": "actual_readback.pull_request_state",
  "remediation": "reopen the PR or mark the agent run failed; do not export as a passed episode"
}
```

## Integrity Requirements

- Every record has a stable `record_id`.
- Every mutating tool call links to one or more expected deltas.
- Every expected delta links to one or more readback snapshots.
- Every verifier result links to the source-of-truth snapshots it used.
- Every evidence packet can be verified from ledger records and hashes.
- Every exported training row links back to immutable evidence.

