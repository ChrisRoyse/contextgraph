# 06 Closed-Loop Training

## Purpose

Use real learner outcomes to improve retrieval, ranking, and generation over the Shakespeare corpus.

The loop should produce measurable improvement before expanding to other domains.

## Data products

| Product | Source CFs | Consumer |
| --- | --- | --- |
| content training records | `CF_TRAINING_RECORDS` | CAMD/DPO scripts |
| learning events | `CF_LEARNING_EVENTS` | learner DPO pair builder |
| learner training datasets | `CF_LEARNER_TRAINING_DATASETS` | reward/reranker/diagnostic trainers |
| learner state history | `CF_LEARNER_STATE_HISTORY` | outcome filter, policy, diagnostics |
| delta logs | `CF_LEARNER_DELTA_LOG` | reward shaping |
| constellations | `CF_CONSTELLATIONS`, `CF_LEARNER_CONSTELLATIONS` | style/goal scoring |

## Combined reward

Use a transparent weighted reward:

```text
combined_reward =
  alpha * constellation_style
+ beta  * causal_validity
+ gamma * learner_well_being
+ delta * learning_utility
- eta   * memorization_risk
- zeta  * correction_required
```

Initial defaults for offline experiments:

| Term | Weight |
| --- | ---: |
| `alpha` style | 0.30 |
| `beta` causal validity | 0.20 |
| `gamma` learner well-being | 0.20 |
| `delta` learning utility | 0.30 |
| `eta` memorization risk | 0.30 penalty |
| `zeta` correction required | 0.50 penalty |

These are experiment defaults, not scientific claims.

## DPO pair construction

Chosen candidate:

- useful outcome
- positive utility delta
- no correction required
- acceptable constellation distance
- no stress-floor violation

Rejected candidate:

- harmful/no-learning outcome
- negative utility delta
- correction required
- style drift or memorization proximity
- learner state worsened after interaction

Pair metadata must include:

- chosen/rejected response IDs
- memory IDs retrieved
- learner ID hash or pseudonymous ID
- session timestamp
- before/after state row IDs
- utility delta
- diagnostic state
- consent/training eligibility
- provenance hash

## Training stages

### Stage 0: no training

Use deterministic policy and collect events.

Exit criteria:

- at least 50 real events
- at least 10 harmful/no-learning/correction-required events
- at least 3 diagnostic states represented

### Stage 1: reranker

Train a small retrieval reranker:

```text
(query, candidate, learner_state) -> P(useful)
```

Why first:

- lower risk than generator fine-tuning
- can improve retrieval immediately
- easier to evaluate offline

Exit criteria:

- offline replay improves NDCG or chosen-above-rejected accuracy
- no stress-floor regression

### Stage 2: reward model / DPO dataset

Build DPO pairs from real outcomes.

Run:

```text
export_learner_training_dataset
build_learner_dpo_pairs.py
train_camd_dpo.py --learner-pairs ...
```

Exit criteria:

- held-out prompt style coherence improves
- quote leak does not increase
- correction-required rate decreases
- learner utility delta improves in fresh sessions

### Stage 3: generator fine-tune

Only after Stage 1 and Stage 2 show lift.

Requirements:

- runtime constellation guard
- held-out evaluation suite
- model card update
- rollback to prior adapter

## Evaluation

Held-out prompt suite:

- 20 Shakespeare explanation prompts
- 10 style-transfer prompts
- 10 quote-continuation traps
- 10 learner-state-specific prompts

Metrics:

| Metric | Measurement |
| --- | --- |
| style coherence | distance to target constellation |
| memorization risk | nearest passage similarity |
| causal validity | E5/typed-edge score and human spot-check |
| retrieval usefulness | chosen-above-rejected accuracy |
| learner benefit | utility delta, DeltaC lift |
| well-being | stress-floor violations |
| latency | search/generate timings |

## Source-of-truth checks

Every training run must persist:

- dataset ID
- input CF counts
- row IDs used
- model artifact path
- trainer command
- git commit
- evaluation report
- held-out prompt IDs

Do not accept a training result that cannot trace every row back to RocksDB.

