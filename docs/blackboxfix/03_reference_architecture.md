# Reference Architecture

## System Overview

BlackBoxFix sits between agent runtimes and operational systems. It captures
agent decisions and tool calls, verifies results through source-of-truth
connectors, and stores evidence in an append-only ledger.

```text
agent runtime
  -> adapter/sdk
  -> policy and tool gateway
  -> tool execution
  -> source-of-truth connectors
  -> FSV verifier
  -> evidence ledger
  -> replay, dashboards, exports
```

## Components

### Agent Adapters

Adapters instrument common agent runtimes without forcing a single framework.
Initial adapters should include:

- local CLI wrapper;
- MCP client/server wrapper;
- OpenAI/Anthropic style tool-call wrapper;
- coding-agent shell/git wrapper;
- generic HTTP webhook ingestion.

Each adapter emits `AgentRun`, `AgentStep`, `ToolCall`, and `ModelDecision`
records.

### Tool Gateway

The gateway is the controlled path for mutating tool calls. It performs:

- strict schema validation;
- capability lookup;
- authorization and approval checks;
- request redaction and hashing;
- execution correlation IDs;
- before-state capture when a connector supports it;
- post-execution verification dispatch.

If a mutating tool has no verifier, the platform should label it
`unverified_blocked` in governed mode.

### Source-Of-Truth Connectors

Connectors read durable state from actual systems. They must support an explicit
readback operation that is separate from the mutating call path.

Examples:

- Git repository state;
- GitHub pull requests, checks, reviews, issues, deployments;
- CI provider job and artifact state;
- package scanners and SBOM outputs;
- Kubernetes objects;
- Terraform plan/apply state;
- relational databases;
- object storage;
- ticketing systems;
- cloud audit logs.

### FSV Verifier

The verifier compares an expected persisted delta against independently read
source-of-truth state. It writes `VerifierResult` and `EvidencePacket` records.

Verifier states:

- `passed`: expected and actual match.
- `failed`: source-of-truth readback contradicts expected delta.
- `blocked`: verification cannot run because permission, schema, or connector
  state is invalid.
- `not_applicable`: the action is non-mutating and has no persisted delta.

`blocked` is not success. It is a governance failure in controlled workflows.

### Ledger

The ledger is append-only and hash-chained by run and by tenant. Records are
schema-versioned. Payloads that may contain sensitive data are redacted and
stored as digests plus optional encrypted evidence references.

### Replay And Forensics

Replay has two modes:

- Deterministic replay: repeat a local or sandboxable action with fixed inputs.
- Forensic replay: reconstruct the timeline and evidence from stored records
  without mutating the real system again.

Production incident response should default to forensic replay.

### Exporters

Exporters turn verified runs into:

- `episodes.jsonl` for model training and process supervision;
- eval fixtures for regression tests;
- audit packets for security and compliance;
- SIEM/SOAR events;
- OpenTelemetry traces/logs/metrics for existing observability backends.

## Deployment Patterns

### Local Developer Mode

Single binary plus local ledger:

- records CLI or agent actions;
- verifies file/Git/test outcomes;
- exports JSONL;
- useful for demos and developer adoption.

### Team SaaS Mode

Hosted control plane:

- shared ledger;
- GitHub/CI/ticketing integrations;
- dashboard;
- team policy;
- eval exporter.

### Enterprise Private Mode

Customer-controlled deployment:

- private network connectors;
- customer-owned encryption keys;
- SSO and RBAC;
- long retention;
- SIEM export;
- no outbound payload access unless configured.

## Data Flow For Mutating Actions

1. Agent proposes a tool call.
2. Adapter emits `ModelDecision` and pending `ToolCall`.
3. Gateway validates schema and policy.
4. Connector captures pre-state.
5. Tool executes.
6. Connector reads source of truth through a separate path.
7. Verifier compares expected delta to readback.
8. Ledger persists result, evidence, score, and remediation.
9. Exporter updates training/eval corpus.

## Design Constraint

Do not depend on agent honesty. The agent can claim success, but the verifier
must read the system of record and prove it.

