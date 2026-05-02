# Replay, Forensics, And Incidents

Replay is how users understand what happened after an agent acted. The platform
must support both deterministic replay and forensic replay.

## Replay Modes

### Forensic Replay

Forensic replay reconstructs the run from immutable records:

- prompts and model decisions, redacted;
- policy decisions;
- tool calls;
- source-of-truth snapshots;
- verifier results;
- scores;
- incident packets.

It does not mutate production systems again.

### Deterministic Replay

Deterministic replay re-executes an action in a controlled environment when all
inputs, dependencies, and side effects can be pinned.

Good candidates:

- local file transforms;
- repository diffs;
- test commands;
- static analysis;
- dataset export.

Poor candidates:

- production cloud changes;
- ticket updates;
- external SaaS calls with changing state;
- irreversible actions.

For these, replay should use captured evidence and readback snapshots.

## Incident Packet

An incident packet is created when:

- verifier status is `failed` on a high-risk action;
- verifier status is `blocked` in governed mode;
- policy denies an attempted action;
- the agent accesses a sensitive resource unexpectedly;
- source-of-truth state diverges after a prior pass;
- replay detects tampering or hash-chain breakage.

```json
{
  "incident_packet_id": "inc_...",
  "severity": "high",
  "run_id": "run_...",
  "trigger": "verifier_failed",
  "error_code": "CI_REQUIRED_CHECK_FAILED",
  "timeline": ["rec_1", "rec_2", "rec_3"],
  "source_of_truth": ["github:org/service/pull/123"],
  "blast_radius": {
    "repos": ["org/service"],
    "tickets": ["SEC-123"],
    "deployments": []
  },
  "remediation": "revert commit or rerun agent with failing test output as input"
}
```

## Timeline View

The UI should show:

- task request;
- model decision;
- tool call;
- policy decision;
- expected delta;
- readback;
- verifier status;
- score;
- remediation.

The timeline should let a user filter by:

- failed verifier;
- high risk;
- sensitive data touched;
- unapproved tool;
- source of truth;
- connector.

## Replay Output

Replay should produce a stable, exportable report:

```text
Run: run_123
Task: remediate CVE in org/service
Status: failed
Failure: CI_REQUIRED_CHECK_FAILED

Step 4: github_create_pull_request
Expected: PR open and CI passing
Readback: PR open, unit-tests failed
Source: github:org/service/actions/runs/456
Remediation: inspect test artifact artifact_789
```

## Root-Cause Workflow

1. Identify first failing verifier in the run.
2. Inspect source-of-truth readback.
3. Compare to expected delta.
4. Determine whether failure is policy, connector, execution, or agent plan.
5. Create remediation task.
6. Export failed episode to eval corpus.
7. Add or update regression eval.

## Tamper Evidence

The ledger should detect:

- missing records;
- modified payloads;
- out-of-order records;
- evidence artifact hash mismatch;
- broken run hash chain;
- tenant chain discontinuity.

Tamper evidence should become an incident packet because governance value
depends on audit integrity.

