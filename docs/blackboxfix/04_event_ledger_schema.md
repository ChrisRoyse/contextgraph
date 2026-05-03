# Event Ledger Schema

The ledger is the durable record of agent behavior and verifier evidence. It
must be append-only, schema-versioned, tenant-scoped, and hash-chained.

## Phase 8 Physical Schema (shipped)

**Source of truth**: a SQLite database file at `--ledger PATH` containing two
tables. Defined in `crates/context-graph-cli/src/commands/blackbox.rs`,
function `initialize_schema`.

### Table `ledger_records` (append-only)

```sql
CREATE TABLE ledger_records (
  sequence                    INTEGER PRIMARY KEY AUTOINCREMENT,
  record_id                   TEXT NOT NULL UNIQUE,
  schema_version              INTEGER NOT NULL,   -- 5 in Phase 8
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
| `schema_version` | `5` | constant |
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

Phases 1-8 ship the following `record_kind` values. The remaining families
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

When introducing a new kind, bump `LEDGER_SCHEMA_VERSION`, update the table
above, **and wipe the ledger** rather than writing a migration shim (no
backwards compatibility).

## Common Record Envelope

The physical columns reify this envelope. JSON-shape view:

```json
{
  "schema_version": 5,
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
Actions readbacks. No new table or schema version was required because the
existing payloads already include connector id, operation, source locator,
observed state, typed error fields, and remediation.

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
