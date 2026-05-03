# Phase 5 — State, Invariants, And Episode Export FSV Tests

This document is the canonical reference for the **shipped** Phase 5
implementation.

## Source Of Truth

| Surface | Physical source of truth |
|---|---|
| Ledger | SQLite file at `--ledger PATH`, table `ledger_records` |
| Episode rows | `episodes.jsonl` under `--out` |
| Export manifest | `manifest.json` under `--out` |
| Eval fixtures | JSON files under `--out/evals/` |
| Evidence linkage | physical `ledger_records` rows with matching `evidence_packet_id` |

Phase 5 did not change the live ledger schema. Current ledger records now use
`LEDGER_SCHEMA_VERSION = 8` after Phase 14; exported rows still add
`episode_schema_version = 1`.

## Shipped Command

```text
blackbox export episodes --ledger PATH --out DIR [--force]
```

The command verifies ledger integrity, rejects unverified tool calls, validates
all evidence packet row references, writes a temporary export directory,
re-reads every JSONL/manifest/eval file, and promotes the directory only after
readback succeeds.

## Output Files

```text
episodes.jsonl
manifest.json
evals/*.json
```

`episodes.jsonl` contains `verified_agent_episode` rows and
`incident_agent_episode` rows. Every row contains
`source.evidence_packet_id`, `source.ledger_sequence_range`, and
`source.ledger_tip_sha256_at_export`.

`manifest.json` has format `blackbox_episodes_manifest_json_v1`, records the
exported ledger sequence range, split counts, JSONL byte count, JSONL SHA-256,
and eval fixture hashes.

## Automated FSV

Run:

```bash
cargo test --package context-graph-cli --test blackbox_phase5_export_fsv -- --nocapture
```

Current suite: 4/4 passing. Coverage includes:

- happy-path export from a real SQLite ledger with one passed verifier and one
  failed verifier;
- direct JSONL parse, manifest parse, SHA comparison, eval fixture parse, and
  physical SQLite evidence id checks;
- existing output directory without `--force` fails with `BBX_OUTPUT_EXISTS`;
- tampered ledger blocks with `BBX_HASH_CHAIN_BROKEN` before writing files;
- unverified tool call blocks with
  `BBX_VERIFIER_MISSING_FOR_MUTATING_ACTION` before writing files.

## Manual FSV Evidence

Manual FSV completed on 2026-05-02:

```text
Log: /tmp/bbx-phase5-manual-fsv.exRXYj/fsv.log
Ledger source of truth: /tmp/bbx-phase5-manual-fsv.exRXYj/blackbox.sqlite#ledger_records
Export source of truth: /tmp/bbx-phase5-manual-fsv.exRXYj/episodes-export/
Git source of truth: /tmp/bbx-phase5-manual-fsv.exRXYj/repo/.git + working tree
Final record counts: agent_run=1, tool_call=1, expected_delta=2,
readback_snapshot=2, verifier_result=2, evidence_packet=2, incident_packet=1
Export rows: verified_agent_episode=2, incident_agent_episode=1
Splits: passed=1, failed=1, incident=1, blocked=0, edge_case=0
Eval fixtures: 2
Export SHA: sha256:c0c4d0a76db8ed18ba2c708208f6556de5040a9854ca51a1cf44f069c53afcda
```

Happy-path physical proofs:

- `episodes.jsonl`, `manifest.json`, and `evals/` existed after export.
- Direct JSONL readback parsed 3 rows.
- Direct manifest readback reported
  `format=blackbox_episodes_manifest_json_v1`.
- Direct SHA-256 over `episodes.jsonl` matched
  `manifest.output.episodes_sha256`.
- Every row's `source.evidence_packet_id` matched exactly one physical
  `ledger_records` row with `record_kind='evidence_packet'`.
- Ledger row count before and after export remained `11`.

Manual edge cases:

1. Existing output directory: before files `marker.txt`; after files
   `marker.txt`; stderr carried `BBX_OUTPUT_EXISTS`; ledger row count stayed
   `11`.
2. Unverified tool call: before counts `agent_run=1`, `tool_call=1`; after
   counts unchanged; output directory absent; stderr carried
   `BBX_VERIFIER_MISSING_FOR_MUTATING_ACTION`.
3. Tampered ledger: before row count `11`; after row count `11`; output
   directory absent; stderr carried `BBX_HASH_CHAIN_BROKEN` with exit code `2`.

## Invariants

- No episode export may read from return values; it must read the physical
  SQLite ledger and verify the hash chain first.
- No mutating tool call may be exported unless it has evidence packet verifier
  output.
- No existing output directory may be overwritten unless `--force` is passed.
- No final output directory may appear until JSONL, manifest, and eval fixture
  temp files have been written, read back, parsed, and hashed.
- No JSONL row may omit `source.evidence_packet_id`; consumers must be able to
  re-open the ledger and verify the row's evidence packet.
