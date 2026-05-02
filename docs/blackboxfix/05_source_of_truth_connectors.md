# Source-Of-Truth Connectors

Connectors define how BlackBoxFix reads and verifies real systems. A connector
is not just a tool wrapper. It must provide **independent readback** and
explicit evidence extraction.

Phase 1 ships exactly one connector: `sqlite_local_v1` (the ledger itself; no
external system). Phase 2 adds Git + filesystem; Phase 3 adds GitHub + CI.

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

### Phase 2: Git Local (filesystem only, no network)

Source of truth:

- `.git` object database (read via `git2` or shelling out to `git`);
- working tree status;
- commit graph;
- diff output;
- tags and refs.

Verifier examples (`verifier_id`):

- `git_branch_exists_v1`
- `git_commit_exists_v1`
- `git_file_diff_matches_patch_v1`
- `git_working_tree_clean_v1`
- `git_changed_paths_subset_v1`

### Phase 2: Local Test Command

Source of truth: the exit code, stdout/stderr, and any artifact files of a
declared test command. Verifier examples:

- `cmd_exit_zero_v1`
- `cmd_stdout_contains_v1`
- `cmd_artifact_hash_matches_v1`

### Phase 3: GitHub

Source of truth:

- repository metadata; pull requests; branches; commits; checks; reviews;
  issues; deployments.

Verifier examples:

- `github_pr_open_v1`
- `github_branch_head_matches_commit_v1`
- `github_check_suite_passed_v1`
- `github_issue_state_changed_v1`
- `github_review_approved_v1`

### Phase 3: CI Providers

Initial target: GitHub Actions. Add later: Buildkite, CircleCI, GitLab CI.

Verifier examples:

- `gha_workflow_run_completed_v1`
- `gha_required_jobs_passed_v1`
- `gha_artifact_present_v1`
- `gha_test_report_no_failures_v1`

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

Until Phase 2 lands, the only physically present connector is the ledger
itself. After every connector is added, the operator must run:

```bash
context-graph blackbox connector list
context-graph blackbox connector test --connector <id> --source <fixture>
sqlite3 <ledger> 'SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY 1;'
```

…and confirm the row counts match the test plan. Anything else is hearsay.
