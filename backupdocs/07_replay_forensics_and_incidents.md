# Replay, Forensics, And Incidents

Replay is how users understand what happened after an agent acted. The
platform supports forensic replay in Phase 4 and may add deterministic replay
for tightly pinned local workflows later.

Phase 1 provides the primitive forensic readback: `blackbox inspect ledger`
returns the full per-record timeline (sequence, kind, hashes, redaction
report, payload). Phase 4 ships richer evidence packets, incident packets,
read-only replay, and JSON evidence export.

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

## Shipped Phase 4 Commands

```text
blackbox replay          --ledger PATH --run-id RUN_ID
blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE
```

`blackbox replay` verifies ledger integrity, reconstructs a single run from
immutable rows, validates every `evidence_packet.record_refs[]` entry against
physical SQLite rows, and returns a forensic report. It does not mutate the
ledger or any external source of truth.

`blackbox export evidence` builds the same replay report, writes a JSON report
to `--out`, immediately reads the file back, byte-compares the readback, and
parses the JSON before returning success. Directory paths, missing parents,
readback mismatches, and invalid exported JSON fail closed.

## Incident Packet (Phase 4)

In the shipped Phase 4 implementation, an incident packet is created when:

- verifier status is `failed` on a high-risk action;

Later governed-mode phases may also create incidents when:

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
  "timeline": ["bbx_rec_1", "bbx_rec_2", "bbx_rec_3"],
  "source_of_truth": "github:org/service/pull/123",
  "blast_radius": {
    "repos": ["org/service"],
    "tickets": ["SEC-123"],
    "deployments": []
  },
  "remediation": "revert commit or rerun agent with failing test output as input"
}
```

## Timeline View

The Phase 7 dashboard UI shows:

- task request;
- model decision;
- tool call;
- policy decision;
- expected delta;
- readback;
- verifier status;
- score;
- remediation.

The shipped timeline supports filters by record kind, status, risk tier, and
source of truth. Connector and tool-specific drilldowns are exposed in
`api/verifier-status.json`; sensitive-data and approval filters become richer
once later phases add explicit data-classification and approval lifecycle
records.

## Replay Output (shipped)

Replay produces a stable, exportable JSON report. Example shape:

```json
{
  "mode": "forensic",
  "mutates_source_of_truth": false,
  "summary": {
    "run_id": "run_123",
    "status": "failed",
    "kinds": {
      "agent_run": 1,
      "tool_call": 1,
      "expected_delta": 2,
      "readback_snapshot": 2,
      "verifier_result": 2,
      "evidence_packet": 2,
      "incident_packet": 1
    },
    "first_failure": {
      "error_code": "CI_REQUIRED_CHECK_FAILED"
    }
  },
  "timeline": [],
  "incidents": [],
  "evidence_hash_verification": {
    "status": "passed"
  }
}
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
- evidence packet reference/hash mismatch;
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
