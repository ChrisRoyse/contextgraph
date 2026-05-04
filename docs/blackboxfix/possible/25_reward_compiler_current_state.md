# Reward Compiler Current State

The reward compiler is now implemented as a first-class BlackBoxFix command:

```text
context-graph blackbox reward compile \
  --ledger PATH \
  --run-id RUN_ID \
  --input reward-spec.json \
  [--tenant TENANT] [--agent AGENT] [--model MODEL]
```

It compiles durable reward and loss values from verified system outcomes. The
source of truth is not a model answer. The source of truth is the physical
SQLite `ledger_records` table plus the reward spec file hash.

## What It Does

For one run, the compiler:

1. verifies the physical ledger hash chain;
2. requires a persisted `scorecard` row for the run;
3. validates a strict reward spec JSON file;
4. reads scorecard features from `score.score` and `score.score_inputs`;
5. computes weighted component contributions;
6. clips and normalizes the reward;
7. stores `reward`, `loss`, and a `verified_outcome_reward_sample`;
8. appends one `reward_compilation` row;
9. performs a separate SQLite readback by sequence, id, run id, payload hash,
   and record hash before reporting success.

Failed compilation appends no row.

## Why This Matters

This turns BlackBoxFix from evidence and observability into a measurable
optimization surface. A model/harness can now be evaluated by verified outcome
features:

- did the source-of-truth verifier pass?
- did every tool call have evidence?
- was readback coverage complete?
- was policy missing or denied?
- did an incident occur?
- how high was risk?
- how valuable is this episode for training/eval reuse?

That gives the system a concrete bridge from:

```text
LLM -> system action -> source-of-truth evidence -> scorecard -> reward/loss
```

The reward compiler is intentionally outcome-supervised. It does not ask an LLM
to grade its own answer.

## Reward Spec Contract

Example:

```json
{
  "reward_spec_schema_version": 1,
  "reward_id": "verified_outcome_reward_v1",
  "objective": "maximize verified source-of-truth outcomes while penalizing risk, incidents, and missing governance",
  "bounds": { "min": -10.0, "max": 10.0 },
  "components": [
    { "id": "correctness", "feature": "correctness", "weight": 4.0 },
    { "id": "verification_confidence", "feature": "verification_confidence", "weight": 2.0 },
    { "id": "risk_penalty", "feature": "risk", "weight": -2.0 },
    { "id": "incident_penalty", "feature": "incident_present", "weight": -3.0 },
    { "id": "policy_missing_penalty", "feature": "policy_missing", "weight": -1.0 },
    { "id": "training_value", "feature": "training_value", "weight": 1.0 }
  ]
}
```

Strict validation:

- root object fields are fixed;
- schema version must match;
- bounds must be finite and ordered;
- components must be non-empty, unique, and use known features;
- weights must be finite;
- nullable source features fail as `REWARD_FEATURE_NULL` unless the component
  explicitly sets `"null_behavior": "zero"`;
- unknown fields and unknown features fail closed.

## Available Features

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

## Persisted Output

The durable `reward_compilation` row includes:

- `reward_compilation_id`;
- reward spec path and `sha256`;
- latest scorecard row ref;
- scorecard input record refs;
- `source_record_refs_sha256`;
- feature vector values with source pointers and meanings;
- component contribution table;
- `reward.raw`;
- `reward.clipped`;
- `reward.normalized_0_1`;
- `reward.loss`;
- `reward.class`;
- `episode_context` with task/prompt context, redacted tool inputs, tool refs,
  verifier readbacks, and verifier score evidence;
- `training_sample.sample_kind = verified_outcome_reward_sample`.

## Manual FSV Proof

Latest manual FSV run:

```text
/tmp/blackbox-reward-compiler-manual-EJOYc8
```

Source of truth:

```text
sqlite:/tmp/blackbox-reward-compiler-manual-EJOYc8/blackbox.sqlite#ledger_records
```

Happy path physical readback:

```text
sequence  record_kind         reward_compilation_id                    raw_reward  loss    reward_class
--------  ------------------  ---------------------------------------  ----------  ------  ------------
9         reward_compilation  reward_0793a30bbdca4c1997391ad8aa075d88  4.985       -4.985  positive
```

Feature proof read from the stored row:

```json
{
  "reward": {
    "raw": 4.985,
    "clipped": 4.985,
    "normalized_0_1": 0.7493,
    "loss": -4.985,
    "class": "positive"
  },
  "feature_vector": {
    "correctness": { "value": 1.0, "source": "/score/correctness" },
    "risk": { "value": 0.58, "source": "/score/risk" },
    "verification_confidence": {
      "value": 0.9725,
      "source": "/score/verification_confidence"
    },
    "policy_missing": {
      "value": 0.0,
      "source": "/score_inputs/policy_compliance"
    }
  },
  "training_sample_kind": "verified_outcome_reward_sample"
}
```

Edge-case proof:

```text
EDGE_EMPTY_BEFORE=1 AFTER=1 EXIT=1 ERROR=REWARD_SPEC_INVALID
EDGE_UNKNOWN_BEFORE=1 AFTER=1 EXIT=1 ERROR=REWARD_FEATURE_UNKNOWN
EDGE_NULL_BEFORE=1 AFTER=1 EXIT=1 ERROR=REWARD_FEATURE_NULL
```

Final physical row counts:

```text
agent_run           2
evidence_packet     1
expected_delta      1
policy_decision     1
readback_snapshot   1
reward_compilation  1
scorecard           2
tool_call           1
verifier_result     1
```

## Automated FSV

Implemented test:

```text
crates/context-graph-cli/tests/blackbox_reward_compiler_fsv.rs
```

Verified commands:

```text
cargo test -p context-graph-cli --test blackbox_reward_compiler_fsv -- --nocapture
cargo test -p context-graph-cli --test blackbox_phase12_scorecard_fsv -- --nocapture
```

The reward compiler test uses a real SQLite ledger, a real local Git
repository, real verifier evidence, a real scorecard, and a real reward spec.
It then opens SQLite directly to prove the persisted reward row exists and the
failure cases do not append rows.

## Research Basis

The implementation follows a practical lesson from reward-modeling and
benchmark research: reward signals are useful, but proxy objectives are easy
to over-optimize if they are detached from real outcomes.

Relevant basis:

- InstructGPT/RLHF shows reward models can train better behavior, but the
  reward function becomes the optimization target:
  <https://arxiv.org/abs/2203.02155>
- Constitutional AI/RLAIF shows rule-based and AI-feedback reward signals can
  drive training, which makes the reward-source contract important:
  <https://arxiv.org/abs/2212.08073>
- RewardBench exists because reward models need direct evaluation rather than
  assumed validity:
  <https://arxiv.org/abs/2403.13787>
- OpenAI's SWE-bench Verified retirement note is a current example of why eval
  signals must be audited for contamination, saturation, and broken ground
  truth:
  <https://openai.com/index/why-we-no-longer-evaluate-swe-bench-verified/>
- OpenTelemetry GenAI semantic conventions support traceability around model
  and agent operations, while BlackBoxFix adds source-of-truth outcome reward:
  <https://opentelemetry.io/docs/specs/semconv/gen-ai/>

The compiler therefore stores provenance and readback refs alongside the reward
itself. A downstream trainer can optimize the reward while still knowing which
durable evidence produced it.
