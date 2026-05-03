# Reference Architecture

## System Overview

BlackBoxFix sits between agent runtimes and operational systems. It captures
agent decisions and tool calls, verifies results through source-of-truth
connectors, and stores evidence in an append-only ledger.

```text
agent runtime
  -> adapter / sdk
  -> policy and tool gateway
  -> tool execution
  -> source-of-truth connectors
  -> FSV verifier
  -> evidence ledger
  -> replay, dashboards, exports
```

## Concrete Layout (shipped through Phase 9)

| Layer | Shipped implementation | File path |
|---|---|---|
| CLI entrypoint (standalone) | `blackbox` binary | `crates/context-graph-cli/src/bin/blackbox.rs` |
| CLI entrypoint (umbrella) | `context-graph blackbox …` | `crates/context-graph-cli/src/main.rs:136-139,210` |
| Command implementations | `init`, `record`, `verify`, `replay`, `export evidence`, `export episodes`, `dashboard build`, `dashboard api`, `enterprise ...`, `inspect ledger` | `crates/context-graph-cli/src/commands/blackbox.rs` |
| Ledger store | SQLite WAL, sync=FULL | `ledger_records` + `blackbox_metadata` tables |
| Hash chain | SHA-256 over canonical (sorted-key) JSON | `compute_record_hash` / `compute_run_chain_hash` |
| Redaction | `redact_v1` (field-name + literal-pattern + post-scan) | `redact_tool_input` / `is_sensitive_field_name` / `looks_like_secret_literal` |
| Local source readback | Git, filesystem, command connectors | `git_local_v1`, `fs_local_v1`, `cmd_local_v1` |
| Hosted source readback | GitHub, GitHub Actions, and GitLab CI connectors | `github_v1`, `gha_v1`, `gitlab_ci_v1` |
| Evidence/replay/export | `evidence_packet`, `incident_packet`, JSON evidence, JSONL episodes, eval fixtures | `blackbox replay`, `blackbox export evidence`, `blackbox export episodes` |
| MCP/policy gateway | JSON-RPC stdio MCP recorder/gateway, policy rows, capability registry | `blackbox mcp recorder`, `blackbox mcp gateway`, `blackbox policy ...` |
| Dashboard | Static dashboard bundle plus dashboard API JSON files over the verified ledger | `blackbox dashboard build`, `blackbox dashboard api` |
| Enterprise controls | SSO/OIDC/SAML metadata, SCIM directory snapshots, RBAC/tenant-isolation decisions, audit/SIEM export | `blackbox enterprise ...` |
| FSV harness | Phase 1-9 tests against real systems/artifacts | `crates/context-graph-cli/tests/blackbox_phase*_fsv.rs` |

Anything outside Phases 1-9 is **not yet implemented**. Hosted multi-tenant
serving remains reserved for later work.

## Components (target architecture)

### Agent Adapters

Adapters instrument common agent runtimes without forcing a single framework.
Initial adapters in priority order:

1. local CLI wrapper (Phase 1: implicit — operator invokes `blackbox record …`);
2. MCP recorder server (Phase 6 shipped over JSON-RPC stdio);
3. OpenAI/Anthropic style tool-call wrapper (reserved after Phase 6);
4. coding-agent shell/git wrapper (Phase 2);
5. generic HTTP webhook ingestion (reserved after Phase 6).

The shipped MCP adapter emits `AgentRun`, `AgentStep`, `ToolCall`, and
`ModelDecision` records for the verified Git patch workflow.

### Tool Gateway (Phase 6)

The gateway is the controlled path for mutating tool calls. It performs:

- strict schema validation;
- capability lookup;
- authorization and approval checks;
- request redaction and hashing;
- execution correlation IDs;
- before-state capture when a connector supports it;
- post-execution verification dispatch.

If a mutating tool has no verifier, the platform must label it
`unverified_blocked` in governed mode.

### Source-Of-Truth Connectors (Phase 2/3/9)

Connectors read durable state from actual systems. They must support an
explicit readback operation that is separate from the mutating call path.

Examples (priority order, see `05_source_of_truth_connectors.md`):

- Git repository state;
- GitHub PRs / checks / reviews / issues / deployments;
- CI provider job and artifact state;
- package scanners and SBOM outputs;
- Kubernetes objects;
- Terraform plan/apply state;
- relational databases;
- object storage;
- ticketing systems;
- cloud audit logs.

### FSV Verifier (Phase 2/3/9)

The verifier compares an expected persisted delta against independently read
source-of-truth state. It writes `VerifierResult` and `EvidencePacket`
records.

Verifier states:

- `passed` — expected and actual match.
- `failed` — readback contradicts expected delta.
- `blocked` — verification cannot run (permission / schema / connector).
- `not_applicable` — non-mutating action.

`blocked` is a governance failure in controlled workflows, not success.

### Ledger (Phase 1 done)

Append-only and hash-chained at two levels:

- **per-record chain**: `previous_record_sha256 → record_sha256` over the full
  ordered ledger.
- **per-run chain**: `run_previous_record_sha256 → run_chain_sha256` over the
  records that share a `run_id`.

Both chains are recomputed on every `inspect ledger` run; any mismatch yields
`BBX_HASH_CHAIN_BROKEN` (exit 2). The metadata row `ledger_tip_sha256` must
match the head of the per-record chain on read; mismatch is also a hash-chain
failure.

### Replay And Forensics (Phase 4)

Replay has two modes:

- **Forensic replay** — reconstruct the timeline and evidence from stored
  records without mutating the real system again. This is the default for
  production incident response.
- **Deterministic replay** — repeat a local or sandboxable action with fixed
  inputs.

### Exporters (Phase 5)

Verified runs become:

- `episodes.jsonl` for model training and process supervision;
- eval fixtures for regression tests;
- audit packets for security and compliance;
- SIEM/SOAR events;
- OpenTelemetry traces/logs/metrics.

Pattern reference: the existing DynamicJEPA exporter at
`crates/context-graph-cli/src/commands/export_dynamicjepa.rs` plus its FSV
test at `crates/context-graph-cli/tests/dynamicjepa_bundle_export_fsv.rs`.
The Phase 5 exporter must follow the same shape: physical-row read,
manifest with content hash, JSONL re-parsed line-by-line after write.

### Dashboard (Phase 7)

The shipped dashboard is a local static bundle backed by machine-readable API
JSON files:

- `index.html`;
- `manifest.json`;
- `api/summary.json`;
- `api/runs.json`;
- `api/evidence.json`;
- `api/verifier-status.json`;
- `api/errors.json`;
- `api/exports.json`;
- `api/timeline_<run_id>.json`.

`blackbox dashboard build` verifies the SQLite hash chain, verifies
`evidence_packet.record_refs[]`, writes every file to a temp directory, reads
each JSON/HTML file back, records sha256/byte counts in the manifest, promotes
the directory, then re-reads the final manifest and files before returning
success.

### Enterprise Controls (Phase 8)

Phase 8 persists enterprise controls in the same append-only ledger:

- `enterprise_tenant_config` for SSO/OIDC/SAML metadata, RBAC roles,
  retention classes, private deployment metadata, SIEM format, and tenant
  customer-managed key id;
- `enterprise_directory_snapshot` for SCIM User/Group snapshots and group to
  role assignments;
- `enterprise_access_decision` for allowed and denied RBAC/tenant-isolation
  decisions.

`blackbox enterprise export-audit` first appends and reads back an
`enterprise_access_decision` for the export authorization attempt. It creates
no files unless an active SCIM principal has `audit.export`; when authorized,
it writes tenant-scoped `audit.jsonl`, `siem-rfc5424.log`, `controls.json`,
and `manifest.json`, then reads files back and verifies hashes before
returning success.

## Deployment Patterns

### Local Developer Mode (Phase 1)

Single binary plus local SQLite ledger. Useful for demos and developer
adoption.

### Team SaaS Mode (Phase 7)

Local team dashboard shipped in Phase 7 as a static bundle over the verified
ledger. Hosted shared-ledger serving remains a later deployment concern.

### Enterprise Private Mode (Phase 8)

Customer-controlled deployment: private network connectors, customer-owned
encryption keys, SSO/RBAC, long retention, SIEM export.

## Data Flow For Mutating Actions (target)

1. Agent proposes a tool call.
2. Adapter emits `ModelDecision` and pending `ToolCall`.
3. Gateway validates schema and policy.
4. Connector captures pre-state.
5. Tool executes.
6. Connector reads source of truth through a separate path.
7. Verifier compares expected delta to readback.
8. Ledger persists result, evidence, score, and remediation.
9. Exporter updates training/eval corpus.

Phase 1 implemented the local ledger/CLI recorder. Phase 2 implemented local
source-of-truth readback and verification for Git, filesystem, and command
artifacts. Hosted GitHub/CI readback shipped in Phase 3. Policy/gateway
pre-state orchestration shipped in Phase 6 for the local
`verified_git_apply_patch` workflow. Phase 7 shipped dashboard read models and
static UI artifacts over the verified ledger. Phase 8 shipped enterprise
control rows and audit/SIEM export over tenant-scoped verified ledger state.

## Design Constraint

Do not depend on agent honesty. The agent can claim success, but the verifier
must read the system of record and prove it.

## FSV Of This Document

A fresh-context AI must be able to map every component above to either:

- an existing path inside `crates/context-graph-cli/src/commands/blackbox.rs`,
  or
- an explicit "Phase N" tag indicating the component is **not yet built**.

If neither holds, this doc is wrong and must be updated before code lands.
