# Phase 20 — Kubernetes Deployment State And Invariants

Phase 20 ships the first infrastructure source-of-truth connector. It verifies
Kubernetes Deployment state from a physical `kubectl get ... -o json` snapshot
instead of trusting apply/rollout command return values.

## Shipped Surface

- Connector:
  `k8s_deployment_snapshot_v1`
- Command surface:
  `blackbox connector test --connector k8s_deployment_snapshot_v1 --source deployment-snapshot.json`
- Verifier surface:
  `blackbox verify --ledger PATH --input k8s-verify-spec.json`
- Verifiers:
  `k8s_deployment_image_digest_v1`,
  `k8s_replicas_match_v1`,
  `k8s_rollout_completed_v1`
- Record kinds:
  existing `expected_delta`, `readback_snapshot`, `verifier_result`,
  `evidence_packet`, and `incident_packet`
- Schema:
  no table-shape change; current `LEDGER_SCHEMA_VERSION = 14` after reward
  compiler

## Source Of Truth

The source of truth is a physical JSON file captured from Kubernetes:

```bash
kubectl get deployments.v1.apps --namespace <namespace> -o json > deployment-snapshot.json
```

The connector accepts either one `apps/v1` `Deployment` object or an `apps/v1`
`DeploymentList`. It records file path, byte count, SHA-256, parse status,
normalized Deployment metadata/spec/status, container images, and status
conditions in `readback_snapshot`.

## Verifier Contracts

`k8s_deployment_image_digest_v1` requires:

```json
{
  "source": {
    "snapshot": "/tmp/deployment-snapshot.json",
    "snapshot_format": "kubectl-deployment-json"
  },
  "expected": {
    "namespace": "prod",
    "deployment": "payments-api",
    "container": "api",
    "image_repository": "registry.example.com/payments-api:v1.2.3",
    "image_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  }
}
```

`image_repository` is optional, but `image_digest` must be a valid
`sha256:<64 hex chars>` digest. A Deployment image without a digest fails.

`k8s_replicas_match_v1` requires exact expected values for:

- `spec_replicas`
- `status_replicas`
- `updated_replicas`
- `ready_replicas`
- `available_replicas`
- `unavailable_replicas`

`k8s_rollout_completed_v1` requires:

```json
{
  "namespace": "prod",
  "deployment": "payments-api",
  "rollout_status": "completed"
}
```

Completion requires the status generation to observe the current metadata
generation, `Available=True`, `Progressing=True` with
`reason=NewReplicaSetAvailable`, no `ReplicaFailure=True`, all desired
replicas updated/ready/available, and zero unavailable replicas.

## Persisted Invariants

- Every verifier appends an `expected_delta` row before reading the snapshot.
- A valid read attempt appends a `readback_snapshot` row with connector
  `k8s_deployment_snapshot_v1`.
- Every verifier appends a `verifier_result` and `evidence_packet`.
- Failed state mismatches append an `incident_packet`.
- Blocked schema/read failures persist the blocked verifier result and
  evidence packet, but do not create an incident packet.
- Missing Deployment, missing container, wrong digest, replica mismatch, stale
  generation, failed rollout conditions, invalid JSON, unsupported snapshot
  format, and malformed Deployment schema all fail closed with typed error
  codes.

## Error Codes

- `K8S_SNAPSHOT_PATH_MISSING`
- `K8S_SNAPSHOT_NOT_FILE`
- `K8S_SNAPSHOT_READ_FAILED`
- `K8S_SNAPSHOT_NOT_FOUND`
- `K8S_SNAPSHOT_JSON_INVALID`
- `K8S_SNAPSHOT_SCHEMA_DRIFT`
- `K8S_SNAPSHOT_FORMAT_UNSUPPORTED`
- `K8S_EXPECTED_IMAGE_DIGEST_INVALID`
- `K8S_DEPLOYMENT_NOT_FOUND`
- `K8S_CONTAINER_NOT_FOUND`
- `K8S_CONTAINER_AMBIGUOUS`
- `K8S_IMAGE_DIGEST_MISSING`
- `K8S_IMAGE_DIGEST_INVALID`
- `K8S_IMAGE_REPOSITORY_MISMATCH`
- `K8S_IMAGE_DIGEST_MISMATCH`
- `K8S_REPLICA_COUNT_MISMATCH`
- `K8S_ROLLOUT_STATUS_UNSUPPORTED`
- `K8S_DEPLOYMENT_GENERATION_STALE`
- `K8S_ROLLOUT_REPLICA_FAILURE`
- `K8S_ROLLOUT_PROGRESS_DEADLINE_EXCEEDED`
- `K8S_ROLLOUT_NOT_AVAILABLE`
- `K8S_ROLLOUT_NOT_COMPLETED`
- `K8S_ROLLOUT_REPLICA_COUNTS_INCOMPLETE`

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase20_k8s_deployment_fsv -- --nocapture
```

The test writes real kubectl-style JSON files, runs the real
`context-graph blackbox` binary, and verifies the physical SQLite ledger. It
proves:

- connector list/test exposes and reads the Kubernetes snapshot connector;
- happy path passes image digest, replica, and rollout verifiers;
- missing Deployment fails with `K8S_DEPLOYMENT_NOT_FOUND`;
- wrong image digest fails with `K8S_IMAGE_DIGEST_MISMATCH`;
- malformed JSON is blocked with `K8S_SNAPSHOT_JSON_INVALID`;
- direct SQLite counts prove every expected/readback/verifier/evidence row and
  incident row exists.

Expected final test counts:

- `agent_run=1`
- `tool_call=1`
- `expected_delta=6`
- `readback_snapshot=6`
- `verifier_result=6`
- `evidence_packet=6`
- `incident_packet=2`

## Manual FSV Checklist

1. Capture pre-state:

```sql
SELECT record_kind, COUNT(*)
FROM ledger_records
GROUP BY record_kind
ORDER BY record_kind;
```

2. Run the three happy verifiers against a known snapshot.
3. Capture post-state with the same SQL and confirm three new
   expected/readback/verifier/evidence rows and no incident rows.
4. Run these edge cases and print before/after counts for each:
   missing Deployment, wrong image digest, invalid JSON snapshot.
5. Read the latest `verifier_result` row after each edge:

```sql
SELECT sequence,
       json_extract(payload_json, '$.verifier_id'),
       json_extract(payload_json, '$.status'),
       json_extract(payload_json, '$.error_code'),
       json_extract(payload_json, '$.actual_readback')
FROM ledger_records
WHERE record_kind='verifier_result'
ORDER BY sequence DESC
LIMIT 1;
```

6. Read the latest `readback_snapshot` row and verify
   `source_of_truth.kind='k8s_deployment_snapshot_v1'`.
