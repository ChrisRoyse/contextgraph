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
`gitlab_ci_v1` for hosted GitLab pipeline and job-state readback.

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

The connector uses GitHub REST API `GET` endpoints only. It sends
`Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`,
and a stable user agent. `BBX_GITHUB_TOKEN` or `GITHUB_TOKEN` is optional for
public resources and required by operators for private resources.

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

### Phase 3: Package And Vulnerability Scanners

Initial targets: OSV-Scanner, `cargo audit`, `npm audit`, Trivy. Verifiers:

- `scanner_vulnerability_absent_v1`
- `scanner_lockfile_pinned_to_version_v1`
- `scanner_sbom_artifact_present_v1`

### Later Phases (deferred)

- Kubernetes: `k8s_deployment_image_digest_v1`,
  `k8s_rollout_completed_v1`, `k8s_replicas_match_v1`.
- Terraform/Cloud: `tf_plan_only_approved_changes_v1`,
  `tf_state_attribute_v1`, `cloud_resource_present_v1`,
  `cloud_audit_principal_action_v1`.
- Ticketing: `jira_ticket_state_v1`, `linear_ticket_state_v1`,
  `servicenow_ticket_state_v1`, `github_issue_evidence_link_v1`.

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

## First Connector Set (Phase 2 + 3 demo)

For the first product demo (`18_first_wedge_demo.md`):

- local Git;
- GitHub PRs / checks / issues;
- local test command verifier;
- OSV-Scanner / cargo audit;
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
suite uses `actions/checkout` unless `BBX_GITHUB_TEST_REPO` overrides it. For
GitLab CI shipped in Phase 9, `<fixture>` is `GROUP/PROJECT` and the test
suite uses `gitlab-org/gitlab-runner` unless `BBX_GITLAB_TEST_PROJECT`
overrides it. Confirm the row counts match the test plan. Anything else is
hearsay.
