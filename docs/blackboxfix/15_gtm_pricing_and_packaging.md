# Go-To-Market, Pricing, And Packaging

## Category

Position BlackBoxFix as **Verified AgentOps**: operational proof for AI agent
actions, where every shipped claim has an expected delta and a physical
readback.

## Initial Wedge

DevOps/SecOps agent governance:

- coding agents;
- dependency remediation agents;
- CI repair agents;
- vulnerability triage agents;
- ticket/evidence agents.

This wedge is concrete, budgeted, and verifiable with the shipped local CLI.

## Buyer Personas

| Persona | Cares about |
|---|---|
| AI platform lead | reusable controls for many agent teams; APIs; eval exports; model-provider neutrality |
| CISO / security architect | authorization, audit, incident response, SIEM, data controls |
| Engineering leader | shipping agents without breaking production |
| Agent vendor | enterprise trust, embedded SDK, customer proof |

## Shipped Pilot Package

Phase 15 turns the GTM claim into an executable artifact:

```bash
blackbox package pilot-report \
  --ledger PATH \
  --tenant TENANT \
  --principal-id PRINCIPAL_WITH_COMMERCIAL_PACKAGE \
  --pilot-id PILOT_ID \
  --min-actions 20 \
  --out pilot-package \
  --agent commercial_operator
```

Source of truth:

- SQLite `ledger_records` row with `record_kind='pilot_package'`;
- package directory files `pilot-report.json`, `usage-meter-events.jsonl`,
  `eval-candidates.jsonl`, and `manifest.json`;
- dashboard read model `api/pilot-packages.json`.

The command is fail-closed. Invalid request fields create no rows/files.
Unauthorized principals create only the denied `enterprise_access_decision`.
Unmet success criteria still create durable package evidence, then return
`PILOT_PACKAGE_CRITERIA_NOT_MET`.

## Pilot Success Metric

A package passes only when all criteria are true:

- observed `agent_run` rows are at least `--min-actions`;
- every mutating `tool_call` has an `evidence_packet`;
- at least one failure/block/incident/gate block becomes an eval candidate;
- at least one `release_gate` decision exists.

The output report stores both the summary and the exact failed criteria.

## Pricing Meter Rows

`usage-meter-events.jsonl` emits independently parseable rows with:

- `event_name`;
- `customer_identifier`;
- numeric `value`;
- source timestamp;
- stable idempotency `identifier`;
- low-cardinality dimensions;
- ledger source ref.

Initial meter names:

- `blackbox_agent_run`;
- `blackbox_verified_tool_call`;
- `blackbox_evidence_packet`;
- `blackbox_incident_packet`;
- `blackbox_release_gate`.

These are package artifacts for pricing analysis, not live billing calls.

## Pricing Hypotheses

| Tier | Mechanism |
|---|---|
| Developer | free or low-cost local recorder; JSONL exporter; local Git/file verifiers |
| Team | per active agent or verified run volume; shared dashboard; GitHub/CI/ticket/scanner integrations |
| Enterprise | annual platform fee plus usage tier for verified tool calls; private deployment; retention and compliance packages |
| Vendor / OEM | SDK license; per end-customer deployment; white-label enterprise controls |

## Sales Proof

The demo must show physical evidence:

1. Run a real agent workflow.
2. Verify source-of-truth state.
3. Score the run.
4. Evaluate privacy/release gates.
5. Build the pilot package.
6. Query SQLite for the `pilot_package` row.
7. Read `manifest.json`, `usage-meter-events.jsonl`, and
   `eval-candidates.jsonl` from disk.

Screenshots are secondary. The artifact is accepted only when the buyer can
run the commands and inspect the same rows/files.

## FSV Of GTM Claims

A GTM claim is FSV-passing only if it has:

1. A reproducible CLI command sequence.
2. A direct SQLite query proving the ledger row.
3. Direct file reads proving package outputs and manifest hashes.
4. A test path under `crates/context-graph-cli/tests/`.

If any of those four are missing, the claim is unverified and must not appear
in collateral.
