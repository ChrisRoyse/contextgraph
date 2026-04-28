---
namespace: patterns
agent_id: agent-25-method-designer
timestamp: 2026-04-17T08:30:00Z
status: active
locked: true
depends_on:
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/handoff/agent-24-to-25--counterfactuals-ready.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
downstream_consumers:
  - agent-26-sampling-strategist
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
tags: weakness-remediation, disclosure-plan, W1, W2, W7, case-3, ablations
---

# Weakness Remediation Plan — W1, W2, W7 (and the rest)

## 0. Purpose

Translate the weakness register (W1–W10) from agent-24 into **actionable
remediation steps** with two outcomes per weakness: (a) the remediation
run that would close the weakness, (b) the disclosure language if the
run cannot land in time. The paper will publish with either the
remediation or the disclosure; never with neither.

The three CRITICAL / HIGH items called out by the agent-24 handoff
(**W1**, **W2**, **W7**) are addressed in detail; the remaining
weaknesses (W3–W10) receive a concise remediation-or-disclose pair.

---

## 1. W1 — Encoder-matched evaluation on Case 3 (CRITICAL)

### 1.1 What the weakness is

Case 3's headline number (0.961 mean WavLM SECS) comes from the
argmax over $K = 12$ candidates under **WavLM cosine ≥ 0.95**, scored
by **WavLM SECS**. The guard and evaluator are the same encoder. The
0.961 is **the signature of the guard working**, not cross-encoder
identity evidence.

### 1.2 Remediation (planned run)

**Procedure**:

1. Take the same 10 held-out English sentences and the same argmax
   candidate $\hat y^{(k^*)}$ accepted by the WavLM guard.
2. Compute SECS with a second, architecturally-independent speaker
   encoder. **Preferred scorers**:
   - **ECAPA-TDNN** (SpeechBrain implementation; ResNet-TDNN hybrid
     trained on VoxCeleb with AAM-softmax).
   - **Resemblyzer** (GE2E-loss-trained LSTM d-vector; open-source
     `resemble-ai/Resemblyzer`).
3. Report, for each held-out sentence:
   - Cosine under ECAPA-TDNN $s^{\text{ECAPA}}$.
   - Cosine under Resemblyzer $s^{\text{Res}}$.
   - Agreement measure: $|s^{\text{WavLM}} - s^{\text{ECAPA}}|$ and
     $|s^{\text{WavLM}} - s^{\text{Res}}|$.
4. Aggregate: mean cosine under each scorer; 9-of-10-above-threshold
   counts under pool-appropriate thresholds
   ($\tau^{\text{ECAPA}}, \tau^{\text{Res}}$ calibrated on the same
   reference set per §3.3 of `tct-protocol.md`).

**Success criterion for strong remediation**:
- Mean cross-encoder SECS within ±0.10 of 0.961 (i.e., $\in [0.86,
  1.00]$).
- Agreement rate: at least 7 / 10 sentences pass the cross-encoder
  threshold under its own calibration.

**Partial success**:
- Cross-encoder mean $< 0.86$: the WavLM-matched number remains
  selection-induced but the cross-encoder agreement still above
  chance demonstrates speaker identity is preserved in part.

**Negative / falsification outcome**:
- Cross-encoder mean $\ll 0.86$ with chance-level agreement: H2a is
  falsified beyond within-encoder interpretation; paper must reframe
  headline.

### 1.3 Disclosure fallback (if the run does not land in time)

**Required sentence in §6.3** (near the 0.961 headline):

> *"The 0.961 mean SECS is an encoder-matched measurement: the
> best-of-$K$ guard selects on WavLM-base-plus-sv cosine, and the
> evaluator scores WavLM-base-plus-sv SECS. This demonstrates the
> guard behaves as specified; it does not constitute cross-encoder
> identity verification. A cross-encoder evaluation (ECAPA-TDNN and
> Resemblyzer) on the same held-out set is named future work (§10)."*

**Required entry in §8 Limitations** (L-01):

> *"L-01. **Encoder-matched evaluation.** The headline Case-3 SECS
> value in §6.3 was measured with the same encoder used by the
> best-of-$K$ guard (WavLM-base-plus-sv). The reported number is
> therefore the signature of the guard behaving as specified, not
> an independent identity-quality measurement. Cross-encoder
> replication is future work (RO-equivalent).*"

**Reframing of H2a in §5.4**: from "general identity quality" to
"within-WavLM-family identity agreement".

### 1.4 Status flag carried to agent-37 / agent-39

- Default to disclosure (§6.3 + §8 L-01) unless the cross-encoder
  run completes and is reported inline.
- If W1 remediation lands: replace the "future work" language with
  the measured cross-encoder numbers and a ±0.10 agreement statement.

---

## 2. W2 — Missing LoRA-only vs. LoRA+TCT ablation (CRITICAL)

### 2.1 What the weakness is

Case 3 reports the post-selection mean SECS (0.961) of the argmax
under the WavLM guard, but **not** the pre-selection distribution
over the $K = 12$ candidates. We therefore cannot distinguish:

- **(a)** Sampling-diversity effect: raw candidates already
  concentrate near 0.95; argmax selection adds ~0.01 absolute.
- **(b)** Threshold-rejection effect: raw candidates span
  $\sim 0.80$–$0.99$; argmax selection adds $\sim 0.10$ absolute.

Scenarios (a) and (b) imply very different causal attributions to
TCT. Note: Case 3 is **inference-only** (Qwen3-TTS not fine-tuned);
"LoRA-only" in the agent-24 register is best interpreted as "the
ablation that separates best-of-$K$ sampling from threshold
selection".

### 2.2 Remediation (planned run)

**Procedure**:

1. For each of the 10 held-out sentences, generate $K = 12$
   candidates at $T = 0.3$ exactly as in the headline run.
2. For each candidate $\hat y^{(k)}$, compute
   $s^{(k)} = \cos(\phi_{\text{speaker}}(\hat y^{(k)}),
   c_{\text{speaker}})$.
3. Report for each sentence:
   - Mean, min, max, 10th / 50th / 90th percentile of $\{s^{(k)}\}$.
4. Aggregate across 10 sentences:
   - **Pre-selection mean** $= \text{mean over all 120 candidates}$
     of $s^{(k)}$.
   - **Post-selection mean** = 0.961 (existing measurement).
   - **TCT-delta** = post-selection − pre-selection.
5. Additionally report the **"random-of-$K$" baseline**: pick one
   $\hat y^{(k)}$ uniformly at random per sentence; compute mean
   SECS over random draws. This isolates sampling-quality from
   selection-quality.

**Success criterion**:
- Pre-selection mean $\leq 0.90$ and post-selection mean = 0.961
  → **TCT selection contributes ≥ 0.06 absolute**; causal
  attribution to the guard is defensible.
- Pre-selection mean $\approx 0.95$ and post-selection mean = 0.961
  → attribution softens to "TCT verifies and selectively accepts".

**Falsification outcome**:
- Pre-selection mean = post-selection mean: TCT contributes nothing
  beyond the base model; guard becomes ornamental. H2a stands (the
  number is still 0.961) but the §5.4 attribution to TCT is
  falsified.

### 2.3 Disclosure fallback

**Required sentence in §6.3**:

> *"The reported 0.961 is the mean cosine of the best-of-$K=12$
> argmax; the pre-selection distribution over the 12 candidates is
> not reported in this manuscript. The TCT-delta (post-selection
> minus pre-selection) is therefore not directly measured here;
> we disclose this in §8 L-02 and soften the §5.4 attribution."*

**Required entry in §8 Limitations** (L-02):

> *"L-02. **Pre-selection distribution not reported.** Case 3's
> headline SECS is the post-selection argmax over $K = 12$
> candidates. The raw pre-selection distribution is not reported,
> so we cannot quantify the TCT-guard's marginal contribution
> (post- minus pre-selection mean) from the data in this manuscript.
> This is a measurement gap, not a claim that the guard is
> ineffective. Future work will report the full distribution."*

**Reframing of §5.4**: "TCT causes the identity lock" → "TCT
verifies and selectively accepts under a per-output predicate; the
marginal contribution beyond best-of-$K$ sampling is not quantified
in Case 3 and is named future work."

### 2.4 Stretch remediation

**Best-of-$K$ with $\tau = 0$**: run the same $K = 12$ argmax but
with no rejection threshold, and report mean SECS. This isolates
sampling-diversity from threshold-rejection.

---

## 3. W7 — No $M > 1$ evidence for multi-modal strictness (HIGH)

### 3.1 What the weakness is

H2b asserts that the multi-modal conjunctive guard $G_\tau = \mathbb{1}[\forall m : s_m \geq \tau_m]$
is **strictly stronger** than any single-modality check. The
derivation is elementary probability under Axiom 2, but Case 3 is
$M = 1$; Case 2 is T3-projected. The paper's "multi-modal paradigm"
framing has no measured $M > 1$ data.

### 3.2 Remediation (planned run)

**Primary remediation**: run the Case 2 pipeline end-to-end
(EchoMimicV3 + LoRA + TCT Phase 2 + Phase 3) to produce $M \geq 2$
measured cosines (e.g., speaker $\times$ global-visual). Report:

- Per-modality cosine distributions on the held-out set.
- Single-modality accept-rate at operating threshold (per modality).
- Conjunction accept-rate at the operating thresholds.
- Demonstrate empirically
  $\Pr[G_\tau = 1] \leq \min_m \Pr[s_m \geq \tau_m]$ (inequality of
  H2b).

**Case-1 assist**: if a second text-level encoder can be added
cheaply (e.g., cross-encoder on sentence-level semantics plus
token-level stylometric proxy), a weak $M = 2$ text-only
instantiation of Case 1 would serve as a small multi-modal anchor.

### 3.3 Disclosure fallback

**If Case 2 end-to-end measurement does not land before submission**:

**Required sentence in §5.4**:

> *"Multi-modal strictness (H2b) is derivational under Axiom 2
> (elementary probability). We have no measured $M > 1$ instantiation
> in this manuscript: Case 3 is $M = 1$ and Case 2 is T3-projected.
> Empirical $M > 1$ measurement is the single most valuable
> follow-up for the multi-modal framing (§10)."*

**Required entry in §8 Limitations** (L-05):

> *"L-05. **No $M > 1$ empirical measurement.** The multi-modal
> conjunction strictness claim (H2b, §5.4) is derivational only.
> Case 3 is single-modality; Case 2 (multi-modal) is projected.
> Empirical $M > 1$ instantiation on Case 2 is the single highest-
> value follow-up measurement."*

**Framing note in §6.2**: Case 2 section opens with explicit
"multi-modal instantiation is the critical next measurement".

### 3.4 Scope note carried in §1 and §7

- The paper does NOT claim "multi-modal is empirically better than
  single-modal in our data"; it claims the multi-modal conjunction
  **enables** stronger per-output predicates in principle (H2b
  derivation) and will be empirically validated on Case 2 (pending).

---

## 4. Remaining weaknesses (W3, W4, W5, W6, W8, W9, W10)

### 4.1 W3 — Single $n_m$ operating point (HIGH)

- **Remediation**: minimal $n_m \in \{10, 25, 50, 100\}$ curve on
  Case 3. See §3.5 of case-study protocols.
- **Fallback disclosure**: §8 L-03 (single operating point; curve
  is RO-10 future work).

### 4.2 W4 — Held-out-session audit (MEDIUM)

- **Mandatory resolution at writing time** (agent-26 / agent-37).
  Not optional.
- Outcome:
  - If same-session → disclose as §8 L-04.
  - If cross-session → document as strengthener in §6.3.

### 4.3 W5 — Axiom 2 pairwise MI unmeasured (HIGH)

- **Remediation (preferred)**: on a sample of 100–1000 inputs from
  Context Graph's production corpus, compute cosine-correlation or
  MI estimate on all $\binom{13}{2} = 78$ embedder pairs. Report
  redundancy histogram in an appendix.
- **Fallback disclosure**: §3.4 headline becomes "up to 91
  signals/input"; §8 L-06; RO-8 carried as future work.

### 4.4 W6 — TCT causal attribution vs. embedder conditioning (MEDIUM)

- **Remediation**: no additional measurement. Language change in
  §3.6, §5.4, §7, §9 to credit frozen-embedder conditioning
  literature (cluster-D retrieval / contrastive geometry).
- **Reframing**: "TCT causes the sample-complexity collapse" →
  "TCT operationalises the sample-complexity collapse implicit in
  well-conditioned frozen embedders".

### 4.5 W8 — "Dimensionally distinct" overclaim on H5 (MEDIUM)

- **Remediation**: wording change. "dimensionally distinct" →
  "semantically distinct / measured as structured-signal density
  rather than Shannon bits". Applies to §3.1 and §5.2.

### 4.6 W9 — H4 scope boundary (MEDIUM)

- **Remediation**: §3.3 language tighten: "DDA's scope argument
  addresses generator-in-loop recursion of the Shumailov-2024 form;
  other forms of embedder-mediated data selection fall outside
  those specific theorems but remain eligible for separate analysis."

### 4.7 W10 — Determinism of guard path (LOW)

- **Remediation**: §5.3 footnote:
  > *"The guard is deterministic up to GPU floating-point
  > reproducibility; the implementation uses identical parameters,
  > inputs, and kernel order at both training and inference time."*
- **Optional**: run 10 replicates of the guard on the Case-3 held-out
  set and report per-decision stability.

---

## 5. Remediation priority matrix

| W  | Severity | Remediation cost | Paper-value without it | Priority order |
|---|---|---|---|---|
| W1 | CRITICAL | Low (one cross-encoder run) | **Critical loss** | **1** |
| W2 | CRITICAL | Low–Medium (one ablation run) | **Critical loss** | **2** |
| W7 | HIGH | High (Case 2 end-to-end) | Medium loss | **3** (stretch) |
| W3 | HIGH | Medium ($n_m$ sweep) | Medium loss | **4** |
| W4 | MEDIUM | Zero (audit / document) | Low loss; mandatory | **5** |
| W5 | HIGH | Medium (small RO-8 run) | Medium loss | **6** (stretch) |
| W6 | MEDIUM | Zero (wording) | Low loss | **7** |
| W8 | MEDIUM | Zero (wording) | Low loss | **8** |
| W9 | MEDIUM | Zero (wording / audit) | Low loss | **9** |
| W10 | LOW | Zero (footnote) | Negligible | **10** |

**Decision rule**: remediate W1 and W2 first; disclose W3, W5, W7 if
remediation cost is prohibitive. W4 must be resolved regardless. W6,
W8, W9, W10 are wording fixes and carry no cost.

---

## 6. Disclosure language index (paste-ready for agent-37)

| Section | Language to insert | Weakness anchor |
|---|---|---|
| §3.3 | "DDA's scope argument addresses generator-in-loop recursion of the Shumailov-2024 form." | W9 |
| §3.4 | "up to 91 signals/input" (not "91 signals/input") | W5 |
| §3.6 | "TCT operationalises the sample-complexity collapse implicit in well-conditioned frozen embedders." | W6 |
| §5.2 | "semantically distinct / measured as structured-signal density rather than Shannon bits" | W8 |
| §5.3 footnote | "Deterministic modulo GPU floating-point reproducibility." | W10 |
| §5.4 | "Multi-modal strictness (H2b) is derivational under Axiom 2; empirical $M > 1$ instantiation is Case 2 future work." | W7 |
| §5.4 | "TCT verifies and selectively accepts under a per-output predicate; marginal contribution beyond best-of-$K$ sampling is not quantified in Case 3." | W2 |
| §6.3 | "The 0.961 mean SECS is an encoder-matched measurement [...]" | W1 |
| §6.3 | "Held-out sentences come from [same / different] recording session as the reference set." | W4 |
| §7 | Credit frozen-embedder conditioning literature (cluster-D). | W6 |
| §8 L-01 | Encoder-matched evaluation limitation | W1 |
| §8 L-02 | Pre-selection distribution not reported | W2 |
| §8 L-03 | Single $n_m$ operating point | W3 |
| §8 L-04 | Same-session confound (if applicable) | W4 |
| §8 L-05 | No $M > 1$ empirical measurement | W7 |
| §8 L-06 | Pairwise embedder MI not measured | W5 |

---

## 7. Confidence

**HIGH** that the remediation plan addresses every weakness in
agent-24's register with either a run or a disclosure.

**HIGH** that the disclosure language is sufficient for publication
even if none of the stretch remediations (W1 cross-encoder, W2
ablation, W3 $n_m$ curve, W5 MI, W7 Case 2 end-to-end) land before
submission — the empirical spine stands on the T1-measured Case 3
numbers with explicit scope disclosure.

**MEDIUM** on feasibility of the three CRITICAL/HIGH remediations
(W1, W2, W7) landing before submission; success depends on
engineering capacity.

## Changelog

- 2026-04-17 — Initial issue by agent-25-method-designer. Locked.
