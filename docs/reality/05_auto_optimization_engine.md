# Auto-Optimization Engine

## Name

The working name is **Reality Autoresearch Engine**.

It adapts Karpathy AutoResearch from one metric on one training file into a general engine for real-world agent outcomes.

## Core Loop

```text
baseline
  -> propose experiment
  -> execute through ContextGraph
  -> verify durable source of truth
  -> compile reward/loss
  -> keep, discard, repair, or escalate
  -> update prompt/compiler memory
  -> repeat
```

## Experiment Types

The engine should support multiple experiment classes:

| Experiment Type | Mutates | Metric |
|---|---|---|
| SWE repair | candidate patch | official SWE-bench resolved status and reward |
| Prompt optimization | prompt template | verifier pass rate, reward, incident rate |
| Tool-interface optimization | ACI command description/schema | model tool success and verifier pass rate |
| Policy tuning | policy rules, thresholds | denied unsafe actions, allowed safe actions |
| Reward tuning | reward spec candidate | correlation with held-out verified outcomes |
| DynamicJEPA planning | planner prompt/params | surprise rate and outcome reward |
| Model fine-tuning | LoRA/adapter weights | held-out reality reward |

## Keep / Discard / Repair

Each attempt produces a decision:

| Decision | Condition |
|---|---|
| `keep` | reward improves and mandatory verifiers pass |
| `discard` | reward equal/worse, repeated patch, or non-useful failure |
| `repair` | failure is actionable and still within attempt budget |
| `escalate` | policy, credential, verifier, or environment issue needs human decision |
| `promote` | held-out verification passes promotion gate |

## Champion State

The engine maintains a champion for each task/model/scope:

```json
{
  "champion_id": "champion_...",
  "task_id": "psf__requests-2317",
  "model": "qwen3:8b",
  "attempt_id": "attempt_003",
  "reward": 4.25,
  "mandatory_verifiers": "passed",
  "source_of_truth_refs": ["file:...", "sqlite:..."],
  "artifact_hashes": ["sha256:..."]
}
```

No champion can be selected from unverifiable evidence.

## Prompt Compiler Loop

The prompt compiler is itself an optimization target.

Inputs:

- task requirements;
- current champion;
- failed attempt evidence;
- source-of-truth snippets;
- reward components;
- surprise records;
- policy decisions;
- repeated mistake list.

Outputs:

- next prompt;
- prompt hash;
- prompt template id;
- reason for each included evidence item;
- excluded evidence list.

Optimization signal:

- prompt template reward average;
- prompt template incident rate;
- prompt template repeated-action rate;
- prompt template token cost;
- prompt template official success rate.

## Data Flywheel

Each attempt becomes:

- benchmark row;
- reward sample;
- preference pair;
- failure cluster;
- training episode;
- regression test;
- prompt compiler example;
- policy example;
- incident pattern.

That creates the improvement flywheel:

```text
verified attempts
  -> better prompt compiler
  -> better tool interface
  -> better model fine-tuning data
  -> better policy/reward specs
  -> higher verified success
  -> more verified attempts
```

## Model Improvement Paths

### Prompt-Level Improvement

Fastest path. Use verified failure evidence to refine:

- system prompt;
- task prompt;
- tool descriptions;
- context selection;
- output schema instructions.

### Tool-Level Improvement

Improve the Agent-Computer Interface:

- smaller command set;
- better line-numbered file views;
- compact failing test summaries;
- patch validators before execution;
- explicit no-repeat constraints.

### Retrieval/Memory Improvement

Use ContextGraph memory to retrieve:

- previous solved examples with similar failures;
- prior failed patches for this instance;
- successful patches in same repo;
- failure clusters by test name;
- code locations with repeated surprise.

### LoRA/Fine-Tuning

Use only verified episodes:

- positive examples: official resolved attempts;
- negative examples: official unresolved attempts with failure cause;
- preference pairs: higher reward attempt preferred over lower reward attempt for same task;
- rejected samples: malformed patches and repeated failed patches.

### Policy/Reward Improvement

Optimize reward specs only against held-out verified outcomes. The optimizer cannot edit its own production reward spec directly.

## Pseudocode

```python
while budget.remaining():
    state = read_reality_state()
    prompt = prompt_compiler.compile(state)
    model_output = model.generate(prompt)
    action = action_gateway.validate(model_output)
    policy = policy_gate.evaluate(action)
    if not policy.allowed:
        persist_incident(policy)
        break

    execution = executor.run(action)
    readback = verifier.read_source_of_truth(action)
    evidence = ledger.persist(execution, readback)
    reward = reward_compiler.compile(evidence)

    if promotion_gate.passes(evidence, reward):
        promote(evidence)
        break

    if reward.improved_over(champion):
        champion = evidence

    if failure_is_actionable(evidence):
        continue

    escalate(evidence)
    break
```

## What ContextGraph Adds Beyond AutoResearch

Karpathy AutoResearch has one simple metric and one editable file. ContextGraph adds:

- multi-system source-of-truth readback;
- typed tool actions;
- policy decisions;
- incident packets;
- tamper-evident ledger;
- artifact provenance;
- training/eval export;
- DynamicJEPA surprise;
- reward decomposition;
- model comparison;
- buyer-facing evidence packets.

That is the bridge from autonomous experimentation to enterprise-governed agent improvement.
