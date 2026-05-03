# Event Ledger Schema

The ledger is the durable record of agent behavior and verifier evidence. It
must be append-only, schema-versioned, tenant-scoped, and hash-chained.

## Phase 17 Physical Schema (shipped)

**Source of truth**: a SQLite database file at `--ledger PATH` containing two
tables. Defined in `crates/context-graph-cli/src/commands/blackbox.rs`,
function `initialize_schema`.

### Table `ledger_records` (append-only)

```sql
CREATE TABLE ledger_records (
  sequence                    INTEGER PRIMARY KEY AUTOINCREMENT,
  record_id                   TEXT NOT NULL UNIQUE,
  schema_version              INTEGER NOT NULL,   -- 11 in Phase 17
  record_kind                 TEXT NOT NULL,      -- see Record Families
  tenant_id                   TEXT NOT NULL,
  run_id                      TEXT,
  step_id                     TEXT,
  created_at                  TEXT NOT NULL,      -- RFC3339 UTC
  actor_json                  TEXT NOT NULL,      -- canonical JSON
  payload_json                TEXT NOT NULL,      -- canonical JSON
  redaction_json              TEXT NOT NULL,      -- canonical JSON
  payload_sha256              TEXT NOT NULL,      -- 'sha256:<hex>'
  previous_record_sha256      TEXT,               -- NULL for sequence=1
  run_previous_record_sha256  TEXT,               -- NULL on first row of run
  run_chain_sha256            TEXT NOT NULL,
  record_sha256               TEXT NOT NULL UNIQUE
);

CREATE INDEX idx_ledger_records_run_id ON ledger_records(run_id, sequence);
CREATE INDEX idx_ledger_records_kind   ON ledger_records(record_kind, sequence);

-- Append-only enforcement
CREATE TRIGGER ledger_records_no_update BEFORE UPDATE ON ledger_records
  BEGIN SELECT RAISE(ABORT, 'ledger_records is append-only; updates are forbidden'); END;
CREATE TRIGGER ledger_records_no_delete BEFORE DELETE ON ledger_records
  BEGIN SELECT RAISE(ABORT, 'ledger_records is append-only; deletes are forbidden'); END;
```

PRAGMAs applied on every connection: `foreign_keys=ON`, `journal_mode=WAL`,
`synchronous=FULL`, `busy_timeout=5000`.

### Table `blackbox_metadata`

Single-row-per-key configuration set on `blackbox init`:

| Key | Initial value | Maintained by |
|---|---|---|
| `schema_version` | `11` | constant |
| `ledger_id` | random 16-byte hex | constant per ledger |
| `created_at` | UTC RFC3339 of init | constant |
| `redaction_policy_id` | `redact_v1` | constant |
| `ledger_record_count` | `0` | every `append_record` commit |
| `ledger_tip_sha256` | empty string | every `append_record` commit |

`inspect ledger` cross-checks `ledger_record_count` and `ledger_tip_sha256`
against the physical row count and last `record_sha256`. Mismatch →
`BBX_HASH_CHAIN_BROKEN` (exit 2).

`ensure_schema` also validates the physical schema before any record or
inspection path proceeds: table columns, required indexes, UNIQUE constraints,
trigger attachment, trigger SQL, `schema_version`, and `redaction_policy_id`.
A same-name trigger with different behavior is `BBX_SCHEMA_VALIDATION_FAILED`,
not a warning.

## Canonical JSON Rule

Every JSON written to the ledger is **canonicalized**: object keys are sorted
recursively into a `BTreeMap` before serialization. Hashes are computed over
the canonical bytes. Any verifier that recomputes a hash must use the same
canonicalization to match. See `canonicalize` in `blackbox.rs`.

## Hash Chain Rule

Two chains run in parallel:

```text
record_sha256 = sha256(canonical({
  schema_version, record_id, record_kind, tenant_id, run_id, step_id,
  created_at, actor_json_sha256, payload_sha256, redaction_json_sha256,
  previous_record_sha256, run_previous_record_sha256, run_chain_sha256
}))

run_chain_sha256 = sha256(canonical({
  schema_version, record_id, record_kind, tenant_id, run_id, step_id,
  created_at, actor_json_sha256, payload_sha256, redaction_json_sha256,
  run_previous_record_sha256
}))

payload_sha256 = sha256(canonical(payload_json_object))
actor_json_sha256, redaction_json_sha256 = sha256(actor_json_string), sha256(redaction_json_string)
```

All hashes are stored as `sha256:<lowercase-hex>`.

## Record Families

Phases 1-17 ship the following `record_kind` values. The remaining families
are reserved for later phases and **must not be invented ad-hoc**:

| Phase | `record_kind` | Purpose |
|---|---|---|
| 1 | `agent_run` | one task or session |
| 1 | `tool_call` | one validated invocation request and result envelope |
| 2 | `expected_delta` | the durable state change the action is expected to produce |
| 2 | `readback_snapshot` | independent source-of-truth state after execution |
| 2 | `verifier_result` | comparison of expected delta and readback |
| 6 | `policy_decision` | authorization and approval result |
| 4 | `evidence_packet` | exportable proof bundle |
| 4 | `incident_packet` | high-severity failure bundle |
| 5 | `episode_record` | reserved future ledger-side export audit row; Phase 5 currently emits JSONL rows outside the ledger |
| 6 | `agent_step` | one reasoning/action step inside a run |
| 6 | `model_decision` | model output, tool choice, confidence, policy context |
| 8 | `enterprise_tenant_config` | validated SSO/RBAC/retention/deployment/key metadata for one tenant |
| 8 | `enterprise_directory_snapshot` | validated SCIM user/group/role snapshot for one tenant |
| 8 | `enterprise_access_decision` | persisted RBAC and tenant-isolation access decision |
| 12 | `scorecard` | deterministic run-level trust/risk/training-value score snapshot |
| 13 | `retention_decision` | authorized retention review decision for one run and data-subject digest |
| 13 | `evidence_deletion` | append-only erasure/deletion-request approval or denial audit row |
| 14 | `release_gate` | append-only release/promotion approval or block decision over score, evidence, incident, RBAC, and privacy state |
| 15 | `pilot_package` | append-only pilot packaging/report row with usage meters, eval-candidate summary, file manifest, and source refs |
| 16 | `risk_assessment` | append-only risk/open-question readiness decision over ledger, runtime, package, incident, secret-scan, and required decision evidence |
| 17 | `http_request_audit` | append-only authenticated HTTP request/response audit row for the Phase 17 single-writer server |

When introducing a new kind, bump `LEDGER_SCHEMA_VERSION`, update the table
above, **and wipe the ledger** rather than writing a migration shim (no
backwards compatibility).

## Common Record Envelope

The physical columns reify this envelope. JSON-shape view:

```json
{
  "schema_version": 11,
  "record_id": "bbx_rec_<uuid_simple>",
  "record_kind": "tool_call",
  "tenant_id": "tenant_...",
  "run_id": "run_...",
  "step_id": null,
  "created_at": "2026-05-02T00:00:00Z",
  "actor": {
    "actor_kind": "ai_agent",
    "agent_id": "agent_...",
    "model_id": "provider/model | null",
    "human_approver_id": null
  },
  "hash": {
    "payload_sha256": "sha256:...",
    "previous_record_sha256": "sha256:... | null",
    "run_previous_record_sha256": "sha256:... | null",
    "run_chain_sha256": "sha256:...",
    "record_sha256": "sha256:..."
  },
  "redaction": {
    "policy_id": "redact_v1",
    "redacted_fields": ["$.api_key"],
    "secret_digest_fields": {"$.api_key": "sha256:..."}
  }
}
```

## HttpRequestAudit Payload (Phase 17, shipped)

`blackbox server serve --ledger PATH --bind 127.0.0.1:7607` writes one
`http_request_audit` row for every protected HTTP request, including refused
authentication, tenant mismatch, malformed schema, and body-limit failures.
The row is the durable proof that the HTTP trigger happened and records the
route, tenant/audit scope, status code, typed error code, body hashes, and
server safety configuration.

```json
{
  "record_kind": "http_request_audit",
  "http_request_audit_id": "http_audit_<uuid_simple>",
  "http_request_audit_schema_version": 1,
  "request": {
    "request_id": "http_req_...",
    "method": "POST",
    "path": "/v1/runs",
    "route_id": "POST /v1/runs",
    "tenant_id": "tenant_...",
    "started_at": "2026-05-03T00:00:00Z",
    "finished_at": "2026-05-03T00:00:00Z",
    "body_sha256": "sha256:... | null"
  },
  "response": {
    "status_code": 200,
    "outcome": "ok | error",
    "error_code": null,
    "body_sha256": "sha256:..."
  },
  "server": {
    "connector": "blackbox_http_server_v1",
    "bind": "127.0.0.1:7607",
    "max_body_bytes": 65536,
    "api_key_storage": "environment_variable_hash_in_memory",
    "single_writer_lock": true
  }
}
```

Authentication and schema failures use tenant id `tenant_http_gateway` so the
failure itself is still auditable without trusting an unauthenticated tenant
claim.

## EnterpriseTenantConfig Payload (Phase 8, shipped)

`blackbox enterprise configure --ledger PATH --tenant TENANT --input FILE`
validates and persists a fail-closed enterprise-control baseline. The input
must contain either OIDC or SAML metadata, explicit RBAC roles, retention
classes, private deployment metadata, and a tenant key id. Raw credential
material is refused; only key ids and SHA-256 fingerprints are stored.

```json
{
  "record_kind": "enterprise_tenant_config",
  "enterprise_control_schema_version": 1,
  "tenant_id": "tenant_...",
  "sso": {
    "protocol": "oidc",
    "issuer": "https://idp.example.test/tenant",
    "client_id": "blackboxfix-enterprise",
    "jwks_sha256": "sha256:<64-hex>",
    "required_claims": ["iss", "sub", "aud", "exp"]
  },
  "rbac": {
    "roles": [
      { "role_id": "security_admin", "permissions": ["audit.export"] }
    ]
  },
  "retention": {
    "default_class": "standard",
    "classes": [
      { "class_id": "standard", "retain_days": 365, "legal_hold": false }
    ]
  },
  "deployment": {
    "mode": "private_vpc",
    "region": "us-test-1",
    "data_residency": "us",
    "kms_key_id": "kms_tenant_key_id",
    "siem_format": "rfc5424_jsonl"
  },
  "tenant_key_store": {
    "customer_managed": true,
    "key_scope": "tenant",
    "tenant_id": "tenant_...",
    "kms_key_id": "kms_tenant_key_id",
    "data_residency": "us"
  }
}
```

## EnterpriseDirectorySnapshot Payload (Phase 8, shipped)

`blackbox enterprise sync-directory` persists a SCIM-shaped snapshot only after
the tenant config exists. Users must reference known groups, and groups must
reference configured roles.

```json
{
  "record_kind": "enterprise_directory_snapshot",
  "enterprise_control_schema_version": 1,
  "tenant_id": "tenant_...",
  "schemas": [
    "urn:ietf:params:scim:schemas:core:2.0:User",
    "urn:ietf:params:scim:schemas:core:2.0:Group"
  ],
  "users": [
    {
      "id": "user_alice",
      "userName": "alice@example.test",
      "active": true,
      "groups": ["group_secops"]
    }
  ],
  "groups": [
    {
      "id": "group_secops",
      "displayName": "SecOps",
      "roles": ["security_admin"]
    }
  ],
  "source_refs": {
    "tenant_config": {
      "record_kind": "enterprise_tenant_config",
      "record_sha256": "sha256:..."
    }
  }
}
```

## EnterpriseAccessDecision Payload (Phase 8, shipped)

`blackbox enterprise access-evaluate` always persists a decision row when the
tenant config and directory snapshot are valid. Denials are durable evidence,
not hidden return values.

```json
{
  "record_kind": "enterprise_access_decision",
  "enterprise_control_schema_version": 1,
  "access_decision_id": "enterprise_access_<uuid>",
  "tenant_id": "tenant_...",
  "principal_id": "user_alice",
  "permission": "audit.export",
  "resource": "tenant:tenant_...:audit/export",
  "allowed": true,
  "decision": "allowed",
  "reason_code": null,
  "roles": ["security_admin"],
  "permissions": ["audit.export"],
  "source_refs": {
    "tenant_config": { "record_sha256": "sha256:..." },
    "directory_snapshot": { "record_sha256": "sha256:..." }
  }
}
```

## AgentRun Payload (Phase 1+, shipped)

```json
{
  "record_kind": "agent_run",
  "run_id": "run_<uuid_simple>",
  "task": {
    "task_kind": "local_agent_task",
    "requested_outcome": "<--task value>",
    "risk_tier": "low|medium|high"
  },
  "runtime": {
    "framework": "cli",
    "adapter_version": "<CARGO_PKG_VERSION of context-graph-cli>"
  },
  "status": "running",
  "summary": {
    "steps_total": 0,
    "tool_calls_total": 0,
    "verifier_passed": 0,
    "verifier_failed": 0,
    "verifier_blocked": 0
  }
}
```

## ToolCall Payload (Phase 1+, shipped)

```json
{
  "record_kind": "tool_call",
  "tool_call_id": "tool_<uuid_simple>",
  "run_id": "run_...",
  "tool_name": "<--tool value>",
  "capability_id": "<--capability value, default 'local.tool'>",
  "input_digest": "sha256:<hex over canonical(input_redacted)>",
  "input_redacted": { ... },
  "authorization": {
    "policy_decision_id": null,
    "mode": "local",
    "approved_by": null
  },
  "execution": {
    "started_at": "RFC3339",
    "finished_at": "RFC3339",
    "status": "ok|failed|blocked",
    "external_request_id": "<optional>"
  }
}
```

## ExpectedDelta / ReadbackSnapshot / VerifierResult (Phase 2/3/4)

These are stored by `blackbox verify` after a recorded `tool_call` exists for
the requested `run_id` and tenant.

```json
{
  "record_kind": "expected_delta",
  "expected_delta_id": "expected_<uuid_simple>",
  "tool_call_id": "tool_...",
  "verifier_id": "git_branch_exists_v1",
  "source_of_truth": "git:/tmp/repo#refs/heads/main",
  "expected_persisted_delta": { "branch": "main" },
  "spec_redacted": { "...": "..." },
  "recorded_at": "RFC3339"
}
```

```json
{
  "record_kind": "readback_snapshot",
  "readback_id": "readback_...",
  "tool_call_id": "...",
  "source_of_truth": { "kind": "git_local_v1", "locator": "git:/tmp/repo#app.txt" },
  "read_path": {
    "connector_id": "git_local_v1",
    "operation": "diff --no-ext-diff --no-color -- <path>",
    "credential_key_id": null
  },
  "observed_state": {
    "repository": { "head": "..." },
    "observed": { "path": "app.txt", "patch_sha256": "sha256:..." }
  },
  "read_at": "RFC3339"
}
```

```json
{
  "record_kind": "verifier_result",
  "verifier_result_id": "verify_...",
  "tool_call_id": "tool_...",
  "verifier_id": "cmd_exit_zero_v1",
  "status": "passed|failed|blocked",
  "expected_persisted_delta": {},
  "actual_readback": {},
  "score": { "correctness": 1.0, "risk": 0.18, "confidence": 0.99 },
  "error_code": null,
  "error_message": null,
  "field_path": null,
  "remediation": null,
  "source_of_truth": "cmd:/tmp/work#sha256:...",
  "connector": "cmd_local_v1",
  "recorded_at": "RFC3339"
}
```

Typed failure example:

```json
{
  "record_kind": "verifier_result",
  "status": "failed",
  "error_code": "GIT_WORKING_TREE_DIRTY",
  "error_message": "Git working tree cleanliness did not match the expected state",
  "field_path": "actual_readback.clean",
  "remediation": "commit, revert, or remove working tree changes before claiming the run is clean"
}
```

Phase 3 reuses these same three evidence record kinds for GitHub/GitHub
Actions readbacks. Phase 9 reuses them again for GitLab CI pipeline/job
readbacks. Phase 10 reuses them again for scanner artifact readbacks. Phase 11
reuses them again for GitHub issue evidence-link readbacks. No new table was
required because the existing payloads already include connector id,
operation, source locator, observed state, typed error fields, and
remediation.

## Scorecard (Phase 12, shipped)

`blackbox score run` appends one `scorecard` row after independently reading
the physical ledger. It excludes prior scorecards from score inputs, verifies
evidence packet references, and reads the appended row back by sequence and
hash before reporting success.

```json
{
  "record_kind": "scorecard",
  "scorecard_id": "score_<uuid_simple>",
  "run_id": "run_...",
  "scorecard_schema_version": 1,
  "score_rule_version": "score_v1",
  "source_of_truth": "sqlite:/tmp/bbx.sqlite#ledger_records",
  "score": {
    "correctness": 1.0,
    "verification_confidence": 0.9725,
    "risk": 0.58,
    "policy_compliance": "allowed",
    "training_value": 0.2,
    "trust_level": "verified_with_policy"
  },
  "score_inputs": {
    "tool_calls_total": 1,
    "tool_calls_with_evidence": 1,
    "verifier_status_counts": { "passed": 1, "failed": 0, "blocked": 0 },
    "policy_decision_counts": { "allowed": 1, "denied": 0 },
    "evidence_hash_verification": { "status": "passed" }
  },
  "input_record_refs": [
    {
      "role": "score_input",
      "sequence": 1,
      "record_kind": "agent_run",
      "payload_sha256": "sha256:...",
      "record_sha256": "sha256:..."
    }
  ]
}
```

## Privacy Retention And Erasure Decisions (Phase 13, shipped)

`blackbox privacy evaluate-retention` and `blackbox privacy request-erasure`
both start by verifying the physical ledger, checking the parent `agent_run`,
and persisting an `enterprise_access_decision` for the required RBAC
permission (`privacy.review` or `privacy.erase`). Only authorized requests can
append a privacy decision row. The raw `--data-subject` value is never stored;
the payload stores only `data_subject_sha256`.

```json
{
  "record_kind": "retention_decision",
  "privacy_schema_version": 1,
  "privacy_decision_id": "privacy_retention_<uuid_simple>",
  "tenant_id": "tenant_...",
  "run_id": "run_...",
  "request": {
    "request_kind": "retention_review",
    "principal_id": "user_privacy",
    "permission": "privacy.review",
    "resource": "tenant:tenant_...:privacy/run_...",
    "data_subject_sha256": "sha256:<hex>",
    "data_subject_storage": "sha256_only_raw_value_not_persisted",
    "purpose": "case-id-or-purpose"
  },
  "retention_policy": {
    "class_id": "standard",
    "retain_days": 365,
    "legal_hold": false,
    "config_record_ref": { "record_kind": "enterprise_tenant_config" }
  },
  "run_evidence_summary": {
    "record_count": 1,
    "record_kind_counts": { "agent_run": 1 },
    "evidence_packets_total": 0,
    "incident_packets_total": 0,
    "input_record_refs_sha256": "sha256:..."
  },
  "decision": {
    "status": "retained",
    "allowed": true,
    "reason_code": "RETENTION_POLICY_APPLIED",
    "ledger_action": "retain_append_only_audit",
    "artifact_action": "none",
    "deletes_ledger_rows": false
  },
  "source_refs": {
    "tenant_config": { "record_sha256": "sha256:..." },
    "authorization_decision": { "record_kind": "enterprise_access_decision" },
    "input_records": []
  }
}
```

For erasure requests the record kind is `evidence_deletion`. A request can be
`approved` only when the configured retention class has `legal_hold=false` and
the run has no incident packet. Legal hold and incident evidence produce a
durable `denied` decision with `LEGAL_HOLD_ACTIVE` or
`INCIDENT_EVIDENCE_LOCKED`. The ledger remains append-only in every case; the
decision records whether matching external artifact stores may proceed.

## Release Gate Decisions (Phase 14, shipped)

`blackbox gate evaluate` appends one `release_gate` row for authorized gate
evaluations. The command first verifies the physical ledger, requires the
parent `agent_run`, persists and reads back an `enterprise_access_decision`
for `release.approve`, reads the latest `scorecard`, verifies
`evidence_packet.record_refs[]`, checks incident/verifier/privacy state, and
then reads back the appended gate row by sequence, `release_gate_id`,
`run_id`, `payload_sha256`, and `record_sha256`.

```json
{
  "record_kind": "release_gate",
  "release_gate_schema_version": 1,
  "release_gate_rule_version": "release_gate_v1",
  "release_gate_id": "release_gate_<uuid_simple>",
  "tenant_id": "tenant_...",
  "run_id": "run_...",
  "request": {
    "principal_id": "user_release",
    "permission": "release.approve",
    "resource": "tenant:tenant_...:release/production/run_...",
    "environment": "production",
    "min_trust_level": "verified_with_policy",
    "max_risk": 0.7,
    "require_policy": true,
    "require_privacy": true
  },
  "run_evidence_summary": {
    "record_kind_counts": {
      "agent_run": 1,
      "scorecard": 1,
      "retention_decision": 1
    },
    "verifier_status_counts": {
      "passed": 1,
      "failed": 0,
      "blocked": 0
    },
    "incident_packets_total": 0,
    "input_record_refs_sha256": "sha256:..."
  },
  "checks": [
    {
      "check_id": "risk_at_or_below_threshold",
      "passed": true,
      "observed": 0.58,
      "expected": 0.7,
      "reason_code": null
    }
  ],
  "decision": {
    "status": "approved",
    "allowed": true,
    "reason_code": "RELEASE_GATE_APPROVED",
    "failed_checks": [],
    "promotes_to_environment": "production"
  },
  "source_refs": {
    "authorization_decision": { "record_kind": "enterprise_access_decision" },
    "input_records": []
  }
}
```

Blocked evaluations persist `decision.status=blocked` and a non-empty
`failed_checks[]`, then the CLI exits non-zero with `RELEASE_GATE_BLOCKED`.
Invalid gate input, a missing parent run, or unauthorized principal does not
create a `release_gate` row. Unauthorized attempts still persist the
`enterprise_access_decision` denial.

## Pilot Package (Phase 15, shipped)

`blackbox package pilot-report` appends one `pilot_package` row for authorized
pilot package builds. The command first verifies the physical ledger, persists
and reads back an `enterprise_access_decision` for `commercial.package`,
computes metrics from tenant rows excluding previous package rows, writes and
reads back the package files, appends the package row, reads it back by
sequence, `pilot_package_id`, `payload_sha256`, and `record_sha256`, then
promotes and re-verifies the final package directory.

```json
{
  "record_kind": "pilot_package",
  "pilot_package_schema_version": 1,
  "pilot_package_rule_version": "pilot_package_v1",
  "pilot_package_id": "pilot_package_<uuid_simple>",
  "tenant_id": "tenant_...",
  "pilot_id": "pilot_...",
  "request": {
    "principal_id": "user_package",
    "permission": "commercial.package",
    "resource": "tenant:tenant_...:commercial/pilot/pilot_...",
    "min_actions": 20,
    "out": "/tmp/pilot-package"
  },
  "usage_summary": {
    "agent_runs_total": 20,
    "tool_calls_total": 20,
    "tool_calls_with_evidence": 20,
    "release_gates_total": 1,
    "eval_candidates_total": 1
  },
  "pilot_success": {
    "status": "passed",
    "minimum_real_actions": {
      "required": 20,
      "observed": 20,
      "passed": true
    },
    "failed_criteria": []
  },
  "pricing_meter_summary": {
    "schema": "blackbox_usage_meter_events_jsonl_v1",
    "events_total": 43,
    "aggregation": "sum(value) by event_name, customer_identifier, and billing period",
    "idempotency": "identifier is derived from ledger sequence and record_sha256"
  },
  "file_manifest": {
    "format": "blackbox_pilot_package_manifest_json_v1",
    "files": [
      { "path": "pilot-report.json", "sha256": "sha256:...", "bytes": 1234 },
      { "path": "usage-meter-events.jsonl", "sha256": "sha256:...", "bytes": 5678 },
      { "path": "eval-candidates.jsonl", "sha256": "sha256:...", "bytes": 901 }
    ]
  },
  "source_refs": {
    "authorization_decision": { "record_kind": "enterprise_access_decision" },
    "input_records": []
  }
}
```

If pilot success criteria are not met, the package row and files are still
durable evidence with `pilot_success.status=needs_more_evidence`; the CLI
then exits non-zero with `PILOT_PACKAGE_CRITERIA_NOT_MET`. Invalid request
fields and unauthorized principals create no package row or package files.

## Risk Assessment (Phase 16, shipped)

`blackbox risk evaluate` appends one `risk_assessment` row for authorized
pilot-readiness risk reviews. The command validates the required open-question
decision register before authorization, verifies the physical ledger and hash
chain, persists and reads back an `enterprise_access_decision` for
`risk.review`, evaluates the current ledger/runtime/package/incidents state,
appends a durable accepted or blocked assessment, and reads the row back by
sequence, `risk_assessment_id`, `payload_sha256`, and `record_sha256`.

```json
{
  "record_kind": "risk_assessment",
  "risk_assessment_schema_version": 1,
  "risk_assessment_rule_version": "risk_register_v1",
  "risk_assessment_id": "risk_assessment_<uuid_simple>",
  "tenant_id": "tenant_...",
  "request": {
    "principal_id": "user_risk",
    "permission": "risk.review",
    "resource": "tenant:tenant_...:risk/assessment",
    "decisions_path": "/tmp/phase16-decisions.json"
  },
  "ledger_runtime": {
    "ledger_schema_version": 11,
    "journal_mode": "wal",
    "synchronous": 2,
    "busy_timeout_ms": 5000
  },
  "risk_summary": {
    "checks_total": 10,
    "checks_passed": 10,
    "checks_failed": 0,
    "critical_failed": 0,
    "risk_posture": "accepted"
  },
  "checks": [
    {
      "check_id": "open_questions_closed",
      "passed": true,
      "severity": "high",
      "observed": 5,
      "expected": 5,
      "reason_code": null
    }
  ],
  "open_question_decisions": {
    "decision_schema_version": 1,
    "required_decisions_total": 5,
    "closed_decisions_total": 5,
    "status_counts": { "resolved": 3, "accepted": 1, "deferred": 1 }
  },
  "decision": {
    "status": "accepted",
    "allowed": true,
    "reason_code": "RISK_ASSESSMENT_ACCEPTED",
    "failed_checks": []
  },
  "source_refs": {
    "authorization_decision": { "record_kind": "enterprise_access_decision" },
    "input_records": []
  }
}
```

Blocked assessments persist `decision.status=blocked` and a non-empty
`failed_checks[]`, then the CLI exits non-zero with
`RISK_ASSESSMENT_BLOCKED`. Invalid decision files or unauthorized principals
do not create a `risk_assessment` row. Unauthorized attempts still persist the
denied `enterprise_access_decision`.

## EvidencePacket (Phase 4, shipped)

Every associated `blackbox verify` call appends an `evidence_packet` after the
`verifier_result`. The packet is not a detached copy of success output; it is a
portable proof index over the immutable physical ledger rows used by the
verification.

```json
{
  "record_kind": "evidence_packet",
  "evidence_packet_id": "ev_<uuid_simple>",
  "run_id": "run_...",
  "tool_call_id": "tool_...",
  "verifier_id": "git_branch_exists_v1",
  "verifier_result_id": "verify_...",
  "verifier_status": "passed|failed|blocked",
  "source_of_truth": [
    { "kind": "git_local_v1", "locator": "git:/tmp/repo#refs/heads/main" }
  ],
  "record_refs": [
    {
      "sequence": 1,
      "record_id": "bbx_rec_...",
      "record_kind": "agent_run",
      "payload_sha256": "sha256:...",
      "record_sha256": "sha256:..."
    }
  ],
  "hashes": {
    "expected_delta_record_sha256": "sha256:...",
    "readback_snapshot_record_sha256": "sha256:... | null",
    "verifier_result_record_sha256": "sha256:..."
  },
  "provenance": {
    "model": "w3c-prov-inspired",
    "entities": [],
    "activities": [],
    "agents": []
  }
}
```

`blackbox replay` validates every `record_refs[]` entry against the physical
SQLite rows by `sequence`, `record_kind`, `payload_sha256`, and
`record_sha256`. A missing or mismatched reference fails closed with an
evidence-integrity error.

## IncidentPacket (Phase 4, shipped)

A failed verifier appends an `incident_packet` after the evidence packet. The
Phase 4 implementation creates incidents for `verifier_failed`; later
governed-mode phases may add blocked-policy incidents.

```json
{
  "record_kind": "incident_packet",
  "incident_packet_id": "inc_<uuid_simple>",
  "trigger": "verifier_failed",
  "severity": "high",
  "run_id": "run_...",
  "tool_call_id": "tool_...",
  "verifier_id": "git_branch_exists_v1",
  "verifier_result_id": "verify_...",
  "evidence_packet_id": "ev_...",
  "evidence_packet_sequence": 6,
  "evidence_packet_record_sha256": "sha256:...",
  "error_code": "GIT_BRANCH_NOT_FOUND",
  "error_message": "...",
  "field_path": "actual_readback.branch",
  "remediation": "create the branch before claiming it exists",
  "source_of_truth": "git:/tmp/repo#refs/heads/missing",
  "timeline": ["bbx_rec_...", "bbx_rec_..."],
  "blast_radius": {
    "repos": [],
    "tickets": [],
    "deployments": [],
    "files": []
  }
}
```

## Integrity Requirements

- Every record has a stable `record_id` (UUID simple form, prefixed).
- Phase 2+: every mutating tool call links to one or more expected deltas;
  every expected delta links to one or more readback snapshots; every verifier
  result links to the source-of-truth snapshots it used.
- Every evidence packet must be verifiable from physical ledger records and
  hashes; Phase 4 replay enforces this.
- Every exported training/eval JSONL row must link back to immutable evidence;
  Phase 5 enforces this in `blackbox export episodes`.
- The append-only triggers must be **present and enabled** on every read; the
  Phase 1 FSV tests `phase1_append_only_trigger_blocks_direct_update` and
  `phase1_schema_validation_rejects_noop_append_only_trigger` prove this
  against a real SQLite database.

## FSV Of This Document

Source of truth: the actual `CREATE TABLE` and `CREATE TRIGGER` statements
emitted by `initialize_schema` in `blackbox.rs`. To verify after edits:

```bash
sqlite3 /tmp/bbx.sqlite '.schema ledger_records'
sqlite3 /tmp/bbx.sqlite '.schema blackbox_metadata'
sqlite3 /tmp/bbx.sqlite "SELECT name, sql FROM sqlite_master WHERE type='trigger';"
```

The output must match this document column-for-column and trigger-for-trigger.
If they diverge, **the document is wrong** — update it before merging.
