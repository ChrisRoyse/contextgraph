---
namespace: discoveries
agent_id: agent-10-theoretical-framework-analyst
timestamp: 2026-04-16T20:00:00Z
status: active
supersedes: none
tags: theoretical-gaps, open-problems, future-work, section-10, epistemology
depends_on:
  - memory/discoveries/agent-10-theoretical-framework-analyst--framework-map.md
  - memory/patterns/agent-10-theoretical-framework-analyst--theoretical-contributions.md
  - memory/discoveries/agent-09-methodology-scanner--methodological-gaps.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
downstream_consumers:
  - agent-11-systematic-reviewer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-41-future-work-writer
  - agent-42-adversarial-reviewer
---

# Theoretical Gaps Our Paper Exposes but Does NOT Close

This file collects **open theoretical problems** that the manuscript surfaces
by virtue of its contributions but does not formally resolve. Each gap is:

1. **Exposed**, not invented — the gap is created by a claim we make in
   §3 / §5 and would not exist if we did not make that claim.
2. **Open**, not closed — we do not claim to resolve it; this is future
   work for §10 and honest limitation material for §8.
3. **Evidence-grounded** — traced to a specific construct, a specific
   cluster, and (where applicable) a specific hedge already in the
   glossary.

These gaps are **distinct from agent-09's methodological gaps**: those
are comparator methods the existing literature lacks (and that we fill
with our protocols). The gaps below are **theoretical** — open
conceptual problems whose resolution would require additional research
beyond this manuscript.

**Structure per gap.**
- **Gap statement** — one sentence naming the open problem.
- **Origin** — which of our contributions surfaces it.
- **Framework lineage** — which cluster's theory would need to be
  extended / broken to close it.
- **Why we do not close it here** — scope / resource / plausibility.
- **How to close it** — a sketch of the research programme that could.
- **Confidence that the gap is open** — our assessment that no existing
  work closes it.

---

## GAP-1. Formal embedder-independence measurement

**Gap statement.** There is no operational protocol for measuring the
**information-theoretic independence** of $N$ frozen embedders
$\{\phi_m\}_{m=1}^{N}$ applied to the same input; hence the
$\binom{N}{2}$ pairwise count in DDA is an **upper bound**, not a
measured signal yield.

**Origin.** Contribution 1 (DDA axis) and Contribution 3 (role-flip).
The count $n(N + \binom{N}{2})$ depends on the assumption that pairwise
interaction features $\rho_{jk}$ are non-degenerate; if two embedders
collapse to a linear reparameterisation of each other, their pair
contributes no new information.

**Framework lineage.**
- **Cluster C (compression-as-intelligence)** would need a cross-
  compressor mutual-information protocol; current theory is per-compressor.
- **Cluster D (embedders / retrieval)** would need a formal definition
  of "approximately independent" beyond the glossary #14 hedge.
- **Information theory** broadly would provide the substrate (MI
  estimation, canonical correlation analysis), but no cited paper in our
  master pool applies it to N-embedder panels for supervisory-signal
  counting.

**Why we do not close it here.** Our claim is structural (the count
$\binom{N}{2}$ is the correct structural quantity) rather than
informational (how many bits each pair actually carries). The
information-theoretic measurement requires large-scale MI estimation
across 13 (Context Graph) or 7 (ClipCannon) embedders on held-out
corpora; that is a separate research project.

**How to close it.**
- Define a mutual-information-adjusted MC ratio (the "Secondary
  indicator" in `meaning-compression-definition.md` §2.5):
  $\mathrm{MC}_{\mathrm{MI}}(\Phi, D) = \sum_m I(\phi_m(X); Y) +
  \sum_{j<k} I(\rho_{jk}(X); Y \mid \phi_{1:j-1}, \phi_{j+1:k-1},
  \phi_{k+1:N})$ for a target $Y$.
- Run MI estimators (e.g., MINE, InfoNCE lower bounds) on each
  embedder-pair slot.
- Report the ratio of measured MI to the count-bound to estimate
  embedder-panel redundancy.

**Confidence the gap is open.** **HIGH**. Agent-06's cluster scan and
agent-09's methodology scan both confirm no paper in the master pool
reports cross-embedder MI estimates for N-embedder panels.

**Manuscript placement.** §8 Limitations (glossary #14 hedge); §10
Future Work (the MI protocol).

---

## GAP-2. Scaling-law modification under DDA-enriched data

**Gap statement.** If $D$ is reinterpreted as
$n(N + \binom{N}{2})$ structured signals, the Kaplan / Chinchilla
power-law exponents $\alpha, \beta$ may shift, be preserved, or
saturate; no empirical measurement of this shift exists.

**Origin.** Contribution 1 (DDA axis). Re-interpreting $D$ is the
contribution; measuring the consequent scaling-law behaviour is not.

**Framework lineage.**
- **Cluster A (scaling laws)**: Kaplan 2020; Hoffmann 2022; Villalobos
  2024. The power-law form would be re-fitted under DDA-enriched data;
  the exponents would be re-estimated; the compute-optimal
  tokens-per-parameter becomes signals-per-parameter.
- **Cluster C (compression-as-intelligence)**: Huang 2024's linear
  compression–capability correlation would need to be re-evaluated with
  meaning-compression on the x-axis.

**Why we do not close it here.** A full scaling-law study requires
training many models across scales with DDA-enriched supervision. That
is a multi-month compute-heavy project beyond this manuscript's scope
(mission brief P04 / FM-03 forbids AGI overclaim; we honestly flag the
projected outcome as unknown).

**How to close it.**
- Select a model family (GPT-2/3/Pythia-style).
- Train two matched scales: one with token-count-measured $D$; one with
  DDA-enriched signals-per-input $D'$ where the additional signals are
  used as auxiliary losses or as explicit targets.
- Fit the power law; compare exponents.

**Confidence the gap is open.** **HIGH**. No Cluster-A paper fits a
scaling law under DDA-enriched supervision.

**Manuscript placement.** §10 Future Work (primary slot); §8 Limitations
(V22 projected flag on any claim that the data-wall crossover moves).

---

## GAP-3. Theoretical status of the collapse theorem under frozen-embedder supervision

**Gap statement.** Shumailov et al. (2024)'s self-consumption collapse
theorem is **silent** on the DDA regime (frozen embedders on unchanged
real inputs); we argue categorically that DDA does not satisfy the
generator-in-loop precondition, but no theorem has been proven that
states "frozen-embedder supervision on unchanged inputs does not induce
tail-mass loss or variance contraction".

**Origin.** Contribution 1 (DDA) + our §3.3 categorical distinction.
The distinction is sound at the definitional level; a formal theorem
would strengthen it.

**Framework lineage.**
- **Cluster B (model collapse)**: Shumailov 2024 Nature; Alemohammad
  2023 MAD; Dohmatob 2024 tails; Gerstgrasser 2024 accumulation; Feng
  2024 verification; Seddik 2024 statistical analysis.
- The theorem would need to formalise "the collapse theorem does not
  apply when supervisory signals are deterministic projections of real
  inputs through frozen embedders".

**Why we do not close it here.** Our contribution is a **scope claim**,
not a theorem. The scope claim is correct and needed; a full theorem
would require a generative-dynamics analysis that adds depth but is not
load-bearing for the rest of the manuscript.

**How to close it.**
- State the generator-in-loop condition formally (per Shumailov).
- Show that DDA does not satisfy the condition (embedders are
  independently trained on disjoint data; inputs are not generated).
- Derive a "non-collapse" corollary: the DDA signal distribution is a
  deterministic function of the real input distribution, so the
  iterative-distributional-drift theorem does not fire.

**Confidence the gap is open.** **MEDIUM-HIGH**. The scope claim is
uncontroversial once the definitions are read side by side; a formal
theorem would be a courtesy to theoretical reviewers, not a requirement.

**Manuscript placement.** §3.3 (current scope claim); §10 Future Work
(optional formal theorem).

---

## GAP-4. Whether TCT generalises to non-identity targets (safety profiles, behavioural constraints)

**Gap statement.** TCT is defined in this manuscript for **identity-
locked generation** — the subject is a specific person / style and
the constellation represents their identity. It is **unknown** whether
the same protocol (frozen centroids + per-output geometric gate)
generalises to non-identity targets such as safety profiles, ethical
constraints, domain-specific behavioural specifications, or
counterfactual-identity constraints.

**Origin.** Contribution 4 (TCT BRK) + Contribution 6 (identity-locked
predicate). The manuscript frames TCT's target as identity; the
abstraction to arbitrary geometric targets is an obvious next step that
this paper does not take.

**Framework lineage.**
- **Cluster I (alignment)**: safety-oriented RLHF / Constitutional AI
  target behavioural profiles. A safety-centroid formulation would
  replace the identity centroid with a safety centroid (e.g., "safe
  conversation" cluster mean in an embedding space).
- **Cluster F (video generation)**: behavioural constraints (no violence,
  no deepfake of a non-consenting subject) could be encoded as
  negative constellations (reject outputs that are too close to a
  "forbidden" centroid) — this is a conceptual extension beyond our
  current "positive centroid" construction.

**Why we do not close it here.** Every safety-centroid claim would
require a safety dataset, a safety embedder, a calibration protocol,
and an ethics review. Case 3 is identity-locked voice cloning; it is
ethically and technically a different project from safety-locked
generation.

**How to close it.**
- Define a **negative constellation** $C_P^{-} = \{(m, c_m^{-})\}$ of
  forbidden centroids (e.g., deepfake of X, violent content, protected-
  identity impersonation).
- Amend the guard: accept iff $\forall m^+ \in C_P^+ : s_{m^+} \geq
  \tau_{m^+}$ AND $\forall m^- \in C_P^- : s_{m^-} \leq
  \tau_{m^-}^{-}$.
- Run an ethics study; measure false-positive and false-negative rates
  on a safety benchmark.

**Confidence the gap is open.** **HIGH**. Our manuscript does not even
attempt negative constellations; this is entirely open.

**Manuscript placement.** §10 Future Work (primary slot, with ethics
framing per Q19); §8 Limitations (explicit scope of TCT as identity-
locked only in this paper).

---

## GAP-5. Theoretical equivalence (or non-equivalence) between constellation-distance loss and an implicit reward

**Gap statement.** In principle, any loss function can be reinterpreted
as the negative of a reward function. The question is whether the
constellation-distance loss $\sum_m \lambda_m (1 - \cos(\phi_m(\hat{y}),
c_m))$ corresponds to a **coherent preference ordering** that could be
recovered by an RLHF-style Bradley-Terry model; and if so, what
properties that implicit preference has. We claim TCT is **not** RLHF
(Contribution 4 BRK); a theoretical treatment would either (i) formalise
why not, or (ii) reveal that the two are closer than this manuscript's
framing suggests.

**Origin.** Contribution 4 (TCT BRK against Cluster I). The BRK claim is
defended on four structural grounds in `theoretical-contributions.md`
§4; a formal non-equivalence proof would strengthen it.

**Framework lineage.**
- **Cluster I (alignment)**: Ouyang 2022 InstructGPT; Rafailov 2023 DPO
  (explicit implicit-reward formulation); Bai 2022 CAI.
- **Energy-based models / prototypical networks (Cluster D, not in
  master pool as Snell 2017 but flagged)**: an energy function over
  outputs can be read as a negative reward.

**Why we do not close it here.** A formal non-equivalence proof
requires a preference-order analysis (does the distance induce a
consistent total order on outputs?) and a Bradley-Terry-recovery
analysis (can the implicit reward be estimated from generated-accepted-
rejected triples?). These are well-scoped theory problems but not the
manuscript's focus.

**How to close it.**
- Treat the guard's accept/reject trace across many outputs as
  preference-like data.
- Fit a Bradley-Terry model to see if it recovers the cosine distance.
- If yes: TCT is a degenerate RLHF with a cosine-to-centroid reward (our
  BRK claim weakens).
- If no: state the non-recoverability formally (our BRK claim
  strengthens).

**Confidence the gap is open.** **HIGH**. No paper in the master pool
performs this equivalence analysis.

**Manuscript placement.** §8 Limitations (explicit statement that the
BRK claim is structural, not formally proved); §10 Future Work (the
equivalence / non-equivalence analysis).

---

## GAP-6. Cross-encoder SECS as an identification condition — no theoretical characterisation

**Gap statement.** Our Case 3 SECS protocol requires the scoring encoder
(WavLM-base-plus-sv) to be **architecturally distinct** from any encoder
the TTS model (Qwen3-TTS) was conditioned on. This is introduced as a
*good-hygiene* condition to avoid "scoring with your own training
signal" bias (glossary #20). It is **not theoretically characterised** —
there is no formal definition of what "architecturally distinct" buys
in terms of bias bound.

**Origin.** Contribution 5 (constellation guard) as instantiated in
Case 3, §6.3.

**Framework lineage.**
- **Cluster E (speaker-encoder verification)**: Desplanques 2020 ECAPA;
  Chen 2022 WavLM. Both papers report EERs; neither characterises the
  bias of scoring with the conditioning encoder.
- **Causal-inference / identification theory** (not in master pool):
  the analogy is leave-one-out / held-out-instrument identification.

**Why we do not close it here.** A formal bias bound would require a
joint model of the TTS model, the conditioning encoder, and the
scoring encoder; large-scale ablations across encoder pairs; and a
statistical characterisation of the within-encoder vs. cross-encoder
gap. Out of scope.

**How to close it.**
- Train / evaluate K TTS models each with a different conditioning
  encoder; score each with all K encoders.
- Characterise the gap $\Delta = \overline{\mathrm{SECS}}_{\mathrm{same
  encoder}} - \overline{\mathrm{SECS}}_{\mathrm{cross encoder}}$.
- Derive a bias bound in terms of $\Delta$.

**Confidence the gap is open.** **HIGH**. No cited Cluster-E paper runs
cross-encoder scoring ablations of this kind.

**Manuscript placement.** §8 Limitations (hedge the cross-encoder
condition); §10 Future Work (full ablation).

---

## GAP-7. The WavLM-SECS same-session band (0.95–0.99) has no theoretical basis

**Gap statement.** Glossary #17 / V12 / V17: the same-session band is
an empirical observation on the Case 3 speaker's real recordings; no
theory predicts its range or its speaker-specific variance. We use it
as a comparator for Case 3's clone SECS, and we flag it as our own
observation at every use.

**Origin.** Contribution 6 (identity-locked predicate) as instantiated
in Case 3 with $\tau_{\mathrm{WavLM-SECS}}$ calibrated to the band.

**Framework lineage.**
- **Speaker-verification theory** (Chen 2022 WavLM; Desplanques 2020
  ECAPA): reports EER on VoxCeleb / NIST-SRE; does not characterise
  within-session distributions.
- **Signal-processing theory**: within-session variance should be
  driven by acoustic environment, recording device, and short-term
  physiological variation; no cited paper models this for WavLM.

**Why we do not close it here.** Characterising the same-session band
across many speakers and many encoders is a multi-speaker psychophysics
study; we have one speaker's data.

**How to close it.**
- Collect same-session recordings from K (≥ 50) speakers.
- Compute within-session SECS distributions per encoder.
- Report median / IQR / tail; test whether 0.95 is a lower-decile
  approximation across speakers.

**Confidence the gap is open.** **HIGH**. No published
speaker-verification paper reports within-session WavLM-SECS
distributions by our scan.

**Manuscript placement.** §6.3 (with V12 / V17 hedge at every first
use); §8 Limitations (explicit disclosure); §10 Future Work.

---

## GAP-8. Scaling of the constellation guard to many modalities and many subjects

**Gap statement.** The constellation guard is defined for a single
subject with M modalities; Case 3 uses M=1 (WavLM), Case 2 projects
M=5 (visual, speaker, voice, emotion, FLAME/expression). There is no
theoretical characterisation of how the **accept-rate** behaves as M
grows, as the constellation set grows across many subjects, or as
per-modality thresholds drift.

**Origin.** Contribution 5 (constellation guard). The guard is well-
defined at any M, but the probabilistic analysis (what is the accept
rate under random generation?) is not provided.

**Framework lineage.**
- **Hypothesis-testing / multiple comparisons theory** (not in master
  pool): the conjunctive predicate $\forall m: s_m \geq \tau_m$ has
  accept-rate that behaves like a multiple-test intersection.
- **Rejection-sampling theory**: the expected sample budget to first
  accept grows with the product of per-modality rejection rates.

**Why we do not close it here.** Empirically, Case 3 reports K=12 budget;
Case 2 projects a budget that is not yet measured. A theoretical
characterisation needs per-modality acceptance-rate estimates, which
require large-scale sweeps we have not run.

**How to close it.**
- For each modality, estimate $\Pr[s_m \geq \tau_m]$ from many
  generated samples.
- Under approximate independence, estimate
  $\Pr[\text{accept}] \approx \prod_m \Pr[s_m \geq \tau_m]$.
- Derive a budget bound $K = O(1 / \Pr[\text{accept}])$.
- Characterise the safety / budget trade-off for threshold-setting.

**Confidence the gap is open.** **HIGH**. No cited paper (across D / E /
F / I) reports multi-modal guard accept-rate scaling.

**Manuscript placement.** §8 Limitations (budget discussion); §10 Future
Work.

---

## GAP-9. Representation-alignment theory — why frozen-embedder cosine is a meaningful identity metric

**Gap statement.** We treat cosine similarity to a frozen centroid as a
**meaningful alignment metric** for identity. Why this is true in any
formal sense — why cosine rather than Euclidean, why to a centroid rather
than to the reference set directly, why L2-normalisation of both the
embeddings and the mean — is inherited from **folk practice in
embedder evaluation** (speaker verification's cosine convention; FAISS
default metric). A theoretical characterisation of "when cosine to a
centroid is the right metric" is absent.

**Origin.** Contribution 5 (constellation guard) + Contribution 6
(predicate). Both rely on cosine-to-centroid as the geometric primitive.

**Framework lineage.**
- **Cluster D (embedders)**: SigLIP, WavLM, ColBERT — all use cosine
  conventions inherited from contrastive pretraining; the convention
  is not theoretically justified per embedder but rather chosen by
  architectural convenience.
- **Representation-alignment literature** (CKA, SVCCA — not in master
  pool): representation-space alignment has formal definitions but is
  not the same construct as cosine-to-centroid for identity.

**Why we do not close it here.** A principled analysis would need to
relate the pretraining objective (e.g., sigmoid contrastive in SigLIP)
to the downstream appropriateness of cosine-to-centroid; this is a
per-embedder analytical project.

**How to close it.**
- For each embedder family, derive (or cite) the implicit metric
  induced by the pretraining loss.
- Compare cosine-to-centroid to alternative metrics (Mahalanobis,
  Gaussian likelihood in the embedding space).
- Show accept-rate / precision / recall trade-offs empirically.

**Confidence the gap is open.** **MEDIUM-HIGH**. Folk practice is
widespread; principled analysis per embedder is not.

**Manuscript placement.** §8 Limitations (explicit folk-practice
disclosure); §10 Future Work (alternative-metric ablation).

---

## GAP-10. Formalising "teleology" as a property, not merely a name

**Gap statement.** TCT is named "teleological" because the aim (the
constellation) is fixed, not learned. We state this philosophically but
do **not** formalise the property. A formalisation — e.g., "T
teleological iff the loss gradient does not flow into the aim" —
would allow mechanical checks and invite proofs about which of our
three phases preserve / break the property.

**Origin.** Contribution 4 (TCT as geometric frame). The name carries
theoretical weight; the weight is not discharged by a formal definition.

**Framework lineage.**
- **Philosophical lineage** (Aristotelian *telos*; mentioned in §3.3
  of `tct-definition.md`). Philosophical; not formal-ML theory.
- **Frozen-parameters analysis** (implicit in LoRA, Hu et al., 2022):
  parameters may be frozen, but the property "teleological" is stronger
  — the aim itself is frozen, not just some parameters.

**Why we do not close it here.** A formal definition requires careful
scoping: does "aim" mean the centroid? The threshold? The embedder? A
precise definition would allow future work on "partially teleological"
variants (e.g., freeze centroids but adapt thresholds).

**How to close it.**
- Define: a training protocol is teleological iff (i) there exists a
  set of parameters $\Theta^{\mathrm{aim}}$ such that no loss gradient
  flows into any $\theta \in \Theta^{\mathrm{aim}}$ and (ii) the
  acceptance predicate at inference depends only on
  $\Theta^{\mathrm{aim}}$ and the generator's output.
- Prove TCT satisfies both conditions in every phase.
- Derive corollaries for variants that relax condition (i) (e.g.,
  thresholds adapted online).

**Confidence the gap is open.** **HIGH**. The philosophical framing is
present in our paper; no formalisation is attempted.

**Manuscript placement.** §8 Limitations (explicit statement that
"teleological" is a philosophical-naming choice, not a formal
property); §10 Future Work.

---

## Gap summary table

| # | Gap | Originating contribution | Framework(s) to extend | Priority | Confidence open | Manuscript slot |
|---|---|---|---|---|---|---|
| 1 | Formal embedder-independence measurement | DDA (1), Role-flip (3) | C, D, info theory | CRITICAL | HIGH | §8, §10 |
| 2 | Scaling-law modification under DDA | DDA (1) | A, C | HIGH | HIGH | §10 |
| 3 | Collapse theorem formally silent on DDA | DDA (1) | B | MEDIUM | MEDIUM-HIGH | §3.3, §10 |
| 4 | TCT for non-identity targets (safety) | TCT (4), guard (5) | I, F | HIGH | HIGH | §10, ethics |
| 5 | TCT-reward equivalence / non-equivalence | TCT (4) | I | HIGH | HIGH | §8, §10 |
| 6 | Cross-encoder SECS identification theory | Guard (5), Case 3 | E | MEDIUM | HIGH | §8, §10 |
| 7 | Same-session band without theoretical basis | Predicate (6), Case 3 | E | MEDIUM | HIGH | §6.3, §8, §10 |
| 8 | Guard scaling to many modalities / subjects | Guard (5) | rejection sampling, mult-comp | HIGH | HIGH | §8, §10 |
| 9 | Cosine-to-centroid as alignment metric | Guard (5), Predicate (6) | D, rep-align | MEDIUM | MEDIUM-HIGH | §8, §10 |
| 10 | Formal property of "teleological" | TCT (4) | philosophy → formal ML | LOW-MEDIUM | HIGH | §8, §10 |

**CRITICAL gaps** (must be in §10 future work): 1.
**HIGH gaps** (should be in §10): 2, 4, 5, 8.
**MEDIUM gaps** (§8 disclosure + §10 mention): 3, 6, 7, 9.
**LOW-MEDIUM** (§8 disclosure): 10.

---

## Relation to mission-brief Q20 (future-work essential question)

Mission brief / `essential-questions.md` Q20 enumerates future-work items
(i–iv). The ten gaps here map:

- **Q20 (i) — Formal independence measurement** ↔ GAP-1 (direct match).
- **Q20 (ii) — Scaling-law under DDA** ↔ GAP-2.
- **Q20 (iii) — Non-identity TCT targets** ↔ GAP-4.
- **Q20 (iv) — Per-embedder metric justification** ↔ GAP-9 (partial).
- **[additional; agent-10-proposed]** — GAP-3, GAP-5, GAP-6, GAP-7,
  GAP-8, GAP-10.

Agent-10 proposes that §10 Future Work be structured around GAPs 1, 2,
4, 5, 8 as primary (CRITICAL / HIGH) and the remainder as secondary /
paragraph-level mentions. Agent-41 (future-work-writer) owns final
structure.

---

## Confidence statement

**HIGH.** Every gap is traced to a specific contribution of ours and to
a specific cluster's theory; every "why we do not close it" is
scope-honest (we do not claim the gap would be closed if we had a bit
more compute); every "how to close it" is a plausible research
programme, not a hand-wave. The ten-gap set is exhaustive for the
current manuscript's scope; additional gaps may be surfaced by adversarial
review (agent-42) and by PRISMA expansion (agent-11), at which point
this file is updated (not superseded — the gap list grows monotonically).

**Locked elements.**
- Gap numbering 1–10 (add new gaps as 11, 12, … if needed; do not
  renumber).
- Priority labels CRITICAL / HIGH / MEDIUM / LOW-MEDIUM.
- "Confidence the gap is open" reflects agent-10's scan of the master
  pool; agent-11 may revise upward / downward on PRISMA sweep.

**Open for agent-11 / agent-41.**
- Whether GAP-1 and GAP-5 warrant explicit §10 subsections of their own
  (recommended YES for both).
- Whether GAP-4 deserves paired ethics-framing in §8.6 (recommended
  YES; ties to Q19 detector).
- Whether the runtime-verification import (Contribution 5, VERIF-IMPORT)
  needs an adjacent-discipline citation added to the pool to anchor
  §7 Discussion.

---

## Changelog

- 2026-04-16 — Initial issue by agent-10-theoretical-framework-analyst.
