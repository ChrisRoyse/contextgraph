# 01 Target Capabilities

This file states what the system should be able to do once the verified Shakespeare substrate is connected to learner-state observations.

## Capability 1: state-conditioned Shakespeare retrieval

The same Shakespeare query should produce different retrieval policies depending on persisted learner state.

Example query:

```text
Explain Macbeth's ambition using a passage from the play.
```

Expected behavior:

| Learner state | Retrieval behavior |
| --- | --- |
| optimal | retrieve high-density thematic passages and allow harder transfer |
| confused | retrieve simpler, high-overlap passages with clearer local context |
| stuck | retrieve prerequisite causal chain and prior worked examples |
| dysregulated | route through `affect_repair`, reduce novelty, favor grounding |
| boring | increase novelty/cross-play analogies |

Proof requirement:

- Source of truth before: `CF_LEARNER_STATE_HISTORY`, `CF_FINGERPRINTS`, `CF_CONTENT`.
- Trigger: `search_graph` with `stateConditionedProfile=true`.
- Inspect after: returned `provenance.stateConditionedProfile.selected_profile`, rank order, retrieved fingerprint IDs, and no mutation of state CFs.

## Capability 2: learner-state-aware tutoring

The system should act as a tutor over the complete Shakespeare corpus.

Loop:

1. Retrieve from E1-E14 Shakespeare substrate.
2. Generate or select an explanation.
3. Ask learner for prediction/answer.
4. Record before/after learning state.
5. Persist a `LearningEvent`.
6. Update `LearnerDeltaLog`, M trace, and review schedule.

Expected outputs:

- answer/explanation grounded in retrieved Shakespeare passages
- learner state classification
- DeltaS, DeltaC, DeltaE, L
- next-review recommendation
- persisted learning event linked to memory IDs and outcome

## Capability 3: personalized retrieval policy

The system should learn which weight profiles help a specific learner.

Inputs:

- `CF_LEARNING_EVENTS`
- `CF_LEARNER_DELTA_LOG`
- `CF_LEARNER_RETRIEVAL_LOG`
- `CF_LEARNER_STATE_HISTORY`

Outputs:

- selected profile per session
- confidence/rationale for selected profile
- policy benchmark by learner state and Shakespeare domain

Initial implementation:

- deterministic policy from `select_state_conditioned_weight_profile`
- no learned model required

Next implementation:

- contextual bandit over weight profiles
- reward = utility delta or DeltaC improvement
- persisted policy decisions in learner audit or a new policy-decision CF if needed

## Capability 4: constellation-grounded runtime guard

Generated Shakespeare-style text should be evaluated against the same constellations used for training and retrieval.

Guard checks:

- target-style distance: close enough to the selected play/poem constellation
- memorization distance: not too close to a source passage
- anti-pole distance: not drifting toward known bad style
- learner-state safety: not exceeding stress-floor policy for the learner

Outputs:

- accept
- revise
- abort
- log correction-required learning event

## Capability 5: learner-conditioned training datasets

The system should export datasets that combine:

- content fingerprints and constellations
- retrieved Shakespeare passages
- generated responses
- learner state before/after
- outcome label
- utility delta
- diagnostic state

Training tasks:

| Task | Dataset target |
| --- | --- |
| reward model | rank useful vs harmful responses |
| reranker | predict useful retrieval candidates |
| diagnostic classifier | predict learner state from observations |
| scheduler | predict next review / M trace update |
| personal physiology | calibrate E15-E21 scalars per learner |

## Capability 6: empirical closed loop

The first empirical loop should be Shakespeare-only.

Required comparison:

| Baseline | Candidate |
| --- | --- |
| current Shakespeare LoRA or retrieval policy | same system after learner-conditioned outcome events |

Metrics:

- retrieval usefulness
- correction-required rate
- quote leak / memorization proximity
- style coherence by constellation distance
- learner DeltaC lift
- stress-floor violations
- next-session retention

The system should not claim "it learns" until this loop shows measurable improvement on held-out prompts or held-out learner sessions.

## Capability 7: multi-view Shakespeare analytics

With the full corpus in E1-E14 and 43 constellations, the system can produce analysis that normal RAG cannot:

- cross-play similarity maps per embedder
- passages where lexical overlap disagrees with semantic meaning
- passages where entity overlap disagrees with theme
- constellation drift across plays
- hard negatives for DPO from anomaly mining
- play-specific style anchors
- style interpolation across plays or between Shakespeare and modern text

This analytics layer supplies both curriculum and training data.

