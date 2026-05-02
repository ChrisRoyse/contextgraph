# Replay, Forensics, And Incidents

Replay is how users understand what happened after an agent acted. The
platform must support both deterministic and forensic replay (Phase 4).

Phase 1 already provides a primitive forensic readback: `blackbox inspect
ledger` returns the full per-record timeline (sequence, kind, hashes,
redaction report, payload). Phase 4 layers richer evidence packets and
incident packets on top.

## Replay Modes

### Forensic Replay (default for production)

Forensic replay reconstructs the run from immutable records:

- prompts and model decisions, redacted;
- policy decisions;
- tool calls;
- source-of-truth snapshots;
- verifier results;
- scores;
- incident packets.

It does **not** mutate production systems again.

### Deterministic Replay

Deterministic replay re-executes an action in a controlled environment when all
inputs, dependencies, and side effects can be pinned.

Good candidates:

- local file transforms;
- repository diffs;
- test commands;
- static analysis;
- dataset export.

Poor candidates (must use forensic replay instead):

- production cloud changes;
- ticket updates;
- external SaaS calls with changing state;
- irreversible actions.

## Incident Packet (Phase 4)

An incident packet is created when:

- verifier status is `failed` on a high-risk action;
- verifier status is `blocked` in governed mode;
- policy denies an attempted action;
- the agent accesses a sensitive resource unexpectedly;
- source-of-truth state diverges after a prior pass;
- replay detects tampering or hash-chain breakage (already detectable in
  Phase 1 via `BBX_HASH_CHAIN_BROKEN`; Phase 4 promotes it to an incident
  packet record).

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

The UI (Phase 7) must show:

- task request;
- model decision;
- tool call;
- policy decision;
- expected delta;
- readback;
- verifier status;
- score;
- remediation.

The timeline must support filters by: failed verifier, high risk, sensitive
data touched, unapproved tool, source of truth, connector.

## Replay Output

Replay must produce a stable, exportable report. Example:

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

The ledger must detect:

- missing records (gap in `sequence`);
- modified payloads (`payload_sha256` mismatch);
- out-of-order records (`previous_record_sha256` mismatch);
- evidence artifact hash mismatch (Phase 4);
- broken run hash chain (`run_chain_sha256` mismatch);
- tenant chain discontinuity (Phase 8).

Phase 1 already detects the first three through the `verify_ledger_integrity`
sweep run on every `inspect ledger` and on every `append_record` (it is also
re-run inside the append transaction, so a corrupted ledger refuses further
writes).

## FSV Of This Section

To prove the tamper-evidence claim:

```bash
ledger=/tmp/bbx-tamper.sqlite
context-graph blackbox init --ledger "$ledger"
RUN=$(context-graph blackbox record run --ledger "$ledger" --task t --agent a | jq -r .run_id)
echo '{"file":"x","operation":"inspect"}' >/tmp/in.json
context-graph blackbox record tool-call --ledger "$ledger" --run-id "$RUN" --tool t --input /tmp/in.json
sqlite3 "$ledger" "DROP TRIGGER ledger_records_no_update;"
sqlite3 "$ledger" "UPDATE ledger_records SET payload_json='{\"x\":true}' WHERE sequence=2;"
context-graph blackbox inspect ledger --ledger "$ledger"; echo "exit=$?"
# exit must be 2; error_code must be BBX_HASH_CHAIN_BROKEN.
```

This test is also automated as
`crates/context-graph-cli/tests/blackbox_phase1_fsv.rs::phase1_hash_chain_detection_catches_payload_tampering`.
