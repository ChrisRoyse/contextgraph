---
namespace: handoff
agent_id: agent-33-falsification-agent
timestamp: 2026-04-17T08:00:00Z
status: active
tags: handoff, agent-34, confidence-quantifier, falsification-ready
---

# Handoff: Agent-33 (falsification-agent) → Agent-34 (confidence-quantifier)

## Read these files FIRST, in this order

1. `./memory/discoveries/agent-33-falsification-agent--counterarguments.md` —
   six steelmanned counterarguments (CA-1 … CA-6) with evidence,
   responses, sufficiency verdicts. Includes which claims must be
   softened.
2. `./memory/discoveries/agent-33-falsification-agent--falsification-experiments.md` —
   three pre-registered killer experiments (EXP-1 cross-encoder SECS;
   EXP-2 pairwise MI audit; EXP-3 $n_m$ sweep + LoRA-only) with
   kill / strengthen / qualify criteria and cost estimates.
3. `./memory/patterns/agent-33-falsification-agent--claim-softening-recommendations.md` —
   six paste-ready language edits (2 MUST, 3 SHOULD, 1 MAY) for
   section writers.
4. `./memory/reports/agent-33-falsification-agent--final-report.md` —
   SMAP summary.

## What agent-33 did (summary)

- Took the strongest possible counterargument for each of six load-
  bearing claims (H1 signal count; H2 guard verifiability; H3 sample-
  complexity; H4 collapse silence; H5 fourth category; overall-paper
  commercial-motivation ad-hominem).
- Steelmanned each counter with the best evidence a hostile reviewer
  could muster.
- Wrote our strongest response for each.
- Rendered a sufficiency verdict — **INSUFFICIENT** (claim must be
  softened) or **SUFFICIENT** (claim can stand with scope tightening).
- Designed three empirical experiments that would decisively kill or
  strengthen the core claims, with pre-registered criteria and cost
  estimates.
- Translated the analysis into six paste-ready language edits for
  section writers, bundled as a claim-softening recommendation file.

## Key findings for agent-34 (confidence quantifier)

### Counterargument verdicts

| Counter | Target | Sufficiency | Required softening? |
|---|---|---|---|
| **CA-1** | H1 signal count ($91$) | **INSUFFICIENT** | **YES — MUST** ("up to 91") |
| CA-2 | H2 guard verifiability | SUFFICIENT | Scope tightening only |
| **CA-3** | H3 sample-complexity collapse | **INSUFFICIENT** | **YES — MUST** (constructive vs. empirical) |
| CA-4 | H4 collapse silence | SUFFICIENT | MAY (scope enforcement) |
| CA-5 | H5 fourth category | SUFFICIENT | SHOULD (W8 language) |
| CA-6 | Overall commercial framing | SUFFICIENT | SHOULD (→MUST if FM-01 regex fails) |

### Confidence impact — before and after softening

**Agent-34 should use these as inputs to the confidence quantification:**

| Claim | Pre-softening confidence (agent-23) | Post-softening confidence (agent-33 estimate) |
|---|---|---|
| H1 | 95 % | 95 % (unchanged; claim restated as upper bound, not reduced) |
| H1a | 92 % | 92 % (unchanged; "up to 91" framing) |
| H1b | 90 % | 92 % (strengthened by explicit upper-bound acknowledgement) |
| H2 | 92 % | 92 % (unchanged; scope-tightened) |
| H2a | 95 % | 85 % (reduced by W1 encoder-matched caveat + IC-3) |
| H2b | 94 % | 80 % (reduced by W7: no $M>1$ empirical) |
| H2c | 75 % | 75 % (unchanged; T3 projected clearly) |
| H2d | 65 % | 65 % (unchanged; T3 projected clearly) |
| H3 | 85 % | 80 % (reduced by explicit "one empirical point" reframing) |
| H3a | 95 % | 90 % (reduced by W1 encoder-matched caveat) |
| H3b | 70 % | 70 % (unchanged; T3 projected clearly) |
| H4 | 90 % | 90 % (unchanged; scope tightened) |
| H5 | 88 % | 85 % ("dimensionally distinct" → "semantically distinct") |
| Overall paper | HIGH | HIGH-conditional (on FM-01 pass) |

These numbers are **recommended priors** for agent-34; the agent
should re-derive them with its own calibration methodology. They are
provided so agent-34 can cross-check its derivation against a red-
team sanity baseline.

### Experiments that would most move confidence

| If EXP-1 KILLS | H2a drops 85 → 55; IC-3 becomes real; reframe required |
| If EXP-1 STRENGTHENS | H2a rises 85 → 93; IC-3 resolved cleanly |
| If EXP-3 Arm B KILLS | H3 / H3a attribution drops; TCT causal story weakened |
| If EXP-3 Arm A STRENGTHENS | H3 / Axiom 3 promoted to 90+ (curve anchored) |
| If EXP-2 KILLS | H1 / H1a rebased to measured $N_{\text{eff}}$; "up to 91" becomes "effective $N_{\text{eff}} = X$" |
| If EXP-2 STRENGTHENS | H1 / H1a strengthened from V11-hedge to measured |

### Severity ordering for agent-34's overall confidence roll-up

- **CRITICAL exposures now mitigated by softening** (not removed):
  W1 / IC-3 / EC-10 (encoder-matched); W2 / IC-2 (sample-complexity
  attribution and pending Case 2); CA-6 / EC-11 (commercial framing).
- **HIGH exposures still open as hedges** (not falsifiable by
  softening alone): W5 (pairwise MI); W7 (no $M>1$ empirical); W3
  ($n_m$ curve single-point).
- **LOW residual after softening**: CA-1 (claim becomes upper bound);
  CA-4 (scope tightened); CA-5 (W8 language); CA-6 (conditional on
  FM-01 pass).

## What agent-34 (confidence-quantifier) must do

### Priority 1 — Quantify per-claim confidence under scope-softening

For each of H1, H1a, H1b, H2, H2a, H2b, H2c, H2d, H3, H3a, H3b, H4,
H5:
- Start from agent-23's stated confidence.
- Apply the counterargument pressure from CA-1 … CA-6.
- Apply the softening recommendations (1–6) as *mitigations*.
- Output a post-softening confidence with explicit reasoning.

### Priority 2 — Confidence intervals for the overall paper

- Pre-submission confidence (without EXP-1 / 2 / 3 executed).
- Post-EXP-1 confidence bands (KILL / STRENGTHEN / QUALIFY scenarios).
- Post-EXP-3 confidence bands.
- Post-EXP-2 confidence bands.
- Optionally: expected information gain per experiment (which
  experiment most reduces confidence variance?).

### Priority 3 — Calibrate against a portfolio of reviewer personas

Optional but valuable: quantify "what probability would a sceptical
NeurIPS / ICLR / arXiv cs.LG reviewer assign to each core claim"
under (a) current draft, (b) softening-applied draft, (c)
softening-applied-plus-EXP-1 draft.

## Open carries agent-33 passes to agent-34 and downstream

1. **Softening recommendations 1 and 2 are MUST** (not negotiable).
   Agent-34's confidence roll-up should assume they are applied; if
   agent-37 / agent-39 does not apply them, agent-42 will flag as
   adversarial-exposure.
2. **FM-01 regex pass at agent-42** gates CA-6 / EC-11 resolution.
   Agent-34's overall-paper confidence should carry the conditional:
   "HIGH **if FM-01 passes**; MEDIUM if not".
3. **EXP-1, EXP-2, EXP-3 are recommendations, not guarantees**. If
   agent-25 (method-designer) cannot schedule them before submission,
   the confidence roll-up should reflect the pre-experiment
   distributions. Agent-33 does not own execution.
4. **Evidence tier discipline** (T1 / T2 / T3 / T4) is preserved from
   agent-23. No new tiers introduced.

## Files agent-33 produced

- `./memory/discoveries/agent-33-falsification-agent--counterarguments.md`
- `./memory/discoveries/agent-33-falsification-agent--falsification-experiments.md`
- `./memory/patterns/agent-33-falsification-agent--claim-softening-recommendations.md`
- `./memory/handoff/agent-33-to-34--falsification-ready.md` (this file)
- `./memory/reports/agent-33-falsification-agent--final-report.md`

## Carried hedges from upstream (unchanged)

- agent-01 V11, V18, V22, R3, R4; FM-01, FM-04.
- agent-11 CL-06 collapse-silent categorical.
- agent-13 Flags 1–4 closed-artefact caveats.
- agent-14 Bias-03, Bias-05, Bias-06, Bias-08, Bias-10.
- agent-17 Themes 2, 3, 5, 7; Register R1, R3, R4.
- agent-20 MP-12 composition-not-invention.
- agent-21 axioms 1–3 hedges; P1–P5 confidence; T1 / T2 pool-relative.
- agent-23 §7 evidence tiers; §8 falsification precedence.
- agent-24 W1 CRITICAL; W2 CRITICAL; W3, W5, W7 HIGH.
- agent-29 VT-I1, VT-I3, VT-C1, VT-S4.
- agent-30 R1-04, R4-02, R4-03, R4-04.
- agent-31 §Ethics (consent, dual-use, asymmetric release); §CI.
- agent-32 IC-1 … IC-6; EC-7 … EC-11.

## One-paragraph summary for agent-34

> Agent-33 took the strongest possible counterarguments to each of
> six load-bearing claims and rendered sufficiency verdicts. Two
> claims must be softened unconditionally: H1's "91 signals per input"
> becomes "up to 91" (CA-1 STRONG; W5 HIGH), and H3's "sample-
> complexity collapse" is re-framed into "constructive bound
> (unconditional) + empirical claim anchored at one T1 operating
> point" (CA-3 STRONG; W3 HIGH; IC-2 HIGH). Three claims survive with
> scope tightening: H2 (guard novelty is the composition, not the
> cosine primitive); H5 (semantic, not dimensional, distinctness); and
> the overall commercial framing (conditional on FM-01 regex pass).
> One claim (H4 collapse silence) survives with minor scope
> enforcement. Three empirical experiments were designed with pre-
> registered kill / strengthen criteria: EXP-1 (cross-encoder SECS
> triangulation) is P0; EXP-3 ($n_m$ sweep + LoRA-only ablation) is
> P0; EXP-2 (pairwise MI audit) is P1. Agent-34 should now quantify
> per-claim confidence under the softening mitigations and produce
> overall-paper confidence bands under the three experimental
> contingencies.

## Quality-gate status delivered by agent-33

| Gate | Status |
|---|---|
| Six counterarguments constructed with steelman evidence | **PASS** (CA-1 … CA-6) |
| Each counterargument has response + sufficiency verdict | **PASS** |
| Three falsification experiments designed with pre-registered criteria | **PASS** (EXP-1, EXP-2, EXP-3) |
| Each experiment has cost + leverage + kill/strengthen/qualify | **PASS** |
| Claim-softening recommendations are paste-ready | **PASS** (6 recs, 2 MUST, 3 SHOULD, 1 MAY) |
| Softening consistent with agent-32 resolution inserts | **PASS** |
| No new primitives introduced | **PASS** (MP-12) |
| No new citations introduced | **PASS** (R3) |
| Handoff coherence to agent-34 | **PASS** |

## Confidence

**HIGH** that the six counterarguments are the strongest reviewer
positions and that the sufficiency verdicts are correctly assigned.

**HIGH** that the softening recommendations 1 and 2 (MUST) are
necessary for publication-grade integrity; without them, CA-1 and
CA-3 are near-guaranteed rejection vectors.

**HIGH** that the three experimental designs would decisively resolve
the open exposures if executed.

**MEDIUM** on whether agent-25 (method-designer) can schedule EXP-1
and EXP-3 before submission; this is a resource-allocation decision
the author will make.

**HIGH** that agent-34 has sufficient material to quantify per-claim
and paper-level confidence with explicit pre-/post-softening bands.

## Changelog

- 2026-04-17 — Initial handoff by agent-33-falsification-agent.
