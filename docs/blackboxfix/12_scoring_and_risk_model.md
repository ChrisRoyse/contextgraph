# Scoring And Risk Model

BlackBoxFix scores agent actions using verifier evidence, policy context, and
source-of-truth impact. The score must explain why an action is trusted, not
hide the details.

Scoring is not implemented in Phase 1. The contract below is what the
verifier must produce starting in Phase 2 and what the dashboard must
visualize starting in Phase 7.

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

## Dashboard Metrics (Phase 7)

- verifier pass rate;
- verifier failure rate by tool;
- blocked action count;
- mean time to remediation;
- high-risk autonomous actions;
- top error codes;
- source-of-truth coverage (`% mutating tools with at least one verifier`);
- evals generated from failures;
- regression recurrence rate.

## Avoid Black-Box Scoring

Scores must be explainable from ledger fields. The product never produces a
risk number without showing the evidence and rules that produced it. The
score record must include `score_inputs` plus a `score_rule_version`.

## FSV Of The Scoring Contract (Phase 2 target)

Before scoring lands, write three deterministic test cases:

1. **Happy path**: passed verifier on a low-risk read-only action ⇒
   `correctness=1.0`, `risk≤0.2`, `trust_level=verified`.
2. **Failure**: verifier failed with `error_code` set ⇒
   `correctness=0.0`, `trust_level=incident` if action was high-risk.
3. **Blocked**: verifier blocked due to missing permission ⇒
   `correctness=null`, `trust_level=recorded`, `error_code=CONNECTOR_PERMISSION_DENIED`.

Each test must read the score back from the persisted ledger row, not from
the in-memory return value.
