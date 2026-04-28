# 03 TCT Architecture For Shakespeare Style

## Practical Definition

For text, Teleological Constellation Training should mean:

```text
same frozen embedder panel
  -> labels the corpus
  -> selects and weights training examples
  -> mines preference pairs
  -> defines the runtime guard
  -> defines FSV metrics
```

The target is not a learned scalar reward model. The target is a frozen geometric reference panel built from Shakespeare.

## Important Implementation Reality

The paper's geometric loss term is clean mathematically. For autoregressive text, direct gradient flow through `phi_m(g_theta(x))` is not straightforward because sampled tokens are discrete and Context Graph embedders are outside the generator graph.

Therefore, the practical implementation should use TCT in four trainable ways:

1. **Data selection:** only train on examples whose frozen-embedder scores are correct.
2. **Sample weighting:** weight examples by distance to the style constellation and coverage gaps.
3. **Preference optimization:** DPO pairs are mined and labeled by constellation reward.
4. **Auxiliary negative training:** loops, style drops, and malformed archaic pastiche become rejected examples or unlikelihood negatives.

Do not pretend there is true differentiable embedder loss unless a soft-token or embedding-space surrogate is actually implemented and FSV-tested.

## Core Objects

### Style Constellations

Build multiple style constellations instead of one monolithic "Shakespeare":

| Constellation | Purpose |
|---|---|
| `sonnets` | lyric verse, compact metaphor, high style. |
| `soliloquy` | introspective argument, philosophy, moral pressure. |
| `comic_prose` | wit, prose, practical dialogue, lower register. |
| `tragic_blank_verse` | high drama, fate, death, command. |
| `instructional_bridge` | modern topics answered in Shakespeare style, verified bridge set. |

The general guard can use a max-or-mixture over these anchors rather than forcing every output toward sonnet style.

### Content Equivalence Panel

Use content-oriented embedders to ensure meaning survives:

- E1 semantic.
- E10 paraphrase.
- E14 BGE-M3 Dense for multilingual and long context.
- E11 entity where available.
- E7 for code/structure.

### Style Panel

Use style-sensitive views:

- E12 ColBERT.
- E13 SPLADE.
- E7 syntax/code-structure signal, calibrated carefully.
- E9 HDC structural signal.
- E10 paraphrase surface-variation signal.
- Optional E14 only after measured style discrimination.

### Coherence Panel

Add non-style failure detectors:

- repeated n-gram rate,
- longest repeated substring,
- type-token ratio,
- self-BLEU against preceding windows,
- E12 self-MaxSim repetition,
- E9 symbolic recurrence if exposed cheaply,
- finish-reason and EOS behavior.

## Reward Function

For candidate output `y` under prompt `x`:

```text
R(y | x) =
    + style_reward(y)
    + content_reward(x, y)
    + format_reward(x, y)
    - repetition_penalty(y)
    - memorization_penalty(y)
    - contamination_penalty(y)
```

Where:

- `style_reward` is weighted cosine/Mahalanobis proximity to the relevant style constellation.
- `content_reward` measures preservation of user request, entities, and required answer elements.
- `format_reward` is parse/compile/schema pass when applicable.
- `repetition_penalty` catches loops.
- `memorization_penalty` catches exact Shakespeare passage reuse.
- `contamination_penalty` catches scene headers, internal notes, and prompt echo.

The reward should be stored with full per-view trace. Any record without a trace is invalid for training.

## TCT Phases

### Phase 1: Freeze Reference Constellations

Inputs:

- cleaned Shakespeare passages,
- sonnet/play/register labels,
- modern bridge examples only after validation,
- fixed embedder versions and hashes.

Outputs:

- centroid and covariance per constellation,
- threshold calibration set,
- manifest with source hashes and member IDs.

FSV:

- repeated construction from same IDs gives same centroid hash,
- Shakespeare held-out samples score above modern controls,
- dirty scene headers do not score as valid style just because they are from Shakespeare files.

### Phase 2: Train Against The Frozen Constellations

Inputs:

- clean SFT bridge examples,
- CAMD-DPO pairs,
- adversarial style-break pairs,
- loop-negative pairs,
- code and structured-output examples.

Training:

- SFT refresh from v2.1 with sample weights.
- DPO with constellation-mined preferences.
- Optional unlikelihood on loop tokens.

FSV:

- direct harness is informational only,
- production browser FSV is the gate.

### Phase 3: Runtime Guard

Runtime candidates must pass:

- style constellation threshold,
- content preservation threshold where a reference can be computed,
- repetition/coherence threshold,
- format parser when applicable.

No silent fallback. If the guard cannot run, return infrastructure error rather than calling broken output valid.

## Why This Is Stronger Than Prior Runs

Prior runs used pieces of TCT but not all roles consistently:

- v2.1 had working LoRA plus DPO plus guard, but DPO was partly topic-based and evaluation was too narrow.
- v4/v4.1 expanded compositional training but skipped robust DPO and overfit serving config.
- v4.2 implemented CAMD ideas but current sampled data shows contamination.

The optimized architecture makes the frozen constellation the source of truth across all phases.

