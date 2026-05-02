# First Wedge Demo

The first demo should prove the product in a workflow where the source of truth
is concrete and the business value is obvious.

## Demo Thesis

An AI agent remediates a dependency vulnerability. BlackBoxFix records every
action, verifies the resulting state in Git, GitHub, CI, scanner output, and the
ticketing system, then exports verified and failed episodes for evals.

## Workflow

1. Security ticket exists for a vulnerable dependency.
2. Agent inspects repository and vulnerability details.
3. Agent edits dependency files.
4. Agent runs tests and scanner.
5. Agent creates a PR.
6. CI runs.
7. Scanner confirms vulnerability is gone.
8. Ticket is updated with evidence.
9. BlackBoxFix exports an evidence packet and episodes.

## Sources Of Truth

- Git local repository: file changes, commit, branch.
- GitHub: PR, branch head, checks, issue link.
- CI: workflow status, test artifact.
- Scanner: vulnerability report.
- Ticketing: issue status and evidence link.
- Ledger: BlackBoxFix records and evidence packet hashes.

## Happy Path Expected Deltas

```json
{
  "git": {
    "branch_exists": true,
    "commit_count_delta": 1,
    "lockfile_changed": true
  },
  "scanner": {
    "vulnerability_present": false
  },
  "github": {
    "pull_request_state": "open",
    "head_matches_commit": true,
    "checks_passed": true
  },
  "ticket": {
    "status": "ready_for_review",
    "evidence_link_present": true
  }
}
```

## Required Failure Demos

### Failure 1: CI Fails

Expected:

- verifier status `failed`;
- error code `CI_REQUIRED_CHECK_FAILED`;
- readback shows failed job;
- incident packet created;
- episode exported with remediation.

### Failure 2: Scanner Still Finds Vulnerability

Expected:

- verifier status `failed`;
- error code `VULNERABILITY_STILL_PRESENT`;
- scanner report stored as evidence;
- ticket is not marked resolved.

### Failure 3: Missing GitHub Permission

Expected:

- verifier status `blocked`;
- error code `CONNECTOR_PERMISSION_DENIED`;
- no success claim;
- remediation asks for required read scope.

## Demo Commands

Target CLI shape:

```text
context-graph blackbox init --ledger .blackbox/ledger
context-graph blackbox record run --task "remediate CVE-2026-0001"
context-graph blackbox verify --run-id RUN
context-graph blackbox replay --run-id RUN
context-graph blackbox export evidence --run-id RUN --out evidence.json
context-graph blackbox export episodes --run-id RUN --out episodes.jsonl
```

## Evidence To Show Buyer

- timeline of every action;
- before/after source-of-truth state;
- verifier pass/fail result;
- incident packet for controlled failure;
- JSONL episode export;
- eval generated from failure;
- policy mode and approval status.

## Success Criteria

- The demo proves at least one happy path and three edge cases.
- The evidence packet is readable without trusting the agent transcript.
- The JSONL export includes passed, failed, and blocked examples.
- A buyer can explain how this would unblock an internal agent pilot.

