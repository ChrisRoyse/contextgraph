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
`crates/context-graph-cli/tests/blackbox_phase1_fsv.rs`, 10/10 passing):

- manual FSV proves records persist in the SQLite ledger
  (`phase1_happy_path_persists_and_inspects_physical_rows`);
- hash chain verification catches tampering
  (`phase1_hash_chain_detection_catches_payload_tampering`);
- redaction failure blocks persistence
  (`phase1_redaction_violation_blocks_persistence`);
- append-only triggers reject privileged updates
  (`phase1_append_only_trigger_blocks_direct_update`).
- case-insensitive secret prefixes block persistence
  (`phase1_case_insensitive_secret_prefix_blocks_persistence`);
- cross-tenant tool-call/run links are refused
  (`phase1_cross_tenant_tool_call_is_blocked`);
- missing parent run ids are refused before persistence
  (`phase1_missing_parent_run_blocks_tool_call`);
- invalid risk tiers are refused before a row is written
  (`phase1_invalid_risk_tier_blocks_persistence`);
- schema validation rejects no-op append-only trigger drift
  (`phase1_schema_validation_rejects_noop_append_only_trigger`);
- `init` does not silently repair an already-initialized ledger with a
  missing append-only trigger
  (`phase1_init_does_not_repair_missing_append_only_trigger`).

## Phase 2: Git And File Verification — DONE (2026-05-02)

Deliverables (all shipped):

- local Git connector (`git_local_v1`) reading via the real `git` binary;
- file-readback connector (`fs_local_v1`) reading file content + hash;
- test-command evidence capture (`cmd_local_v1`): exit code, stdout/stderr,
  artifact files;
- verifiers: `git_branch_exists_v1`, `git_commit_exists_v1`,
  `git_file_diff_matches_patch_v1`, `git_working_tree_clean_v1`,
  `cmd_exit_zero_v1`;
- new ledger record kinds: `expected_delta`, `readback_snapshot`,
  `verifier_result` (originally bumped `LEDGER_SCHEMA_VERSION` to 2 and
  **wiped** the ledger — no migration shim; current shipped schema is v3 after
  Phase 4).

Exit criteria (verified):

- agent/code workflow can be recorded and verified locally
  (`phase2_happy_path_verifiers_persist_physical_readbacks`);
- three edge cases per mutating verifier are covered against real Git repos,
  files, and commands;
- failed verifiers produce typed errors with connector/source/field/remediation;
- Phase 2 FSV test file
  (`crates/context-graph-cli/tests/blackbox_phase2_fsv.rs`) covers connector
  list/test, five happy-path verifiers, three edge classes per verifier, strict
  command argv schema rejection, and v1-ledger wipe on schema v2 init
  (19/19 passing);
- manual FSV completed against `/tmp/bbx-phase2-manual-fsv.vMFZRR/fsv.log`:
  final physical SQLite state had schema version `2`, 25 rows, hash-chain
  integrity `passed`, and direct Git/filesystem/command reads matched ledger
  readbacks.

No Git library dependency was added. Shelling to the installed `git` binary
keeps Phase 2 aligned with operator-visible source-of-truth commands and
avoids bundled-libgit2 footprint until a later need proves otherwise.

## Phase 3: GitHub And CI Connectors — DONE (2026-05-02)

Deliverables (all shipped):

- GitHub PR/checks/issues connector (`github_v1`);
- CI readback connector for GitHub Actions first (`gha_v1`);
- verifiers: `github_pr_state_v1`,
  `github_branch_head_matches_commit_v1`,
  `github_check_suite_passed_v1`, `github_issue_state_changed_v1`,
  `gha_workflow_run_completed_v1`;
- async GitHub REST client with explicit API version/media headers, typed
  rate-limit/auth/schema errors, and endpoint-specific handling for expected
  `404`/`422` absent-resource readbacks.

Exit criteria (verified):

- GitHub and CI state are verified end-to-end against the live public fixture
  repository `actions/checkout`;
- permission failures are `blocked` with `CONNECTOR_PERMISSION_DENIED` for
  `401`/`403` responses, and rate limits are `blocked` with
  `GITHUB_RATE_LIMITED`;
- stale readback and missing PR edge cases are covered by both automated and
  manual FSV;
- Phase 3 creates no GitHub artifacts. It uses read-only live GitHub/GHA
  source-of-truth reads and writes only the local SQLite ledger.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase3_fsv -- --nocapture
```

Current result: 1/1 passing. The single live integration test discovers real
GitHub state, verifies all five Phase 3 happy paths, then runs 15 real edge
checks covering missing targets, stale/wrong expectations, PR-vs-issue
ambiguity, and schema blocks.

Manual FSV completed against `/tmp/bbx-phase3-manual-fsv.KZAAKb/fsv.log`.
Final physical SQLite state at Phase 3 completion: `agent_run=1`,
`tool_call=1`, `expected_delta=8`, `readback_snapshot=7`,
`verifier_result=8`. The blocked schema edge intentionally wrote no
`readback_snapshot`. Current Phase 4 tests also prove Phase 3 verifiers now
append evidence packets under schema v3.

## Phase 4: Evidence Packets And Replay — DONE (2026-05-02)

Deliverables (all shipped):

- evidence packet builder (`evidence_packet` record kind);
- forensic replay timeline reader (`blackbox replay --ledger PATH --run-id RUN_ID`);
- incident packet generation (`incident_packet` record kind);
- JSON/PDF-ready audit export
  (`blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE`);
- schema bump to `LEDGER_SCHEMA_VERSION = 3`; old v1/v2 ledgers are wiped on
  init, never migrated in-place.

Exit criteria (verified):

- failed verifier produces an incident packet;
- replay reconstructs full run without mutation;
- evidence hashes verify against ledger records.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase4_fsv -- --nocapture
```

Current result: 4/4 passing. Coverage includes passed verifier evidence,
failed verifier incident creation, read-only replay, export file write +
readback + JSON parse, and edge cases for missing run and invalid export path.

Manual FSV completed against `/tmp/bbx-phase4-manual-fsv.M8gObv/fsv.log`.
Final physical SQLite state: `agent_run=1`, `tool_call=1`,
`expected_delta=2`, `readback_snapshot=2`, `verifier_result=2`,
`evidence_packet=2`, `incident_packet=1`. The exported evidence file existed
at `/tmp/bbx-phase4-manual-fsv.M8gObv/evidence-report.json`, parsed as
`blackbox_evidence_report_json_v1`, and its replay report had
`evidence_hash_verification.status=passed`.

## Phase 5: Training/Eval Exporter — DONE (2026-05-02)

Deliverables (all shipped):

- `blackbox export episodes --ledger PATH --out DIR [--force]` (mirrors
  `crates/context-graph-cli/src/commands/export_dynamicjepa.rs`);
- manifest with content sha256 and ledger sequence range;
- JSONL row validation (re-parse each line);
- eval fixture generation for failure classes;
- Phase 5 FSV test
  (`crates/context-graph-cli/tests/blackbox_phase5_export_fsv.rs`).

Exit criteria (verified):

- exported JSONL parses line by line;
- manifest hash matches file;
- every row links back to a ledger evidence packet.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase5_export_fsv -- --nocapture
```

Current result: 4/4 passing. Coverage includes JSONL/manifest/eval happy path,
output-exists fail-closed behavior, hash-chain tamper blocking, and unverified
tool-call blocking.

Manual FSV completed against `/tmp/bbx-phase5-manual-fsv.exRXYj/fsv.log`.
Final physical SQLite state: `agent_run=1`, `tool_call=1`,
`expected_delta=2`, `readback_snapshot=2`, `verifier_result=2`,
`evidence_packet=2`, `incident_packet=1`. The export directory contained
`episodes.jsonl`, `manifest.json`, and two eval fixtures; direct file readback
proved 3 JSONL rows with splits `passed=1`, `failed=1`, `incident=1`, and
manifest/output SHA
`sha256:c0c4d0a76db8ed18ba2c708208f6556de5040a9854ca51a1cf44f069c53afcda`.

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
