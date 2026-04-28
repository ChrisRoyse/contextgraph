---
namespace: patterns
agent_id: agent-34-confidence-quantifier
timestamp: 2026-04-17T08:30:00Z
status: active
locked: false
supersedes: none
tags: confidence, calibration, per-claim, open-philanthropy-style, softening-applied, pre-experiment
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md
  - memory/discoveries/agent-33-falsification-agent--counterarguments.md
  - memory/patterns/agent-33-falsification-agent--claim-softening-recommendations.md
  - memory/handoff/agent-33-to-34--falsification-ready.md
downstream_consumers:
  - agent-35-introduction-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
answers_questions: [Q15, Q20]
---

# Per-Claim Confidence Table — Calibrated Probability Estimates

## 0. Purpose and scope

This file assigns a calibrated Open-Philanthropy-style confidence
level to each major claim the manuscript intends to defend, under
the explicit assumption that the two MUST-softenings from
agent-33 (recommendations 1 and 2) are applied and that the CRITICAL
weaknesses W1, W2, VT-I2, VT-I3, IC-2, IC-3 are disclosed in §6.3
and §8 per agent-32 / agent-33 paste-ready inserts.

## 1. Calibration vocabulary (locked for this paper)

| Band | Verbal handle | Usage rule |
|---|---|---|
| $\geq 95\%$ | "virtually certain" | reserved for derivations under axioms + measured T1 anchors |
| $\sim 85\%$ | "highly confident" | strong theoretical argument + partial empirical support |
| $\sim 70\%$ | "confident" | sound argument; open empirical questions acknowledged |
| $\sim 50\%$ | "roughly even odds" | plausible projection; no current T1 evidence |
| $\sim 30\%$ | "leaning against" | weak support; contrary evidence plausible |
| "can't tell" | agnostic | insufficient information for any point estimate |

**Priors.** Starting points are agent-23's locked confidences; I apply
pressure from CA-1 …CA-6 (agent-33), W1–W10 (agent-24), VT-* (agent-29),
and IC-* / EC-* (agent-32); I apply mitigation credit for the six
paste-ready softenings (2 MUST, 3 SHOULD, 1 MAY).

**Assumption.** All MUST-softenings are applied in the draft. If they
are not, the corresponding confidences drop by 10–20 pp each (CA-1
and CA-3 become near-certain review-kill vectors).

---

## 2. Claim-by-claim confidence table

### 2.1 Theoretical / constructive claims

| ID | Claim (paraphrased) | Pre-softening | **Post-softening** | Language band |
|---|---|---|---|---|
| **C1** | DDA yields **up to** $n(N + \binom{N}{2})$ orthogonal signals per input | 95% | **≥95%** | "virtually certain" |
| **C2** | Meaning compression is a distinct (fourth) compression category, with its own unit (signals / raw-data-unit) | 88% | **~85%** | "highly confident" |
| **C3** | TCT Phase-3 guard provides **per-output verifiability** against the target encoder panel | 92% | **~90%** | "highly confident" (close to "virtually certain") |
| **C4** | DDA pipelines are **structurally exempt** from Shumailov-style recursive-collapse | 90% | **~85%** | "highly confident" |

#### C1 — "Up to $n(N+\binom{N}{2})$ signals" (≥95%)

**Reasoning.** Under Axiom 1 (deterministic frozen $\phi_m$), the
count is a direct combinatorial identity: $N$ individual embedder
signals plus $\binom{N}{2}$ ordered pairs. The softening
("up to …" — agent-33 rec. 1 MUST) reframes the number as a
**structural upper bound**; the upper-bound reading is tautological
once Axiom 1 holds, and Axiom 1 is empirically auditable in ≤1 hour
of inspection on Context Graph. **Nothing an adversarial reviewer
could produce would falsify the upper bound**; only the *tight*
count (which is no longer being claimed) is at risk from RO-8 / W5 /
CA-1. Residual 5% covers the chance that the "frozen" discipline is
silently violated in practice (V18 hedge).

**Justification of language.** "Virtually certain" is warranted
because: (a) derivation is elementary combinatorics; (b) Axiom 1 is
observable; (c) the softened statement cannot be proven less than
$N+\binom{N}{2}$ on any instance where Axiom 1 holds.

#### C2 — Meaning compression as a distinct category (~85%)

**Reasoning.** Pre-softening agent-23 gave 88% resting on a
"dimensionally distinct" claim that W8 correctly calls an
overclaim: bit-compression is count/count and meaning-compression is
also count/count; they are dimensionally the same form. Agent-33
rec. 5 (SHOULD) demands a downgrade to "semantically distinct" /
"measured as structured-signal density rather than Shannon bits".
Once the claim is softened to **semantic** distinctness, the
unit-analytic argument carries: the three classical categories
trade bits, parameters, and memory; meaning compression trades
structured-signal density per raw input. The categories are not
linearly rescalings of one another. Residual 15% covers reviewer
pushback on whether "semantic distinctness" is sufficient to warrant
a **new** taxonomic entry vs. simply a new ratio within existing
bit-compression (Shannon) — a legitimate scholarly disagreement.

**Justification of language.** "Highly confident" is warranted
because the taxonomy argument is sound under the softened wording;
the reviewer-acceptance risk prevents "virtually certain".

#### C3 — TCT Phase-3 per-output verifiability (~90%)

**Reasoning.** Under Axiom 1 (frozen $\phi_m$, frozen $c_m$, fixed
$\tau$), the guard $G_\tau$ is a deterministic, cheap, per-output
geometric test ($O(M \cdot d_{\max})$). Per-output verifiability is
**structurally true** by construction — there is nothing to doubt
about the mathematics. The 10% residual is entirely reviewer
lexicography: (a) CA-2's challenge that "verifiable" in the formal
sense requires soundness/completeness against an independent spec;
(b) W10 GPU fp nondeterminism near the threshold (pedantic but
cheap to footnote); (c) scope risk that readers conflate
"verifiable against the chosen centroid panel" with "verifiable as
general identity", which is a different and much stronger claim.
Agent-33 rec. 3 (SHOULD) tightens scope to "verifiable **relative
to the chosen centroid panel**"; with that tightening the claim
stands at ~90%.

**Justification of language.** "Highly confident / close to
virtually certain" — the mathematical structure is airtight; only
the word-choice around "verifiable" carries any risk.

#### C4 — Shumailov-exempt by construction (~85%)

**Reasoning.** H4 is a **scope argument**, not a new theorem:
the published collapse theorems (Shumailov 2024; Dohmatob 2024;
Alemohammad 2023) all require generator-in-loop recursion $D_{t+1}
\ni \tilde x \sim p_{\theta_t}$. DDA has no such recursion by
construction (Axiom 1: $\phi_m$ is a frozen measurement of real $x
\in D$). The argument is clean. W9 (MEDIUM) correctly points out
that embedder-mediated *curation* (e.g., Context Graph filtering
out low-quality memories based on $\phi$ scores) is not strictly
"fixed $D$" and should be disclosed as out-of-scope of the
Shumailov theorems rather than covered by H4. With W9 scope
tightening (agent-33 rec. 4 MAY; agent-29 recommends apply), the
claim stands at ~85%. Residual 15% covers (a) embedder-curation
boundary noise; (b) the possibility that a generalised collapse
theorem covering non-generator-in-loop feedbacks will be published
and catch DDA; (c) any silent violation of "frozen" discipline
(V18).

**Justification of language.** "Highly confident" — the scope
argument is correct; the residual risk is scholarly and boundary-
delineation rather than any flaw in the argument itself.

---

### 2.2 Empirical claims — Case 3 (measured, T1)

| ID | Claim | Pre-softening | **Post-softening** | Language band |
|---|---|---|---|---|
| **C5** | Pipeline achieves **0.961 mean WavLM SECS** on 10 novel sentences | 98% | **≥98%** | "virtually certain" |
| **C6** | Clone is DNSMOS-P.808 indistinguishable from real speech for THIS speaker | 92% | **~85%** | "highly confident" |
| **C7** | Temperature $T = 0.3$ is optimal under the WavLM-SECS objective for this pipeline/speaker | 88% | **~75%** | "confident" (narrower than pre-soft) |
| **C8** | 50-clip centroid outperforms 1-clip centroid by +0.015 mean | 87% | **~85%** | "highly confident" |

#### C5 — 0.961 mean WavLM SECS (≥98%)

**Reasoning.** The number is **measured** and reported in the SITREP
2026-04-16 and the white paper. Within the encoder-matched scope
(W1 / IC-3 / VT-I3 CRITICAL), the number is exactly what it is: the
post-selection mean WavLM cosine on the 10-sentence held-out set.
The 2% residual covers data-entry / transcription errors and the
small chance the raw `.wav` artefacts were mis-joined to the wrong
scorer output. As a **within-WavLM-family identity-agreement
measurement**, this is virtually certain.

**Critical note.** The confidence applies to the **encoder-matched
measurement**, not to "cross-encoder identity quality" (W1). The
headline must be paired with agent-32 IC-3 / agent-33 rec. 2
implicit-scope language: "0.961 within-WavLM-family identity
agreement; cross-encoder triangulation (ECAPA-TDNN) is EXP-1
pending" (agent-33 EXP-1). Without that scope, C5 drops to ~60%
because a reviewer will (rightly) refuse to treat the number as
evidence of a different claim than the one the apparatus supports.

**Justification of language.** "Virtually certain" — for the
encoder-matched claim as stated. For any broader identity-quality
interpretation, use C5-B: "~70% confident it reflects genuine
cross-encoder identity quality pending EXP-1".

#### C6 — DNSMOS-P.808 parity with real speech for this speaker (~85%)

**Reasoning.** Measured: DNSMOS P.808 = 3.93 (clone) vs. 3.93
(real), N=14 sentences. The parity number is robust at the point
estimate. The reduction from 92% → 85% reflects agent-14 Bias-05 /
Bias-06 and agent-29 VT-S4 / VT-C: **N=14 is small for a
naturalness comparison**, the speaker pool is 1, and P.808 is a
noisy scalar with wide within-clip variance. "Indistinguishable"
is a scope-tightening phrasing that arXiv reviewers will probe; at
N=14, the 95% CI around a mean difference of 0.00 on a 1–5 scale is
roughly ±0.15 — wide. The claim survives ("within noise") but
should not be stated as a formal statistical-equivalence test.
Agent-33 rec. (not explicit on C6) and agent-32 IC-5 scope language
apply.

**Justification of language.** "Highly confident" — the parity is
empirically real at the point estimate; the language
"indistinguishable" is well-supported within the narrow evaluator
(P.808) but should be accompanied by the N=14 caveat.

#### C7 — $T = 0.3$ optimal (~75%)

**Reasoning.** Observed on a temperature sweep where $T \in \{0.1,
0.3, 0.5, 0.7, 0.9\}$ (forensic trail in alldata.md). $T=0.3$
yielded the highest mean post-selection SECS. Two risks reduce
confidence from 88% to 75%: (a) the optimum is **objective-
specific** (WavLM-SECS scored via best-of-K — the guard's preferred
temperature for argmax); (b) **generalisation risk** (the optimum
may not transfer to other speakers, other text corpora, or other
TTS backends). Agent-29 VT-E3 (transportability) and agent-14
Bias-05 apply. The claim survives as "we found $T=0.3$ optimal in
this configuration" but not as "T=0.3 is the right temperature
for voice-cloning in general".

**Justification of language.** "Confident" — the local optimum is
measured; the generalisation question is open and should be hedged
in-text ("…on this pipeline, this speaker, this corpus…").

#### C8 — 50-clip centroid beats 1-clip centroid by +0.015 (~85%)

**Reasoning.** Measured delta. Small effect size (+0.015 absolute
SECS) but consistently across the 10-sentence held-out set. The
87% → 85% drop reflects VT-S3 (small effect size — the delta is
within a small multiple of the within-sample WavLM-cosine sd) and
W3 (single operating point on the $n_m$ curve — a 10-clip or
25-clip point is not measured, so we cannot confirm the delta is
monotone in clip count within the $[1, 50]$ range). A reviewer will
probe whether +0.015 is statistically distinguishable from noise;
at N=10 held-out sentences with paired comparisons, a 0.015 shift
with per-sentence sd ~0.02 yields a t-stat of ~2–3, which is
significant but modest. Agent-28 §4.5 reports this delta in the
ablation table as a validated row.

**Justification of language.** "Highly confident" — the delta is
real at the point estimate and is paired (reducing noise); the
generalisation and $n_m$-curve questions are open.

---

### 2.3 Empirical claims — Case 2 (projected, T3)

| ID | Claim | Pre-softening | **Post-softening** | Language band |
|---|---|---|---|---|
| **C9** | Constellation-guard ON vs OFF materially affects identity-cosine stability | 50% | **~50%** | "roughly even odds" |
| **C10** | Mouth-weighted loss is necessary for lip-sync accuracy | 60% | **~55%** | "roughly even odds (slight lean yes)" |

#### C9 — Guard ON vs OFF affects identity stability (~50%)

**Reasoning.** **No T1 data yet** on Case 2 (constellation
fine-tuning over 2,362 clips). The hypothesis is structurally
plausible (Phase-3 selection should improve identity stability
vs. unguarded generation) but is anchored only by (a) the Case-3
single-modality analogue (where guard does select high-cosine
survivors by construction, but the *counterfactual* is not
measured — see W2 / IC-2), and (b) theoretical prediction under
Axiom 2. The claim is a specific quantitative prediction at $M > 1$,
and we have zero $M > 1$ empirical data (W7 HIGH). Even the
direction of the effect is plausible but not certain: under
severely redundant modalities (the H2b failure mode), guard
ON/OFF could produce indistinguishable identity-cosine stability.

**Justification of language.** "Roughly even odds" — genuinely
uncertain; this is exactly the prediction EXP-3 Arm B would
adjudicate.

#### C10 — Mouth-weighted loss necessary for lip-sync (~55%)

**Reasoning.** The forensic finding (flow-matching sign-bug in
Santa project; `docs/inspect/08_training_audit.md`) is consistent
with mouth-weighted loss mattering: the bug specifically corrupted
mouth-region gradients and, once fixed, lip-sync qualitative
quality improved. But "consistent with" is **not** "demonstrates
necessity", and we have no ablation comparing (mouth-weighted loss
ON) vs. (uniform loss + sign correct). Agent-29 VT-I4 (historical
comparison confound) applies directly: any between-version change
besides mouth weighting (LoRA rank, batch size, dataset version)
could be the cause of the improvement. The hypothesis is slightly
more likely than not but the path from current evidence to
"necessary" is long. Agent-33 (no explicit rec. on C10) and agent-29
L-04-style disclosure applies.

**Justification of language.** "Roughly even odds (slight lean yes)"
— the forensic finding points in the right direction, but
necessity is not supported by the current record.

---

### 2.4 Empirical claims — Case 1 (pending, T3)

| ID | Claim | Pre-softening | **Post-softening** | Language band |
|---|---|---|---|---|
| **C11** | Three-layer enforcement stack passes **15/15 FSV** after retrain on cleaned data | 65% | **~60%** | "roughly even odds (lean yes)" |

#### C11 — 15/15 FSV on cleaned retrain (~60%)

**Reasoning.** One prior run of the Shakespeare LoRA pipeline
passed 15/15 FSV (mission brief Case 1 status note). The retrain on
cleaned data (training-data echo-bug fixed) is **structurally
similar** but is a new run with a different dataset version.
Outcomes from one prior run do not transfer with certainty to a
new training run; in ML, retrain-variance is non-trivial. Factors
that support a lean-positive estimate: (a) the prior failure mode
is now understood and patched (the echo-bug was forensically
diagnosed); (b) the enforcement stack architecture is unchanged; (c)
the prior run did pass, so the pipeline is not obviously broken.
Factors that push toward uncertainty: (d) 15/15 is a binary
success criterion and even a 90% per-check pass rate would yield a
21% chance of 15/15; (e) cleaned-data effects on training
convergence are empirically variable; (f) FSV criteria are
strict by design.

**Justification of language.** "Roughly even odds (lean yes)" —
the retrain is plausible to pass but is far from certain; a
reviewer demanding "show the new FSV output" is a fair demand.

---

### 2.5 Strong / contested claims (per agent-33)

| ID | Claim | Constructive / empirical | **Post-softening** | Language band |
|---|---|---|---|---|
| **C12a** | Sample-complexity collapse **as a theorem** (constructive) | 95% | **≥95%** | "virtually certain" |
| **C12b** | Sample-complexity collapse **as an empirical claim** across modalities | 40% | **~40%** | "leaning against (for the strong phrasing)" |
| **C13** | TCT provides per-output verifiability **that scalar-reward alignment cannot match** | 80% | **~80%** | "highly confident" (with scope-tightening) |
| **C14** | The paper is a **research contribution**, not a marketing piece | 80% | **~80%** | "highly confident" (CONDITIONAL on FM-01 regex pass) |

#### C12a / C12b — The split claim (~95% constructive / ~40% empirical)

**Reasoning — constructive side (C12a).** The theorem is a direct
consequence of Axioms 1, 2, 3 and TCT Phase 1/2/3 structure: any
pipeline that freezes $\phi_m$, constructs $c_m$ from $n_m$
references, and selects via geometric guard satisfies $\mathcal{T}
= O(M \cdot k)$ by construction. There is **no empirical question**
about the constructive theorem; it is either a valid derivation or
an invalid one, and agent-21's proof sketch is valid under the
stated axioms. 95% residual covers any silent axiom-violation in
production.

**Reasoning — empirical side (C12b).** The **measured** side of
the claim rests on **one T1 operating point** ($M=1$, $k=50$, Case
3) that is simultaneously implicated in the encoder-matched
problem (W1) and the LoRA-only ablation gap (W2). Without the
experiments (EXP-3 Arm A: $n_m$ sweep on Case 3; EXP-3 Arm B:
pre-selection SECS distribution), we cannot claim the empirical
collapse is supported across $M > 1$ or across $n_m$ values
genuinely different from 50. Agent-33 CA-3 is INSUFFICIENT and rec.
2 (MUST) mandates the constructive/empirical split. Post-softening,
the empirical side is honestly estimated at ~40% — plausible but
not supported by current measurement. **EXP-3 Arm A strengthening
would lift C12b to ~70%**.

**Justification of language.** C12a: "virtually certain" (the
theorem is a derivation). C12b: "leaning against (for the strong
phrasing)" — note this is leaning *against* the unsoftened
"collapse across modalities" framing, not against the softened
"empirically supported at one T1 point" framing (which would be
≥90%).

#### C13 — TCT beats scalar-reward on per-output verifiability (~80%)

**Reasoning.** Per-output verifiability is a real and well-defined
property (Theorem 2; C3 above). Scalar-reward methods (RLHF / DPO /
Constitutional AI) do **not** provide per-output verifiability in
the same sense: a reward model produces a scalar, and the only
verification is "did we take the high-reward action?", which is a
training-time question not a test-time one. The claim is correct
**as stated**. The 20% residual covers: (a) a reviewer's objection
that comparing geometric verifiability to scalar verifiability is
a category error (agent-23 §9 disclaims this framing to be fair);
(b) the possibility of hybrid methods (reward + geometric checks)
that do provide per-output verifiability — in which case our
novelty is pool-relative not absolute; (c) the R3 pool-relative
scope we have explicitly committed to. Agent-33 CA-2 is
SUFFICIENT with scope tightening; agent-33 rec. 3 (SHOULD) applies
the tightening. With scope tightening applied, the claim is
defensible at ~80%.

**Justification of language.** "Highly confident" — the claim is
sound under the pool-relative scope and under the "per-output
verifiability" technical reading. The 20% residual is
reviewer-interpretation risk, not claim risk.

#### C14 — Research contribution, not marketing (~80%, CONDITIONAL)

**Reasoning.** The pre-softening 80% from agent-33 CA-6
SUFFICIENT is carried forward **conditional on FM-01 regex pass at
agent-42**. FM-01 checks the paper body for vendor/product-promotion
prose that would trigger a "marketing piece" rejection pattern.
Agent-31 (Ethics) already constructed the CI and Ethics statements
to mitigate this. If FM-01 passes, C14 sits at ~80%; if FM-01
fails, C14 drops to ~50% and agent-33 rec. 6 (SHOULD → MUST) gets
upgraded to MUST — i.e., targeted rewriting of any offending prose.

**Justification of language.** "Highly confident (CONDITIONAL)" —
the structural safeguards (agent-31 Ethics + CI statements,
agent-32 EC-11, agent-33 CA-6 response) are in place; the
conditional handles the FM-01 uncertainty.

---

## 3. Paper-level confidence (overall)

**Overall, we are ~78% confident the paper is arXiv-submittable
under the current draft plus PENDING remediations** (agent-33
softenings 1 and 2 MUST; 3, 4, 5 SHOULD; 6 as needed per FM-01).

### Decomposition of the 78%

- **Theoretical spine** (C1, C3, C4, C12a): ~92% average — very strong.
- **Empirical spine (Case 3)** (C5–C8): ~86% average — strong under
  encoder-matched scope, weakens to ~70% if W1/W2 disclosures are
  omitted.
- **Projected cases (C9–C11, C12b, H2c/H2d/H3b)**: ~50–65% average —
  honest T3 projections, all flagged as such.
- **Meta / framing** (C2, C13, C14): ~82% average — depends on
  reviewer lexicography and FM-01 pass.

### Scenarios

| Scenario | Paper-submittability confidence |
|---|---|
| Current draft, softenings NOT applied | **~55%** (CA-1 and CA-3 become review-kill vectors) |
| Current draft, softenings applied, no new experiments | **~78%** (this is the baseline) |
| Softenings applied + EXP-1 STRENGTHENS (cross-encoder SECS $\geq 0.85$) | **~88%** (triangulated empirical spine) |
| Softenings applied + EXP-3 Arm B STRENGTHENS (TCT-delta $\geq +0.05$ SECS) | **~88%** (TCT attribution cleaned up) |
| Softenings applied + EXP-1 and EXP-3 both STRENGTHEN | **~93%** ("virtually certain" band) |
| Softenings applied + EXP-1 KILLS (cross-encoder SECS $< 0.60$) | **~40%** (headline reframe required; H2a loses primary support) |
| Softenings applied + FM-01 regex FAILS | **~55%** (CA-6 blocks; targeted rewrite needed) |

### Lowest-confidence claims that must be softened most

1. **C12b (empirical sample-complexity collapse across modalities, ~40%)**
   — must be rewritten as "constructive theorem holds unconditionally;
   empirical collapse is anchored at one T1 operating point; broader
   empirical support is pending Case 1 and Case 2 T1 measurement".
   Agent-33 rec. 2 (MUST) is exactly this.
2. **C9 (Constellation-guard ON vs OFF effect on identity stability,
   ~50%)** — must be written as T3 projection, not as demonstrated fact.
   The paper already flags this as projected; the writer must not
   accidentally strengthen it in Case-2 prose.
3. **C11 (15/15 FSV on Case 1 retrain, ~60%)** — must be written as
   "the three-layer enforcement stack is validated in structure; the
   specific 15/15 outcome on the cleaned-data retrain is pending at
   the time of writing".
4. **C10 (mouth-weighted loss **necessary** for lip-sync, ~55%)** —
   must be written as "consistent with the forensic finding; an
   ablation isolating mouth-weighted loss from other between-version
   changes is pending (VT-I4)".
5. **C7 ($T=0.3$ optimal, ~75%)** — must be written as "optimal on this
   pipeline / speaker / corpus"; do not generalise.

### Highest-confidence claims that writers may keep strong

- **C1 "up to 91 signals/input"** — can be stated as a strong structural
  upper bound.
- **C3 "per-output verifiability relative to the chosen panel"** — can
  be stated confidently with the scope modifier.
- **C5 "0.961 mean WavLM SECS under encoder-matched protocol"** — can
  be stated as a measured fact with the W1 disclosure sentence.
- **C12a "sample-complexity-collapse theorem under Axioms 1–3"** — can
  be stated as a theorem.

---

## 4. Pre- vs. post-experiment confidence deltas

What changes if each experiment runs and resolves:

| Experiment | Primary claims affected | Pre-exp | If STRENGTHEN | If KILL |
|---|---|---|---|---|
| **EXP-1** (cross-encoder SECS: ECAPA-TDNN on same 10 held-out) | C5, C6 | 98% / 85% | 98% / 90% | 60% / 75% |
| **EXP-3 Arm A** ($n_m$ sweep: 10, 25, 50, 100) | C8, C12a, C12b | 85% / 95% / 40% | 90% / 95% / 70% | 75% / 95% / 25% |
| **EXP-3 Arm B** (pre-selection SECS distribution / TCT-delta) | C5, C13 | 98% / 80% | 98% / 88% | 80% / 65% |
| **EXP-2** (pairwise MI on 100–1000 inputs) | C1 | 95% | 97% ($N_{\text{eff}}$ replaces "up to 91") | 92% ($N_{\text{eff}} < N$ edges the claim but doesn't falsify it) |

**Maximum information gain** (reduces confidence-variance most):
EXP-1 (triangulates the headline), then EXP-3 Arm B (resolves TCT
attribution), then EXP-3 Arm A (curves the axiom), then EXP-2
(tightens the upper-bound to a measurement).

---

## 5. Confidence roll-up under reviewer personas (optional)

What a sceptical reviewer would assign under the softened draft:

| Persona | Per-claim average | Paper-submittability |
|---|---|---|
| **Sympathetic (arXiv cs.LG)** | 80% | ~85% |
| **Standard (NeurIPS mid-tier)** | 72% | ~75% |
| **Adversarial (ICLR / PhD-committee-style)** | 62% | ~65% |
| **Hostile (the steelmanned CA-6 reviewer)** | 50% | ~50% (kills on FM-01 or C12b) |

Median across personas: ~72–75%. This is consistent with the 78%
paper-level estimate in Section 3.

---

## 6. Carried hedges

All upstream hedges carry:
- **V11** on C1, C2, C12a, C12b (approximate independence).
- **V18** on all claims invoking $\phi_m$.
- **V22 / R4** on T1 vs T3 distinction (C5–C8 vs C9–C11).
- **R3** pool-relative on C13, C14.
- **Bias-05 / Bias-06** on C5, C6, C8 (single-speaker, English-only).
- **Theme D** on C5 (single-encoder-pair).
- **FM-01** on C14 (conditional).
- **W1 / W2 / W7** must be disclosed for C5–C8 confidences to hold.

---

## 7. Confidence in this confidence quantification

**HIGH** on internal consistency: every claim has an anchor in
agent-23, an adversarial pressure source, and a softening status.

**HIGH** on calibration against agent-33's red-team baseline: my
post-softening numbers agree with agent-33's recommended priors to
within ±5 pp on every claim, which is within the subjective
calibration noise of this exercise.

**MEDIUM** on the scenario bands in §3: experiment outcomes are by
definition uncertain, and my "if STRENGTHEN / if KILL" deltas rely
on implicit noise assumptions about ECAPA-TDNN vs. WavLM agreement
(EXP-1) and pre-selection SECS variance (EXP-3 Arm B). These bands
are indicative, not predictive.

**HIGH** that the five lowest-confidence claims (C12b, C9, C11, C10,
C7) are correctly identified as the ones requiring most aggressive
softening in the manuscript prose.

## Changelog

- 2026-04-17 — Initial issue by agent-34-confidence-quantifier.
