# Phase 9 State And Invariants - GitLab CI Connector

Phase 9 ships the first hosted CI connector after GitHub Actions:
`gitlab_ci_v1`. It verifies GitLab pipeline and job state through GitLab REST
API v4 and persists proof in the existing SQLite ledger evidence path. There
was no schema bump in Phase 9; `LEDGER_SCHEMA_VERSION` remained `5` then.
The current live schema after Phase 14 is `8`.

## Source Of Truth

Primary source of truth:

- GitLab project pipeline endpoint:
  `GET /projects/:id/pipelines/:pipeline_id`;
- GitLab pipeline jobs endpoint:
  `GET /projects/:id/pipelines/:pipeline_id/jobs`;
- local SQLite ledger table `ledger_records` for expected deltas, readback
  snapshots, verifier results, evidence packets, and incident packets.

Manual/automated FSV must read GitLab independently from the CLI result, then
read the physical SQLite table directly. CLI stdout/stderr alone is not proof.

## Shipped CLI Surface

```bash
context-graph blackbox connector test \
  --connector gitlab_ci_v1 \
  --source GROUP/PROJECT

context-graph blackbox verify \
  --ledger /tmp/bbx.sqlite \
  --input spec.json \
  --tenant TENANT \
  --agent AGENT
```

`BBX_GITLAB_TOKEN` or `GITLAB_TOKEN` may be set for private projects. Tokens
are sent as GitLab `PRIVATE-TOKEN` headers and are never persisted.

## Verifier Specs

`gitlab_pipeline_status_v1` requires:

```json
{
  "source": { "project": "GROUP/PROJECT" },
  "expected": {
    "pipeline_id": 2495492680,
    "status": "success",
    "ref": "18-11-stable",
    "sha": "6822948553cf9cba4bb3c063f36e1a6001746af2"
  }
}
```

It passes only when the pipeline exists and all three fields match:
`status`, `ref`, and `sha`.

`gitlab_pipeline_jobs_completed_v1` requires:

```json
{
  "source": { "project": "GROUP/PROJECT" },
  "expected": {
    "pipeline_id": 2495492680,
    "allowed_statuses": ["success"],
    "allow_failure_failed_ok": false,
    "minimum_jobs": 1
  }
}
```

It passes only when:

- `minimum_jobs >= 1`;
- every jobs page is read using GitLab `Link`/`x-next-page` pagination
  headers;
- the number of observed jobs is at least `minimum_jobs`;
- every observed job status is in `allowed_statuses`, except explicitly
  tolerated failed `allow_failure` jobs when `allow_failure_failed_ok=true`.

## Invariants

- Project source must be a GitLab URL path like `group/project` or
  `group/subgroup/project`; query strings, fragments, empty path segments, and
  shell metacharacters are rejected.
- `connector test` for GitLab is read-only and does not accept `--command`.
- Readback snapshots store HTTP status, final URL, request id, ETag,
  rate-limit headers, pagination headers, response body SHA-256, raw body, and
  normalized observed state.
- Jobs verification stores `observed_state.api_pages[]`; a partial jobs page
  read cannot pass. A full page without `Link rel="next"` or `x-next-page`
  proof is blocked rather than treated as complete.
- `401`/`403` is `CONNECTOR_PERMISSION_DENIED`; `429` or rate-limit exhaustion
  is `GITLAB_RATE_LIMITED`; non-JSON success responses are
  `READBACK_SCHEMA_DRIFT`.
- Invalid expected pipeline/job statuses are blocked before source reads.
- `minimum_jobs=0` is blocked before source reads so an empty pipeline cannot
  be reported as complete.
- Missing or mismatched source-of-truth state is failed and produces an
  `incident_packet`; invalid verifier specs are blocked and produce no
  incident packet.

## Error Codes

- `GITLAB_PROJECT_LOCATOR_INVALID`
- `GITLAB_PROJECT_NOT_FOUND`
- `GITLAB_PIPELINE_NOT_FOUND`
- `GITLAB_PIPELINE_STATUS_INVALID`
- `GITLAB_PIPELINE_STATUS_MISMATCH`
- `GITLAB_PIPELINE_REF_MISMATCH`
- `GITLAB_PIPELINE_SHA_MISMATCH`
- `GITLAB_PIPELINE_JOBS_NOT_FOUND`
- `GITLAB_PIPELINE_JOBS_MISSING`
- `GITLAB_PIPELINE_JOBS_STATUS_MISMATCH`
- `GITLAB_JOB_STATUS_INVALID`
- `GITLAB_MINIMUM_JOBS_INVALID`
- `GITLAB_PAGINATION_INVALID`
- `GITLAB_PAGINATION_INCOMPLETE`
- `GITLAB_RATE_LIMITED`
- `CONNECTOR_PERMISSION_DENIED`
- `READBACK_SCHEMA_DRIFT`
- `SOURCE_OF_TRUTH_UNAVAILABLE`

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase9_gitlab_ci_fsv -- --nocapture
```

Current result: 1/1 passing.

The test uses real GitLab state. It discovers a successful pipeline with
`curl`, drives the real `context-graph blackbox` binary, then opens the
physical SQLite ledger with `rusqlite` and checks row counts and payloads.
Default fixture: `gitlab-org/gitlab-runner`. Override with
`BBX_GITLAB_TEST_PROJECT`.

Coverage:

- connector list includes `gitlab_ci_v1`;
- connector test reads real GitLab project metadata;
- happy pipeline status verifier passes;
- happy jobs verifier passes and records `jobs_total`;
- missing pipeline fails;
- wrong pipeline status fails;
- wrong pipeline ref fails;
- invalid pipeline id schema blocks;
- missing jobs endpoint fails;
- wrong allowed job status fails;
- minimum job count too high fails;
- invalid GitLab job status blocks;
- `minimum_jobs=0` blocks.

## Manual FSV Evidence

Manual run:

- log: `/tmp/bbx-phase9-manual-fsv.IT6RIN/fsv.log`;
- SQLite source of truth:
  `sqlite:/tmp/bbx-phase9-manual-fsv.IT6RIN/gitlab.sqlite#ledger_records`;
- GitLab source of truth: `gitlab:gitlab-org/gitlab-runner`.

Discovered live fixture:

- `project_id=250833`;
- `pipeline_id=2495492680`;
- `status=success`;
- `ref=18-11-stable`;
- `sha=6822948553cf9cba4bb3c063f36e1a6001746af2`;
- `jobs_total=4`;
- job status counts: `[{"status":"success","count":4}]`.

Happy-path proof from physical SQLite:

- `gitlab_pipeline_status_v1` verifier row sequence `5`, status `passed`,
  actual status `success`;
- `gitlab_pipeline_jobs_completed_v1` verifier row sequence `9`, status
  `passed`, actual `jobs_total=4`, `pages_read=1`;
- jobs readback row sequence `8` stored `api_pages=1`, `exists=1`,
  `jobs_total=4`, `pages_read=1`, and a GitLab pagination `Link` header.

Edge-case proof from physical SQLite:

| Case | Expected result | Verifier row |
|---|---|---|
| missing pipeline id `9999999999` | `failed`, `GITLAB_PIPELINE_NOT_FOUND` | sequence `13` |
| expected status `failed` against actual `success` | `failed`, `GITLAB_PIPELINE_STATUS_MISMATCH` | sequence `18` |
| `minimum_jobs=5` against actual `4` | `failed`, `GITLAB_PIPELINE_JOBS_MISSING` | sequence `23` |
| allowed job status `done` | `blocked`, `GITLAB_JOB_STATUS_INVALID` | sequence `27` |
| `minimum_jobs=0` | `blocked`, `GITLAB_MINIMUM_JOBS_INVALID` | sequence `30` |

Final physical SQLite counts:

```json
{
  "agent_run": 1,
  "tool_call": 1,
  "expected_delta": 7,
  "readback_snapshot": 5,
  "verifier_result": 7,
  "evidence_packet": 7,
  "incident_packet": 3
}
```

Final metadata:

```json
{
  "schema_version": "5",
  "ledger_record_count": "31",
  "ledger_tip_sha256": "sha256:9b5586a1bfc5d75db38c65aa8fccd982795f716a74eb3e83b753cfa6a9b823b9"
}
```

The manual run printed before/after SQLite counts for every case and listed
all final verifier/readback/evidence rows, including evidence state hashes.
