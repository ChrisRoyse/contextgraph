# Source-Of-Truth Connectors And Verifiers

BlackBoxFix can verify claims by reading physical systems or captured physical
artifacts. Connector readback is separate from the agent/tool return value.

## Local Connectors

### SQLite Ledger

Connector id:

- `sqlite_local_v1`

Capabilities:

- initialize ledger;
- inspect rows;
- verify schema;
- verify metadata;
- verify append-only triggers;
- verify hash chains.

### Git

Connector id:

- `git_local_v1`

Verifiers:

- `git_branch_exists_v1`
- `git_commit_exists_v1`
- `git_file_diff_matches_patch_v1`
- `git_working_tree_clean_v1`

Capabilities:

- prove branch existence;
- prove commit existence;
- compare file diffs to expected patch;
- prove working tree cleanliness;
- serve as physical proof for the MCP verified patch workflow.

### Filesystem

Connector id:

- `fs_local_v1`

Capabilities:

- read physical files;
- report existence;
- report byte size;
- compute SHA-256;
- read UTF-8 content when decodable.

### Command

Connector id:

- `cmd_local_v1`

Verifier:

- `cmd_exit_zero_v1`

Capabilities:

- execute declared command argv;
- capture exit code/stdout/stderr;
- inspect declared artifact files;
- produce passed or failed verifier evidence.

## Hosted Service Connectors

### GitHub

Connector id:

- `github_v1`

Verifiers:

- `github_pr_state_v1`
- `github_branch_head_matches_commit_v1`
- `github_check_suite_passed_v1`
- `github_issue_state_changed_v1`
- `github_issue_evidence_link_v1`

Capabilities:

- read repository state;
- read pull requests;
- read branches and commits;
- read check suites;
- read issues and issue comments;
- verify evidence links in issue bodies/comments;
- reject pull requests returned through the Issues endpoint as ticket proof.

### GitHub Actions

Connector id:

- `gha_v1`

Verifier:

- `gha_workflow_run_completed_v1`

Capabilities:

- read workflow run state;
- verify completed workflow runs.

### GitLab CI

Connector id:

- `gitlab_ci_v1`

Verifiers:

- `gitlab_pipeline_status_v1`
- `gitlab_pipeline_jobs_completed_v1`

Capabilities:

- read project metadata;
- read pipeline status;
- read job completion state;
- fail closed on schema drift, missing pipeline, failed jobs, and invalid
  expected state.

## Security Scanner Artifacts

Connector id:

- `scanner_artifact_v1`

Verifiers:

- `scanner_vulnerability_absent_v1`
- `scanner_lockfile_pinned_to_version_v1`
- `scanner_sbom_artifact_present_v1`

Capabilities:

- read OSV JSON reports;
- read Cargo lockfiles;
- read CycloneDX JSON SBOMs;
- verify vulnerability absence;
- verify lockfile package version pins;
- verify SBOM package presence;
- treat malformed scanner artifacts as blocked typed errors.

## Kubernetes Artifacts

Connector id:

- `k8s_deployment_snapshot_v1`

Verifiers:

- `k8s_deployment_image_digest_v1`
- `k8s_replicas_match_v1`
- `k8s_rollout_completed_v1`

Capabilities:

- read physical `kubectl get deployments.v1.apps -o json` snapshots;
- verify image digests;
- verify replica counts;
- verify rollout completion;
- produce incidents for missing deployments and mismatches.

## Terraform And Cloud Artifacts

Connector id:

- `terraform_cloud_artifact_v1`

Verifiers:

- `tf_plan_only_approved_changes_v1`
- `tf_state_attribute_v1`
- `cloud_resource_present_v1`
- `cloud_audit_principal_action_v1`

Capabilities:

- read physical Terraform plan JSON;
- read physical Terraform state JSON;
- read normalized cloud inventory JSON;
- read normalized cloud audit JSONL;
- verify allowed plan changes;
- verify state attributes;
- verify resource presence;
- verify cloud audit principal/action evidence.

## Ticketing Artifacts

Connector id:

- `ticketing_artifact_v1`

Verifiers:

- `jira_ticket_state_v1`
- `linear_ticket_state_v1`
- `servicenow_ticket_state_v1`

Capabilities:

- read captured Jira REST issue/search JSON;
- read captured Linear GraphQL issue JSON;
- read captured ServiceNow Table API record/list JSON;
- verify ticket identity;
- verify ticket state/status;
- verify evidence link when required;
- fail closed on unsupported provider formats and schema drift.

## OpenTelemetry Export Connector

Connector id:

- `blackbox_otel_export_v1`

Capabilities:

- read completed OpenTelemetry export directories;
- verify `traces.jsonl`, `logs.jsonl`, `metrics.jsonl`;
- verify `manifest.json`;
- recompute file hashes and item counts;
- reject mixed-signal JSONL and manifest drift.

## SDK Ingestion Connector

Connector id:

- `blackbox_sdk_ingest_v1`

Capabilities:

- read physical CloudEvents JSON object or batch files;
- validate supported SDK event types;
- enforce CloudEvents `source` + `id` idempotency;
- validate W3C `traceparent`;
- enforce expected delta for mutating SDK tool calls;
- map SDK events to ledger rows;
- read back the inserted SQLite rows before returning success.
