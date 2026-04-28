---
namespace: patterns
agent_id: agent-10-theoretical-framework-analyst
timestamp: 2026-04-16T19:45:00Z
status: active
supersedes: none
tags: theoretical-contributions, novelty, extensions, framework-breaks, epistemology, writer-ready
depends_on:
  - memory/discoveries/agent-10-theoretical-framework-analyst--framework-map.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/discoveries/agent-09-methodology-scanner--methodological-gaps.md
downstream_consumers:
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Our Paper's Theoretical Contributions — Mapped onto Existing Frameworks

This file is the **writer-ready table of contributions** for §1 Introduction,
§3 Framework, §5 TCT, and §7 Discussion. Every contribution is classified
according to the four-code scheme from the framework map (EXT / COMP / BRK /
ORTH) plus a **contribution-type** code:

- **AXIS-EXT** — extends an existing framework along a new axis the framework
  permits but did not exercise.
- **CAT-EXT** — adds a new category to an existing taxonomy, preserving the
  existing categories.
- **ROLE-FLIP** — reuses an existing framework's substrate but inverts the
  operator direction (e.g., ranker → labeller).
- **POS-EXT** — extends a framework by *removing* a feature (e.g., positive-
  only training in a contrastive framework).
- **PRED** — introduces a new predicate / acceptance concept not present in
  any related framework.
- **VERIF-IMPORT** — imports a construct from an adjacent discipline (formal
  methods / runtime verification) into ML.

Every row is evidence-grounded against a specific cluster + citation from
the LOCKED master reference pool; no new citations.

---

## Contribution 1 — DDA extends scaling-law data-axis

**Contribution-type.** AXIS-EXT.

**Statement.**
> Derived Data Abundance (DDA) preserves the Kaplan–Chinchilla scaling-law
> form $L(N, D, C) = a N^{-\alpha} + b D^{-\beta} + \ldots$ but re-interprets
> the data axis $D$ from "raw token count" to "structured-signal count
> $n(N + \binom{N}{2})$" under a fixed corpus projected through $N$ frozen
> independent embedders.

**Existing frameworks extended.**
- **Kaplan et al. (2020)** — Scaling Laws for Neural Language Models. The
  power-law form in $D$ is preserved; the unit of $D$ is reinterpreted.
- **Hoffmann et al. (2022)** — Training Compute-Optimal LLMs (Chinchilla).
  The compute-optimal argument is preserved; the tokens-per-parameter
  prescription is a special case of signals-per-parameter when signals = 1
  per input.
- **Villalobos et al. (2024)** — Will We Run Out of Data? The finite-corpus
  argument is preserved; the data-wall crossover moves to the right once
  signals-per-input > 1.

**Compatibility code.** **EXT** — the existing frameworks permit the axis
reinterpretation (they never prohibit counting in non-token units); the
reinterpretation has not previously been exercised.

**What the existing frameworks do NOT claim.** Neither Kaplan, Chinchilla,
nor Villalobos forecast the scaling-law trajectory under DDA-enriched data.
The open question (scaling-law modification under DDA) is explicitly carried
to §10 future work (see `theoretical-gaps.md` GAP-1).

**Our evidence.** §3.4 DDA count (TD); §4.1 Context Graph N=13 → 91
signals/input (T2 instantiation); §4.2 ClipCannon N=7 → 28 signals/input
(T2 instantiation).

**Hedges carried.** V14 (approximately independent; strict formal
orthogonality is §10 open work). V22 (projected / pending flag on any
claim that the scaling-law crossover moves; the scaling-law modification
under DDA is not measured in this paper).

**Manuscript home.** §3.1 (first mention with V6 "not synthetic data"
pointer); §3.4 (formal count with $\binom{N}{2}$ per V15); §3.5 (placement
in the four-category compression taxonomy); §10 Future Work (scaling-law
under DDA).

**Adversarial-reviewer target.** A reviewer from Cluster A will read "DDA
extends scaling laws" as a claim to refit Kaplan's power law. Our claim is
**narrower**: the power-law form is preserved; the unit of $D$ is
reinterpreted. FM-03 forbids the broader claim.

---

## Contribution 2 — Meaning compression adds a fourth compression category

**Contribution-type.** CAT-EXT.

**Statement.**
> Meaning compression is a fourth compression category alongside bit
> compression, weight compression, and activation compression; its unit
> column (signals per unit of raw data) is dimensionally distinct from the
> other three.

**Existing frameworks extended.**
- **Delétang et al. (2024)** — Language Modeling Is Compression. Prediction-
  compression duality is preserved as a property of a *trained predictor*;
  our contribution flips the direction and treats compression as a
  *data-preparation* operator.
- **Huang et al. (2024)** — Compression Represents Intelligence Linearly.
  The empirical correlation ($r \approx 0.95$) is preserved as a property
  of models; our category addition does not contradict it.
- **Li et al. (2025) Nature Machine Intelligence; Valmeekam et al. (2023)
  LLMZip; Jiang et al. (2023) gzip-k-NN** — all preserved as instances of
  bit compression (Category i).
- **Adjacent weight-compression referents** (GPTQ Frantar et al. 2022,
  SparseGPT Frantar & Alistarh 2023, distillation Hinton et al. 2015):
  preserved as Category ii.
- **Adjacent activation-compression referents** (TurboQuant, KV-cache
  quantisation): preserved as Category iii.

**Compatibility code.** **EXT (CAT-EXT)** — the four-category taxonomy
preserves the three existing categories exactly; meaning compression is
the orthogonal fourth.

**Why orthogonal, narrowly.** Per §3.1 of `meaning-compression-definition.md`:
the four categories operate on **dimensionally-distinct unit pairs**:
- bit: bits / bits (dimensionless),
- weight: accuracy / parameters,
- activation: memory / output quality,
- meaning: signals / unit raw data.

A system can realise all four simultaneously. Meaning compression does not
substitute for any of them; it adds an axis.

**What the existing frameworks do NOT claim.** No prior compression paper
states that compression can be used to *prepare training data* by running
multiple compressors (embedders) on the same input and counting the
signal yield. Our contribution makes this explicit.

**Our evidence.** §3.1 four-category taxonomy table (TD); §3.4 count (TD);
§4.1 / §4.2 instantiations (T2).

**Hedges carried.** Orthogonality is *dimensional* (unit analysis), not
*information-theoretic* (the four categories are not claimed to be
information-theoretically independent). V5 forbids "semantic compression";
V9 first-use rule mandates the four-category statement.

**Manuscript home.** §3.1 (four-category taxonomy, first use with V9
four-category statement); §3.5 (taxonomy table); §7 Discussion (positioning).

**Adversarial-reviewer target.** A reviewer will ask: "Is meaning
compression really compression?". The answer, per §2.2.2 of the meaning-
compression definition: it is a ratio of signals per raw-data unit; it is
**increase in signal density at fixed raw-data volume**, which is the
dual of the three existing categories' "decrease in resource at fixed
content". Same compression family, different direction.

---

## Contribution 3 — Embedder role-flip: ranker → labeller

**Contribution-type.** ROLE-FLIP.

**Statement.**
> The multi-modal embedder literature (SigLIP, Nomic, WavLM, ColBERT,
> SPLADE, KEPLER/TransE, HNSW, RRF) positions embedders as *retrievers*
> (ranking artefacts by similarity to a query). DDA flips the role:
> embedders become *labellers* producing $N + \binom{N}{2}$ supervisory
> signals per input.

**Existing frameworks extended.**
- **Zhai et al. (2023) SigLIP** — sigmoid contrastive loss remains the
  embedder's training objective; the downstream role changes.
- **Nussbaum et al. (2024) Nomic Embed** — long-context text-embedder
  remains; role shifts from retrieval to labelling.
- **Chen et al. (2022) WavLM** — masked speech pretraining remains;
  speaker-verification role coexists with the DDA labelling role.
- **Khattab & Zaharia (2020) ColBERT; Santhanam et al. (2022) ColBERTv2**
  — late-interaction geometry remains; retrieval semantics coexists with
  labelling semantics.
- **Formal et al. (2021) SPLADE** — sparse lexical expansion remains;
  sparse labels are simply another signal in the DDA set.
- **Wang et al. (2021) KEPLER; Bordes et al. (2013) TransE** —
  knowledge-embedding theory remains; entity embeddings are additional
  DDA signals.
- **Malkov & Yashunin (2018) HNSW** — ANN substrate enables fast
  N-embedder projection but the graph index is used at training-data
  preparation time, not only at retrieval time.
- **Cormack et al. (2009) RRF** — fusion remains in retrieval; the
  DDA $\binom{N}{2}$ count is the *signal-theoretic dual* of RRF's
  *ranking-quality* lift.

**Compatibility code.** **EXT (ROLE-FLIP)** — each embedder's training
theory is preserved; its downstream role is reassigned.

**What the existing frameworks do NOT claim.** Cluster D's fusion
literature (RRF) measures ranking quality; it does not count supervisory
channels. Our $\binom{N}{2}$ pairwise-interaction count is the first
explicit measurement of *signal cardinality* from an embedder panel.

**Our evidence.** §4.1 Context Graph (13 embedders, 15 HNSW indexes per
`CLAUDE.md`, RRF fusion); §4.2 ClipCannon (7 embedders); §3.4 count.

**Hedges carried.** V14 (approximately independent). V15 (write
$\binom{N}{2}$ explicitly in count expressions). V22 (no claim that
pairwise signals are information-theoretically independent; that is §10
future work).

**Manuscript home.** §3.4 (count); §4 Systems (instantiations); §5 TCT
(embedder substrate).

**Adversarial-reviewer target.** A Cluster-D reviewer will point out
that RRF, late-interaction, and sparse+dense hybrids already combine
embedders. Our response: those methods *fuse rankings*; they do not
*count signals*. The substrate is the same (multiple embedders); the
measurement direction is different.

---

## Contribution 4 — TCT is a geometric-alignment frame, framework-breaking for scalar-reward alignment

**Contribution-type.** BRK relative to Cluster I; EXT (POS-EXT) relative
to Cluster D contrastive-learning theory.

**Statement.**
> Teleological Constellation Training (TCT) replaces the scalar-reward
> primitive of RLHF / DPO / CAI with a frozen geometric primitive: a
> per-modality cosine to a frozen centroid. TCT uses no preference pairs,
> no reward model, no implicit reward, and no scalar preference direction.
> TCT is also **not** contrastive learning: no negatives are sampled; the
> only gradient signal is positive distance to a frozen centroid. TCT is
> also **not** metric learning: the metric is fixed by the frozen
> embedders, not learned.

**Existing frameworks engaged.**
- **Cluster I (scalar-reward alignment)**:
  - **Ouyang et al. (2022) InstructGPT** — RLHF: reward model + PPO. TCT's
    target is a frozen centroid, not a learned reward.
  - **Rafailov et al. (2023) DPO** — closed-form preference optimisation.
    TCT's loss is a distance, not a log-ratio of policies; no preference
    pairs.
  - **Bai et al. (2022) Constitutional AI** — AI-feedback preferences.
    TCT has no self-critique and no natural-language rules; the "rule" is
    a cosine threshold.
- **Cluster D (contrastive / metric / prototypical)**:
  - **Zhai et al. (2023) SigLIP** — sigmoid contrastive loss. TCT inherits
    cosine geometry but uses **positive-only** training; no negatives
    sampled.
  - **Snell et al. (2017) Prototypical Networks (cited by glossary #3 NOT
    clause; not in master pool as a distinct citation — flagged)**. TCT
    generates, not classifies; centroids (not prototypes per V1) replace
    prototypes.
  - **Metric-learning family** (siamese / triplet) — TCT's embedders are
    frozen, so the metric is not trained; this is the sharpest
    theoretical distinction within Cluster D.

**Compatibility code.** **BRK** across Cluster I. **EXT (POS-EXT)**
across contrastive and metric-learning theory in Cluster D: extension via
feature *removal* (no negatives; no metric training).

**Why BRK, not EXT, against Cluster I.** An EXT claim would require that
scalar-reward theory subsume TCT under some parameterisation (e.g., "TCT
is RLHF with a degenerate reward that is cosine-to-centroid"). This is
**false in the theoretical sense**: RLHF's implicit-reward argument
(Rafailov 2023) presupposes preferences induced by the reward function;
TCT's cosine-to-centroid *is not derived from a preference relation*.
There is no preference dataset, no Bradley-Terry model, no KL-regulariser
to a reference policy. The theoretical object is different.

**What the existing frameworks do NOT claim.**
- Cluster I: no paper proposes a runtime per-output acceptance predicate;
  no paper uses a frozen geometric target; no paper handles multi-modality
  conjunction without collapsing to a scalar.
- Cluster D: no paper uses a frozen metric for generation alignment; no
  paper runs positive-only training against a frozen target.

**Our evidence.** §5 TCT formal specification (three-phase protocol);
§5.3 constellation guard; §6.3 Case 3 T1 (WavLM SECS 0.961 with guard);
§2.6 / §5.4 contrast table against RLHF / DPO / CAI.

**Hedges carried.** V13 "alignment (disambiguated)" — first use in §1
and §2.6 must distinguish geometric-alignment from scalar-reward alignment.
V9 (first-use spelling). The BRK claim against Cluster I is a theoretical-
novelty claim, not a performance claim; no head-to-head RLHF-vs-TCT
empirical contest is reported.

**Manuscript home.** §1 Introduction (first mention with V13
disambiguation); §2.6 Related Work (alignment sub-line); §5 TCT
(formal protocol); §5.4 or §2.6 contrast table; §7 Discussion (positioning).

**Adversarial-reviewer target.** A reviewer from Cluster I will claim
TCT is a special case of RLHF with a hand-crafted reward. Our response,
tight and defensible:
1. Our "reward" is **not scalar** — it is a per-modality conjunction
   $\forall m: s_m \geq \tau_m$ (Contribution 6, Identity-locked
   generation).
2. Our "reward" is **not learned** — the embedder is frozen; the centroid
   is frozen; the threshold is calibrated not trained.
3. Our method has **no preference pairs** — no $\pi(y_1 \succ y_2 | x)$
   model, no Bradley-Terry, no AI-feedback, no human labels.
4. Our method has **a per-output inference-time gate** — RLHF produces a
   distribution; TCT produces an accepted-or-rejected output.

These four conditions together place TCT outside Cluster I's theoretical
scope; the "special case of RLHF" objection fails on each of the four
counts.

---

## Contribution 5 — Constellation guard is a runtime-verification import into ML

**Contribution-type.** VERIF-IMPORT (from formal methods / runtime verification).

**Statement.**
> The constellation guard is the runtime validator that, for every
> generated output, re-embeds the output through the frozen embedders,
> computes per-modality cosines to the centroids, and accepts iff every
> per-modality cosine meets its threshold. As a construct, the guard is
> novel in ML (Clusters D / E / F / I are silent on per-output cosine
> gates at inference) and imports a pattern from **runtime verification
> in formal methods** — a per-execution monitor that decides accept /
> reject against a specification.

**Existing frameworks engaged.**
- **Cluster D (embedders / retrieval)**: the guard reuses the cosine
  geometry SigLIP / WavLM / ColBERT produce, but gates generation rather
  than ranks artefacts.
- **Cluster E (zero-shot TTS)**: the guard replaces post-hoc SIM-o
  evaluation with per-output acceptance; SIM-o becomes a *sample* from
  the guard's cosine trace, not a batch-mean.
- **Cluster F (video generation)**: the guard replaces post-hoc CSIM
  with per-output multi-modal conjunction; LSE-D / FVD become secondary
  evaluators alongside the guard's acceptance.
- **Cluster I (alignment)**: the guard replaces KL-regulariser-shaped
  distribution with per-output gate; the regime is strictly sampled-then-
  verified rather than sampled-to-maximise-reward.

**Adjacent discipline (not in master pool; flagged).** The runtime-
verification literature in formal methods (e.g., monitoring of temporal-
logic properties; specification-language-based acceptance) is the
closest external precedent. The master reference pool does not include
a runtime-verification citation; **agent-10 flags this as a potential
scope expansion for agent-11** if PRISMA finds that an ML audience
expects a runtime-verification anchor. Agent-10 does **not** introduce
a citation; the absence is noted for the systematic reviewer.

**Compatibility code.** **BRK** across D / E / F / I simultaneously (the
guard has no precedent in any of the four). This is the **single most
theoretically novel move** in the manuscript.

**What the existing frameworks do NOT claim.**
- No embedder paper (D) proposes a per-output acceptance predicate.
- No TTS paper (E) runs a runtime cosine-gate resampling loop.
- No video paper (F) applies a multi-modal cosine conjunction per output.
- No alignment paper (I) has any per-output acceptance predicate at all.

**Our evidence.** §5.3 / `tct-definition.md` §3.2 Phase 3 (formal
protocol); §6.3 Case 3 T1 (WavLM-scored best-of-12 at T=0.3 — concrete
instantiation); §6.2 Case 2 T3 (projected multi-modal guard). Code
artefact: ConstellationGuard in Phoenix / Context Graph (glossary #12).

**Hedges carried.** The "runtime verification" framing is an *import
analogy*, not a formal-methods claim; we do not claim the guard
discharges formal-verification obligations in the sense of temporal
logic (that would require a different proof apparatus). The guard is a
geometric runtime monitor.

**Manuscript home.** §5.3 (formal protocol); §6.3 (Case 3 T1
instantiation); §6.2 (Case 2 T3 projection); §7 Discussion (positioning
as runtime-verification import).

**Adversarial-reviewer target.** A reviewer will ask: "Is this just a
rejection sampler with an embedder as scorer?". Response: yes at the
mechanical level; **no at the theoretical level** — the structural
novelty is the use of a **multi-modal frozen-centroid conjunction** as
the specification, and the importation of the *specification-then-
monitor* pattern from runtime verification into generative ML. No cited
rejection-sampling paper in ML proposes multi-modal conjunctive
acceptance against frozen centroids.

---

## Contribution 6 — Identity-locked generation as a per-output predicate

**Contribution-type.** PRED.

**Statement.**
> Identity-locked generation is the property (not the method) that every
> returned output satisfies the per-output predicate $\forall m \in
> \mathcal{M}: \cos(\phi_m(\hat{y}), c_m) \geq \tau_m$. This predicate is
> distinct from: (a) personalisation (soft distributional preference); (b)
> style transfer (style without identity claim); (c) subject-specific
> fine-tuning alone (no per-output validator); (d) LoRA-personalised
> generation (same: no validator); (e) zero-shot TTS (single-prompt
> conditioning, no centroid, no gate).

**Existing frameworks engaged.**
- **Cluster E (zero-shot TTS)**: zero-shot voice cloning is the closest
  related frame. Case 3 is **not zero-shot** per V19 (uses 50-clip
  reference set aggregated into a centroid).
- **Cluster F (video generation)**: EMO / EchoMimicV3 / Hallo3 condition
  on reference images; no per-output predicate.
- **Cluster I (alignment)**: RLHF / DPO produce distributional
  preferences; no per-output predicate.

**Compatibility code.** **BRK / PRED** — the predicate is introduced as a
first-class concept, not derived from any existing framework. The
predicate **is** the content of Contribution 6.

**What the existing frameworks do NOT claim.** No paper in our master
pool defines "identity-locked generation" as a per-output predicate. The
closest adjacent concepts:
- **Personalisation** (preference steering) — distributional, not per-
  output.
- **Identity preservation** (image-editing literature, V3 forbids this
  phrasing) — batch-average, not per-output.
- **Speaker-verification threshold** (EER-based; Desplanques 2020
  ECAPA) — verification threshold, not generation predicate.

**Our evidence.** §5.3 Phase 3 guard (protocol); §6.3 Case 3 T1 (the 10
held-out sentences satisfy the strict predicate with
$\tau_{\mathrm{WavLM-SECS}}$ from the same-session band; mean 0.961,
max 0.975, SITREP `tmp/SITREP_2026-04-16_135543.md`); §6.2 Case 2 T3
(projected).

**Hedges carried.** V3 always "identity-locked", hyphenated. V12 same-
session-band hedge on every first use of 0.95/0.961/0.975. V19 "50-clip
reference set" never "50-clip centroid". V22 T3 projected flag on Cases
1 and 2.

**Manuscript home.** §1 Introduction (first mention with distinction
from personalisation per glossary #4 usage rule); §5.3 Phase 3; §6.2 /
§6.3 case instantiations; §7 Discussion.

**Adversarial-reviewer target.** A reviewer will ask whether a
personalised LoRA that reliably produces subject-like outputs is
identity-locked. Answer: **no**. Per `identity-locked-definition.md`
§4.5: identity-locked generation is a **per-output** predicate; a
personalised LoRA without a runtime validator is identity-locked **on
average**, which is not the definition. The granularity is the content
of the concept.

---

## Contribution 7 — Pragmatic-constructive epistemology

**Contribution-type.** Meta-epistemological; not a framework claim.

**Statement.**
> The manuscript is not purely positivist (TD arguments are load-bearing
> for DDA count and meaning-compression ratio) and not purely
> constructivist (EB numerics on Case 3 are load-bearing for the
> identity-locked predicate). The paradigm is **pragmatic-constructive**:
> construct + measure + verify across three registers (TD, EB, FC).

**Existing frameworks engaged.** This is a methodological-positioning
claim, not an extension or break. It defends against:
- A pure-TD reviewer who will demand empirical ablations for DDA (answer:
  §10 open work; the count is T4).
- A pure-EB reviewer who will demand Case 3 head-to-head against VALL-E 2
  on LibriSpeech (answer: Q14 scope; we cite VALL-E 2 as comparator on a
  different protocol; no like-for-like head-to-head is claimed).
- A critical reviewer who will demand interpretive framing (answer: the
  numerics are not socially co-produced; Case 3 has measurement proofs).

**Compatibility code.** N/A (meta-claim). The paper **does** carry the
V22 tier discipline (T1 / T2 / T3 / T4) throughout to ensure every claim
lives in exactly one register.

**Manuscript home.** §1 Introduction (brief framing); §8 Limitations
(explicit epistemology statement); §10 Future Work (what each register
leaves open).

---

## Contribution summary table (for §2.6 / §5 / §7)

| # | Contribution | Type | Clusters engaged | Compatibility | Novelty depth |
|---|---|---|---|---|---|
| 1 | DDA extends scaling-law data-axis | AXIS-EXT | A (Kaplan/Chinchilla/Villalobos) | EXT | Moderate — axis reinterpretation of an established theory |
| 2 | Meaning compression fourth category | CAT-EXT | C (Delétang/Huang/Li/Valmeekam/Jiang) + adjacent (GPTQ/SparseGPT/TurboQuant) | EXT | Moderate — new category in an established taxonomy |
| 3 | Embedder role-flip (ranker → labeller) | ROLE-FLIP | D (SigLIP/Nomic/WavLM/ColBERT/SPLADE/KEPLER/TransE/HNSW/RRF) | EXT | High — the $\binom{N}{2}$ count is first-time in the cluster |
| 4 | TCT as geometric-alignment frame | BRK + POS-EXT | I (RLHF/DPO/CAI); D (contrastive/metric) | BRK against I; EXT against D | **High — BRK against alignment** |
| 5 | Constellation guard | VERIF-IMPORT | D + E + F + I | BRK across all four | **Highest — novel in four clusters simultaneously** |
| 6 | Identity-locked generation | PRED | E + F + I | BRK / PRED | High — first-time predicate formalisation |
| 7 | Pragmatic-constructive epistemology | Meta | — | — | N/A (positioning) |

---

## Where each contribution lives in the manuscript (writer table)

| Contribution | §1 Intro | §2 Related Work | §3 Framework | §4 Systems | §5 TCT | §6 Cases | §7 Discussion | §8 Limits | §10 Future |
|---|---|---|---|---|---|---|---|---|---|
| 1 DDA axis | first use (V6) | §2.1 scaling laws | §3.1, §3.4 (count) | §4.1, §4.2 | — | — | positioning | V14 hedge | scaling-law modification |
| 2 Meaning compression category | first use | §2.3 compression | §3.1 four-category table; §3.5 | — | — | — | positioning | MI-adjusted MC |
| 3 Role-flip | (implicit) | §2.5 embedders | §3.4 | §4.1, §4.2 | §5 substrate | — | positioning | N-ablation |
| 4 TCT BRK | first use (V13) | §2.6 alignment | — | — | §5 formal; §5.4 contrast table | §6.1, §6.2, §6.3 | §7 BRK | scope of TCT | safety-centroids, non-identity |
| 5 Constellation guard | (via TCT) | §2.6 | — | §4.1 (code artefact) | §5.3 Phase 3 | §6.3 (Case 3 T1); §6.2 (T3) | §7 runtime verification | limits of guard | non-identity targets |
| 6 Identity-locked predicate | first use (glossary #4) | §2.6 | — | — | §5.3 formal predicate | §6.2 (T3); §6.3 (T1) | §7 | §8 scope | detector (Q19) |
| 7 Epistemology | positioning sentence | — | — | — | — | tier tagging | §7 explicit statement | §8 explicit | — |

---

## Confidence statement

**HIGH.** Every contribution is anchored to a specific cluster from the
framework map, traced to specific citations in the master reference pool,
and labelled with an EXT / COMP / BRK / ORTH code + a contribution-type
code. The five "high-novelty" contributions (3, 4, 5, 6) are each
**framework-breaking in at least one cluster** (5 is BRK in four; 4 is
BRK in Cluster I; 6 is BRK across E/F/I; 3 is high-novelty EXT in D).
No contribution overclaims; every hedge from the glossary and
vocabulary-rules files is carried.

**Locked elements (do not re-debate without a blocker).**
- Contribution-type codes (AXIS-EXT / CAT-EXT / ROLE-FLIP / POS-EXT /
  PRED / VERIF-IMPORT).
- "Constellation guard is BRK across 4 clusters" — highest novelty
  claim, to be enforced in §2 G3 novelty-delta table and §5 framing.
- "TCT is BRK relative to RLHF/DPO/CAI" — the sharpest break; defended
  on four structural grounds (non-scalar, non-learned, no preferences,
  per-output gate).

**Open items for agent-11 / writers.**
- Prototypical Networks (Snell et al., 2017) is cited in the glossary
  #3 NOT clause but **not in the master reference pool** — agent-11
  should consider whether PRISMA expansion of Cluster D to include it
  is warranted for §5.4 contrast-table completeness. Agent-10 flags
  this; it is not a blocker.
- Runtime-verification literature is absent from the master pool —
  agent-11 should consider whether the VERIF-IMPORT framing for the
  constellation guard needs a formal-methods anchor.

---

## Changelog

- 2026-04-16 — Initial issue by agent-10-theoretical-framework-analyst.
