# Phase 12 State And Invariants

Phase 12 ships deterministic run-level scoring. It turns verified ledger
evidence, policy state, incidents, and source-of-truth coverage into an
append-only `scorecard` row.

## Shipped Surface

Command:

```bash
blackbox score run --ledger PATH --run-id RUN_ID \
  [--tenant TENANT] [--agent AGENT] [--model MODEL]
```

Connector:

- `blackbox_scoring_v1`

Ledger record kind:

- `scorecard`

Phase 12 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 6` for
the new `scorecard` record kind. The physical SQLite table shape is unchanged,
but old schema-version ledgers are wiped on `blackbox init`; there is no
migration shim.

## Source Of Truth

The source of truth is the physical SQLite ledger:

```text
sqlite:PATH#ledger_records
```

Before scoring, the command:

1. Opens the initialized SQLite ledger.
2. Verifies the physical table schema, append-only triggers, metadata row
   count, metadata tip hash, per-record hash chain, and per-run hash chain.
3. Finds exactly one `agent_run` row for the requested `run_id` and tenant.
4. Verifies every existing `evidence_packet.record_refs[]` points to the
   physical ledger row with the expected `record_kind`, `payload_sha256`, and
   `record_sha256`.
5. Computes the score only from physical rows in that run, excluding previous
   `scorecard` rows.
6. Appends one `scorecard` row.
7. Performs a separate ledger readback and verifies the appended sequence,
   `scorecard_id`, `run_id`, `payload_sha256`, `record_sha256`, row count, and
   ledger tip.

## Scorecard Payload

`scorecard` rows contain:

- `scorecard_id`;
- `scorecard_schema_version = 1`;
- `score_rule_version = score_v1`;
- `source_ledger` with ledger id, schema version, pre-score row count, and
  pre-score tip hash;
- `score.correctness`;
- `score.verification_confidence`;
- `score.risk`;
- `score.policy_compliance`;
- `score.training_value`;
- `score.trust_level`;
- `score_inputs` with exact counts and ratios used by the score;
- `input_record_refs` for every non-scorecard run row consumed by scoring.

## Deterministic Rules

Correctness:

- any failed verifier => `0.0`;
- all tool calls evidenced, at least one verifier passed, and no blocked
  verifier => `1.0`;
- otherwise `null`.

Trust level:

- any incident, failed verifier, or denied policy => `incident`;
- full evidence coverage plus passed verifier plus allowed policy =>
  `verified_with_policy`;
- full evidence coverage plus passed verifier without policy =>
  `verified`;
- otherwise `recorded`.

Policy compliance:

- denied policy rows => `denied`;
- allowed policy row with approval id => `approved`;
- allowed policy row without approval id => `allowed`;
- no policy rows => `policy_missing`.

Risk:

- base: `low=0.20`, `medium=0.50`, `high=0.80`;
- +0.08 for tool calls;
- +0.10 when a high-risk run has no policy;
- +0.08 when tool calls lack complete evidence coverage;
- +0.10 for failed verifiers;
- +0.05 for blocked verifiers;
- +0.12 for incident packets;
- +0.10 for denied policy;
- capped to `[0.0, 1.0]`, rounded to 4 decimals.

Verification confidence:

- `0.0` when no verifier result exists;
- otherwise: connector reliability `* 0.55`, evidence coverage `* 0.25`,
  readback ratio `* 0.10`, evidence-ref verification `* 0.10`;
- capped to `[0.0, 1.0]`, rounded to 4 decimals.

Training value:

- base `0.05`;
- +0.15 for passed evidence;
- +0.10 for high-risk runs;
- +0.25 for failed verifiers;
- +0.18 for blocked verifiers;
- +0.25 for incident packets;
- +0.20 for denied policy;
- capped to `[0.0, 1.0]`, rounded to 4 decimals.

## Failure Codes

| Error code | Meaning |
|---|---|
| `SCORECARD_RUN_NOT_FOUND` | requested run id has no physical `agent_run` row |
| `SCORECARD_RECORD_SCHEMA_DRIFT` | an input ledger payload has an unsupported or malformed scoring field |
| `SCORECARD_EVIDENCE_WITHOUT_TOOL_CALL` | evidence references a tool call absent from the scored run |
| `SCORECARD_READBACK_MISMATCH` | appended scorecard row did not read back with the expected identity or hashes |

The command also inherits ledger integrity errors such as
`BBX_HASH_CHAIN_BROKEN`, tenant mismatch errors, and SQLite IO errors. It does
not guess missing values.

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase12_scorecard_fsv -- --nocapture
```

Current result: 1/1 passing. The test uses the real CLI, a real SQLite ledger,
and a real local Git repository. It covers:

- happy path: policy-allowed, Git-clean verifier passed, `scorecard` trust
  `verified_with_policy`, correctness `1.0`, risk `0.58`, confidence
  `0.9725`;
- edge 1: run with no tool calls, trust `recorded`, correctness `null`;
- edge 2: high-risk unverified tool call, trust `recorded`, risk `1.0`,
  evidence coverage `0.0`;
- edge 3: failed Git verifier, incident packet present, trust `incident`,
  correctness `0.0`, risk `0.5`;
- edge 4: missing run id returns `SCORECARD_RUN_NOT_FOUND` and writes no
  scorecard row.

Final automated physical SQLite counts:

```json
{
  "agent_run": 4,
  "policy_decision": 1,
  "tool_call": 3,
  "expected_delta": 2,
  "readback_snapshot": 2,
  "verifier_result": 2,
  "evidence_packet": 2,
  "incident_packet": 1,
  "scorecard": 4
}
```

## Manual FSV

Manual FSV completed against:

```text
/tmp/bbx-phase12-manual-fsv.1VNO0R/fsv.log
```

Log SHA-256:

```text
b8da7e8aca9aa7af7b8ff6571bef790849b1d64a99add3d9ddc4c159d5c81958
```

Final physical SQLite counts:

```text
agent_run|4
evidence_packet|2
expected_delta|2
incident_packet|1
policy_decision|1
readback_snapshot|2
scorecard|4
tool_call|3
verifier_result|2
```

Final scorecard rows read directly from `ledger_records`:

```text
8|verified_with_policy|1.0|0.58|0.9725
10|recorded||0.2|0.0
13|recorded||1.0|0.0
21|incident|0.0|0.5|0.9725
```

Ledger integrity readback:

```json
{
  "ledger_tip_sha256": "sha256:569e35c03709ee476cf6090064c78058525c76b11f848921ad7fa51e4da3188a",
  "metadata_record_count": "21",
  "metadata_tip_sha256": "sha256:569e35c03709ee476cf6090064c78058525c76b11f848921ad7fa51e4da3188a",
  "record_count": 21,
  "schema_version": 6,
  "status": "passed"
}
```

## Research Basis

Phase 12 follows the scoring doctrine in
`12_scoring_and_risk_model.md` and external guidance:

- NIST AI RMF: risk should be governed, mapped, measured, and managed with
  documented measurement context.
- NIST SP 800-53 audit/accountability controls: audit records need enough
  content to support review and accountability.
- OWASP LLM guidance: agentic systems need governance around excessive agency,
  permissions, and evidence of control.
