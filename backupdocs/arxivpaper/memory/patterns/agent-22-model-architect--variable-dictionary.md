---
namespace: patterns
agent_id: agent-22-model-architect
timestamp: 2026-04-17T05:55:00Z
status: active
locked: false
supersedes: none
tags: variable-dictionary, notation, canonical-symbols, glossary-bridge
depends_on:
  - memory/decisions/agent-22-model-architect--master-architecture.md
  - memory/patterns/agent-22-model-architect--subfigures.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
downstream_consumers:
  - agent-23-hypothesis-generator
  - agent-37-methodology-writer
  - agent-38-systems-writer
  - agent-42-adversarial-reviewer
---

# Variable Dictionary — Canonical Symbol List for Figures 1–4 and §3 / §5

## How to use this file

This file is the **single canonical symbol list** for the manuscript.
Every variable that appears in Figures 1–4, in the §3 + §5 equations,
or in any axiom / proposition / theorem statement is listed here with:

- **Symbol** (LaTeX glyph as it will appear in the paper).
- **Name** (human-readable label).
- **Type / space** (mathematical type).
- **Typical range / example value** (numeric or qualitative).
- **First use in figures** (which figure / level introduces it).
- **First use in theory** (which axiom / proposition / theorem).
- **Glossary anchor** (agent-02 glossary number, where applicable).
- **Hedge / discipline** (V11 / V18 / V22 / R3 / R4 notes).

Writers must not introduce a new variable without adding it here.
Adversarial reviewers (agent-42) will reference this file when
checking notational consistency.

---

## 1. Core sets and cardinalities

| Symbol | Name | Type / space | Typical range / example | First figure | First theory | Glossary | Hedge |
|---|---|---|---|---|---|---|---|
| $\mathcal{X}$ | Input space (admissible raw inputs) | Set (text / image / audio / video; heterogeneous across modalities) | Text strings, image tensors, audio waveforms | Fig. 1 Level 0 | Axiom 1 | #1 | — |
| $\mathcal{Y}$ | Output space (generated candidates) | Set of outputs $\hat y$ | Text / audio / image / video | Fig. 1 Level 8 | P3, Theorem 2 | — | — |
| $\mathcal{Z}$ | Conditioning / latent space | Set of prompts / latents / reference inputs to generator | Text prompts, noise latents, ICL reference clips | Fig. 4 Phase 2, Phase 3 | TCT §3.2 | — | — |
| $\mathcal{M}$ | Set of TCT modalities | Finite set $\{m_1, \ldots, m_M\}$ | Case 1: $\{\text{text-style}\}$; Case 2: $\{\text{visual, speaker, }\ldots\}$; Case 3: $\{\text{speaker}\}$ | Fig. 1 Level 6, Fig. 4 | TCT Phase 1 | #6 | — |
| $D$ | Fixed raw corpus | Subset of $\mathcal{X}$ of cardinality $n$ | Context Graph: ingested memories; ClipCannon: 2,362 clips from 16 min source | Fig. 1 Level 0 | Axiom 1, P1, P2 | #1 | — |
| $D'$ | Derived dataset | Set of tuples $(x, \{\phi_m(x)\}, \{\rho_{jk}\})$ | Size $n \cdot (N + \binom{N}{2})$ | Fig. 1 Level 3 | P1 | — | — |
| $\Phi$ | Frozen embedder panel | Set $\{\phi_m\}_{m=1}^N$ | Context Graph: E1–E13; ClipCannon: 7 modality heads | Fig. 1 Level 1; Fig. 2; Fig. 3 | Axioms 1, 2 | #5 | V11, V18 |
| $R_m$ | Per-modality reference set (subject-$P$-specific) | $\{r_{m,i}\}_{i=1}^{n_m} \subseteq D$ | Case 3: 50 clips; Case 2: 2,362 clips | Fig. 1 Level 4; Fig. 4 Phase 1 | Axiom 3, P5 | #9 | V22 (T1 vs T3) |
| $C_P$ | Constellation (the spine) | $\{(m, c_m) : m \in \mathcal{M}\}$ | Cardinality $M$ | Fig. 1 Level 6; Fig. 4 | Phase 1; every axiom / proposition / theorem | #8 | — |

---

## 2. Cardinalities and indices

| Symbol | Name | Type | Range | First figure | First theory | Hedge |
|---|---|---|---|---|---|---|
| $n$ | Corpus cardinality | $\mathbb{Z}_{>0}$ | $10^3 \lesssim n \lesssim 10^9$ in practice | Fig. 1 Level 0 | Axiom 1; P1 | — |
| $N$ | DDA embedder-panel cardinality | $\mathbb{Z}_{>0}$ | Context Graph: 13; ClipCannon: 7; general paper: 2 to ~20 | Fig. 1 Level 1; Fig. 2; Fig. 3 | P1 | — |
| $M$ | TCT modality-set cardinality (= $\lvert \mathcal{M} \rvert$) | $\mathbb{Z}_{>0}$ | Case 1: 1; Case 2: 2–7; Case 3: 1 | Fig. 1 Level 6; Fig. 4 | P5, Theorems 1, 2 | $M \neq N$ in general |
| $n_m$ | Reference-set size for modality $m$ | $\mathbb{Z}_{>0}$ | $[10, 10^2]$ by Axiom 3; Case 3 = 50 (T1); Case 2 = 2,362 (T3) | Fig. 1 Level 4; Fig. 4 Phase 1 | Axiom 3, P5 | V22, R4 |
| $k$ | Max reference-set size $= \max_m n_m$ | $\mathbb{Z}_{>0}$ | Case 3: 50; order $O(10^2)$ | Fig. 4 Phase 1 | Theorem 1 | V22 (T1 anchor only) |
| $K$ | Best-of-$K$ sample budget (Phase 3) | $\mathbb{Z}_{>0}$ | Case 3: $K = 12$ | Fig. 4 Phase 3 | TCT §3.2 | — |
| $d_m$ | Output dimensionality of $\phi_m$ | $\mathbb{Z}_{>0}$ | E1: 1024; E7: 1536; E12: 128/token | Fig. 2; Fig. 4 | P1 (precursor) | — |
| $d_{\max}$ | $\max_m d_m$ | $\mathbb{Z}_{>0}$ | Context Graph: 1536 (E7); ClipCannon: ~1024 | Fig. 4 Phase 3 | Theorem 2 | — |

---

## 3. Per-embedder and per-input objects

| Symbol | Name | Type | Example | First figure | First theory | Glossary | Hedge |
|---|---|---|---|---|---|---|---|
| $x$, $x_i$ | Single raw input / $i$-th corpus element | Element of $\mathcal{X}$ | A text memory; a video clip; an audio waveform | Fig. 1 Level 0 | Axiom 1 | — | — |
| $\phi_m$ | Frozen embedder for modality / axis $m$ | Function $\mathcal{X} \to \mathbb{R}^{d_m}$ (dense) or sparse / variable-length output space | E1: e5-large-v2 (text → 1024D); WavLM-SV (audio → 512D+); CLIP ViT-L/14 | Fig. 1 Level 1; Fig. 2; Fig. 3; Fig. 4 | Axiom 1; appears in every proposition / theorem | #5, #10 | V18 (frozen), V11 |
| $\phi_m(x)$ | Per-embedder projection of a single input | Vector in $\mathbb{R}^{d_m}$ (or sparse / per-token) | E1: 1024D vector; SPLADE: sparse 30,522-D | Fig. 1 Level 2a | Axiom 1; P1 | — | — |
| $\rho_{jk}$ | Cross-embedder interaction feature | $\mathbb{R}^{d_j} \times \mathbb{R}^{d_k} \to \mathbb{R}^{r_{jk}}$ (often scalar) | Normalised cosine $\cos(\phi_j(x), \phi_k(x))$ | Fig. 1 Level 2b | Axiom 2, P1 | #15 | V11 |
| $r_{m,i}$ | $i$-th reference sample of subject $P$ in modality $m$ | Element of $\mathcal{X}$ | 50 audio clips of the author (Case 3); 2,362 video clips (Case 2) | Fig. 1 Level 4; Fig. 4 Phase 1 | Axiom 3 | #9 | V22 |
| $v_{m,i}$ | L2-normalised per-modality embedding of reference sample | Unit vector in $\mathbb{R}^{d_m}$ | $\text{normalize}(\phi_m(r_{m,i}))$ | Fig. 4 Phase 1 | Phase 1 specification | — | — |
| $c_m$ | Per-modality centroid (unit vector) | $\mathbb{S}^{d_m - 1}$ (unit sphere) | L2-normalised mean of $\{v_{m,i}\}$ | Fig. 1 Level 5; Fig. 4 Phase 1 | Axiom 3, P5; Theorem 1 | #7 | — |

---

## 4. TCT objective / inference objects

| Symbol | Name | Type | Typical range | First figure | First theory | Hedge |
|---|---|---|---|---|---|---|
| $g_\theta$ | Generator with parameters $\theta$ | Function $\mathcal{Z} \to \mathcal{Y}$ | LLM (Case 1); flow-matching video model (Case 2); Qwen3-TTS (Case 3) | Fig. 4 Phase 2 | TCT §3.2 | V18 ("frozen-with-LoRA" or "inference-only") |
| $\theta$, $\theta^*$ | Generator parameters (pre- and post-training) | $\mathbb{R}^P$ for some parameter count $P$ | Order $10^9$ params for LLM / TTS backbones | Fig. 4 Phase 2, Phase 3 | P5, Theorem 1 | — |
| $\hat y$, $\hat y^{(k)}$ | Generated candidate / $k$-th sample | Element of $\mathcal{Y}$ | Text output; audio waveform; video frame | Fig. 1 Level 7–8; Fig. 4 Phase 2, 3 | P3, Theorem 2 | — |
| $z$ | Conditioning input | Element of $\mathcal{Z}$ | Prompt; reference clip; latent noise | Fig. 4 Phase 2, 3 | TCT §3.2 | — |
| $\mathcal{L}_{\mathrm{gen}}$ | Base generator loss | $\mathbb{R}_{\geq 0}$ | Cross-entropy (LLM); flow-matching MSE; DPO pair loss | Fig. 4 Phase 2 | TCT §3.2 | — |
| $\mathcal{L}_{\mathrm{TCT}}$ | Full Phase-2 objective | $\mathbb{R}_{\geq 0}$ | $\mathcal{L}_{\mathrm{gen}} + \sum_m \lambda_m(1-\cos)$ | Fig. 1 Level 7a; Fig. 4 Phase 2 | P5, Theorem 1 | — |
| $\lambda_m$ | Per-modality Phase-2 loss weight | $\mathbb{R}_{\geq 0}$ | Case 2: mouth-weighted λ_speaker high, λ_global visual low; typically [0, 1] | Fig. 4 Phase 2 | TCT §3.2 | — |
| $\eta$ | Learning rate (generic) | $\mathbb{R}_{> 0}$ | $10^{-5}$ to $10^{-4}$ for LoRA fine-tuning | Fig. 4 Phase 2 | — | — |
| $T$ | Sampling temperature (Phase 3) | $\mathbb{R}_{> 0}$ | Case 3: $T = 0.3$ | Fig. 4 Phase 3 | — | — |
| $\tau_m$ | Per-modality acceptance threshold | $\mathbb{R} \in [-1, 1]$ (cosine domain) | Case 3: $\tau_{\text{WavLM-SECS}} = 0.95$; Cases 1, 2 T3-projected | Fig. 4 Phase 3 | P3, Theorem 2 | V22 / R4 |
| $w_m$ | Relaxed-variant weight in best-of-$K$ selector | $\mathbb{R}_{\geq 0}$, typically $\sum_m w_m = 1$ | Case 3: uniform across the speaker modality (single modality → $w = 1$) | Fig. 4 Phase 3 relaxed | TCT §3.2 | — |

---

## 5. Guard / verification objects

| Symbol | Name | Type | Range | First figure | First theory | Hedge |
|---|---|---|---|---|---|---|
| $s_m(\hat y)$ | Per-modality cosine score for candidate | $[-1, 1]$ | Case 3: mean 0.961; max 0.975 (T1-measured) | Fig. 1 Level 8; Fig. 4 Phase 3 | P3, Theorem 2 | V22 |
| $\{s_m(\hat y)\}$ | Per-output cosine trace (provenance) | Tuple in $[-1, 1]^M$ | Attached deterministically to each accepted $\hat y$ | Fig. 1 Level 8; Fig. 4 Phase 3 | Theorem 2 | — |
| $G_\tau(\hat y; C_P)$ | Constellation-guard predicate | $\{0, 1\}$ | $\mathbb{1}[\forall m : s_m \geq \tau_m]$ | Fig. 1 Level 7b; Fig. 4 Phase 3 | P3, Theorem 2 | — |
| $\mathbf{\tau}$ | Vector of thresholds $(\tau_m)_{m \in \mathcal{M}}$ | $[-1, 1]^M$ | Case 3: $(0.95)$; Cases 1, 2 T3-projected | Fig. 4 Phase 3 | P3, Theorem 2 | V22 / R4 |

---

## 6. Sample-complexity / theorem-level objects

| Symbol | Name | Type | Range | First figure | First theory | Hedge |
|---|---|---|---|---|---|---|
| $\mathcal{T}_{\mathrm{TCT}}(P)$ | TCT subject-specific sample complexity | $\mathbb{Z}_{>0}$ (or $O(\cdot)$ asymptotic) | Case 3: $O(50)$; in general $O(M \cdot k)$ | Fig. 1 Level 8 anchor | P5, Theorem 1 | V22, R4 |
| $\mathcal{T}_{\mathrm{conv}}(P)$ | Conventional-pipeline sample complexity (baseline) | $\mathbb{Z}_{>0}$ | Typically $\gg \mathcal{T}_{\mathrm{TCT}}$ in cluster E / F literature | — | P5 | R3 pool-relative |
| "Signal yield" | $N + \binom{N}{2}$ signals per input | $\mathbb{Z}_{>0}$ | $N=13 \to 91$; $N=7 \to 28$ | Fig. 1 Level 3; Fig. 2; Fig. 3 | P1 | — |
| $\mathrm{MC}(\Phi, D)$ | Meaning-compression ratio | $\mathbb{Q}_{>0}$ | $N + \binom{N}{2}$ at fixed $D$; 91 for Context Graph; 28 for ClipCannon | Fig. 1 Level 3; meaning-compression §2.3 | P1 (ratio view) | V11 |

---

## 7. Case-study specific values (T1 / T2 / T3 flagged)

| Value | Case | Meaning | Tier | Source |
|---|---|---|---|---|
| $N = 13$ | Context Graph | DDA panel size (E1–E13) | T2 constructive | Mission brief "Key Architecture Facts" |
| $N = 7$ | ClipCannon | Video-modality panel size | T2 constructive | Mission brief |
| $|D'|/n = 91$ | Context Graph | Signals per input under P1 | T4 derivation + T2 instantiation | DDA definition §1.2.3 |
| $|D'|/n = 28$ | ClipCannon | Signals per input under P1 | T4 + T2 | DDA definition §1.2.3 |
| $n_m = 50$ | Case 3 (voice) | Reference-set size | T1 MEASURED | SITREP 2026-04-16 |
| $n_m = 2{,}362$ | Case 2 (Santa) | Reference-set size | T3 PROJECTED | Mission brief |
| $K = 12$, $T = 0.3$ | Case 3 | Best-of-$K$ budget and temperature | T1 MEASURED | SITREP 2026-04-16 |
| $\tau_{\text{WavLM-SECS}} = 0.95$ | Case 3 | Acceptance threshold | T1 MEASURED | SITREP 2026-04-16 |
| Mean SECS = 0.961 (max 0.975) | Case 3 | Measured identity-lock score (10 unseen English sentences) | T1 MEASURED | SITREP 2026-04-16, white paper |
| DNSMOS P.808 = 3.93 (parity with real at 3.93) | Case 3 | Naturalness parity | T1 MEASURED | SITREP 2026-04-16 |
| UTMOS = 3.012 (vs real 2.997) | Case 3 | Naturalness parity | T1 MEASURED | SITREP 2026-04-16 |

---

## 8. Notational discipline rules (inherited from agent-01 / agent-02)

1. **V11 at every "orthogonal" / "independent" use.** Axiom 2 and
   Proposition 1 both trigger V11; the figure must carry the hedge
   clause inline.
2. **V18 at every "frozen" use.** Never "pretrained" where "frozen"
   is meant. LoRA-augmented regime is "frozen-with-LoRA".
3. **V22 at every measured vs. projected number.** Numbers with T1
   tier get MEASURED; T3 get PROJECTED; the Variable-Dictionary table
   carries these tags.
4. **R3 pool-relative at every "no cited method" clause.** Figures'
   footnotes carry R3 where novelty / absence is claimed.
5. **R4 projected vs. measured** at every value in the case-study
   table above.
6. **$N$ vs $M$ discipline.** $N$ is DDA panel size. $M$ is TCT
   modality-set size. They may coincide but are different objects.
   See master-architecture §5.
7. **Glyph stability across figures.** $\phi_m$, $c_m$, $C_P$,
   $G_\tau$, $\hat y$, $s_m$ have identical meaning everywhere they
   appear. A variable renamed in one figure must be renamed
   everywhere.

---

## 9. Notation not used (explicit non-list for reviewer clarity)

The following symbols are **intentionally avoided** to prevent
collision with common machine-learning notation that means something
different in this paper:

- **"$r$"** is reserved for *reference samples* ($r_{m,i}$), **not**
  a reward function. Use $r_\psi$ only inside §2.6 / §5 when
  contrasting with RLHF; elsewhere avoid.
- **"$\mathcal{L}$"** refers to generator loss only; never to a
  likelihood function.
- **"$P$"** is the *subject* (a person, style, or identity);
  **not** a probability distribution. When probability is needed,
  use $p$ or $\mathbb{P}$.
- **"prototype"** is **never** a substitute for centroid
  ($c_m$) per V1; the glossary forbids the swap (prototypical
  networks, cognitive prototypes, and retrieval anchors each carry
  their own disciplinary baggage).

---

## 10. Confidence

**HIGH** on symbol inventory completeness; every variable in the
master architecture, the sub-figures, and the §3 + §5 equations is
listed. **HIGH** on discipline alignment with glossary (agent-02) and
construct definitions (agent-04). Writers who add a new variable
must append a row here and re-issue this file.

## Changelog

- 2026-04-17 — Initial issue by agent-22-model-architect.
