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
tags: construct, definition, meaning-compression, locked, q05
answers_questions: [Q05]
---

# Construct 2 — Meaning Compression

**Glossary anchor.** Tier A, entry #2 (`canonical-glossary.md`). This file is the
paper-ready expansion of that entry; the glossary wins on any disagreement.

**Evidence tier.** T4 (taxonomy, unit analysis). Supporting citations for the three adjacent
categories are T1 / T4 in `_evidence_01_data_wall.md`.

**Binding question.** Q05 from `essential-questions.md`.

**Forbidden substitute.** "Semantic compression" (rule V5). Always "meaning compression",
lower-case, two words, never hyphenated.

---

## 2.1 Conceptual definition

**Meaning compression** is a compression category that increases the number of
information-theoretically-structured **labeled supervisory signals per unit of raw
(untokenized, un-processed) data**, by projecting the raw data through $N$ independently-trained
frozen embedders (glossary #1 and `dda-definition.md`). It is a **category**, not a single
algorithm: any procedure that decomposes a fixed corpus along a panel of frozen measurement
axes and extracts a structured-signal yield exceeding the size of the original corpus
realises meaning compression. The "compression" is in the ratio (signals per unit of raw
data), not in bits per byte of the raw data itself.

Meaning compression is **a fourth, orthogonal category** alongside three pre-existing
categories in the ML systems literature: bit compression, weight compression, and activation
compression. It occupies a different axis of the compression landscape because its
input–output units are dimensionally distinct from the other three.

---

## 2.2 Operational definition

### 2.2.1 The compression ratio

Let $D$ be a fixed raw corpus. Let $\mathrm{raw}(D)$ denote the "unit amount" of raw data
(e.g., number of inputs $n$, or a chosen unit such as one 16-minute interview). Let
$\mathrm{signals}(D, \Phi)$ denote the count of structured supervisory signals produced from
$D$ under embedder panel $\Phi$, via the DDA operational definition (§1.2):

$$
\mathrm{signals}(D, \Phi) \;=\; |D'| \;=\; n \cdot \Bigl(N + \binom{N}{2}\Bigr).
$$

Define the **meaning-compression ratio**:

$$
\mathrm{MC}(\Phi, D) \;\triangleq\; \frac{\mathrm{signals}(D, \Phi)}{\mathrm{raw}(D)} \;=\; N + \binom{N}{2}
\;\text{ (per-input units) } \;=\; \frac{N(N+1)}{2}.
$$

When $\mathrm{raw}(D) = n$, the ratio collapses to $N + \binom{N}{2}$ signals per input. For
Context Graph's $N=13$ panel: $\mathrm{MC} = 91$ (T4 count; T2 instantiation). This is the
**unit of measurement** of meaning compression: labeled signals per unit of raw data, holding
the raw corpus constant.

### 2.2.2 Why this is a compression operation

Conventional compression asks: "For a fixed unit of semantic content, how few bits, weights,
or activations can I use?" Meaning compression flips the question: "For a fixed amount of
raw data, how many information-theoretically-structured supervisory signals can I extract?"
Formally, if we hold the input corpus $D$ constant (so the information content of the raw
data is held constant in an information-theoretic sense), then any increase in
$\mathrm{signals}(D, \Phi)$ is an increase in **information gain per unit of raw data**.

Under the approximate-independence assumption (V11), each added non-redundant embedder
$\phi_{N+1}$ contributes additional bits of mutual information with the downstream target,
so that the per-input yield grows and the ratio $\mathrm{MC}$ grows. Thus meaning compression
increases structured-signal density against a held-constant raw-data baseline; the act of
"compressing" is the act of packing more supervisory signal into the same raw corpus by
reading it through more instruments. This is distinct from bit / weight / activation
compression, which reduce a resource at fixed content; meaning compression increases a
signal count at fixed raw-data volume.

### 2.2.3 Indicator equation (paper-ready)

$$
\boxed{\;\mathrm{MC}(\Phi, D) \;=\; N + \binom{N}{2} \;=\; \frac{N(N+1)}{2}\;\text{ signals / input, at fixed }D\;}
$$

First use in §3.1 of the manuscript must present this equation alongside the four-category
taxonomy table (§2.3) for the orthogonality claim to be legible.

---

## 2.3 The four-category compression taxonomy (§3.5 / §3.1 table)

The four categories are distinguished by **units**. The unit columns are dimensionally
distinct — this is the novelty delta, per P07.

| Category | Input unit | Output unit | Ratio / dimension | Typical techniques | Canonical references |
|---|---|---|---|---|---|
| **(i) Bit compression** | bits of raw data | bits of compressed data | bits / bits (dimensionless) | gzip, zstd, PNG, FLAC, Huffman, arithmetic coding | Shannon (1948); standard codecs |
| **(ii) Weight compression** | trainable parameters / accuracy at those parameters | reduced-parameter accuracy | accuracy / parameters | GPTQ (Frantar et al., 2023), SparseGPT (Frantar & Alistarh, 2023), distillation (Hinton et al., 2015) | `_evidence_01_data_wall.md` |
| **(iii) Activation compression** | peak memory / output quality | reduced-memory output quality | memory (bytes) / output quality | KV-cache quantization, TurboQuant (cited in `_evidence_01_data_wall.md`), FlashAttention memory reuse | `_evidence_01_data_wall.md` |
| **(iv) Meaning compression (this paper)** | unit of raw data ($n$ inputs) | structured labeled signals | signals / unit raw data | DDA (§3.1 / §3.4) | this manuscript; glossary #2 |

**Orthogonality claim (narrow, defensible, per P07).** The four categories operate on
dimensionally-distinct unit pairs. A system can realise bit compression, weight compression,
activation compression, **and** meaning compression simultaneously, because the four
categories trade resources along different axes of the compression landscape. Meaning
compression does not replace the other three; it adds a fourth axis along which a pipeline
can be compared.

**What orthogonality does NOT mean.** We do not claim the four categories are
information-theoretically independent as mathematical objects. We claim their unit
dimensions (and therefore their design trade-offs) are distinct, and that optimising one
does not by itself produce any compression in the others.

---

## 2.4 Relationship to bit / weight / activation compression

- **Relationship to bit compression.** Bit compression reduces storage per fixed content.
  Meaning compression increases labeled-signal count per fixed raw corpus. A gzip of a
  corpus loses no information and produces no additional signals; DDA of the same corpus
  adds structured signals but does not shrink the raw data on disk. Both can be applied to
  the same corpus independently.
- **Relationship to weight compression.** Weight compression targets a trained model's
  parameter count / memory footprint. Meaning compression targets the training-signal
  density before a model is trained. They are orthogonal in the pipeline: meaning
  compression changes the **training data**, weight compression changes the **model**.
- **Relationship to activation compression.** Activation compression reduces the peak memory
  consumed during forward / backward passes (TurboQuant, KV-cache quantisation). Meaning
  compression changes which supervisory signals drive training; activation compression
  changes which numbers must be held in memory while training. Distinct axes.

None of the three prior categories substitutes for meaning compression. A model that is
aggressively weight- and activation-compressed but trained on a corpus without DDA-style
signal densification still faces the data-wall ceiling (glossary #26). A corpus put through
DDA is independent of whether the trained model is later weight- or activation-compressed.

---

## 2.5 Variables and indicators

| Symbol / name | Role |
|---|---|
| $\mathrm{raw}(D)$ | Chosen unit of raw data (e.g., number of inputs $n$, or one 16-minute interview) |
| $\mathrm{signals}(D, \Phi)$ | Structured-signal count from DDA (§1.2) |
| $\mathrm{MC}(\Phi, D)$ | Meaning-compression ratio |
| $N$ | Number of frozen embedders in the panel |
| $N + \binom{N}{2}$ | Per-input meaning-compression ratio, at fixed $D$ |

**Primary indicator.** $\mathrm{MC}(\Phi, D)$ in units of "structured labeled signals per
unit of raw data". Reported for each system (Context Graph: 91 at $N=13$; ClipCannon: 28 at
$N=7$, from $7 + \binom{7}{2} = 7 + 21 = 28$).

**Secondary indicator (future work, per Q20 (i)).** Mutual-information-adjusted MC:
$\mathrm{MC}_{\mathrm{MI}}(\Phi, D) = \sum_m I(\phi_m(X); Y) + \sum_{j<k} I(\rho_{jk}(X); Y \mid
\phi_{1:j-1}, \phi_{j+1:k-1}, \phi_{k+1:N})$ for a target $Y$ of interest. Measuring this
would replace the count-form upper bound with an information-theoretic lower bound. Not
claimed in this manuscript.

---

## 2.6 What meaning compression is NOT (glossary #2's NOT clause, expanded)

- **NOT weight compression.** GPTQ / SparseGPT / distillation reduce parameters at fixed
  task accuracy. Meaning compression leaves the model alone and changes the training-signal
  density on fixed raw data.
- **NOT activation compression.** KV-cache quantisation / TurboQuant reduce runtime
  activation memory. Meaning compression is a data-side operation, not a runtime-memory
  operation.
- **NOT bit compression.** gzip / PNG / FLAC preserve information content and reduce bytes
  on disk. Meaning compression adds structured signals; bytes on disk are unchanged or
  larger (the derived dataset is larger than the raw corpus).
- **NOT semantic compression (IR / representation-learning sense).** In IR and
  representation learning, "semantic compression" often refers to mapping text to a
  lower-dimensional embedding while preserving task-relevant semantic distinctions. That is
  a sub-case of activation / representation compression, not the category named here. V5
  forbids the term "semantic compression" in this manuscript; use "meaning compression".
- **NOT summarisation.** Summarisation reduces content length; meaning compression increases
  structured-signal density at fixed content.

---

## 2.7 Usage rule (V5, V9)

- Always "meaning compression", lower-case, two words, not hyphenated.
- First use in §1 and §3.1: spell out. No acronym is assigned (the glossary intentionally
  does not create "MC" as an acronym; the ratio $\mathrm{MC}$ in equations is the single
  exception).
- First use in §3.1 must state the four-category orthogonality explicitly (glossary #2
  usage rule; this file §2.3 is the source of the table).
- Never "semantic compression" (V5).
- Never hyphenate ("meaning-compression" is wrong in body prose).

---

## 2.8 Cross-references

- **Glossary #2** — canonical entry and NOT clause (authority).
- **DDA definition** (`dda-definition.md`) — meaning compression is the ratio view of DDA.
- **Q05** (essential-questions.md) — binding question.
- **Q04** — DDA count feeds this ratio.
- **`_evidence_01_data_wall.md`** — GPTQ, SparseGPT, TurboQuant references for the three
  adjacent categories.

---

## Change log

- 2026-04-16 — Initial lock by agent-04-construct-definer.
