# BlackBoxFix Capabilities Report

This directory summarizes what the shipped BlackBoxFix implementation can do
after Phase 24. It is written as an operator/product capability map, not as a
future roadmap.

## Current Status

BlackBoxFix is now a local-first Verified AgentOps system inside
`context-graph-cli`. It can record agent activity, verify claimed outcomes
against physical sources of truth, persist tamper-evident evidence, enforce
policy/governance decisions, export proof artifacts, generate dashboards,
serve a local authenticated HTTP API, ingest provider-neutral SDK events, and
certify Full State Verification evidence.

The authoritative source of truth for BlackBoxFix itself is the SQLite ledger
at `--ledger PATH`, especially the append-only `ledger_records` table and
`blackbox_metadata` rows.

## Report Files

- `01_capability_overview.md` - high-level system capability summary.
- `02_cli_api_and_command_surface.md` - commands, local HTTP API, MCP, and SDK
  ingestion surfaces.
- `03_ledger_evidence_and_forensics.md` - ledger, hash chain, redaction,
  replay, evidence, and incident capabilities.
- `04_source_of_truth_connectors_and_verifiers.md` - connectors and verifier
  families now available.
- `05_governance_security_privacy_and_risk.md` - enterprise controls, policy,
  privacy, release gates, risk, and audit capabilities.
- `06_reporting_dashboards_exports_and_packages.md` - dashboard, evidence,
  episode, OpenTelemetry, pilot, demo, and FSV package outputs.
- `07_integrations_and_runtime_ingestion.md` - MCP gateway, HTTP server,
  CloudEvents SDK ingest, hosted/local artifact integrations.
- `08_workflows_now_possible.md` - concrete workflows the system can execute
  end to end.
- `09_full_state_verification_and_tested_proof.md` - shipped FSV coverage and
  how success is proven.
- `10_current_boundaries.md` - what is intentionally not shipped yet or still
  constrained.
- `11_hardening_review.md` - latest cross-phase hardening result and the API
  contract issue that was closed.

## One-Screen Summary

BlackBoxFix can now answer:

- What did the agent do?
- Which tool did it choose and why?
- What did it claim changed?
- What did the real source of truth show after the action?
- Did verification pass, fail, or block?
- Which incidents, risks, gates, privacy decisions, and audit events exist?
- Can a buyer, security team, or release owner inspect durable proof?
- Can verified evidence be exported for dashboards, evals, pilot reports,
  OpenTelemetry, and FSV certification?
- Can external runtimes send SDK events without being tied to one model
  provider?

## Canonical Implementation Pointers

- Main implementation:
  `crates/context-graph-cli/src/commands/blackbox.rs`
- Standalone binary:
  `crates/context-graph-cli/src/bin/blackbox.rs`
- Umbrella binary:
  `context-graph blackbox ...`
- Phase FSV tests:
  `crates/context-graph-cli/tests/blackbox_phase*_fsv.rs`
- Phase 24 SDK FSV:
  `crates/context-graph-cli/tests/blackbox_phase24_sdk_ingest_fsv.rs`
- Cross-phase API hardening FSV:
  `crates/context-graph-cli/tests/blackbox_cross_phase_api_surface_hardening_fsv.rs`
- Main planning index:
  `docs/blackboxfix/00_master_index.md`
