# Phase 14 State And Invariants

Phase 14 ships release/promotion gate decisions. It turns the verified ledger
state from Phases 8, 12, and 13 into an executable approval or block decision.

## Shipped Surface

Command:

```bash
blackbox gate evaluate \
  --ledger PATH --tenant TENANT --run-id RUN_ID \
  --principal-id PRINCIPAL --environment ENV \
  [--min-trust-level TRUST] [--max-risk FLOAT] \
  [--require-policy] [--require-privacy] \
  --agent AGENT [--model MODEL]
```

Connector:

- `blackbox_release_gate_v1`

Ledger record kind:

- `release_gate`

Phase 14 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 8`. The
physical SQLite table shape is unchanged, but old schema-version ledgers are
wiped on `blackbox init`; there is no migration shim. Phase 15 later bumped the
live schema to `LEDGER_SCHEMA_VERSION = 9` for pilot-package rows, and Phase
17 later bumps the current live schema to `LEDGER_SCHEMA_VERSION = 11` for
HTTP request-audit rows.

## Source Of Truth

The source of truth is the physical SQLite ledger:

```text
sqlite:PATH#ledger_records
```

Before a gate row is appended, the command:

1. Opens the initialized SQLite ledger.
2. Verifies table shape, metadata, append-only triggers, global hash chain, and
   per-run hash chain.
3. Requires the parent `agent_run` row to exist for the requested tenant.
4. Reads the latest `enterprise_tenant_config` and
   `enterprise_directory_snapshot`.
5. Appends and reads back an `enterprise_access_decision` for
   `release.approve`.
6. Fails with `RELEASE_GATE_RBAC_DENIED` if the principal lacks permission; no
   `release_gate` row is written.
7. Reads the latest physical `scorecard` row for the run and checks trust and
   risk thresholds.
8. Verifies policy, privacy, incident, verifier, and evidence-reference
   invariants from physical ledger rows.
9. Appends one `release_gate` row for approved or blocked decisions.
10. Reads the appended gate row back by sequence, `release_gate_id`, `run_id`,
    `payload_sha256`, and `record_sha256`.

## Invariants

- Missing parent run fails as `RELEASE_GATE_RUN_NOT_FOUND` before
  authorization or gate rows are appended.
- Invalid environment, trust level, or risk threshold fails as
  `RELEASE_GATE_REQUEST_INVALID` or `RELEASE_GATE_THRESHOLD_INVALID` before
  any new row is appended.
- Unauthorized principals produce a durable `enterprise_access_decision`
  denial and then fail as `RELEASE_GATE_RBAC_DENIED`; no `release_gate` row is
  appended.
- Missing scorecards append a blocked `release_gate` row with
  `RELEASE_GATE_SCORECARD_REQUIRED`.
- Scorecard trust below `--min-trust-level` appends a blocked row with
  `RELEASE_GATE_TRUST_TOO_LOW`.
- Scorecard risk above `--max-risk` appends a blocked row with
  `RELEASE_GATE_RISK_TOO_HIGH`.
- `--require-policy` requires policy compliance of `allowed` or `approved`.
- `--require-privacy` requires a retained privacy decision and blocks runs
  with approved erasure decisions.
- Failed or blocked verifier rows, incident packets, or broken evidence packet
  references block promotion.
- A blocked gate is a durable decision and exits non-zero after the row is
  read back; invalid requests and RBAC denials create no gate row.

## Error Codes

| Code | Meaning |
|---|---|
| `RELEASE_GATE_RUN_NOT_FOUND` | requested run has no physical `agent_run` row |
| `RELEASE_GATE_RBAC_DENIED` | principal lacks `release.approve` |
| `RELEASE_GATE_REQUEST_INVALID` | request field is empty, padded, too long, or unsupported |
| `RELEASE_GATE_THRESHOLD_INVALID` | risk threshold is outside `[0,1]` |
| `RELEASE_GATE_SCORECARD_REQUIRED` | latest physical `scorecard` row is absent |
| `RELEASE_GATE_TRUST_TOO_LOW` | latest scorecard trust is below the required level |
| `RELEASE_GATE_RISK_TOO_HIGH` | latest scorecard risk exceeds the configured maximum |
| `RELEASE_GATE_POLICY_REQUIRED` | required policy approval/allowance is absent |
| `RELEASE_GATE_PRIVACY_REQUIRED` | required retained privacy decision is absent |
| `RELEASE_GATE_PRIVACY_ERASURE_APPROVED` | approved erasure exists for the run |
| `RELEASE_GATE_VERIFIER_NOT_CLEAN` | failed or blocked verifier rows exist |
| `RELEASE_GATE_INCIDENT_OPEN` | incident evidence exists for the run |
| `RELEASE_GATE_EVIDENCE_REF_INVALID` | evidence packet references do not match physical rows |
| `RELEASE_GATE_READBACK_MISMATCH` | appended gate row could not be read back by id/hash |
| `RELEASE_GATE_BLOCKED` | umbrella non-zero error after a blocked gate row is persisted |

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase14_release_gate_fsv -- --nocapture
```

The test uses a real SQLite ledger, real enterprise config JSON, real SCIM
directory JSON, a real local Git repository, and real CLI commands. It
verifies:

- approved gate persists one `release_gate` with decision status `approved`;
- missing scorecard persists a blocked gate with
  `RELEASE_GATE_SCORECARD_REQUIRED`;
- too-low `--max-risk` persists a blocked gate with
  `RELEASE_GATE_RISK_TOO_HIGH`;
- unauthorized principal creates no `release_gate` row and does create a
  denied `enterprise_access_decision`;
- incident-bearing run persists a blocked gate;
- invalid max-risk input creates no new ledger rows;
- connector test reads the real persisted `release_gate` row count.

Final automated physical counts:

```text
agent_run=4
enterprise_access_decision=8
enterprise_directory_snapshot=1
enterprise_tenant_config=1
evidence_packet=3
expected_delta=3
incident_packet=1
policy_decision=3
readback_snapshot=3
release_gate=4
retention_decision=3
scorecard=3
tool_call=3
verifier_result=3
```

## Manual FSV

Manual FSV must print before/after source-of-truth state for the happy path and
at least three edge cases. The source-of-truth query is direct SQLite, not
command stdout.

Completed manual FSV:

- log: `/tmp/bbx-phase14-manual-fsv.JAJAYu/fsv.log`
- log SHA-256:
  `e22a6ee2996cce9757db8401bd298a01a1ea693f5b12204493f6c7379d39ca0e`
- final ledger integrity: `schema_version=8`, `record_count=33`,
  `status=passed`
- final ledger tip:
  `sha256:9f4c0a5877cf56ee9401621f5a5860f6ef6ec452fbf1ca4a646cb8f8f0c61657`
- final physical counts: `agent_run=3`, `enterprise_access_decision=7`,
  `enterprise_directory_snapshot=1`, `enterprise_tenant_config=1`,
  `evidence_packet=2`, `expected_delta=2`, `incident_packet=1`,
  `policy_decision=2`, `readback_snapshot=2`, `release_gate=4`,
  `retention_decision=2`, `scorecard=2`, `tool_call=2`,
  `verifier_result=2`
- final release gate rows:

```text
14|run_adfe20f23678429f896474ae0bf26ed4|approved|RELEASE_GATE_APPROVED|
17|run_65e6869f7a7141ddbfbd5baeea97c0cc|blocked|RELEASE_GATE_BLOCKED|RELEASE_GATE_SCORECARD_REQUIRED
19|run_adfe20f23678429f896474ae0bf26ed4|blocked|RELEASE_GATE_BLOCKED|RELEASE_GATE_RISK_TOO_HIGH
33|run_de485dc4e6bf4933905108932e6e32b1|blocked|RELEASE_GATE_BLOCKED|RELEASE_GATE_TRUST_TOO_LOW
```

Required checks:

```bash
sqlite3 "$ledger" \
  "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind;"

sqlite3 "$ledger" \
  "SELECT sequence, run_id, json_extract(payload_json,'$.decision.status'), json_extract(payload_json,'$.decision.code'), json_extract(payload_json,'$.decision.primary_reason_code') FROM ledger_records WHERE record_kind='release_gate' ORDER BY sequence;"

sqlite3 "$ledger" \
  "SELECT key, value FROM blackbox_metadata WHERE key IN ('schema_version','record_count','ledger_tip_sha256') ORDER BY key;"
```

The metadata record count and tip hash must match `blackbox inspect ledger`.
If the direct SQLite rows do not prove the gate decision, Phase 14 is
incomplete.

## Research Basis

Phase 14 follows:

- NIST AI RMF: gate readiness is a governed, measured, managed decision over
  explicit evidence.
- NIST SP 800-218 Secure Software Development Framework: release decisions
  should be tied to reviewable verification evidence and traceable practices.
- SLSA provenance: consumers need to know exactly which evidence state was
  approved or blocked.
