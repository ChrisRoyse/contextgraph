# Full State Verification And Tested Proof

BlackBoxFix is designed around Full State Verification. The shipped tests do
not rely on return values alone; they create real local state or captured
artifacts, execute the real CLI/server/MCP paths, and inspect physical
SQLite/files/API state after execution.

## FSV Rule

Every mutating workflow must:

1. define the source of truth;
2. print/read before-state;
3. execute the trigger action;
4. read after-state separately;
5. compare expected and actual state;
6. persist evidence;
7. exercise edge cases.

## Shipped FSV Test Families

Current FSV files include:

- `blackbox_phase1_fsv.rs`
- `blackbox_phase2_fsv.rs`
- `blackbox_phase3_fsv.rs`
- `blackbox_phase4_fsv.rs`
- `blackbox_phase5_export_fsv.rs`
- `blackbox_phase6_mcp_policy_fsv.rs`
- `blackbox_phase7_dashboard_fsv.rs`
- `blackbox_phase8_enterprise_fsv.rs`
- `blackbox_phase9_gitlab_ci_fsv.rs`
- `blackbox_phase10_scanner_fsv.rs`
- `blackbox_phase11_github_issue_evidence_fsv.rs`
- `blackbox_phase12_scorecard_fsv.rs`
- `blackbox_phase13_privacy_retention_fsv.rs`
- `blackbox_phase14_release_gate_fsv.rs`
- `blackbox_phase15_pilot_package_fsv.rs`
- `blackbox_phase16_risk_register_fsv.rs`
- `blackbox_phase17_http_server_fsv.rs`
- `blackbox_phase18_first_wedge_demo_fsv.rs`
- `blackbox_phase19_fsv_protocol_fsv.rs`
- `blackbox_phase20_k8s_deployment_fsv.rs`
- `blackbox_phase21_terraform_cloud_fsv.rs`
- `blackbox_phase22_ticketing_fsv.rs`
- `blackbox_phase23_otel_export_fsv.rs`
- `blackbox_phase24_sdk_ingest_fsv.rs`

## Sources Verified In Tests

The test suite covers real or physical:

- SQLite ledgers;
- Git repositories;
- local files;
- commands and artifact files;
- dashboard files;
- enterprise audit files;
- scanner artifact files;
- package JSON/JSONL files;
- FSV package files;
- Kubernetes JSON snapshots;
- Terraform/cloud artifact files;
- ticketing artifact files;
- OpenTelemetry JSONL export files;
- CloudEvents SDK event files;
- local HTTP requests/server behavior;
- live hosted API readbacks for the phases that require them.

## Edge Case Coverage

Shipped FSV tests include edge cases such as:

- missing append-only triggers;
- hash-chain tampering;
- invalid JSON;
- missing files;
- wrong Git diffs;
- failed commands;
- failed/missing CI jobs;
- scanner mismatches;
- missing issue evidence links;
- unauthorized principals;
- legal holds;
- missing scorecards;
- low trust/risk threshold failures;
- incomplete pilot evidence;
- invalid decision registers;
- bad HTTP auth;
- tenant mismatch;
- oversized HTTP body;
- missing FSV evidence;
- wrong post-state;
- bad artifact hash;
- missing Kubernetes deployment;
- wrong image digest;
- unapproved Terraform plan change;
- state mismatch;
- invalid cloud audit JSONL;
- missing ticket;
- wrong ticket state;
- empty OpenTelemetry export ledger;
- existing output directory;
- duplicate SDK source/id;
- missing SDK expected delta;
- empty SDK batch;
- invalid W3C traceparent;
- atomic SDK rollback when a later batch event is invalid.

## What The Tests Prove

The tests prove:

- commands execute against the real binary;
- ledger rows are physically present after writes;
- expected record kinds appear in SQLite;
- generated files exist on disk;
- manifests match physical file hashes;
- source-of-truth readbacks are independent of return values;
- error paths do not append hidden success rows;
- failed/blocked states are visible and typed;
- redaction happens before persistence;
- generated package/export artifacts reparse after write.

## How To Run The Full Phase-Level Proof

Run targeted phase tests while developing:

```bash
cargo test --package context-graph-cli --test blackbox_phase24_sdk_ingest_fsv -- --nocapture
```

Run broader suites as needed. Some earlier hosted-provider tests require live
network/API state and credentials; local artifact phases do not.

## Evidence Standard

A capability should be considered real only when it has:

- a command or API surface;
- a physical source-of-truth definition;
- a separate readback path;
- typed success/failure/blocking behavior;
- FSV tests that inspect durable state;
- docs explaining expected happy and edge behavior.
