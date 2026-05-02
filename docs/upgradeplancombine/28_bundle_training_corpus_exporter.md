# 28 — Bundle-to-Training-Corpus Exporter

This artifact converts a passed DynamicJEPA v2 bundle into JSON Lines
episodes for model training, verifier replay, and process-supervision
analysis.

## Command

```text
context-graph export dynamicjepa-episodes \
  --bundle <bundle-root> \
  --out <episodes.jsonl> \
  [--manifest-out <episodes.manifest.json>] \
  [--domain <domain-id>]... \
  [--no-edge-cases] \
  [--overwrite]
```

## Source Of Truth

The physical source of truth is the bundle's RocksDB directory, not command
return values:

- `run_manifest.json` must declare `bundle_schema_version = 2` and
  `status = passed`.
- `fsv_report.json` must declare `status = passed`.
- `db_counts_after.json` selects the canonical per-domain DBs.
- Each selected domain DB is opened with `create_if_missing=false` and
  `create_missing_column_families=false`.
- Every plan episode is independently re-read from `dj_plan_traces`,
  `dj_predictions`, `dj_guard_decisions`, and `dj_verification_runs`.
- Edge-case episodes require an existing evidence file and verified
  before/after count deltas.

`db_counts_after.json` is retained as provenance. If it has drifted from the
physical DB, the exporter records `manifest_count_drift` in the manifest and
CLI summary; it does not use stale manifest counts as the data source.

## JSONL Row Contract

Every line is one JSON object:

```json
{
  "schema_version": 1,
  "record_kind": "dynamicjepa_plan_episode",
  "episode_id": "career_taxonomy:plan:<uuid>",
  "bundle": {},
  "source_of_truth": {},
  "state": {},
  "action": {},
  "expected_persisted_delta": {},
  "actual_readback": {},
  "verifier_status": {},
  "error_code": null,
  "remediation": null
}
```

Plan rows carry linked predictions and guard decisions. Edge-case rows carry
the typed failure, expected persisted delta, actual before/after readback,
and remediation text.

## Failure Discipline

The exporter fails closed when:

- the bundle or DB path is missing;
- the bundle or FSV status is not `passed`;
- a selected domain has no plan trace or no passed verification run;
- any linked plan, prediction, or guard readback is missing or inconsistent;
- edge-case evidence is missing or the expected delta does not match the
  recorded before/after counts;
- output or manifest files already exist without `--overwrite`;
- JSONL readback fails after writing.

Output is written to a temporary file first, parsed back line-by-line, hashed,
then promoted to the requested path. The manifest is also written and parsed
before promotion.
