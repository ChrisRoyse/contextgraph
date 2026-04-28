---
namespace: decisions
agent_id: agent-22-model-architect
timestamp: 2026-04-17T05:45:00Z
status: active
locked: false
supersedes: none
tags: architecture, figure-1, master-diagram, constellation-spine, dda-to-tct, identity-locked
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/patterns/agent-21-theory-builder--theory-for-writers.md
  - memory/handoff/agent-21-to-22--theory-ready.md
downstream_consumers:
  - agent-23-hypothesis-generator
  - agent-24-opportunity-identifier (if applicable)
  - agent-37-methodology-writer
  - agent-38-systems-writer
  - agent-39-discussion-writer
answers_questions: [Q04, Q06, Q07, Q15]
---

# Master Architecture — Figure 1: The Constellation as Spine

## 0. Purpose

This file issues the **master conceptual architecture** for the
manuscript. It is not a neural-network computation graph; it is the
**relationship diagram** that connects every primitive named by
agents 04 and 21 into a single topology:

```
corpus D  →  frozen panel Φ  →  derived D'  +  cross-corr ρ_jk
                                         │
                       aggregate over R_m │
                                         ▼
                          per-modality c_m  ─→  C_P  (Phase 1)
                                                  │
                          ┌───────────────────────┼───────────────────────┐
                          │ Phase 2 (loss target) │ Phase 3 (runtime guard)│
                          ▼                       ▼                        ▼
                   L_TCT(θ)                G_τ(ŷ; C_P)                  {s_m(ŷ)}
                                                                 per-output trace
                                                                        │
                                                                        ▼
                                                              identity-locked ŷ
```

All downstream writers (agent-37, -38, -39) will render Figure 1 from
this specification. This file is the single source of truth for
Figure 1.

---

## 1. The master figure — Figure 1 (arXiv Figure 1)

### 1.1 Structural ASCII tree (load-bearing)

```
                          ┌───────────────────────────────────────────────┐
                          │                LEVEL 0 — INPUT                │
                          │                                               │
                          │     D = {x_1, x_2, …, x_n}  ⊂  𝒳              │
                          │     fixed raw corpus of n inputs              │
                          │     (Context Graph: text memories;            │
                          │      ClipCannon: 2,362 clips from 16 min)     │
                          └───────────────────────────────────────────────┘
                                               │
                                               │    project every x through every φ_m
                                               │    (Axiom 1: grounded measurement)
                                               │    no gradient flows back into Φ
                                               ▼
   ┌────────────────────────────────────────────────────────────────────────────┐
   │                         LEVEL 1 — FROZEN PANEL                             │
   │                                                                            │
   │          Φ = { φ_1, φ_2, …, φ_N }     (N approximately-independent)        │
   │                                                                            │
   │   φ_1 : 𝒳 → ℝ^{d_1}       φ_2 : 𝒳 → ℝ^{d_2}     …    φ_N : 𝒳 → ℝ^{d_N}     │
   │                                                                            │
   │   • Context Graph instance:   N = 13  (E1…E13; ~11,008 dense + sparse)     │
   │   • ClipCannon instance:      N = 7   (7 modalities × 4,044 dim)           │
   │                                                                            │
   │   Hedge V11: "approximately independent; strict formal orthogonality       │
   │   is an open question (§10)"  — carried at every first use.                │
   └────────────────────────────────────────────────────────────────────────────┘
                              │                                   │
                              │ per-embedder                      │ per-pair
                              │ projection                        │ interaction
                              ▼                                   ▼
   ┌──────────────────────────────────────┐   ┌──────────────────────────────────┐
   │    LEVEL 2a — PER-EMBEDDER SIGNALS   │   │  LEVEL 2b — CROSS-CORRELATIONS   │
   │                                      │   │                                  │
   │     { φ_m(x_i) }                     │   │  { ρ_{jk}(φ_j(x_i), φ_k(x_i)) }  │
   │      for m = 1…N, i = 1…n            │   │   for 1 ≤ j < k ≤ N, i = 1…n     │
   │                                      │   │                                  │
   │     count: n · N                     │   │  count: n · C(N,2) = n·N(N−1)/2  │
   │     (Context Graph: 13·n)            │   │  (Context Graph: 78·n)           │
   │     (ClipCannon:     7·n)            │   │  (ClipCannon:    21·n)           │
   └──────────────────────────────────────┘   └──────────────────────────────────┘
                              │                                   │
                              └─────────────────┬─────────────────┘
                                                ▼
                ┌──────────────────────────────────────────────────┐
                │       LEVEL 3 — DERIVED DATASET  D'              │
                │                                                  │
                │   D' = { ( x_i , {φ_m(x_i)}_m , {ρ_{jk}}_{j<k} ) │
                │          : i = 1…n }                             │
                │                                                  │
                │   |D'|  =  n · ( N + C(N,2) )  =  n · N(N+1)/2   │
                │                                                  │
                │   Context Graph (N=13):  |D'|/n  =  91           │
                │   ClipCannon   (N=7) :   |D'|/n  =  28           │
                │                                                  │
                │   ◄═══  Proposition 1 (signal-count identity)    │
                │   ◄═══  Meaning-compression ratio view (§3.1)    │
                └──────────────────────────────────────────────────┘
                                                │
                                                │ (aggregate subject-specific subset
                                                │  R_m ⊂ D restricted to subject P,
                                                │  per modality m)
                                                ▼
    ┌──────────────────────────────────────────────────────────────────────────┐
    │              LEVEL 4 — PER-SUBJECT REFERENCE SETS  { R_m }               │
    │                                                                          │
    │   For subject P and modality m ∈ ℳ :                                     │
    │     R_m  =  { r_{m,1}, r_{m,2}, …, r_{m,n_m} }                           │
    │                                                                          │
    │   • Case 3 (voice, speaker mod.):    n_m = 50     (T1-MEASURED)          │
    │   • Case 2 (Santa, 7 visual mods.):  n_m = 2,362  (T3-PROJECTED)         │
    │   • Case 1 (Shakespeare, text):      n_m = TBD    (T3-PROJECTED)         │
    │                                                                          │
    │   Axiom 3 anchor: n_m ∈ [10, 10²] suffices for well-conditioned φ_m.     │
    └──────────────────────────────────────────────────────────────────────────┘
                                                │
                                                │ per-modality centroid
                                                │ (L2-norm of L2-normed mean)
                                                ▼
       ┌──────────────────────────────────────────────────────────────┐
       │            LEVEL 5 — PER-MODALITY CENTROID  c_m              │
       │                                                              │
       │   c_m = normalize( (1/n_m) · Σ_{i=1..n_m}                    │
       │                    normalize( φ_m ( r_{m,i} ) ) )            │
       │                                                              │
       │   c_m ∈ 𝕊^{d_m − 1}   (unit vector on the sphere)            │
       │                                                              │
       │   frozen once computed; no gradient flows into c_m           │
       └──────────────────────────────────────────────────────────────┘
                                                │
                                                │ gather over all modalities m ∈ ℳ
                                                ▼
    ╔══════════════════════════════════════════════════════════════════════════╗
    ║                                                                          ║
    ║              LEVEL 6 — THE CONSTELLATION  C_P   (the spine)              ║
    ║                                                                          ║
    ║                 C_P  =  { (m, c_m)  :  m ∈ ℳ }                           ║
    ║                                                                          ║
    ║                 |C_P| = M = |ℳ|    (TCT modality cardinality)            ║
    ║                                                                          ║
    ║                 ◄════  Phase 1 output                                    ║
    ║                 ◄════  Theorem 1 sample-complexity target                ║
    ║                 ◄════  Theorem 2 runtime-guard target                    ║
    ║                                                                          ║
    ║    (frozen for the remainder of the pipeline; no gradient enters C_P)    ║
    ╚══════════════════════════════════════════════════════════════════════════╝
                           │                                     │
           (Phase 2: loss)  │                                     │  (Phase 3: guard)
                           ▼                                     ▼
   ┌────────────────────────────────────────┐   ┌──────────────────────────────────────┐
   │        LEVEL 7a — PHASE 2 TRAINING     │   │      LEVEL 7b — PHASE 3 RUNTIME      │
   │                                        │   │                                      │
   │   Generator g_θ : 𝒵 → 𝒴                │   │   For each conditioning z:           │
   │                                        │   │     1. Draw K candidates             │
   │   Loss (per step):                     │   │        ŷ^{(k)} = g_{θ*}(z; seed_k)   │
   │                                        │   │     2. For each m ∈ ℳ:               │
   │   L_TCT(θ) =                           │   │        s_m^{(k)} = cos(φ_m(ŷ^{(k)}), │
   │       L_gen(θ; z, y, ŷ)                │   │                      c_m)            │
   │     + Σ_{m∈ℳ} λ_m · ( 1 − cos(         │   │     3. Guard predicate:              │
   │                        φ_m(ŷ) ,        │   │        G_τ(ŷ; C_P) =                 │
   │                        c_m  ) )        │   │           𝟙[ ∀m : s_m ≥ τ_m ]        │
   │                                        │   │                                      │
   │   Update θ by ∇_θ L_TCT                │   │     (relaxed variant:                │
   │                                        │   │       k* = argmax_k Σ w_m · s_m^{(k)})│
   │   ◄── Proposition 5 (sample complex.)  │   │                                      │
   │   ◄── Axiom 1 (φ_m frozen)             │   │   ◄── Proposition 3 (verifiability)  │
   │                                        │   │   ◄── Theorem 2 (O(M·d_max), no aux. │
   │                                        │   │        learned model)                │
   └────────────────────────────────────────┘   └──────────────────────────────────────┘
                           │                                     │
                           │    θ*                               │  accepted ŷ
                           │    (trained generator)              │  + cosine trace {s_m(ŷ)}
                           │                                     │
                           └──────────────┬──────────────────────┘
                                          ▼
                ┌────────────────────────────────────────────────┐
                │         LEVEL 8 — IDENTITY-LOCKED OUTPUT       │
                │                                                │
                │   ŷ   such that   G_τ( ŷ ; C_P ) = 1           │
                │                                                │
                │   attached provenance:  { s_m(ŷ) }_{m ∈ ℳ}     │
                │                          (deterministic trace) │
                │                                                │
                │   ◄── Proposition 5                            │
                │   ◄── Theorem 1  ( 𝒯_TCT(P) = O(M·k) )          │
                │   ◄── Case 3 T1-MEASURED instantiation:        │
                │       mean SECS 0.961 (max 0.975);             │
                │       DNSMOS 3.93; UTMOS 3.012                 │
                └────────────────────────────────────────────────┘
```

### 1.2 The canonical caption (paste-ready for Figure 1)

> **Figure 1. The constellation as spine.** A fixed corpus $D$ is
> projected through a panel $\Phi = \{\phi_m\}_{m=1}^{N}$ of frozen
> embedders (Level 1). The per-embedder projections and pairwise
> cross-correlations together form the derived dataset $D'$ of size
> $n \cdot (N + \binom{N}{2})$ (Levels 2–3; **Proposition 1**). From
> the subject-$P$ restriction $\bigcup_m R_m \subseteq D$, per-modality
> centroids $c_m$ are aggregated and frozen (Level 5), giving the
> **constellation** $C_P = \{(m, c_m) : m \in \mathcal{M}\}$ (Level
> 6). $C_P$ appears in two roles downstream: it is the **target of
> the Phase-2 geometric-proximity loss**
> $\mathcal{L}_{\mathrm{TCT}}(\theta) = \mathcal{L}_{\mathrm{gen}}(\theta) + \sum_m \lambda_m\bigl(1 - \cos(\phi_m(\hat y), c_m)\bigr)$
> and the **reference for the Phase-3 runtime guard**
> $G_\tau(\hat y; C_P) = \mathbb{1}[\,\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m\,]$
> (Level 7). Accepted generations $\hat y$ carry a per-output cosine
> trace $\{s_m(\hat y)\}$ as provenance (Level 8). **Theorem 1** bounds
> the subject-specific sample complexity at $O(M \cdot k)$ with
> $k = \max_m n_m$, independent of $|D|$. **Theorem 2** bounds the
> per-output guard cost at $O(M \cdot d_{\max})$ and states it requires
> no auxiliary learned model. Axioms 1–3 underpin the construction.

### 1.3 Legend / notation conventions

| Shape / symbol | Meaning |
|---|---|
| Plain box `┌─┐` | **Data / artefact** at a level (corpus, dataset, centroid, generation). |
| Doubled box `╔═╗` | **Spine object** — $C_P$. There is exactly one in the figure. |
| Arrow `→` | **Flow of information** (data moves, or is computed). |
| Arrow `◄──` | **Theoretical anchor** — the proposition / theorem / axiom this level instantiates. |
| Dashed frozen note | Indicates that gradients do not flow through this boundary (Axiom 1). |
| $\mathcal{M}$ | Set of TCT modalities (Case 3: one; Case 2: seven; Case 1: one). |
| $\Phi, N$ | DDA embedder panel; $N$ counts panel cardinality. |
| $\mathcal{M}, M$ | TCT modality set; $M = \lvert \mathcal{M} \rvert$. $N$ and $M$ **may differ** (e.g., Context Graph's $N=13$ panel can be used in a TCT constellation with $M=1$ text-style modality). |

### 1.4 Role-composition annotation (P4)

Levels 1–3 and Levels 5–7 both consume the **same frozen embedder
instances $\phi_m$**:

- **Retriever role (Context Graph §4.1):** $\phi_m$ is queried on stored
  content at runtime to rank candidate memories by cosine similarity.
  Same instances that populate Level 1.
- **Labeller role (DDA §3.4):** $\phi_m$ is evaluated on $D$ at Level 2
  to produce $n \cdot N$ signals + $n \cdot \binom{N}{2}$ pairs.
- **Guard role (TCT Phase 3):** $\phi_m$ is evaluated on $\hat y$ at
  Level 7b to compute $s_m(\hat y) = \cos(\phi_m(\hat y), c_m)$.

No parameter update to any $\phi_m$ occurs in any of the three roles.
This is **Proposition 4 (role-composition coherence)** instantiated
visually. Figure 1 marks this by using the **same symbol $\phi_m$** at
Levels 1, 5, and 7 — different roles, identical function instances.

### 1.5 Hedge callouts that must appear on Figure 1

Three on-figure hedges are load-bearing and must survive the TikZ /
Mermaid render:

1. **V11 (Level 1, approximate independence)** — bottom-right of the
   panel box: "*approximately independent; strict formal
   orthogonality is an open question*".
2. **V22 / R4 (Level 4, measured vs. projected)** — inset next to
   reference-set sizes: "*Case 3: $n_m = 50$ T1-MEASURED; Cases 1, 2:
   T3-PROJECTED*".
3. **R3 (Level 7b, pool-relative novelty)** — footnote on guard box:
   "*no auxiliary learned model is queried — pool-relative to the
   LOCKED 88-citation master pool*".

---

## 2. Where each arrow in Figure 1 comes from (the interconnect layer)

See `agent-22-model-architect--subfigures.md` §4 for the full
interconnect table mapping each arrow to its axiom / proposition /
theorem anchor. At the master-figure level, the five critical arrows
are:

| # | Arrow (Level → Level) | Theoretical anchor |
|---|---|---|
| 1 | $D \to \{\phi_m(x)\}$ (Level 0 → 2a) | **Axiom 1** (grounded measurement) |
| 2 | $\{\phi_j, \phi_k\} \to \rho_{jk}$ (Level 1 → 2b) | **Axiom 2** (approximate orthogonality) |
| 3 | Level 2a + 2b → Level 3 ($|D'| = n \cdot (N + \binom{N}{2})$) | **Proposition 1** (signal-count identity) |
| 4 | Level 5 → Level 6 ($\{c_m\} \to C_P$) | **Axiom 3** + Phase 1 specification |
| 5 | Level 6 → Level 7a and Level 6 → Level 7b | **Proposition 4** (role composition); Phase 2 / Phase 3 of TCT |
| 6 | Level 7b → Level 8 ($G_\tau = 1$ accept) | **Proposition 3** + **Theorem 2** |
| 7 | Level 4 → Level 8 (effective sample requirement) | **Proposition 5** + **Theorem 1** |

---

## 3. What this figure is NOT (scope discipline)

- **Not** a neural-network computation graph. There are no layers,
  activations, or per-parameter updates rendered.
- **Not** a dataflow pipeline for any specific system. Context Graph
  and ClipCannon each **instantiate** Level 1 (at $N=13$ and $N=7$
  respectively), but their full internal DAGs appear in Figures 2 and
  3, not here.
- **Not** a causal / probabilistic graphical model. Arrows represent
  deterministic computation under frozen $\Phi$, not conditional
  dependencies.
- **Not** a closed theorem. Axioms 1–3 and their hedges are visible
  on the figure; the figure is a **composition diagram**, not a proof.

---

## 4. Figure 1 cross-reference table (for writers)

| Manuscript section | Figure 1 reference type |
|---|---|
| §1 Introduction | Figure 1 first cue; caption at §1 end as anchor for §3 + §5 teaching. |
| §3 Framework | Figure 1 is **the** §3 figure; §3.0 framing paragraph references every level by number. |
| §3.4 DDA counting | Cites Levels 2–3 and Proposition 1 anchor. |
| §4 Systems | Figure 2 (Context Graph) and Figure 3 (ClipCannon) are the §4 figures; each references Level 1 of Figure 1 as the common structural slot. |
| §5 TCT method | Figure 4 (TCT protocol) is the §5 figure; it expands Levels 4–8 of Figure 1. |
| §6 Case studies | §6.3 references Level 8 with Case 3's T1-measured values (0.961 mean SECS). |
| §7 Discussion | Figure 1 re-referenced for the composition-simplicity claim (P4). |

---

## 5. Notation reconciliation — $N$ vs. $M$

$N$ and $M$ are **not** the same quantity. The master figure is the
reference for the distinction:

- $N$ counts the **DDA panel cardinality** — the number of frozen
  embedders in $\Phi$ used to decompose $D$ at Level 1 and to label
  $D'$ at Level 3.
- $M$ counts the **TCT modality cardinality** — the number of
  modalities in $\mathcal{M}$ that appear in the constellation $C_P$
  at Level 6 and in the guard predicate at Level 7b.

In Context Graph's DDA role, $N = 13$. When TCT is applied to Case 3
(speaker modality only), $M = 1$. When applied to Case 2 (visual +
speaker), $M = 2$ at minimum (ClipCannon's 7 modality slots expose up
to $M = 7$). Therefore:

- $M \leq N$ when TCT is operated over a subset of the DDA panel.
- $M > N$ is possible if TCT is operated over modality slots that do
  not coincide with DDA's panel (e.g., cross-encoder hygiene
  modalities like WavLM-base-plus-sv in Case 3, which is **not** in
  Context Graph's E1–E13 panel — **WavLM is a dedicated speaker
  embedder outside the E1–E13 semantic panel**).

The figure uses distinct letters $N$ and $M$ at their respective
levels to make this visible to the reader.

---

## 6. Confidence

**HIGH** on the structural correctness and dependency ordering of the
figure. All levels trace to locked agent-04 primitives or agent-21
axioms / propositions / theorems. All hedges (V11, V18, V22, R3, R4)
are annotated on-figure. No new primitives introduced
(composition-not-invention MP-12).

**MEDIUM** on the final rendering aesthetic: the ASCII is load-bearing
(ordering and connectivity), but TikZ / Mermaid styling is a downstream
render decision for agent-37 / agent-38.

## Changelog

- 2026-04-17 — Initial issue by agent-22-model-architect.
