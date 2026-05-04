# Capability Overview

BlackBoxFix is currently capable of running as a local, evidence-first
AgentOps recorder and verifier. It is built around a physical SQLite ledger,
real source-of-truth readbacks, typed failures, and Full State Verification.

## Core Capabilities

### Agent Activity Recording

The system can persist:

- agent runs;
- agent steps;
- model tool-choice decisions;
- tool calls;
- policy decisions;
- verifier results;
- evidence packets;
- incident packets;
- enterprise controls;
- scorecards;
- privacy decisions;
- release gates;
- pilot packages;
- risk assessments;
- HTTP request audit rows;
- first-wedge demo rows;
- executable FSV certification rows.

Every persisted record is tenant-scoped, schema-versioned, canonicalized,
redacted, hash-chained, and stored in `ledger_records`.

### Independent Verification

BlackBoxFix can compare an expected state change against an independently read
source of truth. The source of truth may be:

- a local Git repository;
- a local file;
- a command result and artifact files;
- GitHub repository, pull request, check, issue, and workflow state;
- GitLab project, pipeline, and job state;
- scanner artifacts such as OSV, lockfiles, and CycloneDX SBOMs;
- Kubernetes Deployment JSON snapshots;
- Terraform plan/state JSON;
- normalized cloud inventory and audit JSONL;
- ticketing API artifacts from Jira, Linear, and ServiceNow;
- SQLite ledger rows;
- generated package/export files.

### Fail-Closed Governance

The system does not treat missing proof as success. Failures are typed as:

- `passed` when expected and actual state match;
- `failed` when the source of truth contradicts the expected delta;
- `blocked` when verification cannot run due to permission, schema, missing
  source, invalid input, or connector failure.

Blocked verification is a governance failure in controlled workflows.

### Evidence And Forensics

BlackBoxFix can reconstruct run timelines, export evidence reports, create
incident packets for failed verification, and produce replayable or auditable
records without rerunning the original action.

### Product And Buyer Proof

The system can generate buyer-facing and operator-facing artifacts:

- static dashboard bundle;
- API-style dashboard JSON files;
- evidence reports;
- JSONL training/eval episodes;
- pilot reports;
- usage meter JSONL;
- eval-candidate JSONL;
- first-wedge demo package;
- FSV certification package;
- OpenTelemetry-compatible traces, logs, and metrics.

### Runtime Ingestion

BlackBoxFix can accept data from multiple runtime surfaces:

- CLI commands;
- JSON-RPC stdio MCP recorder/gateway;
- authenticated loopback HTTP server;
- provider-neutral CloudEvents SDK JSON ingestion.

Phase 24 means external language SDKs can target a stable event envelope even
before packaged Python or TypeScript clients exist.

## What This Means In Practice

The system can now support a full local proof loop:

1. Record the agent task and proposed action.
2. Capture expected delta.
3. Execute or ingest the action.
4. Read the physical source of truth separately.
5. Persist verifier result and evidence.
6. Score the run.
7. Apply privacy, release, and risk decisions.
8. Package evidence for dashboards, buyers, audits, evals, or telemetry.
9. Certify Full State Verification evidence by reading the source state again.

## Current Operating Model

BlackBoxFix is strongest as a local/private deployment component. It is not
yet a hosted multi-tenant SaaS control plane. It expects operators or tests to
provide real local files, real API credentials, or captured provider artifacts
when a connector needs external state.
