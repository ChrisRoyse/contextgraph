# BlackBoxFix — Verified AgentOps Master Index

> Codename for a Verified AgentOps platform: a black-box recorder and verifier
> for AI agents. Records every agent action, verifies the claimed effect against
> the physical source of truth, scores the action, supports replay/incidents,
> and exports verified episodes for training/evals.

This index is the **canonical entry point**. A fresh-context AI agent should
read this file first, then `13_mvp_implementation_phases.md`, then
`19_full_state_verification_protocol.md` before touching code.

## Repository State (as of 2026-05-03)

- **Repo root**: `/home/cabdru/contextgraph`
- **Workspace**: 12 crates; BlackBoxFix lives inside `context-graph-cli`.
- **Phase 0 (Foundation)**: COMPLETE — this doc set, schema, and contracts.
- **Phase 1 (Local Ledger + CLI)**: COMPLETE and verified.
  - Implementation: `crates/context-graph-cli/src/commands/blackbox.rs`
  - Standalone binary entry: `crates/context-graph-cli/src/bin/blackbox.rs`
  - Subcommand wiring (within `context-graph` binary): `crates/context-graph-cli/src/main.rs:136-139, 210`
  - FSV test suite: `crates/context-graph-cli/tests/blackbox_phase1_fsv.rs`
    (10/10 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase1_fsv`).
  - Source of truth: SQLite file at `--ledger PATH`, table `ledger_records`,
    append-only via triggers `ledger_records_no_update`/`_no_delete`.
- **Phase 2 (Git/File/Command Verification)**: COMPLETE and verified.
  - Connectors: `git_local_v1`, `fs_local_v1`, `cmd_local_v1`.
  - Verifiers: `git_branch_exists_v1`, `git_commit_exists_v1`,
    `git_file_diff_matches_patch_v1`, `git_working_tree_clean_v1`,
    `cmd_exit_zero_v1`.
  - Phase 2 originally introduced schema v2 record kinds. The current shipped
    ledger schema is `LEDGER_SCHEMA_VERSION = 13` after Phase 23; old ledgers are
    wiped on init, never migrated in-place.
  - FSV test suite: `crates/context-graph-cli/tests/blackbox_phase2_fsv.rs`
    (19/19 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase2_fsv`).
- **Phase 3 (GitHub + GitHub Actions Verification)**: COMPLETE and verified.
  - Connectors: `github_v1`, `gha_v1`.
  - Verifiers: `github_pr_state_v1`,
    `github_branch_head_matches_commit_v1`,
    `github_check_suite_passed_v1`, `github_issue_state_changed_v1`,
    `gha_workflow_run_completed_v1`.
  - Phase 3 added no new record kinds. Current ledgers run under schema v13
    after Phase 23.
  - FSV test suite: `crates/context-graph-cli/tests/blackbox_phase3_fsv.rs`
    (1 live integration test covering 5 happy verifiers + 15 edge checks; run
    with `cargo test --package context-graph-cli --test blackbox_phase3_fsv`).
- **Phase 4 (Evidence Packets + Replay)**: COMPLETE and verified.
  - Record kinds: `evidence_packet`, `incident_packet`.
  - Commands: `blackbox replay --ledger PATH --run-id RUN_ID` and
    `blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE`.
  - Ledger schema: `LEDGER_SCHEMA_VERSION = 13`; older ledgers are wiped on
    init, never migrated in-place.
  - FSV test suite: `crates/context-graph-cli/tests/blackbox_phase4_fsv.rs`
    (4/4 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase4_fsv`).
- **Phase 5 (Training/Eval Exporter)**: COMPLETE and verified.
  - Command: `blackbox export episodes --ledger PATH --out DIR [--force]`.
  - Output source of truth: `episodes.jsonl`, `manifest.json`, and `evals/`
    under the export directory.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase5_export_fsv.rs`
    (4/4 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase5_export_fsv`).
- **Phase 6 (MCP Gateway + Policy)**: COMPLETE and verified.
  - Commands: `blackbox mcp recorder --ledger PATH`,
    `blackbox mcp gateway --ledger PATH --policy-mode MODE`,
    `blackbox policy evaluate`, `blackbox policy registry`, and
    `blackbox api surface`.
  - New record kinds originally added under schema v4: `policy_decision`,
    `agent_step`, `model_decision`. Current ledgers run under schema v13 after
    Phase 19.
  - Gateway first workflow: `verified_git_apply_patch` applies a real Git patch
    and returns verifier evidence from `git_file_diff_matches_patch_v1`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase6_mcp_policy_fsv.rs`
    (4/4 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase6_mcp_policy_fsv`).
- **Phase 7 (Team Dashboard)**: COMPLETE and verified.
  - Commands: `blackbox dashboard build --ledger PATH --out DIR [--force]`
    and `blackbox dashboard api --ledger PATH --view VIEW [--run-id RUN_ID]`.
  - Output source of truth: `index.html`, `manifest.json`, and
    `api/*.json` under the dashboard directory.
  - Dashboard views: summary, runs, per-run timelines, evidence,
    verifier status, error drilldown, and export commands.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase7_dashboard_fsv.rs`
    (5/5 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase7_dashboard_fsv`).
- **Phase 8 (Enterprise Controls)**: COMPLETE and verified.
  - Commands: `blackbox enterprise configure`,
    `blackbox enterprise sync-directory`,
    `blackbox enterprise access-evaluate`, and
    `blackbox enterprise export-audit`.
  - New record kinds under schema v5: `enterprise_tenant_config`,
    `enterprise_directory_snapshot`, `enterprise_access_decision`.
  - Export source of truth: `audit.jsonl`, `siem-rfc5424.log`,
    `controls.json`, and `manifest.json` under the audit export directory.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase8_enterprise_fsv.rs`
    (6/6 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase8_enterprise_fsv`).
- **Phase 9 (GitLab CI Hosted Connector)**: COMPLETE and verified.
  - Connector: `gitlab_ci_v1`.
  - Verifiers: `gitlab_pipeline_status_v1`,
    `gitlab_pipeline_jobs_completed_v1`.
  - Source of truth: GitLab REST API project/pipeline/jobs endpoints plus the
    local SQLite ledger evidence rows. Phase 9 added no schema bump; current
    ledgers now run under `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase9_gitlab_ci_fsv.rs`
    (1 live integration test covering 2 happy verifiers + 9 edge checks; run
    with
    `cargo test --package context-graph-cli --test blackbox_phase9_gitlab_ci_fsv`).
- **Phase 10 (Scanner Artifact Connector)**: COMPLETE and verified.
  - Connector: `scanner_artifact_v1`.
  - Verifiers: `scanner_vulnerability_absent_v1`,
    `scanner_lockfile_pinned_to_version_v1`,
    `scanner_sbom_artifact_present_v1`.
  - Source of truth: physical OSV JSON report files, Cargo lockfiles,
    CycloneDX JSON SBOM files, plus local SQLite ledger evidence rows. Phase
    10 added no schema bump; current ledgers now run under
    `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase10_scanner_fsv.rs`
    (1 local integration test covering 3 happy verifiers + 12 edge checks; run
    with
    `cargo test --package context-graph-cli --test blackbox_phase10_scanner_fsv`).
- **Phase 11 (GitHub Issue Evidence-Link Verification)**: COMPLETE and verified.
  - Connector surface: existing `github_v1`.
  - Verifier: `github_issue_evidence_link_v1`.
  - Source of truth: GitHub Issues REST API issue body and issue comments,
    plus local SQLite ledger evidence rows. Pull requests returned by the
    Issues endpoint are rejected as ticket evidence. Phase 11 added no schema
    bump; current ledgers now run under `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase11_github_issue_evidence_fsv.rs`
    (1 live integration test covering connector exposure, 1 happy verifier,
    and 7 edge checks; run with
    `cargo test --package context-graph-cli --test blackbox_phase11_github_issue_evidence_fsv`).
- **Phase 12 (Deterministic Scorecards)**: COMPLETE and verified.
  - Connector: `blackbox_scoring_v1`.
  - Command: `blackbox score run --ledger PATH --run-id RUN_ID`.
  - Record kind: `scorecard`.
  - Source of truth: physical SQLite `ledger_records` rows, including
    verifier, evidence, incident, policy, and tool-call rows. Phase 12 bumps
    the logical ledger schema to `LEDGER_SCHEMA_VERSION = 6`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase12_scorecard_fsv.rs`
    (1 local integration test covering a verified happy scorecard, no-tool
    edge, unverified-tool edge, failed-verifier incident edge, and missing-run
    fail-closed behavior; run with
    `cargo test --package context-graph-cli --test blackbox_phase12_scorecard_fsv`).
- **Phase 13 (Privacy Retention And Erasure Decisions)**: COMPLETE and verified.
  - Connector: `blackbox_privacy_v1`.
  - Commands: `blackbox privacy evaluate-retention` and
    `blackbox privacy request-erasure`.
  - Record kinds: `retention_decision`, `evidence_deletion`.
  - Source of truth: physical SQLite `ledger_records`, including
    `enterprise_tenant_config`, `enterprise_directory_snapshot`,
    `enterprise_access_decision`, and parent run rows. Phase 13 bumps the
    logical ledger schema to `LEDGER_SCHEMA_VERSION = 7`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase13_privacy_retention_fsv.rs`
    (1 local integration test covering retention review, approved erasure,
    legal-hold denial, missing retention class, unauthorized principal, missing
    run, and empty data subject; run with
    `cargo test --package context-graph-cli --test blackbox_phase13_privacy_retention_fsv`).
- **Phase 14 (Release Gate Decisions)**: COMPLETE and verified.
  - Connector: `blackbox_release_gate_v1`.
  - Command: `blackbox gate evaluate`.
  - Record kind: `release_gate`.
  - Source of truth: physical SQLite `ledger_records`, including parent run,
    scorecard, verifier/evidence/incident, enterprise RBAC, and privacy rows.
    Phase 14 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 8`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase14_release_gate_fsv.rs`
    (1 local integration test covering an approved gate, missing scorecard,
    low risk threshold, unauthorized principal, incident-bearing run, and
    invalid threshold; run with
    `cargo test --package context-graph-cli --test blackbox_phase14_release_gate_fsv`).
- **Phase 15 (Pilot Packaging, Usage Meters, And Buyer Proof)**: COMPLETE and verified.
  - Connector: `blackbox_packaging_v1`.
  - Command: `blackbox package pilot-report`.
  - Record kind: `pilot_package`.
  - Source of truth: physical SQLite `ledger_records` plus package files
    `pilot-report.json`, `usage-meter-events.jsonl`,
    `eval-candidates.jsonl`, and `manifest.json`. Dashboard output also
    includes `api/pilot-packages.json`.
  - Phase 15 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 9`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase15_pilot_package_fsv.rs`
    (1 local integration test covering successful packaging, unmet success
    criteria, unauthorized principal, invalid minimum actions, connector
    readback, dashboard readback, and manifest hash verification; run with
    `cargo test --package context-graph-cli --test blackbox_phase15_pilot_package_fsv`).
- **Phase 16 (Risks And Open Questions Closure)**: COMPLETE and verified.
  - Connector: `blackbox_risk_v1`.
  - Command: `blackbox risk evaluate`.
  - Record kind: `risk_assessment`.
  - Source of truth: physical SQLite `ledger_records`, SQLite runtime PRAGMAs,
    enterprise RBAC rows, package/dashboard files, incident/eval evidence,
    and a real decision-register JSON file containing every required Phase 16
    open-question decision.
  - Phase 16 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 10`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase16_risk_register_fsv.rs`
    (1 local integration test covering blocked and accepted assessments,
    unauthorized principal, invalid decisions, dashboard/API readback,
    connector readback, and direct SQLite proof; run with
    `cargo test --package context-graph-cli --test blackbox_phase16_risk_register_fsv`).
- **Phase 17 (Authenticated HTTP Server)**: COMPLETE and verified.
  - Connector: `blackbox_http_server_v1`.
  - Command: `blackbox server serve`.
  - Record kind: `http_request_audit`.
  - Source of truth: real HTTP traffic plus physical SQLite `ledger_records`
    and `blackbox_metadata` rows.
  - Phase 17 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 11`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase17_http_server_fsv.rs`
    (1 local integration test starting a real HTTP server, recording a run,
    step, tool call, dashboard read, invalid auth, tenant mismatch, and
    oversized-body edge; run with
    `cargo test --package context-graph-cli --test blackbox_phase17_http_server_fsv -- --nocapture`).
- **Phase 18 (First-Wedge Demo Package)**: COMPLETE and verified.
  - Connector: `blackbox_first_wedge_demo_v1`.
  - Command: `blackbox demo first-wedge`.
  - Record kind: `first_wedge_demo`.
  - Source of truth: physical SQLite `ledger_records`, local Git diff
    readback, physical OSV scanner JSON files, live GitHub Actions run
    readback, live GitHub issue evidence-link readback, and output package
    files `demo-report.json`, `evidence-index.json`, `demo-script.jsonl`, and
    `manifest.json`.
  - Phase 18 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 12`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase18_first_wedge_demo_fsv.rs`
    (1 integration test covering the complete happy demo, missing-run,
    incomplete-evidence, and output-exists edges; run with
    `cargo test --package context-graph-cli --test blackbox_phase18_first_wedge_demo_fsv -- --nocapture`).
- **Phase 19 (Executable Full-State Verification Protocol)**: COMPLETE and verified.
  - Connector: `blackbox_fsv_protocol_v1`.
  - Command: `blackbox fsv certify`.
  - Record kind: `full_state_verification`.
  - Source of truth: physical SQLite `ledger_records` and
    `blackbox_metadata`, SQLite `PRAGMA integrity_check`, FSV evidence JSON,
    declared artifact files, package files `fsv-report.json`,
    `provenance.json`, `edge-cases.jsonl`, and `manifest.json`.
  - Phase 19 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase19_fsv_protocol_fsv.rs`
    (1 integration test covering successful certification, missing evidence,
    wrong post-state, bad artifact hash, connector readback, manifest hashes,
    and output-exists behavior; run with
    `cargo test --package context-graph-cli --test blackbox_phase19_fsv_protocol_fsv -- --nocapture`).
- **Phase 20 (Kubernetes Deployment Verification)**: COMPLETE and verified.
  - Connector: `k8s_deployment_snapshot_v1`.
  - Verifiers: `k8s_deployment_image_digest_v1`,
    `k8s_replicas_match_v1`, `k8s_rollout_completed_v1`.
  - Source of truth: physical JSON snapshot produced by
    `kubectl get deployments.v1.apps --namespace <namespace> -o json`, plus
    local SQLite verifier/evidence rows.
  - Phase 20 adds no new ledger record kind and does not bump the physical
    schema; current ledgers remain at `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase20_k8s_deployment_fsv.rs`
    (1 integration test covering connector readback, happy image/replica/
    rollout verification, missing Deployment, wrong digest, invalid JSON, and
    direct SQLite proof; run with
    `cargo test --package context-graph-cli --test blackbox_phase20_k8s_deployment_fsv -- --nocapture`).
- **Phase 21 (Terraform And Cloud Artifact Verification)**: COMPLETE and verified.
  - Connector: `terraform_cloud_artifact_v1`.
  - Verifiers: `tf_plan_only_approved_changes_v1`,
    `tf_state_attribute_v1`, `cloud_resource_present_v1`,
    `cloud_audit_principal_action_v1`.
  - Source of truth: physical Terraform JSON artifacts produced by
    `terraform show -json <planfile>` and `terraform show -json <statefile>`,
    normalized cloud inventory JSON, normalized cloud audit JSONL, plus local
    SQLite verifier/evidence rows.
  - Phase 21 adds no new ledger record kind and does not bump the physical
    schema; current ledgers remain at `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase21_terraform_cloud_fsv.rs`
    (1 integration test covering connector readback, happy Terraform plan,
    state, cloud inventory, cloud audit verification, unapproved plan change,
    state attribute mismatch, invalid audit JSONL, and direct SQLite proof;
    run with
    `cargo test --package context-graph-cli --test blackbox_phase21_terraform_cloud_fsv -- --nocapture`).
- **Phase 22 (Ticketing Artifact Verification)**: COMPLETE and verified.
  - Connector: `ticketing_artifact_v1`.
  - Verifiers: `jira_ticket_state_v1`, `linear_ticket_state_v1`,
    `servicenow_ticket_state_v1`.
  - Source of truth: physical Jira REST issue/search JSON artifacts, Linear
    GraphQL issue JSON artifacts, ServiceNow Table API record JSON artifacts,
    plus local SQLite verifier/evidence rows.
  - Phase 22 adds no new ledger record kind and does not bump the physical
    schema; current ledgers remain at `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase22_ticketing_fsv.rs`
    (1 integration test covering connector readback, happy Jira/Linear/
    ServiceNow ticket verification, missing ticket, wrong state, invalid JSON,
    and direct SQLite proof; run with
    `cargo test --package context-graph-cli --test blackbox_phase22_ticketing_fsv -- --nocapture`).
- **Phase 23 (OpenTelemetry Evidence Export)**: COMPLETE and verified.
  - Connector: `blackbox_otel_export_v1`.
  - Command: `blackbox export otel --ledger PATH --out DIR [--force]`.
  - Output source of truth: physical `traces.jsonl`, `logs.jsonl`,
    `metrics.jsonl`, and `manifest.json` files under the export directory,
    plus the verified SQLite ledger rows used as input.
  - Phase 23 adds no new ledger record kind and does not bump the physical
    schema; current ledgers remain at `LEDGER_SCHEMA_VERSION = 13`.
  - FSV test suite:
    `crates/context-graph-cli/tests/blackbox_phase23_otel_export_fsv.rs`
    (4 local integration tests covering happy OTLP JSONL export, connector
    readback, empty ledger, existing output, hash-chain tampering, manifest
    hashes, and direct SQLite/file proof; run with
    `cargo test --package context-graph-cli --test blackbox_phase23_otel_export_fsv -- --nocapture`).

## Build And Test Quick Reference

```bash
# Build the workspace (CUDA feature is the default; harmless on Phase 1)
cargo build --release --workspace

# Phase 1 unit + FSV tests (executes the real binary against a real SQLite DB)
cargo test --package context-graph-cli --test blackbox_phase1_fsv -- --nocapture

# Phase 2 FSV tests (real SQLite + real Git repos + real commands/files)
cargo test --package context-graph-cli --test blackbox_phase2_fsv -- --nocapture

# Phase 3 FSV tests (live GitHub/GitHub Actions API + real SQLite ledger)
cargo test --package context-graph-cli --test blackbox_phase3_fsv -- --nocapture

# Phase 4 FSV tests (real SQLite ledger + real exported evidence files)
cargo test --package context-graph-cli --test blackbox_phase4_fsv -- --nocapture

# Phase 5 FSV tests (real SQLite ledger + real JSONL/manifest/eval files)
cargo test --package context-graph-cli --test blackbox_phase5_export_fsv -- --nocapture

# Phase 6 FSV tests (real MCP stdio + real SQLite + real Git patch gateway)
cargo test --package context-graph-cli --test blackbox_phase6_mcp_policy_fsv -- --nocapture

# Phase 7 FSV tests (real SQLite ledger + real dashboard files)
cargo test --package context-graph-cli --test blackbox_phase7_dashboard_fsv -- --nocapture

# Phase 8 FSV tests (real SQLite ledger + real enterprise audit/SIEM files)
cargo test --package context-graph-cli --test blackbox_phase8_enterprise_fsv -- --nocapture

# Phase 9 FSV tests (live GitLab CI API + real SQLite ledger)
cargo test --package context-graph-cli --test blackbox_phase9_gitlab_ci_fsv -- --nocapture

# Phase 10 FSV tests (real scanner files + real SQLite ledger)
cargo test --package context-graph-cli --test blackbox_phase10_scanner_fsv -- --nocapture

# Phase 11 FSV tests (live GitHub issue/comments API + real SQLite ledger)
cargo test --package context-graph-cli --test blackbox_phase11_github_issue_evidence_fsv -- --nocapture

# Phase 12 FSV tests (real SQLite scorecards + real Git verifier evidence)
cargo test --package context-graph-cli --test blackbox_phase12_scorecard_fsv -- --nocapture

# Phase 13 FSV tests (real SQLite privacy retention/erasure decisions)
cargo test --package context-graph-cli --test blackbox_phase13_privacy_retention_fsv -- --nocapture

# Phase 14 FSV tests (real SQLite release gate decisions + real Git evidence)
cargo test --package context-graph-cli --test blackbox_phase14_release_gate_fsv -- --nocapture

# Phase 15 FSV tests (real SQLite pilot packages + real package/dashboard files)
cargo test --package context-graph-cli --test blackbox_phase15_pilot_package_fsv -- --nocapture

# Phase 16 FSV tests (real SQLite risk assessments + decision register/dashboard readback)
cargo test --package context-graph-cli --test blackbox_phase16_risk_register_fsv -- --nocapture

# Phase 17 FSV tests (real authenticated HTTP server + direct SQLite audit readback)
cargo test --package context-graph-cli --test blackbox_phase17_http_server_fsv -- --nocapture

# Phase 18 FSV tests (real first-wedge package + direct SQLite/file readback)
cargo test --package context-graph-cli --test blackbox_phase18_first_wedge_demo_fsv -- --nocapture

# Phase 19 FSV tests (real executable FSV certification + SQLite/file readback)
cargo test --package context-graph-cli --test blackbox_phase19_fsv_protocol_fsv -- --nocapture

# Phase 20 FSV tests (real Kubernetes JSON snapshot files + SQLite readback)
cargo test --package context-graph-cli --test blackbox_phase20_k8s_deployment_fsv -- --nocapture

# Phase 21 FSV tests (real Terraform/cloud artifact files + SQLite readback)
cargo test --package context-graph-cli --test blackbox_phase21_terraform_cloud_fsv -- --nocapture

# Phase 22 FSV tests (real ticketing API JSON artifact files + SQLite readback)
cargo test --package context-graph-cli --test blackbox_phase22_ticketing_fsv -- --nocapture

# Phase 23 FSV tests (real SQLite ledger + OTLP JSONL file readback)
cargo test --package context-graph-cli --test blackbox_phase23_otel_export_fsv -- --nocapture

# Run the standalone binary against a temp ledger
cargo run -p context-graph-cli --bin blackbox -- \
    init --ledger /tmp/bbx.sqlite

# Same logic via the umbrella context-graph binary
cargo run -p context-graph-cli --bin context-graph -- \
    blackbox init --ledger /tmp/bbx.sqlite
```

## Planning Documents

- `01_product_vision.md` — product thesis, users, principles, non-goals.
- `02_market_and_buyer_research.md` — buyers, urgency, wedge, competitors, pricing.
- `03_reference_architecture.md` — system components and deployment patterns.
- `04_event_ledger_schema.md` — record contracts and the current Phase 23
  SQLite schema.
- `05_source_of_truth_connectors.md` — connector contract and shipped
  local/GitHub/GitLab/enterprise/scanner-artifact/Kubernetes/Terraform-cloud/ticketing/scoring/privacy/release-gate/packaging/risk/HTTP/demo/FSV/OpenTelemetry connectors.
- `06_fsv_verification_engine.md` — verifier protocol and shipped verifier registry.
- `07_replay_forensics_and_incidents.md` — replay, audit packets, incident workflow.
- `08_training_eval_corpus.md` — shipped verified episode export (Phase 5).
- `09_security_governance_compliance.md` — security, NIST/OWASP/MITRE alignment.
- `10_api_cli_mcp_surface.md` — CLI / API / MCP / SDK surface (Phase 23 OpenTelemetry export shipped).
- `11_storage_retention_privacy.md` — storage, retention, encryption, tenancy.
- `12_scoring_and_risk_model.md` — verifier status, risk scoring, trust levels.
- `13_mvp_implementation_phases.md` — sequenced build plan; Phases 0–23 done.
- `14_tests_acceptance_and_fsv.md` — acceptance tests, manual FSV templates.
- `15_gtm_pricing_and_packaging.md` — go-to-market, packaging, sales motion.
- `16_risks_open_questions.md` — risks; resolved open questions noted inline.
- `17_research_basis.md` — public references and design implications.
- `18_first_wedge_demo.md` — first proof-of-value workflow (Phase 2/3 demo).
- `19_full_state_verification_protocol.md` — **mandatory** FSV protocol for
  every phase; defines source-of-truth read pattern, edge-case rules, and
  evidence-of-success format. Every PR must conform to this file.
- `20_phase1_state_and_invariants.md` — physical schema, invariants, error
  codes, and exact synthetic happy/edge inputs and expected outputs of the
  shipped Phase 1.
- `21_phase2_state_and_invariants.md` — shipped Phase 2 connectors,
  verifiers, ledger rows, tests, and manual FSV evidence.
- `22_phase3_state_and_invariants.md` — shipped Phase 3 GitHub/GHA
  connectors, verifiers, live tests, and manual FSV evidence.
- `23_phase4_state_and_invariants.md` — shipped Phase 4 evidence packets,
  incident packets, replay/export commands, tests, and manual FSV evidence.
- `24_phase5_state_and_invariants.md` — shipped Phase 5 episode exporter,
  JSONL/manifest/eval artifacts, tests, and manual FSV evidence.
- `25_phase6_state_and_invariants.md` — shipped Phase 6 MCP stdio gateway,
  policy records, capability registry, tests, and manual FSV evidence.
- `26_phase7_state_and_invariants.md` — shipped Phase 7 dashboard builder,
  dashboard API files, tests, and manual FSV evidence.
- `27_phase8_state_and_invariants.md` — shipped Phase 8 enterprise controls,
  audit/SIEM export files, tests, and manual FSV evidence.
- `28_phase9_state_and_invariants.md` — shipped Phase 9 GitLab CI connector,
  verifiers, live tests, and manual FSV evidence.
- `29_phase10_state_and_invariants.md` — shipped Phase 10 scanner artifact
  connector, verifiers, local tests, and manual FSV evidence.
- `30_phase11_state_and_invariants.md` — shipped Phase 11 GitHub issue
  evidence-link verifier, live tests, and manual FSV evidence.
- `31_phase12_state_and_invariants.md` — shipped Phase 12 deterministic
  scorecards, tests, and manual FSV evidence.
- `32_phase13_state_and_invariants.md` — shipped Phase 13 privacy retention
  and erasure decisions, tests, and manual FSV evidence.
- `33_phase14_state_and_invariants.md` — shipped Phase 14 release gate
  decisions, tests, and manual FSV evidence.
- `34_phase15_state_and_invariants.md` — shipped Phase 15 pilot packaging,
  usage-meter rows, dashboard/package files, tests, and manual FSV evidence.
- `35_phase16_state_and_invariants.md` — shipped Phase 16 risk assessments,
  open-question decision closure, dashboard/API readback, tests, and manual
  FSV evidence.
- `36_phase17_state_and_invariants.md` — shipped Phase 17 authenticated HTTP
  server, audit rows, tenant-scoped dashboard readback, tests, and manual FSV
  checklist.
- `37_phase18_state_and_invariants.md` — shipped Phase 18 first-wedge demo
  package, required proof categories, package manifest, tests, and manual FSV
  checklist.
- `38_phase19_state_and_invariants.md` — shipped Phase 19 executable FSV
  certification, evidence schema, package manifest, tests, and manual FSV
  checklist.
- `39_phase20_state_and_invariants.md` — shipped Phase 20 Kubernetes
  Deployment snapshot connector, verifiers, tests, and manual FSV checklist.
- `40_phase21_state_and_invariants.md` — shipped Phase 21 Terraform/cloud
  artifact connector, verifiers, tests, and manual FSV checklist.
- `41_phase22_state_and_invariants.md` — shipped Phase 22 ticketing artifact
  connector, verifiers, tests, and manual FSV checklist.
- `42_phase23_state_and_invariants.md` — shipped Phase 23 OpenTelemetry
  export, connector readback, tests, and manual FSV checklist.

## Build Doctrine

Every mutating action must produce durable evidence:

1. Define the source of truth before execution.
2. Capture the pre-state independently.
3. Execute the agent or tool action.
4. Read the source of truth through a separate read path.
5. Compare expected persisted delta to actual readback.
6. Emit a typed verifier result with error code and remediation.
7. Persist the evidence packet.
8. Export accepted and rejected episodes into eval/training data.

The system **fails closed**. Missing connectors, missing readback permissions,
schema drift, stale evidence, or unverified claims are product-visible
failures, not soft warnings. There is **no fallback path**: typed error +
non-zero exit code, every time.

## Project-Wide Rules (apply to every doc, every commit)

1. **No backwards compatibility.** This product is pre-production. On a schema
   change, wipe the SQLite ledger; never write a migration shim. Current
   physical ledger schema is version `13`.
2. **Fail fast, no fallbacks.** A failure must surface as a typed error code
   with `source_of_truth`, `connector`, `field_path`, and `remediation`. If the
   path errors, do not catch and substitute a default — let it bubble.
3. **Real data only.** Tests must hit real SQLite files, real Git repos, real
   connector sandboxes. Mocks are forbidden. A test that passes against a
   broken system is itself a defect.
4. **Manual Full-State Verification (FSV) is non-optional.** Every phase exits
   only after FSV proves rows/files actually exist on disk and match expected
   shape. See `19_full_state_verification_protocol.md` for the exact protocol.
5. **Occam's razor.** Reach for the simplest mechanism that physically proves
   the outcome. The Phase 1 ledger is plain SQLite with sorted-key canonical
   JSON and SHA-256 because that is the smallest set of tools that gives
   tamper-evidence; do not introduce new dependencies, indices, or formats
   without first showing the simpler approach is insufficient.

## First Product Wedge

DevOps/SRE/SecOps agent governance: an AI agent proposes or performs code, CI,
dependency, infrastructure, or ticket actions; BlackBoxFix records the run and
verifies physical outcomes in Git, GitHub, CI, scanner output, Terraform/
Kubernetes state, and ticketing systems. Sources of truth are inspectable, the
workflow has budget, and the cost of unverified agent behavior is concrete.
