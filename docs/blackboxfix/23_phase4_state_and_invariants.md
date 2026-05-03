# Phase 4 — State, Invariants, And Replay FSV Tests

This document is the canonical reference for the **shipped** Phase 4
implementation.

## Source Of Truth

| Surface | Physical source of truth |
|---|---|
| Ledger | SQLite file at `--ledger PATH`, table `ledger_records` |
| Evidence packet verification | physical `ledger_records` rows referenced by `evidence_packet.record_refs[]` |
| Replay | immutable SQLite rows for one `run_id`; replay must not mutate rows |
| Export | JSON file at `--out`, read back from disk byte-for-byte |
| Git edge fixture | real `.git` object DB and refs read through the installed `git` binary |

Phase 4 shipped `LEDGER_SCHEMA_VERSION = 3`. The current live ledger schema is
`LEDGER_SCHEMA_VERSION = 7` after Phase 13. `blackbox init` removes older ledger
files (`.sqlite`, `-wal`, `-shm`) before initializing the current schema. There
is no migration shim.

## Shipped Record Kinds

Phase 4 adds:

```text
evidence_packet   exportable proof index over immutable ledger rows
incident_packet   high-severity failure bundle for failed verifiers
```

Every associated `blackbox verify` call appends an `evidence_packet`. A failed
verifier appends an `incident_packet` after the evidence packet, then returns
the typed verifier error on stderr.

## Shipped Commands

```text
blackbox replay          --ledger PATH --run-id RUN_ID
blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE
```

`blackbox replay` verifies ledger integrity, reconstructs the run timeline,
validates all evidence packet row references against physical SQLite rows, and
returns `mutates_source_of_truth=false`.

`blackbox export evidence` validates `--out`, builds the replay report, writes
`blackbox_evidence_report_json_v1`, reads it back from disk, byte-compares the
bytes, parses the JSON, and returns the export byte count and sha256.

## Automated FSV

Run:

```bash
cargo test --package context-graph-cli --test blackbox_phase4_fsv -- --nocapture
```

Current suite: 4/4 passing. Coverage includes:

- passed verifier writes an evidence packet and replay is read-only;
- failed verifier writes both an evidence packet and an incident packet;
- export writes a real JSON file and readback matches the written bytes;
- missing run and invalid export-path edge cases fail closed without mutating
  ledger state.

## Manual FSV Evidence

Manual FSV completed on 2026-05-02:

```text
Log: /tmp/bbx-phase4-manual-fsv.M8gObv/fsv.log
Ledger source of truth: /tmp/bbx-phase4-manual-fsv.M8gObv/blackbox.sqlite#ledger_records
Export source of truth: /tmp/bbx-phase4-manual-fsv.M8gObv/evidence-report.json
Git source of truth: /tmp/bbx-phase4-manual-fsv.M8gObv/repo/.git + working tree
Final record counts: agent_run=1, tool_call=1, expected_delta=2,
readback_snapshot=2, verifier_result=2, evidence_packet=2, incident_packet=1
```

Happy-path physical proofs:

- `git_branch_exists_v1` passed against a real local Git branch.
- The evidence packet existed in `ledger_records` at sequence `6`.
- Direct SQLite reads proved each `record_refs[]` entry matched the referenced
  row's `sequence`, `record_kind`, `payload_sha256`, and `record_sha256`.
- `blackbox replay` returned `mode=forensic`,
  `mutates_source_of_truth=false`, and
  `evidence_hash_verification.status=passed`.
- Ledger row count before and after replay remained `6`.
- `blackbox export evidence` wrote
  `/tmp/bbx-phase4-manual-fsv.M8gObv/evidence-report.json`; direct file
  readback parsed as `blackbox_evidence_report_json_v1` and reported
  `evidence_hash_verification.status=passed`.
- Ledger row count before and after export remained `6`.

Manual edge cases:

1. Missing branch: direct Git ref lookup failed; verifier persisted
   `status=failed`, `error_code=GIT_BRANCH_NOT_FOUND`, wrote a second
   `evidence_packet`, wrote one `incident_packet`, then returned non-zero.
2. Missing replay run: before row count `11`, after row count `11`; stderr
   carried `BBX_RECORD_NOT_FOUND`.
3. Export to directory: before row count `11`, after row count `11`; stderr
   carried `BBX_EXPORT_PATH_INVALID` with connector `fs_local_v1`.

## Invariants

- No replay path may mutate the ledger or an external source of truth.
- No export path may return success before reading the written file back from
  disk, byte-comparing it, and parsing it as JSON.
- No evidence packet may be accepted if any referenced row is missing or if
  `record_kind`, `payload_sha256`, or `record_sha256` differs from the
  physical SQLite row.
- No failed verifier may return before durable evidence and incident rows are
  appended, when the verifier can be associated with a run/tool call.
- No filesystem export-path error may be reported as a SQLite connector error.
