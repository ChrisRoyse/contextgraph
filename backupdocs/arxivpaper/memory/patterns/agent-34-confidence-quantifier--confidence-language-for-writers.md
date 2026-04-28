---
namespace: patterns
agent_id: agent-34-confidence-quantifier
timestamp: 2026-04-17T08:45:00Z
status: active
locked: false
supersedes: none
tags: confidence-language, hedges, paste-ready, writers-cue-sheet, epistemic-calibration
depends_on:
  - memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md
  - memory/patterns/agent-33-falsification-agent--claim-softening-recommendations.md
downstream_consumers:
  - agent-35-introduction-writer
  - agent-36-relatedwork-writer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-41-conclusion-writer
---

# Confidence Language Cue-Sheet for Section Writers

## 0. Purpose

This file tells writers **exactly which hedges to use** for each
claim, so that the paper's prose faithfully represents the calibrated
confidence levels from `agent-34--per-claim-confidence.md`.

## 1. Global rules (apply everywhere)

1. **Never claim "demonstrated" / "proved" / "established"** for any
   empirical finding with confidence < 95%.
2. **Never claim "improves over all prior work"**; use **pool-relative**
   language ("no cited method in our locked pool", R3).
3. **Never say "X causes Y"** for observational deltas; use
   "X is associated with Y" or "our pipeline with X achieves Y".
4. **Always distinguish constructive from empirical** claims. A
   derivation under axioms is not the same as a measured result.
5. **Always state the scope** of measurements: encoder, speaker set,
   language, number of held-out items.
6. **Always flag T1 vs T3** (measured vs projected) using agent-22's
   variable-dictionary tier codes or explicit "measured" / "projected"
   phrasing.
7. **Never use "dimensionally distinct"** for H5; use "semantically
   distinct / measured in a different unit" (agent-33 rec. 5 SHOULD).

## 2. Per-claim language templates

### C1 — "up to $N+\binom{N}{2}$ signals/input" (≥95% confident)

**Permitted verbs**: "yields **up to**", "admits a structural upper
bound of", "permits at most".

**Forbidden verbs**: "yields exactly", "produces", "generates N+C(N,2)".

**Paste-ready sentence (§3.4)**:

> "Under Axioms 1 and 2, running $N$ frozen approximately-independent
> embedders over a corpus of $n$ raw inputs yields **up to**
> $n \cdot (N + \binom{N}{2})$ structured supervisory signals — a
> **structural upper bound** whose tightness is pending empirical
> mutual-information measurement (RO-8)."

**Paste-ready sentence (§4.1)**:

> "For Context Graph's production panel at $N=13$, the per-input
> structured-signal count is **up to 91** ($13 + 78$). Qualitative
> blind-spot cross-check evidence confirms the effective count
> strictly exceeds $N$; the tight value within $[N, N + \binom{N}{2}]$
> is pending RO-8."

### C2 — Meaning compression as a fourth category (~85% confident)

**Permitted**: "semantically distinct", "measured in a different unit",
"trades along a different axis from", "we propose as a fourth
taxonomic entry".

**Forbidden**: "dimensionally distinct", "dimensionally
non-interconvertible" (agent-33 rec. 5 / W8), "proven novel".

**Paste-ready sentence (§3.1 / §5.2)**:

> "Meaning compression trades **structured-signal density per raw
> input** — a unit semantically distinct from Shannon bits, parameter
> accuracy, and activation memory. We propose it as a fourth
> taxonomic entry alongside bit-, weight-, and activation-compression;
> reviewer acceptance of the taxonomic distinction is a scholarly
> judgement we do not pre-empt."

### C3 — TCT Phase-3 per-output verifiability (~90% confident)

**Permitted**: "per-output verifiable **relative to the chosen
centroid panel**", "admits a cheap deterministic per-output test",
"produces a deterministic accept/reject decision".

**Forbidden**: "per-output verifiable against general identity",
"formally sound and complete", "guaranteed correct" (overclaim
lexicography).

**Paste-ready sentence (§5.1 / §5.3)**:

> "Phase 3 provides per-output verifiability **relative to the
> target centroid panel**: the guard $G_\tau$ is a deterministic
> $O(M \cdot d_{\max})$ geometric test that admits no auxiliary
> learned model (Theorem 2). Scope note: 'verifiable' is used in the
> pool-relative sense — the guarantee is that $\hat y$ satisfies the
> specified cosine predicate, not that $\hat y$ is correct under any
> external identity criterion."

### C4 — DDA collapse-silent by construction (~85% confident)

**Permitted**: "structurally not subject to the Shumailov-2024
recursion regime", "the preconditions of the cited collapse theorems
do not apply to DDA by construction", "generator-in-loop recursion is
absent".

**Forbidden**: "immune to all forms of model collapse", "proven
collapse-free" (W9 boundary issue).

**Paste-ready sentence (§3.3)**:

> "DDA is **structurally not subject** to the generator-in-loop
> recursive-collapse regime of Shumailov et al. (2024), because
> $D'$ depends only on a real corpus $D$ and frozen measurements
> $\phi_m$. **Scope note**: this argument addresses recursive
> self-distillation of the Shumailov form; embedder-mediated data
> curation (using $\phi$ scores to filter $D$) is a different
> feedback not covered by the cited theorems and is acknowledged as
> out-of-scope (W9)."

### C5 — 0.961 mean WavLM SECS (≥98% encoder-matched)

**Permitted**: "under the encoder-matched protocol we measure",
"within-WavLM-family identity agreement", "the Phase-3 guard selects
… and the post-selection mean WavLM SECS is 0.961 (max 0.975)".

**Mandatory companion sentence** (agent-24 W1 / agent-32 IC-3 /
agent-33 rec. 2):

> "The 0.961 mean is an **encoder-matched** measurement: guard and
> evaluator are both WavLM-base-plus-sv. This demonstrates the guard
> behaves as specified; it does not constitute cross-encoder identity
> verification. Cross-encoder triangulation (ECAPA-TDNN on the same
> 10 held-out sentences) is the EXP-1 remediation; if run and
> STRENGTHEN, the headline is promoted to cross-encoder agreement;
> otherwise §8 L-01 carries the within-WavLM-family scope."

### C6 — DNSMOS-P.808 parity for this speaker (~85%)

**Permitted**: "DNSMOS-P.808 scores are within evaluator noise
(clone 3.93 vs. real 3.93, N=14)", "we observe parity within
noise", "indistinguishable within the P.808 evaluator".

**Forbidden**: "statistically equivalent" (N=14 is too small for a
formal equivalence test), "indistinguishable from real speech in
general".

**Paste-ready sentence (§6.3)**:

> "DNSMOS-P.808 MOS for the clone (3.93) matches the real recording
> (3.93) within the evaluator's observed variance on this 14-sentence
> comparison set; UTMOS shows the clone marginally higher (3.012 vs.
> 2.997). Single-speaker, English-only scope (Bias-05, Bias-06)
> applies; a formal statistical equivalence test across a larger
> speaker panel is out of scope."

### C7 — $T=0.3$ optimal (~75%)

**Permitted**: "we find $T = 0.3$ optimal in this pipeline /
speaker / corpus", "$T=0.3$ produced the highest post-selection mean
SECS on our sweep".

**Forbidden**: "$T=0.3$ is the optimal temperature for voice
cloning", "T=0.3 generalises to other speakers / corpora /
backends".

**Paste-ready sentence (§6.3)**:

> "In our sweep $T \in \{0.1, 0.3, 0.5, 0.7, 0.9\}$, $T=0.3$ produced
> the highest post-selection mean SECS under WavLM best-of-12. The
> optimum is **pipeline-specific**; we make no claim about transport
> to other speakers, other TTS backends, or other text corpora."

### C8 — 50-clip centroid > 1-clip centroid by +0.015 (~85%)

**Permitted**: "the 50-clip centroid improves post-selection mean
SECS by +0.015 over the 1-clip baseline on our held-out set",
"a paired comparison across 10 held-out sentences yields +0.015
mean delta".

**Forbidden**: "50 clips is the optimal number of references"
(single operating point; W3), "centroid size matters in general".

**Paste-ready sentence (§6.3 ablation table caption)**:

> "Centroid ablation (paired, 10 held-out sentences): 50-clip
> centroid mean = 0.961; 1-clip centroid mean = 0.946; delta = +0.015.
> Single operating point on the $n_m$ curve; sample-complexity curve
> RO-10 is future work."

### C9 — Constellation guard ON vs OFF on Case 2 (~50%)

**Mandatory framing**: this is a **T3 PROJECTED** prediction; NO T1
data.

**Permitted**: "we predict that the guard materially affects identity-
cosine stability", "based on analogy to Case 3, we project",
"Case 2 T1 measurement pending".

**Forbidden**: "we find", "we show", "is necessary", "we demonstrate".

**Paste-ready sentence (§6.2)**:

> "**Projected (T3).** By analogy to Case 3 and under Axiom 2, we
> predict the constellation guard materially affects identity-cosine
> stability on Case 2. T1 measurement is pending (H2d, H3b)."

### C10 — Mouth-weighted loss necessary for lip-sync (~55%)

**Permitted**: "consistent with", "the forensic finding is
consistent with", "we hypothesise".

**Forbidden**: "is necessary", "must", "cannot be achieved without",
"proven to improve".

**Paste-ready sentence (§6.2)**:

> "The forensic audit (Santa project `docs/inspect/08_training_audit.md`)
> identified a flow-matching sign-bug whose fix coincided with
> qualitative lip-sync improvement. This is **consistent with**
> mouth-weighted loss contributing to lip-sync accuracy; an ablation
> isolating mouth weighting from other between-version changes
> (LoRA rank, batch size, dataset version) is VT-I4 future work."

### C11 — Case 1 15/15 FSV on cleaned retrain (~60%)

**Permitted**: "a prior run of the Shakespeare LoRA pipeline passed
15/15 FSV; the cleaned-data retrain is in progress at the time of
writing", "SFT+DPO retrain on cleaned data is ongoing".

**Forbidden**: "the cleaned retrain passes 15/15", "we demonstrate
15/15 after retrain" (retrain not yet complete).

**Paste-ready sentence (§6.1)**:

> "**Projected (T3).** A prior run of the three-layer enforcement
> stack passed 15/15 FSV; SFT+DPO retrain on forensically-cleaned
> data is in progress at the time of writing. The structure of the
> enforcement stack is unchanged; we project pass but disclose the
> retrain status as pending T1 confirmation."

### C12a — Sample-complexity theorem (≥95%)

**Permitted**: "Theorem 1", "constructive", "under Axioms 1–3 the
bound $O(M \cdot k)$ holds by construction", "any pipeline satisfying
the stated Phase 1/2/3 structure".

**Forbidden**: using the theorem as if it were an empirical finding.

**Paste-ready sentence (§5.4)**:

> "**Theorem 1 (constructive).** Under Axioms 1–3 and the TCT
> Phase 1/2/3 structure, the subject-specific sample complexity
> satisfies $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$ with
> $k = \max_m n_m$, independent of $|D|$. The bound is
> **constructive**: any pipeline with exactly this structure
> satisfies it by definition."

### C12b — Sample-complexity collapse empirical (~40%)

**Critical**: agent-33 rec. 2 MUST softening splits C12a and C12b.

**Permitted for empirical side**: "empirical support for the
collapse is anchored at **one T1 operating point** (Case 3, $M=1$,
$k=50$); broader empirical validation is pending", "one observation
consistent with the bound".

**Forbidden**: "we demonstrate the collapse", "across modalities",
"in practice" without the "one T1 point" qualifier, "sample-complexity
decreases" as a general empirical statement.

**Paste-ready sentence (§1 headline / §5.4 / §6.3 / §7)**:

> "**Constructive theorem** (C12a): sample complexity collapses to
> $O(M \cdot k)$ under Axioms 1–3 by construction.
> **Empirical anchor** (C12b): the measured collapse is supported at
> **one T1 operating point** ($M=1$, $k=50$, mean SECS 0.961) on
> Case 3. Broader empirical support (Case 1 text; Case 2 multi-modal;
> $n_m$ curve) is named future work (H2c, H2d, H3b; RO-10)."

### C13 — TCT beats scalar-reward on per-output verifiability (~80%)

**Permitted**: "relative to the locked 88-citation master pool",
"no cited method in our pool achieves per-output verifiability
via geometric alignment under frozen embedders", "scalar-reward
methods do not provide per-output verifiability in the same
technical sense".

**Forbidden**: "outperforms RLHF / DPO / CAI" (category error and
benchmark-comparison-free; agent-23 §9), "best alignment method",
"strictly superior to" any named scalar-reward approach.

**Paste-ready sentence (§5.1 / §7)**:

> "**Pool-relative**: no method in our locked reference pool achieves
> per-output verifiability via geometric alignment under frozen
> embedders. Scalar-reward alignment (RLHF, DPO, Constitutional AI)
> provides training-time shaping of a scalar objective but does not
> provide a per-output accept/reject predicate of the form $G_\tau$
> without auxiliary learned machinery. The comparison is structural,
> not a benchmark contest."

### C14 — Research contribution, not marketing (~80% CONDITIONAL)

**Mandatory**: Ethics statement (agent-31) and Competing-Interests
statement (agent-31) must be present.

**Permitted**: acknowledgement of Teleox.ai / Leapable.ai
affiliations; single footnote or acknowledgement sentence.

**Forbidden in body text**: product pitches, URLs to commercial
platforms, pricing / revenue numbers, market-sizing claims.

**Paste-ready sentence (Acknowledgements / Ethics)**:

> "The author acknowledges affiliation with Teleox.ai (primary) and
> Leapable.ai (companion marketplace). The framework described here
> predates and is independent of commercial deployment choices;
> Competing Interests are disclosed per agent-31-ethics-reviewer's
> statement. The paper makes no claims about commercial performance,
> market size, or product positioning."

---

## 3. Section-by-section checklist

### §1 Introduction

- [ ] Use "up to 91 signals" (C1 softening MUST).
- [ ] Use "0.961 WavLM SECS **under the encoder-matched protocol**" with
      the W1 disclosure inline or forward-referenced (C5).
- [ ] Frame sample-complexity collapse as "constructive theorem +
      one T1 empirical anchor" (C12a/C12b split MUST).
- [ ] Do NOT claim "solves the data wall" — use "reframes".

### §3 Framework

- [ ] "Up to $N+\binom{N}{2}$" everywhere (C1).
- [ ] "Semantically distinct" not "dimensionally distinct" (C2).
- [ ] H4 scope note: Shumailov-recursion specifically (C4 / W9).

### §4 Systems

- [ ] Context Graph: "up to 91 signals/input" (H1a; C1).
- [ ] ClipCannon: "up to 28 signals/input" at $N=7$ (C1 instantiated).
- [ ] Leapable: acknowledgement only; no commercial prose (C14).

### §5 TCT method

- [ ] "Verifiable **relative to the chosen centroid panel**" (C3).
- [ ] Theorem 2 scope note: pool-relative novelty (C13).
- [ ] Theorem 1: "constructive" (C12a); empirical anchor disclaimer
      (C12b).

### §6 Case studies

- **Case 1**: "SFT+DPO retrain in progress; 15/15 projected" (C11).
- **Case 2**: "T3 PROJECTED" prefix on all quantitative claims (C9, C10, H2d, H3b).
- **Case 3**:
  - [ ] Headline sentence: "0.961 mean SECS … encoder-matched protocol"
  - [ ] Mandatory companion: W1 single-sentence disclosure (C5).
  - [ ] T=0.3 claim restricted to "this pipeline / speaker / corpus" (C7).
  - [ ] 50-clip centroid: "+0.015 vs 1-clip on paired held-out; single
        operating point, RO-10 future work" (C8).
  - [ ] DNSMOS parity: "within evaluator noise, N=14" (C6).

### §7 Discussion

- [ ] No "outperforms" / "beats" RLHF; use pool-relative + structural.
- [ ] Credit frozen-embedder conditioning literature (W6).
- [ ] Frame TCT as "operationalises" not "causes" sample-complexity
      collapse (W6).

### §8 Limitations

- [ ] L-01: W1 encoder-matched; cross-encoder EXP-1 pending.
- [ ] L-02: W2 LoRA-only / pre-selection SECS pending.
- [ ] L-03: W4 held-out session relationship disclosed.
- [ ] L-04: W5 pairwise MI; RO-8 pending.
- [ ] L-05: W7 no $M>1$ empirical; Case 2 T1 pending.
- [ ] L-06: W3 single $n_m$ operating point; RO-10 pending.

### §9 Conclusion

- [ ] Headline: "constructive theorem + one T1 anchor".
- [ ] No "solves" / "proves" language.
- [ ] Conditional framing: "if EXP-1 and EXP-3 land, …".

---

## 4. Banned phrases (do not use anywhere in the paper)

| Banned | Allowed substitute |
|---|---|
| "91 signals per input" | "up to 91 signals per input" |
| "dimensionally distinct" | "semantically distinct / in a different unit" |
| "the collapse" | "the constructive collapse bound" or "the empirical anchor at one operating point" |
| "0.961 mean SECS" (bare) | "0.961 mean SECS under the encoder-matched protocol" |
| "indistinguishable from real speech" (bare) | "DNSMOS parity within evaluator noise (N=14, single speaker)" |
| "solves the data wall" | "reframes the data wall as three complementary paths" |
| "outperforms RLHF" | "provides per-output verifiability pool-relative to RLHF/DPO/CAI" |
| "T=0.3 is optimal" | "T=0.3 is optimal in this pipeline/speaker/corpus" |
| "proven collapse-free" | "structurally not subject to the cited generator-in-loop collapse regime" |
| "mouth-weighted loss is necessary" | "the forensic finding is consistent with mouth-weighted loss contributing to lip-sync" |
| "we demonstrate the sample-complexity collapse" | "we demonstrate the collapse at one T1 operating point; broader support is named future work" |

---

## 5. Confidence

**HIGH** that the per-claim language templates match the calibrated
confidence levels in agent-34's per-claim table.

**HIGH** that the banned-phrase list captures the top review-kill
vectors identified by agent-33's CA-1 … CA-6 analysis.

**HIGH** that writers who follow this cue-sheet will produce a
paper whose prose accurately represents the epistemic strength of
the evidence.

**MEDIUM** on the temperature of downstream writer compliance;
agent-42 (adversarial reviewer) is expected to spot-check compliance
with the banned-phrase list.

## Changelog

- 2026-04-17 — Initial issue by agent-34-confidence-quantifier.
