# 2026-04-30 O*NET ZIP Preflight FSV

## Scope

Continue with the verified O*NET 30.2 text ZIP and close the remaining ambiguity around O*NET availability, archive provenance, and fail-closed behavior.

## Source Of Truth

- Raw O*NET source ZIP: `tmp/onet_probe/db_30_2_text.zip`
- Raw ZIP SHA-256: `b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741`
- Final broad-smoke bundle: `tmp/5090jepa_evidence/research_smoke/test_runtime/test_research_smoke_46288`
- O*NET archive preflight evidence: `tmp/5090jepa_evidence/research_smoke/test_runtime/test_research_smoke_46288/onet_source_preflight.json`
- Bundle data manifest: `tmp/5090jepa_evidence/research_smoke/test_runtime/test_research_smoke_46288/data_manifest.json`
- Bundle FSV report: `tmp/5090jepa_evidence/research_smoke/test_runtime/test_research_smoke_46288/fsv_report.json`
- Career RocksDB: `tmp/5090jepa_evidence/research_smoke/test_runtime/test_research_smoke_46288/dbs/test_research_smoke_46288_career_taxonomy_rocksdb`

## Web Research

- Official O*NET database page confirms O*NET 30.2 is distributed in tab-delimited text files and documents the 30.2 release: https://www.onetcenter.org/database.html
- Official O*NET database license page confirms CC BY 4.0 attribution/non-endorsement requirements: https://www.onetcenter.org/license_db.html
- Rust `zip` crate docs warn against using raw archive member names for extraction and recommend `ZipFile::enclosed_name` for safe path validation: https://docs.rs/zip/latest/zip/read/struct.ZipFile.html
- OWASP path traversal guidance documents the broader risk class for attacker-controlled paths escaping intended roots: https://owasp.org/www-community/attacks/Path_Traversal

## Findings

The O*NET ZIP is not blocked. It exists locally, is readable, and is already used by passing smoke and paper-small bundles.

The remaining issue class was robustness around archive provenance: the workflow must prove the archive is the exact expected O*NET 30.2 text shape before generated fixtures, RocksDB rows, training, tables, or plots are trusted.

Current implementation already persists an O*NET preflight source-of-truth file with:

- safe archive member validation;
- required `db_30_2_text/` root;
- `O*NET 30.2 Database` release marker;
- CC BY license marker;
- exact file count: `41`;
- required member row counts:
  - `Occupation Data.txt`: `1016`
  - `Job Zones.txt`: `923`
  - `Skills.txt`: `62580`
  - `Related Occupations.txt`: `18460`

I added missing regression coverage for unsafe ZIP member paths so this cannot silently regress.

## Code Change

Changed:

- `crates/context-graph-cli/tests/dynamicjepa_research_smoke_fsv.rs`

Added:

- `research_smoke_fails_closed_for_unsafe_onet_zip_member_path`

The test creates an adversarial ZIP member named `../evil.txt`, runs the real CLI, verifies structured `VALIDATION`, reads the persisted `onet_source_preflight.json`, and asserts no RocksDB source-of-truth directory was created. This is not a mock-data success path; it is an invalid-input boundary test.

## Happy Path

Command:

```bash
cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
```

Result:

```text
7 passed; 0 failed; finished in 207.24s
```

Broad gate:

```bash
cargo test --workspace --all-features smoke -- --nocapture
```

Result: passed.

Additional checks:

```bash
cargo check -p context-graph-cli --all-features
cargo fmt --check
```

Results: passed.

## Execute And Inspect

Final broad-smoke bundle readback:

```json
{
  "status": "passed",
  "run_id": "test_research_smoke_46288",
  "purpose": "smoke",
  "career_event_count": 120,
  "career_training_seed": 42
}
```

O*NET preflight readback:

```json
{
  "status": "passed",
  "archive_entry_count": 42,
  "archive_file_count": 41,
  "source_zip_sha256": "b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741",
  "member_rows": {
    "db_30_2_text/Occupation Data.txt": 1016,
    "db_30_2_text/Job Zones.txt": 923,
    "db_30_2_text/Skills.txt": 62580,
    "db_30_2_text/Related Occupations.txt": 18460
  }
}
```

Career RocksDB readback:

```json
{
  "status": "ok",
  "counts": {
    "dj_raw_events": 124,
    "dj_adapter_runs": 124,
    "dj_instrument_readings": 1200,
    "dj_latent_panels": 120,
    "dj_dataset_shards": 3,
    "dj_training_runs": 1,
    "dj_model_artifacts": 1,
    "dj_predictions": 6,
    "dj_guard_decisions": 5,
    "dj_plan_traces": 1,
    "dj_verification_runs": 1,
    "dj_audit_log": 379
  }
}
```

Artifact hash readback:

- 9 artifact domains.
- 5 files per domain.
- 0 hash mismatches.

Generated table/plot readback:

```text
table_prediction_metrics.csv: present
table_ablation.csv: present
table_edge_cases.csv: present
plot_manifest.json: present
```

## Manual Edge Audit

### Edge 1: incomplete O*NET ZIP

Before:

```text
BEFORE edge=incomplete_onet run_dir=tmp/5090jepa_manual_edges_onet/manual_incomplete_onet_38871 exists=false bad_zip_bytes=252
```

After:

```text
AFTER edge=incomplete_onet exit_code=1 run_dir_exists=true db_entries=0 preflight_exists=true
```

Persisted error:

```json
{
  "status": "failed",
  "error_code": "VALIDATION",
  "error_message": "validation failed: O*NET 30.2 text ZIP has 1 files, expected 41 (field=research_smoke.onet.zip_file_count) remediation=use the complete unmodified O*NET 30.2 text database ZIP"
}
```

Expected outcome: failed bundle evidence exists, no DB source-of-truth paths created. Verified.

### Edge 2: unsafe ZIP member path

Before:

```text
BEFORE edge=unsafe_onet run_dir=tmp/5090jepa_manual_edges_onet/manual_unsafe_onet_38873 exists=false bad_zip_bytes=146
```

After:

```text
AFTER edge=unsafe_onet exit_code=1 run_dir_exists=true db_entries=0 preflight_exists=true
```

Persisted error:

```json
{
  "status": "failed",
  "error_code": "VALIDATION",
  "error_message": "validation failed: O*NET ZIP member has unsafe path: \"../evil.txt\" (field=research_smoke.onet.zip_member_path) remediation=use the unmodified O*NET text ZIP; archive members must not be absolute paths or escape the archive root"
}
```

Expected outcome: failed bundle evidence exists, no DB source-of-truth paths created, unsafe path rejected before extraction or generation. Verified.

### Edge 3: too-small real O*NET event count

Before:

```text
BEFORE edge=count14_onet run_dir=tmp/5090jepa_manual_edges_onet/manual_count14_onet_38891 exists=false source_zip_exists=true
```

After:

```text
AFTER edge=count14_onet exit_code=1 run_dir_exists=false stdout_bytes=504 stderr_bytes=452
```

Structured error:

```json
{
  "status": "error",
  "error_code": "VALIDATION",
  "error_message": "validation failed: O*NET-backed career_event_count must be at least 15, got 14 (field=research_smoke.career_event_count) remediation=use enough rows to populate train, validation, and test splits plus edge-case verification",
  "field": "research_smoke.career_event_count",
  "operation": "dynamicjepa_research_smoke"
}
```

Expected outcome: fail before bundle creation because the input violates the generator contract. Verified.

## Conclusion

There is no O*NET acquisition blocker. The verified ZIP is present and valid.

The robust path is now protected by:

- persisted O*NET archive preflight evidence in every generated O*NET bundle;
- exact 30.2 archive, row-count, release, and license checks;
- fail-closed missing/incomplete/archive-path boundary tests;
- manual FSV proving failed inputs do not create DB source-of-truth state;
- broad workspace smoke passing with the verified ZIP.
