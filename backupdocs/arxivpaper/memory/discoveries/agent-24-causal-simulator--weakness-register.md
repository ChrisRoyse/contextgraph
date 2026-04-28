---
namespace: discoveries
agent_id: agent-24-causal-simulator
timestamp: 2026-04-17T07:20:00Z
status: active
locked: false
supersedes: none
tags: weaknesses, must-disclose, confounds, selection-effects, adversarial-exposure
depends_on:
  - memory/discoveries/agent-24-causal-simulator--counterfactual-analyses.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
downstream_consumers:
  - agent-25-method-designer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Weakness Register — Confirmed Weaknesses We Must Disclose

## 0. Purpose

This is the **must-disclose** list. Each entry is a weakness in the
paper's causal claims that (a) survived counterfactual stress
testing, (b) is not adequately excluded by current evidence, and (c)
will be found by an adversarial reviewer if we do not find it first.

Each entry has:
- **Severity**: CRITICAL / HIGH / MEDIUM / LOW.
- **Scope**: which hypotheses / sections it touches.
- **Disclosure venue**: where in the manuscript the weakness must be
  acknowledged.
- **Remediation**: what method-designer / methodology-writer must
  add, if anything.

---

## W1 — Encoder-matched evaluation on Case 3 (CRITICAL)

### Statement

The Case-3 pipeline uses **WavLM-base-plus-sv** as both the Phase-3
guard encoder (accept iff cosine $\geq 0.95$) and the held-out SECS
evaluator. Samples are explicitly **selected** for WavLM cosine
$\geq 0.95$ and then scored on WavLM cosine. The resulting mean SECS
= 0.961 is exactly the number the selection induces; it cannot serve
as independent evidence of cross-encoder identity quality.

### Why it survived stress testing

- The agent-17 Theme D "single-encoder-pair hedge" and agent-22
  note ("WavLM is outside E1–E13") acknowledge the architectural
  independence of WavLM from Context Graph's semantic panel.
- But **architectural independence from the retrieval panel is not
  the relevant independence**. The relevant independence is between
  the **guard encoder** and the **evaluator encoder**, and those
  are the same.
- No counterfactual scenario reverses this: any best-of-$K$ pipeline
  using WavLM as the guard produces WavLM SECS $\geq \tau$ on
  survivors by construction.

### Severity

**CRITICAL.** The paper's headline empirical result (0.961 mean
SECS, 9/10 $\geq 0.95$) is the signature of the guard working, not
cross-encoder identity validation. A reviewer will catch this
immediately.

### Scope

- **Hypotheses**: H2a, H3a.
- **Sections**: §6.3 Case 3 headline; §5.4 per-output predicate; §1
  opener (insofar as it cites the number).

### Disclosure venue

- §6.3 Case 3 must contain a **crisp single sentence** near the
  0.961 headline: "*The 0.961 mean SECS is an encoder-matched
  measurement: the guard and evaluator are both WavLM-base-plus-sv.
  This demonstrates the guard behaves as specified; it does not
  constitute cross-encoder identity verification.*"
- §8 Limitations must state this as L-01.

### Remediation

- **Mandatory**: agent-25 (method-designer) must add a cross-encoder
  evaluation (ECAPA-TDNN or Resemblyzer) on the same 10 held-out
  sentences. Report cross-encoder SECS alongside WavLM SECS.
- **Acceptable fallback if cross-encoder measurement cannot be
  added in time**: state the W1 disclosure as part of the Case-3
  headline and reframe H2a as "within-WavLM-family identity
  agreement, 0.961 mean, 9/10 $\geq 0.95$", not as a general
  identity-quality claim.

---

## W2 — Missing LoRA-only vs. LoRA+TCT ablation (CRITICAL)

### Statement

Case 3's pipeline is "identity-locked LoRA + best-of-$K=12$ TCT
Phase 3 with $\tau = 0.95$". We report the post-selection mean SECS
(0.961) but **not** the raw pre-selection SECS distribution of the
12 candidates.

Without this distribution, we cannot distinguish:
- **(a)** LoRA alone produces mean SECS $\approx 0.85$; TCT best-
  of-$K$ lifts to 0.961 (TCT contributes 0.11 absolute, meaningful).
- **(b)** LoRA alone produces mean SECS $\approx 0.95$; TCT best-
  of-$K$ lifts to 0.961 (TCT contributes 0.01 absolute, near-
  redundant).

Under scenario (b), the measured identity-lock and the claimed
sample-complexity collapse are **properties of the LoRA + 50 clips**,
not properties of TCT. The paper's causal attribution to TCT is then
overclaim.

### Why it survived stress testing

No ablation exists. The pipeline is run end-to-end and the summary
metric is reported. Every counterfactual simulation hits the same
wall: we cannot causally attribute the 0.961 to TCT rather than to
LoRA without the ablation.

### Severity

**CRITICAL.** This is the dominant confound for both H2a (guard
efficacy) and H3a (sample-complexity collapse). A reviewer trained
in ablation-discipline (which is essentially every reviewer) will
demand this.

### Scope

- **Hypotheses**: H2, H2a, H3, H3a.
- **Sections**: §6.3 Case 3; §5.4 identity-locked property
  derivation; §7 Discussion (sample-complexity collapse
  attribution); §1 opener if it attributes to TCT.

### Disclosure venue

- §6.3 Case 3 must either: (i) include the pre-selection SECS
  distribution and the TCT-delta, or (ii) disclose the absence of
  the ablation and caveat the attribution.
- §5.4 attribution must soften from "TCT causes" to "TCT verifies
  and selectively accepts".
- §7 Discussion must acknowledge the open attribution question.

### Remediation

- **Mandatory**: agent-25 must add to the Case-3 run list: report
  SECS histogram over all 12 best-of-$K$ candidates per held-out
  sentence, compute mean, compare to post-selection mean.
- **Stretch**: compare also to best-of-$K$ with $\tau = 0$ (no
  rejection) to separate best-of-$K$ sampling diversity from
  threshold rejection.

---

## W3 — Single operating point on the $n_m$ curve (HIGH)

### Statement

Axiom 3 asserts $n_m \in [10, 10^2]$ suffices. The evidence for this
axiom is a **single operating point**: $n_m = 50$ produces mean SECS
0.961 on Case 3. We have no data at $n_m = 10$, $n_m = 25$, $n_m =
100$, $n_m = 500$, etc. The curve shape is unknown; Theorem 1's
practical $k$ value is a single sample, not a range.

### Why it survived stress testing

RO-10 is explicitly named as future work by agent-19. No case study
varies $n_m$. The single point is the anchor.

### Severity

**HIGH.** Not critical because the theorem is constructive and the
axiom is hedged; the paper does not falsely claim a curve it does
not have. But a reviewer will note that the "$[10, 10^2]$ range"
language in Axiom 3 overstates what a single observation supports.

### Scope

- **Hypotheses**: H3, H3a, Axiom 3 directly.
- **Sections**: §3.6 sample-complexity framing; §5.4 Theorem 1;
  §6.3 Case 3; §8 Limitations; §10 Future Work.

### Disclosure venue

- §5.4 Theorem 1 statement must clarify: "Case 3 supplies the
  single T1 operating point at $k=50$ on one modality; the
  sample-complexity curve is RO-10 future work".
- §8 Limitations L-02: single operating point; no $n_m$ ablation.
- §10 Future Work: RO-10 lifted as a named deliverable.

### Remediation

- **Preferred**: agent-25 adds a minimal $n_m \in \{10, 25, 50,
  100\}$ ablation at Case 3. Even three additional points would
  change the claim from "single anchor" to "monotone curve".
- **Acceptable fallback**: disclose single-point limitation and
  carry RO-10 as explicit future work.

---

## W4 — Held-out-session diversity on Case 3 not audited (MEDIUM)

### Statement

The 50 reference clips and the 10 held-out sentences come from the
same 16-minute voice source. The manuscript does not specify
whether the held-out sentences come from:
- **(a)** the same recording session (same mic, same room, same
  day) as the reference clips — in which case acoustic artefacts
  are confounded with speaker identity.
- **(b)** a different recording session — which would rule out
  acoustic-environment confound.

### Why it survived stress testing

We have no disclosure either way. WavLM embeddings are somewhat
session-invariant in general, but at SECS $\geq 0.95$ we are in the
tail where small artefact correlations matter.

### Severity

**MEDIUM.** If held-out is same-session, the 0.961 is still a valid
measurement but it's partly measuring acoustic-environment agreement,
not purely speaker identity. If held-out is cross-session, this
concern is moot.

### Scope

- **Hypotheses**: H2a, H3a.
- **Sections**: §6.3 Case 3 methodology; §8 Limitations.

### Disclosure venue

- §6.3 Case 3 methodology must state the recording-session
  relationship between reference and held-out sets.
- §8 if same-session, disclose as L-03.

### Remediation

- **Mandatory at writing time**: methodology-writer must resolve
  and disclose the session relationship.
- **Preferred if cheap**: include at least one cross-session held-
  out clip for contrast.

---

## W5 — Axiom 2 pairwise MI unmeasured on Context Graph corpus (HIGH)

### Statement

The "91 signals/input" headline (H1a) assumes $\binom{13}{2} = 78$
pairwise interaction features carry non-redundant information.
RO-8 (empirical MI measurement across pairs) is explicitly future
work. On any real corpus, some embedder pairs (e.g., E1 semantic +
E10 paraphrase; E1 + E11 entity) may be substantially redundant.

### Why it survived stress testing

No measurement exists. The V11 hedge is carried linguistically
("approximately independent") but is not backed by data.

### Severity

**HIGH.** The "91 signals/input" number is one of the paper's
headline structural claims. If a reviewer demands "measure it", we
have nothing.

### Scope

- **Hypotheses**: H1, H1a, H1b (which carries the hedge).
- **Sections**: §3.4 DDA counting; §4.1 Context Graph specifics.

### Disclosure venue

- §3.4 headline must say "up to 91" not "91".
- §8 Limitations L-04 with RO-8 pointer.
- §10 Future Work RO-8 lifted.

### Remediation

- **Preferred**: run a small-scale RO-8 measurement — sample 100–1000
  inputs, compute pairwise MI estimates on embedder outputs, report
  redundancy histogram. Even a back-of-envelope measurement would
  dramatically strengthen H1a.
- **Acceptable fallback**: disclose as upper bound + future work.

---

## W6 — TCT causal attribution vs. embedder-conditioning (MEDIUM)

### Statement

The sample-complexity collapse (H3 / P5 / Theorem 1) is causally
attributed to **TCT operating on DDA-enriched data**. But the
non-trivial content reduces to Axiom 3: *modern frozen embedders are
well-conditioned enough that $n_m = 50$ suffices*. This is a
property of **the embedders**, not of **TCT**. TCT is the vehicle
that exploits embedder conditioning; it does not cause it.

The paper's current framing risks crediting TCT with what is
arguably inherited from the retrieval / contrastive geometry
literature that trained the embedders.

### Why it survived stress testing

Agent-21's Axiom 3 boundary condition already admits this: *"Axiom
3 is a claim about good conditioning of modern frozen embedders,
not a universal statement."* But this admission is buried in an
axiom hedge; the discussion section will read as if TCT caused the
sample-complexity collapse.

### Severity

**MEDIUM.** The claim is not wrong, but the attribution is at risk
of overclaim. A reviewer trained in "inheritance from prior work"
will demand that the embedder-conditioning literature be credited.

### Scope

- **Hypotheses**: H3, H3a, H3b, P5, Theorem 1.
- **Sections**: §3.6; §5.4; §7 Discussion; §9 Conclusion.

### Disclosure venue

- §7 Discussion must credit the frozen-embedder conditioning
  literature (cluster-D retrieval / contrastive geometry; any
  concentration-on-the-sphere analysis of dense encoders).
- §9 Conclusion must avoid "TCT causes sample-complexity collapse"
  in favour of "TCT operationalises the sample-complexity collapse
  implicit in well-conditioned frozen embedders".

### Remediation

- Rewrite attribution language in §3.6, §5.4, §7, §9.
- No additional measurement required.

---

## W7 — Multi-modal strictness claim has no $M>1$ evidence (HIGH)

### Statement

H2b ("multi-modal conjunction is strictly stronger than single-
modality check") is derivationally true under Axiom 2 at $M>1$, but
we have **zero measured data** at $M>1$. Case 3 is $M=1$. Case 2 and
Case 1 are T3-projected. The paper's claim that the multi-modal
conjunction is empirically stricter rests on no measurement.

Worse, the **sharp** claim (strict, not just weak, domination) can
fail if modalities are comonotonic at the threshold — i.e., the
same samples that pass modality-1 also pass modality-2. If so,
multi-modal check collapses to single-modal check in practice.

### Why it survived stress testing

No $M>1$ case study has produced measurements. The argument is
derivational. The paper should acknowledge that the strictness
claim is elementary probability under Axiom 2, not an empirical
finding.

### Severity

**HIGH.** This is the second-biggest open exposure after the
encoder-matched evaluation problem. A reviewer will point out that
the multi-modal paradigm is not evidenced by the paper's own data.

### Scope

- **Hypotheses**: H2b, H2d, H3b.
- **Sections**: §5.4; §6.2 Case 2 (projected); §7 Discussion.

### Disclosure venue

- §5.4 must state: multi-modal strictness is derivational; $M>1$
  empirical measurement is pending Case 2.
- §6.2 (Case 2 projected) must flag this as the critical pending
  measurement.
- §8 L-05.

### Remediation

- **Preferred**: if Case 2 partial measurements exist (even on a
  tiny subset), report them with appropriate hedging.
- **Acceptable fallback**: disclose as pending and flag as the
  measurement that would most strongly validate the multi-modal
  framing.

---

## W8 — H5 "dimensionally distinct" language is an overclaim (MEDIUM)

### Statement

H5 claims the meaning-compression unit ("signals / raw-data-unit")
is **dimensionally distinct** from the three classical compression
categories. Dimensional analysis: bit-compression is bits/bits
(count/count); meaning-compression is signals/raw-data-unit
(count/count). **Dimensionally, the two are the same form.** The
distinctness is semantic (counting different things — Shannon bits
vs. per-embedder structured signals) but not dimensional.

### Why it survived stress testing

Agent-21's §5.2 language "*dimensionally non-interconvertible*" is
not defended by the unit analysis it claims to rest on. This is a
load-bearing overclaim.

### Severity

**MEDIUM.** The taxonomic claim stands (meaning compression is a
semantic fourth category); the wording needs a targeted downgrade.

### Scope

- **Hypotheses**: H5.
- **Sections**: §3.1 meaning-compression ratio view; §5.2 relation
  to existing frameworks.

### Disclosure venue

- §5.2 language change: "dimensionally distinct" → "semantically
  distinct / measured as structured-signal density rather than
  Shannon bits".

### Remediation

- Targeted wording change in §3.1 and §5.2. No additional
  measurement.

---

## W9 — H4 scope boundary for embedder-mediated curation (MEDIUM)

### Statement

H4 argues DDA is collapse-silent because no generator-in-loop
recursion exists. True in the Shumailov-theorem sense. But if the
Context Graph ingestion pipeline uses embedder-derived signals to
filter low-quality memories from $D$, there is an **embedder-
mediated** data selection loop that, while not a generator-in-loop
recursion, is not strictly "fixed $D$". The scope argument should
explicitly state it addresses generator recursion, not all forms of
data-selection feedback.

### Why it survived stress testing

We have not audited whether Context Graph's ingestion uses embedder
scores for filtering. If it does, the "fixed $D$" picture is too
clean.

### Severity

**MEDIUM.** The cited collapse theorems do not apply to embedder-
mediated curation; the scope argument is technically correct. But
a broad reader may mis-read "collapse-silent" as "no feedback of
any kind", which is stronger than what we can defend.

### Scope

- **Hypotheses**: H4, P2.
- **Sections**: §3.3 DDA scope argument; §8 Limitations.

### Disclosure venue

- §3.3 must specify: the scope argument addresses **generator-in-
  loop recursion of the Shumailov-2024 form**. Other forms of
  embedder-mediated data selection are out of scope of those
  specific theorems.

### Remediation

- Language tightening in §3.3; optionally audit Context Graph's
  ingestion to confirm the "fixed $D$" boundary.

---

## W10 — Determinism of guard path not explicitly audited (LOW)

### Statement

Theorem 2 claims the guard $G_\tau$ produces a deterministic per-
output decision. True by construction (frozen $\phi$, frozen $c_m$,
fixed $\tau$, cosine comparison). But in practice, GPU floating-
point nondeterminism can cause tiny cosine fluctuations across
runs. At $\tau = 0.95$ with outputs clustering near the threshold,
this could in principle flip decisions.

### Why it survived stress testing

No explicit re-run audit has been reported. The construction is
deterministic modulo GPU fp noise.

### Severity

**LOW.** This is a pedantic point. A reviewer is unlikely to push
on it unless the paper overclaims strict determinism. Stating
"deterministic up to GPU fp noise" is cheap insurance.

### Scope

- **Hypotheses**: H2, Theorem 2.
- **Sections**: §5.3 guard definition.

### Disclosure venue

- §5.3 footnote: "*Deterministic modulo GPU floating-point
  reproducibility; the guard implementation uses identical
  parameters, inputs, and kernel order at both training and
  inference time.*"

### Remediation

- Add the footnote. Optionally run 10 replicates of the guard on
  the Case-3 held-out set to confirm decision stability and report
  as a sentence.

---

## Summary

| ID | Severity | Blocks publication? | Remediation cost |
|---|---|---|---|
| W1 | CRITICAL | No (with disclosure) / Yes (without) | Low (disclosure) + optional cross-encoder run |
| W2 | CRITICAL | No (with disclosure) / Yes (without) | Medium (run ablation) |
| W3 | HIGH | No (with disclosure) | Medium (run $n_m$ ablation) |
| W4 | MEDIUM | No | Low (disclosure) |
| W5 | HIGH | No | Low (disclosure) or Medium (small RO-8 run) |
| W6 | MEDIUM | No | Zero (language change) |
| W7 | HIGH | No | Zero (disclosure) |
| W8 | MEDIUM | No | Zero (language change) |
| W9 | MEDIUM | No | Zero (language tightening) |
| W10 | LOW | No | Zero (footnote) |

**Net**: No weakness on its own blocks publication. **Three
critical/high weaknesses (W1, W2, W7) compound** such that without
disclosure the empirical spine looks weaker than the derivational
spine. With disclosure in §6.3, §8, and remediation where cheap
(cross-encoder run, LoRA-only ablation, small $n_m$ ablation), the
paper's evidence-to-claim ratio is defensible.

**Priority for method-designer (agent-25)**:
1. Cross-encoder evaluation on Case 3 (remediates W1).
2. LoRA-only ablation on Case 3 (remediates W2).
3. Small $n_m$ ablation at Case 3 (remediates W3).
4. Held-out-session audit (remediates W4).
5. RO-8 mini-measurement (remediates W5).

Items 1–3 are the high-value-per-cost remediations. If agent-25 can
budget these, the paper's empirical spine is transformed from
"single anchor" to "triangulated anchor".

## Changelog

- 2026-04-17 — Initial issue by agent-24-causal-simulator.
