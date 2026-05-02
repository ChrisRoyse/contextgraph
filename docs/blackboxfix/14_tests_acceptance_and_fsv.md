# Tests, Acceptance, And Full-State Verification

Testing must prove the system works by inspecting durable state after execution.
Unit tests are useful, but they are not enough for this product.

## Source Of Truth For The Platform

For BlackBoxFix itself, sources of truth are:

- ledger database;
- evidence artifact store;
- hash-chain verification output;
- external systems under connector tests;
- exported JSONL and manifests.

## Test Levels

### Unit Tests

- schema validation;
- redaction;
- comparison functions;
- score calculation;
- error-code mapping.

### Integration Tests

- connector readback against real local systems;
- ledger write and readback;
- evidence artifact hash verification;
- exporter parse and manifest verification.

### Manual FSV

For each mutating workflow:

1. Print before-state from source of truth.
2. Execute action.
3. Print after-state from source of truth through a separate read.
4. Verify expected persisted delta.
5. Print evidence packet ids and ledger rows.
6. Run at least three edge cases.

## Acceptance Criteria

The MVP is accepted only when:

- every mutating action has a verifier or is blocked in governed mode;
- every verifier has happy path and three edge cases;
- every failure has error code and remediation;
- every evidence packet can be read back from storage;
- every exported JSONL row links to evidence;
- hash-chain tampering is detected;
- redaction failures stop persistence;
- connector permission failures are not reported as success.

## Manual FSV Template

```text
FSV: github_pr_open_v1
Source of truth: GitHub API pull request record

Before:
  PR exists: false
  branch exists: true

Trigger:
  verified_github_create_pr

After readback:
  PR exists: true
  PR state: open
  head_ref: agent/fix-cve

Expected delta:
  PR exists: true
  PR state: open
  head_ref: agent/fix-cve

Verifier:
  status: passed
  evidence_packet_id: ev_123
```

## Required Edge Cases

Every verifier defines at least three:

- missing target state;
- wrong target state;
- permission or schema failure.

Additional useful edge cases:

- stale readback;
- partial write;
- duplicate action;
- rate limit;
- redaction violation;
- artifact hash mismatch;
- tampered ledger record.

## Real Data Rule

Tests should use real local repositories, real temporary databases, real files,
real command output, and real connector sandboxes. No test should pass by
asserting a fabricated success response while the source of truth is broken.

For hosted systems, use controlled test repositories and sandboxes. If a live
external service is not available in CI, run those tests as gated integration
tests and keep local source-of-truth tests mandatory.

## Regression Evals

Every production incident and every new verifier edge case should produce:

- a ledger fixture;
- an episode row;
- an eval case;
- expected verifier status;
- expected remediation.

