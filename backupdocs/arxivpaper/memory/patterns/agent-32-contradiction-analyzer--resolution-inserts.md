---
namespace: patterns
agent_id: agent-32-contradiction-analyzer
timestamp: 2026-04-16T21:25:00Z
status: active
locked: false
supersedes: none
tags: paste-ready, clarifying-sentences, resolution-inserts, section-writers, contradiction-repair
depends_on:
  - memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-31-ethics-reviewer--ethics-statement.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
downstream_consumers:
  - agent-35-abstract-writer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-38-case-study-writer
  - agent-39-discussion-writer
  - agent-40-conclusion-writer
  - agent-41-ethics-writer
  - agent-42-adversarial-reviewer
---

# Resolution Inserts — Paste-Ready Clarifying Sentences

## 0. Purpose

Every contradiction in the register has a **paste-ready clarifying
sentence** (or short paragraph) that dissolves the tension without
adding new content or rewriting upstream constructs. Section writers
(agents 35–41) paste these verbatim at the named anchor points.
Adversarial-reviewer (agent-42) verifies the resolutions are in
place before regex scans.

Each insert has:
- **ID** matching the register entry.
- **Anchor** — section + position.
- **Insert** — exact sentence(s) to paste.
- **Tone discipline** — hedge vocabulary (V11, V18, V22, R3, R4).

No new citations are introduced. No new primitives are introduced
(MP-12). Every insert respects agent-01 failure-mode list and
agent-01 evaluation-criteria.

---

## 1. Internal contradiction inserts

### IC-1-RI — "Third path" is a composition, not a new primitive

**Anchor 1 (§1 opener, after the three-paths framing is introduced).**

> *We use "third path" to name a **route** around the data-wall
> constraint, not a newly-invented primitive. Every component of that
> route — frozen embedders, contrastive geometry, LoRA, flow-matching,
> best-of-$K$ sampling, cosine thresholds — is drawn from the cited
> literature. The contribution is the **composition**: a particular
> end-to-end arrangement of these components that the cited pool
> does not walk end-to-end for the stated purpose of identity-locked,
> per-output-verifiable generation.*

**Anchor 2 (§3 Framework opener).**

> *The framework is composition-not-invention. No new primitive is
> introduced; each axiom, proposition, and theorem below is built
> from objects already locked by the construct-definer. The
> contribution is how they compose into a single coherent spine
> (Proposition 4: role-composition coherence).*

**Anchor 3 (§7 Discussion, novelty-placement paragraph).**

> *We EXT Chinchilla scaling by adding a signal-density axis (agent-
> 10 EXT); we CAT-EXT compression-as-intelligence by adding a
> signal-count category (agent-10 CAT-EXT); we structurally BRK with
> scalar-reward alignment by swapping aim, instrument, metric, and
> runtime posture (agent-10 BRK). "Third path" names this
> combined movement as a route; each ingredient is existing prior
> art.*

**Anchor 4 (§9 Conclusion).**

> *We do not claim a new theoretical primitive; we claim a
> composition of existing primitives into an end-to-end identity-
> locked, per-output-verifiable pipeline that the cited pool does
> not deploy as a unit.*

**Tone.** R3 pool-relative scope; V11 approximately-independent
hedge carried; MP-12 composition-not-invention explicit.

---

### IC-2-RI — Sample-complexity collapse with evidence-tier flags

**Anchor 1 (§1 headline, on first mention of sample-complexity
collapse).**

> *Under Teleological Constellation Training applied on top of DDA-
> enriched data, the subject-specific sample complexity for
> identity-locked generation is $O(M \cdot k)$ with $k \in
> [10, 10^2]$ (Theorem 1), independent of the general corpus size.
> The theorem is constructive; the anchor measurement is
> $M=1, k=50$ (Case 3, **T1 MEASURED**). Cases 1 and 2 are
> **T3 PROJECTED** and not yet measured at $M>1$.*

**Anchor 2 (§3.6 sample-complexity framing).**

> *The collapse is a **derivational** statement under Axioms 1–3,
> anchored by a **single T1 operating point** (Case 3 voice at
> $k=50$). The curve over $n_m$ is named future work (RO-10); the
> multi-modal $M>1$ measurement is named future work (pending Case
> 2 T1). The language "collapse" refers to the **theoretical bound**,
> not to an empirical curve we have measured.*

**Anchor 3 (§6.2 Case 2 opener, verbatim agent-31 sentence fragment).**

> *Case 2 numbers reported in this section are **T3-projected**;
> the full EchoMimicV3 + LoRA + TCT end-to-end run with per-modality
> constellation-guard evaluation has not yet been completed at
> submission time. The architecture and protocol are described in
> full; the empirical instantiation is deferred.*

**Anchor 4 (§7 Discussion, attribution paragraph).**

> *We distinguish two propositions: (a) **Theorem 1 is constructive**
> — any pipeline with TCT Phase 1/2/3 as defined satisfies the
> $O(M \cdot k)$ bound by construction; (b) **whether $k \in [10,
> 10^2]$ suffices is an empirical claim** about well-conditioned
> frozen embedders (Axiom 3). Case 3 supplies one operating point
> for (b) at $M=1$; Cases 1 and 2 extend the protocol to new
> modalities but do not supply new $k$ measurements.*

**Anchor 5 (§8 Limitations L-02, inherits agent-24 W3).**

> *Single operating point on the sample-complexity curve. Axiom 3's
> $[10, 10^2]$ range is anchored by one datum ($k=50$ in Case 3);
> the curve shape is unknown. Future work (RO-10) must run an
> $n_m$-ablation to quantify the curve per modality.*

**Tone.** V22 clause-level measured/projected flagging; R4 tier flags.

---

### IC-3-RI — Per-output guarantee is against the target encoder (NOT across encoders)

**Anchor 1 (§5.3 guard definition, after the predicate is stated).**

> *The guard $G_\tau(\hat y; C_P)$ is a **per-output predicate
> against $C_P$ under $\Phi$**: verification is against the same
> frozen embedder panel the guard is defined against. Cross-encoder
> transportability — whether identity agreement holds under
> architecturally-independent encoders — is a **separate question**,
> and within-family SECS does not evidence it. We carry the
> single-encoder-pair hedge (agent-14 Bias-06; agent-17 Theme D)
> throughout.*

**Anchor 2 (§6.3 Case 3, W1 disclosure, verbatim from agent-24 W1).**

> *The 0.961 mean SECS is an **encoder-matched** measurement: the
> Phase-3 guard encoder and the held-out SECS evaluator are both
> WavLM-base-plus-sv. This demonstrates the guard behaves as
> specified; **it does not constitute cross-encoder identity
> verification**. Cross-encoder evaluation (e.g., ECAPA-TDNN,
> Resemblyzer) is named future work.*

**Anchor 3 (§5.4 attribution language tightening).**

> *TCT enforces a per-output predicate against $C_P$ under $\Phi$.
> We do **not** claim identity-level cross-encoder verifiability from
> this; we claim guard-level verifiability against the target
> encoder panel.*

**Anchor 4 (§8 Limitations L-01, inherits agent-24 W1).**

> *Encoder-matched evaluation on Case 3. The empirical anchor
> measurement uses the same encoder family for guard and evaluator.
> Any claim that "0.961 SECS demonstrates cross-encoder identity" is
> **not supported** by this paper; cross-encoder ablation is named
> future work.*

**Tone.** Bias-06 carried; Theme D carried; CRITICAL weakness W1
explicitly owned.

---

### IC-4-RI — Centroids frozen; thresholds calibrated

**Anchor 1 (§5.3 guard definition, after centroid freeze statement).**

> *We distinguish two objects of the guard: centroids $\{c_m\}$ are
> **frozen** after Phase 1 (Axiom 3 stability); thresholds
> $\{\tau_m\}$ are **calibrated empirically** per modality, against
> a same-session-band floor measured on the reference set. Centroid
> stability is a property of the measurement geometry; threshold
> calibration is a tuning decision over the acceptance predicate.
> The two are independent.*

**Anchor 2 (§6.3 Case 3 methodology).**

> *The threshold $\tau = 0.95$ in Case 3 was set from the measured
> WavLM same-session-band floor on the 50-clip reference set; it is
> not a universal constant. Case 1 (text-style, H2c) and Case 2
> (visual + speaker, H2d) predict per-modality threshold bands
> before measurement.*

**Anchor 3 (§10 Future Work).**

> *RO-10 (sample-complexity curve) and agent-19 Q2 (false-positive /
> false-negative curve vs. $\tau$) together would characterise the
> threshold-calibration behaviour per modality; this paper fixes
> thresholds by measurement but does not produce the curves.*

**Tone.** V18 frozen discipline (on centroids); explicit calibration
language (on thresholds); no confusion between the two.

---

### IC-5-RI — Best-of-K is inference-time selection (not centroid recursion)

**Anchor 1 (§5.3 guard definition, Phase 3 description).**

> *Phase 3 best-of-$K$ is an **inference-time selection** step.
> Among $K$ candidates drawn from the trained generator, we retain
> the one whose per-modality cosine traces satisfy $G_\tau$. The
> centroid $c_m$ is **not updated** with the selected candidate;
> centroids remain frozen per Phase 1. No generator-in-loop
> recursion is introduced at centroid-maintenance time.*

**Anchor 2 (§6.3 Case 3 methodology).**

> *The 50-clip centroid is built once from real author audio and is
> not re-computed after best-of-$K=12$ selection. The selected
> candidate is the pipeline's output; it does not feed back into
> reference or training data.*

**Anchor 3 (§8 Limitations L-06, new).**

> *The collapse-silence argument (§3.3) addresses the pipeline as
> specified (real-reference centroids, no centroid recursion). A
> variant that **re-fed** selected best-of-$K$ outputs into centroid
> re-computation would introduce a generator-in-loop at the
> centroid-maintenance level; such a variant is out of scope and
> would require P2's scope argument to be reassessed.*

**Tone.** V18 frozen discipline (centroid); explicit "inference-time
vs. training-time" distinction.

---

### IC-6-RI — Asymmetric release: protocol released, Case 2/3 artefacts withheld

**Anchor 1 (§1 opener, artefact scope sentence).**

> *This paper releases the full TCT protocol, evaluation scripts,
> manifest schema, and hyperparameter tables. Case 1 (Shakespeare
> LoRA) artefacts are released for byte-identical reproduction.
> Case 2 and Case 3 subject-specific artefacts (centroids, LoRA
> weights, reference clips, held-out clips) are **withheld** on
> consent and dual-use grounds (§Ethics). Cases 2 and 3 are therefore
> reproducible at the **protocol level** — same pipeline, same
> hyperparameters, applied to a consented subject's own data — not
> at the artefact level.*

**Anchor 2 (§5 TCT protocol description, release scope box).**

> *Release scope: (i) Protocol pseudocode, hyperparameters, manifest
> schema — public. (ii) Context Graph + ClipCannon source code —
> public. (iii) Case 1 Shakespeare corpus + LoRA weights — public
> (byte-identical reproduction). (iv) Case 2 raw footage, derived
> clips, LoRA weights, centroid .npz, FLAME fits — **withheld**.
> (v) Case 3 speaker corpus, centroid, reference + held-out clips —
> **withheld**.*

**Anchor 3 (§8 L-Reproducibility, paste-ready new limitation).**

> *L-Reproducibility. This paper makes an asymmetric release
> commitment. Methodology is fully released; Case 1 artefacts are
> fully released; Case 2 and Case 3 subject-specific artefacts are
> withheld on consent and dual-use grounds. "Reproducibility" in
> this paper therefore means **byte-identical for Case 1** and
> **protocol-level for Cases 2 and 3**. Any claim of "full
> reproducibility" or "open reproducibility" that does not carry
> this qualifier overstates the commitment.*

**Anchor 4 (§Ethics — already drafted by agent-31; this insert is a
cross-reference note).**

> *See §Ethics for the consent scope (Case 2) and the author-as-
> subject disclosure (Case 3) that govern the withholding decisions.
> See §Competing Interests for the commercial-affiliation disclosure
> (Teleox.ai, Leapable.ai).*

**Tone.** R3 pool-relative scope; FM-05 single-subject disclosure;
consent-revocation-ready framing.

---

## 2. External apparent-contradiction inserts

### EC-7-RI — Meaning compression is complementary, not competitive, with Delétang / Huang

**Anchor 1 (§3.1 meaning-compression ratio view, unit-analysis
paragraph).**

> *Four compression categories sit on distinct unit pairs at distinct
> pipeline stages:*
>
> | Category | Unit pair | Stage | Tradition |
> |---|---|---|---|
> | Bit-compression | bits / bits | Inference (on sequences) | Shannon |
> | Weight-compression | accuracy / parameters | Post-training (on models) | GPTQ, SparseGPT, distillation |
> | Activation-compression | memory / output-quality | Inference (on activations) | KV-cache quant., TurboQuant |
> | **Meaning-compression** | **structured-signals / raw-data-unit** | **Training-data preparation (on corpora)** | **DDA (this paper)** |
>
> *Delétang et al. (2024) and Huang et al. (2024) instantiate
> bit-compression on trained models at inference; meaning compression
> instantiates a distinct signal-density axis at training-data
> preparation. The two axes are complementary, not competing; MDL /
> rate-distortion (ISO-04) is the shared information-theoretic
> root.*

**Anchor 2 (§5.2 Relation to existing frameworks, compression-as-
intelligence subsection — already drafted by agent-21 §5.2, this
insert is a clarifying sentence).**

> *We EXT the compression-as-intelligence frame (CAT-EXT, agent-10).
> We do not claim Delétang / Huang are wrong, nor that bit-
> compression-at-inference is subsumed by signal-density-at-training;
> we claim the two are distinct measurement stages of the same
> information-theoretic tradition.*

**Anchor 3 (W8 language tightening).**

> *Where agent-21 §5.2 originally read "dimensionally distinct", we
> now say "**semantically distinct and measured at a different
> pipeline stage**". The unit forms (count / count for both) do not
> establish dimensional distinctness by themselves; the distinctness
> is in **what is being counted** and **when in the pipeline it is
> counted**.*

**Tone.** CAT-EXT explicit; no retraction-of-prior-work tone; MDL
shared root acknowledged.

---

### EC-8-RI — DDA is out of scope of Shumailov's theorems (not a new immunity result)

**Anchor 1 (§3.3 DDA scope argument, collapse-silent paragraph).**

> *The cited collapse theorems (Shumailov et al., 2024; Dohmatob et
> al., 2024; Alemohammad et al., 2023) establish that recursive
> training on generator outputs induces distributional drift. DDA's
> operational procedure does not satisfy the generator-in-loop
> precondition: by Axiom 1, each $\phi_m(x)$ is a frozen measurement
> of a real input $x \in D$; $D' = \{\phi_m(x), \rho_{jk}\}$ is a
> **decomposition** of $D$, **not synthesis** of new inputs. The
> precondition **fails**; the theorems are **silent on this
> configuration**. This is a **scope argument**, not a new
> independence-from-collapse theorem. We explicitly do **not** claim
> DDA cannot suffer analogous failure modes; we claim the cited
> collapse apparatus does not reach it.*

**Anchor 2 (§8 L-05, inherits agent-24 W9).**

> *H4 scope boundary. The collapse-silent claim addresses generator-
> in-loop recursion of the Shumailov-2024 form. Other forms of
> embedder-mediated data-selection feedback (e.g., using embedder
> scores to filter low-quality inputs from $D$) are out of scope of
> the specific cited theorems and out of scope of the claim as
> stated.*

**Anchor 3 (§10 Future Work, empirical falsification).**

> *A clean falsification test for H4: run two training regimes in
> parallel — (A) recursive self-distillation, (B) DDA-enriched
> supervision over a frozen corpus — and show collapse dynamics
> differ as P2 predicts. If the DDA regime exhibits Shumailov-style
> drift attributable to generator-in-loop mechanics, H4 is
> falsified.*

**Tone.** "Scope argument" discipline; no strong-immunity claim; R3
pool-relative.

---

### EC-9-RI — TCT and RLHF address different problem classes

**Anchor 1 (§5.1 scalar-vs-geometric contrast table).**

> | Axis | Scalar-reward alignment | Geometric-proximity alignment (TCT) |
> |---|---|---|
> | Aim | Learned scalar function $r_\psi$ | Frozen multi-modal constellation $C_P$ |
> | Instrument | Learned reward model | Frozen embedders $\Phi$ |
> | Metric | Scalar preference | Per-modality cosine |
> | Runtime check | None (typical) | Per-output guard $G_\tau$ |
> | Problem class | Preference alignment over open-ended generation | Identity lock with per-output verifiability |
>
> *The four rows define a **structural break** in paradigm (agent-10
> BRK-I), not a competitive comparison on scalar-reward benchmarks.*

**Anchor 2 (§5.3 guard definition, paradigm-scope sentence).**

> *We do **not** claim RLHF / DPO / Constitutional AI are wrong, nor
> that they fail their intended task. We claim TCT addresses a
> **different problem class** — identity-locked generation with per-
> output geometric verifiability against a frozen target — that
> scalar-reward alignment does not target. No RLHF benchmark
> comparison is reported or intended.*

**Anchor 3 (§7 Discussion, complementarity paragraph).**

> *Scalar-reward alignment and geometric-proximity alignment are
> complementary paradigms for complementary problems. A production
> system could deploy both: RLHF-style preference alignment for
> open-ended response quality, TCT-style identity / safety guards
> for per-output verifiable constraints. The paradigms compose; they
> do not compete.*

**Tone.** BRK-I declared on paradigm axis, not on benchmark axis; no
retraction tone; explicit complementarity.

---

### EC-10-RI — 0.961 SECS is a personalised-paradigm result (not apples-to-apples with VALL-E 2)

**Anchor 1 (§1 opener, on first mention of 0.961).**

> *Case 3 reports 0.961 mean WavLM SECS on 10 unseen sentences in a
> **personalised setting** (50-clip speaker reference centroid +
> TCT best-of-$K=12$ guard at $\tau=0.95$). Published zero-shot TTS
> "human-parity" numbers (e.g., Chen et al. 2024 VALL-E 2 at ~0.881
> WavLM-large-sv) are from a **different problem class** (no
> reference set; single-prompt conditioning). The numbers are not
> directly comparable.*

**Anchor 2 (§6.3 Case 3 headline, paradigm-scope disclosure).**

> *The 0.961 result is a **protocol-level demonstration that the
> personalised paradigm admits identity agreement above the
> published zero-shot-TTS plateau band**. It is **not** a claim that
> TCT outperforms VALL-E 2 on VALL-E 2's task. Encoder difference
> (WavLM-base-plus-sv here vs. WavLM-large-sv in VALL-E 2) is part
> of the encoder-matched W1 caveat (§5.3, §6.3). Test-set
> differences (author-selected sentences vs. LibriSpeech test-clean)
> are part of the self-reported sentence-selection caveat (agent-14
> Bias-08, agent-29 VT-I1).*

**Anchor 3 (§9 Conclusion, non-competitive framing).**

> *We do **not** frame the 0.961 result as "exceeding VALL-E 2". We
> frame it as "a protocol-level demonstration of a different
> paradigm — personalised, per-output-verifiable identity-locked
> generation — that reaches identity-agreement levels published as
> 'human parity' in zero-shot settings, while acknowledging that the
> paradigm difference makes the headline numbers incommensurable".*

**Tone.** Bias-06, Bias-08, Theme D, VT-I1 all carried; no apples-to-
apples claim; closed-artefact caveat for VALL-E 2 inherits from
agent-13 Flag 1.

---

### EC-11-RI — Commercial affiliation is disclosed, not promoted

**Anchor 1 (page-1 footnote — agent-31 paste-ready, verbatim).**

> *Author affiliations: Teleox.ai (primary); Leapable.ai (companion
> marketplace). The author holds equity in both entities. Context
> Graph and ClipCannon are open-source research systems developed in
> the course of this work; no vendor product is cited, endorsed, or
> evaluated in this paper. See §Competing Interests for full
> disclosure.*

**Anchor 2 (§4 Systems description, framing sentence).**

> *Context Graph and ClipCannon are described here as **open-source
> research systems** (source code released; see §8 L-Reproducibility
> for artefact scope). They are not positioned as commercial
> products for the purposes of this paper; the relevant commercial
> context is disclosed in §Competing Interests.*

**Anchor 3 (§9 Conclusion, non-commercial framing).**

> *We frame the contribution as a **research methodology** and its
> **empirical instantiation**, not as a product announcement. Future
> work (watermarking, cryptographic provenance, cross-encoder
> detection) is named in §10 as research deliverables.*

**Tone.** FM-01 compliant; no promotional vocabulary; methodology
framing.

**Regex scan (agent-42 gate).** Run FM-01 regex from agent-31
§Competing Interests §3 on §1–§9 body text. Zero matches required
before submission lock.

---

## 3. Global style discipline applied by all inserts

- **V11**: every use of "independent" / "orthogonal" carries the
  "approximately independent; strict formal orthogonality is an open
  question" hedge.
- **V18**: every use of "frozen" is explicit (never ambiguous
  "pretrained").
- **V22**: every empirical number carries its evidence tier (T1
  MEASURED / T2 CONSTRUCTIVE / T3 PROJECTED / T4 DERIVED).
- **R3**: every "no cited method" clause is pool-relative to the
  LOCKED 88-pool.
- **R4**: measured-vs-projected language at clause level.
- **MP-12**: composition-not-invention on all novelty claims.
- **FM-01**: zero marketing vocabulary in body; commercial context
  in §Competing Interests + §Acknowledgements only.
- **FM-04**: measured/projected numbers flagged at every use.
- **FM-05**: single-subject (Cases 2, 3) disclosure at first mention.
- **FM-09**: vocabulary drift avoided; construct names per
  agent-02 glossary.

---

## 4. Cross-reference index

| Insert | Register entry | Weakness reg. | Ethics | Theme | Bias |
|---|---|---|---|---|---|
| IC-1-RI | IC-1 | — | — | agent-17 Theme E | — |
| IC-2-RI | IC-2 | W3 | — | agent-17 Theme F | Bias-05 |
| IC-3-RI | IC-3 | W1 | agent-31 §Ethics | Theme D | Bias-06 |
| IC-4-RI | IC-4 | — | — | — | — |
| IC-5-RI | IC-5 | W9 | — | Theme A | — |
| IC-6-RI | IC-6 | — | agent-31 §Ethics | — | — |
| EC-7-RI | EC-7 | W8 | — | Theme 3 | — |
| EC-8-RI | EC-8 | W9 | — | Theme 2 | — |
| EC-9-RI | EC-9 | — | — | Theme 7 | — |
| EC-10-RI | EC-10 | W1 | — | Theme 5 | Bias-08 |
| EC-11-RI | EC-11 | — | agent-31 §CI | — | Bias-03 |

---

## 5. Confidence

**HIGH** that every insert is paste-ready, construct-consistent, and
hedge-consistent.

**HIGH** that no insert introduces a new primitive or citation
(MP-12, R3 compliance).

**HIGH** that every insert has a specific section anchor and a
specific contradiction-register entry it resolves.

**MEDIUM** on EC-11-RI: the insert is correct in substance, but
FM-01 regex pass at agent-42 is the gating test; if the regex finds
matches in other body prose, additional rewrites are required
beyond this insert.

## Changelog

- 2026-04-16 — Initial issue by agent-32-contradiction-analyzer.
