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
    **wiped** the ledger — no migration shim; current shipped schema is v5 after
  Phase 10).

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
append evidence packets under the current schema.

## Phase 4: Evidence Packets And Replay — DONE (2026-05-02)

Deliverables (all shipped):

- evidence packet builder (`evidence_packet` record kind);
- forensic replay timeline reader (`blackbox replay --ledger PATH --run-id RUN_ID`);
- incident packet generation (`incident_packet` record kind);
- JSON/PDF-ready audit export
  (`blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE`);
- schema bump to `LEDGER_SCHEMA_VERSION = 3` at Phase 4; older ledgers are wiped on
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

Status: **DONE** (2026-05-02).

Deliverables:

- recorder MCP server (Tool registry + JSON-RPC stdio) shipped as
  `blackbox mcp recorder`;
- gateway MCP server shipped as `blackbox mcp gateway`;
- strict MCP tool schemas with `additionalProperties=false`;
- capability registry shipped through `blackbox policy registry`;
- policy modes (`observe_only`, `supervised`, `bounded_autonomous`, `blocked`)
  persisted as `policy_decision` rows;
- HTTP API surface from `10_api_cli_mcp_surface.md` shipped as
  `blackbox api surface`;
- first verified gateway workflow shipped as `verified_git_apply_patch`.

Exit criteria:

- an MCP-compatible agent can run through the gateway;
- every mutating MCP tool returns verifier evidence;
- governed mode blocks unverified mutating tools.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase6_mcp_policy_fsv -- --nocapture
```

Current result: 4/4 passing. Coverage includes MCP `tools/list`, a real
`verified_git_apply_patch` happy path against a physical Git repo, blocked-mode
denial before mutation, missing-verifier policy denial, and invalid MCP
arguments failing before new rows are written.

Manual FSV completed against `/tmp/bbx-phase6-manual-fsv.98fUrJ/fsv.log`.
Happy-path source-of-truth readback: SQLite kind counts were
`agent_run=1`, `agent_step=1`, `model_decision=1`, `policy_decision=1`,
`tool_call=1`, `expected_delta=1`, `readback_snapshot=1`,
`verifier_result=1`, `evidence_packet=1`; physical Git diff SHA matched the
expected patch SHA `c53f17a0f04b39466228cf0139f211e029beae6f1fd529b2dd2727a0b255dbb2`.

## Phase 7: Team Dashboard

Status: **DONE** (2026-05-02).

Deliverables:

- run timeline UI shipped in `index.html` and `api/timeline_<run_id>.json`;
- evidence viewer shipped in `api/evidence.json`;
- verifier status dashboard shipped in `api/verifier-status.json`;
- error-code drilldown shipped in `api/errors.json`;
- export UI/read model shipped in `api/exports.json`.

Exit criteria:

- user can inspect a passed run and a failed run;
- user can export evidence and episodes through the generated commands;
- source-of-truth links are visible in HTML and dashboard API JSON.

Commands:

```bash
blackbox dashboard build --ledger PATH --out DIR [--force]
blackbox dashboard api --ledger PATH --view summary|runs|timeline|evidence|verifiers|errors|exports [--run-id RUN_ID]
```

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase7_dashboard_fsv -- --nocapture
```

Current result: 5/5 passing. Coverage includes dashboard happy path over a
physical ledger with one passed run and one failed run, manifest/file readback
hash verification, existing-output fail-closed behavior, empty-ledger
fail-closed behavior, hash-chain tamper blocking, and missing timeline run id
blocking without ledger mutation.

Manual FSV completed against `/tmp/bbx-phase7-manual-fsv.W4eqmG/fsv.log`.
Happy-path source-of-truth readback: SQLite kind counts remained
`agent_run=2`, `tool_call=2`, `expected_delta=2`, `readback_snapshot=2`,
`verifier_result=2`, `evidence_packet=2`, `incident_packet=1` before and
after dashboard build. The dashboard directory contained `index.html`,
`manifest.json`, six top-level API files, and two per-run timeline files.
Manifest readback proved `api/summary.json` hash
`sha256:16788f6c494990ca77fc51cdd19e91d4480e5b03dbe1fba4b4a985bc16988776`
and `index.html` hash
`sha256:554870febfa02a6ece9b45f4a4619cda32f120c3731bc82570d85f5eb796f930`.

## Phase 8: Enterprise Controls — DONE (2026-05-02)

Deliverables (all shipped):

- SSO/SAML/OIDC metadata validation via `blackbox enterprise configure`;
- SCIM-shaped directory snapshots via `blackbox enterprise sync-directory`;
- RBAC and tenant isolation decisions via
  `blackbox enterprise access-evaluate`;
- SIEM export via `blackbox enterprise export-audit`
  (`siem-rfc5424.log`);
- retention policies persisted on `enterprise_tenant_config`;
- private deployment path persisted as private VPC, region, data residency,
  SIEM format, and tenant customer-managed key id.

Exit criteria (met):

- enterprise pilot can run in a controlled environment using a local append-only
  ledger, explicit identity metadata, explicit SCIM directory state, and
  fail-closed RBAC decisions;
- audit export satisfies a security review by producing tenant-scoped
  `audit.jsonl`, `siem-rfc5424.log`, `controls.json`, and `manifest.json`,
  with readback hash verification against the physical files.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase8_enterprise_fsv -- --nocapture
```

Current result: 6/6 passing. The suite executes the real binary against real
SQLite ledgers and real export files, then reads `ledger_records`,
`audit.jsonl`, `siem-rfc5424.log`, `controls.json`, and `manifest.json`
directly. It covers the happy path plus invalid OIDC claims, unknown SCIM
role assignment, cross-tenant resource denial, unauthorized audit export, and
hash-chain tamper blocking before export files are written. The hardened
export path persists an `enterprise_access_decision` for both allowed and
denied `export-audit` attempts.

## Phase 9: GitLab CI Hosted Connector — DONE (2026-05-02)

Deliverables (all shipped):

- hosted CI connector `gitlab_ci_v1`;
- `blackbox connector test --connector gitlab_ci_v1 --source GROUP/PROJECT`;
- verifiers `gitlab_pipeline_status_v1` and
  `gitlab_pipeline_jobs_completed_v1`;
- GitLab REST readback with explicit `Accept`, `User-Agent`, optional
  `BBX_GITLAB_TOKEN` / `GITLAB_TOKEN` `PRIVATE-TOKEN` auth, rate-limit
  headers, pagination headers, request ids, response body hashes, and typed
  fail-closed errors;
- Phase 9 FSV test
  (`crates/context-graph-cli/tests/blackbox_phase9_gitlab_ci_fsv.rs`).

Exit criteria (met):

- hosted CI state can be verified without trusting an agent's return value;
- pipeline id, status, ref, and commit SHA must all match before
  `gitlab_pipeline_status_v1` passes;
- pipeline jobs must be present, `minimum_jobs` must be at least 1, every
  paginated jobs page must be read, and every job must match explicit allowed
  statuses before `gitlab_pipeline_jobs_completed_v1` passes;
- missing pipelines/jobs, stale status/ref/SHA expectations, invalid job
  status values, and malformed schema fail with typed error codes and durable
  evidence.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase9_gitlab_ci_fsv -- --nocapture
```

Current result: 1/1 passing. The live integration test independently
discovers a real public GitLab project/pipeline/jobs fixture with `curl`,
executes the real `context-graph blackbox` binary, then reads physical
SQLite `ledger_records` rows. It covers 2 happy paths and 9 edge checks:
missing pipeline, wrong pipeline status, wrong ref, schema-invalid pipeline
id, missing jobs endpoint, wrong allowed job status, minimum-job mismatch,
invalid allowed job status, and zero-minimum job schema blocking.

Manual FSV completed against `/tmp/bbx-phase9-manual-fsv.IT6RIN/fsv.log`.
Final physical SQLite state: `agent_run=1`, `tool_call=1`,
`expected_delta=7`, `readback_snapshot=5`, `verifier_result=7`,
`evidence_packet=7`, `incident_packet=3`. Direct SQLite readback proved the
happy jobs verifier stored `pages_read=1`, `api_pages=1`, `jobs_total=4`, and
`mismatched_count=0`, with the first GitLab jobs API page preserving a
pagination `Link` header; invalid job status and `minimum_jobs=0` were
blocked before source reads and therefore wrote no readback snapshots.

No schema bump. Phase 9 reuses the existing Phase 2-4 evidence path and keeps
the live ledger schema at `LEDGER_SCHEMA_VERSION = 5`.

## Phase 10: Scanner Artifact Connector — DONE (2026-05-02)

Deliverables (all shipped):

- local scanner artifact connector `scanner_artifact_v1`;
- `blackbox connector test --connector scanner_artifact_v1 --source REPORT`;
- OSV JSON report readback that normalizes packages, vulnerability ids,
  aliases, and grouped ids;
- Cargo lockfile readback through TOML parsing, not ad-hoc string matching;
- CycloneDX JSON SBOM readback with explicit `bomFormat`, `specVersion`, and
  component validation;
- verifiers `scanner_vulnerability_absent_v1`,
  `scanner_lockfile_pinned_to_version_v1`, and
  `scanner_sbom_artifact_present_v1`;
- Phase 10 FSV test
  (`crates/context-graph-cli/tests/blackbox_phase10_scanner_fsv.rs`).

Exit criteria (met):

- vulnerability remediation can be verified against the scanner report rather
  than the agent transcript;
- lockfile pinning proves the durable dependency version in `Cargo.lock`;
- SBOM presence proves a CycloneDX artifact exists and contains expected
  components;
- malformed JSON, schema drift, missing files, unsupported formats, missing
  packages, wrong versions, and vulnerability-still-present cases fail closed
  with typed evidence.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase10_scanner_fsv -- --nocapture
```

Current result: 1/1 passing. The suite writes real OSV JSON reports, a real
Cargo lockfile, and real CycloneDX JSON SBOM files into a temporary directory,
executes the real `context-graph blackbox` binary, then opens the physical
SQLite ledger with `rusqlite`. It covers 3 happy paths and 9 edge checks:
vulnerability still present, missing report, invalid report JSON, wrong
lockfile version, missing lockfile package, unsupported lockfile format,
missing SBOM, invalid SBOM schema, and invalid zero component minimum.

Manual FSV completed against `/tmp/bbx-phase10-manual-fsv.Ww4wd6/fsv.log`.
Final physical SQLite state: `agent_run=1`, `tool_call=1`,
`expected_delta=12`, `readback_snapshot=10`, `verifier_result=12`,
`evidence_packet=12`, `incident_packet=5`. Direct file readback proved the
artifact hashes persisted in readbacks:

- clean OSV report:
  `sha256:0cf3d92ab61968b557143878af6bebb17d66c1b0927f38e3a197a669b1850a0b`;
- vulnerable OSV report:
  `sha256:0c15bcc3437d8e19545e80aeee3595f56db157236047c7e74107f13b647b387e`;
- Cargo lockfile:
  `sha256:2f8d405ca4fe677428530293e9be3420df45d436fe501eaf429e79e18c92b20e`;
- CycloneDX SBOM:
  `sha256:52bada329ad56a72d75c80ecb7698a96f21ee0a6c071c5b745fdb960b2cad8a8`.

No schema bump. Phase 10 reuses the existing Phase 2-4 evidence path and
keeps the live ledger schema at `LEDGER_SCHEMA_VERSION = 5`.

## Phase Sequencing Rules

1. Each phase ships behind a single PR set with a `Phase N FSV` test file
   and the exit-criteria checklist marked `[x]`.
2. Schema bumps wipe the ledger. No migration shims, no compatibility flags.
3. No phase begins until the previous phase's FSV test file is green on a
   fresh checkout.
4. If a phase's exit criterion cannot be verified by reading a physical
   source of truth, the phase is incomplete — re-open it.
