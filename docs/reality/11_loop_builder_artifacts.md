# Loop Builder Artifacts

The meta reality loop needs concrete artifacts it can edit, score, keep, or discard.

## Artifact Classes

| Artifact | Mutable By Outer Loop | Promotion Needs Review | Purpose |
|---|---:|---:|---|
| Prompt compiler template | yes | no for dev, yes for promotion | Build next model prompt from verified evidence. |
| Evidence compressor | yes | yes | Turn logs/readbacks into compact failure facts. |
| Repository context selector | yes | yes | Choose source snippets for the model. |
| Tool schema/descriptions | yes | yes | Improve the agent-computer interface. |
| Retry policy | yes | yes | Decide when to retry, diversify, or stop. |
| Repetition detector | yes | yes | Prevent unchanged patches and repeated strategies. |
| Reward spec candidate | shadow only | yes | Test alternate scoring without changing production reward. |
| Task curriculum | yes | yes | Choose which tasks enter dev/holdout/promotion. |
| LoRA export recipe | yes | yes | Create fine-tuning datasets from verified episodes. |
| Model routing policy | yes | yes | Pick model per task or compare all models. |
| Dashboard/explainer | yes | no | Help humans understand evidence and progress. |

## Required Manifest Per Artifact

Each artifact needs:

```json
{
  "artifact_id": "prompt_compiler_v3",
  "artifact_kind": "prompt_compiler",
  "version": 3,
  "path": "crates/context-graph-cli/src/bin/reality-loop.rs",
  "sha256": "sha256:...",
  "created_by": "reality_loop_builder",
  "editable_scope": "dev_smoke",
  "promotion_status": "candidate",
  "source_of_truth": "file:..."
}
```

## Artifact Experiment Record

When the outer loop tests an artifact, persist:

```json
{
  "artifact_experiment_id": "exp_...",
  "changed_artifact": "prompt_compiler_v3",
  "baseline_artifact": "prompt_compiler_v2",
  "suite": "dev_smoke",
  "models": ["gemma4:e4b"],
  "score_before": {},
  "score_after": {},
  "decision": "keep",
  "decision_reason": "official failure evidence reached attempt 2 prompt and reward improved for gemma4:e4b"
}
```

## Allowed Outer-Loop Changes

The first outer loop should be allowed to change only:

- prompt compiler text;
- evidence selection rules;
- repository context ranking;
- retry/no-repeat policy;
- reporting/scoreboard docs;
- shadow reward specs.

Do not initially allow it to change:

- official evaluator;
- verifier source code;
- base task data;
- production reward spec;
- Docker harness;
- ledger schema.

## Keep/Discard Evidence

The outer loop must prove its decision by reading:

- before scoreboard;
- after scoreboard;
- official report files;
- ledger counts;
- reward rows;
- prompt manifests;
- artifact hashes.

No artifact is kept because a model says it is better.

## Champion Loop

Maintain:

```text
docs/reality/champions/
  current_inner_loop.json
  current_prompt_compiler.json
  current_context_selector.json
  current_reward_spec.json
```

Each champion points to file hashes and the verified scoreboard that justified promotion.

## Artifact Provenance

Every artifact should have a provenance graph:

```text
prior artifact -> outer-loop hypothesis -> code/doc change
  -> smoke run -> source-of-truth readback
  -> reward delta -> keep/discard decision
```

This turns loop evolution itself into auditable training data.
