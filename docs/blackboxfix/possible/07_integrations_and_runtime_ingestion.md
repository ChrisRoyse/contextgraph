# Integrations And Runtime Ingestion

BlackBoxFix can ingest and verify events from local commands, MCP, HTTP,
CloudEvents SDK files, hosted APIs, and captured provider artifacts.

## CLI Runtime

The CLI is the primary local control plane. It is suitable for:

- local development;
- CI jobs;
- one-off forensic capture;
- test harnesses;
- file-based evidence packaging;
- local private deployments.

## MCP Runtime

The JSON-RPC stdio MCP recorder/gateway can wrap tool execution. The shipped
end-to-end workflow is `verified_git_apply_patch`.

The MCP path can:

- record agent steps;
- record model decisions;
- evaluate policy;
- record tool calls;
- execute a real Git patch;
- verify the resulting diff;
- persist evidence and incidents.

## HTTP Runtime

The local HTTP server can accept authenticated requests over one ledger.

Useful for:

- local agent runtimes that prefer HTTP;
- service wrappers in a private environment;
- dashboard API access;
- request-level audit testing.

Important constraints:

- loopback/private by design;
- single-writer ledger path;
- bearer API-key authentication;
- tenant header scoping;
- body-size limits;
- request audit rows.

## SDK CloudEvents Runtime

Phase 24 adds file-based SDK ingestion through CloudEvents.

Supported event types:

- `dev.contextgraph.blackbox.agent_run.v1`
- `dev.contextgraph.blackbox.agent_step.v1`
- `dev.contextgraph.blackbox.model_decision.v1`
- `dev.contextgraph.blackbox.tool_call.v1`

Capabilities:

- ingest one event object or one batch array;
- cap batches at 128 events;
- validate CloudEvents required fields;
- require JSON data;
- validate optional W3C `traceparent`;
- enforce idempotency on `source` + `id`;
- require expected delta for mutating tool calls;
- atomically append validated batches;
- read back inserted ledger rows.

This lets external Python, TypeScript, Rust, shell, or provider-specific
wrappers produce a common event file without adding provider lock-in.

## Hosted API Integrations

The system can directly query:

- GitHub REST APIs;
- GitHub Actions workflow APIs;
- GitLab REST APIs.

These are used for live connector tests and verifier readbacks when
credentials/rate limits allow.

## Captured Artifact Integrations

The system can verify provider state from captured artifact files:

- OSV scanner JSON;
- Cargo lockfiles;
- CycloneDX SBOM JSON;
- Kubernetes Deployment snapshots;
- Terraform plan/state JSON;
- normalized cloud inventory JSON;
- normalized cloud audit JSONL;
- Jira REST issue/search JSON;
- Linear GraphQL issue JSON;
- ServiceNow Table API record/list JSON.

This means verification can run in controlled/offline settings where the
artifact is the physical evidence captured from the provider.

## Observability Integration

OpenTelemetry export gives existing observability stacks a bridge into
BlackBoxFix evidence:

- traces for ledger rows;
- logs for important failures and incidents;
- metrics for low-cardinality summary counts.

The ledger remains the source of truth; OpenTelemetry is an export path.

## Integration Pattern

Every integration follows the same pattern:

1. Define expected state.
2. Read or capture source-of-truth state.
3. Normalize observed state.
4. Compare expected and observed state.
5. Persist durable rows.
6. Export/package only after readback succeeds.
