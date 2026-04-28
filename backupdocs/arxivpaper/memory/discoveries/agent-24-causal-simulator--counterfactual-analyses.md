---
namespace: discoveries
agent_id: agent-24-causal-simulator
timestamp: 2026-04-17T07:10:00Z
status: active
locked: false
supersedes: none
tags: counterfactuals, causal-stress-test, circular-reasoning, reverse-causality, spurious-correlation, h1, h2, h3, h4, h5
depends_on:
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-22-model-architect--master-architecture.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/patterns/agent-23-hypothesis-generator--evidence-map.md
  - memory/handoff/agent-23-to-24--hypotheses-ready.md
downstream_consumers:
  - agent-25-method-designer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Counterfactual Analyses — Per-Hypothesis Stress Tests

## 0. Scope and stance

For each locked hypothesis (H1 through H5 and their sub-hypotheses) we
ask six questions:

1. **Counterfactual.** Given the theoretical framework, what would the
   world look like if the hypothesis were **false**?
2. **Observable signature of falsity.** What would our case studies
   (Case 1 Shakespeare, Case 2 ClipCannon/Santa, Case 3 voice clone)
   measurably show if H were false?
3. **Do our measurements exclude the counterfactual?** If yes, how
   strongly; if no, what residual probability of the counterfactual
   survives.
4. **Circular reasoning check.** Does our defense of H depend on H
   itself — or on a construct that is numerically entangled with the
   measurement we use to defend it?
5. **Reverse causality check.** Could the causal arrow run backward —
   e.g., is TCT enabling the identity-locked generation, or is an
   identity-locked LoRA doing all the work and TCT just redundantly
   reporting what the LoRA already produced?
6. **Spurious correlation check.** Is there a third factor that
   explains the observation without the hypothesis being true?

We are deliberately brutal on **H2** (guard accept-rate) and **H3**
(sample-complexity collapse), because these are the claims the
manuscript leans on hardest, and they are the claims most likely to
survive adversarial review on weak grounds.

No new primitives. No new citations. Pool-relative R3 preserved.

---

## 1. H1 — DDA signal-count identity $\lvert D' \rvert = n(N + \binom{N}{2})$

### 1.1 Counterfactual

**H1 is false** iff the derived-dataset cardinality falls below the
structural upper bound $n(N + \binom{N}{2})$ by a substantial margin —
i.e., either (a) per-embedder projections are not all distinct
signals (the $n \cdot N$ count fails), or (b) pairwise interaction
features collapse to linear reparameterisations of single-embedder
projections (the $n \cdot \binom{N}{2}$ count fails).

Concretely: if only some embedders $\phi_m$ act as independent
measurement channels and the rest collapse to noisy copies of one
another, then $|D'|_{\text{effective}} \ll n \cdot 91$ for the
$N=13$ Context Graph instance.

### 1.2 Observable signature of falsity

- Step 2 of the H1 falsification harness (§1 of evidence-map) would
  find pairs of embedders with cosine correlation $> 0.95$ on random
  inputs — evidence of linear dependence.
- An MI-based estimator would reveal two or more pairs with $I/H >
  0.9$ (near-complete dependence).
- In practice this would show up as: throwing away half the $\Phi$
  panel does not reduce downstream retrieval / labelling quality
  proportionally.

### 1.3 Do our measurements exclude the counterfactual?

**Partially, with residual probability ~20%.** What we know:

- The Context Graph codebase runs 13 embedders with 15 HNSW indexes
  in production (MEMORY.md "13 embedders" fact); this establishes
  T2 instantiation — all 13 are operationally distinct at the
  infrastructure level.
- The embedders are trained on **different objectives** (E1
  e5-large-v2 semantic; E5 causal asymmetric; E6/E13 sparse
  keyword/SPLADE; E7 code-specific Qodo; E11 KEPLER entity; E12
  ColBERT late-interaction; E2/E3/E4 temporal) — this is prima facie
  evidence for Axiom 2 (approximate orthogonality).
- However, **we have not measured the $\binom{13}{2} = 78$
  pairwise MI values** (RO-8 is explicitly future work per
  agent-19).

**Residual weakness.** The gap between "architecturally distinct
embedders" and "mutually low-MI projections of the same input" is
exactly what V11 hedges. Some pairs may in fact be near-redundant on
specific corpora — e.g., E1 (e5-large-v2 semantic) and E10
(paraphrase) are both dense semantic embedders and could carry
substantially overlapping information on non-paraphrase-heavy inputs.
The manuscript **must state "up to 91 signals/input"** and not "91
signals/input" as a tight count.

### 1.4 Circular reasoning check

**PASS.** H1's derivation is purely combinatorial under Axioms 1 and
2 (count the projections, count the pairs). There is no step where
the defense of H1 invokes H1. Axiom 2 is the live premise — if it
fails, H1 degrades to H1b (upper bound only). The reasoning is not
circular; it is conditional.

### 1.5 Reverse causality check

**N/A structurally.** H1 is a counting identity — the direction of
"cause" is from the embedder panel $\Phi$ and corpus $D$ to the
derived signal count $|D'|$. There is no generator-in-loop and no
feedback from downstream to upstream signal counting.

### 1.6 Spurious correlation check

**One mild concern.** The "91 signals/input" headline could be
spuriously inflated if **effective information content per signal**
is very low for some of the 78 pair signals. That is, the number
of signals can be 91 while the effective bits-per-signal is much
less than the bits-per-signal of the $n \cdot N$ per-embedder
projections. The manuscript already carries H1b as a hedge, but the
reader may conflate "91 signals" with "91× more information", which
is not what the identity says. **Action: The paper must disambiguate
structural count from effective information content in §3.4.**

### 1.7 Verdict on H1

- **Counterfactual strength**: Moderate. Near-redundant embedders are
  plausible and untested. RO-8 measurement is a substantive open
  question.
- **Paper-level exposure**: Low as written (H1b hedge carries V11
  explicitly). Elevated if §3.4 drops "up to" and states "91" as a
  tight information lower bound.
- **Strongest attack a reviewer would mount**: "You never measured
  embedder-pair MI. Your $\binom{N}{2}$ term could be mostly
  redundant on your own corpus. Report effective-signal count."

### 1.8 H1a — Context Graph 91 signals/input

Same analysis as H1 at $N=13$. Residual probability ~20% that
effective count on the Context Graph corpus is substantially below
91 due to pair redundancy. **H1a survives** as a **structural upper
bound claim**, fails as a **tight count**.

### 1.9 H1b — Structural-upper-bound hedge

**PASS trivially.** H1b is the hedge itself — it is vacuously
supported. It cannot be falsified by discovering redundancy; its
only falsification path is finding that embedder pairs are
**zero-redundant** (which would make H1b uninformative but not
false).

---

## 2. H2 — Constellation guard accept-rate bound (HIGH SCRUTINY)

### 2.1 Counterfactual

**H2 is false** iff any of the following holds:

- **H2-F1.** The guard $G_\tau$ produces non-deterministic accept /
  reject decisions on fixed $(\hat y, C_P, \tau)$ (violates Axiom 1).
- **H2-F2.** The claimed "no auxiliary learned model" property fails
  — e.g., the guard implementation silently queries a learned
  reward head or a confidence calibrator to produce the accept
  decision.
- **H2-F3.** The multi-modal conjunction offers no meaningful
  acceptance strictness over single-modality checks on actually-
  observed data (H2b degenerate case: modalities are comonotonic at
  the threshold; Axiom 2 fails hard).
- **H2-F4.** The "strictly stronger than scalar reward" clause of
  P3 fails because a scalar-reward inference-time predicate can in
  principle be constructed that matches guard behaviour on the same
  outputs.

### 2.2 Observable signature of falsity

- **H2-F1.** Ten re-runs of $G_\tau$ on the same $\hat y$ give
  different accept / reject outcomes. Our measured Case-3 pipeline
  applies deterministic cosine + threshold comparison; a code audit
  should trivially confirm determinism.
- **H2-F2.** A code audit finds a learned model in the guard path.
  In the Case-3 pipeline, WavLM-base-plus-sv is frozen by design
  (it is a pretrained embedder, not trained on our data); the
  "learned model" clause requires a **generator-coupled** learned
  model, which is not present in WavLM inference.
- **H2-F3.** Case 2 (future) measures $p_\cap \approx \min_m p_m$
  within 1%. Per §6 of evidence-map this would falsify H2b and
  indirectly weaken H2.
- **H2-F4.** Any credible construction of a scalar-reward inference
  predicate that matches guard strictness on identity predicates.
  (This is the most subtle counterfactual and is discussed below.)

### 2.3 Do our measurements exclude the counterfactual?

**Partially, with important residual weakness.**

- **H2-F1 exclusion**: STRONG. The guard computation is pure cosine +
  threshold comparison on frozen $\phi$ and frozen $c_m$; determinism
  follows from construction. Residual: 1-2% (measurement noise in
  $\phi$ forward pass at different batch positions — non-zero but
  negligible).
- **H2-F2 exclusion**: STRONG at Case 3 (WavLM-base-plus-sv is a
  frozen pretrained public model not fine-tuned on our data).
  Residual: Case 2 / Case 1 use different encoders (CLIP, E1) — if
  any LoRA adapter were active during guard evaluation, H2-F2 would
  fire. The manuscript must confirm at Case 1/2 write time that no
  LoRA is active during guard evaluation.
- **H2-F3 exclusion**: NOT EXCLUDED. We have zero multi-modal
  empirical evidence. H2a at $M=1$ cannot disprove a comonotonic
  failure mode at $M>1$. **This is a genuine open exposure.**
- **H2-F4 exclusion**: PARTIALLY EXCLUDED by P3 derivation (scalar
  rewards in InstructGPT/DPO/CAI are Bradley-Terry identified up to
  monotone transformations, so no meaningful absolute threshold
  exists). Residual: a hypothetical pool-external method that
  commits to inference-time reward thresholding is not structurally
  excluded; R3 pool-relative hedge applies.

### 2.4 Circular reasoning check — **LOAD-BEARING WEAKNESS**

**This is the strongest weakness in the paper's causal chain for
Case 3.**

The Case-3 chain, spelled out:

1. We train a voice-cloning LoRA on 50 reference clips.
2. We do best-of-$K=12$ TCT Phase-3 rejection sampling using
   WavLM-base-plus-sv as the guard encoder with $\tau = 0.95$.
3. We **score** held-out generations using WavLM-base-plus-sv SECS.
4. Mean SECS is 0.961.

The guard and the evaluator are **the same encoder family**.
WavLM-base-plus-sv is the encoder; the guard takes WavLM-base-plus-sv
cosine $\geq 0.95$; the evaluation reports WavLM-base-plus-sv SECS.

**Counterfactual that we cannot exclude**: *Any* best-of-$K$ voice
pipeline that rejects on WavLM-base-plus-sv cosine $\geq 0.95$ will
produce held-out WavLM-base-plus-sv SECS $\geq 0.95$ on samples that
passed the guard. That is a **selection effect**: we rejected the
samples that would have scored below 0.95, therefore the surviving
samples score above 0.95.

This is not a falsification of H2 itself — H2 says the guard
**accepts** at the claimed conjunction. It is a **threat to the
evidential value** of Case-3 SECS numbers for H2a/H3a. The paper's
headline "0.961 mean SECS" is exactly the number that any
encoder-matched guard would produce by construction — it is the
signature of the guard working as claimed, not independent evidence
of identity quality.

**The manuscript already half-acknowledges this with agent-17 Theme D
(single-encoder-pair hedge) and agent-22 master-architecture §5
(WavLM is outside E1–E13)**, but the disclosure needs to be
**crisper**:

> *The Case-3 SECS numbers are encoder-matched: the guard and the
> evaluator are the same encoder. This demonstrates that the guard
> behaves as constructed; it does **not** independently verify
> identity quality in a cross-encoder sense. An
> architecturally-independent evaluator (ECAPA-TDNN, Resemblyzer)
> would be required to rule out the null hypothesis "any
> encoder-matched best-of-K pipeline with $\tau$ set to the
> threshold yields mean score at $\tau$+small margin".*

**This is not circular reasoning in the strict logical sense** —
the guard and the evaluator produce logically distinct objects
(accept/reject vs. continuous SECS on held-out data). But the
**evidential value** of the SECS number as independent proof of
identity quality is severely reduced, because the guard's selection
on WavLM cosine mechanically induces high held-out WavLM SECS by
selection on the outcome variable.

**Cross-encoder disclosure is not optional; it is the load-bearing
disclosure of the paper.**

### 2.5 Reverse causality check

Not strictly "reverse causality" in the statistical sense, but
relevant: **does TCT cause identity-locked generation, or does the
identity-locked LoRA cause both the TCT-accepting generations and
the high SECS, with TCT playing a purely confirmatory role?**

The Case-3 empirical evidence **cannot distinguish these two
hypotheses** because it reports the post-selection metric only, not
an ablation of best-of-K vs. best-of-1.

**Observable that would discriminate**:
- Take the same LoRA, generate $K=12$ candidates, report the SECS
  histogram of all 12 candidates (not just the argmax that passes
  the guard). If the LoRA alone produces mean SECS ≈ 0.85 and the
  guard lifts it to 0.96, TCT is doing real work. If the LoRA alone
  already produces mean SECS ≈ 0.95, TCT is near-redundant and H3a
  especially is at risk.

**We have no evidence that excludes the "LoRA is doing all the
work" hypothesis.** This is a serious under-disclosed confound for
H2, H2a, H3, and H3a simultaneously.

**Action**: The methodology section must report the **raw** (not
best-of-$K$) SECS distribution from the Case-3 LoRA, and compute the
**delta** between raw-mean and best-of-$K$-mean as the measured
contribution of TCT Phase 3. Without this, the paper cannot
causally attribute identity quality to TCT versus the LoRA.

### 2.6 Spurious correlation check

**Confound 1 — Encoder family self-agreement.** As spelled out in
§2.4, WavLM-base-plus-sv-guarded $\to$ WavLM-base-plus-sv-scored is
a strong within-family agreement effect. This is the dominant
spurious-correlation risk.

**Confound 2 — Test-set overlap with reference set.** The 50
reference clips and the 10 held-out sentences both come from a
16-minute voice source. If the held-out sentences are phonemically /
prosodically close to the reference clips (e.g., same recording
session, same mic, same room), the centroid $c_{\text{speaker}}$
and the test embeddings share acoustic-environment artefacts that
are not speaker identity. A cross-session held-out set (different
recording session, different device) would rule this out.
**Not currently excluded by Case 3 setup.**

**Confound 3 — DNSMOS / UTMOS parity is separate from identity.**
The paper reports DNSMOS=3.93 (parity real 3.93) and UTMOS=3.012
(vs. real 2.997). These are **naturalness** metrics, not identity
metrics. They do not help exclude Confound 1 or 2. The paper should
not let DNSMOS parity bolster the identity claim by proximity; they
are orthogonal.

### 2.7 Verdict on H2

- **H2 as a guard-construction claim (deterministic, no aux
  model)**: survives strongly.
- **H2 as a "strictly stronger than scalar reward" claim**:
  survives derivationally, pool-relative.
- **H2a as "mean SECS 0.961 is evidence of cross-encoder identity
  quality"**: does **not** survive without cross-encoder
  disclosure. This is the strongest weakness we must disclose.
- **H2b as "multi-modal strictly stronger"**: survives derivationally
  at $M=1$ trivially; genuinely open at $M>1$.
- **H2c, H2d as T3 predictions**: survive as predictions; T1
  measurement pending.

**Net**: H2 is partially open. The cross-encoder confound on Case 3
must be disclosed crisply in §6.3 and §8 (Limitations). The
"LoRA-vs-TCT ablation" is a methodological gap that agent-25
(method-designer) should add to the run list.

---

## 3. H3 — Sample-complexity collapse $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$ (HIGH SCRUTINY)

### 3.1 Counterfactual

**H3 is false** iff any of:

- **H3-F1.** The claimed $k = \max_m n_m \in [10, 10^2]$ bound fails
  — i.e., real centroid stability requires $k \gg 10^2$ on Case 3's
  voice modality.
- **H3-F2.** The $|D|$-independence claim fails — i.e., performance
  does depend measurably on the pretraining corpus size, so the
  "only $R_m$ is subject-specific" argument understates the real
  data dependency.
- **H3-F3.** The "O(M·k) independent of subject-specific fine-tuning
  data beyond $R_m$" claim is technically true but vacuous because
  the "LoRA training" on those same $R_m$ clips is doing the heavy
  lifting, making the sample-complexity collapse claim a restatement
  of "LoRA fine-tuning works with small data", not a novel property
  of TCT.

### 3.2 Observable signature of falsity

- **H3-F1**: vary $n_m \in \{10, 25, 50, 100, 200, 500, 1000\}$,
  measure SECS. Non-monotonic or flat-until-500 curves falsify
  Axiom 3's $[10, 10^2]$ claim. We have **no** such curve.
- **H3-F2**: truncate the pretraining corpus of the generator
  (Qwen3-TTS), re-run Case 3, measure SECS. If SECS drops
  measurably, the "independent of $|D|$" clause fails. We have **no**
  such ablation.
- **H3-F3**: replace TCT best-of-$K$ with plain LoRA best-of-1,
  measure SECS. If the SECS is comparable, TCT is not contributing
  to sample-complexity collapse; LoRA is. We have **no** such
  ablation.

### 3.3 Do our measurements exclude the counterfactual?

**NO, not really.** This is the paper's biggest measured-vs-claimed
gap.

- **H3-F1 exclusion**: We have a **single operating point** at
  $n_m = 50$ with mean SECS 0.961. Axiom 3 says $n_m \in [10, 10^2]$
  suffices. We have zero evidence at $n_m = 10$ or $n_m = 100$; we
  cannot distinguish the curve shape. RO-10 is explicitly named as
  future work. Residual probability of H3-F1 being true (in the
  sense that $n_m = 50$ is load-bearing and the curve collapses at
  $n_m = 25$): **plausibly 30%**.
- **H3-F2 exclusion**: We do not vary the pretraining corpus of the
  generator in any case study. Qwen3-TTS is a fixed model. The
  claim "independent of $|D|$" is derivationally true by TCT's
  construction (Phase 2's supervisory signals can come from any
  general corpus), but **empirically untested**. This is a
  theorem-is-true-by-construction but is-it-useful question —
  the claim is valid; the evidence that it's operationally relevant
  is missing.
- **H3-F3 exclusion (TCT vs. LoRA attribution)**: **NOT EXCLUDED.**
  As with H2, we have no LoRA-only vs. LoRA+TCT ablation. The 0.961
  mean SECS is **jointly** a claim about the LoRA and about TCT.
  The paper currently attributes the sample-complexity collapse to
  TCT on DDA-enriched data, but without the ablation, a reviewer
  can argue "identity-locked LoRA on 50 clips achieves mean SECS
  around 0.95 by itself; TCT best-of-$K$ just lifts it slightly".
  The sample-complexity claim becomes "LoRA on 50 clips works",
  which is a pre-existing, un-novel observation.

### 3.4 Circular reasoning check — **STRUCTURAL WEAKNESS**

Theorem 1 says $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$
**by construction**. This is a definitional theorem: if your
pipeline has exactly Phase 1 + Phase 2 + Phase 3 as specified, and
Phase 1 uses only $R_m$, then the subject-specific data is exactly
$\bigcup_m R_m$, so the bound trivially holds.

**The non-trivial content of Theorem 1 is entirely in Axiom 3**:
that $k$ can be taken $O(10^2)$ without overfitting. Everything else
is counting.

But Axiom 3's support reduces to Case 3's single operating point
$k=50$. So the chain is:

> Theorem 1 (sample complexity is $O(M \cdot k)$) ←
> Axiom 3 ($k \leq 10^2$ suffices) ←
> Case 3 observation ($k = 50$ produces mean SECS 0.961 on 1 subject,
>                      1 language, 1 modality, 1 scorer family)

This is **not** circular in the strict sense, but it is a **single-
point tower**. The theorem is propped up by an axiom propped up by
one measurement. The manuscript has to be explicit that Theorem 1's
empirical anchor is a single operating point and that the curve is
unknown.

### 3.5 Reverse causality check

**The critical reverse-causality hypothesis for H3 is**: the
sample-complexity collapse is not caused by TCT; it is caused by
(a) modern frozen-embedder conditioning being good enough that
small $R_m$ produces useful centroids *for any downstream
consumer*, not specifically TCT. If true, the sample-complexity
collapse is a property of the **embedders**, not of **TCT**, and
the paper should re-frame accordingly.

**We cannot rule this out.** In fact the manuscript's own Axiom 3
text admits it: *"Axiom 3 is a claim about good conditioning of
modern frozen embedders, not a universal statement."*

The reverse framing is this: *the paper shows that (modern frozen
embedders + small $R_m$ + any downstream consumer) gives useful
identity vectors*. TCT is one particular downstream consumer. The
sample-complexity claim is a claim about the **embedders**; TCT is
the vehicle that exploits it.

**Action**: The discussion must credit the embedder-conditioning
literature with the sample-complexity result, not TCT alone. P5's
claim is that TCT **inherits** sample-complexity collapse from
well-conditioned embedders, not that TCT **produces** it.

### 3.6 Spurious correlation check

**Confound A — 16-minute source shared between Cases 2 and 3.**
Case 3 uses 50 clips from a 16-minute voice capture; Case 2's
2,362 clips also come from the same 16-minute source. If the 16-
minute source is unusually clean / unusually phonemically-diverse
/ unusually well-mic'd, the sample-complexity result on this source
will not generalise to wild-data sources. **Not currently excluded.**

**Confound B — Subject selection.** Case 3 is one subject. If that
subject has an unusually distinctive voice (high within-identity
concentration in WavLM space), $n_m = 50$ suffices; if the subject
has a generic voice, $n_m = 200$+ might be needed. The paper
implicitly generalises from one subject to "subjects in general".
Agent-14 Bias-05 already flags this.

**Confound C — Scorer quality.** If WavLM-base-plus-sv is a
particularly strong voice encoder (high SNR on identity
discrimination), the sample-complexity collapse is partly a
property of the scorer. A weaker scorer would produce a different
curve.

### 3.7 Verdict on H3

- **H3 as a constructive theorem about pipeline structure**:
  survives trivially; it is nearly a tautology under the TCT
  specification.
- **H3 as a non-trivial empirical claim (Axiom 3 with $k \in [10,
  10^2]$)**: **partially open**. Case 3's $k=50$ is a single
  operating point. The curve is unknown. The paper must state the
  single-point limitation clearly; it must promise RO-10 as future
  work; and it must not claim $k=10$ suffices without evidence.
- **H3a as T1 measured**: survives, with the cross-encoder /
  single-subject / single-language / LoRA-vs-TCT ablation gaps
  disclosed as Bias-05, Theme D, and a new methodological
  disclosure.
- **H3b as T3 projected**: survives as a prediction; T1 measurement
  pending.

**Net**: H3 is partially open. The single biggest disclosure gap is
the LoRA-only vs. LoRA+TCT ablation. The second biggest is the
sample-complexity curve shape (RO-10). Until either is closed, the
sample-complexity collapse headline is a **theorem-by-construction
with a single empirical anchor**, not a measured curve.

---

## 4. H4 — DDA is collapse-silent by construction

### 4.1 Counterfactual

**H4 is false** iff a DDA-enriched pipeline (no recursion of
generator outputs into $D$; only frozen $\phi_m$ on fixed $D$)
produces distributional drift of the kind Shumailov-2024 /
Dohmatob-2024 / Alemohammad-2023 describe.

### 4.2 Observable signature of falsity

A toy comparison:
- **A**: generator trained on its own outputs for $T$ rounds
  (recursive self-distillation).
- **B**: generator trained once on $D' = \{\phi_m(x), \rho_{jk}\}$
  signals derived from a frozen, fixed $D$.

If **B** shows Shumailov-style mode collapse / tail-decay despite
no recursion, H4 is falsified.

### 4.3 Do our measurements exclude the counterfactual?

**No — we have zero empirical evidence.** The claim is a **scope
argument**: the preconditions of Shumailov's theorem (generator-in-
loop) are not met by DDA, therefore the theorem does not apply.
This is logically sound but does **not** constitute empirical
evidence.

**What the paper claims vs. what the paper proves**:
- **Claims**: DDA is structurally exempt from collapse theorems.
- **Proves**: The preconditions of those specific theorems are
  not met.
- **Does not prove**: DDA pipelines in practice do not exhibit
  collapse-like drift for unrelated reasons.

### 4.4 Circular reasoning check

**PASS.** The argument is a straightforward scope / logical
implication. No step invokes H4 in support of H4.

### 4.5 Reverse causality check

**N/A.** H4 is a scope negation; there is no causal arrow to
reverse.

### 4.6 Spurious correlation check

**One subtle concern.** DDA pipelines in practice may include
**data-curation loops** (e.g., filtering training data by embedder-
derived quality scores) that *themselves* constitute a form of
feedback. If the Context Graph ingestion pipeline uses embedder-
derived quality scores to reject low-quality memories, the
**corpus is generator-adjacent** even if no generator output is
fed back. This is not recursion in the Shumailov sense but is a
form of embedder-mediated curation that would violate the "fixed
$D$, no recursion" cleanliness.

**We have not audited whether Context Graph's ingestion pipeline
uses embedder-derived signals to filter $D$.** If it does, the
clean scope argument weakens.

**Action**: The §3.3 scope argument should explicitly state the
boundary: DDA is collapse-silent **with respect to generator-in-
loop recursion**; other forms of embedder-mediated data selection
are out of scope for the specific collapse theorems cited but are
in scope for a broader "data-quality drift" concern.

### 4.7 Verdict on H4

**Survives as scope argument.** The manuscript should tighten
language from "DDA is collapse-silent" (too strong) to "the
preconditions of the cited collapse theorems are not satisfied by
DDA" (exact). Confidence: 90% that the scope argument stands under
adversarial reading; residual 10% for the embedder-mediated
curation concern above.

---

## 5. H5 — Meaning compression as a dimensionally-distinct category

### 5.1 Counterfactual

**H5 is false** iff the unit "structured-signals / raw-data-unit"
can be shown to be a linear rescaling of one of the three classical
compression unit pairs (bits/bits, accuracy/parameters,
memory/quality).

### 5.2 Observable signature of falsity

Construct an invertible transformation $T$ such that
$u_{\text{meaning}} = T(u_{\text{classical}})$ for some classical
$u$. If $T$ exists, H5 collapses.

### 5.3 Do our measurements exclude the counterfactual?

**STRONG exclusion, derivationally.** Units are:
- Bit compression: information / information (dimensionless ratio).
- Weight compression: accuracy / parameters (accuracy is
  dimensionless; parameters is count; ratio is accuracy-per-param).
- Activation compression: memory / output-quality.
- Meaning compression: signals / raw-data-unit (count / count).

The dimensional form "signals/raw-data-unit" is **count/count**,
same as bit-compression's "bits/bits". **Dimensionally, meaning
compression is not distinct from bit compression.** It is
**semantically** distinct: "signals" counts per-embedder
projections and pair interactions, not Shannon bits. But a
reviewer could reasonably argue that a count of projections is a
lossy surrogate for a bit count, and the ratio is just a special
case of bit compression applied to multi-encoder outputs.

**Residual probability of the counterfactual (H5 is a rescaling of
bit compression): plausibly 25%.** The dimensional argument is weaker
than agent-21 claims. The stronger claim — *meaning compression is
a semantic extension, not a dimensional extension* — is defensible
but must be stated that way.

### 5.4 Circular reasoning check

**PASS.** H5's argument rests on unit analysis, not on H5.

### 5.5 Reverse causality check

**N/A.**

### 5.6 Spurious correlation check

**Weak concern.** Huang et al. 2024's $r \approx 0.95$ compression-
capability correlation (cited as R-C1) is a **within-model** result.
The paper's use of H5 to sit "alongside" Huang in a taxonomy may
create the impression of supporting evidence when in fact the two
claims operate on orthogonal axes. Tension 2 (agent-17 resolution)
addresses this, but the manuscript should not cite Huang as
*evidence for* H5; H5 is a taxonomic claim, Huang is cross-axis.

### 5.7 Verdict on H5

Survives as a **semantic taxonomic claim**. Must not be claimed as
a **dimensional** distinctness claim because bits/bits and
signals/raw-data-unit share dimensional form (both count/count).
Downgrade language in §5.2 from "dimensionally distinct" to
"semantically distinct / fourth taxonomic entry measured in
structured-signal density rather than Shannon bits". Confidence:
88% (as given by agent-23) but with the language downgrade
applied.

---

## 6. Cross-hypothesis causal DAG (failure propagation)

```
    Axiom 1 fails (ϕ updated)
         │
         ├──► H1 fails (signal count relies on frozen ϕ)
         ├──► H1a fails
         ├──► H2 fails (guard relies on frozen ϕ)
         ├──► H2a evidence reduces to self-distillation artefact
         ├──► H3 fails (T1 result contaminated by generator-ϕ coupling)
         ├──► H3a evidence value collapses
         └──► H4 fails (recursion re-enters through ϕ update)

    Axiom 2 severe fail (two embedders linearly dependent)
         │
         ├──► H1 degrades to H1b (upper bound only)
         ├──► H1a "91" becomes "up to 91"
         └──► H5 semantic claim unchanged (taxonomic, not count-dependent)

    H2a replication fails (measured SECS < 0.85 on rerun)
         │
         ├──► H2 loses its T1 anchor (argument-only)
         ├──► H3 loses its T1 anchor
         ├──► H3a falsified
         └──► §6.3 Case 3 headline reframed from "measured" to "reported"

    Cross-encoder evaluation shows Case-3 SECS ≈ 0.80 (not 0.961)
         │
         ├──► H2a evidential value for identity quality collapses
         ├──► H3a evidential value for sample-complexity collapses
         ├──► The 0.961 number stands as a WITHIN-ENCODER result only
         └──► Paper must re-lead §6.3 with cross-encoder numbers

    LoRA-alone ablation: raw mean SECS ≈ 0.95 (without TCT best-of-K)
         │
         ├──► TCT's measured contribution ≈ 0.01 absolute
         ├──► H3's "TCT causes sample-complexity collapse" weakens to
         │    "TCT inherits sample-complexity from the LoRA"
         └──► P5 attribution shifts: TCT is a verification layer, not
              a sample-complexity instrument.

    RO-8 measurement: 2+ pairs in Φ have MI/H > 0.9
         │
         ├──► H1 count becomes noticeably below 91
         ├──► H5 headline is "fourth category" still holds semantically
         └──► §3.4 headline number requires downward revision
```

The **dominant failure mode** is not a single catastrophic
falsification; it is the compounding of three partial exposures:

1. Encoder-matched evaluation on Case 3.
2. Missing LoRA-only ablation.
3. Single operating point on the $n_m$ curve (no RO-10 data).

Any one of these alone is survivable with disclosure. All three
together reduce the paper's measured claims to "we ran one pipeline
on one subject, scored it with the same encoder used in the guard,
and did not ablate the contribution of the guard versus the LoRA".
The theoretical framework survives; the empirical headline weakens
to "single measured anchor, promising but not validated".

---

## 7. What agent-25 (method-designer) must do in response

1. **Add a LoRA-only vs. LoRA+TCT ablation to the Case-3 run list.**
   Report the SECS histogram of all 12 best-of-$K$ candidates
   (not just the argmax that passes the guard). Compute the delta
   attributable to TCT.
2. **Add a cross-encoder evaluation on Case 3.** ECAPA-TDNN and
   Resemblyzer are the canonical alternatives; the paper must
   report SECS under at least one non-WavLM-family scorer to rule
   out the encoder-matched selection effect.
3. **Add a minimal $n_m$ ablation** ($n_m \in \{10, 25, 50, 100\}$
   at least). Without this, the $k \in [10, 10^2]$ claim of Axiom
   3 is a single-point claim masquerading as a range.
4. **Audit the Case-3 held-out set for cross-session / cross-device
   diversity.** If all held-out sentences are from the same
   recording session as the reference clips, disclose that and
   caveat accordingly.
5. **Audit the guard code path for any learned-model involvement.**
   Confirm determinism via 10 re-runs. Report the code inspection
   result in §5.3 as an explicit claim, not a background
   assumption.
6. **Audit Context Graph's ingestion pipeline** to confirm that
   embedder-derived signals do not silently re-curate $D$. If they
   do, disclose the boundary of H4's scope argument.

---

## 8. Summary scorecard

| H | Counterfactual survived? | Residual weakness | Confidence post-stress |
|---|---|---|---|
| H1 | Yes (as upper bound) | RO-8 MI unmeasured; "91" should be "up to 91" | 80% (was 95%) |
| H1a | Yes (as upper bound) | Same | 78% (was 92%) |
| H1b | Yes (hedge) | — | 90% (unchanged) |
| H2 | Yes as construction claim | Cross-encoder & LoRA-ablation gaps are material | 75% (was 92%) |
| H2a | Yes as within-encoder result | Cross-encoder unverified; held-out-session diversity not audited | 70% (was 95%) |
| H2b | Yes derivationally at $M=1$ | No $M>1$ evidence; genuine open exposure | 85% (was 94%) |
| H2c | Prediction stands | Case 1 T1 pending | 70% (was 75%) |
| H2d | Prediction stands | Case 2 T1 pending; widest uncertainty | 60% (was 65%) |
| H3 | Yes as constructive theorem | Non-trivial content rests on single operating point | 65% (was 85%) |
| H3a | Yes as within-encoder, within-subject | Curve shape unknown; LoRA-ablation missing | 70% (was 95%) |
| H3b | Prediction stands | Case 2 T1 pending | 60% (was 70%) |
| H4 | Yes as scope argument | Embedder-mediated curation boundary needs disclosure | 85% (was 90%) |
| H5 | Yes as semantic-taxonomic claim | "Dimensionally distinct" language is overclaim | 75% (was 88%) |

**Confidence levels dropped across the board after stress testing.**
The three measured claims (H2a, H3a, H4) drop by 10–25 points;
structural claims (H1, H5) drop by 10–15 points. This is the
expected effect of a rigorous counterfactual audit and does not
indicate the hypotheses are unsound — it indicates that the
manuscript's confidence statements prior to stress testing were
calibrated to "derivations closed + single measurement anchors"
and did not account for the residual counterfactuals this audit
surfaces.

---

## 9. Changelog

- 2026-04-17 — Initial issue by agent-24-causal-simulator.
