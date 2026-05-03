# API, CLI, MCP, And SDK Surface

BlackBoxFix exposes the same core semantics through CLI, API, SDK, and MCP
wrappers. The product surface makes the FSV lifecycle explicit.

Phase 17 ships the local CLI plus JSON-RPC stdio MCP recorder/gateway, policy
decision rows, a capability registry, a machine-readable HTTP API surface
registry, a static dashboard/API bundle, and enterprise-control commands for
SSO metadata, SCIM directory snapshots, RBAC/tenant-isolation decisions, and
audit/SIEM export. It also ships the GitLab CI hosted connector and the local
scanner artifact connector, GitHub issue evidence-link verification, and
deterministic run scorecards. Phase 13 adds authorized privacy retention and
erasure-request decision rows. Phase 14 adds an authorized release/promotion
gate that persists approved and blocked gate decisions. Phase 15 adds
authorized pilot packaging, usage-meter JSONL, eval-candidate JSONL, package
manifests, and dashboard package readback. Phase 16 adds authorized
risk/open-question assessment, `risk_assessment` ledger rows, and dashboard
risk readback. Phase 17 adds an authenticated loopback HTTP server, a
single-writer ledger path, tenant-scoped dashboard reads, and durable
`http_request_audit` rows for every protected HTTP request. Phase 18 adds the
first executable buyer-demo package command, `first_wedge_demo` ledger rows,
and manifest-verified demo artifacts. Phase 19 adds executable FSV
certification, `full_state_verification` ledger rows, a provenance-shaped FSV
package, and `blackbox_fsv_protocol_v1` connector readback. Phase 20 adds
Kubernetes Deployment snapshot verification through
`k8s_deployment_snapshot_v1`. Phase 21 adds Terraform plan/state, cloud
inventory, and cloud audit artifact verification through
`terraform_cloud_artifact_v1`. Phase 22 adds Jira, Linear, and ServiceNow
ticketing artifact verification through `ticketing_artifact_v1`. Phase 23
adds OpenTelemetry OTLP JSONL export through `blackbox_otel_export_v1`.
SDKs remain reserved for later phases.

## CLI (shipped Phase 23)

Two binaries, identical semantics for the BlackBoxFix subset:

| Binary | Path | Invocation |
|---|---|---|
| Standalone | `crates/context-graph-cli/src/bin/blackbox.rs` | `blackbox <subcommand>` |
| Umbrella | `crates/context-graph-cli/src/bin/context-graph.rs` | `context-graph blackbox <subcommand>` |

Build via `cargo build --release -p context-graph-cli`. Both binaries land in
`target/release/`.

### Commands (verified by FSV tests)

```text
blackbox init                  --ledger PATH
blackbox connector list
blackbox connector test        --connector git_local_v1 --source REPO_PATH
blackbox connector test        --connector fs_local_v1 --source FILE_PATH
blackbox connector test        --connector cmd_local_v1 --source CWD
                               --command ARGV...
blackbox connector test        --connector github_v1 --source OWNER/REPO
blackbox connector test        --connector gha_v1 --source OWNER/REPO
blackbox connector test        --connector gitlab_ci_v1 --source GROUP/PROJECT
blackbox connector test        --connector scanner_artifact_v1 --source OSV_REPORT_JSON
blackbox connector test        --connector k8s_deployment_snapshot_v1
                               --source DEPLOYMENT_SNAPSHOT_JSON
blackbox connector test        --connector terraform_cloud_artifact_v1
                               --source TERRAFORM_PLAN_JSON
blackbox connector test        --connector ticketing_artifact_v1
                               --source TICKETING_API_JSON
blackbox connector test        --connector blackbox_otel_export_v1
                               --source OTEL_EXPORT_DIR
blackbox connector test        --connector enterprise_controls_v1 --source LEDGER_PATH
blackbox connector test        --connector blackbox_scoring_v1 --source LEDGER_PATH
blackbox connector test        --connector blackbox_privacy_v1 --source LEDGER_PATH
blackbox connector test        --connector blackbox_release_gate_v1 --source LEDGER_PATH
blackbox connector test        --connector blackbox_packaging_v1 --source LEDGER_PATH
blackbox connector test        --connector blackbox_http_server_v1 --source LEDGER_PATH
blackbox connector test        --connector blackbox_first_wedge_demo_v1 --source LEDGER_PATH
blackbox connector test        --connector blackbox_fsv_protocol_v1 --source LEDGER_PATH
blackbox record run            --ledger PATH --task TASK --agent AGENT
                               [--tenant TENANT]    (default: tenant_local)
                               [--model MODEL]      (default: null)
                               [--task-kind KIND]   (default: local_agent_task)
                               [--risk-tier TIER]   (default: medium)
blackbox record step           --ledger PATH --run-id RUN_ID --summary TEXT
                               [--step-kind KIND] [--tenant TENANT]
                               [--agent AGENT] [--model MODEL]
blackbox record model-decision --ledger PATH --run-id RUN_ID --tool-name TOOL
                               --capability CAP --input PATH
                               [--confidence FLOAT] [--tenant TENANT]
                               [--agent AGENT] [--model MODEL]
blackbox record tool-call      --ledger PATH --run-id RUN_ID --tool TOOL
                               --input PATH         (path to JSON file)
                               [--tenant TENANT]    (default: tenant_local)
                               [--agent AGENT]      (default: agent_unknown)
                               [--model MODEL]
                               [--capability CAP]   (default: local.tool)
                               [--status STATUS]    (ok|failed|blocked, default: ok)
                               [--external-request-id ID]
blackbox verify                --ledger PATH --input SPEC_JSON
                               [--tenant TENANT]    (default: tenant_local)
                               [--agent AGENT]      (default: blackbox_verifier)
                               [--model MODEL]
blackbox inspect ledger        --ledger PATH
                               [--run-id RUN_ID]
                               [--limit N]
blackbox replay                --ledger PATH --run-id RUN_ID
blackbox export evidence       --ledger PATH --run-id RUN_ID --out FILE
blackbox export episodes       --ledger PATH --out DIR [--force]
blackbox export otel           --ledger PATH --out DIR [--force]
blackbox dashboard build       --ledger PATH --out DIR [--force]
blackbox dashboard api         --ledger PATH --view VIEW [--run-id RUN_ID]
blackbox enterprise configure  --ledger PATH --tenant TENANT --input CONFIG
                               --agent AGENT [--model MODEL]
blackbox enterprise sync-directory
                               --ledger PATH --tenant TENANT --input SCIM_JSON
                               --agent AGENT [--model MODEL]
blackbox enterprise access-evaluate
                               --ledger PATH --tenant TENANT
                               --principal-id PRINCIPAL --permission PERM
                               --resource tenant:TENANT:RESOURCE
                               --agent AGENT [--model MODEL]
blackbox enterprise export-audit
                               --ledger PATH --tenant TENANT
                               --principal-id PRINCIPAL --out DIR [--force]
                               --agent AGENT [--model MODEL]
blackbox score run             --ledger PATH --run-id RUN_ID
                               [--tenant TENANT] [--agent AGENT]
                               [--model MODEL]
blackbox privacy evaluate-retention
                               --ledger PATH --tenant TENANT --run-id RUN_ID
                               --principal-id PRINCIPAL
                               --retention-class CLASS
                               --data-subject SUBJECT --purpose PURPOSE
                               --agent AGENT [--model MODEL]
blackbox privacy request-erasure
                               --ledger PATH --tenant TENANT --run-id RUN_ID
                               --principal-id PRINCIPAL
                               --retention-class CLASS
                               --data-subject SUBJECT --reason REASON
                               --agent AGENT [--model MODEL]
blackbox gate evaluate         --ledger PATH --tenant TENANT --run-id RUN_ID
                               --principal-id PRINCIPAL
                               --environment ENV
                               [--min-trust-level TRUST]
                               [--max-risk FLOAT]
                               [--require-policy]
                               [--require-privacy]
                               --agent AGENT [--model MODEL]
blackbox package pilot-report  --ledger PATH --tenant TENANT
                               --principal-id PRINCIPAL --pilot-id PILOT
                               --min-actions N --out DIR [--force]
                               --agent AGENT [--model MODEL]
blackbox risk evaluate         --ledger PATH --tenant TENANT
                               --principal-id PRINCIPAL
                               --decisions DECISIONS_JSON
                               --agent AGENT [--model MODEL]
blackbox server serve          --ledger PATH
                               [--bind 127.0.0.1:7607]
                               [--api-key-env BLACKBOX_API_KEY]
                               [--allow-remote-bind --tls-terminated-by NAME]
blackbox demo first-wedge      --ledger PATH --tenant TENANT --run-id RUN_ID
                               --out DIR [--force]
                               --agent AGENT [--model MODEL]
blackbox fsv certify           --ledger PATH --tenant TENANT --phase N
                               --evidence FSV_EVIDENCE_JSON
                               --out DIR [--force]
                               [--run-id RUN_ID]
                               --agent AGENT [--model MODEL]
blackbox policy evaluate       --ledger PATH --mode MODE --tool TOOL
                               --capability CAP [--run-id RUN_ID]
                               [--mutating] [--verifier-id VERIFIER]
                               [--source-scope SCOPE]
blackbox policy registry       [--ledger PATH]
blackbox api surface           [--ledger PATH]
blackbox mcp recorder          --ledger PATH
blackbox mcp gateway           --ledger PATH --policy-mode MODE
```

The `--ledger` flag also accepts the env var `BLACKBOX_LEDGER`.

`-v`/`-vv`/`-vvv` increase log verbosity (warn/info/debug/trace, written to
stderr). Stdout always carries the JSON result on success.

### Verifier Spec Input (Phase 2/3/9/10/11/20/21/22)

`blackbox verify --input` takes a JSON object:

```json
{
  "verifier_id": "git_branch_exists_v1",
  "run_id": "run_...",
  "tool_call_id": "tool_...",
  "source": { "repo": "/tmp/repo" },
  "expected": { "branch": "main" }
}
```

Supported Phase 2/3/9/10/11/20/21/22 verifier ids:

- `git_branch_exists_v1`
- `git_commit_exists_v1`
- `git_file_diff_matches_patch_v1`
- `git_working_tree_clean_v1`
- `cmd_exit_zero_v1`
- `github_pr_state_v1`
- `github_branch_head_matches_commit_v1`
- `github_check_suite_passed_v1`
- `github_issue_state_changed_v1`
- `github_issue_evidence_link_v1`
- `gha_workflow_run_completed_v1`
- `gitlab_pipeline_status_v1`
- `gitlab_pipeline_jobs_completed_v1`
- `scanner_vulnerability_absent_v1`
- `scanner_lockfile_pinned_to_version_v1`
- `scanner_sbom_artifact_present_v1`
- `k8s_deployment_image_digest_v1`
- `k8s_replicas_match_v1`
- `k8s_rollout_completed_v1`
- `tf_plan_only_approved_changes_v1`
- `tf_state_attribute_v1`
- `cloud_resource_present_v1`
- `cloud_audit_principal_action_v1`
- `jira_ticket_state_v1`
- `linear_ticket_state_v1`
- `servicenow_ticket_state_v1`

Every verifier writes `expected_delta`, then a `readback_snapshot` when the
connector was able to read physical state, then `verifier_result`, then an
`evidence_packet`. A failed verifier also writes an `incident_packet`. A
failed or blocked verifier returns non-zero **after** persisting the verifier
evidence, except for invalid top-level JSON that cannot be associated with a
run/tool-call.

## CLI Failure Shape (enforced)

Every failure prints a JSON object on **stderr** and exits non-zero:

```json
{
  "status": "error",
  "error_code": "BBX_HASH_CHAIN_BROKEN",
  "message": "...",
  "source_of_truth": "sqlite:/tmp/bbx.sqlite#ledger_records",
  "connector": "sqlite_local_v1",
  "field_path": "ledger_records[2].payload_sha256",
  "remediation": "treat the ledger as tampered or corrupt; restore from trusted backup or start a new ledger"
}
```

Exit codes:

- `0` — success.
- `2` — `BBX_HASH_CHAIN_BROKEN` (treat as integrity emergency).
- `1` — every other typed error.

The full error-code list is in `20_phase1_state_and_invariants.md`.

## CLI Success Shape

Every success prints a JSON object on stdout:

```json
{
  "status": "ok",
  "command": "blackbox init",
  "ledger_path": "/tmp/bbx.sqlite",
  "source_of_truth": "sqlite:/tmp/bbx.sqlite#ledger_records",
  "schema_version": 13,
  "redaction_policy_id": "redact_v1",
  "record_count": 0,
  "ledger_tip_sha256": ""
}
```

`record run`, `record tool-call`, and `verify` additionally return the new
`record_id`, `run_id` (and `tool_call_id` for tool calls), `ledger_sequence`,
`payload_sha256`, `previous_record_sha256`, `run_chain_sha256`, and
`record_sha256`.

`verify` also returns `evidence_packet_id`, `evidence_packet_sequence`,
`evidence_packet_record_sha256`, and the incident packet sequence/hash when
one was created.

`inspect ledger` returns `integrity` plus a `records` array with each row's
fields and hashes. `replay` returns a read-only forensic report. `export
evidence` returns the exported file path, byte count, sha256, and evidence/
incident counts after reading the file back from disk. `export episodes`
returns the output directory, `episodes.jsonl`, `manifest.json`, `evals/`,
row counts, split counts, JSONL sha256, ledger sequence range, and ledger tip
after reading all files back from disk.

`dashboard build` returns the dashboard directory, `index.html`,
`manifest.json`, `api/`, run/evidence/incident counts, manifest sha256, and
ledger tip after reading every generated file back from disk. `dashboard api`
returns one dashboard read model (`summary`, `runs`, `timeline`, `evidence`,
`verifiers`, `errors`, `exports`, `pilot-packages`, or `risk-assessments`)
after verifying the source ledger.

`enterprise configure` writes `enterprise_tenant_config` after validating
OIDC/SAML metadata, RBAC roles, retention classes, private deployment
metadata, SIEM format, and customer-managed tenant key id. `enterprise
sync-directory` writes `enterprise_directory_snapshot` after checking SCIM
User/Group schema markers, user/group references, and role assignments.
`enterprise access-evaluate` writes `enterprise_access_decision`; allowed and
denied decisions are both durable rows. `enterprise export-audit` also writes
and reads back an `enterprise_access_decision` for the export authorization
attempt. If the principal lacks `audit.export`, the command creates no files
and returns `ENTERPRISE_RBAC_DENIED`; if authorized, it writes `audit.jsonl`,
`siem-rfc5424.log`, `controls.json`, and `manifest.json`; the exporter reads
every file back and validates hashes before reporting success.

`score run` writes one `scorecard` row after reading the physical
`ledger_records` source of truth, verifying the ledger hash chains, verifying
evidence packet references, computing deterministic score dimensions, and then
reading the appended `scorecard` row back by sequence, `scorecard_id`,
`run_id`, `payload_sha256`, and `record_sha256`.

`privacy evaluate-retention` writes one `retention_decision` row after reading
the physical ledger, verifying the parent run, persisting and checking an
authorized `enterprise_access_decision` for `privacy.review`, resolving the
configured tenant retention class, hashing the data subject, and reading the
appended row back by sequence, `privacy_decision_id`, `run_id`,
`payload_sha256`, and `record_sha256`. `privacy request-erasure` uses the same
flow with `privacy.erase` and writes `evidence_deletion`; legal hold or
incident evidence produces a durable denied decision instead of deleting
append-only ledger rows.

`gate evaluate` writes one `release_gate` row after reading the physical
ledger, verifying the parent run, persisting and checking an
`enterprise_access_decision` for `release.approve`, resolving the latest
scorecard, checking score trust/risk thresholds, checking policy and privacy
requirements when requested, verifying evidence packet references, and
blocking any failed/blocked verifier or incident state. Approved and blocked
gate decisions are both durable rows and are read back by sequence,
`release_gate_id`, `run_id`, `payload_sha256`, and `record_sha256`.
Unauthorized principals fail as `RELEASE_GATE_RBAC_DENIED` after the denied
authorization row is persisted and create no `release_gate` row. Invalid
request fields fail before new rows are written.

`package pilot-report` writes one pilot package directory and one
`pilot_package` row after reading the physical ledger, persisting and checking
a `commercial.package` authorization decision, computing pilot success
metrics, writing `pilot-report.json`, `usage-meter-events.jsonl`,
`eval-candidates.jsonl`, and `manifest.json`, parsing every file back, and
reading the appended package row back by sequence, `pilot_package_id`,
`payload_sha256`, and `record_sha256`. A package that lacks enough evidence is
persisted and exits non-zero with `PILOT_PACKAGE_CRITERIA_NOT_MET`; invalid
requests and unauthorized principals create no package row/files.

`risk evaluate` writes one `risk_assessment` row after validating a real
decision-register JSON file, reading the physical ledger and SQLite runtime
PRAGMAs, persisting and checking a `risk.review` authorization decision,
evaluating ledger integrity, runtime durability, secret-scan, enterprise
controls, source-of-truth coverage, verifier/policy status, release gates,
pilot package success, incident-to-eval conversion, and required
open-question closure, then reading the appended row back by sequence,
`risk_assessment_id`, `payload_sha256`, and `record_sha256`. Blocked risk
posture is persisted and exits non-zero with `RISK_ASSESSMENT_BLOCKED`;
invalid decision registers and unauthorized principals create no
`risk_assessment` row.

`connector test --connector gitlab_ci_v1` reads GitLab project metadata and
the latest pipeline from GitLab REST API v4. `gitlab_pipeline_status_v1`
passes only when the expected pipeline id exists and the status, ref, and SHA
all match. `gitlab_pipeline_jobs_completed_v1` passes only when the pipeline
has a non-zero expected minimum job count, every GitLab jobs page has been
read through pagination, and every job status is explicitly allowed, with
optional explicit handling for `allow_failure` failed jobs.

`connector test --connector scanner_artifact_v1` reads an OSV JSON report
from disk and returns the physical artifact path, byte count, SHA-256,
package count, vulnerability count, alias/group ids, and normalized packages.
`scanner_vulnerability_absent_v1` passes only when the requested
vulnerability id is absent from the OSV report after optional
package/ecosystem/version filters. `scanner_lockfile_pinned_to_version_v1`
passes only when a real Cargo lockfile contains the requested package and all
matching package entries have the expected version.
`scanner_sbom_artifact_present_v1` passes only when a real CycloneDX JSON
SBOM has `bomFormat="CycloneDX"`, a `specVersion`, enough components, and the
expected component/version when provided.

`github_issue_evidence_link_v1` passes only when the target GitHub issue
exists, is not a pull request, and the exact expected absolute `http(s)`
evidence URL appears in the requested `link_scope`: `body`, `comments`, or
`body_or_comments`. The readback stores issue metadata, issue body SHA/length,
comment page summaries, pagination headers, and matched locations.

`connector test --connector blackbox_release_gate_v1` opens the SQLite ledger,
verifies the hash chains, and returns the total persisted `release_gate` rows
read from `ledger_records`.

`connector test --connector blackbox_packaging_v1` opens the SQLite ledger,
verifies the hash chains, and returns the total persisted `pilot_package` rows
read from `ledger_records`.

`connector test --connector blackbox_risk_v1` opens the SQLite ledger,
verifies the hash chains, and returns the total persisted `risk_assessment`
rows read from `ledger_records`.

`connector test --connector blackbox_http_server_v1` opens the SQLite ledger,
verifies the hash chains, and returns the total persisted
`http_request_audit` rows read from `ledger_records`.

`connector test --connector blackbox_first_wedge_demo_v1` opens the SQLite
ledger, verifies the hash chains, and returns the total persisted
`first_wedge_demo` rows read from `ledger_records`.

## HTTP API Surface (Phase 17 shipped server)

`blackbox api surface [--ledger PATH]` returns the HTTP endpoint registry below
as JSON, after verifying the ledger when `--ledger` is supplied. Phase 17 also
ships `blackbox server serve`, an authenticated JSON server over one local
ledger. The server refuses non-loopback binds unless the operator explicitly
passes `--allow-remote-bind --tls-terminated-by NAME`; API keys are read only
from an environment variable and are compared by hash in memory.

Protected routes require:

```text
Authorization: Bearer <BLACKBOX_API_KEY>
X-Blackbox-Tenant: tenant_...
Content-Type: application/json   # for POST bodies
```

Phase 17 served endpoints:

```text
GET  /v1/health
POST /v1/runs
GET  /v1/runs/{run_id}
POST /v1/runs/{run_id}/steps
POST /v1/tool-calls
GET  /v1/dashboard/summary
GET  /v1/dashboard/runs
GET  /v1/dashboard/runs/{run_id}/timeline
GET  /v1/dashboard/evidence
GET  /v1/dashboard/verifier-status
GET  /v1/dashboard/errors
GET  /v1/dashboard/exports
GET  /v1/dashboard/pilot-packages
GET  /v1/dashboard/risk-assessments
```

Every protected request writes a `http_request_audit` row after acquiring the
single-writer ledger lock. Authentication failures, tenant mismatches,
schema errors, and 64 KiB body-limit failures are also audited. The source of
truth is `sqlite:PATH#ledger_records`; return values are not considered proof
without reading that table.

Full registry endpoints:

```text
POST /v1/runs
GET  /v1/runs/{run_id}
POST /v1/runs/{run_id}/steps
POST /v1/tool-calls
POST /v1/tool-calls/{tool_call_id}/verify
GET  /v1/tool-calls/{tool_call_id}/evidence
POST /v1/replay
POST /v1/exports/episodes
GET  /v1/connectors
POST /v1/connectors/{connector_id}/test
GET  /v1/policies
POST /v1/policies/evaluate
POST /v1/enterprise/tenants/{tenant_id}/config
POST /v1/enterprise/tenants/{tenant_id}/directory-sync
POST /v1/enterprise/tenants/{tenant_id}/access/evaluate
POST /v1/enterprise/tenants/{tenant_id}/audit-export
POST /v1/runs/{run_id}/scorecard
POST /v1/privacy/tenants/{tenant_id}/runs/{run_id}/retention/evaluate
POST /v1/privacy/tenants/{tenant_id}/runs/{run_id}/erasure-requests
POST /v1/gates/tenants/{tenant_id}/runs/{run_id}/evaluate
POST /v1/commercial/tenants/{tenant_id}/pilot-packages
POST /v1/risk/tenants/{tenant_id}/assessments
GET  /v1/dashboard/summary
GET  /v1/dashboard/runs
GET  /v1/dashboard/runs/{run_id}/timeline
GET  /v1/dashboard/evidence
GET  /v1/dashboard/verifier-status
GET  /v1/dashboard/errors
GET  /v1/dashboard/exports
GET  /v1/dashboard/pilot-packages
GET  /v1/dashboard/risk-assessments
```

Write endpoints return the created ledger ids. They must not claim the action
is correct unless verifier evidence exists.

## MCP Surface (Phase 6 shipped)

Two server modes:

### Recorder MCP Server

Tools (no execution side-effects on real systems):

```text
blackbox_start_run
blackbox_record_decision
blackbox_record_tool_call
blackbox_verify_tool_call
blackbox_get_evidence
blackbox_export_episodes
blackbox_policy_evaluate
```

### Gateway MCP Server

Tools that wrap external systems and **always** produce verifier evidence for
the shipped first workflow:

```text
verified_git_apply_patch
```

`verified_git_apply_patch` requires a real Git repo path, target path, and
patch. It records `agent_step`, `model_decision`, `policy_decision`, and
`tool_call`, applies the patch through Git, then runs
`git_file_diff_matches_patch_v1`. The MCP tool result includes source-of-truth
ids, verifier result, and evidence packet data. Governed modes block
unverified mutating calls before external mutation.

## SDK (reserved after Phase 6)

Start with Python and TypeScript:

```python
with blackbox.run(task="fix vulnerability", agent_id="agent_coder") as run:
    call = run.tool_call("git_apply_patch", input=patch)
    result = call.execute()
    evidence = call.verify(expected_delta=expected)
```

SDK constraints:

- all mutating calls require expected delta;
- all verifiers return typed result;
- recorder works without model-provider lock-in;
- redaction hooks run before persistence;
- source-of-truth readback is explicit.

## OpenTelemetry Export (Phase 23 shipped)

OpenTelemetry is an export path, not the primary source of truth. Phase 23
ships:

- `blackbox export otel --ledger PATH --out DIR [--force]`;
- `blackbox_otel_export_v1` connector readback over completed export
  directories;
- `traces.jsonl` with `resourceSpans` and one span per physical ledger row;
- `logs.jsonl` with `resourceLogs` for incidents, failed/blocked verifiers,
  denied policies, non-ok HTTP audits, and export summary logs;
- `metrics.jsonl` with `resourceMetrics` for low-cardinality record kind,
  verifier status, policy decision, and pass-ratio metrics;
- `manifest.json` with source-ledger metadata, file hashes, byte counts, and
  item counts.

The ledger remains authoritative. The exporter verifies the ledger hash chain
before writing, writes to a temp directory, reads all JSONL files and the
manifest back, promotes the directory, and verifies the promoted files again.

Phase 7 shipped local dashboard metrics, Phase 8 shipped audit/SIEM files,
Phase 9 shipped hosted GitLab CI readbacks, Phase 10 shipped local scanner
artifact readbacks, Phase 11 shipped GitHub ticket evidence-link readbacks,
Phase 12 shipped deterministic scorecards, Phase 13 shipped privacy
retention/erasure decision audit rows, and Phase 14 shipped release-gate
decision rows. Phase 15 shipped usage-meter JSONL package output and kept
OpenTelemetry emission as a later export path. Phase 16 shipped
risk-assessment rows and dashboard risk readbacks while keeping OpenTelemetry
as an export-only path. Phase 18 shipped a manifest-verified first-wedge demo
package and durable `first_wedge_demo` rows. Phase 23 ships the export-only
OpenTelemetry path without adding ledger record kinds.

## Versioning

- APIs are versioned by URL.
- Ledger records include `schema_version` (= 13 in Phase 23).
- Phase 3 reused schema version 2 and added no new record kinds. Phase 4 bumped
  the schema to 3 for `evidence_packet` and `incident_packet`.
- Phase 5 did not bump the ledger schema. It writes export artifacts outside
  the ledger with `episode_schema_version = 1`.
- Phase 6 bumps the ledger schema to 4 for `policy_decision`, `agent_step`,
  and `model_decision`.
- Phase 7 does not bump the ledger schema. It writes dashboard artifacts
  outside the ledger with `dashboard_schema_version = 1`.
- Phase 8 bumps the ledger schema to 5 for `enterprise_tenant_config`,
  `enterprise_directory_snapshot`, and `enterprise_access_decision`.
- Phase 9 does not bump the ledger schema. It reuses Phase 2-4 verifier and
  evidence records for GitLab CI readbacks.
- Phase 10 does not bump the ledger schema. It reuses Phase 2-4 verifier and
  evidence records for scanner artifact readbacks.
- Phase 11 does not bump the ledger schema. It reuses Phase 2-4 verifier and
  evidence records for GitHub issue evidence-link readbacks.
- Phase 12 bumps the ledger schema to 6 for the new `scorecard` record kind.
- Phase 13 bumps the ledger schema to 7 for `retention_decision` and
  `evidence_deletion`.
- Phase 14 bumps the ledger schema to 8 for the new `release_gate` record
  kind.
- Phase 15 bumps the ledger schema to 9 for the new `pilot_package` record
  kind and writes package artifacts outside the ledger with
  `pilot_package_schema_version = 1`.
- Phase 16 bumps the ledger schema to 10 for the new `risk_assessment` record
  kind with `risk_assessment_schema_version = 1`.
- Phase 17 bumps the ledger schema to 11 for the new `http_request_audit`
  record kind with `http_request_audit_schema_version = 1`.
- Phase 18 bumps the ledger schema to 12 for the new `first_wedge_demo`
  record kind with `first_wedge_demo_schema_version = 1`.
- Phase 19 bumps the ledger schema to 13 for the new
  `full_state_verification` record kind.
- Phase 20 does not bump the ledger schema. It reuses Phase 2-4 verifier and
  evidence records for Kubernetes Deployment snapshot readbacks.
- Phase 21 does not bump the ledger schema. It reuses Phase 2-4 verifier and
  evidence records for Terraform/cloud artifact readbacks.
- Phase 22 does not bump the ledger schema. It reuses Phase 2-4 verifier and
  evidence records for ticketing artifact readbacks.
- Phase 23 does not bump the ledger schema. It writes OpenTelemetry export
  artifacts outside the ledger with `blackbox_otel_export_manifest_json_v1`.
- Verifier specs include `verifier_id` with version suffix.
- Connectors include `connector_id` with version suffix
  (`sqlite_local_v1`, `git_local_v1`, `fs_local_v1`, `cmd_local_v1`,
  `github_v1`, `gha_v1`, `enterprise_controls_v1`, `gitlab_ci_v1`,
  `scanner_artifact_v1`, `blackbox_scoring_v1`, `blackbox_privacy_v1`,
  `blackbox_release_gate_v1`, `blackbox_packaging_v1`,
  `blackbox_risk_v1`, `blackbox_http_server_v1`, and
  `blackbox_first_wedge_demo_v1`, `blackbox_fsv_protocol_v1`,
  `k8s_deployment_snapshot_v1`, `terraform_cloud_artifact_v1`,
  `ticketing_artifact_v1`, and `blackbox_otel_export_v1` today).
- Export manifests record all versions.
- Schema bumps wipe the ledger; no migration shims.

## FSV Of The CLI Surface

For each shipped command, the FSV protocol is:

1. Run the command with a real synthetic input.
2. Read the SQLite source of truth directly.
3. For connectors/verifiers, read Git/filesystem/command artifacts directly.
4. Confirm row counts, payload structure, readbacks, and hash-chain consistency
   match.

Concrete inputs and expected outputs live in
`20_phase1_state_and_invariants.md`,
`21_phase2_state_and_invariants.md`,
`22_phase3_state_and_invariants.md`,
`23_phase4_state_and_invariants.md`,
`24_phase5_state_and_invariants.md`,
`25_phase6_state_and_invariants.md`,
`26_phase7_state_and_invariants.md`,
`27_phase8_state_and_invariants.md`,
`28_phase9_state_and_invariants.md`,
`29_phase10_state_and_invariants.md`,
`30_phase11_state_and_invariants.md`,
`31_phase12_state_and_invariants.md`,
`32_phase13_state_and_invariants.md`,
`33_phase14_state_and_invariants.md`,
`34_phase15_state_and_invariants.md`,
`35_phase16_state_and_invariants.md`,
`36_phase17_state_and_invariants.md`,
`37_phase18_state_and_invariants.md`,
`38_phase19_state_and_invariants.md`,
`39_phase20_state_and_invariants.md`,
`40_phase21_state_and_invariants.md`,
`41_phase22_state_and_invariants.md`, and
`42_phase23_state_and_invariants.md`. The automated equivalents are
`crates/context-graph-cli/tests/blackbox_phase1_fsv.rs` (10 tests, PASS) and
`crates/context-graph-cli/tests/blackbox_phase2_fsv.rs` (19 tests, PASS),
plus `crates/context-graph-cli/tests/blackbox_phase3_fsv.rs` (1 live
integration test, PASS) and
`crates/context-graph-cli/tests/blackbox_phase4_fsv.rs` (4 tests, PASS) and
`crates/context-graph-cli/tests/blackbox_phase5_export_fsv.rs` (4 tests,
PASS), `crates/context-graph-cli/tests/blackbox_phase6_mcp_policy_fsv.rs`
(4 tests, PASS), and
`crates/context-graph-cli/tests/blackbox_phase7_dashboard_fsv.rs` (5 tests,
PASS), `crates/context-graph-cli/tests/blackbox_phase8_enterprise_fsv.rs`
(6 tests, PASS), `crates/context-graph-cli/tests/blackbox_phase9_gitlab_ci_fsv.rs`
(1 live integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase10_scanner_fsv.rs` (1 local
integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase11_github_issue_evidence_fsv.rs`
(1 live integration test, PASS),
`crates/context-graph-cli/tests/blackbox_phase12_scorecard_fsv.rs`
(1 local integration test, PASS),
`crates/context-graph-cli/tests/blackbox_phase13_privacy_retention_fsv.rs`
(1 local integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase14_release_gate_fsv.rs`
(1 local integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase15_pilot_package_fsv.rs`
(1 local integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase16_risk_register_fsv.rs`
(1 local integration test, PASS),
`crates/context-graph-cli/tests/blackbox_phase17_http_server_fsv.rs`
(1 local integration test, PASS),
`crates/context-graph-cli/tests/blackbox_phase18_first_wedge_demo_fsv.rs`
(1 local integration test, PASS),
`crates/context-graph-cli/tests/blackbox_phase19_fsv_protocol_fsv.rs`
(1 local integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase20_k8s_deployment_fsv.rs`
(1 local integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase21_terraform_cloud_fsv.rs`
(1 local integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase22_ticketing_fsv.rs`
(1 local integration test, PASS), and
`crates/context-graph-cli/tests/blackbox_phase23_otel_export_fsv.rs`
(4 local integration tests, PASS).
