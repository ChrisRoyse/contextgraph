# BlackBoxFix — Verified AgentOps Master Index

> Codename for a Verified AgentOps platform: a black-box recorder and verifier
> for AI agents. Records every agent action, verifies the claimed effect against
> the physical source of truth, scores the action, supports replay/incidents,
> and exports verified episodes for training/evals.

This index is the **canonical entry point**. A fresh-context AI agent should
read this file first, then `13_mvp_implementation_phases.md`, then
`19_full_state_verification_protocol.md` before touching code.

## Repository State (as of 2026-05-02)

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
    ledger schema is `LEDGER_SCHEMA_VERSION = 3` after Phase 4; old v1/v2
    ledgers are wiped on init, never migrated in-place.
  - FSV test suite: `crates/context-graph-cli/tests/blackbox_phase2_fsv.rs`
    (19/19 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase2_fsv`).
- **Phase 3 (GitHub + GitHub Actions Verification)**: COMPLETE and verified.
  - Connectors: `github_v1`, `gha_v1`.
  - Verifiers: `github_pr_state_v1`,
    `github_branch_head_matches_commit_v1`,
    `github_check_suite_passed_v1`, `github_issue_state_changed_v1`,
    `gha_workflow_run_completed_v1`.
  - Phase 3 added no new record kinds. Current ledgers run under schema v3
    after Phase 4.
  - FSV test suite: `crates/context-graph-cli/tests/blackbox_phase3_fsv.rs`
    (1 live integration test covering 5 happy verifiers + 15 edge checks; run
    with `cargo test --package context-graph-cli --test blackbox_phase3_fsv`).
- **Phase 4 (Evidence Packets + Replay)**: COMPLETE and verified.
  - Record kinds: `evidence_packet`, `incident_packet`.
  - Commands: `blackbox replay --ledger PATH --run-id RUN_ID` and
    `blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE`.
  - Ledger schema: `LEDGER_SCHEMA_VERSION = 3`; v1/v2 ledgers are wiped on
    init, never migrated in-place.
  - FSV test suite: `crates/context-graph-cli/tests/blackbox_phase4_fsv.rs`
    (4/4 passing; run with
    `cargo test --package context-graph-cli --test blackbox_phase4_fsv`).
- **Phases 5–8**: NOT started. See `13_mvp_implementation_phases.md` for the
  exact next-step order and `20`/`21`/`22`/`23` state-and-invariant docs for
  the shipped baseline.

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
- `04_event_ledger_schema.md` — record contracts and the current Phase 4
  SQLite schema.
- `05_source_of_truth_connectors.md` — connector contract and shipped Phase 2 connectors.
- `06_fsv_verification_engine.md` — verifier protocol and shipped Phase 2 registry.
- `07_replay_forensics_and_incidents.md` — replay, audit packets, incident workflow.
- `08_training_eval_corpus.md` — verified episode export (Phase 5).
- `09_security_governance_compliance.md` — security, NIST/OWASP/MITRE alignment.
- `10_api_cli_mcp_surface.md` — CLI / API / MCP / SDK surface (Phase 4 CLI shipped).
- `11_storage_retention_privacy.md` — storage, retention, encryption, tenancy.
- `12_scoring_and_risk_model.md` — verifier status, risk scoring, trust levels.
- `13_mvp_implementation_phases.md` — sequenced build plan; Phases 0–4 done.
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
   physical ledger schema is version `3`.
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
