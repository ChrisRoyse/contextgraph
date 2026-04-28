---
namespace: decisions
agent_id: agent-04-construct-definer
timestamp: 2026-04-16T16:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
tags: construct, definition, tct, teleological-constellation-training, locked, q06, q07
answers_questions: [Q06, Q07]
---

# Construct 3 — Teleological Constellation Training (TCT)

**Glossary anchor.** Tier A, entry #3 (`canonical-glossary.md`). This file is the
paper-ready expansion. Glossary wins on disagreement.

**Evidence tier.** T4 (method specification); T2 (constructive — the ConstellationGuard
code artifact exists in Context Graph / Phoenix).

**Binding questions.** Q06 (definition + contrast with RLHF/DPO/CAI) and Q07 (three-phase
protocol) from `essential-questions.md`.

---

## 3.1 Conceptual definition

**Teleological Constellation Training (TCT)** is a training method for generative models in
which the training target is a **frozen** (glossary #10), geometrically-specified goal — a
set of per-modality centroids (glossary #7) called a **constellation** (glossary #8) — and
the per-output acceptance criterion at inference is a bank of cosine-similarity thresholds
(glossary #11) scored against that same constellation by the same frozen embedders
(glossary #5). TCT is **teleological** in the philosophical sense that the aim (the
constellation) is a fixed, pre-specified end state, not a learned reward or a distribution
matched in expectation — every generated output is directly compared to the goal at
inference via the constellation guard (glossary #12).

Informally: TCT says "here is a geometric shape the output must land inside" and enforces
that shape during training via distance loss and at inference via a per-output cosine
check. The shape itself — the constellation — is constructed ahead of training and never
updated.

---

## 3.2 Formal specification — three-phase protocol

TCT is a three-phase protocol. Phase 1 constructs the constellation (inputs: reference
sets; output: constellation). Phase 2 trains the generator against the constellation
(inputs: constellation, generator base, DDA-enriched training data; output: fine-tuned
generator). Phase 3 runs the constellation guard at inference (inputs: generator, constellation,
thresholds; output: accepted generations).

### Phase 1 — Constellation construction

Inputs:

- A subject $P$ (e.g., a specific speaker, a specific identity, a specific authorial style).
- A set of modalities $\mathcal{M} = \{m_1, \ldots, m_M\}$ (glossary #6).
- For each modality $m$, a reference set $R_m = \{r_{m,i}\}_{i=1}^{n_m}$ (glossary #9) —
  the raw clips / samples of $P$ in modality $m$ (e.g., 50 reference audio clips in Case 3;
  2,362 visual clips in Case 2).
- For each modality $m$, a frozen embedder $\phi_m$ (glossary #5, #10).

Procedure (per modality):

$$
c_m \;=\; \mathrm{normalize}\Bigl(\frac{1}{n_m} \sum_{i=1}^{n_m} \mathrm{normalize}\bigl(\phi_m(r_{m,i})\bigr)\Bigr),
$$

where $\mathrm{normalize}(v) = v / \|v\|_2$. This is the L2-normalised mean of L2-normalised
embeddings per glossary #7. $c_m$ is the centroid for $(P, m)$; it is a single unit vector in
$\mathbb{R}^{d_m}$.

Output:

$$
C_P \;=\; \bigl\{(m, c_m) : m \in \mathcal{M}\bigr\},
$$

the **constellation** (glossary #8) for subject $P$. Cardinality: $|C_P| = |\mathcal{M}|$.
$C_P$ is frozen for the remainder of the pipeline; no gradient flows into it.

### Phase 2 — Constellation-conditioned fine-tuning

Inputs:

- A generator $g_\theta : \mathcal{Z} \to \mathcal{Y}$ (generates outputs $\hat{y} \in
  \mathcal{Y}$ from conditioning $z \in \mathcal{Z}$, e.g., a text prompt, a reference
  clip, a latent noise sample).
- The constellation $C_P$ from Phase 1.
- The frozen embedders $\{\phi_m\}$ (same instruments used in Phase 1).
- DDA-enriched training data (§1; optional but compatible) providing additional supervisory
  signals.
- Per-modality loss weights $\lambda_m \geq 0$.

Training objective:

$$
\mathcal{L}(\theta) \;=\; \mathcal{L}_{\mathrm{gen}}(\theta) \;+\; \sum_{m \in \mathcal{M}} \lambda_m \bigl(1 - \cos(\phi_m(\hat{y}), c_m)\bigr),
$$

where $\mathcal{L}_{\mathrm{gen}}$ is the generator's base loss (e.g., cross-entropy for
autoregressive models, flow-matching loss for diffusion / flow models, DPO pair loss for
preference-tuned models). The added term is the **geometric-proximity term** — a
distance-to-centroid penalty in each modality. $\cos(\cdot, \cdot)$ is cosine similarity.
$\phi_m$ is applied to the generated output $\hat{y}$ (or to a modality-specific projection
of it — e.g., for Case 3, $\phi_{\text{speaker}}$ is WavLM applied to the decoded waveform).

Training mode discipline (per V18):

- "Frozen-with-LoRA" is the default Case 1 / Case 2 regime: base model parameters are
  frozen; low-rank adapters are updated; no gradients flow into $\phi_m$ or $c_m$.
- "Inference-only" is the Case 3 regime for the Qwen3-TTS backbone: no parameter update at
  all; Phase 2 reduces to a selection procedure (the best-of-$K$ step at inference, §3.2.1
  below) rather than a training update.

Output: trained parameters $\theta^*$ (or, in inference-only mode, the unchanged generator).

### Phase 3 — Runtime constellation guard (glossary #12)

Inputs:

- The trained generator $g_{\theta^*}$.
- The constellation $C_P$.
- Per-modality thresholds $\{\tau_m\}_{m \in \mathcal{M}}$ (glossary #11).
- A conditioning input $z$ (prompt, reference, latent).

Procedure per generation:

1. Generate a candidate $\hat{y} = g_{\theta^*}(z)$.
2. For every modality $m \in \mathcal{M}$, compute the per-modality cosine
   $s_m = \cos(\phi_m(\hat{y}), c_m)$.
3. Accept iff **every** per-modality cosine meets or exceeds its threshold:
   $\hat{y} \text{ is accepted} \iff \forall m \in \mathcal{M}.\; s_m \geq \tau_m$.
4. If rejected, resample (vary latents / temperature / seed) and repeat up to a budget $K$.

This is the strict (conjunctive) acceptance predicate. A **relaxed / soft variant** that
Case 3's best-of-$K$ selector uses:

$$
\hat{y}^{*} \;=\; \arg\max_{k=1,\ldots,K} \sum_{m \in \mathcal{M}} w_m \cdot \cos(\phi_m(\hat{y}^{(k)}), c_m),
$$

i.e., draw $K$ candidates, rank by a weighted-sum score, return the argmax. The strict
predicate can be applied post-hoc to $\hat{y}^{*}$; Case 3 uses both (select via weighted
sum of WavLM-SECS across 12 candidates, then verify the selected candidate falls within the
same-session band).

Output: accepted generation $\hat{y}$ with its cosine trace $\{s_m\}$ as a per-output
provenance record.

### 3.2.1 Formal pseudocode (manuscript body / appendix — FM-06, no Python or Rust)

```
Algorithm TCT (three phases)

# Phase 1 — Constellation construction
input: subject P; modalities M; per-modality reference sets R_m; frozen embedders phi_m
for each m in M:
    for each r in R_m:
        v_{m,r} := normalize(phi_m(r))
    c_m := normalize(mean_{r in R_m} v_{m,r})
C_P := { (m, c_m) : m in M }
return C_P           # frozen constellation

# Phase 2 — Constellation-conditioned fine-tuning
input: generator g_theta; constellation C_P; frozen embedders phi_m;
       loss weights lambda_m; DDA-enriched training data D'; generator base loss L_gen
repeat until convergence:
    sample z (and target y if L_gen is supervised) from D'
    y_hat := g_theta(z)
    L := L_gen(theta; z, y, y_hat)
         + sum_{m in M} lambda_m * (1 - cos(phi_m(y_hat), c_m))
    update theta by gradient of L
return theta*        # (or return g unchanged if inference-only)

# Phase 3 — Runtime constellation guard
input: trained generator g_theta*; constellation C_P; thresholds tau_m; prompt z; budget K
for k = 1..K:
    y_hat_k := g_theta*(z; seed_k)
    s_{m,k} := cos(phi_m(y_hat_k), c_m) for each m in M
    if all m: s_{m,k} >= tau_m:
        return y_hat_k  with trace { s_{m,k} }
# relaxed/soft variant (used in Case 3):
k* := argmax_k  sum_m w_m * s_{m,k}
return y_hat_{k*}  with trace { s_{m,k*} }
```

Pseudocode only per FM-06; no Python or Rust in the manuscript body.

---

## 3.3 Why "teleological"? (the noun the method is named for)

TCT's training target is a **frozen, externally-specified geometric shape** — a fixed set of
centroids that the generator must approach in embedding space. In the Aristotelian sense,
the constellation is the method's *telos*: the final cause toward which training is
directed. This is in contrast to scalar-reward methods (RLHF, DPO, Constitutional AI) where
the aim is a learned reward signal that is itself updated during training (directly or via
preference data that shapes its gradient). In TCT:

- The **instrument of measurement** (the embedder) is frozen.
- The **target** (the centroid) is frozen.
- The **metric** (cosine similarity) is fixed.
- Only the **generator** is updated (or not, in inference-only Case 3).

The method is therefore teleological in that the end state is fixed in advance and every
training step is a step toward a geometrically-specified goal; no aspect of the goal is
learned or drifts.

---

## 3.4 Variables, indicators, and design choices

| Symbol / name | Role | Typical choice |
|---|---|---|
| $P$ | Subject (identity, style, speaker) | Case 2: the author; Case 3: the author's voice; Case 1: the Shakespeare stylistic target |
| $\mathcal{M}$ | Set of modalities | Case 3: $\{\text{speaker}\}$ with WavLM; Case 2: $\{\text{visual}, \text{speaker}\}$ with CLIP/SigLIP + WavLM; Case 1: $\{\text{text-style}\}$ |
| $\phi_m$ | Frozen embedder for modality $m$ | WavLM-SV; SigLIP; CLIP ViT-L/14; E1 sentence embedder |
| $R_m, n_m$ | Reference set and its cardinality | Case 3: $n_{\text{speaker}} = 50$ |
| $c_m$ | Centroid | L2-normalised mean of L2-normalised $\phi_m(r)$ over $R_m$ |
| $C_P$ | Constellation | $\{(m, c_m)\}_{m \in \mathcal{M}}$ |
| $\lambda_m$ | Phase-2 loss weight | Case 2: mouth-weighted (higher on speaker, lower on global visual); other modalities 0–1 |
| $\tau_m$ | Per-modality threshold | Case 3: $\tau_{\text{WavLM-SECS}}$ calibrated to the same-session band (see Q08); Cases 1–2 thresholds projected (T3) |
| $K$ | Best-of-$K$ sample budget | Case 3: $K = 12$ |
| $T$ | Sampling temperature | Case 3: $T = 0.3$ |

---

## 3.5 What TCT is NOT (glossary #3's NOT clause, expanded)

TCT is **not**:

- **RLHF** (Reinforcement Learning from Human Feedback). RLHF trains a scalar reward model
  on human preference pairs, then optimises the generator against that learned reward
  (Christiano et al., 2017; Ouyang et al., 2022). There is no fixed geometric target; the
  reward model is itself learned and applied at every step. No per-output geometric check at
  inference. TCT's reward is not scalar and not learned; it is a cosine-to-centroid distance
  against a frozen target.
- **DPO** (Direct Preference Optimization, Rafailov et al., 2023). DPO re-expresses the
  RLHF objective as a closed-form loss on preference pairs; the implicit reward is still a
  learned function of preferences. Same contrast: TCT has no preference pairs and no
  learned reward.
- **Constitutional AI** (Bai et al., 2022, Anthropic). CAI augments RLHF with
  self-generated critique and revision against natural-language rules. The rules are
  applied via a learned classifier / revision model. TCT's "rules" are frozen cosine
  thresholds against frozen centroids; no rule-revision loop.
- **Contrastive learning** (e.g., InfoNCE, SimCLR, CLIP contrastive loss). Contrastive
  methods sample negative pairs from the batch and optimise a temperature-scaled InfoNCE.
  TCT samples no negatives; the only geometric signal is positive distance to a fixed
  centroid.
- **Metric learning** (siamese / triplet / prototypical nets). Metric learning updates the
  embedding function to produce a task-discriminative metric. In TCT the embedder is
  frozen; the metric is not learned.
- **Prototypical networks** (Snell et al., 2017). Prototypical nets classify queries
  against class prototypes in a learned embedding space. TCT generates, not classifies;
  the embedding space is frozen; centroids (glossary #7) replace prototypes under V1.
- **Vanilla LoRA fine-tuning.** Standard LoRA fine-tunes on cross-entropy alone. TCT adds
  the geometric-proximity term $\sum_m \lambda_m (1 - \cos(\phi_m(\hat{y}), c_m))$ to the
  loss, and runs the constellation guard at inference.
- **Reward model with a fixed architecture.** A reward model is still a learned function on
  output features; TCT's "scorer" is an untrained cosine to a frozen point.

TCT differs from **all** of the above in at least two dimensions: (a) the aim is a frozen
geometric shape, not a learned reward; (b) every output is verified at inference by the
same frozen instrument used during training.

---

## 3.6 Contrast summary table (for §2.6 / §5 in the manuscript)

| Dimension | RLHF / DPO / CAI | Contrastive / Metric / Prototypical | Vanilla LoRA | **TCT (this paper)** |
|---|---|---|---|---|
| Aim | learned scalar reward | learned metric space | task objective (e.g., CE) | frozen constellation $C_P$ |
| Instrument | reward model (learned) | embedding function (learned) | none explicit | frozen embedders $\{\phi_m\}$ |
| Metric | scalar preference | learned distance | task loss | fixed cosine |
| Per-output runtime check | no | no | no | **yes** (constellation guard) |
| Training data | preference pairs | positive/negative pairs | task examples | task examples + centroids |
| Update target | generator + reward model | embedder + head | generator (or adapters) | generator (or adapters) only |

---

## 3.7 Usage rule (V9)

- First use (§1, §5, §6): "**Teleological Constellation Training (TCT)**".
- Thereafter: "TCT".
- Never "TCTraining" / "TC-Training" / "constellation training" alone.
- The three-phase protocol is always phrased as "Phase 1 constellation construction,
  Phase 2 constellation-conditioned fine-tuning, Phase 3 runtime constellation guard".
- First use of "alignment" in §2.6 must carry V10's geometric-vs-scalar-reward
  disambiguation; after that, "alignment" is usable unqualified within the section.

---

## 3.8 Cross-references

- **Glossary #3** (TCT) — canonical entry.
- **Glossary #8** (constellation), #7 (centroid), #9 (reference set), #10 (frozen), #11
  (per-modality threshold), #12 (constellation guard), #13 (alignment, disambiguated).
- **Q06** — TCT vs. RLHF/DPO/CAI contrast.
- **Q07** — three-phase protocol specification.
- **Q08** — threshold calibration (Case 3 gets T1 values, Cases 1–2 T3).
- **Q15** — sample-complexity collapse; TCT on top of DDA-enriched data enables it.
- **DDA definition** (`dda-definition.md`) — Phase 2's training data is DDA-enriched.
- **Identity-locked generation** (`identity-locked-definition.md`) — strict acceptance
  predicate with TCT's Phase-3 guard.

---

## Change log

- 2026-04-16 — Initial lock by agent-04-construct-definer.
