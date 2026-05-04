# Demo Example Catalog

This catalog defines the first demos for showing BlackBoxFix capabilities.
Each example must run against real local files, commands, and ledger state.

## Example 1: Verified Deployment Artifact Fix

Gemma fixes a broken deployment YAML health-check configuration. BlackBoxFix
proves the exact file changed, validation passed, and Git diff matched the
expected patch.

Implementation note: the current shipped harness expands this from a deployment
file wedge into a full AgentOps research demo. The original deployment scenario
remains a good buyer-friendly follow-on, but the implemented proof now uses the
real DynamicJEPA pipeline because it demonstrates more of the system:

- data ingest into RocksDB;
- panel materialization;
- action-conditioned dataset export;
- CUDA tiny training artifact provenance;
- planning and guard decisions;
- surprise recording;
- BlackBoxFix verifier pass plus deliberate failed verifier incident;
- FSV certification.

Implemented command:

```bash
scripts/blackboxfix_gemma_tool_demo.py \
  --root /tmp/blackboxfix-agentops-demo-manual-03 \
  --model gemma4:e4b \
  --blackbox-bin target/debug/context-graph
```

Latest proof summary:

```text
DynamicJEPA: raw_events=62, latent_panels=60, dataset_shards=3,
training_runs=1, model_artifacts=1, predictions=6, guard_decisions=5,
plan_traces=1, surprise_events=1, verification_runs=1.

BlackBoxFix: agent_run=1, agent_step=1, model_decision=1, tool_call=1,
expected_delta=2, readback_snapshot=2, verifier_result=2,
evidence_packet=2, incident_packet=1, scorecard=1,
full_state_verification=1.
```

Source of truth:

- `repo/deploy/service.yaml`
- `repo/tools/validate_deployment.py`
- `repo/.git`
- `blackbox.sqlite#ledger_records`

Tools:

- `read_deployment_file`
- `run_deployment_validator`
- `write_deployment_file`
- `read_git_diff`

Success proof:

- file exists and SHA-256 matches;
- validator exits `0`;
- Git diff matches expected patch;
- ledger contains run, decision, tool, verifier, evidence, scorecard, and FSV
  rows.

Failure variant:

- expected patch claims `timeout_seconds: 9`;
- actual diff contains `timeout_seconds: 5`;
- BlackBoxFix stores `verifier_result.status=failed` and an `incident_packet`.

## Example 2: Scanner Finding Remediation Proof

Gemma updates a pinned dependency after a scanner artifact reports a vulnerable
version. BlackBoxFix proves the lockfile and scanner artifacts reflect the
fixed state.

Source of truth:

- `repo/Cargo.lock` or a small lockfile fixture;
- `artifacts/osv-report.json`;
- `artifacts/sbom.cdx.json`;
- `blackbox.sqlite#ledger_records`.

Tools:

- `read_file`
- `write_file`
- `run_lockfile_validator`
- `read_scanner_report`

Verifiers:

- `scanner_vulnerability_absent_v1`
- `scanner_lockfile_pinned_to_version_v1`
- `scanner_sbom_artifact_present_v1`
- `cmd_exit_zero_v1`

Failure variant:

- Gemma updates a manifest but not the lockfile. Scanner/lockfile verification
  fails and creates an incident.

## Example 3: Ticket Evidence Closure

Gemma closes a ticket only after attaching evidence. BlackBoxFix proves ticket
state and evidence link from a captured provider artifact.

Source of truth:

- Jira, Linear, ServiceNow, or GitHub issue JSON artifact;
- generated evidence export file;
- `blackbox.sqlite#ledger_records`.

Tools:

- `read_ticket_artifact`
- `write_ticket_artifact`
- `attach_evidence_link`

Verifiers:

- `jira_ticket_state_v1`
- `linear_ticket_state_v1`
- `servicenow_ticket_state_v1`
- `github_issue_evidence_link_v1`

Failure variant:

- ticket status is `Done` but no evidence link exists. Verification fails.

## Example 4: Kubernetes Deployment Snapshot Proof

Gemma fixes a captured Kubernetes Deployment snapshot. BlackBoxFix proves image
digest, replica count, and rollout state.

Source of truth:

- `artifacts/k8s/deployment.json`
- `blackbox.sqlite#ledger_records`

Tools:

- `read_k8s_snapshot`
- `write_k8s_snapshot`
- `run_k8s_snapshot_validator`

Verifiers:

- `k8s_deployment_image_digest_v1`
- `k8s_replicas_match_v1`
- `k8s_rollout_completed_v1`

Failure variant:

- image still uses a floating tag or replicas are wrong. BlackBoxFix records a
  failed verifier and incident.

## Example 5: Terraform Plan Guardrail

Gemma adjusts a Terraform plan artifact so only approved actions remain.
BlackBoxFix proves plan JSON, state attributes, and cloud inventory evidence.

Source of truth:

- `artifacts/tf/plan.json`
- `artifacts/tf/state.json`
- `artifacts/cloud/inventory.json`
- `blackbox.sqlite#ledger_records`

Tools:

- `read_tf_plan`
- `write_tf_plan`
- `read_tf_state`
- `run_tf_plan_validator`

Verifiers:

- `tf_plan_only_approved_changes_v1`
- `tf_state_attribute_v1`
- `cloud_resource_present_v1`

Failure variant:

- plan includes an unapproved delete action. Verifier fails and the release
  gate later blocks.

## Example 6: Release Gate From Verified Evidence

After a verified fix, BlackBoxFix computes a scorecard and evaluates release
readiness.

Source of truth:

- ledger scorecard row;
- release gate row;
- enterprise access decision row.

Commands:

- `blackbox score run`
- `blackbox gate evaluate`
- optional `blackbox enterprise access-evaluate`

Failure variant:

- run contains an incident or missing scorecard. Gate persists `blocked`.

## Example 7: Buyer Package And Training Corpus Export

After several verified actions, BlackBoxFix exports buyer evidence and
training/eval episodes.

Source of truth:

- `blackbox.sqlite#ledger_records`
- `pilot-report/manifest.json`
- `episodes/manifest.json`
- `dashboard/manifest.json`

Commands:

- `blackbox package pilot-report`
- `blackbox export episodes`
- `blackbox dashboard build`
- `blackbox export otel`

Failure variant:

- package is requested before minimum verified evidence exists. Packaging fails
  closed with typed proof requirements.
