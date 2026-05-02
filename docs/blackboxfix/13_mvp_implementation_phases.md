# MVP Implementation Phases

The MVP proves source-of-truth verification and training/eval export before
expanding into many connectors. Phases land **one at a time**; do not batch
phases or skip ahead.

## Phase 0: Product Foundation — DONE

Deliverables:

- planning docs (this directory);
- ledger schema (`04_event_ledger_schema.md`);
- verifier spec format (`06_fsv_verification_engine.md`);
- connector contract (`05_source_of_truth_connectors.md`);
- first wedge demo design (`18_first_wedge_demo.md`);
- threat model (`09_security_governance_compliance.md`);
- acceptance criteria (`14_tests_acceptance_and_fsv.md`).

Exit criteria (met):

- every core record has a schema;
- first workflow has sources of truth defined;
- first three connectors are selected (Git local, GitHub, GitHub Actions).

## Phase 1: Local Ledger And CLI — DONE (2026-05-02, commit `1cf89284`)

Deliverables (all shipped):

- local append-only SQLite ledger
  (`crates/context-graph-cli/src/commands/blackbox.rs`);
- `blackbox init` (operator can initialize a fresh ledger);
- `blackbox record run` (writes one `agent_run` row);
- `blackbox record tool-call` (writes one `tool_call` row, redacted);
- `blackbox inspect ledger` (re-verifies both hash chains and the
  `blackbox_metadata` cross-checks);
- dual hash chains (per-record + per-run);
- redaction policy `redact_v1` with field-name and literal-pattern rules
  plus a post-scan that rejects any unredacted secret-like literal.

Exit criteria (verified by automated FSV at
`crates/context-graph-cli/tests/blackbox_phase1_fsv.rs`, 4/4 passing):

- manual FSV proves records persist in the SQLite ledger
  (`phase1_happy_path_persists_and_inspects_physical_rows`);
- hash chain verification catches tampering
  (`phase1_hash_chain_detection_catches_payload_tampering`);
- redaction failure blocks persistence
  (`phase1_redaction_violation_blocks_persistence`);
- append-only triggers reject privileged updates
  (`phase1_append_only_trigger_blocks_direct_update`).

## Phase 2: Git And File Verification — NEXT

Deliverables:

- local Git connector (`git_local_v1`) reading via `git2` or shell;
- file-readback connector (`fs_local_v1`) reading file content + hash;
- test-command evidence capture (`cmd_local_v1`): exit code, stdout/stderr,
  artifact files;
- verifiers: `git_branch_exists_v1`, `git_commit_exists_v1`,
  `git_file_diff_matches_patch_v1`, `git_working_tree_clean_v1`,
  `cmd_exit_zero_v1`;
- new ledger record kinds: `expected_delta`, `readback_snapshot`,
  `verifier_result` (bumps `LEDGER_SCHEMA_VERSION` to 2 and **wipes** the
  ledger — no migration shim).

Exit criteria:

- agent/code workflow can be recorded and verified locally;
- three edge cases per mutating verifier;
- failed verifiers produce typed errors;
- Phase 2 FSV test file (`tests/blackbox_phase2_fsv.rs`) covers happy path
  + 3 edge cases per verifier against real Git fixtures.

Cargo dependencies to add (Phase 2 only): `git2 = "0.19"` or
`gix` (decide based on bundled-libgit2 footprint).

## Phase 3: GitHub And CI Connectors

Deliverables:

- GitHub PR/checks/issues connector (`github_v1`);
- CI readback connector for GitHub Actions first (`gha_v1`);
- verifiers for PR state, branch head, check suite, issue state.

Exit criteria:

- vulnerability remediation demo (`18_first_wedge_demo.md`) verifies GitHub
  and CI state end-to-end;
- permission failures are `blocked` with `CONNECTOR_PERMISSION_DENIED`;
- stale readback and missing PR edge cases are covered;
- Phase 3 FSV uses a controlled test repo (do **not** create test artifacts
  on production-shaped repos).

## Phase 4: Evidence Packets And Replay

Deliverables:

- evidence packet builder (`evidence_packet` record kind);
- forensic replay timeline reader;
- incident packet generation (`incident_packet` record kind);
- JSON/PDF-ready audit export.

Exit criteria:

- failed verifier produces an incident packet;
- replay reconstructs full run without mutation;
- evidence hashes verify against ledger records.

## Phase 5: Training/Eval Exporter

Deliverables:

- `blackbox export episodes` (mirrors
  `crates/context-graph-cli/src/commands/export_dynamicjepa.rs`);
- manifest with content sha256 and ledger sequence range;
- JSONL row validation (re-parse each line);
- eval fixture generation for failure classes;
- Phase 5 FSV test (`tests/blackbox_phase5_export_fsv.rs`).

Exit criteria:

- exported JSONL parses line by line;
- manifest hash matches file;
- every row links back to a ledger evidence packet.

## Phase 6: MCP Gateway + Policy

Deliverables:

- recorder MCP server (Tool registry + JSON-RPC stdio);
- gateway MCP server (verified tool wrappers for first workflow);
- strict schemas;
- capability registry;
- policy modes (`observe_only`, `supervised`, `bounded_autonomous`,
  `blocked`);
- HTTP API surface from `10_api_cli_mcp_surface.md`.

Exit criteria:

- an MCP-compatible agent can run through the gateway;
- every mutating MCP tool returns verifier evidence;
- governed mode blocks unverified mutating tools.

## Phase 7: Team Dashboard

Deliverables:

- run timeline UI;
- evidence viewer;
- verifier status dashboard;
- error-code drilldown;
- export UI.

Exit criteria:

- user can inspect a passed run and a failed run;
- user can export evidence and episodes;
- source-of-truth links are visible.

## Phase 8: Enterprise Controls

Deliverables:

- SSO/SAML/OIDC;
- RBAC;
- tenant isolation;
- SIEM export;
- retention policies;
- private deployment path.

Exit criteria:

- enterprise pilot can run in a controlled environment;
- audit export satisfies a security review.

## Phase Sequencing Rules

1. Each phase ships behind a single PR set with a `Phase N FSV` test file
   and the exit-criteria checklist marked `[x]`.
2. Schema bumps wipe the ledger. No migration shims, no compatibility flags.
3. No phase begins until the previous phase's FSV test file is green on a
   fresh checkout.
4. If a phase's exit criterion cannot be verified by reading a physical
   source of truth, the phase is incomplete — re-open it.
