# Scoring And Risk Model

BlackBoxFix should score agent actions using verifier evidence, policy context,
and source-of-truth impact. The score should explain why an action is trusted,
not hide the details.

## Score Dimensions

### Correctness

Did the expected persisted delta match actual readback?

- 1.0: passed.
- 0.0: failed.
- null: blocked or not applicable.

### Verification Confidence

How strong is the source-of-truth evidence?

Factors:

- connector reliability;
- freshness of readback;
- number of independent sources;
- schema version confidence;
- artifact hash verification;
- polling window completeness.

### Risk

How dangerous was the action?

Factors:

- mutating vs read-only;
- production vs development;
- data classification;
- permission scope;
- blast radius;
- irreversible side effects;
- human approval state.

### Policy Compliance

Did the action satisfy policy?

- allowed;
- approved;
- denied;
- blocked;
- policy missing.

### Training Value

How useful is the episode for training/evals?

High value:

- failed verifier with clear remediation;
- high-risk passed action with strong evidence;
- policy-denied action;
- edge-case FSV episode;
- incident packet with root cause.

## Trust Levels

- `unobserved`: no recorder data.
- `recorded`: action was logged but not verified.
- `verified`: source-of-truth readback passed.
- `verified_with_policy`: readback passed and policy passed.
- `incident`: verifier failed, policy denied, or evidence tampering detected.

Only `verified` and `verified_with_policy` should be marketed as proven
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

## Scoring Rules

- Verifier failure sets correctness to 0.0.
- Verifier blocked sets correctness to null and trust level to `recorded`.
- Missing policy on high-risk action raises risk.
- Independent readbacks can raise verification confidence.
- Evidence hash mismatch creates an incident regardless of action result.

## Dashboard Metrics

- verifier pass rate;
- verifier failure rate by tool;
- blocked action count;
- mean time to remediation;
- high-risk autonomous actions;
- top error codes;
- source-of-truth coverage;
- evals generated from failures;
- regression recurrence rate.

## Avoid Black-Box Scoring

Scores must be explainable from ledger fields. The product should never produce
a risk number without showing the evidence and rules that produced it.

