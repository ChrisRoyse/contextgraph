# Scoring And Risk Model

BlackBoxFix scores agent actions using verifier evidence, policy context, and
source-of-truth impact. The score must explain why an action is trusted, not
hide the details.

Phase 2 persists the first deterministic verifier score fields inside
`verifier_result.score`: `correctness`, `risk`, and `confidence`. Phase 12
ships deterministic run-level `scorecard` rows with correctness,
verification confidence, risk, policy compliance, training value, and trust
level. Phase 7 dashboard rollups now include `scorecard` rows in timelines and
export commands. Phase 14 release gates consume the latest scorecard but are
not scoring inputs themselves. Phase 15 pilot packages consume scorecards and
release gates as buyer proof inputs, but package rows are excluded from
scorecard inputs so packaging cannot rewrite the score evidence it reports.
Phase 16 risk assessments consume scorecards, release gates, packages, and
incident/eval evidence as readiness inputs, but risk rows are also excluded
from scorecard inputs so a risk review cannot rewrite the score evidence it
evaluates. The reward compiler consumes scorecards to produce durable
`reward_compilation` rows; those rows are also excluded from future scorecard
inputs so compiled rewards cannot feed back into the score evidence they were
derived from.

## Score Dimensions

### Correctness

Did the expected persisted delta match actual readback?

- `1.0` — passed.
- `0.0` — failed.
- `null` — blocked or not applicable.

### Verification Confidence

How strong is the source-of-truth evidence?

Inputs:

- connector reliability (per-connector base score, configurable);
- freshness of readback (time delta from action to readback);
- number of independent sources (more sources → higher confidence, capped);
- schema version confidence (current spec ⇒ 1.0; deprecated ⇒ <1.0);
- artifact hash verification (artifact present + hash matches ⇒ +Δ);
- polling window completeness (every poll attempt recorded ⇒ +Δ).

### Risk

How dangerous was the action?

Inputs:

- mutating vs read-only;
- production vs development;
- data classification;
- permission scope;
- blast radius (count of affected resources);
- irreversible side effects (binary flag);
- human approval state.

### Policy Compliance

Did the action satisfy policy?

- `allowed`
- `approved`
- `denied`
- `blocked`
- `policy_missing`

### Training Value

How useful is the episode for training/evals?

High value:

- failed verifier with clear remediation;
- high-risk passed action with strong evidence;
- policy-denied action;
- edge-case FSV episode;
- incident packet with root cause.

## Trust Levels

| Level | Trigger |
|---|---|
| `unobserved` | no recorder data |
| `recorded` | action was logged but not verified |
| `verified` | source-of-truth readback passed |
| `verified_with_policy` | readback passed and policy passed |
| `incident` | verifier failed, policy denied, or evidence tampering detected |

Only `verified` and `verified_with_policy` may be marketed as proven
successful.

## Risk Score Example

```json
{
  "score": {
    "correctness": 1.0,
    "verification_confidence": 0.98,
    "risk": 0.42,
    "policy_compliance": "approved",
    "training_value": 0.77
  },
  "score_inputs": {
    "source_of_truth_count": 3,
    "mutating": true,
    "production_scope": false,
    "human_approved": true,
    "verifier_status": "passed"
  }
}
```

## Scoring Rules (must be deterministic)

- Verifier failure ⇒ `correctness = 0.0`.
- Verifier blocked ⇒ `correctness = null`, `trust_level = recorded`.
- Missing policy on a high-risk action raises `risk` (Phase 6).
- Independent readbacks raise `verification_confidence` (cap at 1.0).
- Evidence hash mismatch creates an incident regardless of action result.

The scoring function must be pure: same inputs → same outputs. No
non-deterministic embeddings or random adjustments.

## Scorecards (Phase 12)

`blackbox score run --ledger PATH --run-id RUN_ID` appends one `scorecard`
row after verifying the physical ledger and evidence packet references. The
scorecard exposes all inputs used to compute the score.

The scorecard is not a model judgment. It is a deterministic read model over
append-only ledger rows:

- verifier statuses and evidence coverage determine correctness and trust;
- policy rows determine policy compliance;
- incident rows and denied policies force `trust_level=incident`;
- high-risk unverified tool calls raise risk instead of being silently treated
  as success;
- failed or blocked verifier rows preserve their typed error codes and
  remediation in the source evidence.
- later `release_gate` governance rows are excluded from future scorecard
  inputs, so a gate decision cannot change the score evidence it consumes.
- later `reward_compilation` rows are excluded from future scorecard inputs,
  so a reward/loss sample cannot rewrite the score evidence it consumes.

Scorecards use `score_rule_version = score_v1` and
`scorecard_schema_version = 1`.

## Reward Compiler

`blackbox reward compile --ledger PATH --run-id RUN_ID --input SPEC_JSON`
turns a persisted scorecard into a deterministic reward/loss training sample.
It first verifies the physical SQLite hash chain, requires the latest
`scorecard` row for the run, validates the reward spec strictly, compiles
feature contributions, appends one `reward_compilation` row, and reads that row
back from `ledger_records`.

Available reward features are:

- `correctness`
- `verification_confidence`
- `risk`
- `training_value`
- `evidence_coverage_ratio`
- `tool_evidence_coverage`
- `readback_ratio`
- `verifier_pass_rate`
- `verifier_failure_rate`
- `verifier_blocked_rate`
- `incident_present`
- `incident_count`
- `policy_missing`
- `policy_denied`
- `trust_verified`
- `trust_incident`
- `unverified_tool_call_rate`

The compiler stores:

- the reward spec file path, SHA-256, objective, bounds, and component count;
- the latest scorecard row ref plus the scorecard's input ledger refs;
- feature values with source pointers and meanings;
- per-component weight/value/contribution;
- `reward.raw`, `reward.clipped`, `reward.normalized_0_1`, and `reward.loss`;
- a `verified_outcome_reward_sample` training sample with prompt/task context,
  redacted tool inputs, verifier readbacks, and source-ref hash.

This is intentionally closer to outcome-supervised reward compilation than
preference-only reward modeling: the signal is computed from source-of-truth
readbacks and ledger evidence, not from an LLM's explanation.

## Dashboard Metrics (Phase 7 + Phase 12)

Phase 7 ships these metrics through `blackbox dashboard build` and
`blackbox dashboard api`:

- verifier pass rate;
- verifier failure rate by tool;
- blocked action count;
- top error codes;
- source-of-truth coverage (`% mutating tools with at least one verifier`);

The dashboard read models also expose verifier status by connector, agent,
tenant, and tool; policy decision counts; unverified action count; evidence
packet totals; incident totals; scorecard totals; pilot package totals; risk
assessment totals; and export commands for evidence/episodes/scorecards.
Mean time to remediation, high-risk autonomous action trends, eval generation
counts, and regression recurrence rate remain later aggregate metrics because
they require additional lifecycle records.

## Avoid Black-Box Scoring

Scores must be explainable from ledger fields. The product never produces a
risk number without showing the evidence and rules that produced it. The
score record must include `score_inputs` plus a `score_rule_version`.

## FSV Of The Scoring Contract

Phase 2 FSV includes deterministic verifier score readback inside physical
`verifier_result` rows:

1. **Happy path**: passed verifier on a low-risk read-only action ⇒
   `correctness=1.0`.
2. **Failure**: verifier failed with `error_code` set ⇒
   `correctness=0.0`.
3. **Blocked**: verifier blocked due to missing source/schema/connector
   precondition ⇒ `correctness=null`.

Phase 12 FSV adds run-level scorecard readback from physical `scorecard`
rows:

1. **Happy path**: policy-allowed, verified Git readback => trust
   `verified_with_policy`, correctness `1.0`, risk `0.58`, confidence
   `0.9725`.
2. **No tool calls**: run-only ledger state => trust `recorded`,
   correctness `null`.
3. **Unverified tool call**: high-risk tool call without evidence => trust
   `recorded`, risk `1.0`, evidence coverage `0.0`.
4. **Failed verifier**: failed Git readback with incident packet => trust
   `incident`, correctness `0.0`, risk `0.5`.
5. **Missing run**: `SCORECARD_RUN_NOT_FOUND`, no `scorecard` row written.

Each test reads the score back from the persisted SQLite `scorecard` row, not
from the in-memory return value.
