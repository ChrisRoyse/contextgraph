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

## Concrete Layout (shipped through Phase 24)

| Layer | Shipped implementation | File path |
|---|---|---|
| CLI entrypoint (standalone) | `blackbox` binary | `crates/context-graph-cli/src/bin/blackbox.rs` |
| CLI entrypoint (umbrella) | `context-graph blackbox …` | `crates/context-graph-cli/src/main.rs:136-139,210` |
| Command implementations | `init`, `record`, `sdk`, `verify`, `replay`, `export evidence`, `export episodes`, `export otel`, `dashboard build`, `dashboard api`, `enterprise ...`, `score ...`, `privacy ...`, `gate ...`, `package ...`, `risk ...`, `demo ...`, `inspect ledger` | `crates/context-graph-cli/src/commands/blackbox.rs` |
| Ledger store | SQLite WAL, sync=FULL | `ledger_records` + `blackbox_metadata` tables |
| Hash chain | SHA-256 over canonical (sorted-key) JSON | `compute_record_hash` / `compute_run_chain_hash` |
| Redaction | `redact_v1` (field-name + literal-pattern + post-scan) | `redact_tool_input` / `is_sensitive_field_name` / `looks_like_secret_literal` |
| Local source readback | Git, filesystem, command connectors | `git_local_v1`, `fs_local_v1`, `cmd_local_v1` |
| Hosted source readback | GitHub, GitHub Actions, and GitLab CI connectors | `github_v1`, `gha_v1`, `gitlab_ci_v1` |
| Scanner artifact readback | OSV JSON report, Cargo lockfile, and CycloneDX JSON SBOM connector | `scanner_artifact_v1` |
| Ticket evidence readback | GitHub issue body/comment evidence-link verification | `github_issue_evidence_link_v1` on `github_v1` |
| Evidence/replay/export | `evidence_packet`, `incident_packet`, JSON evidence, JSONL episodes, eval fixtures | `blackbox replay`, `blackbox export evidence`, `blackbox export episodes` |
| MCP/policy gateway | JSON-RPC stdio MCP recorder/gateway, policy rows, capability registry | `blackbox mcp recorder`, `blackbox mcp gateway`, `blackbox policy ...` |
| Dashboard | Static dashboard bundle plus dashboard API JSON files over the verified ledger | `blackbox dashboard build`, `blackbox dashboard api` |
| Enterprise controls | SSO/OIDC/SAML metadata, SCIM directory snapshots, RBAC/tenant-isolation decisions, audit/SIEM export | `blackbox enterprise ...` |
| Scorecards | Deterministic run-level trust/risk/readiness score over physical ledger rows | `blackbox score run` |
| Privacy controls | RBAC-gated retention and erasure-request decisions over enterprise config/directory rows | `blackbox privacy ...` |
| Release gates | RBAC-gated release/promotion approval or block decisions over scorecards, evidence, incidents, and privacy state | `blackbox gate evaluate` |
| Pilot packaging | RBAC-gated pilot report, usage-meter JSONL, eval-candidate JSONL, and manifest over physical ledger rows | `blackbox package pilot-report` |
| Risk assessment | RBAC-gated risk/open-question assessment over ledger, runtime, package, incident/eval, and decision-register evidence | `blackbox risk evaluate` |
| HTTP server | Authenticated loopback HTTP server, single-writer ledger path, tenant-scoped dashboard reads, request audit rows | `blackbox server serve` |
| First-wedge demo package | Manifest-verified buyer-demo package over real Git/scanner/GitHub/GHA evidence and required failure demos | `blackbox demo first-wedge` |
| Executable FSV certification | Validates captured FSV evidence against physical SQLite/file state and persists certification rows | `blackbox fsv certify` |
| Kubernetes deployment readback | Physical `kubectl get deployments.v1.apps -o json` snapshots for image digest, replica, and rollout verification | `k8s_deployment_snapshot_v1` |
| Terraform/cloud artifact readback | Physical `terraform show -json` plan/state files plus normalized cloud inventory JSON and audit JSONL artifacts | `terraform_cloud_artifact_v1` |
| Ticketing artifact readback | Physical Jira REST, Linear GraphQL, and ServiceNow Table API JSON artifacts for ticket state and evidence-link verification | `ticketing_artifact_v1` |
| OpenTelemetry export readback | Physical OTLP JSONL traces/logs/metrics plus manifest hashes exported from a verified ledger | `blackbox_otel_export_v1` |
| SDK event ingestion | Provider-neutral CloudEvents JSON object/batch ingestion with source/id idempotency, W3C traceparent validation, expected-delta enforcement for mutating calls, and SQLite readback | `blackbox sdk ingest`, `blackbox_sdk_ingest_v1` |
| FSV harness | Phase 1-24 tests against real systems/artifacts | `crates/context-graph-cli/tests/blackbox_phase*_fsv.rs` |

Anything outside Phases 1-24 is **not yet implemented**. Public multi-tenant
SaaS serving, packaged language SDK clients, and provider-managed TLS remain
reserved for later work.

## Components (target architecture)

### Agent Adapters

Adapters instrument common agent runtimes without forcing a single framework.
Initial adapters in priority order:

1. local CLI wrapper (Phase 1: implicit — operator invokes `blackbox record …`);
2. MCP recorder server (Phase 6 shipped over JSON-RPC stdio);
3. provider-neutral CloudEvents SDK ingestion bridge (Phase 24 shipped);
4. coding-agent shell/git wrapper (Phase 2);
5. generic HTTP webhook ingestion (reserved after Phase 6).

The shipped MCP adapter emits `AgentRun`, `AgentStep`, `ToolCall`, and
`ModelDecision` records for the verified Git patch workflow. The shipped SDK
ingestion bridge accepts strict CloudEvents JSON files from any language
runtime and persists the same ledger record families after file and ledger
readback validation.

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

### Source-Of-Truth Connectors (Phase 2/3/9/10/11/12/13/14/15/16/17/18/20/21/22/24)

Connectors read durable state from actual systems. They must support an
explicit readback operation that is separate from the mutating call path.

Examples (priority order, see `05_source_of_truth_connectors.md`):

- Git repository state;
- GitHub PRs / checks / reviews / issues / deployments;
- CI provider job and artifact state;
- package scanners and SBOM outputs;
- Kubernetes Deployment objects from physical JSON snapshots;
- Terraform plan/apply state from physical `terraform show -json` artifacts;
- normalized cloud inventory and audit artifacts;
- ticketing API artifacts from Jira, Linear, and ServiceNow;
- relational databases;
- object storage;
- cloud audit logs.

### FSV Verifier (Phase 2/3/9/10/11/12/13/14/15/16/20/21/22)

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
- `api/pilot-packages.json`;
- `api/risk-assessments.json`;
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

### Scorecards (Phase 12)

`blackbox score run` reads the physical SQLite ledger after integrity
verification, verifies evidence packet references, computes deterministic
trust/risk dimensions, appends one `scorecard` row, and reads it back by
sequence, `scorecard_id`, `run_id`, `payload_sha256`, and `record_sha256`.
Scorecards exclude previous `scorecard` and later `release_gate` governance
rows from their input set so the evidence score stays stable.

### Privacy Decisions (Phase 13)

`blackbox privacy evaluate-retention` and `blackbox privacy request-erasure`
verify tenant config, SCIM directory state, parent runs, and RBAC decisions
from the ledger before appending `retention_decision` or `evidence_deletion`
rows. Unauthorized attempts persist the denied authorization row and create no
privacy decision row.

### Release Gates (Phase 14)

`blackbox gate evaluate` is the executable promotion gate. It verifies the
ledger, parent run, `release.approve` RBAC decision, latest scorecard,
evidence reference integrity, verifier status, incident state, policy state,
and optional privacy retention state. Approved and blocked gates both append a
`release_gate` row and read it back. Invalid requests and unauthorized
principals fail closed before any gate row is written.

### Pilot Packaging (Phase 15)

`blackbox package pilot-report` is the buyer-facing packaging path for a
pilot. It verifies the ledger, persists and reads back a
`commercial.package` authorization decision, computes pilot success metrics
from physical tenant rows, writes `pilot-report.json`,
`usage-meter-events.jsonl`, `eval-candidates.jsonl`, and `manifest.json`,
re-reads each file, appends a `pilot_package` row, reads that row back by
sequence/id/hash, promotes the package directory, and re-checks final file
hashes. Packages with unmet success criteria are durable evidence and exit
non-zero; invalid requests and unauthorized principals create no package row
or files.

### Risk Assessments (Phase 16)

`blackbox risk evaluate` closes the prose-risk gap. It validates a real
decision-register JSON file, verifies the physical ledger and SQLite runtime
durability settings, persists and reads back a `risk.review` authorization
decision, checks source-of-truth coverage, verifier/policy status, enterprise
controls, release gates, pilot-package evidence, incident-to-eval conversion,
secret findings, and required open-question closure, appends a
`risk_assessment` row, and reads it back by sequence/id/hash. Accepted and
blocked assessments are both durable rows; invalid decision files and
unauthorized principals create no risk row.

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
Phase 9 shipped hosted GitLab CI readback. Phase 10 shipped scanner artifact
readback for OSV JSON reports, Cargo lockfiles, and CycloneDX JSON SBOMs.
Phase 11 shipped GitHub issue evidence-link readback for ticket proof. Phase
12 shipped deterministic scorecards. Phase 13 shipped privacy retention and
erasure decisions. Phase 14 shipped release/promotion gates over the verified
ledger state. Phase 15 shipped pilot packaging, usage-meter rows, eval
candidate rows, and dashboard package read models. Phase 16 shipped
risk-assessment rows, required open-question decision closure, and dashboard
risk read models.

## Design Constraint

Do not depend on agent honesty. The agent can claim success, but the verifier
must read the system of record and prove it.

## FSV Of This Document

A fresh-context AI must be able to map every component above to either:

- an existing path inside `crates/context-graph-cli/src/commands/blackbox.rs`,
  or
- an explicit "Phase N" tag indicating the component is **not yet built**.

If neither holds, this doc is wrong and must be updated before code lands.
