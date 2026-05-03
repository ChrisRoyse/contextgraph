# Phase 3 — State, Invariants, And Live FSV Tests

This document is the canonical reference for the **shipped** Phase 3
implementation.

## Source Of Truth

| Surface | Physical source of truth |
|---|---|
| Ledger | SQLite file at `--ledger PATH`, table `ledger_records` |
| GitHub PRs/issues/branches/check suites | GitHub REST API for the target `OWNER/REPO` |
| GitHub Actions runs | GitHub REST API workflow-run endpoints for the target `OWNER/REPO` |

Phase 3 reused `LEDGER_SCHEMA_VERSION = 2`; it added no new record kinds. The
current live ledger schema is `LEDGER_SCHEMA_VERSION = 5` after Phase 11.
`blackbox verify` writes `expected_delta`, `readback_snapshot` when a source
read occurred, `verifier_result`, and then Phase 4 evidence packet rows.

## Shipped Connectors

```text
github_v1   GitHub repository, pull request, issue, commit, and check-suite readback
gha_v1      GitHub Actions workflow-run readback
```

The GitHub client is async `reqwest`, not `reqwest::blocking`, because the CLI
runs under Tokio. The client sends:

- `Accept: application/vnd.github+json`
- `X-GitHub-Api-Version: 2022-11-28`
- `User-Agent: contextgraph-blackbox/0.1`
- optional bearer token from `BBX_GITHUB_TOKEN` or `GITHUB_TOKEN`

`401`/`403` are `CONNECTOR_PERMISSION_DENIED`. Rate-limit exhaustion is
`GITHUB_RATE_LIMITED`. Network/service instability is
`SOURCE_OF_TRUTH_UNAVAILABLE`. Invalid/non-JSON source responses are
`READBACK_SCHEMA_DRIFT`.

## Shipped Verifiers

| Verifier | Required spec | Passed condition | Failed condition | Blocked condition |
|---|---|---|---|---|
| `github_pr_state_v1` | `source.owner`, `source.repo`, `expected.pr_number`, `expected.state` | PR exists and state matches | `GITHUB_PR_NOT_FOUND`, `GITHUB_PR_STATE_MISMATCH` | schema/source error, auth/rate/network/API error |
| `github_branch_head_matches_commit_v1` | `source.owner`, `source.repo`, `expected.branch`, `expected.commit` | branch/ref resolves to expected commit SHA | `GITHUB_BRANCH_NOT_FOUND`, `GITHUB_BRANCH_HEAD_MISMATCH` | schema/source error, auth/rate/network/API error |
| `github_issue_state_changed_v1` | `source.owner`, `source.repo`, `expected.issue_number`, `expected.state` | issue exists, is not a PR, and state matches | `GITHUB_ISSUE_NOT_FOUND`, `GITHUB_ISSUE_IS_PULL_REQUEST`, `GITHUB_ISSUE_STATE_MISMATCH` | schema/source error, auth/rate/network/API error |
| `github_check_suite_passed_v1` | `source.owner`, `source.repo`, `expected.ref`, `expected.conclusion` | at least one suite exists and all conclusions match | `GITHUB_CHECK_SUITE_NOT_FOUND`, `GITHUB_CHECK_SUITE_CONCLUSION_MISMATCH` | schema/source error, auth/rate/network/API error |
| `gha_workflow_run_completed_v1` | `source.owner`, `source.repo`, `expected.run_id`, `expected.status`, `expected.conclusion` | workflow run status and conclusion match | `GHA_WORKFLOW_RUN_NOT_FOUND`, `GHA_WORKFLOW_RUN_STATUS_MISMATCH`, `GHA_WORKFLOW_RUN_CONCLUSION_MISMATCH` | schema/source error, auth/rate/network/API error |

Schema/type failures are `blocked` and still write `expected_delta` and
`verifier_result` when the top-level verifier spec can be associated with an
existing run/tool-call. They do not write `readback_snapshot` if no physical
read occurred.

## Automated FSV

Run:

```bash
cargo test --package context-graph-cli --test blackbox_phase3_fsv -- --nocapture
```

Current suite: 1/1 passing. The test is a live integration test, not a mock.
It discovers real data from `actions/checkout` by default, or from
`BBX_GITHUB_TEST_REPO=OWNER/REPO` if set. It then runs the CLI and reads the
physical SQLite ledger directly.

Coverage:

- connector list/test for `github_v1` and `gha_v1`;
- happy path for all five Phase 3 verifiers in one recorded run;
- missing target, wrong/stale expected value, PR-vs-issue ambiguity, and
  invalid schema/source edge cases;
- endpoint-specific missing-ref behavior where GitHub returns `422` for
  absent commit/check-suite refs.

## Manual FSV Evidence

Manual FSV completed on 2026-05-02:

```text
Log: /tmp/bbx-phase3-manual-fsv.KZAAKb/fsv.log
Ledger source of truth: /tmp/bbx-phase3-manual-fsv.KZAAKb/blackbox.sqlite#ledger_records
GitHub source of truth: actions/checkout via GitHub REST API
Final record counts: agent_run=1, tool_call=1, expected_delta=8,
readback_snapshot=7, verifier_result=8
```

Live source snapshot used by the manual test:

```text
repo=actions/checkout default_branch=main
main_sha=0c366fd6a839edf440554fa01a7085ccba70ac98
pr=2418 state=open head_sha=b23eb344e8ec38502b46db2f402ab6445a12fea0
issue=2424 state=open
workflow_run=25068311674 status=completed conclusion=success
workflow_head_sha=71f7057fab274d6d2bd87f9525dce0cdc9fe895e
check_suites=6 conclusions=[success, success, success, success, success, success]
```

Happy-path physical proofs:

- `github_pr_state_v1` persisted `status=passed`, PR `2418`, `state=open`,
  `head_sha=b23eb344e8ec38502b46db2f402ab6445a12fea0`.
- `github_branch_head_matches_commit_v1` persisted `status=passed`,
  branch `main`, SHA `0c366fd6a839edf440554fa01a7085ccba70ac98`.
- `github_issue_state_changed_v1` persisted `status=passed`, issue `2424`,
  `state=open`, `is_pull_request=false`.
- `github_check_suite_passed_v1` persisted `status=passed`, `total_count=6`,
  `mismatched_count=0`.
- `gha_workflow_run_completed_v1` persisted `status=passed`, run
  `25068311674`, `status=completed`, `conclusion=success`.

Manual edge cases:

1. Missing PR `999999999`: before count `readback_snapshot=5`, after
   `readback_snapshot=6`; verifier persisted `status=failed`,
   `error_code=GITHUB_PR_NOT_FOUND`, `http_status=404`.
2. Stale branch expected SHA `0000000000000000000000000000000000000000`:
   before count `readback_snapshot=6`, after `readback_snapshot=7`;
   verifier persisted `status=failed`,
   `error_code=GITHUB_BRANCH_HEAD_MISMATCH`, actual SHA
   `0c366fd6a839edf440554fa01a7085ccba70ac98`.
3. Invalid GHA `expected.run_id="not-an-integer"`: before count
   `readback_snapshot=7`, after `readback_snapshot=7`; verifier persisted
   `status=blocked`, `error_code=VERIFIER_SPEC_FIELD_TYPE`, and no readback
   snapshot was written.

## Invariants

- No GitHub/GHA verifier may run under `reqwest::blocking` inside the Tokio CLI
  runtime.
- No `401`, `403`, or rate-limit response may be converted into a verifier
  mismatch; those are blocked connector states.
- No GitHub `404`/endpoint-specific absent-ref `422` may be treated as success.
  It must become a failed verifier with persisted readback when the endpoint
  read completed.
- No issue verifier may accept a pull request returned by the Issues API; PRs
  must use the PR verifier.
- No schema/type failure may write a `readback_snapshot` unless a real source
  read occurred.
- No GitHub artifact may be created by Phase 3 tests. The test suite is
  read-only against GitHub and mutates only temporary local SQLite ledgers.
