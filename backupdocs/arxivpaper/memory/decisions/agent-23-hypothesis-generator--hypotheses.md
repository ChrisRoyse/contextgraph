---
namespace: decisions
agent_id: agent-23-hypothesis-generator
timestamp: 2026-04-17T06:30:00Z
status: active
locked: true
supersedes: none
tags: hypotheses, testable-claims, falsification, evidence-map, dda, tct, meaning-compression, sample-complexity, collapse-silence
depends_on:
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-22-model-architect--master-architecture.md
  - memory/patterns/agent-22-model-architect--subfigures.md
  - memory/patterns/agent-22-model-architect--variable-dictionary.md
  - memory/handoff/agent-22-to-23--architecture-ready.md
downstream_consumers:
  - agent-24-causal-simulator
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
answers_questions: [Q04, Q06, Q07, Q15, Q20]
---

# Hypothesis Ledger — The Claims This Paper Defends

## 0. Purpose and scope

This file issues the **locked hypothesis ledger** for the manuscript.
Every hypothesis is a testable, falsifiable prediction that maps to:

1. A specific **proposition (P1–P5)** or **theorem (T1, T2)** from
   agent-21's theoretical framework.
2. A specific **Figure-1 arrow cluster (A1–A13)** from agent-22's
   interconnect table.
3. A specific **evidence status** (T1-measured / T2-constructive /
   T3-projected / T4-derived) from agent-22's variable dictionary.

The ledger contains **9 locked hypotheses** grouped into **5 families**
plus **2 threshold-calibration sub-hypotheses** (H2c, H2d) that serve
as falsification hooks for T3-projected Cases 1 and 2.

**Composition-not-invention (MP-12).** No hypothesis introduces a new
primitive; each is a testable statement about constructs already
defined by agent-04 and composed by agents 21 and 22.

**Pool-relative scope (R3).** Every "no cited method" clause is
pool-relative to the LOCKED 88-citation master pool (agent-08).

---

## 1. Hypothesis catalogue (at-a-glance)

| H-ID | Family | Target | Evidence tier | Anchor |
|---|---|---|---|---|
| **H1** | DDA signals-per-input | $\lvert D' \rvert / n = N + \binom{N}{2}$ | T4 derived + T2 instantiated | P1, A1–A3 |
| H1a | DDA practical yield | Context Graph $N=13 \Rightarrow 91$ signals/input | T2 constructive | P1, A3 |
| H1b | DDA hedge | Count is a **structural upper bound** (V11) | T4 + RO-8 future | P1, V11 |
| **H2** | TCT geometric alignment | Guard accept-rate bounded by per-modality threshold conjunction | T4 derived (+ T1 partial) | P3, T2, A10–A12 |
| H2a | TCT Case 3 voice | Mean WavLM SECS 0.961, 9/10 $\geq$ 0.95 consistent with H2 | **T1 MEASURED** | P3, T2, A10 |
| H2b | TCT multi-modal strictness | Multi-modal conjunction admits stricter identity predicates than single-modality check | T4 derived | P3, A10 |
| H2c | TCT Case 1 threshold | Text-style $\tau_m \in [0.80, 0.92]$ prior prediction | **T3 PROJECTED** | P3, T2 |
| H2d | TCT Case 2 threshold | Visual + speaker $\tau_m$ matrix (see §4.H2d) | **T3 PROJECTED** | P3, T2 |
| **H3** | Sample-complexity collapse | $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$, independent of $\lvert D \rvert$ | T4 derived + T1 anchor | P5, T1, A6+A7+A13 |
| H3a | Sample-complexity Case 3 | 50-clip centroid $\to$ $\geq 0.95$ SECS on unseen text | **T1 MEASURED** | P5, T1 |
| H3b | Sample-complexity Case 2 | 2,362 clips from 16 min source sufficient post-training | **T3 PROJECTED** | P5, T1 |
| **H4** | Collapse-silence | DDA is structurally not subject to Shumailov-2024 recursion | T4 scope argument | P2, A5 (absence) |
| **H5** | Meaning compression as category | Unit (signals / raw-data-unit) is dimensionally distinct from bit / weight / activation compression | T4 derived (unit analysis) | P1 ratio view, A4 |

---

## 2. Family 1 — DDA signals-per-input multiplication

### H1 (primary) — Signal-count identity

**Statement.** Under Axioms 1 (grounded measurement) and 2 (approximate
orthogonality), running $N$ independent frozen embedders $\Phi =
\{\phi_1, \ldots, \phi_N\}$ over a fixed corpus $D$ of $n$ raw inputs
yields

$$
\lvert D' \rvert \;=\; n \cdot \Bigl(N + \binom{N}{2}\Bigr) \;=\; n \cdot \frac{N(N+1)}{2}
$$

**structured supervisory signals**, where $D'$ is the derived dataset
of agent-04 DDA-definition §1.2.2.

**Anchors.** P1 (signal-count identity); arrows A1, A2, A3 of Figure 1.

**Evidence available.**
- T4 derivation (agent-21 P1 proof sketch).
- T2 constructive instantiations in mission brief "Key Architecture
  Facts" (Context Graph at $N=13$, ClipCannon at $N=7$).
- agent-10 cluster-A scaling-law contrast (axis-extension novelty).

**Evidence pending.**
- Formal mutual-information measurement of $I(\phi_j(X); \phi_k(X))$
  between embedder pairs (RO-8; agent-19).

**Falsification condition.**
- Observation that $\lvert D' \rvert < n \cdot N$ (i.e., the $n \cdot
  N$ per-embedder count itself fails) would falsify H1 directly
  (violates Axiom 1 determinism).
- Observation that across a panel $\Phi$, more than $\binom{N}{2}$
  pairwise features collapse to linear reparameterisations of single
  $\phi_m(x)$ would reduce the count to a trivially loose upper
  bound and falsify H1 as a tight count (pushing H1 toward H1b).

**Confidence.** 95 %. The identity is a **structural upper bound**
(agent-21 P1 confidence).

**Manuscript anchor.** §3.4 (worked at $N=13$ and $N=7$); §3.1
(meaning-compression ratio view).

### H1a (secondary) — Context Graph practical yield (91 signals/input)

**Statement.** For the Context Graph production configuration
$N = 13$ (E1–E13 semantic / temporal / causal / sparse / code /
graph / HDC / paraphrase / entity / late-interaction / SPLADE
embedders), the per-input signal yield is

$$
N + \binom{N}{2} \;=\; 13 + 78 \;=\; 91 \text{ signals / input}.
$$

**Anchors.** P1 (instantiation); A3; Figure 2 Level 3.

**Evidence available.**
- T2 constructive from system architecture (13 HNSW indexes + sparse
  / MaxSim inverted indexes; mission brief "13 embedders" fact).
- T4 derivation: $91 = 13 + 78$ by P1.
- Figure 2 ASCII tree (agent-22 subfigures §1).

**Evidence pending.**
- Empirical audit that each of the 13 embedders produces a
  non-constant per-input signal on a representative corpus (RO-8
  follow-up measurement).

**Falsification condition.**
- If two or more of E1–E13 are discovered to collapse to linear
  reparameterisations of one another under the production panel
  configuration, the effective count drops below 91; headline number
  must be stated as "up to 91" (V11 hedge).

**Confidence.** 92 %. The count is a pool-relative (R3) headline
number for the Context Graph DDA instance; V11 hedge carried at
every use.

**Manuscript anchor.** §4.1 Context Graph systems section; Fig. 2.

### H1b (secondary) — Structural-upper-bound hedge

**Statement.** The count $n \cdot (N + \binom{N}{2})$ in H1 is a
**structural upper bound**; formal mutual-information measurement
may reveal that the effective information carried by $D'$ is
strictly less due to partial embedder redundancy.

**Anchors.** P1 hedge clause; V11 (approximate independence); RO-8
(future work).

**Evidence available.**
- agent-21 P1 confidence note ("structural upper bound, not
  information-theoretic lower bound").
- agent-01 V11 hedge discipline.

**Evidence pending.**
- RO-8 (future work, agent-19): formal measurement of
  $I(\phi_j(X); \phi_k(X))$ and of marginal $H(\phi_j(X))$ on a
  representative corpus to quantify diminishing returns.

**Falsification condition.**
- If mutual-information measurement reveals **zero** redundancy
  across $\Phi$ (i.e., $I(\phi_j(X); \phi_k(X)) = 0$ for all pairs),
  the upper bound is tight and H1b is vacuously true (not
  falsified; just uninformative).
- If redundancy exceeds a threshold such that effective-signal
  count drops below $n \cdot N$ (worse than single-embedder
  baseline), H1b is too weak and the bound needs tightening —
  this would also falsify H1 as a useful count.

**Confidence.** 90 %. V11 carries explicitly.

**Manuscript anchor.** §3.4 V11 clause; §8 Limitations (RO-8
pointer); §10 Future Work.

---

## 3. Family 2 — TCT geometric alignment

### H2 (primary) — Constellation guard accept-rate bound

**Statement.** Under Axiom 1 (frozen $\phi_m$), Phase 3 of TCT
accepts candidate $\hat y$ at a rate bounded by the **conjunction of
per-modality cosine thresholds**:

$$
\Pr[G_\tau(\hat y; C_P) = 1] \;=\; \Pr\!\left[\,\forall m \in \mathcal{M} : \cos(\phi_m(\hat y), c_m) \geq \tau_m\,\right].
$$

Per-output evaluation is $O(M \cdot d_{\max})$ and requires no
auxiliary learned model (Theorem 2).

**Anchors.** P3 (geometric verifiability), Theorem 2; arrows A10,
A11, A12 of Figure 1.

**Evidence available.**
- T4 derivation (agent-21 P3 and Theorem 2 proofs).
- T1 partial: Case 3 voice-modality (see H2a).
- ISO-01 (RV) + ISO-07 (SPC) tradition parallels (agent-20).

**Evidence pending.**
- Full $M > 1$ case-study instantiation (Cases 1, 2).
- False-positive / false-negative curve vs. $\tau$ (Q2 future work;
  agent-19).

**Falsification condition.**
- Observation that $G_\tau$ produces **non-deterministic** accept /
  reject decisions across runs on the same $(\hat y, C_P, \tau)$
  input (would violate Axiom 1 frozen-measurement).
- Observation that an auxiliary learned model is required to
  achieve the claimed per-output decision (would falsify the "no
  auxiliary learned model" clause of Theorem 2).

**Confidence.** 92 %. The "strictly stronger" clause of P3 rests on
Axiom 2 (each modality carries non-redundant identity information);
under perfectly-correlated modalities, $G_\tau$ collapses to the
minimum over redundant dimensions and H2 degrades to a
single-dimension predicate.

**Manuscript anchor.** §5.1 (scalar-vs-geometric contrast); §5.3
(guard definition); §5.4 (per-output predicate).

### H2a (secondary) — Case 3 voice-modality evidence

**Statement.** Case 3 (T1-measured: single subject, English, voice
modality only, 50 reference clips) produces mean WavLM-base-plus-sv
SECS = 0.961 (max 0.975), with **9 of 10 held-out sentences at
SECS $\geq 0.95$**, consistent with H2 at $M = 1$,
$\tau_{\text{WavLM-SECS}} = 0.95$, $K = 12$, $T = 0.3$.

**Anchors.** P3, Theorem 2; A10–A12; Figure 4 Phase 3 legend.

**Evidence available.**
- **T1 MEASURED** (SITREP 2026-04-16; white paper).
- Architecturally-independent scorer (WavLM-base-plus-sv is **not**
  in the Context Graph E1–E13 panel — agent-22 master-architecture
  §5 note).
- DNSMOS P.808 = 3.93 (parity with real 3.93); UTMOS = 3.012 (vs
  real 2.997) — naturalness parity measurements.

**Evidence pending.**
- Multi-speaker / multi-language extension (single-speaker,
  English-only scope flagged by agent-14 Bias-05 / Bias-06).
- Single-encoder-pair hedge (agent-17 Theme D) — evaluation scorer
  diversity.

**Falsification condition.**
- If reproduction attempt yields mean SECS $< 0.85$ under the same
  protocol (K=12, T=0.3, $\tau = 0.95$, 50-clip centroid, WavLM-
  base-plus-sv scorer), H2a is falsified.
- If the 9/10 $\geq 0.95$ rate drops below 5/10 on a comparable
  held-out set of 10 sentences, H2a is falsified.

**Confidence.** 95 %. **T1 MEASURED** — the single anchor for the
entire identity-lock family in this manuscript.

**Manuscript anchor.** §6.3 Case 3 (measured instantiation).

### H2b (secondary) — Multi-modal conjunction strictness

**Statement.** The conjunctive multi-modal guard $G_\tau = \mathbb{1}[
\forall m : s_m \geq \tau_m]$ admits **strictly stricter** identity
predicates than any single-modality check at equal per-modality
thresholds, because failing any one modality rejects.

Formally: for $M > 1$ and $\tau_m < 1$,

$$
\Pr[G_\tau = 1] \;\leq\; \min_m \Pr[s_m \geq \tau_m],
$$

with equality iff all per-modality events are comonotonic.

**Anchors.** P3 (geometric-over-scalar), ISO-03 (biological multi-
modal recognition — metaphor only).

**Evidence available.**
- T4 derivation (elementary probability: conjunction of events
  bounded by the minimum).
- MP-01 (agent-16): single-encoder similarity check is a
  pool-external convention weaker than multi-modal conjunction.

**Evidence pending.**
- Empirical measurement of $\Pr[G_\tau = 1]$ vs. $\min_m \Pr[s_m \geq \tau_m]$
  on Cases 1, 2 where $M > 1$.

**Falsification condition.**
- If on Case 2 (visual + speaker, $M \geq 2$) the multi-modal
  conjunctive accept-rate equals or exceeds any single-modality
  accept-rate at matched thresholds, H2b is falsified (would mean
  the modalities encode **no** discriminative information over one
  another — violating Axiom 2).

**Confidence.** 94 %. The inequality is elementary; the **sharp**
claim ("strictly stronger") rests on modalities carrying
non-redundant identity information (Axiom 2).

**Manuscript anchor.** §5.4 per-output predicate; §7 Discussion
(identity-lock strictness).

### H2c (threshold-calibration — Case 1 text-style)

**Statement.** Before Case 1 measurement, we predict that the
same-session-band cosine threshold for text-style modality $\tau_{
\text{text-style}}$ lies in $[0.80, 0.92]$ under Context Graph's
E1 (e5-large-v2) for a target author at $n_m \sim 50\text{–}100$
reference passages.

**Anchors.** P3, Theorem 2; A10; Axiom 3 ($n_m \in [10, 10^2]$).

**Evidence available.**
- T3 projection from agent-04 TCT definition §3.2 Phase 3 threshold
  discussion.
- Analogy from Case 3 ($\tau = 0.95$ for voice) adjusted for
  text-style modality's lower within-author concentration.
- Existing Context Graph E1 gate thresholds (CAUSAL_THRESHOLD=0.04,
  NON_CAUSAL_THRESHOLD=0.008 — but these are for E5 asymmetric
  causal direction, not E1 text-style identity).

**Evidence pending.**
- **Case 1 T1 measurement** — Shakespeare LoRA style transfer
  pipeline (scripts/shakespeare_full_pipeline.py; recent commits).
- Same-session band floor measurement on 50–100 authentic
  Shakespeare-style passages embedded with E1.

**Falsification condition.**
- If Case 1 T1 measurement yields same-session-band floor $< 0.75$
  or $> 0.95$ for text-style modality under E1, H2c is falsified.
- If a cross-encoder scorer (architecturally-independent of E1)
  disagrees with E1's band floor by more than 0.10 absolute
  cosine, H2c needs reframing to acknowledge scorer-pair sensitivity
  (agent-17 Theme D hedge).

**Confidence.** 75 %. T3-PROJECTED; the prior is the
"$\tau \in [0.80, 0.95]$" band agent-04 suggests and the Case 3
data point at 0.95.

**Manuscript anchor.** §6.1 Case 1 (projected); §10 Future Work.

### H2d (threshold-calibration — Case 2 visual + speaker)

**Statement.** Before Case 2 measurement, we predict the following
same-session-band cosine threshold matrix for ClipCannon modalities
at $n_m = 2{,}362$:

| Modality | Predicted $\tau_m$ | Encoder |
|---|---|---|
| $m_4$ speaker audio | $[0.90, 0.97]$ | WavLM-base-plus-sv |
| $m_1$ global visual | $[0.82, 0.92]$ | CLIP ViT-L/14 or SigLIP |
| $m_2$ local visual | $[0.75, 0.88]$ | Patch / crop encoder |
| $m_3$ motion | $[0.70, 0.85]$ | Optical-flow encoder |
| $m_6$ text caption | $[0.80, 0.92]$ | E1-class text encoder |

**Anchors.** P3, Theorem 2; A10; Axiom 3.

**Evidence available.**
- T3 projection based on encoder-specific within-identity
  concentration (speaker audio tightest; motion loosest).
- Case 3 speaker-audio anchor (0.95 threshold at $n_m = 50$).
- ClipCannon 7-modality panel specification (agent-22 Figure 3).

**Evidence pending.**
- **Case 2 T1 measurement** — EchoMimicV3 + LoRA + flow-matching
  loss + $\Sigma_m \lambda_m (1 - \cos)$ term; constellation-
  conditioned fine-tuning over 2,362 clips.
- Held-out identity-lock evaluation on unseen visual scenes /
  speech prompts.

**Falsification condition.**
- If measured per-modality same-session band floors fall outside
  the predicted intervals by more than 0.05 on **any two** of the
  five modalities, H2d is falsified and the band-prediction model
  needs reframing.
- If $G_\tau$ across the full conjunctive $\mathcal{M}_2$ rejects
  more than 95 % of candidates at $K = 12$ (budget exhaustion),
  the threshold matrix is too strict (relaxed best-of-K variant
  must be used; Case 3 practice).

**Confidence.** 65 %. T3-PROJECTED; highest uncertainty among the
hypotheses because (a) multi-modal case has no T1 anchor, (b)
ClipCannon modalities span the widest encoder-diversity range.

**Manuscript anchor.** §6.2 Case 2 (projected); §10 Future Work.

---

## 4. Family 3 — Sample-complexity collapse

### H3 (primary) — Sample-complexity bound $O(M \cdot k)$

**Statement.** Under Axioms 1, 2, 3 and TCT Phases 1–3, the subject-
specific sample complexity for identity-locked generation of subject
$P$ satisfies

$$
\mathcal{T}_{\mathrm{TCT}}(P) \;=\; O\!\Bigl(\sum_{m=1}^M n_m\Bigr) \;=\; O(M \cdot k), \qquad k = \max_m n_m \in [10, 10^2],
$$

**independent** of $\lvert D \rvert$ (the general corpus size) and
**independent** of any subject-$P$-specific fine-tuning data beyond
the reference sets $\{R_m\}$. This is Theorem 1 (sample-complexity
collapse).

**Anchors.** P5 (sample-complexity collapse), Theorem 1; arrows
A6, A7, A9, A13 of Figure 1.

**Evidence available.**
- T4 derivation (agent-21 Theorem 1 proof sketch).
- T1 anchor (Case 3; see H3a).
- T3 projection (Case 2; see H3b).
- MP-05 (agent-16): cluster-E / F conventional pipeline uses
  thousands of subject-specific examples.

**Evidence pending.**
- Sample-complexity curve per modality (RO-10; agent-19).
- Multi-modality $M > 1$ T1 measurement.

**Falsification condition.**
- If under a like-for-like Case-3 reproduction, successful
  identity-locked generation requires $n_m > 10^3$ subject-
  specific samples (exceeding Axiom 3's $[10, 10^2]$ bound), H3 is
  falsified.
- If sample complexity is shown to grow with $\lvert D \rvert$
  (i.e., a bigger general corpus makes identity-lock require
  **more** subject-specific samples), H3 is falsified.

**Confidence.** 85 %. The theorem is **constructive** (any pipeline
with exactly the TCT Phase 1/2/3 structure satisfies the bound by
construction); the non-trivial content is Axiom 3's claim that $k$
can be taken $O(10^2)$ without overfitting.

**Manuscript anchor.** §1 headline; §3.6 sample-complexity framing;
§5.4; §6.3; §7; §9.

### H3a (secondary) — Case 3 measured operating point

**Statement.** Case 3's Phase 1 centroid constructed from 50 audio
reference clips ($n_m = 50$) produces held-out mean SECS = 0.961
(max 0.975) with 9/10 $\geq 0.95$ on **unseen English text**
generated via Qwen3-TTS best-of-$K$ at $K = 12, T = 0.3$, consistent
with H3 at $M = 1, k = 50$.

**Anchors.** P5, Theorem 1, Axiom 3; A7 + A13; Figure 4 Phase 3
legend.

**Evidence available.**
- **T1 MEASURED** (SITREP 2026-04-16; white paper).
- 50 clips from a single 16-minute voice capture (also Case 2's
  source).
- Architecturally-independent scorer (WavLM-base-plus-sv) vs. the
  identity construction (50-clip centroid).

**Evidence pending.**
- Scorer-ablation (a second WavLM-variant, ECAPA-TDNN, Resemblyzer
  for scorer diversity; agent-17 Theme D).

**Falsification condition.**
- If reducing $n_m$ to 10 still yields mean SECS $\geq 0.95$ on the
  same held-out set, H3a is **strengthened** — not falsified, but
  the $k = 50$ value becomes non-tight and RO-10 (sample-complexity
  curve) is prioritised.
- If increasing $n_m$ to 500 is required to reach the same 0.961
  mean (i.e., 50 clips are insufficient on replication), H3a is
  falsified under the SECS / WavLM-base-plus-sv scoring protocol.

**Confidence.** 95 %. **T1 MEASURED**.

**Manuscript anchor.** §6.3 Case 3 (measured instantiation).

### H3b (secondary) — Case 2 projected operating point

**Statement.** Case 2's Phase 1 constellation constructed from
$n_m = 2{,}362$ clips per modality (ClipCannon-curated from 16
minutes of source video) is sufficient for identity-locked Case-2
generation post-training, with per-modality SECS on held-out visual
scenes / speech prompts projected to $\geq$ same-session band
minimum of H2d per modality.

**Anchors.** P5, Theorem 1, Axiom 3; A7 + A13; Figure 3.

**Evidence available.**
- T3 projection from mission brief "2,362 clips from 16 minutes".
- Case 3 audio-modality anchor from the same 16-minute source.
- ClipCannon 7-modality panel (agent-22 Figure 3).

**Evidence pending.**
- **Case 2 T1 measurement** — EchoMimicV3 + LoRA + TCT Phase 2 +
  Phase 3 full run.

**Falsification condition.**
- If Case 2 training fails to converge (loss plateau above
  acceptable threshold) with 2,362 clips per modality, H3b is
  falsified; RO-10 sample-complexity curve must be consulted to
  re-estimate $n_m$.
- If post-training identity-lock rate is $< 50 \%$ on a held-out
  scene / prompt set, H3b is falsified.

**Confidence.** 70 %. **T3 PROJECTED**; projected specifically
because multi-modal case has no T1 anchor and ClipCannon DAG
downstream performance has not been run end-to-end.

**Manuscript anchor.** §6.2 Case 2 (projected); §8 Limitations; §10
Future Work.

---

## 5. Family 4 — Model-collapse exemption

### H4 (primary) — DDA is collapse-silent by construction

**Statement.** DDA pipelines are **structurally not subject to the
model-collapse regime** of Shumailov et al. (2024), Dohmatob et al.
(2024), and Alemohammad et al. (2023), because no recursion of
generator outputs exists in the pipeline: by Axiom 1 each
$\phi_m(x)$ is a frozen measurement of a real input $x \in D$, and
$D' = \{(\phi_m(x), \rho_{jk})\}$ is a **decomposition** of $D$,
**not synthesis** of new inputs.

Formally: the collapse theorems require a recursion
$D_{t+1} \ni \tilde x \sim p_{\theta_t}$ where $p_{\theta_t}$ is a
generator trained on $D_t$. DDA's signals depend only on $x$ and
frozen $\phi_m$, not on any $p_{\theta_t}$. The recursion
precondition **fails**; the theorems **do not apply**.

**Anchors.** P2 (collapse silence); arrow A5 "absence" of Figure 1.

**Evidence available.**
- T4 scope argument (agent-21 P2 proof sketch).
- Axiom 1 grounded-measurement guarantee.
- agent-10 cluster-C orthogonal-path (ORTH-B).

**Evidence pending.**
- Empirical demonstration on a toy setup: train a generator in two
  regimes (A) recursive self-distillation vs. (B) DDA-enriched
  supervision over a frozen corpus, and show collapse dynamics
  differ as P2 predicts.

**Falsification condition.**
- If a DDA-enriched training pipeline (where generator outputs are
  **not** fed back into $D$) is shown to produce the distributional
  drift characteristic of Shumailov-2024 collapse, H4 is falsified
  **only if** the drift cannot be attributed to other causes (e.g.,
  dropout, data imbalance). The falsification condition must
  isolate generator-in-loop as the causal mechanism; mere drift is
  not sufficient.
- If the DDA pipeline is shown to secretly contain a recursion
  (e.g., the embedder is fine-tuned on generator outputs during
  labelling), H4 is falsified at the construction level; V18
  "frozen" discipline prevents this by design.

**Confidence.** 90 %. The claim is **categorical scope**, not an
immunity theorem (agent-21 P2 confidence note). We claim the
preconditions of collapse are unsatisfied; we do **not** claim a
new independence-from-collapse result.

**Manuscript anchor.** §3.3 DDA scope argument; §1 three-paths
framing; §2.2 related work on synthetic-data collapse.

---

## 6. Family 5 — Meaning compression as a distinct category

### H5 (primary) — Dimensional distinctness of meaning-compression unit

**Statement.** Meaning compression has a **unit** — signals per
unit-of-raw-data — that is **dimensionally distinct** from the units
of the three classical compression categories:

| Category | Unit pair | Tradition-family |
|---|---|---|
| Bit-compression | bits / bits | Shannon |
| Weight-compression | accuracy / parameters | GPTQ, SparseGPT, distillation |
| Activation-compression | memory / output-quality | KV-cache quant., TurboQuant |
| **Meaning-compression** | **structured-signals / raw-data-unit** | **DDA (this paper)** |

The four unit pairs are dimensionally non-interconvertible; the four
categories trade along different axes. Therefore meaning compression
is **not a rescaling** of any of the three classical categories but
a **fourth taxonomic entry**.

**Anchors.** P1 (ratio view); arrow A4; §5.2 of agent-21 framework.

**Evidence available.**
- T4 unit-analytic derivation (agent-04 meaning-compression-
  definition §2.3; agent-21 §5.2).
- agent-10 CAT-EXT novelty code of cluster-C compression theory.
- ISO-04 (Kolmogorov / MDL / rate-distortion) tradition parallel
  (agent-20).

**Evidence pending.**
- Quantitative re-fit of Kaplan / Chinchilla exponents $\alpha,
  \beta$ under DDA-enriched supervision (RO-1 F1b; agent-19).
- Engagement with Huang et al. (2024) within-model compression /
  capability correlation ($r \approx 0.95$) to show cross-axis
  complementarity (Tension 2 resolved by agent-17).

**Falsification condition.**
- If meaning-compression's unit (structured-signals / raw-data-unit)
  can be shown to be a **linear rescaling** of any of the three
  classical unit pairs (e.g., signals = bits × constant), H5 is
  falsified.
- If the $\binom{N}{2}$ pairwise term is shown to contribute
  **zero** non-redundant information on realistic corpora (so that
  effective signal count reduces to $n \cdot N$), H5 retains its
  categorical distinctness but loses its quantitative headline;
  H1b hedge applies.

**Confidence.** 88 %. The unit-analytic argument is airtight
(the dimensions do not reduce by any known transformation); the
**practical usefulness** of the fourth category depends on the
Kaplan / Chinchilla re-fit (RO-1).

**Manuscript anchor.** §3.1 meaning-compression ratio view; §5.2
relation-to-existing-frameworks; §7 Discussion (fourth category
synthesis).

---

## 7. Hypothesis-to-evidence summary

| H-ID | Evidence available | Evidence pending | Confidence | Tier |
|---|---|---|---|---|
| H1 | T4 derivation + T2 instantiation (N=13, N=7) | RO-8 formal MI measurement | 95 % | Fully supported |
| H1a | T2 constructive + T4 (91 = 13+78) | RO-8 per-embedder MI audit | 92 % | Fully supported |
| H1b | V11 hedge discipline | RO-8 empirical redundancy measurement | 90 % | Hedge-only support |
| H2 | T4 derivation + partial T1 (Case 3) | Cases 1, 2 T1; FP/FN curve (Q2) | 92 % | Partially supported |
| H2a | **T1 MEASURED** | Scorer diversity ablation | 95 % | **Fully evidenced** |
| H2b | T4 derivation (elementary) | Case 2 $M>1$ empirical | 94 % | Partially supported |
| H2c | T3 prior | Case 1 T1 measurement | 75 % | **Pending T1** |
| H2d | T3 prior matrix | Case 2 T1 measurement | 65 % | **Pending T1** |
| H3 | T4 + T1 anchor + T3 | Multi-modality M>1 T1; RO-10 curve | 85 % | Partially supported |
| H3a | **T1 MEASURED** | Scorer ablation; $n_m$ ablation | 95 % | **Fully evidenced** |
| H3b | T3 projection | Case 2 T1 measurement | 70 % | **Pending T1** |
| H4 | T4 scope argument | Empirical toy demonstration | 90 % | Argument-supported |
| H5 | T4 unit analysis | RO-1 Kaplan/Chinchilla re-fit | 88 % | Argument-supported |

**Fully evidenced (T1-measured):** H2a, H3a.
**Pending T1 measurement:** H2c (Case 1), H2d (Case 2), H3b (Case 2).
**Argument- or derivation-supported only:** H1, H1a, H1b, H2, H2b,
H3, H4, H5 — these rely on T4 derivation under the axioms + T2 / T3
instantiations; each has a clearly named **evidence pending** item
on RO-1 / RO-8 / RO-10 or on Case 1 / Case 2 T1 measurement.

---

## 8. Falsification precedence (what must hold for the paper to stand)

The paper's empirical spine rests on **H2a and H3a** (both Case 3,
both T1-measured). If these are falsified on replication, the paper
loses its T1 anchor and must be reframed from "measured + projected"
to "projected only", which reduces contribution tier per mission
brief constraint.

The paper's theoretical spine rests on **H1, H3, H4, H5** (derivations
under Axioms 1–3). If any axiom is shown to fail on the instantiations
actually used, the corresponding derivation fails:
- **Axiom 1 failure** (any $\phi_m$ silently updated during training)
  falsifies H1, H3, H4 simultaneously and collapses the manuscript's
  structural argument.
- **Axiom 2 failure** (severe embedder redundancy) weakens H1 toward
  H1b and reduces the headline signal-count claim; H5 retains
  categorical distinctness.
- **Axiom 3 failure** (required $n_m \gg 10^2$ for well-conditioned
  $\phi_m$) falsifies H3a/H3b and weakens H3; the sample-complexity-
  collapse headline is lost.

**Priority order for falsification attempts** (highest first):
1. **H2a** (Case 3 replication under matching protocol) — single
   most important T1 anchor.
2. **H3a** (Case 3 centroid stability at $n_m = 50$) — anchors H3.
3. **Axiom 1 audit** on production Context Graph $\Phi$ — checks the
   "frozen" discipline in practice.
4. **H1a** (empirical per-embedder signal non-constancy on a
   representative corpus) — RO-8.
5. **H4** (toy generator-in-loop vs. DDA-enriched comparison) —
   isolates the collapse-silence mechanism.

---

## 9. What this ledger does NOT claim

- **No hypothesis** claims outperforming RLHF / DPO / CAI on RLHF
  benchmarks. The spine **structurally breaks** with scalar-reward
  alignment (§5.3); it does not compete on the same axis.
- **No hypothesis** claims the constellation guard is a formal
  runtime-verification monitor in the strict logical sense (ISO-01
  is a metaphor-register parallel; agent-20 register flag).
- **No hypothesis** claims global novelty beyond the LOCKED 88-pool
  (MP-16 pool-relative scope; R3 hedge on every "no cited method"
  clause).
- **No hypothesis** claims AGI or imminent-plateau implications
  (mission brief hard constraint; agent-17 Theme A hedge).
- **No hypothesis** is stated **without** a falsification condition.

---

## 10. Hedges carried (inherited verbatim from upstream agents)

- **V11** ("approximately independent; strict formal orthogonality
  is an open question") carried on every H-statement that invokes
  Axiom 2 or the $\binom{N}{2}$ term: H1, H1a, H1b, H5.
- **V18** ("frozen" / "frozen-with-LoRA") carried on every
  H-statement that invokes $\phi_m$: all H1–H4.
- **V22 / R4** (measured vs. projected) carried on every T1-tier
  value (H2a, H3a) and every T3-tier value (H2c, H2d, H3b).
- **R3** (pool-relative scope on "no cited method" clauses) carried
  on H2 (Theorem 2 "no auxiliary learned model" clause) and H4
  (cluster-C orthogonal-path claim).
- **Agent-14 Bias-05 / Bias-06** (single-subject, English-only
  Case-3 scope) carried on H2a and H3a.
- **Agent-17 Theme D** (single-encoder-pair hedge) carried on H2a
  and H3a evidence-pending items.

---

## 11. Cross-references

- **`agent-21-theory-builder--theoretical-framework.md`** — source
  of every axiom / proposition / theorem anchor.
- **`agent-22-model-architect--master-architecture.md`** — source
  of Figure 1 arrow IDs (A1–A13); §2 interconnect table.
- **`agent-22-model-architect--subfigures.md`** — Figures 2, 3, 4
  visual anchors for H1a (Fig. 2), H2d / H3b (Fig. 3), H2a / H3a
  (Fig. 4 Phase 3 legend).
- **`agent-22-model-architect--variable-dictionary.md`** — every
  symbol in the H-statements with typed range and hedge discipline.
- **`agent-19-opportunity-identifier--research-opportunities.md`** —
  RO-1 (H5 Kaplan / Chinchilla re-fit); RO-8 (H1b empirical MI);
  RO-10 (H3 sample-complexity curve).
- **`agent-17-thematic-synthesizer--thematic-frameworks.md`** —
  six themes; H1 ↔ Theme B; H2 ↔ Theme C + D; H3 ↔ Theme F; H4 ↔
  Theme A; H5 ↔ Theme E.

---

## 12. Confidence

**HIGH** on the ledger's internal consistency, proposition-anchor
coverage, and falsifiability of every hypothesis.

- All 9 primary + secondary hypotheses are testable.
- Every hypothesis has an explicit falsification condition.
- Every hypothesis names its evidence tier (T1 / T2 / T3 / T4) and
  its anchor in the theoretical framework and Figure 1.
- All upstream hedges (V11, V18, V22, R3, R4, Bias-05, Bias-06,
  Theme D) carry to the corresponding hypotheses.
- No new citations are introduced (R3 compliance).
- No new primitives are introduced (MP-12 composition-not-invention).

**MEDIUM** on H2d (threshold-calibration matrix, 5 modalities of
Case 2) because multi-modal T3 priors have the widest uncertainty
range in the ledger.

## Changelog

- 2026-04-17 — Initial issue by agent-23-hypothesis-generator; LOCKED.
