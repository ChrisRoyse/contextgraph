# Phase 21 — Terraform And Cloud Artifact State And Invariants

Phase 21 ships `terraform_cloud_artifact_v1`, the first Terraform/cloud
source-of-truth connector. It verifies infrastructure claims by reading
physical Terraform JSON artifacts, normalized cloud inventory JSON, normalized
cloud audit JSONL, and the SQLite ledger rows written by `blackbox verify`.

## Shipped Surface

- Connector: `terraform_cloud_artifact_v1`.
- Connector test:
  `blackbox connector test --connector terraform_cloud_artifact_v1 --source tfplan.json`.
- Verifiers:
  - `tf_plan_only_approved_changes_v1`
  - `tf_state_attribute_v1`
  - `cloud_resource_present_v1`
  - `cloud_audit_principal_action_v1`
- Ledger record kinds reused:
  - `expected_delta`
  - `readback_snapshot`
  - `verifier_result`
  - `evidence_packet`
  - `incident_packet`

Phase 21 adds no new record kind and does not bump
`LEDGER_SCHEMA_VERSION`; current ledgers remain version `13`.

## Sources Of Truth

- Terraform plan: `tf:/path/to/tfplan.json#plan`, where the file is produced
  by `terraform show -json <planfile>`.
- Terraform state: `tf:/path/to/tfstate.json#state`, where the file is
  produced by `terraform show -json <statefile>`.
- Cloud inventory: `cloud:/path/to/cloud-inventory.json#inventory`, in
  `blackbox-cloud-inventory-json-v1` format.
- Cloud audit: `cloud:/path/to/cloud-audit.jsonl#audit-log`, in
  `blackbox-cloud-audit-jsonl-v1` format.
- Ledger: `sqlite:/path/to/blackbox.sqlite#ledger_records`.

Return values are not evidence. Successful verification requires a separate
file readback plus a direct SQLite read proving rows were appended.

## Invariants

- Terraform JSON `format_version` must be major version `1`; unsupported major
  versions are blocked as schema drift.
- Plan verification only passes when every non-`no-op` resource/output change
  exactly matches the explicit approval set.
- State verification only passes for a unique resource address, a non-sensitive
  attribute path, and exact JSON value equality.
- Cloud inventory verification only passes for an exact provider/type/id/region
  match with all required attributes equal.
- Cloud audit verification only passes when at least one JSONL event matches
  provider, principal, action, resource, outcome, and optional region.
- Missing artifacts and mismatches are `failed` verifier results and create
  `incident_packet` rows.
- Invalid JSON/JSONL, unsupported source formats, duplicate identities,
  Terraform schema drift, and sensitive Terraform state paths are `blocked`
  verifier results and do not create incident rows.

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase21_terraform_cloud_fsv -- --nocapture
```

The test writes real Terraform plan/state JSON, normalized cloud inventory
JSON, and cloud audit JSONL files. It runs the real CLI and reads SQLite
directly.

Expected final ledger counts:

```text
agent_run=1
tool_call=1
expected_delta=7
readback_snapshot=7
verifier_result=7
evidence_packet=7
incident_packet=2
```

Covered paths:

- happy Terraform plan approval;
- happy Terraform state attribute readback;
- happy cloud inventory resource readback;
- happy cloud audit principal/action proof;
- unapproved Terraform plan change (`TF_PLAN_UNAPPROVED_CHANGES`);
- wrong Terraform state value (`TF_STATE_ATTRIBUTE_MISMATCH`);
- invalid cloud audit JSONL (`CLOUD_AUDIT_JSONL_INVALID`).

## Manual FSV Checklist

1. Create a fresh ledger with `blackbox init`.
2. Write physical Terraform plan/state JSON and cloud inventory/audit
   artifacts.
3. Record a run and tool call.
4. Run all four happy verifiers.
5. Query `ledger_records` directly and confirm four
   `expected_delta/readback_snapshot/verifier_result/evidence_packet` rows and
   zero incidents.
6. Run the three edge cases.
7. Query `ledger_records` directly and confirm seven verifier/evidence rows
   and two incidents.
8. Read the latest verifier payloads and confirm the expected error codes and
   actual readback values are physically present.
