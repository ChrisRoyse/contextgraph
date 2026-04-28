---
namespace: decisions
agent_id: agent-04-construct-definer
timestamp: 2026-04-16T16:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
tags: construct, definition, dda, derived-data-abundance, locked, q04
answers_questions: [Q04]
---

# Construct 1 — Derived Data Abundance (DDA)

**Glossary anchor.** Tier A, entry #1 (`canonical-glossary.md`). This file expands that entry
into a paper-ready definition block. The glossary entry is the authority for spelling and
"what it is / is not"; this file is the authority for the formal derivation and indicators.
No contradiction with the glossary is permitted; if one is found, the glossary wins and this
file is re-issued.

**Evidence tier.** T4 (theoretical derivation) for the $N + \binom{N}{2}$ count; T2
(constructive) for the $N=13$ instantiation in Context Graph.

**Binding question.** Q04 from `essential-questions.md`.

---

## 1.1 Conceptual definition (prose, for a first-pass cs.LG reviewer)

**Derived Data Abundance (DDA)** is a data-enrichment principle. Given a corpus of raw
inputs and a fixed panel of independently-trained frozen embedding models ("embedders"),
DDA treats every embedder as a distinct measurement instrument that decomposes each input
along a different perceptual or semantic axis. For a corpus of $n$ inputs and a panel of
$N$ such embedders, DDA produces per-embedder labeled vectors and pairwise cross-embedder
interaction features without generating any new inputs. The yield is sub-quadratic in the
panel size while the underlying raw corpus is held fixed; the resulting abundance is
"derived" — it is a decomposition of existing data, not synthesis of additional data.

---

## 1.2 Operational definition (formal)

### 1.2.1 Setup

Let

- $D = \{x_i\}_{i=1}^{n} \subset \mathcal{X}$ be a fixed corpus of $n$ raw inputs, where
  $\mathcal{X}$ is the space of admissible inputs (text strings, image tensors, audio
  waveforms, etc.; $\mathcal{X}$ may be heterogeneous across modalities, in which case each
  embedder is defined on the appropriate subdomain of $\mathcal{X}$).
- $\Phi = \{\phi_m\}_{m=1}^{N}$ be a panel of $N$ **frozen** (glossary #10) embedders, with
  $\phi_m : \mathcal{X} \to \mathbb{R}^{d_m}$ (dense) or into a sparse / variable-length
  output space (e.g., SPLADE sparse codes, ColBERT per-token matrices). "Frozen" means
  parameters are fixed during the DDA pipeline and during any downstream TCT training
  (glossary #3, #10).
- Each $\phi_m$ is "independently trained" in the sense of glossary #14: trained on largely
  disjoint corpora, under different objectives, with different architectures. **Strict
  statistical independence is not claimed; we carry the V11 hedge** — "approximately
  independent; strict formal orthogonality is an open question (§10)".
- Let $\rho_{jk} : \mathbb{R}^{d_j} \times \mathbb{R}^{d_k} \to \mathbb{R}^{r_{jk}}$ be a
  **cross-embedder interaction feature** (glossary #15) between embedders $\phi_j$ and
  $\phi_k$ with $j < k$. Typical choices for $\rho_{jk}$: normalised cosine
  $\cos(\phi_j(x), \phi_k(x))$ after per-embedder L2 normalisation in a shared projection;
  pairwise mutual-information estimates; learned low-rank projections $W_{jk}
  [\phi_j(x); \phi_k(x)]$; simple concatenation followed by a frozen adapter.

### 1.2.2 The derived dataset

Apply every embedder to every input and materialise every pairwise interaction:

$$
D' = \bigl\{\bigl(x_i,\; \phi_1(x_i), \ldots, \phi_N(x_i),\;
      \bigl(\rho_{jk}(\phi_j(x_i), \phi_k(x_i))\bigr)_{1 \le j < k \le N}\bigr)\bigr\}_{i=1}^{n}.
$$

### 1.2.3 The signal count

Count the number of structured supervisory signals $|D'|$:

$$
|D'| \;=\; n \cdot \Bigl(N + \binom{N}{2}\Bigr) \;=\; n \cdot \frac{N(N+1)}{2}.
$$

- $n \cdot N$ per-embedder labeled vectors (one per (input, embedder) pair).
- $n \cdot \binom{N}{2}$ pairwise cross-embedder interaction features (one per
  (input, embedder-pair) triple with $j < k$).

Total per-input yield: $N + \binom{N}{2} = \frac{N(N+1)}{2}$ signals per input.

**Worked instantiation — Context Graph (T2 constructive).** For $N = 13$ (E1–E13 in the
Context Graph panel, glossary #5 and mission brief "Key Architecture Facts"):

$$
13 + \binom{13}{2} \;=\; 13 + 78 \;=\; 91 \quad \text{signals / input.}
$$

This is the "N=13 → 91 signals/input" figure cited in core claim 1 of the mission brief.
The 91 is a **count**, not a performance claim; labeling it T4 for the derivation and T2
for the Context Graph instantiation satisfies the evidence-tier discipline (P04).

### 1.2.4 Independence / approximation assumptions (made explicit)

DDA's multiplicative-gain claim holds under the following conditions; each is reviewer-facing
and is carried forward to §8 Limitations:

1. **Approximate independence of embedders.** Embedders in $\Phi$ are drawn from different
   base architectures, trained on largely disjoint corpora, for different objectives
   (e.g., contrastive sentence embedding, masked language modelling, speaker-ID, visual
   classification). Strict statistical independence is not claimed. If two embedders
   collapse to a linear reparameterisation of each other, their pair contributes no new
   information and the $\binom{N}{2}$ count is an upper bound.
2. **Non-degenerate interaction features.** Each $\rho_{jk}$ is a non-constant function of
   its arguments. Constant-valued $\rho_{jk}$ contribute no signal.
3. **Fixed corpus.** DDA enlarges the labeled-signal count on a fixed corpus $D$. The claim
   is **not** "DDA expands the distribution covered by $D$"; it is "DDA exposes more
   structure of the distribution already covered by $D$".
4. **Frozen embedders only.** If an embedder is updated during training, it is no longer a
   frozen measurement instrument; the independence argument collapses to self-distillation
   (glossary #6 / #10). V18 enforces the terminology: "frozen" or "frozen-with-LoRA", never
   an ambiguous "pretrained" where "frozen" is meant.

### 1.2.5 Boundary conditions of the multiplicative-gain claim

The $n \cdot \frac{N(N+1)}{2}$ count is dimensionally a **count of signals**, not a count of
bits of mutual information. Two signals may be formally present while carrying near-zero
additional information beyond a previously counted signal. The gain is therefore **upper-bounded**
by the count and **lower-bounded** by the minimum of per-signal mutual information across
embedders. We state the count as the structural upper bound; we do not claim that every
pair $\rho_{jk}$ is information-theoretically non-redundant. This is precisely the open
question flagged in Q20 (iv) — formal embedder-independence measurement is future work.

---

## 1.3 Variables and indicators

| Symbol / name | Role | Example in this manuscript |
|---|---|---|
| $D, n$ | Fixed corpus and its cardinality | Context Graph: the Rust workspace's ingested memory documents; ClipCannon: 2,362 curated clips from 16 min of source |
| $\Phi, N$ | Frozen embedder panel and its cardinality | Context Graph: $N=13$ (E1–E13); ClipCannon: $N=7$ named modalities |
| $\phi_m, d_m$ | Individual embedder and its output dim | E1 at 1024D; E7 at 1536D; E12 ColBERT 128D/token |
| $\rho_{jk}$ | Pairwise cross-embedder interaction feature | Normalised cosine $\cos(\phi_j(x), \phi_k(x))$ after per-embedder L2 normalisation |
| $D'$ | The derived dataset of structured supervisory signals | $\|D'\| = n \cdot (N + \binom{N}{2})$ |
| $n \cdot N$ | Per-embedder signals | 13 dense embedder outputs per input in Context Graph |
| $n \cdot \binom{N}{2}$ | Pairwise interaction signals | 78 pairwise cosines per input in Context Graph |

**What counts as a "signal"?** A scalar or low-dimensional feature that (a) is a deterministic
function of the raw input under frozen embedders and frozen interaction rules, (b) is
attached to a specific (input, embedder) or (input, embedder-pair) index, and (c) can be
used as either an input or a target in a downstream loss without requiring any new data
collection.

**What makes an embedder "independent"?** Operationally, two embedders are treated as
approximately independent if (i) their base architectures differ, (ii) their pretraining
corpora overlap less than some threshold (not enforced here; measured in future work per
Q20), (iii) their objectives differ (e.g., contrastive vs. MLM vs. audio-classification), and
(iv) they produce embeddings in distinct output spaces. We carry V11's hedge at every use of
"orthogonal".

**Boundary conditions for the multiplicative gain.** The gain saturates as embedder
correlations rise; it is an upper bound, not a guaranteed floor. See §1.2.5 and §8
Limitations.

---

## 1.4 What DDA is NOT (matching glossary #1's "NOT" clause, verbatim structure)

DDA is **not**:

- **synthetic data generation.** DDA generates no new inputs; it decomposes existing ones.
- **data augmentation.** Augmentations (crops, noise, mixup, paraphrase) perturb the input
  and hope the label survives; DDA leaves the input unchanged and projects it through $N$
  fixed instruments.
- **distillation.** Distillation teaches a student to mimic a teacher; DDA does not involve
  a teacher–student relationship. Embedders are measurement instruments, not teachers.
- **self-training / pseudo-labelling.** Self-training uses model-predicted labels to expand
  a training set; DDA's signals are not pseudo-labels, they are direct outputs of frozen
  instruments.
- **active learning.** Active learning selects which inputs to label next; DDA does not
  change the input set.
- **multi-task supervision** in the loose sense. Multi-task setups usually imply
  task-specific heads that are trained jointly; DDA's signals come from frozen embedders
  whose heads are not updated.

A writer who is about to type any of the forbidden phrases above in a passage about DDA
must consult V6 first and switch to "Derived Data Abundance (DDA)".

---

## 1.5 Usage rule (V9, V11 re-stated)

- First use in §1, §3, §5, §6: write "**Derived Data Abundance (DDA)**".
- Thereafter in the same section: "DDA".
- Within two sentences of the first section-level use, write the phrase "not synthetic
  data" OR include a pointer to §3.3 where the distinction is drawn (V6).
- First use of "orthogonal" (applied to signals) carries the V11 hedge: "approximately
  independent; strict formal orthogonality is an open question (§10)".
- Count-form expressions use $\binom{N}{2}$ explicitly (glossary #15 usage rule).

---

## 1.6 Cross-references

- **Glossary #1** — canonical spelling and "NOT" clause (authority for terminology).
- **Glossary #14** — orthogonality / independence hedge.
- **Glossary #15** — cross-embedder interaction feature definition.
- **Q04** (essential-questions.md) — binding question for this construct.
- **Q16** — embedder-non-independence threat; lives in §8 Limitations.
- **Q20** (i) — formal independence measurement is future work.
- **R01** (ambiguity-resolutions: DDA vs. synthetic data) — already settled.
- **`_evidence_01_data_wall.md`** — cite Villalobos et al. (2024) on the data wall and
  Shumailov et al. (2024) on model collapse when distinguishing DDA (§3.3).

---

## Change log

- 2026-04-16 — Initial lock by agent-04-construct-definer.
