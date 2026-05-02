# API, CLI, MCP, And SDK Surface

BlackBoxFix should expose the same core semantics through CLI, API, SDK, and MCP
wrappers. The product surface should make the FSV lifecycle explicit.

## CLI

Proposed top-level command:

```text
blackbox <command>
```

Initial commands:

```text
blackbox init --ledger PATH
blackbox connector list
blackbox connector test --connector github --source org/repo
blackbox record run --task TASK --agent AGENT_ID
blackbox record tool-call --run-id RUN --tool TOOL --input FILE
blackbox verify --run-id RUN
blackbox replay --run-id RUN
blackbox inspect ledger --run-id RUN
blackbox export episodes --run-id RUN --out episodes.jsonl
blackbox export evidence --run-id RUN --out evidence.json
```

The existing `context-graph` CLI can host this initially as:

```text
context-graph blackbox ...
```

## CLI Failure Shape

Every failure should include:

- error code;
- source of truth;
- connector;
- field path;
- remediation;
- evidence id if written.

```json
{
  "status": "error",
  "error_code": "READBACK_SCHEMA_DRIFT",
  "source_of_truth": "github:org/service/pull/123",
  "connector": "github_v1",
  "field_path": "checks[0].conclusion",
  "remediation": "update github_check_suite_passed_v1 schema or connector parser"
}
```

## HTTP API

Core endpoints:

```text
POST /v1/runs
GET  /v1/runs/{run_id}
POST /v1/runs/{run_id}/steps
POST /v1/tool-calls
POST /v1/tool-calls/{tool_call_id}/verify
GET  /v1/tool-calls/{tool_call_id}/evidence
POST /v1/replay
POST /v1/exports/episodes
GET  /v1/connectors
POST /v1/connectors/{connector_id}/test
GET  /v1/policies
POST /v1/policies/evaluate
```

All write endpoints should return the created ledger ids. They should not claim
the action is correct unless verifier evidence exists.

## MCP Surface

MCP is strategically important because it gives agents a standard way to access
tools and context. BlackBoxFix should support two MCP patterns:

### Recorder MCP Server

The recorder exposes tools such as:

```text
blackbox_start_run
blackbox_record_decision
blackbox_record_tool_call
blackbox_verify_tool_call
blackbox_get_evidence
blackbox_export_episodes
```

### Gateway MCP Server

The gateway wraps external tools:

```text
verified_github_create_pr
verified_git_apply_patch
verified_ci_wait_for_checks
verified_ticket_update
verified_k8s_rollout
```

Each gateway tool has strict schemas and returns source-of-truth ids plus
verifier results.

## SDK

SDKs should start with Python and TypeScript:

```python
with blackbox.run(task="fix vulnerability", agent_id="agent_coder") as run:
    call = run.tool_call("git_apply_patch", input=patch)
    result = call.execute()
    evidence = call.verify(expected_delta=expected)
```

SDK constraints:

- all mutating calls require expected delta;
- all verifiers return typed result;
- recorder works without model-provider lock-in;
- redaction hooks run before persistence;
- source-of-truth readback is explicit.

## OpenTelemetry Export

OpenTelemetry should be an export path, not the primary source of truth. Emit:

- trace spans for run, step, tool call, verifier;
- logs for failures and remediation;
- metrics for pass rates, blocked actions, risk scores, latency;
- span attributes for connector, source kind, tool capability, risk tier.

The ledger remains authoritative.

## Versioning

- APIs are versioned by URL.
- Ledger records include `schema_version`.
- Verifier specs include `verifier_id` with version suffix.
- Connectors include `connector_id` with version suffix.
- Export manifests record all versions.

