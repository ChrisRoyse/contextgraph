---
namespace: patterns
agent_id: agent-21-theory-builder
timestamp: 2026-04-17T05:05:00Z
status: active
locked: false
supersedes: none
tags: theory-scaffold, section-3, methodology-writer, paste-ready, constellation-spine, axioms, propositions, theorems
depends_on:
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/patterns/agent-17-thematic-synthesizer--thematic-frameworks.md
  - memory/patterns/agent-17-thematic-synthesizer--narrative-arc.md
  - memory/patterns/agent-20-interdisciplinary-pollinator--isomorphisms.md
downstream_consumers:
  - agent-37-methodology-writer
  - agent-38-systems-writer
  - agent-39-discussion-writer
  - agent-22-model-architect
---

# Theory for Writers — §3-Scaffold for the Methodology Writer

## How to use this file

This file is a **paste-ready §3 scaffold** for agent-37 (methodology-
writer) and anyone else constructing theoretical-framing paragraphs.
Every block below is structured so that:

1. **The spine is named first** (the constellation $C_P$).
2. **Axioms are introduced before propositions**.
3. **Propositions are introduced before theorems**.
4. **Each block carries its hedge verbatim from upstream agents.**
5. **The composition-not-invention register (MP-12) is preserved in
   every introductory clause.**

Writers should treat the scaffold as a **structural guide**, not final
prose. Smooth-prose editing is fine; re-ordering the block sequence is
not (the dependency order is load-bearing for the theorem derivations).

---

## §3 opening (theme B Framework opener; see agent-17 Theme B)

### §3.0 — one-paragraph framing

> The unifying theoretical object of this paper is the **constellation**
> $C_P = \{(m, c_m) : m \in \mathcal{M}\}$: a set of per-modality,
> L2-normalised centroids computed from reference sets of a subject $P$
> by a panel of frozen embedders. Every other contribution in the paper
> is defined in relation to this object. **Derived Data Abundance
> (DDA)** supplies the per-embedder projections from which the
> centroids are constructed. **Meaning compression** is the ratio view
> of that construction: how many informationally-structured supervisory
> signals per unit of raw data a fixed corpus yields when projected
> through $N$ frozen embedders. **Teleological Constellation Training
> (TCT)** uses the frozen constellation as its training loss target
> (Phase 2) and as its runtime acceptance predicate (Phase 3).
> **Identity-locked generation** is the per-output property TCT's
> Phase-3 guard delivers: each generation is verified to satisfy
> $\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m$. The four
> contributions are compositions of existing primitives around the
> single constellation object.

### §3.0.1 — three isomorphisms to cue the reader (ISO-02, ISO-04, ISO-06)

> The constellation carries three cross-field intuitions the reader may
> find useful. **Astronomically**, a constellation is a collection of
> fixed reference points used to orient motion in an unknown space
> (ISO-06): we choose the name deliberately to signal that $C_P$ is
> multi-reference and frozen. **In signal-processing terms**, the
> $N$-embedder panel is a bank of $N$ approximately-independent
> measurements of the same input, and DDA's counting formula
> $N + \binom{N}{2}$ is the corresponding single-plus-pairwise yield
> (ISO-02). **In information-theoretic terms** (Kolmogorov / MDL /
> rate-distortion tradition, ISO-04), meaning compression is a **ratio
> of structured-signals per unit of raw data**, complementing
> bit-compression's bits-per-symbol. None of these three is a
> mathematical-equivalence claim; each is a topology-of-mechanism
> parallel that makes the construction legible.

---

## §3.1 — Meaning compression and the four-category taxonomy

### Paste-ready paragraph (use `meaning-compression-definition.md` §2.3 table verbatim)

> Meaning compression is a **fourth compression category**, orthogonal
> by unit-analysis to bit-, weight-, and activation-compression. Its
> input unit is a unit of raw data (e.g., $n$ inputs of a fixed
> corpus); its output unit is the count of structured supervisory
> signals produced by an $N$-frozen-embedder panel. For a fixed corpus
> $D$ of cardinality $n$ under an approximately-independent embedder
> panel $\Phi$ of size $N$, the meaning-compression ratio is
>
> $$
> \mathrm{MC}(\Phi, D) \;=\; N + \binom{N}{2} \;=\; \frac{N(N+1)}{2}
> \quad \text{structured labeled signals per input at fixed } D.
> $$
>
> For Context Graph's panel ($N = 13$): $\mathrm{MC} = 91$ (T4
> derivation; T2 instantiation). For ClipCannon's panel ($N = 7$):
> $\mathrm{MC} = 28$. The taxonomy is summarised in Table 1; each
> category has dimensionally-distinct input / output unit pairs.
>
> *(Insert Table 1: bit / weight / activation / meaning compression
> taxonomy; copy from `meaning-compression-definition.md` §2.3.)*

**Hedge clause to include (Bias-10, V11).** "We do not claim the four
categories are information-theoretically independent as mathematical
objects; we claim their unit dimensions are distinct and that
optimising one does not by itself produce any compression in the
others."

---

## §3.2 — The three axioms (new §3 subsection dedicated to axiom listing)

### Paste-ready paragraph — Axiom 1

> **Axiom 1 (Grounded measurement).** A frozen embedder $\phi$ applied
> to a real input $x$ is a deterministic, reproducible function of $x$
> and the embedder's fixed parameters only. For any generator $g_\theta$
> under training, $\partial \phi(x)/\partial \theta = 0$. Measurements
> are grounded in real inputs; they do not drift as the generator's
> parameters change. *Hedge:* the axiom holds only while the embedder
> is frozen (V18); updating $\phi$ collapses downstream results to
> self-distillation.

### Paste-ready paragraph — Axiom 2

> **Axiom 2 (Approximate orthogonality).** A panel
> $\Phi = \{\phi_1, \ldots, \phi_N\}$ of frozen embedders trained on
> largely disjoint corpora, under different objectives, with different
> architectures carries approximately independent information axes for
> the target downstream predictions. Formally, we assume pairwise
> mutual-information terms $I(\phi_j(X); \phi_k(X))$ are small relative
> to the marginal entropies. *Hedge (V11):* approximately independent;
> strict formal orthogonality is an open question — formal measurement
> is future work (see §10 and RO-8).

### Paste-ready paragraph — Axiom 3

> **Axiom 3 (Reference-set learnability).** For a subject $P$ and
> modality $m$, the centroid $c_m$ computed as the L2-normalised mean
> of L2-normalised $\phi_m$-embeddings of a reference set $R_m$ is
> stable on held-out inputs for $n_m \in [10, 10^2]$, provided $\phi_m$
> is a modern frozen embedder with well-conditioned output geometry.
> *Hedge:* the axiom's sample-size bound is an engineering claim
> grounded in one T1 operating point (Case 3, $n_m = 50$; §6.3). The
> sample-complexity curve across modalities is named future work
> (RO-10).

---

## §3.3 — DDA scope argument (P2; collapse-silence)

### Paste-ready paragraph

> The generator-in-loop model-collapse theorems (Shumailov et al.,
> 2024; Dohmatob et al., 2024; Alemohammad et al., 2023) require a
> recursion in which training rounds consume samples produced by a
> generator from the previous round. DDA's operational procedure does
> not satisfy this precondition: by Axiom 1, each $\phi_m(x)$ is a
> frozen measurement of a real input $x$, independent of any
> generator's parameters. The derived dataset $D'$ is a decomposition
> of $D$, not synthesis of new inputs. The collapse theorems therefore
> do not apply to DDA. *Hedge:* this is a **categorical scope** claim,
> not an immunity theorem — we claim the preconditions are unsatisfied,
> not that DDA has a new independence-from-collapse result.

---

## §3.4 — DDA counting (P1 formal statement)

### Paste-ready paragraph

> **Proposition 1 (Signal count).** Under Axioms 1 and 2, a fixed
> corpus $D$ of $n$ raw inputs projected through a panel $\Phi$ of $N$
> approximately-independent frozen embedders produces
>
> $$
> |D'| \;=\; n \cdot \Bigl(N + \binom{N}{2}\Bigr) \;=\; n \cdot \frac{N(N+1)}{2}
> $$
>
> structured supervisory signals, where the $n \cdot N$ term counts the
> per-embedder projections and the $n \cdot \binom{N}{2}$ term counts
> the pairwise cross-embedder interaction features (see
> `dda-definition.md` §1.2.3). For $N = 13$ (Context Graph),
> $|D'| / n = 91$; for $N = 7$ (ClipCannon), $|D'| / n = 28$. The count
> is a **structural upper bound**; when pairs of embedders collapse to
> near-linear reparameterisations of each other, their pair contributes
> no new information and the effective count is strictly less than
> $n \cdot (N + \binom{N}{2})$.

### Optional analogical support (ISO-02)

> Treating frozen embedders as $N$ independent measurements of the same
> input echoes the sensor-fusion intuition that $N$ approximately-
> independent sensors yield more reliable state estimation than any
> single sensor (the $\binom{N}{2}$ term corresponds to cross-sensor
> covariance). We use this parallel for reader intuition only; we do
> not claim DDA is a Kalman filter.

---

## §3.5 — Meaning compression as the ratio view of DDA

See §3.1 block above. Writers should place this as the first
subsection of §3 (it frames the taxonomy before the counting argument
in §3.4).

---

## §3.6 — Sample-complexity framing (Theorem 1 lead-in)

### Paste-ready paragraph

> **Theorem 1 (Sample-Complexity Collapse under TCT, informal).** Let
> $P$ be a subject, $C_P = \{(m, c_m)\}_{m \in \mathcal{M}}$ a TCT
> constellation of cardinality $M$ constructed from per-modality
> reference sets of size at most $k$. Then the subject-specific sample
> complexity for identity-locked generation under TCT on DDA-enriched
> data satisfies
>
> $$
> \mathcal{T}_{\mathrm{TCT}}(P) \;=\; O(M \cdot k),
> $$
>
> **independent of the size of the general pretraining corpus $|D|$
> and independent of any subject-$P$-specific fine-tuning data beyond
> the reference sets themselves**. The proof follows by construction:
> Phase 1 uses only $\bigcup_m R_m$; Phase 2's supervisory signals come
> from DDA over the subject-independent corpus $D$; Phase 3 is
> inference-time. The non-trivial content of the theorem lies in Axiom
> 3's claim that $k$ can be taken on the order of $10^2$ — the Case-3
> T1 measurement ($n_m = 50$, mean SECS 0.961 on 10 unseen English
> sentences) is the empirical anchor for this value of $k$ in the
> speaker modality. Cases 1 and 2 remain T3-projected pending
> camera-ready re-verification.

### Hedge clauses to append

- "The cluster-E / F conventional baseline (many subject-specific
  examples + single-prompt ICL) is the comparator; the sample-
  complexity collapse is an AUTHOR-EMPIRICAL phenomenon claim, not a
  proof."
- "Case 3's anchor is single-subject, single-encoder-pair,
  English-only (agent-14 Biases 05, 06, 07); the sample-complexity
  curve across modalities and subjects is future work (RO-10, §10.1)."

---

## §5.1 — TCT theoretical framing (Theme C / E; astronomical terminology defended)

### Paste-ready paragraph — framing with ISO-06 astronomical borrow

> We name the object a **constellation** rather than a **prototype**
> (cognitive-science, typically single-object) or an **anchor**
> (retrieval, typically single pairwise reference). The astronomical
> intuition is deliberate: a constellation is a collection of **fixed
> reference points** used to **orient** motion in an unknown space, not
> a single target to optimise toward. Scalar-reward alignment
> (InstructGPT, DPO, Constitutional AI) optimises toward a single
> learned scalar — a Pole Star. TCT orients toward a **full
> constellation** of per-modality fixed references simultaneously. The
> shift from one axis to $M$ axes is both what makes per-output
> verification possible (Theorem 2) and what structurally distinguishes
> TCT from scalar-reward paradigms.

### Paste-ready paragraph — three-phase protocol one-liner

> TCT is a three-phase protocol (see Algorithm 1 and
> `tct-definition.md` §3.2): **Phase 1** constructs the constellation
> $C_P$ from per-modality reference sets; **Phase 2** fine-tunes the
> generator with a base loss plus a geometric-proximity term
> $\sum_m \lambda_m (1 - \cos(\phi_m(\hat y), c_m))$; **Phase 3** runs
> the constellation guard at inference, accepting $\hat y$ iff
> $\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m$.

---

## §5.3 — Constellation guard (P3, Theorem 2); name both RV and SPC paradigm sources

### Paste-ready paragraph — dual-attribution paradigm source (ISO-01 + ISO-07)

> The constellation guard imports two paradigm sources. From
> **formal-methods runtime verification (RV)**, we borrow the
> per-output monitor topology: specify a condition; monitor each
> execution / observation against it; reject violators. From
> **statistical process control (SPC)**, we borrow the multi-
> dimensional tolerance-bound acceptance predicate: every produced
> unit is inspected against per-dimension tolerance bounds before
> release. The guard is the composition of these two topologies
> specialised to frozen multi-modal embedder targets. We name both
> borrows explicitly because single-source attribution would weaken
> the paradigmatic reading of the move (RV alone reads as a
> deterministic-logic borrow; SPC alone reads as a quality-assurance
> borrow — the combination is the correct read for a probabilistic,
> multi-modal acceptance predicate).

### Paste-ready paragraph — Theorem 2

> **Theorem 2 (Runtime Verifiability of Identity Lock, informal).** For
> every candidate $\hat y$ produced by a TCT-trained generator, the
> identity-locked predicate
>
> $$
> G_\tau(\hat y; C_P) \;=\; \mathbb{1}\!\left[\,\forall m \in \mathcal{M} : \cos(\phi_m(\hat y), c_m) \geq \tau_m\,\right]
> $$
>
> is computable at inference in $O(M \cdot d_{\max})$ and requires no
> auxiliary learned model: $\phi_m$ is frozen (Axiom 1); $c_m$ is
> frozen (Phase 1); $\tau_m$ is a scalar constant. Per-output trace
> $\{s_m(\hat y)\} = \{\cos(\phi_m(\hat y), c_m)\}$ attaches
> deterministically to $\hat y$ as a provenance record. No scalar-
> reward method (InstructGPT, DPO, Constitutional AI) and no cited
> identity-locked-generation method in clusters E / F offers this
> combination of properties in the LOCKED 88-pool (MP-09; MP-16
> pool-relative scope).

### Hedge paragraph to append

> The guard is a **probabilistic acceptance predicate**, not a logical
> runtime-verification monitor in the strict formal-methods sense.
> False-positive / false-negative trade-offs as $\tau$ varies are
> future work. The "no cited method" clause is bounded by the LOCKED
> master pool of 88 citations; a pool-external counter-example would
> be acknowledged under §8 Limitations and would not invalidate the
> composition-level claim that the three-role guard composition over
> identical frozen instances is the specific novel move.

---

## §5.4 — Per-output predicate (identity-locked generation definition)

### Paste-ready paragraph

> **Identity-locked generation** is the inference-time outcome of TCT
> when the subject is a specific person, style, or persona. A
> generation session is identity-locked iff every returned output
> satisfies $G_\tau(\hat y; C_P) = 1$. Per-output, per-modality cosine
> scores $\{s_m(\hat y)\}$ serve as a deterministic provenance trace.
> Case 3 reports a T1-measured instantiation: 50-clip L2-normalised
> WavLM centroid, best-of-12 selection at $T = 0.3$ against a
> $\tau_{\text{WavLM-SECS}} = 0.95$ threshold, with an
> architecturally-independent scorer (Qwen3-TTS conditioning →
> WavLM-base-plus-sv scoring) — 0.961 mean SECS (max 0.975) on 10
> unseen English sentences; DNSMOS P.808 parity (3.93 vs 3.93); UTMOS
> parity (3.012 vs 2.997) vs. real recordings. Cases 1 (Shakespeare
> stylistic guard) and 2 (Santa 7-modality identity-locked clone)
> remain T3-projected at time of writing.

---

## §6.3 — Case 3 framing (Theorem 2 measured instantiation; ISO-07 SPC anchor)

### Paste-ready paragraph

> Case 3 is the measured instantiation of Theorem 2. We use Qwen3-TTS
> with full in-context-learning conditioning and a 50-clip
> L2-normalised centroid of the author's voice as Phase-1
> constellation. At inference (Phase 3), we draw $K = 12$ candidates
> at $T = 0.3$ and accept the best-of-$K$ against
> $\tau_{\text{WavLM-SECS}} = 0.95$. The scorer is
> `microsoft/wavlm-base-plus-sv`, architecturally and
> training-data-independent from Qwen3-TTS's conditioning encoder —
> a **cross-encoder SECS** hygiene move that preempts within-encoder
> circular-evaluation concerns (RO-5). The SPC intuition is the right
> engineering read: 0.95 is an acceptance tolerance, not a proxy
> reward; the 0.961 mean SECS observed is the measured process
> capability at this operating point. DNSMOS and UTMOS parity with
> real recordings confirm the acceptance does not trade off
> naturalness.

### Hedge clauses to append (Bias-05, -06, -07)

- "Case 3 is single-subject (the author), single-encoder-pair
  (Qwen3-TTS × WavLM-base-plus-sv), English-only. Generalisation to
  additional subjects, encoder pairs, and languages is named future
  work (RO-10 F5a / F5c; §10.1)."
- "The operating point is a **single point** on a curve; the
  sample-complexity curve across $n_{\text{clips}} \in \{1, 5, 10,
  25, 50, 100\}$ is named future work (RO-10 F6b)."

---

## §7 — Discussion (composition-not-invention; three primary ISOs)

### Paste-ready paragraph — composition-simplicity claim

> The four contributions of this paper — Derived Data Abundance,
> meaning compression, Teleological Constellation Training, and the
> constellation guard — are **compositions of existing primitives**,
> not new theoretical primitives (MP-12). What makes the composition
> a contribution is the single-object spine: every contribution is
> defined around the constellation $C_P$, and the role-composition
> coherence (Proposition 4) means the same frozen $\Phi$ can
> simultaneously serve as retriever, labeller, and runtime guard
> without retraining. We draw three cross-field parallels to make
> the composition legible: (i) the constellation-guard topology
> imports from **formal-methods runtime verification** (each output
> monitored against a specification; ISO-01); (ii) the N-embedder
> panel echoes the **signal-processing sensor-fusion** intuition (N
> approximately-independent measurements yield better inference;
> ISO-02); (iii) the terminological choice **constellation** is
> deliberately astronomical (multi-reference; frozen; orient, not
> target; ISO-06). None of these is a citation within the LOCKED
> 88-citation pool; each is a tradition-family we name as a
> paradigmatic frame borrow.

### Paste-ready paragraph — relation to existing frameworks

> The spine **extends** Chinchilla scaling by introducing a new axis:
> where Hoffmann et al. (2022) and Kaplan et al. (2020) treat data as
> bit-compressed token counts, meaning compression treats data as
> supervisory-signal density, and DDA supplies the multiplicative
> factor $(N + \binom{N}{2})$ per input. Re-fitting scaling exponents
> under DDA-enriched supervision is named future work (RO-1 F1b). The
> spine **extends** compression-as-intelligence (Huang et al., 2024)
> by introducing a fourth taxonomic category alongside bit-, weight-,
> and activation-compression — unit-analytic orthogonality, not
> mathematical independence. The spine **structurally breaks** with
> scalar-reward alignment (InstructGPT, DPO, Constitutional AI) on
> four axes: aim (learned scalar vs. frozen multi-modal
> constellation), instrument (learned reward model vs. frozen
> embedders), metric (scalar preference vs. fixed cosine), and
> runtime verification (absent vs. per-output guard). We do not claim
> TCT outperforms RLHF on RLHF benchmarks; we claim it offers
> per-output geometric verifiability that RLHF benchmarks do not
> evaluate.

---

## §8 — Limitations (axiom-hedges carried explicitly)

### Paste-ready paragraph

> Three axioms underpin the framework, each with explicit boundary
> conditions. **Axiom 1 (grounded measurement)** holds only while
> embedders remain frozen; "frozen-with-LoRA" is acceptable under
> V18's discipline. **Axiom 2 (approximate orthogonality)** is V11-
> hedged: strict embedder independence is not claimed; empirical
> pairwise-MI measurement across the panel is named future work
> (RO-8). **Axiom 3 (reference-set learnability)** has a single
> T1-measured operating point (Case 3 at $n_m = 50$); other modalities
> and subjects are T3-projected. Additionally: the constellation guard
> is a **probabilistic** acceptance predicate, not a logical runtime-
> verification monitor — false-positive / false-negative trade-offs
> vs. $\tau$ are Q2 future work (agent-19). All "no cited method"
> claims in §§2, 5, 8 are **pool-relative** to the LOCKED 88-citation
> master pool (MP-16); we do not claim global-literature absences.

---

## Figure 1 specification (for agent-22 model-architect)

**Figure 1 — The spine.** A single diagram showing $C_P$ at the
centre, with four arrows pointing inward:

```
                    [raw corpus D]
                          │
                          │  (project through Φ)
                          ▼
              ┌──────────────────────┐
              │  DDA: {φ_m(x)}       │  Proposition 1
              │  |D'| = n(N+C(N,2))  │  (§3.4)
              └──────────────────────┘
                          │
                          │  (aggregate over R_m)
                          ▼
              ┌─────────────────────────────┐
              │                             │
              │      C_P = {(m, c_m)}       │◄──  Phase 1 (TCT)
              │      (the spine)            │
              │                             │
              └─────────────────────────────┘
                ▲                         ▲
                │  (Phase 2 loss          │  (Phase 3 guard
                │   target; §5.2)         │   verifier; §5.3)
                │                         │
           L_TCT(θ)                   G_τ(ŷ; C_P)
           = L_gen(θ) +               = ∀m: cos(φ_m(ŷ), c_m) ≥ τ_m
             Σ_m λ_m(1 −              (Proposition 3; Theorem 2)
             cos(φ_m(ŷ), c_m))
                │                         │
                └──────────┬──────────────┘
                           │
                           ▼
                  [identity-locked ŷ]
                  (Proposition 5; Theorem 1)
```

**Caption (paste-ready).** "The constellation $C_P$ is the single
paper-level object. DDA produces the per-embedder projections that
aggregate into the centroids; TCT's Phase-2 loss targets the frozen
$C_P$; TCT's Phase-3 guard verifies per-output proximity to $C_P$;
identity-locked generation is the runtime property the guard delivers.
Theorem 1 bounds the subject-specific sample complexity of producing
$C_P$-locked output at $O(M \cdot k)$, independent of general-corpus
size. Theorem 2 bounds the inference-time cost of the guard at
$O(M \cdot d_{\max})$ with no auxiliary learned model."

---

## Algorithm 1 specification (pseudocode; no Python / Rust per FM-06)

Use the pseudocode block from `tct-definition.md` §3.2.1 verbatim.
Three phases; comment headers; no language-specific syntax.

---

## Writer checklist (agent-37 methodology-writer acceptance criteria)

When composing §3 + §5 from this scaffold, the writer must ensure:

- [ ] The constellation $C_P$ is named before any axiom / proposition.
- [ ] All three axioms are introduced before any proposition that
      depends on them.
- [ ] Propositions are introduced before the theorems they feed.
- [ ] Every "no cited method" clause is pool-relative (MP-16).
- [ ] Every number has a T1 / T2 / T3 / T4 tag (V22).
- [ ] Every "orthogonal" / "independent" use carries V11.
- [ ] Every "frozen" use is correct per V18 (never "pretrained" where
      "frozen" is meant).
- [ ] The composition-not-invention register (MP-12) is carried at
      every first-use of a construct.
- [ ] Astronomical "constellation" terminology is never substituted
      with "prototype" / "anchor" / "target".
- [ ] Three ISO parallels (ISO-01, ISO-02, ISO-06) appear in §7
      synthesis; two additional (ISO-04, ISO-07) appear in §3.1 /
      §5.3; no ISO adds to the LOCKED citation pool.
- [ ] Hedges from agents 11, 13, 14, 17, 20 carry verbatim.

---

## Paragraph word-count targets (guidance, not hard limits)

| Block | Target words | Register |
|---|---|---|
| §3.0 framing | 150–200 | Composition-not-invention |
| §3.0.1 ISOs | 100–150 | Tradition-family only |
| §3.1 four-category | 200–250 | Unit-analysis hedged |
| §3.2 axioms | 100 × 3 = 300 | One paragraph per axiom |
| §3.3 scope argument | 150–200 | Categorical-scope hedged |
| §3.4 counting | 180–220 | T4 derivation + T2 instantiation |
| §3.6 Theorem 1 | 200–250 | Informal theorem |
| §5.1 TCT framing | 200–250 | Astronomical terminology defended |
| §5.3 guard | 200–250 | Dual-paradigm import |
| §5.3 Theorem 2 | 180–220 | Informal theorem |
| §5.4 identity-lock | 150–200 | Per-output property |
| §7 discussion | 250–350 | Three-ISO synthesis + relation-to-frameworks |
| §8 limitations axiom-hedges | 150–200 | Boundary conditions |

Total theoretical-framework prose target: **~2,500–3,000 words** across
§3 + §5, with the decomposition above as the scaffold.

---

## Confidence

**HIGH.**

- Every block is derived from the locked theoretical-framework decision
  document.
- Every block cites its upstream source (agent-04 definitions;
  agent-17 themes; agent-20 isomorphisms).
- Every block carries at least one hedge clause to the writer as
  paste-ready prose.
- Writer checklist is closed-form (every item is an MP / RO / V / ISO
  reference, not a vague quality target).
- No new citations introduced.
- No new primitives introduced.

## Changelog

- 2026-04-17 — Initial issue by agent-21-theory-builder.
