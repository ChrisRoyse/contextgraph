# Phase 23 — OpenTelemetry Export State And Invariants

Phase 23 ships `blackbox export otel`, an export-only path that converts a
verified SQLite ledger into physical OpenTelemetry-compatible JSONL files. The
ledger remains the authoritative evidence source; the export directory is the
artifact source of truth for observability integration.

## Shipped Surface

- Command: `blackbox export otel --ledger PATH --out DIR [--force]`.
- Connector: `blackbox_otel_export_v1`.
- Connector test:
  `blackbox connector test --connector blackbox_otel_export_v1 --source DIR`.
- Output files:
  - `traces.jsonl`
  - `logs.jsonl`
  - `metrics.jsonl`
  - `manifest.json`

Phase 23 adds no new record kind and does not bump `LEDGER_SCHEMA_VERSION`;
current ledgers remain version `13`.

## Sources Of Truth

- Input ledger: `sqlite:/path/to/blackbox.sqlite#ledger_records`.
- Export directory: `file:/path/to/export`.
- Trace file: `file:/path/to/export/traces.jsonl`.
- Log file: `file:/path/to/export/logs.jsonl`.
- Metric file: `file:/path/to/export/metrics.jsonl`.
- Manifest file: `file:/path/to/export/manifest.json`.

Return values are not evidence. Successful export requires a separate read of
the SQLite ledger and a separate read of every exported file, including
manifest hash verification against the actual file bytes.

## Invariants

- The exporter verifies SQLite hash-chain integrity before writing files.
- Empty ledgers fail with `OTEL_EXPORT_EMPTY_LEDGER`.
- Existing output directories fail with `OTEL_OUTPUT_EXISTS` unless
  `--force` is explicit.
- Exports are written to a temp directory, read back, promoted, and read back
  again after promotion.
- `traces.jsonl` contains exactly one top-level OTLP key: `resourceSpans`.
- `logs.jsonl` contains exactly one top-level OTLP key: `resourceLogs`.
- `metrics.jsonl` contains exactly one top-level OTLP key: `resourceMetrics`.
- Each signal file must contain at least one nested signal item.
- `manifest.json` records source-ledger count, schema version, ledger id, tip
  hash, file hashes, byte counts, and item counts.
- The connector rejects manifest/file hash drift, mixed-signal JSONL, missing
  files, and malformed JSONL with typed errors.
- Export does not append ledger rows.

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase23_otel_export_fsv -- --nocapture
```

The test creates a real SQLite ledger, records a run/tool call, runs a passed
and failed `cmd_exit_zero_v1` verifier, exports OTLP JSONL files, reads the
files and manifest back from disk, recomputes SHA-256 hashes, and queries
SQLite directly.

Expected happy-path ledger counts before and after export:

```text
agent_run=1
tool_call=1
expected_delta=2
readback_snapshot=2
verifier_result=2
evidence_packet=2
incident_packet=1
```

Expected exported file proof:

```text
traces.jsonl item_count == physical ledger row count
logs.jsonl item_count >= 3
metrics.jsonl item_count == 4
manifest.files[].sha256 == sha256(actual file bytes)
```

Covered paths:

- happy export and connector readback;
- empty initialized ledger (`OTEL_EXPORT_EMPTY_LEDGER`);
- existing output without `--force` (`OTEL_OUTPUT_EXISTS`);
- tampered ledger blocked before file creation (`BBX_HASH_CHAIN_BROKEN`).

## Manual FSV Checklist

1. Create a fresh ledger with `blackbox init`.
2. Record a run and tool call.
3. Run one passing and one failing verifier to create durable evidence and an
   incident.
4. Query `ledger_records` directly and print record-kind counts.
5. Run `blackbox export otel --ledger PATH --out DIR`.
6. Query `ledger_records` directly again and confirm counts did not change.
7. Read `traces.jsonl`, `logs.jsonl`, `metrics.jsonl`, and `manifest.json`
   directly from disk.
8. Recompute SHA-256 for each JSONL file and compare it to
   `manifest.files[].sha256`.
9. Count spans/log records/metrics from the parsed JSONL files and compare
   them to `manifest.counts`.
10. Run `blackbox connector test --connector blackbox_otel_export_v1 --source DIR`.
11. Run the three edge cases and print before/after SQLite/file state.
