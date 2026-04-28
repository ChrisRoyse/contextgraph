---
namespace: patterns
agent_id: agent-22-model-architect
timestamp: 2026-04-17T05:50:00Z
status: active
locked: false
supersedes: none
tags: subfigures, figure-2, figure-3, figure-4, context-graph, clipcannon, tct-protocol, interconnect
depends_on:
  - memory/decisions/agent-22-model-architect--master-architecture.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
downstream_consumers:
  - agent-23-hypothesis-generator
  - agent-37-methodology-writer
  - agent-38-systems-writer
  - agent-39-discussion-writer
---

# Sub-Figures 2, 3, 4 — Instance Diagrams of the Spine

This file specifies the three sub-figures that accompany the master
Figure 1. Each sub-figure is an **instance** of one or more levels of
the master architecture. The convention is:

- **Figure 2** = instance of Level 1 at $N=13$ (Context Graph).
- **Figure 3** = instance of Level 1 at $N=7$ (ClipCannon video).
- **Figure 4** = expanded view of Levels 4–8 (TCT Phase 1/2/3).

---

## 1. Figure 2 — Context Graph as a DDA instance ($N = 13$, 91 signals/input)

### 1.1 Role in the manuscript

§4.1 figure. Shows how the Context Graph's 13 embedders (E1–E13)
populate **Level 1** of the master architecture, and therefore produce
the **91 signals/input** headline number cited in §3.4 and §4.1.

### 1.2 Structural ASCII

```
                 ┌─────────────────────────────────────────────┐
                 │   INPUT:  a single raw memory x ∈ 𝒳         │
                 │   (text, code, or conversation snippet)     │
                 └─────────────────────────────────────────────┘
                                       │
                    ┌──────────────────┼──────────────────┐
                    │   FROZEN PANEL   Φ = {E1, …, E13}   │
                    │   (Context Graph cluster, N = 13)   │
                    └──────────────────┬──────────────────┘
                                       ▼
    ┌─────────────────────────────────────────────────────────────────────┐
    │  LEVEL 1 — 13 PARALLEL EMBEDDER HEADS  (all frozen; no gradients)   │
    ├─────────────────────────────────────────────────────────────────────┤
    │                                                                     │
    │  E1   semantic          1024-D dense        e5-large-v2             │
    │  E2   recency           512-D dense         sinusoidal PE temporal  │
    │  E3   decay             512-D dense         multi-scale temporal    │
    │  E4   ordering          512-D dense         positional/session      │
    │  E5   causal            768-D dense asym    cause→effect dual       │
    │  E6   sparse keyword    30,522-D sparse     BM25-style              │
    │  E7   code              1536-D dense        Qodo-Embed              │
    │  E8   graph             1024-D dense asym   structural             │
    │  E9   HDC               1024-D dense        hyperdimensional        │
    │  E10  paraphrase        768-D dense asym    multilingual            │
    │  E11  entity            768-D dense         KEPLER/TransE          │
    │  E12  late-interaction  128-D/token         ColBERT MaxSim          │
    │  E13  SPLADE            30,522-D sparse     learned-sparse          │
    │                                                                     │
    │  Total dense   ≈  1024+512+512+512+768+1536+1024+1024+768+768+128   │
    │               ≈  ~11,008 dense dim aggregated                       │
    │  + 2 × 30,522 sparse + 128/token late-interaction                   │
    └─────────────────────────────────────────────────────────────────────┘
                    │                                     │
                    │ per-embedder  (13 × 1 = 13)         │ pairwise cross-corr
                    ▼                                     ▼
      ┌─────────────────────────────┐       ┌──────────────────────────────┐
      │  LEVEL 2a  — 13 signals     │       │  LEVEL 2b — 78 ρ_{jk} pairs  │
      │                             │       │  (C(13,2) = 78)              │
      │  {φ_1(x), φ_2(x), …, φ_13(x)}│      │                              │
      │                             │       │  ρ_{jk}(x) = cos(φ_j(x),     │
      │  per-embedder labeled       │       │               φ_k(x))        │
      │  vectors                    │       │  after per-embedder L2-norm  │
      └─────────────────────────────┘       └──────────────────────────────┘
                    │                                     │
                    └─────────────────┬───────────────────┘
                                      ▼
                ┌───────────────────────────────────────────┐
                │  SIGNAL YIELD (Proposition 1 at N = 13):  │
                │                                           │
                │    per input:  N + C(N,2)                 │
                │              = 13 + 78                    │
                │              = 91                         │
                │                                           │
                │  |D'| = 91 · n  (n = |corpus|)            │
                │                                           │
                │  Downstream roles (P4):                   │
                │   ▸ RETRIEVER (search_graph, MCP)         │
                │   ▸ LABELLER (DDA supervisory signal)     │
                │   ▸ GUARD  (TCT Phase 3, Case 1)          │
                │                                           │
                │  All three roles consume identical        │
                │  frozen φ_m instances.                    │
                └───────────────────────────────────────────┘
                                      │
                                      ▼
             ┌──────────────────────────────────────────────┐
             │  DOWNSTREAM CONSUMERS (Level 4+ in Figure 1) │
             │                                              │
             │   •  RocksDB   (59 column families)          │
             │   •  HNSW index  (15 per-embedder indexes +  │
             │                   Matryoshka shards)         │
             │   •  MCP tool surface (75 tools)             │
             │   •  Phoenix / TCT constellation guard (§5)  │
             └──────────────────────────────────────────────┘
```

### 1.3 Caption (paste-ready)

> **Figure 2. Context Graph as an instance of Derived Data Abundance
> (DDA) at $N = 13$.** Every ingested memory $x$ is projected through
> the 13-embedder frozen panel $\Phi = \{E_1, \ldots, E_{13}\}$,
> producing 13 per-embedder labeled vectors and $\binom{13}{2} = 78$
> pairwise cross-correlation features. The total structured supervisory
> yield is $13 + 78 = 91$ signals per input (Proposition 1;
> `dda-definition.md` §1.2.3, T2 instantiation). The same frozen
> instances are simultaneously used as (i) a **retriever** by
> Context Graph's search surface, (ii) a **labeller** for DDA
> supervisory signals, and (iii) a **runtime guard** in TCT Phase 3
> (Proposition 4, role-composition coherence).

### 1.4 On-figure hedges

- V11 clause on the panel box ("approximately independent").
- V18 discipline on the "frozen" label (embedders are frozen; LoRA
  adapters, when used, do not violate V18 per agent-04).
- Pool-relative R3 clause on the "three roles over identical
  instances" annotation.

---

## 2. Figure 3 — ClipCannon as a DDA instance on video ($N = 7$, 4,044 dim, 23-stage DAG)

### 2.1 Role in the manuscript

§4.2 figure. Shows how ClipCannon's 7-modality video-processing DAG
populates **Level 1** of Figure 1 on video inputs and feeds Case 2's
Phase-1 constellation construction (Level 5–6 of the master figure).

### 2.2 Structural ASCII

```
              ┌─────────────────────────────────────────────────┐
              │   INPUT:  raw video clip x ∈ 𝒳_video            │
              │   (16 min source → 2,362 curated clips)         │
              └─────────────────────────────────────────────────┘
                                        │
                                        ▼
     ┌──────────────────────────────────────────────────────────────┐
     │         CLIPCANNON DAG  (23 stages, pipeline order)          │
     │                                                              │
     │  stage 1  : ingest + decode                                  │
     │  stage 2  : scene segmentation                               │
     │  stage 3  : frame sampling                                   │
     │  …        :                                                  │
     │  stage 23 : output serialisation                             │
     │                                                              │
     │         (internal stages invoke the 7 modalities below;      │
     │          the DAG orchestrates compute, caching, and output)  │
     └──────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
     ┌──────────────────────────────────────────────────────────────┐
     │   FROZEN PANEL  Φ = 7 modality heads  (ClipCannon cluster)   │
     ├──────────────────────────────────────────────────────────────┤
     │                                                              │
     │   m1  visual-global     (CLIP / SigLIP image encoder)        │
     │   m2  visual-local      (patch / crop descriptors)           │
     │   m3  motion            (optical-flow / temporal encoder)    │
     │   m4  audio-speech      (WavLM / speech features)            │
     │   m5  audio-non-speech  (music / environment)                │
     │   m6  text-caption      (ASR + captioning)                   │
     │   m7  layout            (scene graph / composition)          │
     │                                                              │
     │   Aggregated dense feature width: 4,044 dim                  │
     │   (N = 7 ⇒  7 + C(7,2) = 7 + 21 = 28 signals/input)          │
     └──────────────────────────────────────────────────────────────┘
                                        │
                    ┌───────────────────┴────────────────────┐
                    │                                        │
                    ▼                                        ▼
       ┌─────────────────────────┐             ┌────────────────────────────┐
       │  LEVEL 2a — 7 signals   │             │  LEVEL 2b — 21 ρ_{jk} pairs│
       │  per clip               │             │  (C(7,2) = 21)             │
       └─────────────────────────┘             └────────────────────────────┘
                    │                                        │
                    └─────────────────────┬──────────────────┘
                                          ▼
                 ┌──────────────────────────────────────────────┐
                 │  SIGNAL YIELD  (Proposition 1 at N = 7):     │
                 │                                              │
                 │     per clip:  7 + 21 = 28                   │
                 │                                              │
                 │  Over 2,362 curated clips:                   │
                 │     |D'| = 28 × 2,362 ≈  66,136 signals      │
                 │                                              │
                 │  ClipCannon DAG downstream output:           │
                 │     ≥ 12,000 labeled training samples        │
                 │     (T2 constructive, mission brief)         │
                 └──────────────────────────────────────────────┘
                                          │
                                          ▼
   ┌────────────────────────────────────────────────────────────────┐
   │  CASE 2 USE (identity-locked talking-head clone, §6.2)         │
   │                                                                │
   │   Phase 1 (Figure 1 Level 5–6):                                │
   │      c_m  =  normalize( (1/2362) · Σ  normalize(φ_m(r_i)) )    │
   │   for each modality m in the selected 𝒞_2 subset of ℳ.         │
   │                                                                │
   │   Phase 2 (Figure 1 Level 7a):                                 │
   │      EchoMimicV3 + LoRA; mouth-weighted flow-matching loss     │
   │      augmented with  Σ_m λ_m · (1 − cos(φ_m(ŷ), c_m))          │
   │                                                                │
   │   Phase 3 (Figure 1 Level 7b):                                 │
   │      per-output G_τ guard across selected modalities           │
   │                                                                │
   │   Status: T3-PROJECTED (not yet T1-measured)                   │
   │                                                                │
   │   58 MCP tools expose the pipeline and artefacts.              │
   └────────────────────────────────────────────────────────────────┘
```

### 2.3 Caption (paste-ready)

> **Figure 3. ClipCannon as an instance of DDA on video at $N = 7$.**
> ClipCannon's 23-stage DAG ingests raw video and drives a 7-modality
> frozen panel (global visual, local visual, motion, speech audio,
> non-speech audio, text caption, layout) with a 4,044-dim aggregated
> feature width. Per clip, $N + \binom{N}{2} = 7 + 21 = 28$ structured
> signals are produced (Proposition 1). Over 2,362 curated clips
> drawn from a single 16 min source interview, the derived dataset
> contains $\approx 66{,}136$ structured signals; the pipeline
> downstream assembles $\geq 12{,}000$ labeled training samples. In
> Case 2 (identity-locked talking-head clone, §6.2) ClipCannon's
> frozen modality heads are reused for TCT Phase 1 to compute
> per-modality centroids $c_m$, and for TCT Phase 3 to run the
> runtime guard. The 58-tool MCP surface exposes the pipeline and
> its artefacts. Case 2 numbers are T3-projected pending
> camera-ready re-verification.

### 2.4 On-figure hedges

- "T3-PROJECTED" watermark over Case 2 box.
- V11 clause on the panel box.
- Explicit note that the DAG's 23 stages are pipeline orchestration,
  **not** 23 independent embedders (Axiom 2 applies to the 7
  modality heads only, not to the DAG step count).

---

## 3. Figure 4 — The TCT three-phase protocol, expanded

### 3.1 Role in the manuscript

§5 figure. Expands Levels 4–8 of Figure 1 into a detailed per-phase
flow so writers and reviewers see exactly what happens at Phase 1,
Phase 2, and Phase 3, and where the per-output acceptance predicate
is evaluated.

### 3.2 Structural ASCII

```
╔═══════════════════════════════════════════════════════════════════════════════╗
║                    PHASE 1 — CONSTELLATION CONSTRUCTION                       ║
║                                                                               ║
║   subject P ──► reference sets {R_m}_{m∈ℳ}                                    ║
║                  │                                                            ║
║                  │   for each m:                                              ║
║                  ▼                                                            ║
║          ┌─────────────────────────────────────────────┐                      ║
║          │   v_{m,i} := normalize( φ_m( r_{m,i} ) )    │  (φ_m frozen;        ║
║          │   for i = 1 … n_m                           │   Axiom 1)           ║
║          └─────────────────────────────────────────────┘                      ║
║                  │                                                            ║
║                  ▼                                                            ║
║          ┌─────────────────────────────────────────────┐                      ║
║          │   c_m := normalize( (1/n_m) Σ_i v_{m,i} )   │  ← centroid          ║
║          └─────────────────────────────────────────────┘                      ║
║                  │                                                            ║
║                  ▼                                                            ║
║          ╔═════════════════════════════════════════════╗                      ║
║          ║   C_P  :=  { (m, c_m)  :  m ∈ ℳ }           ║ ← SPINE              ║
║          ║   (frozen for Phases 2, 3)                  ║                      ║
║          ╚═════════════════════════════════════════════╝                      ║
║                                                                               ║
║   Cardinality contract: |C_P| = M                                             ║
║   Sample-size bound (Axiom 3): n_m ∈ [10, 10²]                                ║
║   T1 anchor: Case 3 voice, n_m = 50                                           ║
╚═══════════════════════════════════════════════════════════════════════════════╝
                                        │
                                        │   C_P  (frozen)
                                        ▼
╔═══════════════════════════════════════════════════════════════════════════════╗
║            PHASE 2 — CONSTELLATION-CONDITIONED FINE-TUNING                    ║
║                                                                               ║
║   inputs:                                                                     ║
║     • generator g_θ : 𝒵 → 𝒴                                                   ║
║     • constellation C_P  (from Phase 1)                                       ║
║     • frozen embedders {φ_m}  (same instances as Phase 1)                     ║
║     • DDA-enriched training data D'  (from Figure 1 Level 3)                  ║
║     • per-modality loss weights λ_m ≥ 0                                       ║
║                                                                               ║
║   per training step:                                                          ║
║                                                                               ║
║     z, y_target  ←   sample(D')                                               ║
║     ŷ            :=  g_θ(z)                                                   ║
║                                                                               ║
║     L_TCT(θ)  :=   L_gen(θ;  z, y_target, ŷ)                                  ║
║                  + Σ_{m ∈ ℳ}  λ_m · ( 1 − cos( φ_m(ŷ), c_m ) )                ║
║                                                                               ║
║     θ  ←  θ − η · ∇_θ  L_TCT(θ)                                               ║
║                                                                               ║
║   training-mode discipline (V18):                                             ║
║     ▸ "frozen-with-LoRA"   : default Case 1 / Case 2                          ║
║     ▸ "inference-only"     : Case 3 (Qwen3-TTS backbone unchanged;            ║
║                              Phase 2 degenerates to Phase 3 best-of-K)        ║
║                                                                               ║
║   output: θ*  (trained generator)                                             ║
║                                                                               ║
║   ◄── Proposition 5, Theorem 1 (sample-complexity collapse)                   ║
║   ◄── Axiom 1 (φ_m frozen throughout)                                         ║
╚═══════════════════════════════════════════════════════════════════════════════╝
                                        │
                                        │   θ*  (or unchanged generator)
                                        ▼
╔═══════════════════════════════════════════════════════════════════════════════╗
║                  PHASE 3 — RUNTIME CONSTELLATION GUARD                        ║
║                                                                               ║
║   inputs:                                                                     ║
║     • trained generator g_{θ*}                                                ║
║     • constellation C_P                                                       ║
║     • per-modality thresholds {τ_m}                                           ║
║     • conditioning z ;  sample budget K                                       ║
║                                                                               ║
║   STRICT (conjunctive) variant:                                               ║
║   ┌───────────────────────────────────────────────────────────────────────┐   ║
║   │    for k = 1 … K:                                                     │   ║
║   │        ŷ^{(k)}  :=  g_{θ*}( z ;  seed_k )                             │   ║
║   │        for each m ∈ ℳ:                                                │   ║
║   │            s_m^{(k)}  :=  cos( φ_m( ŷ^{(k)} ), c_m )                  │   ║
║   │        if  ∀ m :  s_m^{(k)} ≥ τ_m :                                   │   ║
║   │            return  ŷ^{(k)}  with trace  { s_m^{(k)} }_{m∈ℳ}           │   ║
║   │    (budget exhausted — reject / escalate)                             │   ║
║   └───────────────────────────────────────────────────────────────────────┘   ║
║                                                                               ║
║   RELAXED (best-of-K) variant  (Case 3 practice):                             ║
║   ┌───────────────────────────────────────────────────────────────────────┐   ║
║   │    draw K candidates  { ŷ^{(1)}, … , ŷ^{(K)} }                        │   ║
║   │    k*  :=  argmax_k  Σ_{m∈ℳ}  w_m · cos( φ_m(ŷ^{(k)}), c_m )          │   ║
║   │    return  ŷ^{(k*)}  with trace  { s_m^{(k*)} }_{m∈ℳ}                 │   ║
║   │    (strict predicate may be applied post-hoc for QA)                  │   ║
║   └───────────────────────────────────────────────────────────────────────┘   ║
║                                                                               ║
║   acceptance predicate (formal):                                              ║
║                                                                               ║
║         G_τ( ŷ ;  C_P )  =  𝟙[ ∀ m ∈ ℳ :  cos( φ_m(ŷ), c_m ) ≥ τ_m ]          ║
║                                                                               ║
║   output: accepted ŷ   +   deterministic cosine trace { s_m(ŷ) }              ║
║                                                                               ║
║   ◄── Proposition 3 (geometric verifiability)                                 ║
║   ◄── Theorem 2 ( O(M · d_max), no auxiliary learned model )                  ║
║   ◄── ISO-01  (formal-methods runtime verification)                           ║
║   ◄── ISO-07  (statistical process control tolerance bounds)                  ║
╚═══════════════════════════════════════════════════════════════════════════════╝
                                        │
                                        ▼
                ┌──────────────────────────────────────────┐
                │   identity-locked ŷ  +  provenance trace │
                │                                          │
                │   Case 3 T1-MEASURED:                    │
                │     K = 12,  T = 0.3,                    │
                │     τ_WavLM-SECS = 0.95                  │
                │     mean SECS = 0.961 (max 0.975)        │
                │     DNSMOS 3.93  (parity 3.93 real)      │
                │     UTMOS 3.012  (parity 2.997 real)     │
                │                                          │
                │   Cases 1, 2: T3-PROJECTED               │
                └──────────────────────────────────────────┘
```

### 3.3 Caption (paste-ready)

> **Figure 4. Teleological Constellation Training — the three-phase
> protocol.** **Phase 1** constructs the frozen constellation $C_P$
> from per-modality reference sets $R_m$ of subject $P$ by taking the
> L2-normalised mean of L2-normalised $\phi_m$-embeddings. **Phase 2**
> fine-tunes a generator $g_\theta$ against $C_P$ under the objective
> $\mathcal{L}_{\mathrm{TCT}}(\theta) =
> \mathcal{L}_{\mathrm{gen}}(\theta) +
> \sum_{m \in \mathcal{M}} \lambda_m (1 - \cos(\phi_m(\hat y), c_m))$;
> the V18 training-mode discipline admits "frozen-with-LoRA" (Cases 1
> and 2) and "inference-only" (Case 3). **Phase 3** runs the
> constellation guard at inference: for each candidate $\hat y$, the
> predicate $G_\tau(\hat y; C_P) = \mathbb{1}[\forall m :
> \cos(\phi_m(\hat y), c_m) \geq \tau_m]$ is evaluated; the strict
> variant conjunctively accepts when every per-modality cosine meets
> its threshold, and the relaxed best-of-$K$ variant (used in Case 3)
> argmaxes the weighted sum across $K$ candidates. A deterministic
> cosine trace $\{s_m(\hat y)\}$ attaches to every accepted output as
> provenance. Theorem 1 bounds subject-specific sample complexity at
> $O(M \cdot k)$ independent of corpus size; Theorem 2 bounds
> per-output guard cost at $O(M \cdot d_{\max})$ and requires no
> auxiliary learned model. The guard's paradigm sources are dual:
> formal-methods runtime verification (ISO-01) and statistical
> process control (ISO-07).

### 3.4 On-figure hedges

- V18 discipline label at Phase 2 ("frozen-with-LoRA" vs
  "inference-only").
- V22 / R4 watermark on the Case 3 measured / projected legend.
- ISO-01 + ISO-07 dual-attribution ribbon at Phase 3.
- R3 pool-relative note on the "no auxiliary learned model" clause
  (Theorem 2 anchor).

---

## 4. Interconnect table — how Figure 1's arrows map to propositions / theorems

This is the master **arrow-to-proposition** map. Every load-bearing
arrow in Figure 1 (master) — and, by inheritance, every corresponding
arrow in Figures 2, 3, 4 — resolves to one or more entries from
agent-21's theoretical framework.

| Arrow ID | Figure 1 level flow | Interpretation | Axiom | Proposition | Theorem | Isomorphism |
|---|---|---|---|---|---|---|
| **A1** | $D \to \phi_m(x)$ (Level 0 → 2a) | Every frozen embedder is applied to every input; no gradient flows back | **Axiom 1** | — | — | — |
| **A2** | $(\phi_j, \phi_k) \to \rho_{jk}$ (Level 1 → 2b) | Pairwise cross-correlation feature is a non-constant function on approximately-independent axes | **Axiom 2** | — | — | ISO-02 (sensor fusion) |
| **A3** | Levels 2a + 2b → Level 3 | Assembly of $D'$; the $n(N + \binom{N}{2})$ counting identity holds | Axioms 1, 2 | **P1** | — | ISO-04 (MDL ratio view) |
| **A4** | Level 3 → Level 3' "meaning-compression ratio" | $(N + \binom{N}{2})$ signals / input is a unit-analytically distinct compression category | Axioms 1, 2 | P1 | — | ISO-04 |
| **A5** | $D \not\ni \tilde x \sim p_{\theta_t}$ (DDA not in generator loop) | DDA's signals are decomposition, not synthesis — Shumailov-2024 preconditions unsatisfied | **Axiom 1** | **P2** | — | — |
| **A6** | $R_m \to \{v_{m,i}\}$ (Level 4 → 5 precursor) | Subject-$P$ reference set is embedded by the same frozen $\phi_m$ used at Level 1 | Axiom 1 | **P4** (role composition) | — | — |
| **A7** | $\{v_{m,i}\} \to c_m$ (Level 5 internal) | L2-normalised mean of L2-normalised embeddings converges in expectation; stable at $n_m \in [10, 10^2]$ | **Axiom 3** | P5 (pre-cursor) | — | — |
| **A8** | $\{c_m\} \to C_P$ (Level 5 → 6) | Spine instantiation; Phase-1 output | Axiom 3 | — | — | ISO-06 (astronomical constellation) |
| **A9** | $C_P \to L_{\mathrm{TCT}}(\theta)$ (Level 6 → 7a) | Phase-2 loss target; geometric-proximity term anchors generator to frozen $c_m$ | Axiom 1 | **P4** | **Theorem 1** (sample complexity) | — |
| **A10** | $C_P \to G_\tau(\hat y; C_P)$ (Level 6 → 7b) | Phase-3 guard reference; same frozen object as loss target | Axiom 1 | **P3** (geometric verifiability), **P4** | **Theorem 2** (runtime verifiability) | ISO-01 (RV) + ISO-07 (SPC) |
| **A11** | $\hat y \to \phi_m(\hat y)$ (Level 7 internal) | Guard evaluates the same $\phi_m$ on generated $\hat y$; role-composition | Axiom 1 | **P4** | Theorem 2 | — |
| **A12** | $G_\tau = 1 \Rightarrow \hat y$ (Level 7b → 8) | Per-output acceptance; trace $\{s_m(\hat y)\}$ attached | — | **P3** | **Theorem 2** | ISO-01, ISO-07 |
| **A13** | $\bigcup_m R_m \to \hat y$ (Level 4 → 8 end-to-end) | Subject-specific data contribution bounded at $\sum_m n_m = O(M \cdot k)$ | Axiom 3 | **P5** | **Theorem 1** | — |

### 4.1 Cross-reference quick map

| Claim (mission brief §) | Figure 1 anchor arrows | Spine element |
|---|---|---|
| Claim 1 — DDA signal count (§3.4) | A1, A2, A3 | P1 |
| Claim 2 — meaning compression (§3.1, §5.2) | A3, A4 | P1 (ratio view) |
| Claim 3 — TCT per-output verifiability (§5.3) | A10, A11, A12 | P3, Theorem 2 |
| Claim 4 — sample-complexity collapse (§6.3) | A6, A7, A9, A13 | P5, Theorem 1 |
| Role-composition (§3 opener) | A1, A6, A11 (all reuse $\phi_m$) | P4 |
| Collapse-silence (§3.3) | A1 + A5 "absence" | P2 |

---

## 5. Cross-figure consistency rules

1. **Frozen-symbol consistency.** The glyph $\phi_m$ appears in
   Figures 1, 2, 3, 4 with identical meaning (a frozen embedder). The
   same instance appears in DDA decomposition (Figs. 1, 2, 3), in
   constellation construction (Fig. 4 Phase 1), and in the guard
   (Fig. 4 Phase 3). This is Proposition 4's visual invariant.
2. **$N$ vs $M$ consistency.** In Figure 2, $N = 13$; in Figure 3,
   $N = 7$. In Figure 4, $M = |\mathcal{M}|$ is written abstractly
   because TCT is applied to different $M$ across Cases 1, 2, 3.
3. **Hedge consistency.** V11, V18, V22, R3, R4 appear on every
   figure in which they apply; the writer may not remove a hedge in
   favour of cleaner visuals.
4. **Role-composition consistency.** Every place where $\phi_m$ is
   invoked is annotated with its role (retriever / labeller / guard)
   so Proposition 4 is legible from the figure alone.

---

## 6. Confidence

**HIGH** on correctness and internal consistency of Figures 2, 3, 4
as instance diagrams of Figure 1. **MEDIUM** on rendering aesthetic
(ASCII is load-bearing; TikZ / Mermaid render is downstream). Every
arrow in every figure maps to a proposition / theorem entry in §4
above.

## Changelog

- 2026-04-17 — Initial issue by agent-22-model-architect.
