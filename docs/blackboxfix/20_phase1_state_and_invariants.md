# Phase 1 — State, Invariants, And Synthetic Tests

This document is the canonical reference for the **shipped** Phase 1
implementation. A fresh-context AI agent must be able to read it once and
know exactly what exists on disk, what each command does, and how to verify
it physically.

Historical note: later phases have bumped the live ledger schema to version
`10` and added verifier/evidence/enterprise/score/privacy/release-gate/package/
risk record kinds. This file preserves the Phase 1 baseline. For the current
surface, read the phase state files through
`35_phase16_state_and_invariants.md`.

Source of truth for the implementation: commit `1cf89284` plus the files at
`crates/context-graph-cli/src/commands/blackbox.rs`,
`crates/context-graph-cli/src/bin/blackbox.rs`,
`crates/context-graph-cli/tests/blackbox_phase1_fsv.rs`,
and `crates/context-graph-cli/src/main.rs:136-139,210`.

## File Layout

| Path | Role |
|---|---|
| `crates/context-graph-cli/Cargo.toml` | declares the `blackbox` binary + `rusqlite` dep with `bundled` feature |
| `crates/context-graph-cli/src/bin/blackbox.rs` | standalone binary entry point; calls `commands::blackbox::handle_blackbox_command` |
| `crates/context-graph-cli/src/main.rs` | umbrella entry; hosts `Commands::Blackbox` variant |
| `crates/context-graph-cli/src/commands/mod.rs` | exposes `pub mod blackbox;` |
| `crates/context-graph-cli/src/commands/blackbox.rs` | full Phase 1 implementation (≈1600 lines, single file) |
| `crates/context-graph-cli/tests/blackbox_phase1_fsv.rs` | FSV harness (10 tests, all pass) |

## Phase 1 Constants (superseded by current schema version 12)

```rust
const LEDGER_SCHEMA_VERSION: i64 = 1;
const REDACTION_POLICY_ID:   &str = "redact_v1";
const SQLITE_CONNECTOR_ID:   &str = "sqlite_local_v1";
```

## Source Of Truth

A SQLite database file at the operator-supplied `--ledger PATH` (or env var
`BLACKBOX_LEDGER`). Pragmas applied on every connection:

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA busy_timeout = 5000;
```

Tables and triggers (full DDL is in `04_event_ledger_schema.md`):

- `ledger_records` (append-only, dual-hash-chained)
- `blackbox_metadata` (single row per key)
- triggers `ledger_records_no_update` and `ledger_records_no_delete` raise
  `RAISE(ABORT, 'ledger_records is append-only; …')` on any UPDATE/DELETE.
- `ensure_schema` validates table columns, required indexes, UNIQUE
  constraints, trigger attachment, trigger SQL, `schema_version`, and
  `redaction_policy_id` before record/inspect paths proceed.

Logical source-of-truth string used in error reports:
`sqlite:<path>#ledger_records`.

## Commands And Their JSON Contracts

### `blackbox init --ledger PATH`

Creates / opens the SQLite file, applies the schema, runs
`verify_ledger_integrity` (so reopen on a corrupted file fails loudly).

Stdout (success):

```json
{
  "status": "ok",
  "command": "blackbox init",
  "ledger_path": "/tmp/bbx.sqlite",
  "source_of_truth": "sqlite:/tmp/bbx.sqlite#ledger_records",
  "schema_version": 1,
  "redaction_policy_id": "redact_v1",
  "record_count": 0,
  "ledger_tip_sha256": ""
}
```

### `blackbox record run --ledger PATH --task T --agent A [...]`

Inserts one `record_kind='agent_run'` row. Returns the new `run_id`.

Stdout (success): `{ status, record_id, record_kind, run_id,
ledger_sequence, source_of_truth, payload_sha256, previous_record_sha256,
run_chain_sha256, record_sha256 }`.

### `blackbox record tool-call --ledger PATH --run-id R --tool T --input PATH [...]`

Reads the JSON file at `--input`, redacts it via `redact_v1`, computes
`input_digest = sha256(canonical(input_redacted))`, inserts one
`record_kind='tool_call'` row that links to the parent `run_id` (validated
to exist).

Stdout (success): same envelope as `record run`, plus `tool_call_id` and
`redaction` (the full `RedactionReport` JSON: `policy_id`, `redacted_fields`,
`secret_digest_fields`).

### `blackbox inspect ledger --ledger PATH [--run-id R] [--limit N]`

Re-runs `verify_ledger_integrity`: every per-record hash, every per-run hash,
the metadata cross-checks. Returns the (optionally filtered) records.

Stdout (success): `{ status, source_of_truth, connector, integrity:
{ status: "passed", schema_version, record_count, ledger_tip_sha256,
metadata_record_count, metadata_tip_sha256 }, records: [...] }`.

## Redaction Policy `redact_v1`

Implementation: `redact_value` + `looks_like_secret_literal` +
`is_sensitive_field_name` in `blackbox.rs`.

Sensitive field-name match (after lowercasing and stripping non-alphanumeric):

```
password, passwd, pwd, token, apikey, accesstoken, refreshtoken,
authorization, secret, clientsecret, privatekey, sshkey, connectionstring
```

Plus suffix match: any key ending in `token`, `secret`, or `apikey`.

Sensitive-named field values are replaced with
`[REDACTED:sha256:<hex>]` and recorded in `redaction_json.redacted_fields` /
`secret_digest_fields`.

Sensitive **literal** patterns (case-insensitive):

- contains `bearer ` (Bearer tokens);
- contains `-----begin ` or `private key-----` (PEM blocks);
- contains `api_key=`, `apikey=`, `access_token=`, `password=`,
  `client_secret=`;
- starts with `ghp_`, `github_pat_`, `xoxb-`, `sk-`;
- AWS access key form: 20 chars starting with `AKIA`, all uppercase /
  digits;
- JWT form: three `.`-separated segments, each ≥8 chars, base64url
  alphabet, first segment starts with `eyJ`.

Any literal match in a non-sensitive field aborts the write with
`BBX_REDACTION_VIOLATION` (no row inserted). Verified by
`phase1_redaction_violation_blocks_persistence`.

## Error Codes (Phase 1)

| `error_code` | Trigger | Exit code |
|---|---|---|
| `BBX_INVALID_INPUT` | empty required flag, blank ledger path, etc. | 1 |
| `BBX_IO_ERROR` | filesystem failure (mkdir, read input file) | 1 |
| `BBX_SQLITE_ERROR` | any rusqlite error | 1 |
| `BBX_SCHEMA_VALIDATION_FAILED` | unknown schema_version, missing tables/triggers | 1 |
| `BBX_LEDGER_NOT_INITIALIZED` | running `record`/`inspect` against a non-existent ledger | 1 |
| `BBX_REDACTION_VIOLATION` | sensitive literal outside redacted field | 1 |
| `BBX_RECORD_NOT_FOUND` | `--run-id` does not exist | 1 |
| `BBX_TENANT_MISMATCH` | `record tool-call` references a run owned by a different tenant | 1 |
| `BBX_INPUT_JSON_INVALID` | tool input file is not valid JSON | 1 |
| `BBX_INPUT_SCHEMA_INVALID` | tool input JSON is not an object | 1 |
| `BBX_JSON_CANONICALIZATION_FAILED` | serde_json failure during canonicalization | 1 |
| `BBX_HASH_CHAIN_BROKEN` | tamper detected by `verify_ledger_integrity` | **2** |

Every error is emitted on **stderr** as JSON:
`{ status:"error", error_code, message, source_of_truth, connector,
field_path, remediation }`.

## Identifier Formats

```
run_<uuid_simple>       e.g. run_2f0c9b2eb0bf4c8fa9b30b5d9c72a4d1
bbx_rec_<uuid_simple>   e.g. bbx_rec_dca9cc...
tool_<uuid_simple>      e.g. tool_b1f9...
```

Hashes are stored as `sha256:<lowercase-hex>`.

## Synthetic Tests (operator-runnable; matches the automated suite)

### Setup

```bash
ledger=/tmp/bbx-fsv-$(date +%s).sqlite
context-graph blackbox init --ledger "$ledger"
```

Source-of-truth check after init:

```bash
sqlite3 "$ledger" "SELECT key, value FROM blackbox_metadata ORDER BY key;"
# Expect rows: created_at, ledger_id, ledger_record_count='0', ledger_tip_sha256='',
#              redaction_policy_id='redact_v1', schema_version='1'

sqlite3 "$ledger" "SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name;"
# Expect: ledger_records_no_delete, ledger_records_no_update
```

### Happy Path — Persist Run + Tool Call, Inspect

```bash
RUN=$(context-graph blackbox record run \
        --ledger "$ledger" \
        --task "phase1 happy path" \
        --agent agent_phase1 \
        --tenant tenant_phase1 \
        --risk-tier low | jq -r .run_id)

cat > /tmp/tool-input.json <<'JSON'
{
  "file": "Cargo.toml",
  "operation": "inspect",
  "expected_delta": { "records_added": 0 },
  "api_key": "phase1-secret"
}
JSON

context-graph blackbox record tool-call \
  --ledger "$ledger" \
  --run-id "$RUN" \
  --tool phase1_real_tool \
  --capability phase1.real_tool.write \
  --tenant tenant_phase1 \
  --agent agent_phase1 \
  --input /tmp/tool-input.json

context-graph blackbox inspect ledger --ledger "$ledger" --run-id "$RUN"
```

Source-of-truth proofs (must all hold):

```bash
sqlite3 "$ledger" "SELECT COUNT(*) FROM ledger_records;"
# Expect: 2

sqlite3 "$ledger" "SELECT record_kind FROM ledger_records ORDER BY sequence;"
# Expect: agent_run \n tool_call

sqlite3 "$ledger" \
  "SELECT json_extract(payload_json, '\$.input_redacted.api_key') \
     FROM ledger_records WHERE record_kind='tool_call';"
# Expect: [REDACTED:sha256:<64-hex>]

sqlite3 "$ledger" \
  "SELECT json_extract(redaction_json, '\$.redacted_fields') \
     FROM ledger_records WHERE record_kind='tool_call';"
# Expect: ["$.api_key"]
```

Mirrored automated test:
`phase1_happy_path_persists_and_inspects_physical_rows`.

### Edge Case 1 — Redaction Violation Blocks Persistence

```bash
cat > /tmp/bad-input.json <<'JSON'
{ "note": "Bearer should-not-be-persisted", "operation": "write" }
JSON

context-graph blackbox record tool-call \
  --ledger "$ledger" \
  --run-id "$RUN" \
  --tool phase1_real_tool \
  --tenant tenant_phase1 \
  --agent agent_phase1 \
  --input /tmp/bad-input.json
echo "exit=$?"
```

Expected outcome:

- exit code `1`;
- stderr JSON contains `"error_code": "BBX_REDACTION_VIOLATION"`,
  `"field_path": "$.note"`;
- physical row count unchanged: `sqlite3 "$ledger" "SELECT COUNT(*) FROM
  ledger_records;"` returns the same number as before the call.

Mirrored automated test:
`phase1_redaction_violation_blocks_persistence`.

### Edge Case 2 — Hash-Chain Tampering Detected

```bash
sqlite3 "$ledger" "DROP TRIGGER ledger_records_no_update;"
sqlite3 "$ledger" \
  "UPDATE ledger_records \
     SET payload_json='{\"record_kind\":\"tool_call\",\"tampered\":true}' \
   WHERE sequence=2;"
sqlite3 "$ledger" \
  "CREATE TRIGGER ledger_records_no_update BEFORE UPDATE ON ledger_records \
     BEGIN SELECT RAISE(ABORT, 'ledger_records is append-only; updates are forbidden'); END;"

context-graph blackbox inspect ledger --ledger "$ledger"
echo "exit=$?"
```

Expected outcome:

- exit code `2`;
- stderr JSON contains `"error_code": "BBX_HASH_CHAIN_BROKEN"`.

Mirrored automated test:
`phase1_hash_chain_detection_catches_payload_tampering`.

### Edge Case 3 — Append-Only Trigger Rejects Direct UPDATE

```bash
sqlite3 "$ledger" "UPDATE ledger_records SET record_kind='tampered' WHERE sequence=1;"
echo "exit=$?"
sqlite3 "$ledger" "SELECT record_kind FROM ledger_records WHERE sequence=1;"
```

Expected outcome:

- the `UPDATE` aborts with the SQLite error
  `ledger_records is append-only; updates are forbidden`;
- the readback shows the original `record_kind` (`agent_run`).

Mirrored automated test:
`phase1_append_only_trigger_blocks_direct_update`.

### Additional Guarded Edge Cases

The automated FSV suite also proves these Phase 1 failure paths against real
SQLite files:

- `phase1_case_insensitive_secret_prefix_blocks_persistence`: a non-sensitive
  field containing `SK-SHOULD-NOT-BE-PERSISTED` exits `1` with
  `BBX_REDACTION_VIOLATION` and the row count is unchanged.
- `phase1_cross_tenant_tool_call_is_blocked`: a tool call for `tenant_other`
  referencing a run created under `tenant_phase1_test` exits `1` with
  `BBX_TENANT_MISMATCH` and no tool-call row is written.
- `phase1_missing_parent_run_blocks_tool_call`: a tool call referencing
  `run_missing` exits `1` with `BBX_RECORD_NOT_FOUND` and no row is written.
- `phase1_invalid_risk_tier_blocks_persistence`: `--risk-tier critical` exits
  `1` with `BBX_INVALID_INPUT` and no `agent_run` row is written.
- `phase1_schema_validation_rejects_noop_append_only_trigger`: replacing the
  update trigger with a same-name no-op trigger makes `inspect ledger` exit
  `1` with `BBX_SCHEMA_VALIDATION_FAILED` at
  `schema.triggers.ledger_records_no_update.sql`.
- `phase1_init_does_not_repair_missing_append_only_trigger`: rerunning
  `blackbox init` against an already-initialized ledger with a dropped update
  trigger exits `1` with `BBX_SCHEMA_VALIDATION_FAILED`; it does not recreate
  the trigger silently.

## Invariants That Must Continue To Hold In Phases 2+

If you change Phase 1 code, you must not break any of these. If a phase
genuinely needs to break one (e.g. Phase 2 adds a new `record_kind`), bump
`LEDGER_SCHEMA_VERSION`, **wipe** the ledger, and update both this file and
`04_event_ledger_schema.md` in the same PR.

1. `ledger_records` is append-only via SQL triggers; UPDATE/DELETE always
   abort.
2. `blackbox_metadata.ledger_record_count` matches the physical row count
   after every successful append.
3. `blackbox_metadata.ledger_tip_sha256` matches the latest
   `ledger_records.record_sha256` after every successful append.
4. Each row's `payload_sha256` is `sha256(canonical(payload_json))`.
5. Each row's `record_sha256` chains over `previous_record_sha256` plus the
   row's own canonical-hashed contents.
6. Each row's `run_chain_sha256` chains over `run_previous_record_sha256`
   plus the row's run-relevant contents.
7. Tool-call rows must reference an existing `run_id` of kind `agent_run`.
8. Tool-call rows must use the same `tenant_id` as the parent `agent_run`.
9. `risk_tier` must be exactly `low`, `medium`, or `high`.
10. Physical schema validation checks columns, indexes, unique constraints,
   trigger SQL, and required metadata values; same-name no-op triggers fail.
11. Sensitive-named fields are redacted; secret-like literals in any field
   abort persistence.
12. Errors on stderr; success JSON on stdout.
13. `inspect ledger` re-runs the full integrity check and returns exit `2` on
    `BBX_HASH_CHAIN_BROKEN`.

## Running The Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase1_fsv -- --nocapture
```

Expected: `test result: ok. 10 passed; 0 failed`.

If any test fails, **the implementation is broken** — root-cause the
divergence; do not edit the test to make it green.
