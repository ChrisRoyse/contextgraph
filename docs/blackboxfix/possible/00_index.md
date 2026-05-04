# BlackBoxFix Demo Plan Index

This directory now contains the buyer-facing Gemma + BlackBoxFix demo plan.
The prior capability report that this plan builds on is preserved under
`original/`.

## Preserved Capability Report

- `original/00_index.md`
- `original/01_capability_overview.md`
- `original/02_cli_api_and_command_surface.md`
- `original/03_ledger_evidence_and_forensics.md`
- `original/04_source_of_truth_connectors_and_verifiers.md`
- `original/05_governance_security_privacy_and_risk.md`
- `original/06_reporting_dashboards_exports_and_packages.md`
- `original/07_integrations_and_runtime_ingestion.md`
- `original/08_workflows_now_possible.md`
- `original/09_full_state_verification_and_tested_proof.md`
- `original/10_current_boundaries.md`
- `original/11_hardening_review.md`
- `original/12_gemma_runtime_harness.md`

## New Demo Plan

- `13_demo_program_plan.md` - overall plan for showing BlackBoxFix around
  Gemma as a verified AgentOps layer.
- `14_demo_example_catalog.md` - first seven demo examples, including the five
  required starter examples.
- `15_gemma_tool_runtime_contract.md` - native Ollama/Gemma tool-calling
  contract and allowlisted tool design.
- `16_verified_deployment_fix_demo_spec.md` - flagship broken deployment
  artifact fix demo.
- `17_demo_fsv_and_test_plan.md` - manual and automated Full State
  Verification plan.
- `18_buyer_demo_script.md` - live demo narrative and screen flow.
- `19_demo_implementation_checklist.md` - implementation checklist, typed
  errors, and acceptance criteria.
- `20_agentops_research_demo_current_state.md` - current implemented state of
  the full Gemma-triggered DynamicJEPA + BlackBoxFix research demo.
- `21_agentops_manual_fsv_log.md` - manual Full State Verification evidence
  from the latest successful local run.
- `22_agentops_demo_output_explainer.html` - self-contained visual explainer
  for the latest demo outputs, what they mean, and how to present them.
- `23_correlation_intelligence_design.md` - prompt/output correlation,
  intelligence extraction, capture contract, and current gaps.
- `24_latest_correlation_intelligence_report.md` - actual latest correlation
  report values generated from the local demo run.

## Core Demo Claim

```text
Gemma can take the action. BlackBoxFix proves whether the action actually
persisted correctly.
```

The flagship demo should show Gemma using native Ollama tool calls to fix a
real broken deployment YAML file, then show BlackBoxFix independently proving
the file hash, validation result, Git diff, ledger rows, evidence exports, and
incident behavior for a deliberately bad claim.

## Current Implemented Demo

The implemented demo is broader than the original deployment-only wedge:

```text
scripts/blackboxfix_gemma_tool_demo.py
```

It uses local `gemma4:e4b` through Ollama native `/api/chat` tool calling to
invoke one allowlisted high-level tool:

```text
run_dynamicjepa_blackboxfix_pipeline
```

That tool runs the real DynamicJEPA `counter_world` verification pipeline and
BlackBoxFix proof layer. It now demonstrates:

- data ingest into a physical RocksDB source of truth;
- latent panel materialization;
- trajectory and action-conditioned dataset compilation;
- tiny CUDA training and model artifact registry;
- prediction, planning, guard decisions, and surprise recording;
- DynamicJEPA action-conditioned episode export;
- W3C PROV-style artifact provenance;
- BlackBoxFix SDK ingest, expected delta, readback verification, failed
  verifier incident creation, scorecard, evidence/replay/episode/OTEL/dashboard
  exports, and FSV certification.

Latest successful manual run:

```text
/tmp/blackboxfix-agentops-demo-manual-03
```

See `20_agentops_research_demo_current_state.md` and
`21_agentops_manual_fsv_log.md` for the exact source-of-truth readbacks. Open
`22_agentops_demo_output_explainer.html` for a human-readable walkthrough and
`23_correlation_intelligence_design.md` plus
`24_latest_correlation_intelligence_report.md` for the
intelligence/correlation model and latest measured values.
