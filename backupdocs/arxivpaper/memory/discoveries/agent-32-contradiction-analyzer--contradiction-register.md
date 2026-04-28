---
namespace: discoveries
agent_id: agent-32-contradiction-analyzer
timestamp: 2026-04-16T21:15:00Z
status: active
locked: false
supersedes: none
tags: contradictions, internal-audit, external-audit, apparent-contradictions, resolutions, must-disclose
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/patterns/agent-15-evidence-synthesizer--thematic-synthesis-blocks.md
  - memory/handoff/agent-31-to-32--ethics-ready.md
downstream_consumers:
  - agent-33-falsification-agent
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
  - agent-43-reproducibility-checker
---

# Contradiction Register — Internal + Apparent-External Contradictions

## 0. Purpose and scope

This register catalogues every **internal contradiction** (claim A vs.
claim B inside our own manuscript) and every **apparent external
contradiction** (our claim vs. a cited-pool claim) that an adversarial
reviewer is likely to surface. Each entry has:

- **Contradiction ID** (`IC-N` internal, `EC-N` external).
- **Statement A** / **Statement B** verbatim (or paraphrased when
  exact wording is not yet drafted).
- **Real or apparent?** — whether the surface tension reflects a
  genuine inconsistency we must repair, or a paradigmatic misalignment
  we can dissolve with a clarifying sentence.
- **Resolution** — the explicit textual move that resolves it.
- **Manuscript anchor** — the section(s) where the resolution lives.
- **Severity** — CRITICAL / HIGH / MEDIUM / LOW, calibrated to how
  likely the tension is to be raised in review.

**Scope discipline.** This register does **not** re-litigate
weaknesses already owned by the weakness register (agent-24). Where a
contradiction coincides with a weakness, we cross-reference to W-IDs.
This register also does **not** modify any locked constructs (agent-04)
or hypotheses (agent-23); it only surfaces the tensions and routes
them to clarifying inserts (see sibling paste-ready file).

**Total**: 11 contradictions surfaced. 6 internal (IC-1 … IC-6), 5
apparent-external (EC-7 … EC-11). 0 are real unresolvable
contradictions; all 11 resolve via disclosure, framing, or
language-tightening.

---

## 1. Internal contradictions

### IC-1 — "Third path" vs. composition-not-invention (MEDIUM)

**Statement A (mission brief title & §1 opener).**
"Teleological Constellation Training … **a Third Path Around the Data
Wall**." The manuscript positions TCT + DDA as a **third path**
alongside "Path 1: acquire more data" and "Path 2: synthesise more
data".

**Statement B (agent-17 Register R1 MP-12; agent-21 §0, §7).**
"The framework is **composition-not-invention** (MP-12). No new
primitives … each axiom, proposition, and theorem is built from
objects already defined and locked by agent-04. What is new is how
they compose into a single coherent spine."

**Real or apparent?** — **Apparent.** "Third path" names a novel
**route around a constraint** (the data wall); "composition-not-
invention" names the **construction mode** of the route (no new
primitives, only new composition of already-public primitives). The
two operate on different axes: novelty-of-destination vs. novelty-of-
ingredients. A route composed of existing road segments can still be
a new route if no prior map connected those segments end-to-end for
the stated purpose. MP-12 is a humility hedge about ingredients, not
a retraction of the route-level claim.

**Resolution.**
- §1 must contain a single sentence clarifying that "third path"
  refers to a **new composition** of existing components, not a new
  component. Paste-ready sentence in sibling file IC-1-RI.
- §3 (Framework opener) and §7 (Discussion) must repeat the
  distinction: ingredients are existing, composition is the
  contribution (per P4, role-composition coherence).
- §9 Conclusion must not elevate "third path" beyond MP-12 (no "we
  invented a new paradigm"; yes "we compose existing primitives into
  a route that the cited pool does not walk end-to-end").

**Manuscript anchor.** §1 opener; §3 Framework opener; §7 Discussion;
§9 Conclusion.

**Severity.** MEDIUM. Reviewers trained on R3 pool-relative novelty
(MP-16) will spot any wording that promises invention where we have
composition.

**Cross-references.**
- agent-17 Register R1, R3 (pool-relative scope).
- agent-20 MP-12 (composition-not-invention).
- agent-21 §0 (composition-not-invention); §7 (scope discipline).
- agent-10 novelty codes (EXT Chinchilla, CAT-EXT compression,
  BRK scalar-reward alignment) — "EXT" and "CAT-EXT" are the honest
  labels for most of our claims; only BRK is a structural break.

---

### IC-2 — "Sample-complexity collapse" headline vs. Case 2 T1 PENDING (HIGH)

**Statement A (mission brief Claim 4; §1 headline; §3.6; §9).**
"Identity-locked generation collapses from thousands of subject-
specific examples to 'one reference set per modality + frozen
centroids' when TCT is applied on top of DDA-enriched data." Theorem 1
states $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$ independent of
$|D|$.

**Statement B (agent-23 ledger H3b; mission brief case 2; agent-24
W3; Case 2 status "SFT+DPO training in progress"; H2d / H3b
"**T3 PROJECTED**").**
Case 2 (EchoMimicV3 + LoRA + TCT) numbers are **T3-projected, not
measured**. Case 3 is T1-measured but at $M=1$ (single modality) and
at a single operating point ($k=50$, single subject, English). The
sample-complexity **collapse** claim at $M>1$ is derivational under
Axioms 1–3, not measured.

**Real or apparent?** — **Apparent, but serious.** The theorem is
constructive (any pipeline with Phase 1/2/3 as defined satisfies the
bound by construction). The headline language risks reading as "we
measured collapse at $M>1$", which we have not done. The
contradiction is between **headline simplicity** and **evidence-tier
accuracy**.

**Resolution.**
- §1 headline must carry **T1 MEASURED** / **T3 PROJECTED** tier flags
  wherever the sample-complexity collapse is cited, per agent-01 V22
  and agent-23 §7 evidence tiers.
- §3.6 must state: "The collapse is derivational under Axiom 3; Case
  3 supplies the single T1 operating point at $k=50$, $M=1$; Case 2
  numbers are T3-projected and not yet measured."
- §6.3 Case 3 headline keeps the 0.961 SECS as T1-measured at $M=1$.
- §6.2 Case 2 opening sentence must disclose that numbers are
  projected and that the end-to-end run has not yet been completed.
- §7 Discussion must distinguish "the theorem holds constructively"
  from "the collapse has been measured at $M>1$" (we do the former,
  not the latter).
- §8 L-02 (agent-24 W3) already flags the single-operating-point
  weakness; the contradiction disclosure inherits that limitation.

**Manuscript anchor.** §1 headline; §3.6; §6.2 opener; §6.3 headline;
§7 Discussion; §8 Limitations L-02.

**Severity.** HIGH. This is the headline claim; any reader who
notices Case 2 is PENDING while §1 says "collapse" will flag an
overclaim.

**Cross-references.**
- agent-01 V22 (measured vs. projected clause-level flagging).
- agent-21 Theorem 1 confidence note (constructive bound + Axiom 3
  anchor).
- agent-23 H3, H3a, H3b, §7 evidence-tier summary, §8 falsification
  precedence.
- agent-24 W3 (single operating point, HIGH).

---

### IC-3 — "Verifiable per-output guarantees" vs. encoder-matched Case 3 (CRITICAL)

**Statement A (mission brief Claim 3; Theorem 2; §1 opener; §5.3,
§5.4).**
"TCT gives per-output verifiable guarantees at inference that scalar-
reward methods cannot match." $G_\tau(\hat y; C_P) = \mathbb{1}[
\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m]$ is computable per
output in $O(M \cdot d_{\max})$ with no auxiliary learned model.

**Statement B (agent-24 W1 CRITICAL; agent-23 H2a evidence-pending
"scorer diversity ablation"; agent-17 Theme D "single-encoder-pair
hedge"; agent-31 handoff item §Ethics reproducibility-vs-responsible-
disclosure trade-off).**
Case 3 uses **WavLM-base-plus-sv** as **both** Phase-3 guard encoder
(accept iff cosine $\geq 0.95$) **and** held-out SECS evaluator. The
0.961 mean SECS is an **encoder-matched** measurement: the number the
guard induces by construction. This does **not** constitute cross-
encoder identity verification.

**Real or apparent?** — **Real in the reading that "the 0.961
demonstrates cross-encoder identity-lock"; apparent in the reading
that "the guard is verifiable per output against the frozen target it
was defined against".** The verifiability claim is about the
**guard's own predicate**; the overclaim risk is that a reader
conflates "verifiable against the target encoder" with "verified
across encoders". Theorem 2 states the former; W1 warns against the
latter. Both statements are true — they describe different properties.

**Resolution.**
- §5.3 guard definition must state: "$G_\tau$ is a **per-output
  predicate against $C_P$ under $\Phi$**; verification is against the
  same frozen target the guard was defined against. Cross-encoder
  transportability is a separate question (agent-14 Bias-06, agent-17
  Theme D) and is not evidenced by within-family SECS."
- §6.3 Case 3 must carry the agent-24 W1 disclosure sentence verbatim
  near the 0.961 headline: "*The 0.961 mean SECS is an encoder-
  matched measurement: the guard and evaluator are both WavLM-base-
  plus-sv. This demonstrates the guard behaves as specified; it does
  not constitute cross-encoder identity verification.*"
- §5.4 attribution language must soften from "TCT guarantees identity"
  to "TCT enforces the per-output predicate against $C_P$; whether
  that predicate generalises across encoder families is a separate
  cross-encoder transportability question (open)".
- §7 Discussion must distinguish **guard-level verifiability** (a
  structural property of the predicate) from **identity-level
  verifiability** (would require architecturally-independent encoder
  ablation). We claim the former.
- §8 L-01 (per agent-24 W1) already owns the encoder-matched
  caveat; this contradiction disclosure inherits it.

**Manuscript anchor.** §1 (guarded "per-output predicate against the
target encoder"); §5.3 (guard definition with encoder scope); §5.4
(attribution language); §6.3 (W1 paste-ready sentence); §7
Discussion; §8 Limitations L-01.

**Severity.** CRITICAL. This is the single most likely exposure to
find in adversarial review: the headline number IS the number the
guard induces.

**Cross-references.**
- agent-14 Bias-06 (single-encoder).
- agent-17 Theme D (single-encoder-pair hedge).
- agent-21 Theorem 2 (guard is deterministic against $C_P$ under
  $\Phi$; does not claim cross-encoder property).
- agent-23 H2, H2a (evidence-pending: scorer-diversity ablation).
- agent-24 W1 CRITICAL (primary register entry for this).
- agent-29 VT-I3 / VT-C1 / VT-S4 CONFIRMED (encoder-matched).
- agent-30 R1-04 (encoder-matched rejection risk).

---

### IC-4 — "Frozen centroids are stable" vs. thresholds empirically calibrated (MEDIUM)

**Statement A (agent-21 Axiom 3; mission brief Claim 4; §5.3).**
"For frozen embedders whose output geometry is already well-conditioned
by pretraining, a reference set of $n_m \in [10, 10^2]$ is empirically
sufficient for useful $c_m$." Centroids are frozen after Phase 1 and
held constant through Phases 2 and 3. Axiom 3 asserts **stability**.

**Statement B (agent-04 TCT definition §3.2 Phase 3; agent-23 H2c /
H2d T3-projected threshold matrices; agent-24 W3 single operating
point).**
The **cosine thresholds** $\tau_m$ are **empirically calibrated per
modality** (not frozen by theory). H2c predicts
$\tau_{\text{text-style}} \in [0.80, 0.92]$ for Case 1; H2d predicts a
5-modality matrix for Case 2 with intervals of width 0.07–0.15 per
modality. Case 3's $\tau = 0.95$ was set by measurement (WavLM SECS
same-session band). Thresholds **are tuned**; centroids **are
frozen**.

**Real or apparent?** — **Apparent.** Frozen-centroid stability and
empirically-calibrated thresholds operate on different objects. The
centroid $c_m$ is a L2-normalised mean direction — stability means
the direction does not drift across re-computations on the same
reference set (Axiom 3). The threshold $\tau_m$ is a per-modality
acceptance floor — it is a **parameter of the predicate $G_\tau$**,
not of the centroid. The predicate's parameters are always calibrated;
this is a tuning statement, not a drift statement.

**Resolution.**
- §5.3 must state explicitly: "Centroids $\{c_m\}$ are **frozen**
  after Phase 1 (Axiom 3 stability). Thresholds $\{\tau_m\}$ are
  **calibrated empirically** per modality, against a same-session-band
  floor on the reference set. The two are distinct: centroid stability
  is a property of the measurement geometry; threshold calibration is
  a tuning decision over the acceptance predicate."
- §6.3 Case 3 must show the $\tau = 0.95$ came from measured same-
  session band on the 50-clip reference set, not from theory.
- §10 Future Work must name **threshold-curve characterisation per
  modality** (per RO-10 agent-19; Q2 agent-19 false-positive / false-
  negative curve).

**Manuscript anchor.** §5.3 (centroid-vs-threshold distinction); §6.3
(Case 3 measured $\tau$ origin); §10 Future Work.

**Severity.** MEDIUM. Reviewers trained on "if the threshold is tuned,
the acceptance rate is not theoretically characterised" will push
here. The distinction is clean if surfaced early.

**Cross-references.**
- agent-04 TCT definition §3.2 Phase 3.
- agent-21 Axiom 3 (centroid stability, not threshold fixity).
- agent-23 H2c, H2d (threshold calibration predictions).
- agent-24 W3 (single-operating-point weakness touches threshold
  curve).

---

### IC-5 — "DDA exempt from model collapse" vs. TCT Phase 3 uses TTS output (MEDIUM)

**Statement A (mission brief Claim 1 implication; §3.3; agent-21 P2
collapse silence; agent-23 H4).**
"DDA pipelines are structurally not subject to the model-collapse
regime of Shumailov et al. (2024) because no recursion of generator
outputs exists in the pipeline; $\phi_m(x)$ is a frozen measurement of
a real input $x \in D$; $D'$ is a decomposition of $D$, not synthesis
of new inputs."

**Statement B (TCT Phase 3 best-of-$K$; Case 3 protocol $K=12$,
$T=0.3$, $\tau=0.95$; agent-23 H2a).**
Case 3's best-of-$K$ step **samples $K$ candidates from Qwen3-TTS**
(a generator) and **selects the one with maximum WavLM cosine to
$c_m$**. The selected output is the answer. In principle, that
selected output could be fed back as a reference for re-computing the
centroid; that would introduce a minor generator-in-loop recursion at
the centroid-maintenance level.

**Real or apparent?** — **Apparent.** The paper's current protocol
**does not** re-feed TTS output into centroid re-computation. The
50-clip centroid is built from **real** clips of the author; best-of-
$K$ **selects** among TTS candidates but does **not update $c_m$**
with the winner. P2 remains true for the pipeline as specified. The
contradiction risk is that a careless reader assumes best-of-$K$ is
feedback into training or centroid update; it is not.

**Resolution.**
- §5.3 (guard definition) must state: "Phase 3 best-of-$K$ is an
  **inference-time selection** step. The centroid $c_m$ is **not
  updated** with the selected candidate; centroids remain frozen per
  Phase 1."
- §6.3 Case 3 must specify: "The 50-clip centroid is built from real
  author clips; best-of-$K$ selection at $K=12$ does not feed winners
  back into centroid re-computation."
- §8 L-06 (new): note that **if a future variant** were to update the
  centroid with selected outputs, P2's scope argument would need
  reassessment; the current pipeline's claim is about the current
  pipeline.
- §10 Future Work can reference RO-10 (sample-complexity curve) as
  implicitly bounded to "real reference sets only"; any "centroid
  recursion" extension is flagged as a scope change.

**Manuscript anchor.** §5.3 (inference-time selection vs. training-
time feedback); §6.3 Case 3 methodology; §8 Limitations L-06; §10
Future Work scope note.

**Severity.** MEDIUM. The Shumailov cluster is well-known; any hint
of recursion collides with the collapse-silent headline. Easy to
resolve with one sentence; painful if missed.

**Cross-references.**
- agent-21 P2 (collapse silence).
- agent-23 H4 (collapse-silent claim).
- agent-04 TCT §3.2 Phase 3.
- agent-24 W9 (H4 scope boundary for embedder-mediated curation).

---

### IC-6 — "Release TCT protocol" vs. withhold Case 2/3 artefacts (HIGH)

**Statement A (mission brief "Every URL must be verifiable";
agent-25 `tct-protocol.md`; agent-27 instrument-specifications;
agent-31 "Released: Case 1 Shakespeare LoRA training scripts";
agent-43 reproducibility-checker downstream expectations).**
"The full TCT protocol is released (public)." Context Graph and
ClipCannon source code, Case 1 LoRA, agent-27 reproducibility manifest
schema, weight-pin pointers with SHA-256 + Zenodo DOIs — all released.

**Statement B (agent-31 §Ethics asymmetric release; agent-31 misuse-
mitigation audit; agent-29 VT-I1 and VT-S4; agent-31 §Ethics:
"Withheld: Case 2 raw footage, 2,362 derived clips, Santa LoRA
weights, per-modality centroid .npz, FLAME fits; Case 3 489-clip
speaker corpus, 50-clip reference centroid, two 8–9 s reference
recordings, 10 held-out recordings, any subject-likeness synthetic
artefact.").**
Case 2 and Case 3 subject artefacts are **withheld** on ethical and
dual-use grounds. Thus "full TCT protocol released" does not mean
"byte-identical reproduction possible for Cases 2 and 3".

**Real or apparent?** — **Apparent.** "Protocol release" and
"artefact release" are different release scopes. The **methodology**
is fully released (protocol pseudo-code, hyperparameters, manifest
schema, evaluation metrics). The **subject-specific artefacts**
(centroids, weights, reference clips, held-out clips) are withheld
for Cases 2 and 3 on ethical / consent / dual-use grounds. The
contradiction risk is that "reproducible" without qualification
misleads readers into expecting byte-identical reruns for Cases 2 / 3.

**Resolution.**
- §1, §5, §6 must consistently use the two terms agent-31 already
  coined:
  - **"Byte-identical reproducibility"** — Case 1 Shakespeare LoRA.
  - **"Protocol-level reproducibility"** — Cases 2 and 3.
- §8 L-Reproducibility (new; paste-ready in sibling file): "This
  paper makes an asymmetric release commitment. The TCT protocol,
  evaluation scripts, manifest schema, and hyperparameter tables are
  released in full. Case 1 artefacts (Shakespeare corpus + LoRA
  weights) are released for byte-identical reproduction. Case 2 and
  Case 3 subject-specific artefacts (centroids, LoRA weights, raw
  footage / clips, held-out items) are **withheld on consent and
  dual-use grounds** (§Ethics). Cases 2 and 3 are therefore
  reproducible at the **protocol level** (same pipeline, same
  hyperparameters, applied to a consented subject's own data),
  **not at the artefact level**."
- §Ethics must be consistent with §8 L-Reproducibility.
- §6.2 / §6.3 openers must cite the asymmetric release explicitly
  (agent-31 paste-ready sentences).

**Manuscript anchor.** §1 opener; §5 TCT protocol description; §6.2
Case 2 opener; §6.3 Case 3 opener; §8 L-Reproducibility; §Ethics.

**Severity.** HIGH. Reviewers (especially at arXiv + NeurIPS where
reproducibility checklists are mandatory) will quickly spot any
"reproducible" claim that does not carry the asymmetric qualifier.
Also a prerequisite for ethics-posture consistency (agent-31 pair #1).

**Cross-references.**
- agent-29 VT-I1 (sentence-selection form), VT-S4 (scope of
  reproducibility).
- agent-30 R4-02 (subject consent), R4-04 (WavLM artefact
  distribution).
- agent-31 §Ethics, §Competing Interests, misuse-mitigation,
  reproducibility-vs-responsible-disclosure trade-off.
- agent-43 reproducibility-checker (downstream consumer).

---

## 2. Apparent-external contradictions

### EC-7 — Our "meaning compression" vs. Delétang et al. (2024) "language modeling is compression" (MEDIUM)

**Statement A (mission brief Claim 2; §3.1; agent-21 §5.2; agent-23
H5).**
"Meaning compression is a distinct fourth compression category
orthogonal to bit-, weight-, and activation-compression (e.g.,
TurboQuant). Unit: **structured-signals / raw-data-unit**."

**Statement B (Delétang et al. 2024 ICLR `deletang2024compression`
T1; Huang et al. 2024 COLM `huang2024compression` T2; agent-15
Theme 3 bridge).**
"Language modeling **is** compression." Delétang et al. demonstrate
trained LLMs as general-purpose compressors across text, image, audio
at bits-per-byte competitive with domain-specific codecs. Huang et
al. report $r \approx 0.95$ between compression ratio and downstream
capability across 30+ LLMs.

**Real or apparent?** — **Apparent.** The two compression notions
are measured on **different axes and at different stages of the
pipeline**.
- **Delétang / Huang compression** is **bit-compression** applied to
  held-out sequences **from trained generative models**. The unit is
  **bits of output / bits of input** (or its entropic equivalent).
  The reading is at inference, against sequences.
- **Meaning compression** (ours) is **signals-per-raw-input** applied
  **at training-data preparation time** by a frozen embedder panel.
  The unit is **structured supervisory signals / raw data unit**. The
  reading is at dataset construction, against corpora.
The two are not the same object. Huang's correlation is a **within-
model** compression-capability correlation (agent-17 tension 2
resolved); V11 is an **across-model** embedder-independence hedge.
They operate on orthogonal axes. MDL's ratio object (ISO-04) is the
shared information-theoretic root and carries cleanly across scales.

**Resolution.**
- §3.1 must open with a **unit-analysis table** (already drafted in
  agent-21 §5.2) that locates the four compression categories on
  distinct unit pairs and stages.
- §5.2 must state: "Delétang et al. (2024) and Huang et al. (2024)
  are **not contradicted**. Their compression framing reads
  bit-compression at inference on trained models; our meaning-
  compression framing reads signal-density at training-data
  preparation via a frozen embedder panel. The two are complementary
  axes of the same information-theoretic tradition (MDL / rate-
  distortion, ISO-04)."
- §7 Discussion (compression-as-intelligence synthesis) must avoid
  any wording that reads "they were wrong; we are right". We EXT the
  compression-as-intelligence frame (agent-10 CAT-EXT), we do not
  break it.
- Apply agent-24 W8 remediation: tighten §3.1 / §5.2 wording from
  "dimensionally distinct" to "**semantically distinct / measured as
  structured-signal density rather than Shannon bits at a different
  pipeline stage**".

**Manuscript anchor.** §3.1 unit-analysis table; §5.2 relation-to-
existing-frameworks; §7 Discussion.

**Severity.** MEDIUM. Any reviewer who has read the Delétang paper
will notice the surface collision. One paragraph resolves it.

**Cross-references.**
- agent-10 CAT-EXT (category-extension).
- agent-17 Theme 3 bridge; tension 2 resolution (within-model vs.
  across-model).
- agent-23 H5 (dimensional / semantic distinctness).
- agent-24 W8 (H5 "dimensionally distinct" language is overclaim).

---

### EC-8 — Our "DDA exempt from collapse" vs. Shumailov et al. (2024) "AI models collapse when trained on recursively generated data" (MEDIUM)

**Statement A (mission brief implied; §3.3; agent-21 P2; agent-23
H4).**
"DDA's operational procedure does not satisfy the generator-in-loop
precondition: by Axiom 1, each $\phi_m(x)$ is a frozen measurement of
a real input $x \in D$, independent of any generator's parameters …
Therefore the collapse theorems do not apply to DDA."

**Statement B (Shumailov et al. 2024 Nature `shumailov2024nature` T1;
Dohmatob et al. 2024 `dohmatob2024regression`, `dohmatob2024tails`
T1; Alemohammad et al. 2023 `alemohammad2023mad` T1 ICLR; agent-15
Theme 2).**
"AI models collapse when trained on recursively generated data."
Four independent Tier-1 sources (closed-form + simulation + empirical
MAD) establish that distributional drift occurs under recursion.

**Real or apparent?** — **Apparent.** The Shumailov theorems are
theorems **about recursion**. DDA contains no recursion:
$\phi_m(x)$ depends only on $x$ (real input) and frozen $\phi_m$. The
derived dataset $D' = \{\phi_m(x), \rho_{jk}\}$ is a **decomposition**
of $D$, not **synthesis** of new inputs. The **precondition** of the
collapse theorems (generator-in-loop) is **not satisfied**; therefore
the theorems are **silent** on DDA (i.e., they neither apply nor
forbid; they are out of scope). We do **not** claim "DDA has a new
independence-from-collapse result"; we claim "the precondition fails,
so the theorems don't reach this configuration" (agent-21 P2
confidence note; agent-11 CL-06 MODERATE-categorical).

**Resolution.**
- §3.3 must use the wording: "DDA is **collapse-silent by
  construction**: no generator recursion exists in the pipeline;
  therefore the Shumailov-2024 theorems' preconditions fail and the
  theorems are **silent on this configuration**. This is a **scope
  argument**, not a new independence-from-collapse theorem."
- §8 L-05 (derived from agent-24 W9) must flag the scope boundary
  against **embedder-mediated data curation**: our claim addresses
  generator-in-loop recursion only; other forms of data-selection
  feedback are out of scope of the specific cited theorems.
- §10 Future Work can name RO-Xx (empirical toy demonstration of
  DDA-enriched training vs. recursive self-distillation) as a
  clean falsification test.

**Manuscript anchor.** §3.3; §8 L-05; §10 Future Work.

**Severity.** MEDIUM. Any reviewer with the Shumailov paper in mind
will test whether we have grasped the precondition correctly. The
wording "scope argument, not a new theorem" resolves it cleanly.

**Cross-references.**
- agent-11 CL-06 (collapse-silent categorical scope).
- agent-15 Theme 2 (collapse cluster synthesis).
- agent-21 P2 confidence note.
- agent-23 H4.
- agent-24 W9 (scope boundary for embedder-mediated curation).

---

### EC-9 — Our "per-output guarantee" vs. Ouyang et al. (2022) RLHF "alignment via preferences" (MEDIUM)

**Statement A (mission brief Claim 3; §5.1, §5.3, §5.4; agent-21
P3, Theorem 2; agent-21 §5.3 "BRK of cluster-I scalar-reward
alignment").**
"TCT delivers structural properties that scalar-reward alignment
cannot match (per-output verifiability, multi-modal conjunction,
frozen target). This is a **structural break** with the scalar-
reward paradigm (BRK-I)."

**Statement B (Ouyang et al. 2022 NeurIPS `ouyang2022instructgpt`
T1; Rafailov et al. 2023 NeurIPS `rafailov2023dpo` T1; Bai et al.
2022 `bai2022constitutional` T2 closed-artefact; agent-15 Theme 7).**
RLHF / DPO / Constitutional AI optimise a generator against a
learned scalar reward model trained on human (or AI) preference
pairs. These methods are the canonical alignment lineage.

**Real or apparent?** — **Apparent.** We do **not** claim "RLHF is
wrong" or "RLHF fails to solve its problem". We claim **TCT solves a
different problem class** and **the comparison is on orthogonal
axes**:
- **Aim**: scalar-reward aim is a **learned scalar function**; spine
  aim is a **frozen multi-modal constellation** $C_P$.
- **Instrument**: scalar-reward instrument is a **learned reward
  model**; spine instruments are **frozen embedders** $\Phi$.
- **Metric**: scalar preference vs. fixed cosine.
- **Runtime verification**: scalar-reward does **not** run per-output
  guards at inference; spine runs the constellation guard on every
  output.
The structural break is in **aim + instrument + metric + runtime
posture**, not in **performance on RLHF benchmarks**. We do not argue
TCT yields higher preference scores than RLHF on RLHF benchmarks;
we argue TCT offers a class of per-output geometric verifiability
that RLHF benchmarks do not evaluate.

**Resolution.**
- §5.1 must open with a **scalar-vs-geometric contrast table**
  (already drafted, agent-21 §5.3):

| Axis | Scalar-reward alignment (RLHF, DPO, CAI) | Geometric-proximity alignment (TCT) |
|---|---|---|
| Aim | Learned scalar function | Frozen multi-modal constellation $C_P$ |
| Instrument | Learned reward model $r_\psi$ | Frozen embedders $\Phi$ |
| Metric | Scalar preference | Per-modality cosine |
| Runtime check | None (typical) | Per-output guard $G_\tau$ |
| Problem class | Preference alignment | Identity lock + per-output verifiability |

- §5.3 must state: "**We do not claim RLHF is wrong.** We claim TCT
  addresses a different problem class (identity-locked generation
  with per-output verifiability) that scalar-reward alignment does
  not target. The 'structural break' is a **different paradigm for a
  different problem**, not a competitive claim on RLHF benchmarks."
- §7 Discussion must carry the same framing: complementary paradigms
  for complementary problems, not competing methods for the same
  problem.

**Manuscript anchor.** §5.1 contrast table; §5.3 paradigm-scope
sentence; §7 Discussion.

**Severity.** MEDIUM. RLHF is the most-cited modern alignment
paradigm; any wording that reads "we replace RLHF" would draw
immediate scepticism. The different-problem-class framing resolves
it.

**Cross-references.**
- agent-10 BRK-I code.
- agent-15 Theme 7 (scalar-reward-dominated alignment).
- agent-17 Register R3 (pool-relative scope).
- agent-21 P3, Theorem 2, §5.3.
- agent-23 H2 (no outperformance claim on RLHF benchmarks).

---

### EC-10 — Our "human parity exceeded" vs. Chen et al. (2024) VALL-E 2 "human parity at 0.881 WavLM-large-sv" (HIGH)

**Statement A (mission brief Case 3; §1 opener; §6.3; §9).**
"Voice cloning beyond 0.95 WavLM SECS … **0.961 mean SECS (max
0.975)** on 10 unseen sentences; DNSMOS P.808 parity (3.93 vs 3.93)
and UTMOS parity (3.012 vs 2.997) with real recordings." Case 3
reports **exceeding** the published zero-shot-TTS parity band (~0.86–
0.89 WavLM-cosine; VALL-E 2 0.881 WavLM-large-sv).

**Statement B (Chen et al. 2024 `chen2024valle2` T2 — closed
artefact per agent-13 Flag 1; agent-15 Theme 5 "zero-shot TTS
plateau-pattern"; agent-14 Bias-06 single-encoder).**
VALL-E 2 claims "human parity" on zero-shot TTS at SIM-o $\approx
0.64$ and WavLM-large-sv speaker similarity $\approx 0.881$. This is
the strongest published zero-shot TTS identity number in cluster E.

**Real or apparent?** — **Apparent, but potentially misleading if
not disclosed.** Case 3 is not apples-to-apples with VALL-E 2:
- **VALL-E 2 is zero-shot** (single-prompt conditioning, no reference
  set).
- **Case 3 is personalised** (50-clip reference centroid + TCT guard
  + best-of-$K=12$ at $T=0.3$).
- **Evaluation encoders differ**: VALL-E 2 uses WavLM-**large**-sv;
  Case 3 uses WavLM-**base-plus**-sv. These are related but not
  identical encoders.
- **Test sets differ**: VALL-E 2 reports on LibriSpeech test-clean;
  Case 3 reports on 10 author-selected sentences (agent-29 VT-I1
  sentence-selection-form flag; agent-14 Bias-08 self-reported).
- Therefore the 0.961 > 0.881 surface comparison is an
  **incommensurable comparison across paradigms**. The paper must
  say so.

**Resolution.**
- §6.3 Case 3 must contain a **paradigm-scope disclosure sentence**
  (agent-31 handoff Section 9 already notes this is covered): "*The
  0.961 mean SECS is a **personalised** protocol result (50-clip
  centroid + best-of-12 + $\tau=0.95$); published zero-shot TTS
  numbers (VALL-E 2 at $\sim 0.881$ WavLM-large-sv) are from a
  **different problem class** (no subject reference set, single-
  prompt). The two numbers are not directly comparable; the
  comparison reported here is a **protocol-level demonstration that
  the personalised setting admits identity agreement above the
  zero-shot plateau**, not a claim that TCT outperforms VALL-E 2 on
  VALL-E 2's task.*"
- §1 opener must also carry the paradigm-scope disclosure (single
  clause: "*in a personalised setting distinct from zero-shot TTS
  paradigms*").
- §6.3 must also disclose the encoder difference (WavLM-base-plus-sv
  vs. VALL-E 2's WavLM-large-sv) as part of the encoder-matched W1
  disclosure.
- §9 Conclusion must not frame the result as "exceeding VALL-E 2" in
  an apples-to-apples sense.
- agent-13 Flag 1 (VALL-E 2 closed-artefact) carries to any Case-3-
  vs-VALL-E-2 comparison.

**Manuscript anchor.** §1 opener; §6.3 Case 3 headline + paradigm-
scope disclosure; §9 Conclusion.

**Severity.** HIGH. "Human parity exceeded" will attract the maximum
scepticism; failure to disclose the paradigm mismatch is a near-
guaranteed reviewer exposure.

**Cross-references.**
- agent-13 Flag 1 (VALL-E 2 closed-artefact).
- agent-14 Bias-06 (single-encoder), Bias-08 (self-reported
  sentences).
- agent-15 Theme 5 (zero-shot TTS plateau; Case 3 bridge already
  specifies personalised paradigm).
- agent-17 Theme D (single-encoder-pair hedge).
- agent-24 W1 (encoder-matched evaluation; WavLM-base-plus-sv).
- agent-29 VT-I1 (sentence-selection form).

---

### EC-11 — Commercial positioning (Teleox.ai / Leapable.ai) vs. arXiv's non-marketing expectation (LOW)

**Statement A (mission brief "Affiliations: Teleox.ai (primary);
Leapable.ai (companion marketplace)").**
Author affiliations are commercial entities. Context Graph is the
**backbone** of Teleox.ai; Leapable.ai is the companion marketplace.
The commercial context is real.

**Statement B (mission brief hard constraint: "No vendor pitch prose
in the body. Teleox.ai/Leapable.ai acknowledgement only." arXiv
moderator norms.).**
arXiv cs.LG routinely rejects papers that read as commercial
pitches; FM-01 (marketing-creep failure mode) is a submission-
blocking gate.

**Real or apparent?** — **Apparent.** Commercial affiliation is not
in itself a marketing claim. The contradiction-risk vector is
**prose framing**: if §4 (Context Graph + ClipCannon) reads as a
product pitch, the manuscript fails FM-01 regardless of the COI
disclosure being formally correct.

**Resolution.**
- §Competing Interests (agent-31 paste-ready) discloses the
  affiliations in page-1 footnote + full §CI variants; this is the
  formal channel.
- §4 (Context Graph + ClipCannon) must frame the systems as
  **open-source research systems** (source code released), not as
  commercial products. agent-31 handoff item #4 (commercial
  affiliation vs. methodology novelty claim) already names this
  resolution.
- FM-01 regex scan (embedded in §CI doc §3) must return **zero
  matches** on §1–§9 body text before submission lock (agent-42
  gate).
- §9 Conclusion must not end with a commercial framing; ends with
  research-future framing.
- §Acknowledgements can reference the commercial entities as funders
  / employers; the body must not.

**Manuscript anchor.** §1–§9 body (FM-01 compliant); §Competing
Interests (formal disclosure); §Acknowledgements (factual funding).

**Severity.** LOW (if FM-01 regex pass succeeds) / CRITICAL (if it
fails). The severity is conditional on agent-42's regex pass.

**Cross-references.**
- agent-01 FM-01 (marketing creep).
- agent-14 Bias-03 (industry-lab concentration).
- agent-31 §Competing Interests (footnote + full variants).
- agent-42 (adversarial-reviewer FM-01 regex scan gate).

---

## 3. Summary table

| ID | Type | Contradiction topic | Real or apparent | Severity | Resolution venue |
|---|---|---|---|---|---|
| **IC-1** | Internal | "Third path" vs. composition-not-invention | Apparent | MEDIUM | §1, §3, §7, §9 language |
| **IC-2** | Internal | Sample-complexity collapse vs. Case 2 T1 PENDING | Apparent | HIGH | §1, §3.6, §6.2, §7, §8 L-02 |
| **IC-3** | Internal | Per-output guarantee vs. encoder-matched Case 3 | Real/apparent | CRITICAL | §5.3, §5.4, §6.3, §7, §8 L-01 |
| **IC-4** | Internal | Frozen centroids vs. calibrated thresholds | Apparent | MEDIUM | §5.3, §6.3, §10 |
| **IC-5** | Internal | DDA collapse-silent vs. best-of-K uses TTS | Apparent | MEDIUM | §5.3, §6.3, §8 L-06 |
| **IC-6** | Internal | Release protocol vs. withhold Case 2/3 artefacts | Apparent | HIGH | §1, §5, §6, §8 L-Reproducibility, §Ethics |
| **EC-7** | External | Meaning compression vs. Delétang "LM is compression" | Apparent | MEDIUM | §3.1, §5.2, §7 |
| **EC-8** | External | DDA collapse-silent vs. Shumailov 2024 | Apparent | MEDIUM | §3.3, §8 L-05, §10 |
| **EC-9** | External | Per-output guarantee vs. RLHF | Apparent | MEDIUM | §5.1 table, §5.3, §7 |
| **EC-10** | External | 0.961 SECS vs. VALL-E 2 "human parity" | Apparent | HIGH | §1, §6.3, §9 |
| **EC-11** | External | Commercial COI vs. arXiv non-marketing norm | Apparent (conditional) | LOW/CRITICAL | §Competing Interests; FM-01 regex |

**No real unresolvable contradictions found.** All 11 resolve via
disclosure, framing, or language-tightening; none requires a
structural change to the theoretical framework (agent-21), the
hypothesis ledger (agent-23), or the construct definitions (agent-04).

---

## 4. Dependencies and consumers

**Upstream agents** whose work anchors this register:
- agent-21 (theoretical framework) — provides P1–P5, T1–T2, and the
  five relation-to-existing-frameworks paragraphs (§5.1–§5.4).
- agent-23 (hypothesis generator) — provides H1–H5 with evidence
  tiers and falsification conditions.
- agent-24 (causal simulator) — provides W1–W10 weaknesses; IC-3 and
  IC-5 are contradiction-views of W1 / W2 and W9 respectively.
- agent-31 (ethics reviewer) — provides the six contradiction pairs
  already flagged in the handoff; IC-6 and EC-11 inherit them.
- agent-15 (evidence synthesiser) — provides theme blocks that
  identify the external literature apparent-contradictions EC-7,
  EC-8, EC-9, EC-10.

**Downstream consumers** of this register:
- agent-33 (falsification-agent) — inherits the IC-2, IC-3, EC-8,
  EC-10 falsification hooks (already stated in agent-23 §8).
- agent-37 / agent-38 / agent-39 (section writers) — apply the
  paste-ready clarifying sentences (sibling file).
- agent-42 (adversarial-reviewer) — uses the register as a pre-flight
  checklist; runs FM-01 regex on EC-11.
- agent-43 (reproducibility-checker) — IC-6 asymmetric release
  posture is the governing frame.

---

## 5. Confidence

**HIGH** on internal contradictions (IC-1 … IC-6): each is
well-defined, scope-limited, and resolved by a paste-ready sentence
already drafted.

**HIGH** on external apparent contradictions EC-7, EC-8, EC-9, EC-10:
each is a paradigmatic misalignment resolved by explicit scope
disclosure, and each has a pre-existing bridge in either agent-15
(thematic synthesis) or agent-21 (§5 relation to existing
frameworks).

**MEDIUM** on EC-11: severity is conditional on agent-42's FM-01
regex pass. If the regex finds matches, EC-11 escalates to CRITICAL
and the matched prose must be rewritten.

**HIGH** that **no real, unresolvable contradictions** exist in the
manuscript as planned. Every surface tension is either a paradigm-
scope misalignment (EC-7, EC-8, EC-9) or a headline-vs-evidence-tier
framing issue (IC-2, IC-3, IC-5, EC-10) or an asymmetric-release
statement (IC-6, EC-11). All resolve via the clarifying inserts in
the sibling paste-ready file.

## Changelog

- 2026-04-16 — Initial issue by agent-32-contradiction-analyzer.
