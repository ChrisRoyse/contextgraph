# CLI, API, MCP, And SDK Surface

BlackBoxFix exposes its capabilities through the standalone `blackbox` binary
and through the umbrella `context-graph blackbox ...` command path.

## Primary CLI Commands

### Ledger

```bash
blackbox init --ledger PATH
blackbox inspect ledger --ledger PATH [--run-id RUN_ID] [--limit N]
```

The ledger commands initialize and inspect the physical SQLite source of truth.
Inspection recomputes record and run hash chains and validates metadata.

### Recording

```bash
blackbox record run --ledger PATH --task TASK --agent AGENT
blackbox record step --ledger PATH --run-id RUN_ID --summary TEXT
blackbox record model-decision --ledger PATH --run-id RUN_ID --tool-name TOOL --capability CAP --input INPUT_JSON
blackbox record tool-call --ledger PATH --run-id RUN_ID --tool TOOL --input INPUT_JSON
```

These commands persist core agent/run/tool events with redaction and hash-chain
integrity.

### Verification

```bash
blackbox verify --ledger PATH --input SPEC_JSON
```

`verify` executes connector-specific readback and comparison. It persists
expected delta, readback snapshot, verifier result, evidence packet, and
incident packet rows as appropriate.

### Replay And Evidence Export

```bash
blackbox replay --ledger PATH --run-id RUN_ID
blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE
blackbox export episodes --ledger PATH --out DIR [--force]
blackbox export otel --ledger PATH --out DIR [--force]
```

These commands reconstruct timelines and export durable proof without trusting
agent return values.

### Dashboard

```bash
blackbox dashboard build --ledger PATH --out DIR [--force]
blackbox dashboard api --ledger PATH --view VIEW [--run-id RUN_ID]
```

The dashboard system writes static files and machine-readable API JSON from
verified ledger state.

### Policy And MCP

```bash
blackbox policy evaluate --ledger PATH --mode MODE --tool TOOL ...
blackbox policy registry [--ledger PATH]
blackbox mcp recorder --ledger PATH
blackbox mcp gateway --ledger PATH --policy-mode MODE
```

The policy/MCP surface supports governed tool execution and the shipped
`verified_git_apply_patch` workflow.

### Enterprise, Privacy, Release, Package, Risk

```bash
blackbox enterprise configure --ledger PATH --tenant TENANT --input CONFIG --agent AGENT
blackbox enterprise sync-directory --ledger PATH --tenant TENANT --input SCIM_JSON --agent AGENT
blackbox enterprise access-evaluate --ledger PATH --tenant TENANT --principal-id PRINCIPAL --permission PERM --resource RESOURCE --agent AGENT
blackbox enterprise export-audit --ledger PATH --tenant TENANT --principal-id PRINCIPAL --out DIR [--force] --agent AGENT

blackbox score run --ledger PATH --run-id RUN_ID
blackbox privacy evaluate-retention --ledger PATH --tenant TENANT --run-id RUN_ID ...
blackbox privacy request-erasure --ledger PATH --tenant TENANT --run-id RUN_ID ...
blackbox gate evaluate --ledger PATH --tenant TENANT --run-id RUN_ID ...
blackbox package pilot-report --ledger PATH --tenant TENANT --principal-id PRINCIPAL --pilot-id PILOT --out DIR
blackbox risk evaluate --ledger PATH --tenant TENANT --principal-id PRINCIPAL --decisions DECISIONS_JSON
```

These commands turn verified evidence into operational decisions for
governance, privacy, release readiness, commercial pilot packaging, and risk
closure.

### Demo And FSV Certification

```bash
blackbox demo first-wedge --ledger PATH --tenant TENANT --run-id RUN_ID --out DIR
blackbox fsv certify --ledger PATH --tenant TENANT --phase N --evidence EVIDENCE_JSON --out DIR
```

These commands produce buyer-demo and FSV proof packages backed by physical
ledger/file readback.

### SDK Ingestion

```bash
blackbox sdk schema [--ledger PATH]
blackbox connector test --connector blackbox_sdk_ingest_v1 --source CLOUDEVENTS_JSON
blackbox sdk ingest --ledger PATH --input CLOUDEVENTS_JSON
```

Phase 24 supports strict CloudEvents JSON object or batch ingestion into
existing `agent_run`, `agent_step`, `model_decision`, and `tool_call` rows.

## Connector Test Surface

The system can test these connectors directly:

- `git_local_v1`
- `fs_local_v1`
- `cmd_local_v1`
- `github_v1`
- `gha_v1`
- `gitlab_ci_v1`
- `scanner_artifact_v1`
- `k8s_deployment_snapshot_v1`
- `terraform_cloud_artifact_v1`
- `ticketing_artifact_v1`
- `blackbox_otel_export_v1`
- `enterprise_controls_v1`
- `blackbox_scoring_v1`
- `blackbox_privacy_v1`
- `blackbox_release_gate_v1`
- `blackbox_packaging_v1`
- `blackbox_http_server_v1`
- `blackbox_first_wedge_demo_v1`
- `blackbox_fsv_protocol_v1`
- `blackbox_sdk_ingest_v1`

## Local HTTP API

The shipped HTTP server is a loopback authenticated API over a single ledger.
It supports:

- health checks;
- run recording;
- step recording;
- tool-call recording;
- tenant-scoped dashboard summary/runs/timeline/evidence/verifier/errors;
- request audit rows for protected requests;
- API-key authentication;
- tenant-scope headers;
- max body size enforcement;
- typed auth, tenant, schema, and body-size failures.

The server is not yet a public hosted SaaS API. It is a local/private
single-writer server over one ledger.

## MCP Surface

The MCP recorder/gateway supports JSON-RPC stdio and the governed
`verified_git_apply_patch` workflow. It records the reasoning/action path,
applies a real Git patch, runs verification through `git_file_diff_matches_patch_v1`,
and persists the evidence.

## Machine-Readable API Registry

`blackbox api surface [--ledger PATH]` returns a machine-readable registry of
the available HTTP/API/command surface, supported phase features, connectors,
capabilities, and policy modes.

The registry is now fail-closed about HTTP availability:

- `endpoints` and `served_http_endpoints` contain only routes currently served
  by `blackbox server serve`.
- `planned_http_endpoints` contains future or command-only HTTP shapes with
  `implemented=false`, the CLI command equivalent, and the expected 404 status
  until the route is actually implemented.
- The source of truth for served-route proof is the real HTTP router plus
  SQLite `ledger_records` audit/readback, not the registry return value alone.
