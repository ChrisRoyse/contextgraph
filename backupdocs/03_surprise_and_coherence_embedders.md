# 03 — Surprise (ΔS) and Coherence (ΔC) Embedders

**Purpose.** Operationalise the v1.0-carryover terms ΔS and ΔC using Chris's existing 13-embedder Context Graph stack, plus targeted additions for the simulated-experience term. Both ΔS and ΔC reduce to operations on the same embedder panel, so we treat them together.

The headline observation: **Context Graph already produces both ΔS and ΔC by-product** of doing retrieval. We do not need new embedders for ΔS/ΔC. We need a new readout layer that interprets the existing 13-embedder fingerprint as the UTL v2 surprise + coherence pair.

---

## 1. ΔS — Surprise

### 1.1 UTL v2 definition

```
ΔS = min(1.0,
         max(|predicted − actual|, γ · |predicted − simulated|)
         + 0.5 · exploration_rate)
```

Where γ ≈ 0.7 is the rehearsal-fidelity discount and `predicted` is the learner's expected outcome (text, action, image — task-dependent).

Three quantities to compute: `|predicted − actual|`, `|predicted − simulated|`, and `exploration_rate`.

### 1.2 Computing `|predicted − actual|` with embedders

For text-output learners (e.g., a student answering a question), this is a cosine distance:

```
|predicted - actual| := 1 - cos(φ(predicted), φ(actual))
```

Where φ is one of Chris's existing embedders. The right choice depends on what is being predicted:

| Domain | Recommended embedder | Slot |
|---|---|---|
| factual recall | E1 (e5-large-v2 semantic) | existing |
| code | E7 (Qodo-Embed-1-1.5B) | existing |
| causal/conditional reasoning | E5 (nomic causal, asymmetric) | existing |
| multi-token long answer | E12 (ColBERTv2 MaxSim) | existing |
| typo-robust input | E9 (HDC) | existing |
| keyword overlap | E6 (sparse) ⊕ E13 (SPLADE) | existing |

**Multi-embedder combination.** Following DDA, the right thing to do is compute the prediction error in *every* embedder space and take the panel-level disagreement as the signal. The simplest aggregator is mean cosine distance:

```
ΔS_panel = (1/N) · Σ_m (1 - cos(φ_m(predicted), φ_m(actual)))
```

Where N = 13 (Context Graph default). This is robust because each embedder catches different kinds of errors — E1 catches semantic drift, E5 catches wrong-direction causal claims, E7 catches code-specific errors, E9 catches typo-tolerant errors, E6/E13 catches lexical mismatches.

Even better: weighted average using **TCT-style per-domain weight profile** (Chris already has 14 built-in weight profiles for retrieval).

### 1.3 Computing `|predicted − simulated|` (the new term)

`simulated` is the prediction the learner made during vivid first-person mental rehearsal. Two ways to capture it:

**Way 1: ask the learner.** Have them speak or type their imagined response *before* attempting the task. Embed this through the same panel. The mental-rehearsal `|predicted - simulated|` is then the cosine distance between two text/embedding objects, identical to the previous case.

**Way 2: infer from voice/face during silent rehearsal.** Subvocalisation produces measurable EMG and facial-AU signatures. Detection accuracy is poor for content but good for the *fact* that rehearsal occurred. Combined with self-reported imagery vividness (a Vividness of Movement Imagery Questionnaire, VMIQ-2 score, 1–5), this gives a γ-style fidelity factor without needing the rehearsed content itself.

**Recommendation.** For practical use, ask the learner — Way 1 is cheaper, more accurate, and matches how Anki users already work. The user types/speaks their answer, then sees the correct answer, the system computes both `|predicted - actual|` (after seeing) and `|predicted - simulated|` (their first attempt before seeing).

### 1.4 Computing `exploration_rate`

Self-reported (slider 0–1) or inferred from session metadata (% of attempted tasks that were novel vs. previously seen, normalised across recent history). The latter is computable from Chris's existing audit log.

### 1.5 The full ΔS

```
ΔS = min(1.0,
         max(ΔS_panel, γ · ΔS_panel_simulated)
         + 0.5 · exploration_rate)
```

This is a single MCP-tool callable: `compute_delta_s(predicted_text, actual_text, simulated_text=None, exploration_rate=None)` returning a scalar.

### 1.6 Caveats from the surprisal-as-cognitive-model literature

The surprisal-from-larger-LLMs-poorer-fit-to-reading-times finding (TACL 2023; arXiv 2506.11338) means we should **not** assume a bigger LM gives a better prediction-error proxy. The systematic deviations of larger models (underpredicting reading times of named entities; overpredicting function words) suggest mid-sized embedders (E1's 1024-D e5-large-v2) are closer to human-aligned prediction error than GPT-4-scale models. This is a happy coincidence: Chris's 13-embedder panel is already in the right size regime.

---

## 2. ΔC — Coherence

### 2.1 UTL v2 definition

```
ΔC = min(1.0,
         1/(1 + CV) + 0.3 · gradient_effectiveness + 0.2 · HRV_coherence)
```

Where CV = σ(recent_outcomes) / μ(recent_outcomes), `gradient_effectiveness` is the slope of recent improvement, and `HRV_coherence` is the spectral-resonance term covered in file 02.

Three sub-terms.

### 2.2 Outcome-stability (1/(1+CV))

This requires a sequence of recent outcomes (binary correct/incorrect, or a graded score in [0,1]). Computable directly from session logs, no embedder needed — but the **embedder enhancement** is to use cosine distance from the current goal centroid as a more sensitive "score" than binary correct/incorrect:

```
score_t := cos(φ(answer_t), φ(goal_centroid))      # finer-grained than binary
CV       = σ(score_{t-N..t}) / μ(score_{t-N..t})
ΔC_outcome = 1 / (1 + CV)
```

Goal centroid here is computed in TCT-style: the L2-normalised mean of "correct expert answers" for the topic. For a curriculum, this centroid is built once per learning unit.

### 2.3 Gradient-effectiveness

Slope of recent score trajectory:

```
gradient_effectiveness = clamp(slope(score_{t-N..t}) · scale, 0, 1)
```

Where `scale` normalises slope-per-step to a meaningful [0,1] range (calibrate per cohort).

### 2.4 HRV_coherence

Covered in file 02. Read directly from PaPaGei (E17). The 0.10-Hz spectral resonance is a feature of the PPG embedding decoder; output is in [0, 1].

### 2.5 The new term: multi-embedder agreement (recommended addition for v2.1)

UTL v2 doesn't include this, but it falls out naturally from the DDA panel. **Multi-embedder agreement** is a principled measure of "how much the new fact integrates with everything we already know":

```
agreement_panel = pairwise_mean_cosine(φ_m(answer_t)) over m ∈ {E1, ..., E13}
```

When the 13 embedders agree on where the answer falls in their respective spaces, the answer is *coherently related* to existing knowledge structure. When they disagree (e.g., E1 places it close to a known concept but E5 places it in a contradictory causal direction), coherence is low even if the answer is "correct" by surface match.

This is exactly the bug Case 1 (Shakespeare) hit before the cleanup: the model's outputs scored high on E1 (semantic similarity) but flagged E4 (position-ordering) and E12 (token-level MaxSim) as anomalous because of the play-script-header echo. **The multi-embedder coherence check caught it.** UTL v2 should adopt the same machinery.

Suggested form for v2.1:

```
ΔC = min(1.0,
         1/(1+CV) +
         0.2 · gradient_effectiveness +
         0.2 · HRV_coherence +
         0.2 · panel_agreement)                       # new
```

### 2.6 The contradiction signal (using E5's asymmetric causal axis)

Chris's E5 (nomic-embed-text fine-tuned for causal direction) stores dual cause/effect vectors. A learner's answer can be cosine-similar in cause-space but distant in effect-space — i.e., their causal model is wrong even though they got the surface words right.

Define a **contradiction term**:

```
contradiction(answer) = max(0, cos(answer_cause, goal_effect) - cos(answer_cause, goal_cause))
```

When this is positive, the learner has the relationship reversed (right concepts, wrong causal direction). This is a high-precision diagnostic that no other embedder catches as cleanly.

Add to ΔC as a penalty:

```
ΔC = ΔC - λ · contradiction(answer)        # λ ≈ 0.3 default
```

### 2.7 Recommended ΔC operational form

```
ΔC = clamp(0,
           1/(1+CV) +
           0.2 · gradient_effectiveness +
           0.2 · HRV_coherence +
           0.2 · panel_agreement -
           0.3 · contradiction,
           1.0)
```

Five subterms. Each has a concrete embedder readout. Each can be plotted independently for diagnostic purposes.

---

## 3. The unified prediction-error / coherence pipeline

For each learner answer, compute *both* ΔS and ΔC from a single forward pass through the 13-embedder panel:

```python
def compute_DS_DC(predicted: str, actual: str, simulated: str | None,
                  exploration_rate: float, recent_scores: list[float],
                  HRV_coherence: float, goal_centroid: dict[str, np.ndarray]):
    # One forward pass per embedder per text
    embeds_pred = {m: φ_m(predicted) for m in EMBEDDERS}
    embeds_act = {m: φ_m(actual) for m in EMBEDDERS}
    embeds_sim = {m: φ_m(simulated) for m in EMBEDDERS} if simulated else None

    # ΔS: prediction error
    delta_s_panel = mean(1 - cos(embeds_pred[m], embeds_act[m]) for m in EMBEDDERS)
    delta_s_sim = mean(1 - cos(embeds_pred[m], embeds_sim[m]) for m in EMBEDDERS) if embeds_sim else 0
    delta_s = min(1.0, max(delta_s_panel, 0.7 * delta_s_sim) + 0.5 * exploration_rate)

    # ΔC: coherence
    score_t = mean(cos(embeds_act[m], goal_centroid[m]) for m in EMBEDDERS)
    cv = std(recent_scores) / (mean(recent_scores) + 1e-6)
    grad_eff = clamp(slope(recent_scores) * scale, 0, 1)
    panel_agreement = pairwise_mean_cosine([embeds_act[m] for m in EMBEDDERS])
    contradiction = max(0, cos(embeds_act["E5_cause"], goal_centroid["E5_effect"])
                          - cos(embeds_act["E5_cause"], goal_centroid["E5_cause"]))
    delta_c = clamp(1/(1+cv) + 0.2*grad_eff + 0.2*HRV_coherence
                    + 0.2*panel_agreement - 0.3*contradiction, 0, 1)

    return delta_s, delta_c
```

Single pipeline, one forward pass per embedder per text, returns both terms.

---

## 4. AI-side ΔS / ΔC (preview of file 05)

The same operations work for measuring AI-system learning. For an AI:

- `predicted` = model output before training step
- `actual` = ground-truth target
- `ΔS_panel` = prediction error in the embedding space of the model itself (residual stream representations) plus an external embedder panel for cross-model triangulation
- `panel_agreement` = CKA / SVCCA between successive checkpoints
- `contradiction` = directional consistency between the model's claimed causal structure and a reference causal graph

File 05 develops this in detail. The point here is that **the formulae are identical**; only the embedder choices change.

---

## 5. The exploration-rate term: why 0.5 is the right coefficient

UTL v2 uses `+ 0.5 · exploration_rate` in ΔS to give exploration a fixed contribution. This is the right form (additive, not multiplicative) because exploration adds learning *opportunity* even when prediction-error is low (familiar territory + intentional novelty = useful exposure).

The 0.5 coefficient is a default. Empirical calibration should target: when exploration_rate = 1.0 and prediction_error = 0, ΔS should be ~0.5 (some learning, but less than under high prediction-error). When exploration_rate = 0 and prediction_error = 1.0, ΔS = 1.0 (max learning). When exploration_rate = 1.0 and prediction_error = 1.0, ΔS = 1.0 (clamp bites).

This matches the intuition that the *minimum* of (challenge, novelty) is what determines whether a session contributes to learning, while their max determines the cap.

---

## 6. Practical example: a learner attempting a SAT vocabulary question

```
Question: "What does 'ephemeral' mean?"
Goal centroid: φ_m("lasting only a short time; transitory")

Step 1 (rehearsal, before answer revealed):
  Learner says: "It means something temporary, like a ghost or whisper"
  predicted = embed_panel("It means something temporary, like a ghost or whisper")
  
Step 2 (feedback, after answer revealed):
  actual = embed_panel("lasting only a short time; transitory")
  
Step 3:
  ΔS_panel = 0.31 (E1 puts these close; E6 sparse keyword has them further apart due to "ghost" / "whisper")
  γ · ΔS_simulated = 0 (no separate simulated; rehearsal == prediction here)
  exploration_rate = 0.2 (familiar test format)
  ΔS = min(1.0, 0.31 + 0.5 · 0.2) = 0.41

Step 4 (coherence):
  score_t = 0.78 (high cosine to goal centroid: learner has the concept)
  CV = 0.15 (recent scores 0.7, 0.8, 0.78, 0.75, 0.78 — stable)
  ΔC_outcome = 1/(1+0.15) = 0.87
  HRV_coherence = 0.62 (PaPaGei reading)
  panel_agreement = 0.84 (13 embedders converge)
  contradiction = 0 (correct semantic direction)
  gradient_effectiveness = 0.4
  ΔC = clamp(0.87 + 0.2·0.4 + 0.2·0.62 + 0.2·0.84 - 0.3·0, 0, 1) = clamp(1.24, 0, 1) = 1.0

Step 5 (with HRV good and ΔE = 0.7):
  L = 0.41 · 1.0 · 0.7 = 0.29
  → Optimal-low; deliver next item with slightly higher difficulty.
```

The above is the intended runtime behaviour. Each scalar is observable; each is sourced from a frozen embedder; the whole pipeline runs in ~50 ms on a 5090 (13 embedder forward passes are batchable).

---

## 7. What this gives Chris that he doesn't already have

Chris's Context Graph already runs the 13 embedders for retrieval. This file specifies a **second use** of the same fingerprints: as a per-moment ΔS/ΔC reading for any learner (or training run) interacting with the system.

The required additions are minimal:

- Two new MCP tools: `compute_delta_s` and `compute_delta_c`. Both run on existing fingerprints; no new model loads.
- One new column family: `CF_GOAL_CENTROIDS` (a TCT-style per-topic "expert" centroid that ΔC compares against).
- One new background task: maintain a per-learner sliding window of recent scores for the CV / gradient calculations.

This is a small extension that turns a retrieval system into a learning-measurement system without re-architecting anything.

---

## Sources

- [Context Graph 05 Embedding Subsystem](../../../contextgraph/docs2/contextgraphspecs/05_embedding_subsystem.md) — Chris's E1–E13 stack
- [Context Graph 06 Search and Fusion](../../../contextgraph/docs2/contextgraphspecs/06_search_and_fusion.md) — RRF, weight profiles
- [Context Graph 18 Contrastive and Typed Edges](../../../contextgraph/docs2/contextgraphspecs/18_contrastive_and_typed_edges.md) — anomaly detection, contradiction signals
- [Why Does Surprisal From Larger Transformer-Based Language Models Provide a Poorer Fit to Human Reading Times?](https://direct.mit.edu/tacl/article/doi/10.1162/tacl_a_00548/115371/Why-Does-Surprisal-From-Larger-Transformer-Based)
- [Surprisal from Larger Transformer-based Language Models Predicts fMRI Data More Poorly](https://arxiv.org/html/2506.11338v1)
- [Perplexity for LLM Evaluation](https://www.geeksforgeeks.org/nlp/perplexity-for-llm-evaluation/)
- [Centroids Matching: an efficient Continual Learning approach operating in the embedding space](https://openreview.net/pdf?id=7gzQltQSwr)
- [Centroid-Based Memory Mechanisms (Emergent Mind)](https://www.emergentmind.com/topics/centroid-based-memory-mechanisms)
- [Holistic Continual Learning under Concept Drift with Adaptive Memory Realignment](https://arxiv.org/html/2507.02310v1)
- [PAPER.md TCT framework](../PAPER.md) — Phase 1 centroid construction, Phase 3 guard predicate
