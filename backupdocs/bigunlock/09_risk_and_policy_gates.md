# 09 Risk And Policy Gates

## Hard engineering gates

No capability should be marked "done" unless:

- it has an FSV report
- source-of-truth CFs were physically inspected
- edge cases were run
- no mock data was used for happy path
- failures produce actionable errors
- missing state does not silently fall back

## Scientific gates

The learner-state layer should not overclaim.

Allowed claims after initial FSV:

- learner state was persisted
- deterministic policy selected a different profile
- retrieval output changed under different state
- events exported to training datasets

Not allowed until experiments show it:

- the system improves human learning
- E15-E21 improve outcomes
- physiological channels are causal
- model alignment is safer because of learner state

## Privacy gates

Learner-state data may include sensitive biometric or affective information.

Default posture:

- local-first
- explicit consent
- no cloud upload unless user opts in
- no training export unless consent allows training
- revocation must block future use

## Product gates

Avoid institutional education/workplace deployment until separate compliance work exists.

Safer initial scope:

- personal learner
- local machine
- transparent logs
- user-controlled export/delete
- human-reviewed interpretations

## Failure classes

| Failure | Required behavior |
| --- | --- |
| missing model asset | fail preflight |
| missing learner state | fail state-conditioned retrieval |
| malformed vector | validation error |
| partial write | rollback or exact failed CF error |
| revoked consent | block use/export |
| empty export | error unless explicitly requested dry-run |
| trainer no lift | do not promote model |
| memorization regression | block release |

## Benchmark gates

Before promoting a new policy/model:

- compare against frozen baseline
- use held-out prompts
- report confidence intervals or at least raw counts
- include quote-leak/memorization metric
- include stress-floor violations
- include latency
- include source row IDs

## Audit requirements

For every learner-affecting decision, store:

- learner ID
- session timestamp
- state row ID
- selected policy
- retrieved memory IDs
- response ID/hash
- outcome label
- utility delta
- model/policy version

If this is too heavy for every production call, keep it for pilot mode until the storage volume is understood.

