# Event Ledger Schema

The ledger is the durable record of agent behavior and verifier evidence. It
must be append-only, schema-versioned, tenant-scoped, and hash-chained.

## Phase 1 Physical Schema (shipped)

**Source of truth**: a SQLite database file at `--ledger PATH` containing two
tables. Defined in `crates/context-graph-cli/src/commands/blackbox.rs`,
function `initialize_schema`.

### Table `ledger_records` (append-only)

```sql
CREATE TABLE ledger_records (
  sequence                    INTEGER PRIMARY KEY AUTOINCREMENT,
  record_id                   TEXT NOT NULL UNIQUE,
  schema_version              INTEGER NOT NULL,   -- 1 in Phase 1
  record_kind                 TEXT NOT NULL,      -- 'agent_run' | 'tool_call'
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
| `schema_version` | `1` | constant |
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

## Record Families (target)

Phase 1 ships only two `record_kind` values: `agent_run` and `tool_call`. The
remaining families are reserved for later phases and **must not be invented
ad-hoc**:

| Phase | `record_kind` | Purpose |
|---|---|---|
| 1 | `agent_run` | one task or session |
| 1 | `tool_call` | one validated invocation request and result envelope |
| 2 | `expected_delta` | the durable state change the action is expected to produce |
| 2 | `readback_snapshot` | independent source-of-truth state after execution |
| 2 | `verifier_result` | comparison of expected delta and readback |
| 3 | `policy_decision` | authorization and approval result |
| 4 | `evidence_packet` | exportable proof bundle |
| 4 | `incident_packet` | high-severity failure bundle |
| 5 | `episode_record` | training/eval export row |
| 6 | `model_decision` | model output, tool choice, confidence, policy context |
| 6 | `agent_step` | one reasoning/action step inside a run |

When introducing a new kind, bump `LEDGER_SCHEMA_VERSION`, update the table
above, **and wipe the ledger** rather than writing a migration shim (no
backwards compatibility).

## Common Record Envelope (target shape)

The Phase 1 columns reify this envelope. JSON-shape view:

```json
{
  "schema_version": 1,
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

## AgentRun Payload (Phase 1)

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

## ToolCall Payload (Phase 1)

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

## ExpectedDelta / ReadbackSnapshot / VerifierResult (Phase 2 target)

These are **not yet stored**. Reserved JSON shapes for Phase 2:

```json
{ "record_kind": "expected_delta", "tool_call_id": "...",
  "source_of_truth": [ { "kind": "git", "locator": "...", "readback_spec_id": "..." } ],
  "delta": { "branch_exists": true, "commit_count_delta": 1 } }
```

```json
{ "record_kind": "readback_snapshot", "readback_id": "readback_...",
  "tool_call_id": "...",
  "source_of_truth": { "kind": "github", "locator": "github:org/service/pull/123" },
  "read_path": { "connector_id": "github_v1", "operation": "get_pull_request",
                 "credential_key_id": "key_..." },
  "observed_state": { "state": "open", "head_ref": "agent/fix", "commits": 1 },
  "read_at": "RFC3339" }
```

```json
{ "record_kind": "verifier_result", "verifier_result_id": "verify_...",
  "tool_call_id": "...", "verifier_id": "github_pr_open_v1",
  "status": "passed|failed|blocked|not_applicable",
  "expected_persisted_delta": {}, "actual_readback": {},
  "score": { "correctness": 1.0, "risk": 0.18, "confidence": 0.99 },
  "error_code": null, "remediation": null }
```

Typed failure example:

```json
{ "record_kind": "verifier_result", "status": "failed",
  "error_code": "GITHUB_PR_STATE_MISMATCH",
  "error_message": "expected PR state open, read closed",
  "field_path": "actual_readback.pull_request_state",
  "remediation": "reopen the PR or mark the agent run failed; do not export as a passed episode" }
```

## Integrity Requirements

- Every record has a stable `record_id` (UUID simple form, prefixed).
- Phase 2+: every mutating tool call links to one or more expected deltas;
  every expected delta links to one or more readback snapshots; every verifier
  result links to the source-of-truth snapshots it used.
- Every evidence packet must be verifiable from ledger records and hashes
  (Phase 4).
- Every exported training row must link back to immutable evidence (Phase 5).
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
