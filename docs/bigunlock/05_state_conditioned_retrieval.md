# 05 State-Conditioned Retrieval

## Purpose

Use E15-E21 learner state to choose how the E1-E14 Shakespeare substrate is searched and ranked.

This should be deterministic first. Learned policy can come later.

## Inputs

| Input | Source |
| --- | --- |
| learner state components | `CF_LEARNER_STATE_HISTORY` |
| learner diagnostic state | `CF_LEARNER_DELTA_LOG` or computed from components |
| base profile | user/tool argument |
| content candidates | `CF_FINGERPRINTS`, HNSW/sparse indexes |
| outcomes | `CF_LEARNING_EVENTS` |

## Existing routing

The repo already exposes:

- `resolve_learner_retrieval_policy`
- `search_graph` with `stateConditionedProfile`
- profiles: `affect_repair`, `affect_priming`, `affect_neutral`

The first production behavior should be:

| Condition | Selected profile |
| --- | --- |
| high stress floor | `affect_repair` |
| negative valence, low stress | `affect_priming` |
| high arousal/stress and emotional query | `affect_neutral` |
| otherwise | base profile |

## Shakespeare-specific policy

| Learner condition | Shakespeare retrieval adjustment |
| --- | --- |
| confused | increase E12/E6 phrase precision; favor shorter scenes/sonnets |
| stuck | increase E5/E8 causal/graph; retrieve prerequisite explanations |
| dysregulated | use `affect_repair`; reduce cross-play novelty |
| boring | increase E14/E10; cross-play analogy and long-context transfer |
| optimal | preserve semantic/constellation profile; allow harder passages |

## Retrieval response requirements

Every state-conditioned search response should include:

- selected profile
- base profile
- learner ID/session timestamp
- state component summary
- diagnostic state if available
- final weights when requested
- whether policy was deterministic or learned
- source-of-truth rows read

If these fields are not present, FSV should fail.

## No fallback rules

If `stateConditionedProfile=true`:

- missing `learnerId`: error
- missing `learnerSessionTs`: error
- no state row: error
- unsupported base profile: error
- custom weights combined with state conditioning: error

Do not silently run unconditioned retrieval.

## Outcome-prediction filter

After deterministic routing is proven, add a learned filter:

```text
P(useful | query fingerprint, candidate fingerprint, learner state)
```

Initial implementation:

- train a small model outside the MCP server
- features come from `CF_LEARNING_EVENTS`, `CF_LEARNER_STATE_HISTORY`, `CF_TRAINING_RECORDS`
- inference is optional but must be explicit, e.g. `useOutcomeFilter=true`

FSV:

- before/after candidate list
- physical read of model/version metadata
- deterministic fixture from real persisted events
- fail if model artifact missing when requested

## Bandit policy

Once at least 100 real learning events exist, add a contextual bandit:

| Arm | Meaning |
| --- | --- |
| `semantic_search` | default semantic |
| `causal_reasoning` | causal explanation |
| `multilingual_search` | E14-heavy |
| `long_context` | E14 long-context |
| `affect_repair` | regulated/calming |
| `affect_priming` | valence repair |
| `affect_neutral` | emotional suppression |

Reward:

```text
reward = utilityDelta + 0.5 * DeltaC - stressPenalty - correctionPenalty
```

Do not use the bandit for user-facing routing until its offline replay beats deterministic routing.

## Benchmark suite

Run each query under 4 states:

1. optimal
2. confused
3. stuck
4. dysregulated

Queries:

- Hamlet existence/decision
- Macbeth ambition/causality
- Romeo and Juliet love/conflict
- Lear authority/betrayal
- Sonnets time/beauty

Required evidence:

- selected profile differs where expected
- at least one top-5 ID differs across states
- content readback verifies IDs
- no state mutation during search
- latency remains under target

