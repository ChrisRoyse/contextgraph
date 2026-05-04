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
  **wiped** the ledger — no migration shim; current shipped schema is v13
  after Phase 24).

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
the then-live ledger schema at `LEDGER_SCHEMA_VERSION = 5`; after the reward compiler,
fresh ledgers initialize at schema version 14.

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
- filtered vulnerability absence requires the OSV report to contain at least
  one package matching the requested package/ecosystem/version filters;
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
SQLite ledger with `rusqlite`. It covers 3 happy paths and 12 edge checks:
vulnerability still present, missing report, report package-filter miss,
directory artifact path, invalid report JSON, wrong lockfile version, missing
lockfile package, unsupported lockfile format, missing SBOM, invalid SBOM
schema, invalid zero component minimum, and component version without
component name.

Automated hardened FSV now asserts final verifier/evidence counts of
`expected_delta=15`, `readback_snapshot=11`, `verifier_result=15`,
`evidence_packet=15`, and `incident_packet=6`.

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

Focused hardening manual FSV completed against
`/tmp/bbx-hardening-fsv.ch1qc2hp/fsv.log`. It proved the new
`SCANNER_REPORT_PACKAGE_NOT_FOUND`, `SCANNER_ARTIFACT_NOT_FILE`, and
`SCANNER_SBOM_COMPONENT_VERSION_WITHOUT_NAME` cases with final focused counts
`expected_delta=4`, `readback_snapshot=2`, `verifier_result=4`,
`evidence_packet=4`, and `incident_packet=1`.

No schema bump. Phase 10 reuses the existing Phase 2-4 evidence path and
keeps the then-live ledger schema at `LEDGER_SCHEMA_VERSION = 5`; after the
reward compiler, fresh ledgers initialize at schema version 14.

## Phase 11: GitHub Issue Evidence-Link Verification — DONE (2026-05-02)

Deliverables (all shipped):

- `github_issue_evidence_link_v1` verifier on the existing `github_v1`
  connector surface;
- GitHub issue body readback through
  `GET /repos/{owner}/{repo}/issues/{issue_number}`;
- GitHub issue-comments readback through
  `GET /repos/{owner}/{repo}/issues/{issue_number}/comments` with
  `Link`-header pagination;
- exact evidence URL validation for `http`/`https` URLs only;
- `expected.link_scope` support for `body`, `comments`, and
  `body_or_comments`;
- pull-request guard: issue API responses that contain `pull_request` fail as
  `GITHUB_ISSUE_IS_PULL_REQUEST`, so PRs cannot satisfy ticket evidence;
- Phase 11 FSV test
  (`crates/context-graph-cli/tests/blackbox_phase11_github_issue_evidence_fsv.rs`).

Exit criteria (met):

- remediation evidence can be verified against the real ticket body/comments
  rather than the agent transcript;
- missing issues, missing evidence links, PR-vs-issue ambiguity, invalid
  issue numbers, invalid repository source segments, invalid evidence URLs,
  invalid scopes, schema drift, malformed pagination, and permission/rate-limit
  errors fail closed with typed evidence;
- readback snapshots persist issue metadata, issue body SHA/length, comment
  page snapshots, pagination headers, matched locations, and the requested
  evidence URL;
- blocked invalid specs write expected/verifier/evidence rows without
  readback rows, making it explicit that no source read occurred.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase11_github_issue_evidence_fsv -- --nocapture
```

Current result: 1/1 passing. The live integration test independently
discovers a real public GitHub issue whose body contains an `http(s)` URL,
executes the real `context-graph blackbox` binary, then opens the physical
SQLite ledger with `rusqlite`. It covers connector exposure, the happy
evidence-link verifier, missing issue, wrong evidence URL, issue endpoint
returning a pull request, invalid evidence URL, invalid link scope, invalid
issue number, and invalid repository source segments.

Automated hardened FSV now asserts final verifier/evidence counts of
`expected_delta=8`, `readback_snapshot=4`, `verifier_result=8`,
`evidence_packet=8`, and `incident_packet=3`.

Manual FSV completed against `/tmp/bbx-phase11-manual-fsv.zftsp_oa/fsv.log`.
Final physical SQLite state: `agent_run=1`, `tool_call=1`,
`expected_delta=6`, `readback_snapshot=4`, `verifier_result=6`,
`evidence_packet=6`, `incident_packet=3`. Direct GitHub readback proved
`actions/checkout#2424` was an open issue whose body contained
`https://github.com/xxxxx/xxxxx` with issue body SHA
`sha256:06f5ae49bd5152ec5da468bda6d879e37f0e002e59277fcc5f11ebeb659bc4e5`.

Focused hardening manual FSV completed against
`/tmp/bbx-hardening-fsv.ch1qc2hp/fsv.log`. It proved
`GITHUB_ISSUE_NUMBER_INVALID` and `GITHUB_REPO_LOCATOR_INVALID` block before
source reads, plus a wrong-link failure with incident evidence, with final
focused counts `expected_delta=4`, `readback_snapshot=2`,
`verifier_result=4`, `evidence_packet=4`, and `incident_packet=1`.

No schema bump. Phase 11 reuses the existing Phase 2-4 evidence path and
keeps the then-live ledger schema at `LEDGER_SCHEMA_VERSION = 5`; after the
reward compiler, fresh ledgers initialize at schema version 14.

## Phase 12: Deterministic Scorecards — DONE (2026-05-03)

Deliverables (all shipped):

- scoring connector `blackbox_scoring_v1`;
- `blackbox score run --ledger PATH --run-id RUN_ID`;
- append-only `scorecard` ledger row with `score_rule_version = score_v1`;
- schema bump to `LEDGER_SCHEMA_VERSION = 6` for the new ledger record kind;
- deterministic score dimensions: correctness, verification confidence, risk,
  policy compliance, training value, and trust level;
- score inputs that expose exact row counts, evidence coverage, verifier
  status counts, policy counts, connector reliability, and evidence-ref
  verification;
- dashboard/export surface additions for `scorecard` rows and score commands;
- Phase 12 FSV test
  (`crates/context-graph-cli/tests/blackbox_phase12_scorecard_fsv.rs`).

Exit criteria (met):

- scoring reads the physical SQLite ledger after hash-chain verification and
  never trusts an in-memory command return value;
- evidence packet references must match physical ledger rows before a scorecard
  is produced;
- missing runs, malformed scoring input rows, evidence without tool calls, and
  scorecard readback mismatch fail closed with typed errors;
- a successful score append is immediately read back from `ledger_records`
  and matched by sequence, `scorecard_id`, `run_id`, `payload_sha256`, and
  `record_sha256`;
- previous scorecards are excluded from the next score computation so the same
  run's trust/risk values are stable across repeated scoring snapshots.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase12_scorecard_fsv -- --nocapture
```

Current result: 1/1 passing. The suite executes the real binary against a real
SQLite ledger and a real local Git repository. It covers a verified
policy-allowed happy path plus no-tool, unverified-tool, failed-verifier
incident, and missing-run edge cases. Final physical SQLite counts were
`agent_run=4`, `policy_decision=1`, `tool_call=3`, `expected_delta=2`,
`readback_snapshot=2`, `verifier_result=2`, `evidence_packet=2`,
`incident_packet=1`, and `scorecard=4`.

Manual FSV completed against `/tmp/bbx-phase12-manual-fsv.1VNO0R/fsv.log`
with log SHA-256
`b8da7e8aca9aa7af7b8ff6571bef790849b1d64a99add3d9ddc4c159d5c81958`.
The final direct SQLite scorecard readback proved:

```text
8|verified_with_policy|1.0|0.58|0.9725
10|recorded||0.2|0.0
13|recorded||1.0|0.0
21|incident|0.0|0.5|0.9725
```

Phase 12 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 6` for
the new `scorecard` record kind. The physical table shape is unchanged, but
older schema-version ledgers are wiped on `blackbox init`; no migration shim is
provided.

## Phase 13: Privacy Retention And Erasure Decisions — DONE (2026-05-03)

Deliverables (all shipped):

- privacy connector `blackbox_privacy_v1`;
- `blackbox privacy evaluate-retention --ledger PATH --tenant TENANT --run-id RUN_ID`;
- `blackbox privacy request-erasure --ledger PATH --tenant TENANT --run-id RUN_ID`;
- RBAC-gated authorization through `enterprise_access_decision` with
  `privacy.review` and `privacy.erase` permissions;
- append-only `retention_decision` and `evidence_deletion` ledger rows with
  `privacy_schema_version = 1`;
- data-subject minimization: raw `--data-subject` is rejected if empty or
  secret-like and is persisted only as `data_subject_sha256`;
- legal-hold and incident-evidence erasure denials with durable reason codes;
- connector/API/dashboard surface additions for privacy decision rows;
- Phase 13 FSV test
  (`crates/context-graph-cli/tests/blackbox_phase13_privacy_retention_fsv.rs`).

Exit criteria (met):

- privacy commands read the physical SQLite ledger after hash-chain
  verification and never trust command return values alone;
- the parent run must exist for the tenant before any privacy decision row is
  created;
- every authorized attempt appends and reads back an
  `enterprise_access_decision`; unauthorized attempts fail closed with
  `PRIVACY_RBAC_DENIED` and create no privacy decision row;
- retention classes are resolved from the latest
  `enterprise_tenant_config.retention`; unknown classes fail as
  `PRIVACY_RETENTION_CLASS_NOT_FOUND`;
- successful privacy appends are immediately read back from `ledger_records`
  and matched by sequence, `privacy_decision_id`, `run_id`, `payload_sha256`,
  and `record_sha256`;
- raw data subjects are not present in persisted `payload_json`.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase13_privacy_retention_fsv -- --nocapture
```

Current result: 1/1 passing. The suite executes the real binary against a real
SQLite ledger with real enterprise config and SCIM directory rows. It covers a
retention happy path, approved erasure, legal-hold erasure denial, missing
retention class, incident-evidence erasure denial, unauthorized principal,
missing run, and empty data subject.
Final physical SQLite counts were `enterprise_tenant_config=1`,
`enterprise_directory_snapshot=1`, `agent_run=6`,
`enterprise_access_decision=6`, `retention_decision=1`,
`evidence_deletion=3`, `tool_call=1`, `expected_delta=1`,
`readback_snapshot=1`, `verifier_result=1`, `evidence_packet=1`, and
`incident_packet=1`.

Manual FSV completed against `/tmp/bbx-phase13-manual-fsv.ve11jv/fsv.log`
with log SHA-256
`91f0a2dd062ddc50d59eca23a719c0ca8590a378cb90a31b38182b86008350b7`.
The final direct SQLite readback proved `retention_decision=1`,
`evidence_deletion=3`, `enterprise_access_decision=6`, `incident_packet=1`,
no raw data subject in `payload_json`, and the expected data-subject digest
present in 4 privacy rows.

Phase 13 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 7` for
`retention_decision` and `evidence_deletion`. The physical table shape is
unchanged, but older schema-version ledgers are wiped on `blackbox init`; no
migration shim is provided.

## Phase 14: Release Gate Decisions — DONE (2026-05-03)

Deliverables (all shipped):

- release-gate connector `blackbox_release_gate_v1`;
- `blackbox gate evaluate --ledger PATH --tenant TENANT --run-id RUN_ID`;
- RBAC-gated authorization through `enterprise_access_decision` with the
  `release.approve` permission;
- append-only `release_gate` ledger rows with
  `release_gate_schema_version = 1`;
- deterministic gate checks over latest scorecard trust/risk, policy
  compliance, evidence packet references, verifier status, incident packets,
  and optional privacy retention state;
- connector/API/dashboard surface additions for release gate rows;
- Phase 14 FSV test
  (`crates/context-graph-cli/tests/blackbox_phase14_release_gate_fsv.rs`).

Exit criteria (met):

- gate evaluation reads the physical SQLite ledger after hash-chain
  verification and never trusts CLI return values alone;
- the parent run must exist for the tenant before authorization;
- every authorization attempt appends and reads back an
  `enterprise_access_decision`; unauthorized attempts fail closed with
  `RELEASE_GATE_RBAC_DENIED` and create no `release_gate` row;
- approved and blocked release decisions are durable `release_gate` rows;
- invalid thresholds or environment/trust inputs fail before any new row is
  written;
- successful gate appends are immediately read back from `ledger_records` and
  matched by sequence, `release_gate_id`, `run_id`, `payload_sha256`, and
  `record_sha256`.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase14_release_gate_fsv -- --nocapture
```

Current result: 1/1 passing. The suite executes the real binary against a real
SQLite ledger, real enterprise config and SCIM directory rows, and a real
local Git repository. It covers an approved gate, missing scorecard,
too-low max-risk threshold, unauthorized principal, incident-bearing run,
invalid max-risk input, and connector readback.
Final physical SQLite counts were `enterprise_tenant_config=1`,
`enterprise_directory_snapshot=1`, `agent_run=4`, `policy_decision=3`,
`tool_call=3`, `expected_delta=3`, `readback_snapshot=3`,
`verifier_result=3`, `evidence_packet=3`, `incident_packet=1`,
`scorecard=3`, `retention_decision=3`, `enterprise_access_decision=8`, and
`release_gate=4`.

Manual FSV completed against `/tmp/bbx-phase14-manual-fsv.JAJAYu/fsv.log`
with log SHA-256
`e22a6ee2996cce9757db8401bd298a01a1ea693f5b12204493f6c7379d39ca0e`.
The final direct SQLite readback proved four release gate rows:

```text
14|run_adfe20f23678429f896474ae0bf26ed4|approved|RELEASE_GATE_APPROVED|
17|run_65e6869f7a7141ddbfbd5baeea97c0cc|blocked|RELEASE_GATE_BLOCKED|RELEASE_GATE_SCORECARD_REQUIRED
19|run_adfe20f23678429f896474ae0bf26ed4|blocked|RELEASE_GATE_BLOCKED|RELEASE_GATE_RISK_TOO_HIGH
33|run_de485dc4e6bf4933905108932e6e32b1|blocked|RELEASE_GATE_BLOCKED|RELEASE_GATE_TRUST_TOO_LOW
```

Final manual integrity was `schema_version=8`, `record_count=33`, and
`status=passed`; `blackbox_metadata.record_count` and
`blackbox_metadata.ledger_tip_sha256` matched the physical ledger rows.

Phase 14 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 8` for
`release_gate`. The physical table shape is unchanged, but older
schema-version ledgers are wiped on `blackbox init`; no migration shim is
provided.

## Phase 15: Pilot Packaging And Usage Meter Proof — DONE (2026-05-03)

Deliverables (all shipped):

- `blackbox package pilot-report --ledger PATH --tenant TENANT
  --principal-id PRINCIPAL --pilot-id PILOT --min-actions N --out DIR
  [--force] --agent AGENT`;
- connector `blackbox_packaging_v1`;
- ledger record kind `pilot_package`;
- package files `pilot-report.json`, `usage-meter-events.jsonl`,
  `eval-candidates.jsonl`, and `manifest.json`;
- dashboard package readback file `api/pilot-packages.json`;
- usage meter events with stable `event_name`, `customer_identifier`, numeric
  `value`, timestamp, idempotency `identifier`, dimensions, and ledger source
  refs;
- eval-candidate JSONL rows derived from failed/blocked verifier, incident,
  policy denial, or blocked release-gate evidence.

Exit criteria (verified):

- authorized package builds persist one `enterprise_access_decision`, write
  every package file, parse every JSON/JSONL file back, append one
  `pilot_package` row, read the row back by id/hash/sequence, and verify final
  manifest file hashes;
- pilot success requires the configured minimum real actions, complete
  mutating-tool evidence coverage, at least one eval candidate, and at least
  one release-gate decision;
- unmet success criteria produce durable package evidence and return
  `PILOT_PACKAGE_CRITERIA_NOT_MET`;
- unauthorized principals persist only a denied access decision and create no
  `pilot_package` row/files;
- invalid request fields such as `--min-actions 0` fail before authorization,
  row writes, or file creation;
- the dashboard exposes physical package rows through
  `api/pilot-packages.json`.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase15_pilot_package_fsv -- --nocapture
```

Current result: 1/1 passing. Coverage includes a successful package over real
SQLite/Git/enterprise/score/privacy/release-gate rows, unmet minimum-action
criteria, unauthorized package principal, invalid minimum actions, connector
test readback, package manifest/hash readback, and dashboard package API
readback.

Phase 15 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 9` for
`pilot_package`. The physical table shape is unchanged, but older
schema-version ledgers are wiped on `blackbox init`; no migration shim is
provided.

## Phase 16: Risks And Open Questions Closure — DONE (2026-05-03)

Deliverables (all shipped):

- `blackbox risk evaluate --ledger PATH --tenant TENANT
  --principal-id PRINCIPAL --decisions DECISIONS_JSON --agent AGENT
  [--model MODEL]`;
- connector `blackbox_risk_v1`;
- ledger record kind `risk_assessment`;
- dashboard risk readback file `api/risk-assessments.json`;
- dashboard summary count `risk_assessments_total` and risk error grouping;
- required decision-register schema for the five Phase 16 open-question ids:
  `next_hosted_ci_connector`, `paid_pilot_dashboard_minimum`,
  `raw_model_text_storage_default`, `customer_managed_key_path`, and
  `eval_export_format_after_jsonl`.

Exit criteria (verified):

- a risk review cannot pass from prose alone; the required decision-register
  JSON must contain exactly one closed, future-reviewed, owner/rationale/
  evidence-backed entry for every required id;
- authorized evaluations persist one `enterprise_access_decision`, inspect the
  real SQLite ledger/runtime state, append one `risk_assessment`, and read it
  back by id/hash/sequence;
- missing pilot evidence, unresolved incident-to-eval conversion, runtime
  durability drift, secret-like payload findings, failed/blocked verifiers,
  denied policies, missing enterprise controls, or open decisions block
  readiness with durable `decision.status=blocked`;
- unauthorized principals persist only a denied access decision and create no
  `risk_assessment` row;
- invalid decision registers fail before authorization and create no rows;
- the dashboard exposes physical risk rows through
  `api/risk-assessments.json`.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase16_risk_register_fsv -- --nocapture
```

Current result: 1/1 passing. Coverage includes blocked pre-package risk
posture, accepted risk posture after a passed pilot package exists,
unauthorized principal, invalid decision register, connector readback,
dashboard/API readback, and direct SQLite proof.

Phase 16 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 10` for
`risk_assessment`. The physical table shape is unchanged, but older
schema-version ledgers are wiped on `blackbox init`; no migration shim is
provided.

## Phase 17: Authenticated HTTP Server — DONE (2026-05-03)

Deliverables:

- `blackbox server serve --ledger PATH --bind 127.0.0.1:7607`;
- required `Authorization: Bearer <BLACKBOX_API_KEY>` authentication for all
  protected endpoints;
- required `X-Blackbox-Tenant` tenant scope and request/body tenant matching;
- loopback bind by default, with non-loopback binds refused unless
  `--allow-remote-bind --tls-terminated-by NAME` is explicitly provided;
- single in-process writer lock around ledger mutation and HTTP audit append;
- served endpoints for run creation/readback, step creation, tool-call
  creation, and tenant-scoped dashboard read models;
- bounded 64 KiB request bodies and strict JSON schemas with unknown-field
  rejection;
- durable `http_request_audit` rows for success, authentication failure,
  tenant mismatch, schema failure, and body-limit failure.

Exit criteria (verified):

- HTTP-created runs persist as real `agent_run` rows with
  `runtime.framework="http_server"`;
- every protected request creates a physical `http_request_audit` row;
- tenant-scoped dashboard responses do not expose other tenants' rows;
- invalid API keys, tenant/header mismatches, and oversized bodies fail closed
  with typed HTTP JSON errors and no business row;
- source-of-truth proof reads `ledger_records` directly after each trigger.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase17_http_server_fsv -- --nocapture
```

Current result: 1/1 passing. Coverage starts a real server process, sends real
HTTP requests with `reqwest`, then opens SQLite directly to prove persisted
`agent_run`, `agent_step`, `tool_call`, and `http_request_audit` rows. Edge
cases cover invalid auth, tenant mismatch, and oversized request bodies.

Phase 17 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 11` for
`http_request_audit`. The physical table shape is unchanged, but older
schema-version ledgers are wiped on `blackbox init`; no migration shim is
provided.

## Phase 18: First-Wedge Demo Package — DONE (2026-05-03)

Deliverables:

- `blackbox demo first-wedge --ledger PATH --tenant TENANT --run-id RUN_ID
  --out DIR [--force] --agent AGENT`;
- connector `blackbox_first_wedge_demo_v1`;
- ledger record kind `first_wedge_demo`;
- package files `demo-report.json`, `evidence-index.json`,
  `demo-script.jsonl`, and `manifest.json`;
- required proof categories for the complete vulnerability-remediation buyer
  demo: Git delta, CI success, scanner vulnerability absence, ticket evidence
  link, CI failure demo, scanner-still-present failure demo, and permission
  denied connector demo.

Exit criteria (verified):

- the command fails closed unless all seven required proof categories exist as
  physical `verifier_result` plus `evidence_packet` rows for the selected run;
- successful packages write every file, parse JSON/JSONL back, verify file
  hashes/byte counts from `manifest.json`, append one `first_wedge_demo` row,
  and read that row back from SQLite by sequence/id/hash;
- missing runs, incomplete evidence, and existing output directories create no
  `first_wedge_demo` row and no final package files;
- source-of-truth proof covers a real local Git repository, real OSV JSON
  scanner artifacts, live GitHub Actions run state, live GitHub issue evidence
  links, and direct SQLite readback.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase18_first_wedge_demo_fsv -- --nocapture
```

Current result: 1/1 passing. The test builds a synthetic vulnerability
remediation over a real local Git repository and scanner artifacts, discovers
live GitHub/GitHub Actions fixture state, persists happy and required failure
verifier evidence, builds the demo package, reads package files and manifest
hashes back, tests the connector readback, and prints before/after SQLite
state for the happy path plus three edge cases.

Phase 18 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 12` for
`first_wedge_demo`. The physical table shape is unchanged, but older
schema-version ledgers are wiped on `blackbox init`; no migration shim is
provided.

## Phase 19: Executable Full-State Verification Protocol — DONE (2026-05-03)

Deliverables:

- `blackbox fsv certify --ledger PATH --tenant TENANT --phase N --evidence
  FILE --out DIR [--force] [--run-id RUN_ID] --agent AGENT`;
- connector `blackbox_fsv_protocol_v1`;
- ledger record kind `full_state_verification`;
- package files `fsv-report.json`, `provenance.json`, `edge-cases.jsonl`,
  and `manifest.json`;
- strict evidence schema for trigger command, pre/post source state,
  expected/actual deltas, independent read queries, three mandatory edge
  categories, artifact hashes, and SQLite integrity check.

Exit criteria (verified):

- the command fails closed unless the evidence source-of-truth string matches
  the physical `--ledger` path, the post-state matches a fresh SQLite read,
  and expected/actual deltas match `post_state - pre_state`;
- evidence must include `missing_target_state`, `wrong_target_state`, and
  `permission_or_schema_failure` edge cases with typed non-zero errors and
  before/after state;
- declared artifacts are read from disk and checked by SHA-256 and byte count;
- successful packages write every file, parse JSON/JSONL back, verify
  manifest hashes/byte counts, append one `full_state_verification` row, and
  read that row back from SQLite by sequence/id/hash;
- missing evidence files, mismatched post-state, artifact hash mismatch, and
  existing output directories create no `full_state_verification` row and no
  final package files.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase19_fsv_protocol_fsv -- --nocapture
```

Current result: 1/1 passing. The test records real ledger rows through the
CLI, captures real SQLite/file readbacks, exercises three fail-closed edge
cases plus output-exists behavior, validates a successful FSV package, verifies
package manifest hashes, reads the persisted `full_state_verification` payload
from SQLite, and checks connector readback.

Phase 19 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 13` for
`full_state_verification`. The physical table shape is unchanged, but older
schema-version ledgers are wiped on `blackbox init`; no migration shim is
provided.

## Phase 20: Kubernetes Deployment Verification — DONE (2026-05-03)

Deliverables:

- connector `k8s_deployment_snapshot_v1`;
- connector test against a physical `kubectl get deployments.v1.apps -o json`
  snapshot file;
- verifiers `k8s_deployment_image_digest_v1`,
  `k8s_replicas_match_v1`, and `k8s_rollout_completed_v1`;
- strict parsing for `apps/v1` `Deployment` and `DeploymentList` JSON;
- fail-closed errors for missing snapshot, invalid JSON, schema drift,
  missing Deployment/container, unpinned images, digest mismatch, replica
  mismatch, stale observed generation, failed rollout conditions, and
  unsupported expected rollout states.

Exit criteria (verified):

- connector list/test exposes the Kubernetes connector and reads physical file
  path, byte count, SHA-256, Deployment metadata/spec/status, containers, and
  conditions;
- image verification passes only when the target container image is pinned to
  the exact expected OCI `sha256` digest;
- replica verification requires exact expected spec/status counters;
- rollout verification requires `Available=True`, `Progressing=True` with
  `reason=NewReplicaSetAvailable`, no `ReplicaFailure=True`, current
  observed generation, all desired replicas updated/ready/available, and zero
  unavailable replicas;
- failed mismatches persist `incident_packet` rows, while blocked schema
  failures persist verifier/evidence rows without hiding the failure.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase20_k8s_deployment_fsv -- --nocapture
```

Current result: 1/1 passing. The test writes real kubectl-style JSON files,
runs the real CLI, verifies happy image/replica/rollout paths, exercises
missing Deployment, wrong digest, and invalid JSON edges, and reads the
physical SQLite ledger to prove expected/readback/verifier/evidence/incident
rows exist.

Phase 20 adds no new ledger record kind and does not bump the physical schema;
current ledgers remain at `LEDGER_SCHEMA_VERSION = 14` after the reward compiler.

## Phase 21: Terraform And Cloud Artifact Verification — DONE (2026-05-03)

Deliverables:

- connector `terraform_cloud_artifact_v1`;
- connector test against a physical Terraform plan JSON file produced by
  `terraform show -json`;
- verifiers `tf_plan_only_approved_changes_v1`,
  `tf_state_attribute_v1`, `cloud_resource_present_v1`, and
  `cloud_audit_principal_action_v1`;
- strict parsing for Terraform JSON format major version `1`, Terraform plan
  `resource_changes`/`output_changes`, Terraform state `values.root_module`
  resources and child modules, normalized cloud inventory JSON, and normalized
  cloud audit JSONL;
- fail-closed errors for missing artifacts, invalid JSON/JSONL, Terraform
  schema drift, unsupported format versions, unapproved plan changes, missing
  approved changes, missing state resources/attributes, sensitive state paths,
  cloud resource absence, attribute mismatch, and missing audit proof.

Exit criteria (verified):

- connector list/test exposes the Terraform/cloud artifact connector and reads
  physical plan path, byte count, SHA-256, Terraform format version, changed
  resource count, and changed output count;
- plan verification passes only when every non-`no-op` resource/output change
  exactly matches the explicit approval set;
- state verification reads physical Terraform state JSON and compares the
  exact non-sensitive attribute value at the declared path;
- cloud inventory verification reads a physical normalized inventory artifact
  and checks exact provider/type/id/region plus required attributes;
- cloud audit verification reads physical JSONL and requires a matching
  principal/action/resource/outcome event;
- failed mismatches persist `incident_packet` rows, while blocked schema
  failures persist verifier/evidence rows without hiding the failure.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase21_terraform_cloud_fsv -- --nocapture
```

Current result: 1/1 passing. The test writes real Terraform plan/state JSON,
cloud inventory JSON, and cloud audit JSONL artifacts, runs the real CLI,
verifies happy Terraform plan/state/cloud resource/cloud audit paths, exercises
unapproved plan, wrong state attribute, and invalid audit JSONL edges, and
reads the physical SQLite ledger to prove expected/readback/verifier/evidence/
incident rows exist.

Phase 21 adds no new ledger record kind and does not bump the physical schema;
current ledgers remain at `LEDGER_SCHEMA_VERSION = 14` after the reward compiler.

## Phase 22: Ticketing Artifact Verification — DONE (2026-05-03)

Deliverables:

- connector `ticketing_artifact_v1`;
- connector test against a physical Jira REST issue/search JSON artifact,
  Linear GraphQL issue JSON artifact, or ServiceNow Table API record JSON
  artifact;
- verifiers `jira_ticket_state_v1`, `linear_ticket_state_v1`, and
  `servicenow_ticket_state_v1`;
- strict parsing for documented Jira issue/search shapes, Linear GraphQL
  issue/nodes/edges shapes, and ServiceNow `result` record/list shapes;
- exact ticket identity/status checks plus optional exact evidence URL
  verification across all string fields in the captured API artifact;
- fail-closed errors for missing artifacts, invalid JSON, schema drift,
  unsupported source formats, duplicate ticket identities, missing tickets,
  status mismatches, and missing evidence links.

Exit criteria (verified):

- connector list/test exposes the ticketing connector and reads physical file
  path, byte count, SHA-256, normalized provider, ticket identity, status,
  assignee, update time, URL, and evidence links;
- Jira verification passes only when `fields.status.name` for the exact issue
  key matches the expected status and the optional evidence URL is present;
- Linear verification passes only when `state.name` for the exact issue
  identifier matches the expected state and the optional evidence URL is
  present;
- ServiceNow verification passes only when the exact record number has the
  expected state display/value and the optional evidence URL is present;
- failed mismatches persist `incident_packet` rows, while blocked schema
  failures persist verifier/evidence rows without hiding the failure.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase22_ticketing_fsv -- --nocapture
```

Current result: 1/1 passing. The test writes real Jira, Linear, and
ServiceNow-shaped JSON artifacts, runs the real CLI, verifies happy Jira,
Linear, and ServiceNow ticket state paths, exercises missing ticket, wrong
state, and invalid JSON edges, and reads the physical SQLite ledger to prove
expected/readback/verifier/evidence/incident rows exist.

Phase 22 adds no new ledger record kind and does not bump the physical schema;
current ledgers remain at `LEDGER_SCHEMA_VERSION = 14` after the reward compiler.

## Phase 23: OpenTelemetry Evidence Export — DONE (2026-05-03)

Deliverables:

- command `blackbox export otel --ledger PATH --out DIR [--force]`;
- connector `blackbox_otel_export_v1`;
- export files `traces.jsonl`, `logs.jsonl`, `metrics.jsonl`, and
  `manifest.json`;
- span generation for every verified physical ledger row using deterministic
  trace/span ids derived from ledger/run/record hashes;
- logs for verifier failures, incidents, denied policies, non-ok HTTP audits,
  and export completion;
- low-cardinality metrics for record-kind counts, verifier-status counts,
  policy-decision counts, and verifier pass ratio;
- fail-closed readback for empty ledgers, existing output without `--force`,
  mixed-signal JSONL files, manifest/hash drift, and hash-chain tampering.

Exit criteria (verified):

- export verifies the SQLite hash chain before any files are promoted;
- export does not mutate `ledger_records`;
- each JSONL signal file contains exactly one OTLP top-level key and at least
  one nested signal item;
- `manifest.json` source-ledger count/tip and file hashes match physical
  SQLite/file state after promotion;
- connector test reads the completed export directory and recomputes hashes
  before returning success.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase23_otel_export_fsv -- --nocapture
```

Current result: 4/4 passing. The tests create real SQLite ledgers, run real
command verifiers to produce passed and failed evidence, export OTLP JSONL,
parse and hash the physical files, run connector readback, prove ledger row
counts did not change, and exercise empty-ledger, existing-output, and
tampered-ledger edges.

Phase 23 adds no new ledger record kind and does not bump the physical schema;
current ledgers remain at `LEDGER_SCHEMA_VERSION = 14` after the reward compiler.

## Phase 24: Provider-Neutral SDK Ingestion — DONE (2026-05-03)

Deliverables:

- command `blackbox sdk schema [--ledger PATH]`;
- command `blackbox sdk ingest --ledger PATH --input CLOUDEVENTS_JSON`;
- connector `blackbox_sdk_ingest_v1` for validating physical SDK event files;
- strict CloudEvents 1.0 JSON object/batch contract with a 128-event batch
  limit;
- W3C `traceparent` validation for lowercase hexadecimal shape and non-zero
  trace/parent ids;
- idempotency enforcement on CloudEvents `source` + `id`;
- mapping from SDK events to existing `agent_run`, `agent_step`,
  `model_decision`, and `tool_call` ledger rows;
- fail-closed expected-delta enforcement for mutating tool-call events;
- atomic batch append with separate SQLite readback of every inserted row.

Exit criteria (verified):

- connector test reads and validates the physical CloudEvents file before
  ingest;
- `blackbox sdk ingest` verifies ledger integrity, event idempotency, parent
  run tenancy, redaction, and expected-delta requirements before append;
- batch validation failures append zero rows;
- returned success includes source file hash, before/after ledger state, and
  readback rows from `ledger_records`;
- direct SQLite queries can locate each persisted SDK event through
  `payload.sdk_event.source` and `payload.sdk_event.id`.

Automated FSV:

```bash
cargo test --package context-graph-cli --test blackbox_phase24_sdk_ingest_fsv -- --nocapture
```

Current result: 4/4 passing. The tests write real CloudEvents JSON files, run
the real CLI, verify connector readback, ingest a run plus step/model/tool
events, query physical SQLite rows directly for `payload.sdk_event` metadata
and redacted expected-delta fields, and exercise duplicate source/id, missing
expected delta, empty batch, and invalid traceparent edges.

Phase 24 adds no new ledger record kind and does not bump the physical schema;
current ledgers remain at `LEDGER_SCHEMA_VERSION = 14` after the reward compiler.

## Phase Sequencing Rules

1. Each phase ships behind a single PR set with a `Phase N FSV` test file
   and the exit-criteria checklist marked `[x]`.
2. Schema bumps wipe the ledger. No migration shims, no compatibility flags.
3. No phase begins until the previous phase's FSV test file is green on a
   fresh checkout.
4. If a phase's exit criterion cannot be verified by reading a physical
   source of truth, the phase is incomplete — re-open it.
