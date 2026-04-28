---
namespace: reports
agent_id: agent-25-method-designer
timestamp: 2026-04-17T08:50:00Z
status: active
tags: final-report, SMAP, methodology, tct-protocol, case-studies, weakness-remediation
---

# Agent-25 Final Report — Methodology Design (SMAP)

## S — Situation

The manuscript "Teleological Constellation Training" requires a
reviewer-grade methodology section (§5 TCT + §6 Case Studies) that:

- Specifies the three-phase TCT protocol at a level an independent
  team can re-implement from text alone.
- Instantiates the protocol into three production case studies
  (Shakespeare LoRA, Santa talking-head, voice clone) with explicit
  tier flags on every number.
- Remediates the three CRITICAL / HIGH weaknesses (W1 encoder-
  matched evaluation, W2 missing LoRA-only ablation, W7 no $M > 1$
  evidence) flagged by agent-24's counterfactual audit.
- Preserves every upstream hedge: V11, V18, V22, R3, R4, Bias-05 /
  Bias-06, Theme D, MP-12.

Upstream dependencies:
- Agent-04 construct definitions (TCT, identity-lock, DDA, meaning
  compression).
- Agent-21 theoretical framework (Axioms 1–3, P1–P5, Theorems 1–2).
- Agent-22 master architecture (Figure 1, levels, variable
  dictionary).
- Agent-23 hypothesis ledger (H1–H5, H2a–H3b).
- Agent-24 counterfactual analyses and weakness register (W1–W10).

Downstream consumers:
- Agent-26 sampling-strategist (needs target populations, sample
  sizes with power, eligibility, disjointness).
- Agent-27 instrument-developer (needs encoder / scorer / rater
  specifications).
- Agent-37 methodology-writer (needs §5 and §6 writable spec).
- Agent-39 discussion-writer (needs attribution and disclosure
  language).
- Agent-42 adversarial-reviewer (needs exposed weaknesses with
  remediation traces).

## M — Mission

Produce five memory artefacts that together constitute a
re-implementable, reviewer-defensible methodology package:

1. Locked TCT protocol (Phases 1–3) independent of any case study.
2. Locked case-study evaluation protocols for Cases 1, 2, 3 with
   tier flags on every number.
3. Weakness remediation plan pairing each W with either a
   remediation run or a disclosure fallback, plus paste-ready
   section-level disclosure language.
4. Handoff to agent-26 with clear sampling-strategist task list
   and must-resolve items.
5. This final report (SMAP).

## A — Actions taken

### 1. Read and synthesised upstream artefacts
- mission-brief.md (project scope, constraints, three case
  studies, two production systems).
- agent-04-construct-definer--tct-definition.md (Phase 1/2/3
  formal spec).
- agent-21-theory-builder--theoretical-framework.md (Axioms 1–3,
  propositions, theorems).
- agent-22-model-architect--master-architecture.md (Figure 1
  levels 0–8, interconnect table).
- agent-23-hypothesis-generator--hypotheses.md (H1–H5, H2a–H3b,
  falsification conditions).
- agent-24-causal-simulator--weakness-register.md (W1–W10 with
  disclosure venues and remediation costs).
- agent-24-to-25--counterfactuals-ready.md (handoff with priority
  ablation list).

### 2. Authored the TCT protocol specification
- Phase 1 inputs / procedure / outputs / hyperparameters with
  normalisation convention, degenerate-norm guard, LOO stability
  check, and mandatory provenance record.
- Phase 2 loss definition with frozen-encoder audit hook
  (`param.grad is None` assertion + centroid-hash post-run
  equality check); training-mode discipline
  (full-fine-tune / frozen-with-LoRA / inference-only); $\lambda_m$
  guidance with anti-collapse heuristic.
- Phase 3 strict guard with three rejection policies (regenerate /
  fallback / escalate); relaxed best-of-$K$ variant; threshold
  calibration via same-session-band percentile; per-output
  provenance manifest.
- Cross-phase audits (embedder hash, centroid hash, threshold lock,
  reference-set manifest).

### 3. Authored the three case-study evaluation protocols
- **Case 1 Shakespeare**: SFT + DPO on cleaned data (training in
  progress), 3-layer enforcement stack (model prior via
  $\mathcal{L}_{\text{TCT}}$, constrained decoding at token level,
  constellation guard at sentence level), 15/15 FSV test inventory,
  measurement protocol with PENDING T1 flags, W1 cross-encoder and
  W2 LoRA-only ablation hooks.
- **Case 2 Santa**: EchoMimicV3 + LoRA (rank 256 / alpha 512 / 8
  target modules with `q_audio` excluded per forensic audit), mouth-
  weighted flow-matching base loss, constellation-conditioned TCT
  term, 2,362-clip training dataset, operating thresholds (visual
  0.70 / emotion 0.60 / speaker 0.80 / voice 0.85), measurement
  protocol with T1 PENDING flags.
- **Case 3 Voice**: inference-only Qwen3-TTS Full ICL + 50-clip
  centroid + WavLM best-of-$K=12$ at $T=0.3$ with T1 MEASURED
  headline (mean 0.961 / max 0.975 SECS; DNSMOS P.808 3.93; UTMOS
  3.012) and explicit W1 / W2 / W3 / W4 remediation-or-disclose
  plans.

### 4. Authored the weakness remediation plan
- W1 (CRITICAL): cross-encoder run with ECAPA-TDNN + Resemblyzer on
  same 10 held-out sentences, with fallback disclosure language for
  §6.3 and §8 L-01.
- W2 (CRITICAL): pre-selection distribution / TCT-delta measurement
  over all 12 candidates per sentence, with fallback language for
  §6.3 and §8 L-02 and §5.4 attribution softening.
- W7 (HIGH): Case 2 end-to-end $M > 1$ run (stretch); fallback
  disclosure language for §5.4 and §8 L-05 naming Case 2 as the
  critical pending measurement.
- W3 (HIGH): minimal $n_m \in \{10, 25, 50, 100\}$ ablation on Case
  3; fallback disclosure as §8 L-03.
- W4 (MEDIUM): mandatory session audit resolution — cannot be
  deferred; agent-26 and agent-37 must coordinate.
- W5, W6, W8, W9, W10: disclosure / wording / footnote fixes with
  paste-ready section language.
- Priority matrix (W1 > W2 > W7 > W3 > W4 > W5 > W6 > W8 > W9 >
  W10) with per-weakness remediation-cost / paper-value analysis.

### 5. Authored the handoff to agent-26
- Clear task list: target populations per case; sample sizes with
  power; eligibility screening instruments; disjointness matrices;
  recruitment procedures; sampling constraints.
- Four must-resolve open items: W4 session audit (mandatory);
  power analyses on Cases 1 & 2; $n_m$ curve sampling plan; Case
  2 clip-split disjointness.

## P — Plan for downstream agents

### Immediate (agent-26)
1. Design sampling plans for each case; resolve W4 session audit
   on Case 3; produce disjointness matrices.
2. Power-analyse evaluation-set sizes for Cases 1 and 2.
3. Produce sampling plan for W3 $n_m$ curve remediation.

### Immediate (agent-27 instrument-developer)
1. Specify exact encoder versions / weight hashes for every
   $\phi_m$ referenced in the protocol.
2. Specify cross-encoder scorers (ECAPA-TDNN implementation /
   weights, Resemblyzer implementation / weights) for W1
   remediation.
3. Specify evaluation frameworks (SyncNet / LSE-D for Case 2,
   DNSMOS / UTMOS for Case 3).
4. Specify human-evaluation rubric and Likert instrument for
   Case-1 blind rater pool (if included).

### Manuscript-writing (agent-37, agent-39)
1. Use the remediation-plan's disclosure-language index to place
   each W's sentence in the correct section.
2. Carry every hedge (V11, V18, V22, R3, R4, Bias-05/06, Theme D)
   verbatim.
3. Flag every PENDING number explicitly; never assert a T3 number
   as T1.
4. Soften §5.4 attribution per W2 and §3.6 per W6.

### Adversarial review (agent-42)
1. Verify every weakness has either a remediation or a disclosure
   in the final manuscript.
2. Spot-check for accidental over-claims on unmeasured numbers.
3. Confirm the empirical spine (§6.3 Case 3) carries W1 / W2 / W4
   disclosures.

## Files written

| # | Path | Purpose |
|---|---|---|
| 1 | `./memory/decisions/agent-25-method-designer--tct-protocol.md` | TCT Phases 1–3 re-implementable specification |
| 2 | `./memory/decisions/agent-25-method-designer--case-study-protocols.md` | Case 1/2/3 evaluation protocols |
| 3 | `./memory/patterns/agent-25-method-designer--weakness-remediation.md` | W1–W10 remediation plan with disclosure language |
| 4 | `./memory/handoff/agent-25-to-26--methodology-ready.md` | Handoff to sampling-strategist |
| 5 | `./memory/reports/agent-25-method-designer--final-report.md` | This file (SMAP) |

## Quality gates

| Gate | Status | Evidence |
|---|---|---|
| TCT Phase 1 specification re-implementable | PASS | Double-normalisation, LOO check, provenance manifest specified |
| TCT Phase 2 loss + frozen-encoder audit + $\lambda_m$ guidance | PASS | Loss equation, audit hook pseudocode, starting $\lambda_m$ values |
| TCT Phase 3 guard + thresholds + rejection policies + provenance | PASS | Strict predicate + relaxed best-of-$K$ + three policies + audit record |
| Case 1 SFT+DPO+FSV+3-layer stack with PENDING flags | PASS | Complete inputs / procedure / FSV table / measurement protocol |
| Case 2 EchoMimicV3 + LoRA (rank 256 / alpha 512 / 8 modules, q_audio excluded) + mouth-weighted flow-matching + 4-threshold operating set | PASS | Forensic-derived configuration documented |
| Case 3 Qwen3-TTS + ICL + 50-clip + WavLM K=12/T=0.3 with T1 MEASURED headline (0.961 / 0.975; DNSMOS 3.93; UTMOS 3.012) | PASS | Headline numbers preserved; W1/W2/W3/W4 remediation plans attached |
| W1/W2/W7 remediation plan + fallback disclosure | PASS | Two outcomes per weakness specified |
| W3/W4/W5/W6/W8/W9/W10 remediation or wording | PASS | Disclosure-language index per section |
| All hedges carried unchanged | PASS | V11, V18, V22, R3, R4, Bias-05/06, Theme D, MP-12 |
| Handoff to agent-26 specific and actionable | PASS | Task list with 4 must-resolve items |

## 3-sentence summary (required)

The TCT protocol is a three-phase re-implementable methodology:
**Phase 1 builds a frozen multi-modal centroid $C_P$ from reference
sets via double L2-normalisation; Phase 2 adds a cosine-to-centroid
penalty to the generator's base loss under a strict frozen-encoder
audit; Phase 3 runs a per-output conjunctive cosine guard
$G_\tau(\hat y; C_P)$ with three rejection policies and deterministic
provenance.** The case-study evaluation protocols instantiate the
protocol three ways — Case 1 Shakespeare (SFT+DPO+3-layer stack with
15/15 FSV, T1 PENDING), Case 2 Santa (EchoMimicV3 + LoRA 256/512/8
excluding q_audio + mouth-weighted flow-matching + four operating
thresholds, T1 PENDING), and Case 3 voice (Qwen3-TTS inference-only
+ 50-clip centroid + WavLM best-of-12 at T=0.3, T1 MEASURED 0.961
mean / 0.975 max, DNSMOS 3.93 / UTMOS 3.012). All ten weaknesses
from agent-24 are addressed either via remediation runs (W1
cross-encoder, W2 pre-selection distribution, W3 $n_m$ curve) or
paste-ready section-level disclosure language (W4–W10), with W1 /
W2 as CRITICAL priorities and W4 session-audit as the single
must-resolve-at-writing-time item.

## Confidence

**HIGH** that the methodology package is complete,
re-implementable, and publishable. **HIGH** that the weakness
remediation plan addresses every item in agent-24's register with
explicit run-or-disclose pairs. **MEDIUM** on feasibility of
stretch remediations (W1 cross-encoder, W2 ablation, W3 $n_m$
curve, W5 MI run, W7 Case 2 end-to-end) landing before submission;
the plan is robust to none of them landing (the paper still stands
on the T1-measured Case 3 numbers with explicit scope disclosure).

## Changelog

- 2026-04-17 — Initial SMAP by agent-25-method-designer.
