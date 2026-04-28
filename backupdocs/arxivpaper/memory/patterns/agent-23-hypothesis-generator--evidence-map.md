---
namespace: patterns
agent_id: agent-23-hypothesis-generator
timestamp: 2026-04-17T06:35:00Z
status: active
locked: false
supersedes: none
tags: evidence-map, hypothesis-pointers, t1-t2-t3-t4, citation-index, figure-anchors
depends_on:
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/patterns/agent-22-model-architect--subfigures.md
  - memory/patterns/agent-22-model-architect--variable-dictionary.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/patterns/agent-11-systematic-reviewer--evidence-grading.md
downstream_consumers:
  - agent-24-causal-simulator
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Evidence Map — Per-Hypothesis Evidence Pointer Table

## How to use this file

For **each hypothesis** in the ledger
(`decisions/agent-23-hypothesis-generator--hypotheses.md`) this file
enumerates:

- **Theoretical anchors** (axioms / propositions / theorems from
  agent-21).
- **Figure anchors** (Figure 1 arrows from agent-22).
- **Evidence items available** (T1 / T2 / T4 with pointers and tier
  grade from agent-11 if applicable).
- **Evidence items pending** (T1 measurement gaps / RO pointers from
  agent-19 / open questions from agent-18).
- **Citation pointers** (agent-08 LOCKED 88-pool entries relevant to
  the claim; no new citations introduced).
- **Falsification harness** (the specific measurement protocol that
  would falsify).

No new citations are introduced. Every citation reference points to
the agent-08 master reference pool ID (R1–R88 or the cluster code).

---

## 1. H1 — DDA signal-count identity $\lvert D' \rvert = n(N + \binom{N}{2})$

| Axis | Pointer |
|---|---|
| **Proposition** | P1 (signal-count identity), agent-21 §3 |
| **Theorem** | — (no theorem-level statement; P1 is sharp enough) |
| **Axioms** | Axiom 1 (grounded measurement), Axiom 2 (approximate orthogonality) |
| **Figure-1 arrows** | A1 ($D \to \phi_m(x)$), A2 ($\phi_j, \phi_k \to \rho_{jk}$), A3 (Levels 2a+2b → Level 3) |
| **Figure-2 anchor** | Level 3 signal-yield box ($91 = 13 + 78$) |
| **Figure-3 anchor** | Level 3 signal-yield box ($28 = 7 + 21$) |

### Evidence available

| Item | Tier | Source | agent-11 grade |
|---|---|---|---|
| **E1-1** P1 proof sketch | **T4** | agent-21-theory-builder §3 P1 | — |
| **E1-2** DDA formal definition §1.2.3 | **T4** | agent-04 dda-definition | — |
| **E1-3** $N=13$ Context Graph instantiation | **T2** | Mission brief "Key Architecture Facts"; agent-22 Figure 2 | — |
| **E1-4** $N=7$ ClipCannon instantiation | **T2** | Mission brief; agent-22 Figure 3 | — |
| **E1-5** Cluster-A scaling-law axis contrast | — | Cited: R-A1 (Kaplan 2020), R-A2 (Hoffmann 2022 Chinchilla) — axis-extension novelty (agent-10 EXT) | MODERATE |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E1-P1** Formal MI measurement $I(\phi_j(X); \phi_k(X))$ across E1–E13 | RO-8 (agent-19) | High |
| **E1-P2** Per-embedder signal non-constancy on representative corpus | RO-8 follow-up | High |
| **E1-P3** Effective-signal-count curve as a function of $N$ and panel redundancy | RO-8 + RO-1 | Medium |

### Citation pointers (LOCKED 88-pool)

- R-A1 Kaplan et al. 2020 — scaling-law axis reference.
- R-A2 Hoffmann et al. 2022 — Chinchilla compute-optimal.
- R-C1 Huang et al. 2024 — compression ↔ capability within-model.
- Cluster-B (data scaling) pool entries — referenced for
  axis-extension (not new citation).

### Falsification harness

**Protocol.** Instantiate $\Phi$ at $N = 13$ on the production Context
Graph corpus $D$ (~$10^4$–$10^6$ memories). Measure:

1. $\lvert D' \rvert$ by construction (count of non-trivial per-
   embedder projections + pairwise features). Expected: $91 \cdot
   \lvert D \rvert$ exactly (upper bound).
2. Mutual information $I(\phi_j(X); \phi_k(X))$ for a random sample
   of $\binom{13}{2} = 78$ pairs on a held-out subset.
3. Effective independent-information count using an MI-based
   estimator (to validate H1b hedge).

**Falsification thresholds.**
- If step 1 yields a count strictly less than $91 \cdot \lvert D \rvert$ (not
  counting zero-norm edge cases), H1 is falsified at the structural
  level.
- If step 2 reveals two or more pairs with $I / H > 0.9$ (near-
  complete dependence), H1 is pushed toward H1b and the headline
  "91 signals/input" must be restated as "up to 91".

---

## 2. H1a — Context Graph 91 signals/input

| Axis | Pointer |
|---|---|
| **Proposition** | P1 instantiation |
| **Figure anchor** | Figure 2 Level 3 box (91 = 13 + 78) |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E1a-1** 13 embedders specification (E1–E13 with dimensions) | **T2** | Mission brief "Key Architecture Facts"; agent-22 Figure 2 §1.2 |
| **E1a-2** E1–E13 operational in production | **T1** | MEMORY.md: "13 embedders, 15 HNSW indexes" |
| **E1a-3** Asymmetric embedders E5/E8/E10 dual-vector storage | **T1** | MEMORY.md: "Asymmetric embedders (E5/E8/E10) store dual vectors" |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E1a-P1** Audit that all 13 embedders produce non-constant signals on representative corpus | RO-8 | Medium |
| **E1a-P2** Removal or collapse of any $E_i, E_j$ to linear reparameterisations under panel | RO-8 | Medium |

### Falsification harness

**Protocol.** On Context Graph production corpus, for each of the
78 embedder pairs, compute cosine correlation distribution across
1,000 random inputs.
**Falsification threshold.** If $> 2$ pairs show mean cosine $> 0.95$
(strong linear dependence), the effective count is $< 91$.

---

## 3. H1b — Structural-upper-bound hedge (V11)

| Axis | Pointer |
|---|---|
| **Proposition** | P1 hedge (V11) |
| **Figure anchor** | Figure 1 Level 1 panel V11 annotation |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E1b-1** V11 hedge discipline | — | agent-01 V11; agent-04 DDA §1.2.4 |
| **E1b-2** "Structural upper bound, not info-theoretic lower bound" clause | **T4** | agent-21 P1 confidence note |
| **E1b-3** Open question Q20(iv) | — | agent-04 DDA §5 open items |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E1b-P1** RO-8 formal measurement | agent-19 RO-8 | High |
| **E1b-P2** Diminishing-returns curve measurement | RO-1 F1b | Medium |

### Falsification harness

Covered by H1 falsification harness. H1b is a **hedge**, not a
competing claim; falsifying H1 would also clarify H1b's status.

---

## 4. H2 — Constellation guard accept-rate bound

| Axis | Pointer |
|---|---|
| **Proposition** | P3 (geometric-over-scalar verifiability) |
| **Theorem** | Theorem 2 (runtime verifiability; $O(M \cdot d_{\max})$) |
| **Axioms** | Axiom 1, Axiom 2 (each modality carries non-redundant info) |
| **Figure-1 arrows** | A10 ($C_P \to G_\tau$), A11 ($\hat y \to \phi_m(\hat y)$), A12 ($G_\tau = 1 \Rightarrow \hat y$) |
| **Figure-4 anchor** | Phase 3 acceptance predicate |
| **Isomorphisms** | ISO-01 (RV monitor topology), ISO-07 (SPC tolerance bounds) |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E2-1** P3 proof sketch | **T4** | agent-21 §3 P3 |
| **E2-2** Theorem 2 proof sketch | **T4** | agent-21 §4 Theorem 2 |
| **E2-3** Cluster-I (scalar-reward) "no per-output threshold" contrast | **T2+** | Cited: R-I1 (InstructGPT), R-I2 (DPO), R-I3 (CAI) |
| **E2-4** Cluster-D (retrieval geometry) baseline | — | Cited: R-D cluster |
| **E2-5** Case 3 partial instantiation | **T1** | SITREP 2026-04-16; feeds H2a |
| **E2-6** ISO-01 formal-methods RV tradition | — | agent-20 ISO-01 |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E2-P1** Case 1, 2 T1 measurements | manuscript future work + Cases | High |
| **E2-P2** FP/FN curve vs $\tau$ | Q2 (agent-19) | High |
| **E2-P3** Threshold-calibration protocol (RO-5 hygiene) | agent-19 RO-5 | Medium |

### Citation pointers (LOCKED 88-pool)

- R-I1 Ouyang et al. 2022 (InstructGPT).
- R-I2 Rafailov et al. 2023 (DPO).
- R-I3 Bai et al. 2022 (Constitutional AI).
- R-D cluster (retrieval / contrastive geometry).
- R-E, R-F clusters (identity-locked generation baselines).

### Falsification harness

**Protocol.** On held-out $(\hat y, C_P, \tau)$ triples:
1. Compute $G_\tau(\hat y; C_P)$ 10 times; check determinism.
2. Verify no auxiliary learned model appears in the computation
   path (code audit of guard implementation).

**Falsification thresholds.**
- Non-determinism across the 10 runs → Axiom 1 violation, H2 falsified.
- Auxiliary learned model in the guard code path → Theorem 2 "no
  auxiliary model" clause falsified.

---

## 5. H2a — Case 3 voice-modality evidence (T1 MEASURED)

| Axis | Pointer |
|---|---|
| **Proposition** | P3; Theorem 2 |
| **Figure anchor** | Figure 4 Phase 3 output legend (Case 3 values) |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E2a-1** Mean SECS 0.961 (max 0.975) on 10 held-out English sentences | **T1 MEASURED** | SITREP 2026-04-16; white paper |
| **E2a-2** 9 of 10 $\geq 0.95$ | **T1 MEASURED** | SITREP 2026-04-16 |
| **E2a-3** Protocol: $K = 12, T = 0.3, \tau = 0.95$ | **T1 MEASURED** | SITREP 2026-04-16 |
| **E2a-4** 50-clip centroid ($n_m = 50$) | **T1 MEASURED** | SITREP 2026-04-16 |
| **E2a-5** WavLM-base-plus-sv scorer (arch.-independent of Context Graph E1–E13) | **T1 MEASURED** | agent-22 master-architecture §5 |
| **E2a-6** DNSMOS P.808 = 3.93 (parity real 3.93) | **T1 MEASURED** | SITREP 2026-04-16 |
| **E2a-7** UTMOS 3.012 (vs real 2.997) | **T1 MEASURED** | SITREP 2026-04-16 |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E2a-P1** Scorer diversity ablation (ECAPA-TDNN, Resemblyzer) | agent-17 Theme D | Medium |
| **E2a-P2** Multi-speaker extension | agent-14 Bias-05 | Medium |
| **E2a-P3** Multi-language extension | agent-14 Bias-06 | Low (out-of-scope) |

### Falsification harness

**Protocol (replication).** Re-run with:
- Same 50-clip reference set.
- Same 10 held-out English sentences.
- Same K=12, T=0.3, τ=0.95.
- Same WavLM-base-plus-sv scorer.

**Falsification thresholds.**
- Mean SECS $< 0.85$ on replication → H2a falsified.
- $< 5/10$ at $\geq 0.95$ → H2a falsified.

---

## 6. H2b — Multi-modal conjunction strictness

| Axis | Pointer |
|---|---|
| **Proposition** | P3 |
| **Figure anchor** | Figure 4 Phase 3 strict variant |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E2b-1** Elementary probability derivation ($\Pr[\cap E_m] \leq \min_m \Pr[E_m]$) | **T4** | Agent-23 derivation |
| **E2b-2** MP-01 (single-encoder similarity is weaker) | — | agent-16 MP-01 |
| **E2b-3** ISO-03 (biological multi-modal recognition metaphor) | — | agent-20 ISO-03 |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E2b-P1** Case 2 T1 measurement: $\Pr[G_\tau = 1]$ vs. $\min_m \Pr[s_m \geq \tau_m]$ | Case 2 future | High |

### Falsification harness

**Protocol.** On Case 2 held-out set, measure:
- Per-modality accept-rate $p_m = \Pr[s_m \geq \tau_m]$.
- Conjunctive accept-rate $p_\cap = \Pr[G_\tau = 1]$.

**Falsification threshold.** If $p_\cap \geq \min_m p_m$ within 1 %
(i.e., modalities are comonotonic at the threshold), H2b is
falsified — meaning modalities carry identical identity information,
which would also violate Axiom 2.

---

## 7. H2c — Case 1 text-style threshold prediction ($\tau \in [0.80, 0.92]$)

| Axis | Pointer |
|---|---|
| **Proposition** | P3; Axiom 3 |
| **Figure anchor** | Figure 4 Phase 3 threshold legend |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E2c-1** Case 3 analogy ($\tau = 0.95$ for voice) | **T1 MEASURED** | H2a evidence |
| **E2c-2** Agent-04 TCT $\tau \in [0.80, 0.95]$ prior | **T3** | agent-04 tct-definition |
| **E2c-3** E1 (e5-large-v2) cosine distributions on authorial text | — | Shakespeare LoRA pipeline scripts |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E2c-P1** Same-session-band floor measurement on 50–100 Shakespeare-style passages | Case 1 T1 future | High |
| **E2c-P2** Cross-encoder (non-E1) scorer on same held-out set | agent-17 Theme D; RO-5 | Medium |

### Falsification harness

**Protocol.** For 50–100 authentic Shakespeare-style passages,
compute:
- E1 cosine between adjacent passages from the same work
  (same-session-band).
- E1 cosine between passages from different authors
  (cross-session-band).
- Target $\tau$ = same-session-band floor.

**Falsification thresholds.**
- Same-session-band floor $< 0.75$ → H2c falsified (too strict).
- Same-session-band floor $> 0.95$ → H2c falsified (band too tight;
  insufficient intra-author variation captured by E1).
- Cross-encoder disagreement $> 0.10$ absolute cosine → H2c needs
  Theme D hedge carried forward.

---

## 8. H2d — Case 2 multi-modal threshold matrix

| Axis | Pointer |
|---|---|
| **Proposition** | P3; Axiom 3 |
| **Figure anchor** | Figure 3 modality panel; Figure 4 Phase 3 |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E2d-1** ClipCannon 7-modality panel spec | **T2** | agent-22 Figure 3 |
| **E2d-2** Case 3 speaker-audio anchor ($\tau = 0.95$) | **T1 MEASURED** | SITREP 2026-04-16 |
| **E2d-3** Encoder-specific within-identity concentration priors | **T3** | Cross-encoder heuristics (CLIP / SigLIP / WavLM literature) |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E2d-P1** Case 2 T1 measurement (EchoMimicV3 + LoRA + TCT Phases) | Case 2 future | High |
| **E2d-P2** Per-modality same-session-band floor on 2,362 clips | Case 2 future | High |
| **E2d-P3** Budget-exhaustion rate vs. $K$ | Case 2 future | Medium |

### Falsification harness

**Protocol.** Post-training, for each of the 5 modalities in the H2d
matrix:
- Measure same-session-band floor on held-out clips.
- Compare to predicted interval.

**Falsification thresholds.**
- Band floor outside predicted interval by $> 0.05$ on $\geq 2$ of
  5 modalities → H2d falsified.
- Budget-exhaustion rate $> 95 \%$ at $K = 12$ → threshold matrix
  is too strict; relaxed best-of-K is necessary.

---

## 9. H3 — Sample-complexity collapse $O(M \cdot k)$

| Axis | Pointer |
|---|---|
| **Proposition** | P5 (sample-complexity collapse) |
| **Theorem** | Theorem 1 (sharp collapse statement) |
| **Axioms** | Axiom 1, Axiom 2, Axiom 3 |
| **Figure-1 arrows** | A6 ($R_m \to \{v_{m,i}\}$), A7 ($\{v_{m,i}\} \to c_m$), A9 ($C_P \to \mathcal{L}_{\mathrm{TCT}}$), A13 ($\cup R_m \to \hat y$) |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E3-1** Theorem 1 proof sketch | **T4** | agent-21 §4 Theorem 1 |
| **E3-2** P5 derivation | **T4** | agent-21 §3 P5 |
| **E3-3** Axiom 3 anchor ($n_m \in [10, 10^2]$) | — | agent-21 Axiom 3 |
| **E3-4** Case 3 T1 anchor ($n_m = 50$) | **T1** | SITREP 2026-04-16 (feeds H3a) |
| **E3-5** Cluster-E / F conventional-baseline contrast (thousands of samples) | **T2+** | agent-16 MP-05 |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E3-P1** Sample-complexity curve per modality | RO-10 (agent-19) | High |
| **E3-P2** Multi-modality $M > 1$ T1 measurement | Case 2 future | High |
| **E3-P3** Independence-from-$\lvert D \rvert$ demonstration (large vs. small corpus) | Case 1 + Case 2 | Medium |

### Citation pointers

- R-E cluster (identity-locked text generation baselines).
- R-F cluster (identity-locked audio / voice cloning baselines).
- Cluster-F baselines with thousands of subject-specific samples.

### Falsification harness

**Protocol.** On a single modality (voice, Case 3 setup):
- Vary $n_m \in \{10, 25, 50, 100, 200, 500, 1000\}$.
- For each, construct centroid, run Phase 3 on 10 held-out
  sentences, measure mean SECS.
- Vary $\lvert D \rvert$ (general corpus size) independently: re-run
  with truncated / augmented general corpora.

**Falsification thresholds.**
- Mean SECS does not reach 0.95 at $n_m \leq 10^2$ → Axiom 3
  falsified, H3 falsified.
- Mean SECS depends measurably on $\lvert D \rvert$ (i.e., larger
  general corpus → worse identity-lock) → Theorem 1
  "$\lvert D \rvert$-independence" falsified.

---

## 10. H3a — Case 3 $n_m = 50$ T1-measured (T1 MEASURED)

| Axis | Pointer |
|---|---|
| **Theorem** | Theorem 1 |
| **Axiom** | Axiom 3 ($n_m \in [10, 10^2]$) |
| **Figure anchor** | Figure 4 Phase 1 Case-3 T1 legend |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E3a-1** $n_m = 50$ reference-set size | **T1 MEASURED** | SITREP 2026-04-16 |
| **E3a-2** Mean SECS 0.961 on held-out | **T1 MEASURED** | SITREP 2026-04-16 |
| **E3a-3** 16-minute voice capture as source | **T1** | Mission brief |
| **E3a-4** Qwen3-TTS + ICL-only Phase 2 degenerate-to-Phase-3 best-of-K | **T1** | agent-04 TCT V18 "inference-only" mode |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E3a-P1** $n_m$ ablation (10, 25, 50, 100) | future replication | High |
| **E3a-P2** Scorer ablation (ECAPA-TDNN, Resemblyzer) | agent-17 Theme D | Medium |

### Falsification harness

Covered by H3 falsification harness, restricted to Case 3 single-
modality, single-subject setup.

---

## 11. H3b — Case 2 $n_m = 2{,}362$ projected

| Axis | Pointer |
|---|---|
| **Theorem** | Theorem 1 |
| **Figure anchor** | Figure 3 Level 3 ("2,362 clips") |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E3b-1** Mission-brief $n_m = 2{,}362$ | **T2** | Mission brief |
| **E3b-2** Case-3 same 16-minute source | **T1** | SITREP 2026-04-16 |
| **E3b-3** ClipCannon DAG 23-stage pipeline | **T2** | agent-22 Figure 3 |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E3b-P1** Case 2 T1 measurement end-to-end | Case 2 future | High |
| **E3b-P2** Identity-lock rate on held-out scenes | Case 2 future | High |

### Falsification harness

**Protocol.** Run Case 2 end-to-end (EchoMimicV3 + LoRA + TCT Phase 2
+ Phase 3) on 2,362 clips. Evaluate on held-out scenes + prompts.

**Falsification thresholds.**
- Training does not converge with $n_m = 2{,}362$ → H3b falsified.
- Post-training identity-lock rate $< 50 \%$ on held-out → H3b
  falsified.

---

## 12. H4 — Collapse-silence by construction

| Axis | Pointer |
|---|---|
| **Proposition** | P2 (collapse silence) |
| **Axiom** | Axiom 1 (grounded measurement) |
| **Figure-1 arrow** | A5 (absence: $D \not\ni \tilde x \sim p_{\theta_t}$) |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E4-1** P2 proof sketch (scope argument) | **T4** | agent-21 §3 P2 |
| **E4-2** Shumailov 2024 collapse theorem statement (pre-condition = generator-in-loop) | — | Cited: R-ColA (Shumailov 2024) |
| **E4-3** Dohmatob 2024 collapse extensions | — | Cited: R-ColB (Dohmatob 2024) |
| **E4-4** Alemohammad 2023 self-consuming loops | — | Cited: R-ColC (Alemohammad 2023) |
| **E4-5** agent-10 cluster-C orthogonal-path (ORTH-B) novelty code | — | agent-10 |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E4-P1** Toy empirical demonstration: recursive vs. DDA-enriched training dynamics | future work | Medium |

### Citation pointers

- R-ColA Shumailov et al. 2024 (Nature, "The Curse of Recursion").
- R-ColB Dohmatob et al. 2024.
- R-ColC Alemohammad et al. 2023.

### Falsification harness

**Protocol.** Build two toy pipelines:
- **A** Recursive self-distillation: generator trained on its own
  outputs for $T$ rounds.
- **B** DDA-enriched: frozen $\Phi$ applied to fixed corpus, $D'$
  used as supervisory signals for a single training run (no
  recursion).

Measure distributional drift after $T$ rounds.

**Falsification thresholds.**
- **B** shows Shumailov-type drift comparable to **A** → H4
  falsified (only if confound causes ruled out).
- **B** is shown to secretly contain a recursion (e.g., $\Phi$ is
  updated on generator outputs) → H4 falsified at construction level
  (V18 violation).

---

## 13. H5 — Meaning compression as a distinct category

| Axis | Pointer |
|---|---|
| **Proposition** | P1 (ratio view) |
| **Figure-1 arrow** | A4 (ratio view); §5.2 framework contrast |
| **Isomorphism** | ISO-04 (Kolmogorov / MDL / rate-distortion) |

### Evidence available

| Item | Tier | Source |
|---|---|---|
| **E5-1** Unit-analytic derivation (4 unit pairs) | **T4** | agent-04 meaning-compression §2.3; agent-21 §5.2 |
| **E5-2** Huang 2024 compression ↔ capability correlation | — | Cited: R-C1 (Huang 2024) |
| **E5-3** Kaplan 2020 / Hoffmann 2022 Chinchilla scaling | — | Cited: R-A1, R-A2 |
| **E5-4** GPTQ / SparseGPT / distillation literature (weight-compression) | — | Cited: R-wC cluster |
| **E5-5** KV-cache quant / TurboQuant literature (activation-compression) | — | Cited: R-aC cluster |
| **E5-6** Shannon rate-distortion classics | — | Cited: R-InfoThm entries in pool |
| **E5-7** agent-10 CAT-EXT novelty code | — | agent-10 |
| **E5-8** Tension 2 resolution (Huang within-model vs V11 across-model) | — | agent-17 tension-resolution |

### Evidence pending

| Item | Source | Priority |
|---|---|---|
| **E5-P1** Kaplan / Chinchilla exponent re-fit under DDA-enriched supervision | RO-1 F1b (agent-19) | High |
| **E5-P2** Engagement with Huang r≈0.95 on multi-embedder setup | future work | Medium |

### Citation pointers

- R-A1 Kaplan 2020 — scaling-law roots.
- R-A2 Hoffmann 2022 — Chinchilla.
- R-C1 Huang 2024 — compression ↔ capability.
- R-wC cluster — weight-compression literature.
- R-aC cluster — activation-compression literature.
- R-InfoThm cluster — Shannon / MDL / rate-distortion roots.

### Falsification harness

**Protocol.** Given any classical compression unit $u_{\text{cls}}$
(bits/bits, accuracy/parameters, memory/quality), attempt a linear
transformation $T$ such that $u_{\text{meaning}} = T(u_{\text{cls}})$.

**Falsification thresholds.**
- If such a $T$ exists → meaning-compression reduces to classical
  category, H5 falsified.
- If $\binom{N}{2}$ pairwise term contributes zero non-redundant
  information on realistic corpora → H5 keeps categorical
  distinctness but loses quantitative headline (H1b applies;
  not H5 falsification).

---

## 14. Cross-hypothesis dependency graph

```
   Axiom 1 ────────┬───────► H1, H1a, H1b, H2, H2a, H2b, H2c, H2d, H3, H3a, H3b, H4
                  │
   Axiom 2 ─────────────────► H1, H1b, H5  (V11-hedged)
                  │
   Axiom 3 ─────────────────► H3, H3a, H3b  (directly), H2c, H2d (via stability)
                  │
   P1 ─────────────────────► H1, H1a, H1b, H5 (ratio view)
   P2 ─────────────────────► H4
   P3 ─────────────────────► H2, H2a, H2b, H2c, H2d
   P4 ─────────────────────► (implicit across role-composition, not headline-hypothesised)
   P5 ─────────────────────► H3, H3a, H3b
   Theorem 1 ───────────────► H3, H3a, H3b
   Theorem 2 ───────────────► H2, H2a
```

**Single-failure collapse nodes** (if falsified, take down a whole
family):
- **Axiom 1** → takes down H1–H4 (frozen-embedder discipline is
  load-bearing for all measurement / signal / sample-complexity
  claims).
- **H2a T1 replication failure** → takes down the paper's single
  headline T1 anchor; reframes Case 3 from "measured" to "reported".
- **H3a T1 replication failure** → takes down the 50-clip anchor,
  weakens Theorem 1's practical $k$ value.

---

## 15. Confidence

**HIGH** on coverage (every hypothesis has explicit
theoretical-anchor, figure-anchor, T-tier evidence available,
evidence pending, and falsification harness).

**HIGH** on citation-pointer discipline (no new citations; every
R-ID maps to the LOCKED 88-pool).

**MEDIUM** on H2d / H3b evidence-pending density (Case 2 has the
most T1 measurements still owed).

## Changelog

- 2026-04-17 — Initial issue by agent-23-hypothesis-generator.
