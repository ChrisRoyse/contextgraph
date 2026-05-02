# Phase 7 — Team Dashboard And FSV Tests

This document is the canonical reference for the **shipped** Phase 7
implementation.

## Source Of Truth

| Surface | Physical source of truth |
|---|---|
| Ledger input | SQLite file at `--ledger PATH`, table `ledger_records` |
| Dashboard bundle | output directory containing `index.html`, `manifest.json`, and `api/*.json` |
| Dashboard API | JSON files under `api/` plus `blackbox dashboard api` read models |
| File integrity | `manifest.json.files[]` sha256 and byte counts matched against physical file bytes |

Phase 7 does not bump `LEDGER_SCHEMA_VERSION`; the live ledger remains schema
4. Dashboard artifacts use `dashboard_schema_version = 1`.

## Shipped Commands

```text
blackbox dashboard build --ledger PATH --out DIR [--force]
blackbox dashboard api --ledger PATH --view summary|runs|timeline|evidence|verifiers|errors|exports [--run-id RUN_ID]
```

`dashboard build` writes:

```text
index.html
manifest.json
api/summary.json
api/runs.json
api/evidence.json
api/verifier-status.json
api/errors.json
api/exports.json
api/timeline_<run_id>.json
```

## Invariants

- The dashboard verifies the SQLite hash chain before reading data.
- The dashboard verifies `evidence_packet.record_refs[]` against physical
  ledger rows before writing files.
- Empty ledgers fail with `DASHBOARD_NO_RUNS`; no placeholder dashboard is
  created.
- Existing output directories fail with `BBX_OUTPUT_EXISTS` unless `--force`
  is explicit.
- Every JSON/HTML artifact is written to a temp directory, read back, then
  hashed before the final directory is promoted.
- After promotion, `manifest.json.files[]` is checked against actual final
  file bytes before success is returned.
- Dashboard build is read-only with respect to `ledger_records`.

## Automated FSV

Run:

```bash
cargo test --package context-graph-cli --test blackbox_phase7_dashboard_fsv -- --nocapture
```

Current suite: 5/5 passing. Coverage includes:

- happy path with a real SQLite ledger, real Git repo, one passed run, one
  failed run, generated dashboard files, manifest hash verification, and
  physical SQLite evidence-id checks;
- existing output directory fails before writing dashboard files;
- empty ledger fails before creating output;
- tampered ledger fails with `BBX_HASH_CHAIN_BROKEN`;
- missing timeline run id fails without mutating the ledger.

## Manual FSV

Manual FSV log:

```text
/tmp/bbx-phase7-manual-fsv.W4eqmG/fsv.log
```

Happy-path proof:

- source of truth:
  `/tmp/bbx-phase7-manual-fsv.W4eqmG/happy.sqlite#ledger_records` and
  `/tmp/bbx-phase7-manual-fsv.W4eqmG/dashboard`;
- before dashboard build: row count `13`, kind counts
  `agent_run=2`, `tool_call=2`, `expected_delta=2`,
  `readback_snapshot=2`, `verifier_result=2`, `evidence_packet=2`,
  `incident_packet=1`, dashboard directory absent;
- after dashboard build: row count stayed `13` and the same kind counts were
  present;
- dashboard files existed: `index.html`, `manifest.json`,
  `api/summary.json`, `api/runs.json`, `api/evidence.json`,
  `api/verifier-status.json`, `api/errors.json`, `api/exports.json`, and two
  `api/timeline_<run_id>.json` files;
- `api/runs.json` showed one `passed` run and one `failed` run;
- `index.html` included the timeline filter control (`timeline-kind` count
  `1`);
- every `api/evidence.json.evidence_packets[].evidence_packet_id` had
  physical SQLite count `1`;
- `api/errors.json` included `GIT_BRANCH_NOT_FOUND` with count `2` across the
  `verifier_result` and `incident_packet` rows;
- manifest hash readback matched actual file bytes:
  `api/summary.json` =
  `sha256:16788f6c494990ca77fc51cdd19e91d4480e5b03dbe1fba4b4a985bc16988776`,
  `index.html` =
  `sha256:554870febfa02a6ece9b45f4a4619cda32f120c3731bc82570d85f5eb796f930`.

Edge cases:

- Existing output directory: before row count `13`; command returned
  `BBX_OUTPUT_EXISTS`; after row count `13`, marker file still existed, and
  `manifest.json` was absent.
- Empty ledger: before row count `0`; command returned `DASHBOARD_NO_RUNS`;
  after row count `0`, output directory absent.
- Tampered ledger: before row count `13`; payload row was physically modified
  after dropping and restoring the append-only trigger; command returned
  `BBX_HASH_CHAIN_BROKEN` at
  `ledger_records[1].payload_sha256`; output directory absent.
