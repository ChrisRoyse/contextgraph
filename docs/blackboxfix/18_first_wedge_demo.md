# First Wedge Demo

The first demo proves the product in a workflow where the source of truth is
concrete and the business value is obvious.

The CLI demo is now viable through **Phase 4**: Git + GitHub + CI verifiers,
evidence packets, forensic replay, and evidence export. Phase 5 will add the
episode exporter.

## Demo Thesis

An AI agent remediates a dependency vulnerability. BlackBoxFix records every
action, verifies the resulting state in Git, GitHub, CI, scanner output, and
the ticketing system, then exports verified and failed episodes for evals.

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

| Layer | System | Read path |
|---|---|---|
| Git | local repository | `git log`, `git diff`, `git rev-parse`, `git status` |
| GitHub | API | `GET /repos/.../pulls/N`, `GET /repos/.../check-suites` |
| CI | GitHub Actions API | `GET /repos/.../actions/runs/M` |
| Scanner | OSV-Scanner / `cargo audit` | scanner CLI output + lockfile read |
| Ticketing | Jira/Linear/GitHub Issues API | issue state + linked PR |
| Ledger | BlackBoxFix SQLite | `sqlite3 <ledger> "SELECT … FROM ledger_records …"` |

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

### Failure 1 — CI Fails

Expected:

- verifier status `failed`;
- error code `CI_REQUIRED_CHECK_FAILED`;
- readback shows failed job;
- incident packet created;
- episode exported with remediation.

### Failure 2 — Scanner Still Finds Vulnerability

Expected:

- verifier status `failed`;
- error code `VULNERABILITY_STILL_PRESENT`;
- scanner report stored as evidence;
- ticket is not marked resolved.

### Failure 3 — Missing GitHub Permission

Expected:

- verifier status `blocked`;
- error code `CONNECTOR_PERMISSION_DENIED`;
- no success claim;
- remediation asks for required read scope.

## Demo Commands (Phase 1 today)

The Phase 1 commands work end-to-end against synthetic input. Show this
during early sales conversations to demonstrate the recorder and hash chain;
swap to the full Phase 3 workflow once verifiers exist.

```bash
ledger=/tmp/bbx-demo.sqlite
context-graph blackbox init --ledger "$ledger"
RUN=$(context-graph blackbox record run \
        --ledger "$ledger" \
        --task "remediate CVE-2026-0001" \
        --agent demo_agent \
        --tenant tenant_demo \
        --risk-tier high | jq -r .run_id)

cat > /tmp/tool-input.json <<'JSON'
{
  "repo": "org/service",
  "branch": "agent/fix-cve-2026-0001",
  "title": "Fix CVE-2026-0001",
  "github_token": "ghp_REPLACE_ME"
}
JSON

context-graph blackbox record tool-call \
  --ledger "$ledger" \
  --run-id "$RUN" \
  --tool github_create_pull_request \
  --capability github.pull_request.write \
  --input /tmp/tool-input.json

context-graph blackbox inspect ledger --ledger "$ledger" --run-id "$RUN"

# Source-of-truth proof:
sqlite3 "$ledger" \
  "SELECT sequence, record_kind, run_id, payload_sha256 FROM ledger_records;"
sqlite3 "$ledger" \
  "SELECT json_extract(redaction_json, '\$.redacted_fields') FROM ledger_records WHERE record_kind='tool_call';"
# Must contain '$.github_token' — the redaction policy auto-detected the
# secret-named field and replaced its value with [REDACTED:sha256:...].
```

## Demo Commands (Phase 4 shipped surface)

```bash
ledger=/tmp/bbx-demo.sqlite
context-graph blackbox connector test --connector github_v1 --source org/service
context-graph blackbox verify --ledger "$ledger" --input verify-spec.json
context-graph blackbox replay --ledger "$ledger" --run-id "$RUN"
context-graph blackbox export evidence --ledger "$ledger" --run-id "$RUN" --out evidence.json
```

`blackbox export episodes` is Phase 5; keep it in the demo script only after
the Phase 5 exporter ships.

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

## FSV Of The Demo

Treat the demo itself as a verifier:

1. Capture the SQLite snapshot before the demo (`sha256sum
   "$ledger"` ⇒ pre-hash).
2. Run the script.
3. Capture the SQLite snapshot after (`sha256sum "$ledger"` ⇒ post-hash).
4. Confirm `ledger_record_count` increased by exactly the expected amount.
5. Confirm every `tool_call` row has `redaction_json.redacted_fields` covering
   any sensitive-named field in the input.
6. Confirm `inspect ledger` exit code is `0` and `integrity.status="passed"`.

If any of these checks fails, do **not** show the demo to a buyer until the
discrepancy is rooted out.
