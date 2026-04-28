---
namespace: reports
agent_id: agent-04-construct-definer
timestamp: 2026-04-16T16:00:00Z
status: complete
tags: final-report, construct-definer, smap
---

# Agent-04 (construct-definer) — Final Report

## Mission (SMAP)

**Specific.** Operationalise the four load-bearing constructs of the arXiv manuscript —
Derived Data Abundance (DDA), meaning compression, Teleological Constellation Training
(TCT), identity-locked generation — plus three support constructs (constellation, frozen
embedder / measurement instrument, same-session verification band). Produce paper-ready
definition blocks that every downstream writer pulls from.

**Measurable.** One locked decision file per Tier-A construct; one locked decision file
for the support constructs; one consolidated single-page writer handoff; one handoff to
agent-05; one final report. Every construct has a conceptual definition, an operational
definition (equations where applicable), variables / indicators, a "what it is NOT"
clause, and a usage rule tied to the canonical glossary.

**Achievable.** All binding questions (Q04, Q05, Q06, Q07, Q15) are ANSWERABLE per
agent-03's decomposition; glossary vocabulary is fixed; no blockers required.

**Performed.** Complete. All eight target files written.

---

## Files produced (absolute paths)

1. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-04-construct-definer--dda-definition.md`
2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-04-construct-definer--meaning-compression-definition.md`
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-04-construct-definer--tct-definition.md`
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-04-construct-definer--identity-locked-definition.md`
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-04-construct-definer--support-constructs.md`
6. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md`
7. `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-04-to-05--constructs-locked.md`
8. `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-04-construct-definer--final-report.md` (this file)

---

## Summary of each construct (one paragraph each)

### DDA (Derived Data Abundance) — Q04, T4/T2

DDA is operationalised as follows. Given a fixed corpus $D = \{x_i\}_{i=1}^n$ and a panel
of $N$ frozen independently-trained embedders $\{\phi_m\}_{m=1}^N$, the derived dataset
$D' = \{(x_i, \phi_1(x_i), \ldots, \phi_N(x_i), (\rho_{jk}(\phi_j(x_i), \phi_k(x_i)))_{j<k})\}$
has cardinality $|D'| = n(N + \binom{N}{2}) = n \cdot N(N+1)/2$: $n \cdot N$
per-embedder labeled vectors plus $n \cdot \binom{N}{2}$ pairwise cross-embedder
interaction features. The multiplicative-gain claim holds under four explicit conditions
— approximate embedder independence, non-degenerate interaction features, fixed corpus, and
truly frozen embedders; strict formal orthogonality is an open question (V11 hedge) and
formal mutual-information measurement is future work (Q20 (i)). Context Graph
instantiates $N = 13 \Rightarrow 91$ signals per input (T2 constructive). DDA is **not**
synthetic data, data augmentation, distillation, self-training, active learning, or
multi-task supervision.

### Meaning compression — Q05, T4

Meaning compression is defined as the ratio $\mathrm{MC}(\Phi, D) = \mathrm{signals}(D, \Phi) /
\mathrm{raw}(D) = N + \binom{N}{2}$ signals per input at fixed corpus $D$ — the signal
density achieved by running DDA on a raw corpus. It is a **fourth** compression category
distinguished by unit dimensionality from bit compression (bits/bits), weight compression
(accuracy/parameters), and activation compression (memory/quality); meaning compression
operates on signals/unit-of-raw-data. Orthogonality is **narrow**: the four categories
trade resources along dimensionally-distinct axes; optimising one does not produce
compression in the others; the categories can be stacked. Never "semantic compression"
(V5); never hyphenated.

### TCT (Teleological Constellation Training) — Q06/Q07, T4/T2

TCT is a three-phase protocol: Phase 1 constructs a frozen constellation
$C_P = \{(m, c_m)\}_{m \in \mathcal{M}}$ with $c_m = \mathrm{normalize}(\mathrm{mean}_{r \in R_m}
\mathrm{normalize}(\phi_m(r)))$; Phase 2 fine-tunes the generator with the composite loss
$\mathcal{L}(\theta) = \mathcal{L}_\text{gen}(\theta) + \sum_m \lambda_m(1 - \cos(\phi_m(\hat{y}), c_m))$
while keeping every $\phi_m$ and every $c_m$ frozen; Phase 3 is the **constellation guard**
— every generated output is re-embedded through the same frozen $\phi_m$ and accepted iff
$\cos(\phi_m(\hat{y}), c_m) \geq \tau_m$ for every modality. TCT is "teleological" because
its aim is a **frozen geometric target**, not a learned reward; it is explicitly **not**
RLHF, DPO, Constitutional AI, contrastive learning, metric learning, prototypical-network
classification, or vanilla LoRA fine-tuning. The Case 3 instantiation runs in
inference-only mode with a 50-clip reference set and a WavLM-scored best-of-12 guard.

### Identity-locked generation — Q15, T4/T1/T3

Identity-locked generation is defined by the per-output predicate
$\mathrm{IdentityLocked}(\hat{y}; C_P, \tau) \iff \forall m \in \mathcal{M}.\;
\cos(\phi_m(\hat{y}), c_m) \geq \tau_m$: a subject's identity is operationalised as
the constellation, and every output must fall within the per-modality cosine thresholds.
It is the **inference-time property** that TCT's Phase-3 guard enforces. It is **not**
personalisation, style transfer, subject-specific fine-tuning alone, LoRA-personalised
generation, or zero-shot TTS. Case 3 measures it at T1: WavLM SECS mean 0.961, max 0.975
on 10 held-out sentences; Cases 1 and 2 are T3 projected. V3 forbids "identity-preserving".

### Support constructs — constellation, frozen embedder, same-session verification band

Constellation = $\{(m, c_m)\}_{m \in \mathcal{M}}$, a set of centroids, not an embedding,
not a reference set, not a codebook; frozen after Phase 1. Frozen embedder = a measurement
instrument $\phi_m$ whose parameters are fixed during DDA + TCT + inference; a behaviour
claim about this pipeline, not a history claim. Same-session verification band = the
empirical 0.95–0.99 WavLM-SECS range observed when scoring pairs of Case 3's speaker's own
real recordings from the same session — **our empirical observation**, not a published
benchmark, not a universal value; used to calibrate $\tau_{\text{speaker}}$ for Case 3 only.

---

## Consistency with upstream artefacts

- **Canonical glossary (agent-02).** Every term used in definitions matches the glossary
  spelling exactly. Every FORBIDDEN substitute (prototype, anchor, identity-preserving,
  semantic compression, bare SECS, etc.) is absent from the definition files. Every
  mandated hedge (V11 orthogonality, V12 same-session band, V18 frozen vs. pretrained)
  is embedded.
- **Essential questions (agent-03).** Q04, Q05, Q06, Q07, Q15 are answered directly in the
  deliverable files; the "answers_questions" frontmatter on each file lists its bindings.
- **Guiding principles (agent-01).** P03 (define before first use) is honoured — every
  Tier-A construct now has a canonical paper-ready definition. P04 (evidence tiers) is
  honoured — every formal claim carries a tier tag. P09 (no marketing, no AGI, no
  superlatives) is honoured — no forbidden terms in any deliverable. P10 (failure
  documentation) is scoped out of this agent's deliverable (owned by results-writer and
  discussion-writer).

## No blockers

No construct required extending the glossary beyond its current entries. Every definition
was stateable with existing vocabulary. No contradictions with locked decisions were
introduced.

## Confidence

**HIGH.** Rationale:

1. The 20 essential questions mark Q04 / Q05 / Q06 / Q07 / Q15 as ANSWERABLE with existing
   evidence. All were answered with formal definitions.
2. The canonical glossary supplies unambiguous vocabulary for every construct; no new
   terms were needed.
3. The formal derivations (DDA count, meaning-compression ratio, TCT three-phase protocol,
   identity-lock predicate) are internally consistent and cross-reference a single set of
   symbols ($\phi_m$, $c_m$, $C_P$, $\tau_m$, $\lambda_m$, $\mathcal{M}$, $R_m$).
4. Case 3's T1 numerics (SECS mean 0.961, same-session band 0.95–0.99) plug directly into
   the identity-locked predicate and Phase-3 guard.
5. Every downstream writer has a paste-ready one-paragraph block in the patterns file and
   a full formal file behind it; agent-05's research plan can be built against them
   without re-derivation.

Caveats already acknowledged in the files themselves:

- The $N + \binom{N}{2}$ count is an **upper bound** on information gain; formal
  mutual-information measurement is future work (Q20 (i)).
- The same-session band is **specific to Case 3's subject**; the threshold
  $\tau_{\text{speaker}}$ is calibrated empirically, not universally.
- Cases 1 and 2 thresholds and accept rates are **T3 projected**, not measured.

---

## Handoff pointer

Next agent: **agent-05 (research-planner)**.

Read first:
- `./memory/handoff/agent-04-to-05--constructs-locked.md`
- `./memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md`
- the four Tier-A decision files listed above.

---

## Change log

- 2026-04-16 — Initial report, agent-04 complete.
