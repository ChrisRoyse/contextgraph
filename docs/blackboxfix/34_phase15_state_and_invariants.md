# Phase 15 State And Invariants

Phase 15 ships pilot packaging and buyer proof. It turns verified ledger state
from earlier phases into a package report, usage-meter JSONL, eval-candidate
JSONL, and a durable `pilot_package` ledger row.

## Shipped Surface

Command:

```bash
blackbox package pilot-report \
  --ledger PATH --tenant TENANT --principal-id PRINCIPAL \
  --pilot-id PILOT --min-actions N --out DIR [--force] \
  --agent AGENT [--model MODEL]
```

Connector:

- `blackbox_packaging_v1`

Ledger record kind:

- `pilot_package`

Phase 15 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 9`. The
physical SQLite table shape is unchanged, but old schema-version ledgers are
wiped on `blackbox init`; there is no migration shim.

## Source Of Truth

Primary source of truth:

- SQLite `ledger_records` rows in the user-provided ledger;
- package files under `--out`:
  `pilot-report.json`, `usage-meter-events.jsonl`,
  `eval-candidates.jsonl`, and `manifest.json`;
- dashboard package API file `api/pilot-packages.json`.

Before a package row is appended, the command:

1. Validates ledger, tenant, principal, pilot id, `--min-actions`, output
   directory, and actor fields.
2. Opens the initialized SQLite ledger.
3. Verifies table shape, metadata, append-only triggers, global hash chain,
   and per-run hash chains.
4. Persists and reads back an `enterprise_access_decision` for
   `commercial.package`.
5. Fails as `PILOT_PACKAGE_RBAC_DENIED` if the principal lacks permission; no
   package row/files are created.
6. Builds usage and pilot-success metrics from physical tenant rows,
   excluding previous `pilot_package` rows.
7. Writes package files to a temporary directory.
8. Reads every JSON file back and parses every JSONL line.
9. Appends one `pilot_package` row.
10. Reads the appended row back by sequence, `pilot_package_id`,
    `payload_sha256`, and `record_sha256`.
11. Promotes the directory and verifies final manifest file hashes and byte
    counts.

## Invariants

- `--min-actions` must be at least 1.
- `--pilot-id` must be a non-empty atom with only safe identifier characters.
- Output directories are created only after validation and authorization; an
  existing output path requires `--force`.
- Usage-meter rows include `event_name`, `customer_identifier`, numeric
  `value`, `timestamp`, stable `identifier`, dimensions, and source refs.
- JSONL files contain one valid JSON object per line and no blank lines.
- Pilot success requires enough real actions, complete tool-call evidence
  coverage, at least one eval candidate, and at least one release gate.
- Unmet criteria persist package files and a `pilot_package` row, then exit
  non-zero with `PILOT_PACKAGE_CRITERIA_NOT_MET`.
- Invalid request fields create no rows and no files.
- Unauthorized principals persist only a denied `enterprise_access_decision`
  and create no package row/files.

## Error Codes

| Code | Meaning |
|---|---|
| `PILOT_PACKAGE_MIN_ACTIONS_INVALID` | `--min-actions` is zero |
| `PILOT_PACKAGE_REQUEST_INVALID` | tenant, principal, pilot id, agent, or output path is invalid |
| `PILOT_PACKAGE_RBAC_DENIED` | principal lacks `commercial.package` |
| `PILOT_PACKAGE_SCHEMA_INVALID` | a source row or generated package row is malformed |
| `PILOT_PACKAGE_JSON_READBACK_MISMATCH` | generated JSON did not read back exactly |
| `PILOT_PACKAGE_JSONL_SCHEMA_INVALID` | generated JSONL row format is wrong |
| `PILOT_PACKAGE_JSONL_READBACK_MISMATCH` | generated JSONL did not parse or count correctly |
| `PILOT_PACKAGE_MANIFEST_MISMATCH` | final file hash/byte count does not match manifest |
| `PILOT_PACKAGE_READBACK_MISMATCH` | appended package row could not be read back by id/hash |
| `PILOT_PACKAGE_CRITERIA_NOT_MET` | durable package exists, but pilot success criteria failed |

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase15_pilot_package_fsv -- --nocapture
```

The test uses a real SQLite ledger, real enterprise config JSON, real SCIM
directory JSON, a real local Git repository, and real CLI commands. It
verifies:

- successful package persists one `pilot_package` with
  `pilot_success.status=passed`;
- package manifest hashes and byte counts match physical files;
- usage and eval JSONL line counts match the manifest;
- unmet `--min-actions` persists durable evidence and exits
  `PILOT_PACKAGE_CRITERIA_NOT_MET`;
- unauthorized principal creates no package row/files and does persist a
  denied access decision;
- invalid `--min-actions 0` creates no rows/files;
- connector test reads the physical `pilot_package` count;
- dashboard build reads physical package rows into `api/pilot-packages.json`
  and groups failed pilot criteria in `api/errors.json`.

## Manual FSV Checklist

For manual verification, print before/after direct SQLite counts for each
case:

```sql
SELECT record_kind, COUNT(*)
FROM ledger_records
GROUP BY record_kind
ORDER BY record_kind;
```

Then inspect the latest package row:

```sql
SELECT sequence,
       json_extract(payload_json, '$.pilot_package_id'),
       json_extract(payload_json, '$.pilot_success.status'),
       json_extract(payload_json, '$.pricing_meter_summary.events_total')
FROM ledger_records
WHERE record_kind='pilot_package'
ORDER BY sequence DESC
LIMIT 1;
```

Package file proof must include:

```bash
sha256sum pilot-package/manifest.json
sha256sum pilot-package/pilot-report.json
sha256sum pilot-package/usage-meter-events.jsonl
sha256sum pilot-package/eval-candidates.jsonl
```

The expected happy-path final state contains two real `agent_run` rows, two
verified `tool_call` rows, at least one failed/incident eval source, one
`release_gate`, and one passed `pilot_package` row. Edge cases must include
unmet criteria, unauthorized principal, and invalid minimum actions.

## Manual FSV Evidence 2026-05-03

Source of truth:

- `sqlite:/tmp/bbx-phase15-manual-fsv.QTDxYH/blackbox.sqlite#ledger_records`
- package files under `/tmp/bbx-phase15-manual-fsv.QTDxYH/pilot-package`
- dashboard files under `/tmp/bbx-phase15-manual-fsv.QTDxYH/dashboard`
- log: `/tmp/bbx-phase15-manual-fsv.QTDxYH/fsv.log`
- log SHA-256:
  `4ef2c4f5099cf48c759fdc6f74a2c0ccf2b50eba914ce16a7d582cd3ba35bf75`

Happy path direct readback:

- before: `pilot_package=0`
- after: `pilot_package=1`, `enterprise_access_decision=3`
- latest `pilot_package` row:
  `sequence=24`, `status=passed`, `usage_events_total=8`
- latest payload SHA-256:
  `sha256:6165be338a9defd645d6a5915ca920490bd31f505134d87786de08b35d52adae`
- latest record SHA-256:
  `sha256:aef75eb8b199bfc435c89d3672a16bfcf10593c9d6782469ccc1d723a21d7567`
- file hashes:
  `manifest.json=aeb33c245fbadcff582a9b6327d277294904cd8d903f04c0466e9ff228bbb86b`,
  `pilot-report.json=35ec9068e82eed2d1007dfc39b11ee63683db69eced88e32b34e1961627f65df`,
  `usage-meter-events.jsonl=1ca5e7ee5cd436811386c0988d7975f3a695fe368ad2fb2c58514845e4885ed2`,
  `eval-candidates.jsonl=776401c599a75234df5fb8f0e81f501b6b937ce6a12a60a02bca327225a7a5da`

Boundary and edge case direct readback:

- unmet criteria: before `pilot_package=1`, after `pilot_package=2`; command
  exited `PILOT_PACKAGE_CRITERIA_NOT_MET`; latest row status was
  `needs_more_evidence` with `usage_events_total=8`
- unauthorized principal: before `pilot_package=2`, `enterprise_access_decision=4`;
  after `pilot_package=2`, `enterprise_access_decision=5`; command exited
  `PILOT_PACKAGE_RBAC_DENIED`; output directory did not exist
- invalid `--min-actions 0`: before `total=27`, `pilot_package=2`,
  `enterprise_access_decision=5`; after values were unchanged; command exited
  `PILOT_PACKAGE_MIN_ACTIONS_INVALID`; output directory did not exist

Final physical ledger counts:

```text
agent_run=2
enterprise_access_decision=5
enterprise_directory_snapshot=1
enterprise_tenant_config=1
evidence_packet=2
expected_delta=2
incident_packet=1
pilot_package=2
policy_decision=2
readback_snapshot=2
release_gate=1
retention_decision=1
scorecard=1
tool_call=2
verifier_result=2
```

Dashboard readback:

- `api/pilot-packages.json` contained two physical package rows with statuses
  `passed` and `needs_more_evidence`
- `api/errors.json` contained `GIT_WORKING_TREE_DIRTY` and
  `PILOT_MIN_ACTIONS_NOT_MET`, and did not include successful
  `RELEASE_GATE_APPROVED` or `RETENTION_POLICY_APPLIED` reason codes
- dashboard manifest SHA-256:
  `4fb6ce033cc6ee0e808cef013436961d2b4712c227e870bb060da9815078ef99`
