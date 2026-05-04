# Phase 22 — Ticketing Artifact State And Invariants

Phase 22 ships `ticketing_artifact_v1`, the ticketing source-of-truth
connector. It verifies ticket state claims by reading physical Jira REST,
Linear GraphQL, and ServiceNow Table API JSON artifacts plus the SQLite ledger
rows written by `blackbox verify`.

## Shipped Surface

- Connector: `ticketing_artifact_v1`.
- Connector test:
  `blackbox connector test --connector ticketing_artifact_v1 --source jira-issue.json`.
- Verifiers:
  - `jira_ticket_state_v1`
  - `linear_ticket_state_v1`
  - `servicenow_ticket_state_v1`
- Ledger record kinds reused:
  - `expected_delta`
  - `readback_snapshot`
  - `verifier_result`
  - `evidence_packet`
  - `incident_packet`

Phase 22 adds no new record kind and does not bump `LEDGER_SCHEMA_VERSION`;
current ledgers remain version `13`.

## Sources Of Truth

- Jira: `ticketing:/path/to/jira-issue.json#ticket`, where the file is a Jira
  REST API issue or search JSON response.
- Linear: `ticketing:/path/to/linear-issue.json#ticket`, where the file is a
  Linear GraphQL issue response.
- ServiceNow: `ticketing:/path/to/servicenow-record.json#ticket`, where the
  file is a Table API record/list response.
- Ledger: `sqlite:/path/to/blackbox.sqlite#ledger_records`.

Return values are not evidence. Successful verification requires a separate
file readback plus a direct SQLite read proving rows were appended.

## Invariants

- The source format must be explicit:
  `jira-rest-api-issue-json`, `linear-graphql-issue-json`, or
  `servicenow-table-api-record-json`.
- Jira verification only passes when the exact `issue_key` exists and
  `fields.status.name` equals the expected `status`.
- Linear verification only passes when the exact `issue_identifier` exists and
  `state.name` equals the expected `state`.
- ServiceNow verification only passes when the exact `number` exists and
  `state` or `incident_state` equals the expected `state`.
- If `expected.evidence_url` is supplied, the exact URL must appear in the
  captured API artifact.
- Missing artifacts, missing tickets, state mismatches, and missing evidence
  URLs are `failed` verifier results and create `incident_packet` rows.
- Invalid JSON, unsupported source formats, duplicate ticket identities, and
  provider schema drift are `blocked` verifier results and do not create
  incident rows.

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase22_ticketing_fsv -- --nocapture
```

The test writes real Jira, Linear, and ServiceNow-shaped JSON files. It runs
the real CLI and reads SQLite directly.

Expected final ledger counts:

```text
agent_run=1
tool_call=1
expected_delta=6
readback_snapshot=6
verifier_result=6
evidence_packet=6
incident_packet=2
```

Covered paths:

- happy Jira issue state and evidence URL readback;
- happy Linear issue state and evidence URL readback;
- happy ServiceNow record state and evidence URL readback;
- missing Jira ticket (`TICKETING_TICKET_NOT_FOUND`);
- wrong Linear state (`TICKETING_TICKET_STATUS_MISMATCH`);
- invalid ServiceNow JSON (`TICKETING_ARTIFACT_JSON_INVALID`).

## Manual FSV Checklist

1. Create a fresh ledger with `blackbox init`.
2. Write physical Jira, Linear, and ServiceNow API JSON artifacts.
3. Record a run and tool call.
4. Run all three happy verifiers.
5. Query `ledger_records` directly and confirm three
   `expected_delta/readback_snapshot/verifier_result/evidence_packet` rows and
   zero incidents.
6. Run the three edge cases.
7. Query `ledger_records` directly and confirm six verifier/evidence rows and
   two incidents.
8. Read the latest verifier payloads and confirm the expected error codes,
   ticket keys, actual statuses, evidence links, and artifact hashes are
   physically present.
