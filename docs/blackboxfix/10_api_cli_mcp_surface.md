# API, CLI, MCP, And SDK Surface

BlackBoxFix exposes the same core semantics through CLI, API, SDK, and MCP
wrappers. The product surface makes the FSV lifecycle explicit.

Phase 1 ships only the CLI. The HTTP API, MCP servers, and SDKs are reserved
for Phase 6+.

## CLI (shipped Phase 1)

Two binaries, identical semantics for the BlackBoxFix subset:

| Binary | Path | Invocation |
|---|---|---|
| Standalone | `crates/context-graph-cli/src/bin/blackbox.rs` | `blackbox <subcommand>` |
| Umbrella | `crates/context-graph-cli/src/bin/context-graph.rs` | `context-graph blackbox <subcommand>` |

Build via `cargo build --release -p context-graph-cli`. Both binaries land in
`target/release/`.

### Phase 1 Commands (verified by FSV tests)

```text
blackbox init                  --ledger PATH
blackbox record run            --ledger PATH --task TASK --agent AGENT
                               [--tenant TENANT]    (default: tenant_local)
                               [--model MODEL]      (default: null)
                               [--task-kind KIND]   (default: local_agent_task)
                               [--risk-tier TIER]   (default: medium)
blackbox record tool-call      --ledger PATH --run-id RUN_ID --tool TOOL
                               --input PATH         (path to JSON file)
                               [--tenant TENANT]    (default: tenant_local)
                               [--agent AGENT]      (default: agent_unknown)
                               [--model MODEL]
                               [--capability CAP]   (default: local.tool)
                               [--status STATUS]    (ok|failed|blocked, default: ok)
                               [--external-request-id ID]
blackbox inspect ledger        --ledger PATH
                               [--run-id RUN_ID]
                               [--limit N]
```

The `--ledger` flag also accepts the env var `BLACKBOX_LEDGER`.

`-v`/`-vv`/`-vvv` increase log verbosity (warn/info/debug/trace, written to
stderr). Stdout always carries the JSON result on success.

### Phase 2/3/4/5 Commands (planned, not implemented)

```text
blackbox connector list
blackbox connector test        --connector ID --source LOCATOR
blackbox verify                --run-id RUN_ID
blackbox replay                --run-id RUN_ID
blackbox export episodes       --ledger PATH --out DIR
blackbox export evidence       --run-id RUN_ID --out FILE
```

Implement each in priority order; do not attempt to land all at once.

## CLI Failure Shape (Phase 1 enforced)

Every failure prints a JSON object on **stderr** and exits non-zero:

```json
{
  "status": "error",
  "error_code": "BBX_HASH_CHAIN_BROKEN",
  "message": "...",
  "source_of_truth": "sqlite:/tmp/bbx.sqlite#ledger_records",
  "connector": "sqlite_local_v1",
  "field_path": "ledger_records[2].payload_sha256",
  "remediation": "treat the ledger as tampered or corrupt; restore from trusted backup or start a new ledger"
}
```

Exit codes:

- `0` — success.
- `2` — `BBX_HASH_CHAIN_BROKEN` (treat as integrity emergency).
- `1` — every other typed error.

The full error-code list is in `20_phase1_state_and_invariants.md`.

## CLI Success Shape (Phase 1)

Every success prints a JSON object on stdout:

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

`record run` and `record tool-call` additionally return the new
`record_id`, `run_id` (and `tool_call_id` for tool calls), `ledger_sequence`,
`payload_sha256`, `previous_record_sha256`, `run_chain_sha256`, and
`record_sha256`.

`inspect ledger` returns `integrity` plus a `records` array with each row's
fields and hashes.

## HTTP API (Phase 6 target)

Core endpoints (none implemented yet):

```text
POST /v1/runs
GET  /v1/runs/{run_id}
POST /v1/runs/{run_id}/steps
POST /v1/tool-calls
POST /v1/tool-calls/{tool_call_id}/verify
GET  /v1/tool-calls/{tool_call_id}/evidence
POST /v1/replay
POST /v1/exports/episodes
GET  /v1/connectors
POST /v1/connectors/{connector_id}/test
GET  /v1/policies
POST /v1/policies/evaluate
```

Write endpoints return the created ledger ids. They must not claim the action
is correct unless verifier evidence exists.

## MCP Surface (Phase 6 target)

Two server modes:

### Recorder MCP Server

Tools (no execution side-effects on real systems):

```text
blackbox_start_run
blackbox_record_decision
blackbox_record_tool_call
blackbox_verify_tool_call
blackbox_get_evidence
blackbox_export_episodes
```

### Gateway MCP Server

Tools that wrap external systems and **always** produce verifier evidence:

```text
verified_github_create_pr
verified_git_apply_patch
verified_ci_wait_for_checks
verified_ticket_update
verified_k8s_rollout
```

Each gateway tool has strict schemas and returns source-of-truth ids plus
verifier results.

## SDK (Phase 6 target)

Start with Python and TypeScript:

```python
with blackbox.run(task="fix vulnerability", agent_id="agent_coder") as run:
    call = run.tool_call("git_apply_patch", input=patch)
    result = call.execute()
    evidence = call.verify(expected_delta=expected)
```

SDK constraints:

- all mutating calls require expected delta;
- all verifiers return typed result;
- recorder works without model-provider lock-in;
- redaction hooks run before persistence;
- source-of-truth readback is explicit.

## OpenTelemetry Export (Phase 7)

OpenTelemetry is an export path, not the primary source of truth. Emit:

- trace spans for run, step, tool call, verifier;
- logs for failures and remediation;
- metrics for pass rates, blocked actions, risk scores, latency;
- span attributes for connector, source kind, tool capability, risk tier.

The ledger remains authoritative.

## Versioning

- APIs are versioned by URL.
- Ledger records include `schema_version` (= 1 in Phase 1).
- Verifier specs include `verifier_id` with version suffix.
- Connectors include `connector_id` with version suffix
  (`sqlite_local_v1` today).
- Export manifests record all versions.
- Schema bumps wipe the ledger; no migration shims.

## FSV Of The CLI Surface

For each Phase 1 command, the FSV protocol is:

1. Run the command with a real synthetic input.
2. Read the SQLite source of truth via the `sqlite3` CLI.
3. Confirm row counts, payload structure, and hash-chain consistency match.

Concrete inputs and expected outputs live in
`20_phase1_state_and_invariants.md`. The automated equivalent is
`crates/context-graph-cli/tests/blackbox_phase1_fsv.rs` (4 tests, all PASS).
