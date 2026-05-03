# Source-Of-Truth Connectors

Connectors define how BlackBoxFix reads and verifies real systems. A connector
is not just a tool wrapper. It must provide **independent readback** and
explicit evidence extraction.

Phase 2 shipped four local connectors: `sqlite_local_v1` (the ledger itself),
`git_local_v1`, `fs_local_v1`, and `cmd_local_v1`. Phase 3 shipped read-only
GitHub + GitHub Actions readbacks: `github_v1` and `gha_v1`. Phase 4 reuses
those connectors and adds replay/export behavior over the SQLite and
filesystem sources of truth. Phase 6 adds MCP/policy surfaces
`mcp_stdio_v1` and `blackbox_policy_v1`, and uses `git_local_v1` as the
physical source of truth for the first verified gateway mutation. Phase 8 adds
`enterprise_controls_v1` for tenant config, SCIM directory snapshots,
RBAC/tenant-isolation decisions, and audit/SIEM export readback. Phase 9 adds
`gitlab_ci_v1` for hosted GitLab pipeline and job-state readback. Phase 10
adds `scanner_artifact_v1` for physical OSV report, Cargo lockfile, and
CycloneDX SBOM readback. Phase 11 extends `github_v1` with
`github_issue_evidence_link_v1` for ticket evidence-link verification through
GitHub issue bodies and comments. Phase 12 adds `blackbox_scoring_v1`, which
reads the physical SQLite ledger and verifies scorecard append readback. Phase
13 adds `blackbox_privacy_v1`, which reads the physical ledger plus enterprise
tenant retention/RBAC rows and verifies privacy decision append readback.
Phase 14 adds `blackbox_release_gate_v1`, which reads scorecard, verifier,
evidence, incident, enterprise RBAC, and privacy rows and verifies release
gate append readback. Phase 15 adds `blackbox_packaging_v1`, which reads the
physical ledger, package files, and usage/eval JSONL outputs, then verifies
`pilot_package` append readback. Phase 16 adds `blackbox_risk_v1`, which reads
the physical ledger, SQLite runtime state, package/risk/dashboard rows, and a
required decision-register JSON file, then verifies `risk_assessment` append
readback. Phase 17 adds `blackbox_http_server_v1`, which reads the physical
ledger and verifies durable `http_request_audit` rows produced by the
authenticated HTTP server. Phase 18 adds `blackbox_first_wedge_demo_v1`,
which reads the physical ledger and verifies durable `first_wedge_demo` rows
plus demo package file manifests. Phase 19 adds
`blackbox_fsv_protocol_v1`, which reads the physical ledger, SQLite
`PRAGMA integrity_check`, FSV evidence JSON, declared artifact files, package
manifests, and durable `full_state_verification` rows. Phase 20 adds
`k8s_deployment_snapshot_v1`, which reads physical Kubernetes Deployment JSON
snapshots captured from `kubectl get deployments.v1.apps -o json`. Phase 21
adds `terraform_cloud_artifact_v1`, which reads physical Terraform JSON
plan/state artifacts plus normalized cloud inventory JSON and audit JSONL
artifacts.

## Connector Contract

Each connector declares:

- source kind;
- authentication mode;
- read permissions required;
- mutating capabilities, if any;
- readback capabilities;
- supported verifier specs;
- rate limits and retry semantics;
- redaction policy;
- evidence artifact types;
- failure error codes.

Reference shape:

```json
{
  "connector_id": "github_v1",
  "source_kind": "github",
  "capabilities": [
    "repo.read", "pull_request.read", "pull_request.write", "checks.read"
  ],
  "readback_specs": [
    "github_pr_open_v1",
    "github_check_suite_passed_v1",
    "github_branch_head_matches_commit_v1"
  ],
  "fail_closed": true
}
```

## Required Connector Properties

- Readback **must** use the source system, not the agent's return payload.
- Readback **must** be separately invocable for manual FSV (CLI subcommand
  `blackbox connector test --connector ... --source ...`).
- Mutating calls must produce correlation IDs.
- Connector errors must be typed with the `BBX_*` / connector-specific codes.
- Missing permission must be `blocked`, not `passed`.
- Rate-limit exhaustion must be visible in the ledger (Phase 3).
- Data redaction must happen **before** persistence — the Phase 1
  `redact_v1` post-scan is the model: any unredacted secret-like literal
  outside an explicitly redacted field is a fatal error
  (`BBX_REDACTION_VIOLATION`).

## Priority Connectors

### Phase 2: Git Local (filesystem only, no network) — shipped

Source of truth:

- `.git` object database (read via the real `git` binary);
- working tree status;
- commit graph;
- diff output;
- tags and refs.

Verifier examples (`verifier_id`):

- `git_branch_exists_v1`
- `git_commit_exists_v1`
- `git_file_diff_matches_patch_v1`
- `git_working_tree_clean_v1`

### Phase 2: Local Filesystem — shipped

Source of truth: a file path on disk. The connector returns file existence,
size, SHA-256, and UTF-8 content when decodable. Phase 2 exposes this through
`blackbox connector test`; no mutating filesystem verifier ships yet.

### Phase 2: Local Test Command — shipped

Source of truth: the exit code, stdout/stderr, and any artifact files of a
declared test command. Shipped verifier:

- `cmd_exit_zero_v1`

### Phase 3: GitHub — shipped

Source of truth:

- repository metadata; pull requests; branches; commits; checks; reviews;
  issues; deployments.

Shipped verifiers:

- `github_pr_state_v1`
- `github_branch_head_matches_commit_v1`
- `github_check_suite_passed_v1`
- `github_issue_state_changed_v1`
- `github_issue_evidence_link_v1`

The connector uses GitHub REST API `GET` endpoints only. It sends
`Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`,
and a stable user agent. `BBX_GITHUB_TOKEN` or `GITHUB_TOKEN` is optional for
public resources and required by operators for private resources.

`github_issue_evidence_link_v1` reads the issue endpoint and, when requested,
the issue-comments endpoint across all `Link`-paginated pages. It verifies an
exact absolute `http(s)` evidence URL in the issue body, comments, or either
location. API responses that include `pull_request` are failed as
`GITHUB_ISSUE_IS_PULL_REQUEST`; pull requests cannot satisfy ticket evidence.
Invalid owner/repo source segments, invalid issue numbers, malformed/incomplete
pagination, invalid evidence URLs, and invalid scopes are blocked typed errors,
not partial passes.

### Phase 3/9: CI Providers — GitHub Actions + GitLab CI shipped

Shipped hosted CI targets: GitHub Actions and GitLab CI. Add later:
Buildkite and CircleCI.

Shipped verifier:

- `gha_workflow_run_completed_v1`
- `gitlab_pipeline_status_v1`
- `gitlab_pipeline_jobs_completed_v1`

`gitlab_ci_v1` reads GitLab REST API v4 on GitLab.com. Public projects can be
read without a token; private projects require `BBX_GITLAB_TOKEN` or
`GITLAB_TOKEN`, sent as `PRIVATE-TOKEN`. Connector evidence records the
request URL, HTTP status, request id, ETag, pagination headers, rate-limit
headers, response body SHA-256, and normalized pipeline/job fields.
`gitlab_pipeline_jobs_completed_v1` reads every jobs page using GitLab
`Link`/`x-next-page` pagination headers, persists each API page in the
readback snapshot, blocks malformed or incomplete pagination, and rejects
`minimum_jobs=0` before any source read so an empty pipeline cannot be
misreported as complete.

Deferred examples:

- `gha_required_jobs_passed_v1`
- `gha_artifact_present_v1`
- `gha_test_report_no_failures_v1`

### Phase 17: Authenticated HTTP Server — shipped

Source of truth:

- real HTTP request/response traffic to `blackbox server serve`;
- SQLite `ledger_records` rows for business records and `http_request_audit`;
- SQLite `blackbox_metadata` count/tip rows.

Readback specs:

- `http_request_audit_v1`;
- `tenant_scoped_dashboard_v1`.

Connector properties:

- API key is read from an environment variable only and compared by hash;
- protected endpoints require `X-Blackbox-Tenant`;
- request body tenant must match the tenant header;
- POST bodies are limited to 64 KiB;
- all protected requests write an audit row, including auth/schema failures;
- non-loopback binds fail closed unless explicitly paired with
  `--allow-remote-bind --tls-terminated-by NAME`.

### Phase 18: First-Wedge Demo Package — shipped

Source of truth:

- SQLite `ledger_records` rows for the selected run;
- evidence packets and verifier results for Git, CI, scanner, ticket, and
  failure-demo proofs;
- package files `demo-report.json`, `evidence-index.json`,
  `demo-script.jsonl`, and `manifest.json`.

Readback specs:

- `blackbox_first_wedge_demo_v1`;
- `first_wedge_demo_manifest_v1`.

Connector properties:

- fails closed unless all seven required proof categories exist;
- requires passed Git diff, CI, scanner absence, and ticket evidence proofs;
- requires failed/blocked CI, scanner-still-present, and permission-denied
  proofs;
- writes package files through a temp directory, reads every file back, hashes
  files in the manifest, then appends and reads back one `first_wedge_demo`
  row;
- `blackbox connector test --connector blackbox_first_wedge_demo_v1 --source
  LEDGER_PATH` verifies physical ledger integrity and reports demo row counts.

### Phase 19: Executable FSV Protocol — shipped

Source of truth:

- SQLite `ledger_records` and `blackbox_metadata` rows;
- SQLite `PRAGMA integrity_check`;
- FSV evidence JSON supplied to `--evidence`;
- declared physical artifact files with SHA-256 and byte counts;
- package files `fsv-report.json`, `provenance.json`, `edge-cases.jsonl`,
  and `manifest.json`.

Readback specs:

- `blackbox_full_state_verification_v1`;
- `fsv_evidence_manifest_v1`;
- `sqlite_integrity_check_v1`.

Connector properties:

- evidence `source_of_truth` must match the exact ledger path being certified;
- evidence post-state must match a fresh physical SQLite read;
- expected and actual deltas must match field by field;
- all three mandatory edge categories must be present;
- every declared artifact is read from disk and hash/byte checked;
- package files are written to a temp directory, read back, manifest-checked,
  then one `full_state_verification` row is appended and read back by
  sequence/id/hash;
- `blackbox connector test --connector blackbox_fsv_protocol_v1 --source
  LEDGER_PATH` verifies ledger integrity, runs SQLite integrity check, and
  reports FSV certification row counts.

### Phase 20: Kubernetes Deployment Snapshots — shipped

Source of truth:

- physical JSON file produced by
  `kubectl get deployments.v1.apps --namespace <namespace> -o json`;
- SQLite verifier/evidence rows generated by `blackbox verify`.

Readback specs:

- `k8s_deployment_image_digest_v1`;
- `k8s_replicas_match_v1`;
- `k8s_rollout_completed_v1`.

Connector properties:

- accepts only `apps/v1` `Deployment` or `DeploymentList` JSON;
- records snapshot path, byte count, SHA-256, parsed Deployment metadata,
  pod-template containers/images, replica counters, and status conditions;
- image verification requires an immutable OCI `sha256` digest;
- rollout verification requires fresh observed generation, `Available=True`,
  `Progressing=True` with `NewReplicaSetAvailable`, no `ReplicaFailure=True`,
  all desired replicas updated/ready/available, and zero unavailable replicas;
- missing targets and mismatched state are failed verifiers with incidents;
- invalid JSON, unsupported source formats, and schema drift are blocked
  verifier results with durable evidence rows.

### Phase 21: Terraform And Cloud Artifacts — shipped

Source of truth:

- physical Terraform plan JSON generated by
  `terraform show -json <planfile> > tfplan.json`;
- physical Terraform state JSON generated by
  `terraform show -json <statefile> > tfstate.json`;
- normalized cloud inventory JSON with explicit provider/resource identity and
  attributes;
- normalized cloud audit JSONL with one event per line and explicit
  provider/principal/action/resource/outcome fields;
- SQLite verifier/evidence rows generated by `blackbox verify`.

Readback specs:

- `tf_plan_only_approved_changes_v1`;
- `tf_state_attribute_v1`;
- `cloud_resource_present_v1`;
- `cloud_audit_principal_action_v1`.

Connector properties:

- accepts Terraform JSON format major version `1` only and rejects unsupported
  major versions as schema drift;
- plan verification compares every non-`no-op` resource/output change against
  an explicit approval list and fails on unapproved or missing approved
  changes;
- state verification walks `values.root_module` and child modules, rejects
  sensitive attribute paths, and compares the exact JSON value read from
  physical state;
- cloud inventory verification requires an exact resource identity and exact
  required-attribute matches;
- cloud audit verification requires a matching principal/action/resource/
  outcome event in the physical JSONL export;
- missing target artifacts and mismatched state are failed verifiers with
  incidents; invalid JSON/JSONL, unsupported formats, duplicate identities,
  sensitive state paths, and schema drift are blocked typed errors.

### Phase 8: Enterprise Controls — shipped

Source of truth:

- SQLite `ledger_records` rows for `enterprise_tenant_config`,
  `enterprise_directory_snapshot`, and `enterprise_access_decision`;
- tenant audit export files: `audit.jsonl`, `siem-rfc5424.log`,
  `controls.json`, `manifest.json`.

Readback specs:

- `enterprise_tenant_config_v1`;
- `enterprise_scim_directory_v1`;
- `enterprise_rbac_tenant_isolation_v1`;
- `enterprise_audit_siem_export_v1`.

The connector is fail-closed: missing tenant config/directory state is a typed
error, malformed SSO/SCIM/RBAC/retention/deployment input writes no row, and
unauthorized audit export creates no files but still persists the denied
`enterprise_access_decision`.

### Phase 13: Privacy Retention And Erasure Decisions — shipped

Source of truth:

- SQLite `ledger_records` rows for `enterprise_tenant_config`,
  `enterprise_directory_snapshot`, `enterprise_access_decision`,
  `retention_decision`, and `evidence_deletion`;
- parent run rows and their immutable evidence/incident/scorecard rows.

Shipped connector:

- `blackbox_privacy_v1`

Readback specs:

- `blackbox_retention_decision_v1`;
- `blackbox_evidence_deletion_decision_v1`.

The connector is fail-closed: missing runs, missing tenant config/directory,
unknown retention classes, unauthorized principals, empty data subjects, and
secret-like request fields fail with typed errors. Authorized attempts first
persist an `enterprise_access_decision`; successful privacy decisions are then
read back from SQLite by sequence, decision id, run id, and hashes. Raw data
subjects are not persisted; only `sha256:<hex>` digests are stored.

### Phase 14: Release Gate Decisions — shipped

Source of truth:

- SQLite `ledger_records` rows for the parent run, `scorecard`,
  `verifier_result`, `evidence_packet`, `incident_packet`,
  `enterprise_access_decision`, `retention_decision`, and `evidence_deletion`.

Shipped connector:

- `blackbox_release_gate_v1`

Readback spec:

- `blackbox_release_gate_decision_v1`.

The connector is fail-closed: malformed thresholds and missing runs write no
gate row, unauthorized principals write only the denied RBAC decision, and
blocked gates write a durable `release_gate` row before exiting non-zero.
Gate decisions inspect physical score/risk/trust, evidence-ref integrity,
failed or blocked verifiers, incident packets, and privacy retention/erasure
state. An approved gate is possible only when every configured check passes.

### Phase 15: Pilot Package And Usage Meter Proof — shipped

Source of truth:

- SQLite `ledger_records` rows for tenant evidence, scorecards, release gates,
  privacy rows, enterprise config/directory/access rows, and `pilot_package`;
- package files: `pilot-report.json`, `usage-meter-events.jsonl`,
  `eval-candidates.jsonl`, and `manifest.json`.

Shipped connector:

- `blackbox_packaging_v1`

Readback specs:

- `blackbox_pilot_package_v1`;
- `blackbox_usage_meter_events_jsonl_v1`.

The connector is fail-closed. Invalid package ids, zero `--min-actions`,
empty tenant/principal/agent values, and unsafe output paths fail before
authorization or file creation. Unauthorized principals persist only the
denied `enterprise_access_decision` and create no package row/files.
Authorized package builds read real ledger rows, write package artifacts to a
temporary directory, parse every JSON/JSONL artifact back, verify manifest
hashes, append `pilot_package`, read the row back by id/hash/sequence, then
promote and re-verify final files. Unmet pilot success criteria create a
durable package row/files and return `PILOT_PACKAGE_CRITERIA_NOT_MET`.

### Phase 16: Risk Register And Open Question Closure — shipped

Source of truth:

- SQLite `ledger_records` rows for tenant evidence, incidents, scorecards,
  release gates, pilot packages, enterprise RBAC, and `risk_assessment`;
- SQLite runtime PRAGMAs: WAL journal mode, `synchronous=FULL`, and
  `busy_timeout >= 5000`;
- a real decision-register JSON file containing every required Phase 16
  open-question decision;
- dashboard/API readback files, including `api/risk-assessments.json`.

Shipped connector:

- `blackbox_risk_v1`

Readback specs:

- `blackbox_risk_assessment_v1`;
- `blackbox_open_question_decisions_v1`.

The connector is fail-closed. Empty or malformed tenant/principal/agent
fields, missing ledgers, unknown or duplicate decision ids, open decisions,
expired review dates, unknown severities, extra schema keys, and secret-like
decision text fail before a `risk_assessment` row is written. Unauthorized
principals persist only the denied `enterprise_access_decision`. Authorized
assessments read the physical ledger and runtime state, evaluate the required
risk checks, append an accepted or blocked `risk_assessment`, and read the row
back by id/hash/sequence. Blocked risk posture is durable evidence and exits
non-zero with `RISK_ASSESSMENT_BLOCKED`.

### Phase 10: Package And Vulnerability Scanner Artifacts — shipped

Source of truth:

- OSV-Scanner JSON report files (`source.report`,
  `source.report_format="osv-json"`);
- Cargo lockfiles (`source.lockfile`, `source.lockfile_format="cargo"`);
- CycloneDX JSON SBOM files (`source.sbom`,
  `source.sbom_format="cyclonedx-json"`).

Shipped connector:

- `scanner_artifact_v1`

Shipped verifiers:

- `scanner_vulnerability_absent_v1`
- `scanner_lockfile_pinned_to_version_v1`
- `scanner_sbom_artifact_present_v1`

The connector reads artifacts from disk and stores file existence, byte count,
SHA-256, parse/schema status, and normalized observed state in
`readback_snapshot`. It does not shell out to scanner binaries and does not
trust scanner exit codes as the source of truth. Missing files and readback
mismatches are failed verifier results; malformed JSON, unsupported formats,
and schema drift are blocked typed errors. Invalid specs that cannot identify
a physical source are blocked before a readback row is written.

### Later Phases (deferred)

- Ticketing: `jira_ticket_state_v1`, `linear_ticket_state_v1`,
  `servicenow_ticket_state_v1`.

## Connector Lifecycle

Every new connector lands as a single PR that includes:

1. Connector spec doc (`docs/blackboxfix/connectors/<id>.md`).
2. Read-only readback module (Rust) with typed errors.
3. Mutating capability module (if any).
4. Verifier specs in `crates/context-graph-cli/src/blackbox/verifiers/`.
5. Happy-path FSV test against a real local fixture.
6. Three edge-case FSV tests (missing target, wrong target, permission/schema
   failure — see `06_fsv_verification_engine.md`).
7. Redaction tests with synthetic secret-bearing inputs.
8. SIEM/audit export mapping (Phase 8).

A connector PR that lacks any of items 1–7 must not merge.

## First Connector Set (Phase 2 + 3 + 10 + 11 demo)

For the first product demo (`18_first_wedge_demo.md`):

- local Git;
- GitHub PRs / checks / issues;
- local test command verifier;
- OSV JSON report, Cargo lockfile, and CycloneDX SBOM artifacts;
- GitHub issue evidence-link verification for ticket proof;
- JSONL evidence exporter (Phase 5).

This produces a credible DevSecOps agent governance workflow with inspectable
state and low connector sprawl.

## FSV Of This Document

For shipped local connectors, the operator must run:

```bash
context-graph blackbox connector list
context-graph blackbox connector test --connector <id> --source <fixture>
context-graph blackbox verify --ledger <ledger> --input <spec.json>
python3 - <<'PY'
import sqlite3
conn = sqlite3.connect("<ledger>")
print(conn.execute("SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY 1").fetchall())
PY
```

For GitHub/GHA shipped in Phase 3, `<fixture>` is `OWNER/REPO` and the test
suite uses `actions/checkout` unless `BBX_GITHUB_TEST_REPO` overrides it.
Phase 11 uses the same `github_v1` fixture and independently discovers a real
issue whose body contains an `http(s)` URL before verifying
`github_issue_evidence_link_v1`. For GitLab CI shipped in Phase 9,
`<fixture>` is `GROUP/PROJECT` and the test suite uses
`gitlab-org/gitlab-runner` unless `BBX_GITLAB_TEST_PROJECT` overrides it. For
scanner artifacts shipped in Phase 10, `<fixture>` is an OSV JSON report path,
and verifier specs point at real OSV JSON, `Cargo.lock`, and CycloneDX JSON
files. For scoring shipped in Phase 12, `<fixture>` is the SQLite ledger path;
`blackbox_scoring_v1` reads `ledger_records`, verifies hash chains and evidence
references, appends a `scorecard`, and reads the appended row back. For privacy
shipped in Phase 13, `<fixture>` is also the SQLite ledger path;
`blackbox_privacy_v1` reads the enterprise config/directory, appends an
authorization decision, appends `retention_decision` or `evidence_deletion`,
and reads the appended privacy row back. Confirm the row counts and data
subject digest match the test plan. For release gates shipped in Phase 14,
`blackbox_release_gate_v1` reads and counts physical `release_gate` rows. For
pilot packaging shipped in Phase 15, `blackbox_packaging_v1` reads and counts
physical `pilot_package` rows and package FSV must also read package JSON/JSONL
files and manifest hashes directly. For risk assessment shipped in Phase 16,
`blackbox_risk_v1` reads and counts physical `risk_assessment` rows, while
manual FSV must also read the decision-register JSON and dashboard
`api/risk-assessments.json` directly. Anything else is hearsay.
