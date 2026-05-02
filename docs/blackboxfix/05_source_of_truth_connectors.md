# Source-Of-Truth Connectors

Connectors define how BlackBoxFix reads and verifies real systems. A connector
is not just a tool wrapper. It must provide independent readback and explicit
evidence extraction.

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

```json
{
  "connector_id": "github_v1",
  "source_kind": "github",
  "capabilities": [
    "repo.read",
    "pull_request.read",
    "pull_request.write",
    "checks.read"
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

- Readback must use the source system, not the agent's return payload.
- Readback must be separately invocable for manual FSV.
- Mutating calls must produce correlation IDs.
- Connector errors must be typed.
- Missing permission must be `blocked`, not `passed`.
- Rate-limit exhaustion must be visible in the ledger.
- Data redaction must happen before persistence.

## Priority Connectors

### Git Local

Source of truth:

- `.git` object database;
- working tree status;
- commit graph;
- diff output;
- tags and refs.

Verifier examples:

- branch exists;
- commit exists;
- file diff matches expected patch;
- working tree is clean;
- tests touched expected files only.

### GitHub

Source of truth:

- repository metadata;
- pull requests;
- branches;
- commits;
- checks;
- reviews;
- issues;
- deployments.

Verifier examples:

- PR exists and points at expected branch;
- expected commit is present;
- CI check suite passed;
- vulnerability issue changed state;
- review approval exists.

### CI Providers

Initial targets:

- GitHub Actions;
- Buildkite;
- CircleCI;
- GitLab CI.

Verifier examples:

- workflow run completed;
- required jobs passed;
- artifact exists and hash matches;
- test report contains expected count and no failures.

### Package And Vulnerability Scanners

Initial targets:

- OSV Scanner;
- npm audit;
- cargo audit;
- Trivy;
- Snyk if customer uses it.

Verifier examples:

- vulnerability no longer appears;
- lockfile changed expected package version;
- SBOM artifact exists;
- scanner report hash matches evidence.

### Kubernetes

Source of truth:

- Kubernetes API server;
- resource version;
- deployment status;
- pod health;
- events.

Verifier examples:

- deployment image changed to expected digest;
- rollout completed;
- desired and available replicas match;
- no new crash loops.

### Terraform And Cloud

Source of truth:

- Terraform plan output;
- Terraform state;
- cloud resource API;
- cloud audit logs.

Verifier examples:

- plan contains only approved changes;
- state resource matches expected attributes;
- cloud API confirms resource exists;
- audit log shows principal and action.

### Ticketing

Initial targets:

- Jira;
- Linear;
- ServiceNow;
- GitHub Issues.

Verifier examples:

- ticket state changed;
- linked PR exists;
- incident tag added;
- remediation note contains evidence packet link.

## Connector Lifecycle

1. Define connector spec.
2. Implement read-only readback.
3. Implement mutating capability, if needed.
4. Implement verifier specs.
5. Build happy path FSV.
6. Build three edge-case FSV tests.
7. Add redaction tests.
8. Add SIEM/audit export mapping.

## First Connector Set

For the first product demo, implement:

- local Git;
- GitHub pull requests/checks/issues;
- local test command verifier;
- OSV or package audit scanner;
- JSONL evidence exporter.

This creates a credible DevSecOps agent governance workflow with inspectable
state and low connector sprawl.

