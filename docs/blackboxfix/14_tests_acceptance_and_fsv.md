# Tests, Acceptance, And Full-State Verification

Testing must prove the system works by inspecting durable state after
execution. Unit tests are useful, but they are not enough for this product.
The mandatory operator-side protocol lives in
`19_full_state_verification_protocol.md`; this file lists the testing
**levels** and the per-phase acceptance criteria.

## Source Of Truth For BlackBoxFix Itself

| Source | What it proves |
|---|---|
| SQLite ledger DB at `--ledger PATH` | every row written, every hash chain |
| `blackbox_metadata` rows | record count + tip hash agree with physical rows |
| Evidence packet rows + exported report files (Phase 4) | packet references match physical rows and export files read back byte-for-byte |
| Hash-chain verification output | per-record + per-run chain unbroken |
| External systems under connector tests | readback matches expected delta |
| Exported JSONL + manifest (Phase 5) | every line re-parses; sha256 matches manifest |
| MCP tool registry + policy rows (Phase 6) | advertised tools match shipped schemas; governed decisions persist in `ledger_records` |
| Dashboard bundle (Phase 7) | `index.html`, `manifest.json`, and `api/*.json` read back from disk; manifest sha256 entries match actual bytes |
| Enterprise controls and audit export (Phase 8) | SSO/RBAC/retention/key config rows, SCIM directory rows, access decision rows, `audit.jsonl`, `siem-rfc5424.log`, `controls.json`, and `manifest.json` read back directly |
| GitLab CI API + ledger evidence rows (Phase 9) | live GitLab project/pipeline/job state read independently and persisted as `expected_delta`, `readback_snapshot`, `verifier_result`, `evidence_packet`, and incident rows |
| Scanner artifact files + ledger evidence rows (Phase 10) | OSV JSON reports, Cargo lockfiles, CycloneDX JSON SBOM files, and physical SQLite rows read directly |
| GitHub Issues API + ledger evidence rows (Phase 11) | live GitHub issue body/comment state read independently and persisted as `expected_delta`, `readback_snapshot`, `verifier_result`, `evidence_packet`, and incident rows |
| Scorecard rows (Phase 12) | physical SQLite verifier/evidence/policy/incident/tool-call rows read independently and persisted as `scorecard` rows |
| Privacy retention/deletion rows (Phase 13) | physical SQLite enterprise config/directory/access rows and run rows read independently, then persisted as `retention_decision` and `evidence_deletion` rows |
| Release gate decisions (Phase 14) | physical SQLite scorecard/evidence/policy/privacy/incident/access rows read independently, then persisted as `release_gate` rows |
| Pilot package rows and files (Phase 15) | physical SQLite tenant evidence/access rows read independently, then persisted as `pilot_package`; package JSON/JSONL files and dashboard package API files read back from disk with manifest hashes |
| Risk assessment rows and decisions (Phase 16) | physical SQLite tenant evidence/runtime/access/package rows and a real decision-register JSON file read independently, then persisted as `risk_assessment`; dashboard risk API files read back from disk |
| HTTP request audit rows (Phase 17) | real HTTP requests to `blackbox server serve`, physical SQLite business rows, and `http_request_audit` rows read independently after each request |

## Test Levels

### Unit Tests

- schema validation;
- redaction (field-name + literal pattern + post-scan);
- canonical-JSON sorting;
- hash computation;
- comparison functions;
- score calculation;
- error-code mapping.

### Integration Tests

- connector readback against real local systems (no mocks);
- ledger write and readback on a real SQLite file;
- evidence artifact hash verification;
- exporter parse and manifest verification.

### Full-State Verification (FSV)

For each mutating workflow, follow the protocol in
`19_full_state_verification_protocol.md`:

1. Print the before-state from the source of truth.
2. Execute the action.
3. Print the after-state from the source of truth through a separate read.
4. Verify the expected persisted delta matches.
5. Print evidence packet ids and ledger row sequences.
6. Run at least three edge cases.

Phase 1 has automated FSV (`tests/blackbox_phase1_fsv.rs`). Phase 2 has
automated FSV (`tests/blackbox_phase2_fsv.rs`) against real SQLite ledgers,
real Git repositories, real files, and real commands. Phase 6 adds
`tests/blackbox_phase6_mcp_policy_fsv.rs`, which executes the real MCP stdio
gateway against a real Git patch and then reads SQLite/Git state directly.
Phase 7 adds `tests/blackbox_phase7_dashboard_fsv.rs`, which builds a real
dashboard bundle and reads SQLite/dashboard files directly. Phase 8 adds
`tests/blackbox_phase8_enterprise_fsv.rs`, which persists real enterprise
control rows, exports real audit/SIEM files, and reads SQLite/export files
directly. Phase 9 adds `tests/blackbox_phase9_gitlab_ci_fsv.rs`, which
discovers live GitLab source-of-truth state with `curl`, runs the real CLI,
and reads SQLite rows directly. Phase 10 adds
`tests/blackbox_phase10_scanner_fsv.rs`, which writes real scanner artifact
files, runs the real CLI, and reads SQLite rows plus artifact files directly.
Phase 11 adds `tests/blackbox_phase11_github_issue_evidence_fsv.rs`, which
discovers live GitHub issue source-of-truth state with `curl`, verifies the
evidence link through the real CLI, and reads SQLite rows directly.
Phase 12 adds `tests/blackbox_phase12_scorecard_fsv.rs`, which uses a real
SQLite ledger and real local Git verifier evidence, appends real `scorecard`
rows, and reads those rows directly from `ledger_records`.
Phase 13 adds `tests/blackbox_phase13_privacy_retention_fsv.rs`, which uses a
real SQLite ledger with real enterprise config/directory rows, appends real
authorization, retention, and erasure-decision rows, and reads them directly
from `ledger_records`.
Phase 14 adds `tests/blackbox_phase14_release_gate_fsv.rs`, which uses a real
SQLite ledger with real enterprise config/directory rows, real local Git
verifier evidence, real scorecard/privacy rows, appends approved and blocked
`release_gate` rows, and reads them directly from `ledger_records`.
Phase 15 adds `tests/blackbox_phase15_pilot_package_fsv.rs`, which uses a
real SQLite ledger with real enterprise, Git verifier, scorecard, privacy,
and release-gate rows, appends real `pilot_package` rows, reads them directly
from `ledger_records`, and reads package/dashboard JSON/JSONL files directly
from disk.
Phase 16 adds `tests/blackbox_phase16_risk_register_fsv.rs`, which uses a
real SQLite ledger with real enterprise, Git verifier, scorecard, privacy,
release-gate, package, and decision-register state, appends real
`risk_assessment` rows, reads them directly from `ledger_records`, and reads
dashboard/API risk files directly from disk.
Phase 17 adds `tests/blackbox_phase17_http_server_fsv.rs`, which starts the
real HTTP server process, sends real authenticated HTTP requests, appends
real `agent_run`, `agent_step`, `tool_call`, and `http_request_audit` rows,
and reads those rows directly from `ledger_records`.

## Acceptance Criteria

The MVP is accepted only when, **after running on a fresh checkout against
real systems**:

- every mutating action has a verifier or is blocked in governed mode;
- every verifier has happy path and three edge cases;
- every failure has error code, source of truth, field path, and remediation;
- every evidence packet can be read back from storage;
- every exported JSONL row links to evidence;
- hash-chain tampering is detected;
- redaction failures stop persistence;
- connector permission failures are not reported as success.

## Manual FSV Template

```text
FSV: <verifier_id>
Source of truth: <system + locator>

Before:
  <field>: <value>

Trigger:
  <command or tool call>

After readback (via independent read path):
  <field>: <value>

Expected delta:
  <field>: <value>

Verifier:
  status: passed | failed | blocked
  error_code: <or null>
  evidence_packet_id: <ev_...>

Physical proof:
  <sqlite/git/api command output>
```

## Required Edge Cases (per verifier)

Mandatory three:

1. **Missing target state** (e.g. PR not found, branch missing, file absent).
2. **Wrong target state** (e.g. PR closed, file content mismatch).
3. **Permission or schema failure** (connector rejected; expected
   `BBX_*` or `*_PERMISSION_DENIED`).

Strongly recommended additional edge cases:

- stale readback;
- partial write;
- duplicate action;
- rate limit;
- redaction violation;
- artifact hash mismatch;
- tampered ledger record (Phase 1 already covers this).

## Real Data Rule

Tests use real local repositories, real temporary databases, real files, real
command output, and real connector sandboxes. **No test passes by asserting a
fabricated success response while the source of truth is broken.**

Concretely, that means:

- Phase 1 FSV opens the SQLite file with the standalone `rusqlite` library
  and queries `ledger_records` directly, then asserts row counts and
  payload contents — see `phase1_happy_path_persists_and_inspects_physical_rows`.
- Phase 2 FSV shells to the real `git` binary against a real test repo and
  uses direct `git rev-parse`/`git cat-file`/`git status`/`git diff` plus
  direct SQLite reads before asserting verifier output.
- Phase 3 FSV points at live GitHub/GitHub Actions state and writes only local
  SQLite rows.
- Phase 4 FSV reads `evidence_packet.record_refs[]` back from physical
  SQLite rows, verifies replay is read-only by comparing before/after counts,
  writes a real JSON export file, reads the file back from disk, byte-compares
  it, and parses it before success.
- Phase 5 FSV exports real `episodes.jsonl`, `manifest.json`, and eval
  fixtures, then reads every file back from disk, re-parses every JSONL line,
  checks the manifest SHA against actual bytes, and verifies every episode's
  evidence packet id exists in physical SQLite rows.
- Phase 6 FSV drives `blackbox mcp gateway` over real JSON-RPC stdio, applies
  a real Git patch, then independently reads `ledger_records` and `git diff`
  to prove `policy_decision`, `agent_step`, `model_decision`, `tool_call`,
  verifier, and `evidence_packet` rows exist only when expected.
- Phase 7 FSV builds `index.html`, `manifest.json`, and `api/*.json` from a
  real ledger with one passed run and one failed run, reads each file back
  from disk, verifies manifest hashes against actual bytes, and checks every
  dashboard evidence id exists in physical SQLite rows.
- Phase 8 FSV writes real enterprise config and SCIM directory JSON, verifies
  `enterprise_tenant_config`, `enterprise_directory_snapshot`, and
  `enterprise_access_decision` rows in physical SQLite, exports
  `audit.jsonl`, `siem-rfc5424.log`, `controls.json`, and `manifest.json`,
  then parses and hashes those files directly.
- Phase 9 FSV reads a real public GitLab project, pipeline, and jobs with
  `curl`, verifies the same state through `gitlab_ci_v1`, and then opens the
  physical SQLite ledger to prove expected/readback/verifier/evidence/incident
  rows exist only where expected. The jobs verifier records every GitLab API
  page in `readback_snapshot.observed_state.api_pages`; invalid verifier specs
  such as unsupported job statuses or `minimum_jobs=0` are blocked before
  source reads and therefore do not create readback rows.
- Phase 10 FSV writes real OSV JSON reports, a real Cargo lockfile, and real
  CycloneDX JSON SBOM files; verifies vulnerability absence, lockfile pinning,
  and SBOM presence; then opens the physical SQLite ledger to prove expected,
  readback, verifier, evidence, and incident rows exist only where expected.
  Invalid verifier specs such as unsupported lockfile formats,
  `minimum_components=0`, directory artifact paths, and component versions
  without component names are blocked before source reads and therefore do not
  create readback rows. A clean OSV report for the wrong package filters
  fails and creates incident evidence.
- Phase 11 FSV reads a real public GitHub issue and issue-comments endpoint,
  verifies an exact `http(s)` evidence link in the issue body/comments, then
  opens the physical SQLite ledger to prove expected, readback, verifier,
  evidence, and incident rows exist only where expected. Invalid verifier
  specs such as malformed evidence URLs, unsupported link scopes, invalid
  issue numbers, or invalid repository source segments are blocked before
  source reads and therefore do not create readback rows.
- Phase 17 FSV starts a real `blackbox server serve` process, sends real
  HTTP requests through `reqwest`, then opens the physical SQLite ledger to
  prove business rows and `http_request_audit` rows exist only where expected.
  Invalid auth, tenant mismatch, and oversized-body requests create no
  business rows and do create typed audit rows.
- Phase 12 FSV scores real ledger rows produced by real CLI commands and a
  real local Git verifier. It then opens the physical SQLite ledger to prove
  `scorecard` rows exist only where expected and contain the exact trust level,
  correctness, risk, confidence, training value, score inputs, and readback
  hashes expected from the source rows. Missing run ids fail as
  `SCORECARD_RUN_NOT_FOUND` and do not create scorecard rows.
- Phase 13 FSV writes real enterprise tenant config and SCIM directory rows,
  records real runs, then drives `blackbox privacy evaluate-retention` and
  `blackbox privacy request-erasure`. It opens the physical SQLite ledger to
  prove authorization rows, `retention_decision`, and `evidence_deletion` rows
  exist only where expected; legal hold, missing class, unauthorized principal,
  missing run, and empty subject edge cases produce typed errors or durable
  denials. The raw data subject is checked absent from `payload_json`, while
  the expected SHA-256 digest is present.
- Phase 14 FSV writes real enterprise tenant config and SCIM directory rows,
  creates real local Git verifier evidence, runs `blackbox score run`,
  evaluates privacy retention, then drives `blackbox gate evaluate`. It opens
  the physical SQLite ledger to prove approved/blocked `release_gate` rows and
  `enterprise_access_decision` rows exist only where expected. Missing
  scorecards, too-low risk thresholds, unauthorized principals, incident runs,
  and invalid thresholds produce typed errors or durable blocked decisions
  without fabricating success.
- Phase 15 FSV writes real package files from real SQLite/Git/enterprise/
  score/privacy/release-gate rows, then opens the physical SQLite ledger to
  prove `pilot_package` and `enterprise_access_decision` rows exist only where
  expected. It reads `pilot-report.json`, `usage-meter-events.jsonl`,
  `eval-candidates.jsonl`, `manifest.json`, dashboard
  `api/pilot-packages.json`, and dashboard `api/errors.json` directly from
  disk, verifies manifest hashes/byte counts, and checks unmet-criteria,
  unauthorized-principal, and invalid-minimum-actions edges without
  fabricating success.
- Phase 16 FSV reads a real decision-register JSON file plus physical
  SQLite/Git/enterprise/score/privacy/release-gate/package rows, then opens
  the physical SQLite ledger to prove `risk_assessment` and
  `enterprise_access_decision` rows exist only where expected. It reads
  dashboard `api/risk-assessments.json`, `api/summary.json`, and
  `api/errors.json` directly from disk, and checks blocked posture, accepted
  posture, unauthorized principal, and invalid-decision-register edges without
  fabricating success.

For hosted systems unavailable in CI, run those tests as gated integration
tests with a `BBX_LIVE_GITHUB_TOKEN` env var; keep all local source-of-truth
tests **mandatory** (gating only the live ones is a recipe for green-on-broken).

## Synthetic Data Strategy

For each phase, synthesize input where you control the ground truth, then
read the source of truth and compare. Phase 1 inputs/outputs are enumerated
in `20_phase1_state_and_invariants.md` §"Happy Path", §"Edge Case 1/2/3".
Phase 2 inputs/outputs are enumerated in
`21_phase2_state_and_invariants.md`. Phase 4/5/6/7/8/9/10/11/12/13/14/15/16 inputs/outputs are
enumerated in `23_phase4_state_and_invariants.md`,
`24_phase5_state_and_invariants.md`,
`25_phase6_state_and_invariants.md`, and
`26_phase7_state_and_invariants.md`, and
`27_phase8_state_and_invariants.md`, and
`28_phase9_state_and_invariants.md`, and
`29_phase10_state_and_invariants.md`, and
`30_phase11_state_and_invariants.md`, and
`31_phase12_state_and_invariants.md`, and
`32_phase13_state_and_invariants.md`,
`33_phase14_state_and_invariants.md`,
`34_phase15_state_and_invariants.md`, and
`35_phase16_state_and_invariants.md`.

A synthetic test must include:

1. The exact command(s) issued.
2. The exact JSON file(s) supplied as input.
3. The expected stdout JSON shape (success) **or** stderr JSON shape
   (failure with `error_code`).
4. The expected `sqlite3 …` query and its expected output.
5. The expected exit code (`0`, `1`, or `2`).

If any of these is missing, the test is incomplete.

## Regression Evals

Every production incident and every new verifier edge case produces:

- a ledger fixture (a deterministic SQLite file under `tests/fixtures/`);
- an episode row;
- an eval case;
- expected verifier status;
- expected remediation.

A regression that ships without all five is not a regression test; it is a
note.

## Tying It Together

The shortest valid sentence describing acceptance: **"When I run
`cargo test --package context-graph-cli --test blackbox_phaseN_fsv` on a
fresh checkout against real systems, all tests pass and a manual run of
the documented commands produces byte-identical SQLite/JSON output."**
