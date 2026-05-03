# Phase 16 State And Invariants

Phase 16 ships executable risk and open-question closure. It turns the
previous prose-only risk register into an RBAC-gated, append-only
`risk_assessment` row that is accepted or blocked by physical source-of-truth
evidence.

## Shipped Surface

Command:

```bash
blackbox risk evaluate \
  --ledger PATH --tenant TENANT --principal-id PRINCIPAL \
  --decisions DECISIONS_JSON --agent AGENT [--model MODEL]
```

Connector:

- `blackbox_risk_v1`

Ledger record kind:

- `risk_assessment`

Phase 16 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 10`. The
physical SQLite table shape is unchanged, but old schema-version ledgers are
wiped on `blackbox init`; there is no migration shim. Phase 18 later bumps the
current live schema to `LEDGER_SCHEMA_VERSION = 12` for first-wedge demo
rows.

## Source Of Truth

Primary source of truth:

- SQLite `ledger_records` rows in the user-provided ledger;
- SQLite runtime PRAGMAs: `journal_mode`, `synchronous`, and `busy_timeout`;
- real decision-register JSON file passed through `--decisions`;
- dashboard risk API file `api/risk-assessments.json`.

Before a risk row is appended, the command:

1. Validates ledger, tenant, principal, agent, and decision file path fields.
2. Reads and validates the decision-register JSON file.
3. Requires exactly one closed decision for every required Phase 16 id.
4. Opens the initialized SQLite ledger.
5. Verifies table shape, metadata, append-only triggers, global hash chain,
   and per-run hash chains.
6. Persists and reads back an `enterprise_access_decision` for `risk.review`.
7. Fails as `RISK_ASSESSMENT_RBAC_DENIED` if the principal lacks permission;
   no risk row is created.
8. Evaluates ledger integrity, SQLite runtime durability, secret findings,
   enterprise controls, source-of-truth coverage, verifier/policy status,
   release gates, pilot packages, incident-to-eval conversion, and decision
   closure.
9. Appends one `risk_assessment` row with `decision.status=accepted` or
   `decision.status=blocked`.
10. Reads the appended row back by sequence, `risk_assessment_id`,
    `payload_sha256`, and `record_sha256`.

## Required Decision Ids

The decision register must contain these ids exactly once:

- `next_hosted_ci_connector`
- `paid_pilot_dashboard_minimum`
- `raw_model_text_storage_default`
- `customer_managed_key_path`
- `eval_export_format_after_jsonl`

Each decision must include `status`, `severity`, `owner`, `decision`,
`rationale`, `review_due`, `review_trigger`, and non-empty `evidence`.
Accepted statuses are `resolved`, `accepted`, and `deferred`; `open` fails
closed. `review_due` must be `YYYY-MM-DD` and not earlier than the current
date.

## Invariants

- Accepted risk posture requires all risk checks to pass.
- Blocked posture is persisted as durable evidence, then exits non-zero with
  `RISK_ASSESSMENT_BLOCKED`.
- Invalid decision-register JSON creates no rows.
- Unauthorized principals persist only a denied `enterprise_access_decision`
  and create no `risk_assessment`.
- Secret-like literals in decision text or source ledger payloads block risk
  acceptance.
- SQLite runtime must be WAL, `synchronous=FULL`, and
  `busy_timeout >= 5000`.
- Every mutating tool call must have verifier/evidence coverage.
- Blocked verifier rows and denied policy rows block readiness.
- Failed verifier rows are acceptable only when incident evidence has been
  converted into eval/package candidate evidence.
- A passed pilot package and at least one release gate must exist before final
  accepted posture.

## Error Codes

| Code | Meaning |
|---|---|
| `RISK_REQUEST_INVALID` | ledger, tenant, principal, agent, or decision path is invalid |
| `RISK_DECISIONS_INVALID` | decision register is missing, malformed, open, duplicate, expired, or contains unknown ids/fields |
| `RISK_DECISIONS_SECRET_DETECTED` | decision register contains secret-like text |
| `RISK_ASSESSMENT_RBAC_DENIED` | principal lacks `risk.review` |
| `RISK_ASSESSMENT_SCHEMA_INVALID` | a source row or generated risk row is malformed |
| `RISK_ASSESSMENT_READBACK_MISMATCH` | appended risk row could not be read back by id/hash |
| `RISK_ASSESSMENT_BLOCKED` | durable risk row exists, but one or more readiness checks failed |

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase16_risk_register_fsv -- --nocapture
```

The test uses a real SQLite ledger, real enterprise config JSON, real SCIM
directory JSON, a real local Git repository, real package files, a real
decision-register JSON file, and real CLI commands. It verifies:

- pre-package risk review persists a blocked `risk_assessment`;
- post-package risk review persists an accepted `risk_assessment`;
- unauthorized principal creates no risk row and does persist a denied access
  decision;
- invalid empty decisions create no rows;
- connector test reads the physical `risk_assessment` count;
- dashboard build reads physical risk rows into `api/risk-assessments.json`
  and summary counts.

## Manual FSV Checklist

For manual verification, print before/after direct SQLite counts for each
case:

```sql
SELECT record_kind, COUNT(*)
FROM ledger_records
GROUP BY record_kind
ORDER BY record_kind;
```

Then inspect the latest risk row:

```sql
SELECT sequence,
       json_extract(payload_json, '$.risk_assessment_id'),
       json_extract(payload_json, '$.decision.status'),
       json_extract(payload_json, '$.decision.reason_code'),
       json_extract(payload_json, '$.risk_summary.checks_failed')
FROM ledger_records
WHERE record_kind='risk_assessment'
ORDER BY sequence DESC
LIMIT 1;
```

Dashboard file proof must include:

```bash
sha256sum dashboard/api/risk-assessments.json
sha256sum dashboard/api/summary.json
sha256sum dashboard/api/errors.json
```

The expected happy-path final state contains two real `agent_run` rows, two
verified `tool_call` rows, at least one failed/incident eval source, one
passed `pilot_package` row, and one accepted `risk_assessment` row. Edge cases
must include blocked pre-package posture, unauthorized principal, and invalid
decision register.

## Manual FSV Evidence 2026-05-03

Source of truth:

- `sqlite:/tmp/bbx-phase16-manual-fsv.5X5zkB/blackbox.sqlite#ledger_records`
- decision register:
  `/tmp/bbx-phase16-manual-fsv.5X5zkB/phase16-risk-decisions.json`
- package files under `/tmp/bbx-phase16-manual-fsv.5X5zkB/pilot-package`
- dashboard files under `/tmp/bbx-phase16-manual-fsv.5X5zkB/dashboard`
- log: `/tmp/bbx-phase16-manual-fsv.5X5zkB/fsv.log`
- log SHA-256:
  `14d33b9c3eb4085881087e64a80591405d25b1cc5a32aad67cc92a767a3cde07`

Happy path direct readback:

- blocked pre-package assessment: before `risk_assessment=0`,
  `enterprise_access_decision=2`; after `risk_assessment=1`,
  `enterprise_access_decision=3`; latest row
  `sequence=24|risk_assessment_5f58fe9e57404ddb8ffe199d485f3d25|blocked|RISK_ASSESSMENT_BLOCKED|0`
- accepted post-package assessment: before `risk_assessment=1`,
  `enterprise_access_decision=4`; after `risk_assessment=2`,
  `enterprise_access_decision=5`; latest row
  `sequence=28|risk_assessment_7050223230b749339b117abb27128268|accepted|RISK_ASSESSMENT_ACCEPTED|2`
- accepted row had `decision.failed_checks=[]`
- decision register SHA-256:
  `8a8bb03b407ddf89101692053936e2e41b00407ea2659f1c49c0d9d60e0328c9`

Boundary and edge case direct readback:

- blocked posture edge: command exited `RISK_ASSESSMENT_BLOCKED`; persisted
  failed checks were `RISK_PILOT_PACKAGE_MISSING` and
  `RISK_INCIDENTS_NOT_CONVERTED_TO_EVALS`
- unauthorized principal edge: before `risk_assessment=2`,
  `enterprise_access_decision=5`; after `risk_assessment=2`,
  `enterprise_access_decision=6`; command exited
  `RISK_ASSESSMENT_RBAC_DENIED`
- invalid decisions edge: before `total=29`, `risk_assessment=2`; after
  `total=29`, `risk_assessment=2`; command exited `RISK_DECISIONS_INVALID`

Package/dashboard file proof:

- `pilot-package/manifest.json`:
  `e1b9e2a1e1083638a0c529ed0444c35b335617a0f40a2d5d72b18d42a9095e1a`
- `pilot-package/pilot-report.json`:
  `3e983ca110c8c44799639040c1009de508d093e0db5843ddcc4c33f926a95846`
- `pilot-package/usage-meter-events.jsonl`:
  `da4bef9392faabde41469bbe27bab4da75951b1cd5edb2161336a58219d45e9b`
- `pilot-package/eval-candidates.jsonl`:
  `ac38ff8caa63b2ad60e7fe314ac6e07bf039071b659cea0e72abe7922f86fdb6`
- `dashboard/api/risk-assessments.json`:
  `e0078410c2e1ebd90688e4e9d75231e76c1b5178d35e1763d2c384c1b440c27d`
- `dashboard/api/summary.json`:
  `814560469723d3f9355d5c69a0018b0744cdcd4ddd0e6ca9c3600d03f32631ee`
- `dashboard/api/errors.json`:
  `e99d16a7d61c224bc8685deb828f59957a4e693097bd79fad1934e144fd8ba40`
- `dashboard/manifest.json`:
  `b208a3f40bf7b9a7880f22b360b7cb6b0278795d826ece901e53d5d71bc8a8e5`

Final physical ledger counts:

```text
agent_run=2
enterprise_access_decision=6
enterprise_directory_snapshot=1
enterprise_tenant_config=1
evidence_packet=2
expected_delta=2
incident_packet=1
pilot_package=1
policy_decision=2
readback_snapshot=2
release_gate=1
retention_decision=1
risk_assessment=2
scorecard=1
tool_call=2
verifier_result=2
```

Final metadata readback:

```text
schema_version=10
ledger_record_count=29
ledger_tip_sha256=sha256:68d31840c27eabad2878b820f14ade7e1263dea1afd3f09c8b9b45638122767d
inspect ledger integrity.status=passed
```
