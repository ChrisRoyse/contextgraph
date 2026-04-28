---
namespace: handoff
agent_id: agent-24-causal-simulator
timestamp: 2026-04-17T07:30:00Z
status: active
tags: handoff, agent-25, method-designer, counterfactuals-ready, weakness-register, ablations-required
---

# Handoff: Agent-24 (causal-simulator) → Agent-25 (method-designer)

## Read these files FIRST, in this order

1. `./memory/discoveries/agent-24-causal-simulator--counterfactual-analyses.md`
   — per-hypothesis counterfactual stress test with residual
   probabilities and verdict per hypothesis.
2. `./memory/discoveries/agent-24-causal-simulator--weakness-register.md`
   — 10 confirmed weaknesses (3 CRITICAL, 3 HIGH, 3 MEDIUM, 1 LOW)
   with disclosure venues and remediation costs.
3. `./memory/decisions/agent-23-hypothesis-generator--hypotheses.md`
   — LOCKED hypothesis ledger.
4. `./memory/patterns/agent-23-hypothesis-generator--evidence-map.md`
   — per-hypothesis evidence pointers.

## What agent-24 did

1. Read agent-21 theory spine, agent-22 master architecture, and
   agent-23 hypothesis ledger + evidence map.
2. Ran six-question counterfactual stress test on each of the 13
   hypothesis entries (H1, H1a, H1b, H2, H2a, H2b, H2c, H2d, H3,
   H3a, H3b, H4, H5):
   - Counterfactual (what if H is false?).
   - Observable signature of falsity.
   - Do our measurements exclude the counterfactual?
   - Circular reasoning check.
   - Reverse causality check.
   - Spurious correlation check.
3. Scored each hypothesis post-stress and updated confidence
   values: most dropped 10–25 points vs. agent-23 ledger values.
4. Identified **three CRITICAL weaknesses** (W1 encoder-matched
   evaluation, W2 missing LoRA-only ablation, W7 no $M>1$
   evidence for multi-modal strictness).
5. Built a cross-hypothesis failure-propagation DAG showing how
   axiom failures / measurement failures cascade.
6. Produced a weakness register with 10 entries, each with
   disclosure venue and remediation cost.

## Headline findings for agent-25 (method-designer)

### Which hypotheses survived counterfactual stress?

- **H1, H1a, H1b** (DDA signal count): survive as **upper bound**
  claims. "91 signals/input" must become "up to 91".
- **H2 construction claim** (guard is deterministic, no aux
  model): survives.
- **H2a evidential value as identity proof**: **does not survive**
  without cross-encoder disclosure — the guard and evaluator are
  the same encoder (W1). Survives as "within-WavLM-family
  agreement" only.
- **H2b multi-modal strictness**: survives at $M=1$ trivially, no
  $M>1$ evidence (W7).
- **H3 constructive theorem**: survives; it's nearly a tautology
  under the TCT spec.
- **H3a empirical anchor**: survives, with the caveat that we
  cannot attribute the 0.961 to TCT vs. LoRA without the ablation
  (W2).
- **H4 scope argument**: survives; scope boundary needs disclosure
  (W9).
- **H5 taxonomic claim**: survives as semantic; the "dimensionally
  distinct" wording is an overclaim (W8).

### Which hypotheses remain partially open?

- **H2c Case 1 text-style threshold prediction**: T3 pending; Case
  1 T1 measurement required.
- **H2d Case 2 multi-modal threshold matrix**: T3 pending; widest
  uncertainty range.
- **H3b Case 2 $n_m = 2,362$ projection**: T3 pending; Case 2 T1
  measurement required.
- **Axiom 3 $n_m$ range**: single operating point; curve unknown
  (W3).
- **Axiom 2 pairwise embedder redundancy**: unmeasured (W5).

### The strongest weakness we must disclose

The Case-3 pipeline's headline number (0.961 mean SECS) is
encoder-matched: the guard uses WavLM-base-plus-sv cosine $\geq
0.95$, and the evaluator uses WavLM-base-plus-sv SECS. Selecting
the argmax over $K=12$ candidates by guard cosine and then scoring
with the same encoder is a **selection-on-the-outcome** pattern.
The number we report is the number the selection induces by
construction; it is not independent evidence of identity quality.

This must be:
(a) disclosed crisply in §6.3 Case 3 and §8 Limitations;
(b) remediated if possible with a cross-encoder run (ECAPA-TDNN
    or Resemblyzer);
(c) compounded with the absent LoRA-only ablation (W2), which
    further prevents causal attribution of the result to TCT
    versus the LoRA.

If agent-25 can budget one experimental run to add to the Case-3
replication plan, it should be the cross-encoder evaluation. That
single run would transform the empirical spine from "within-encoder
circularity risk" to "cross-encoder triangulated".

## Open items forwarded to agent-25

### Must-add runs (in priority order)

1. **Cross-encoder evaluation on Case 3** (remediates W1).
   - Same 10 held-out English sentences.
   - Same best-of-$K=12$ samples already accepted by the WavLM
     guard.
   - Additional scorer: ECAPA-TDNN **or** Resemblyzer (prefer
     both).
   - Report mean SECS under the non-WavLM scorer(s) in §6.3.

2. **LoRA-only vs. LoRA+TCT ablation on Case 3** (remediates W2).
   - Report the SECS histogram of **all 12** best-of-$K$
     candidates per held-out sentence, not just the argmax that
     passes the guard.
   - Compute mean raw SECS across all candidates.
   - Compare to post-selection mean (0.961).
   - Report the TCT-delta (post-selection − pre-selection mean) in
     §6.3.

3. **Minimal $n_m$ ablation** at Case 3 (remediates W3).
   - $n_m \in \{10, 25, 50, 100\}$ at minimum.
   - For each, construct centroid, run best-of-$K=12$ at $\tau =
     0.95$, report mean SECS.
   - Even three additional points change the claim from "single
     anchor" to "monotone curve".

4. **Held-out-session audit** on Case 3 (remediates W4).
   - Confirm whether the 10 held-out sentences are from the same
     recording session as the 50 reference clips.
   - If yes, disclose as L-03.
   - If no, document the cross-session status as a strengthener.

5. **RO-8 mini-measurement** on Context Graph $\Phi$ (remediates
   W5).
   - Sample 100–1000 inputs from the production corpus.
   - Compute cosine-correlation and/or MI estimate on all $\binom{13}{2} = 78$ embedder pairs.
   - Report redundancy histogram in an appendix.
   - Even a back-of-envelope estimate dramatically strengthens
     H1a.

### Must-disclose items (no additional runs needed)

6. **§3.4 wording change**: "91 signals/input" → "up to 91
   signals/input" with H1b hedge.
7. **§5.2 wording change**: "dimensionally distinct" → "semantically
   distinct / measured as structured-signal density".
8. **§3.3 scope boundary**: DDA scope argument addresses
   generator-in-loop recursion specifically; other forms of
   embedder-mediated data selection are out of those specific
   theorems' scope.
9. **§7 Discussion attribution**: credit frozen-embedder
   conditioning literature (cluster-D retrieval / contrastive
   geometry) for sample-complexity collapse inheritance; TCT is
   the operational vehicle, not the originating cause.
10. **§5.3 guard footnote**: "deterministic modulo GPU floating-
    point reproducibility".

### Must-carry hedges (from agent-23 and upstream)

- V11 (approximate independence) on every H1 family mention.
- V18 (frozen / frozen-with-LoRA) on every $\phi_m$ mention.
- V22 / R4 (measured / projected) on every T1/T3 number.
- R3 (pool-relative) on every "no cited method" / "no aux learned
  model" clause.
- Bias-05 / Bias-06 (single subject, English-only) on H2a / H3a.
- Theme D (single-encoder-pair) on H2a / H3a — now **upgraded** to
  an explicit cross-encoder disclosure requirement, not just a
  carried hedge.

## Updated confidence scorecard (post-stress-test)

| H | Agent-23 conf. | Post-stress conf. | Drivers of change |
|---|---|---|---|
| H1 | 95% | 80% | W5 MI unmeasured |
| H1a | 92% | 78% | W5 |
| H1b | 90% | 90% | unchanged (hedge) |
| H2 | 92% | 75% | W1, W2 (confound & ablation) |
| H2a | 95% | 70% | W1, W4 (encoder-match + session-audit) |
| H2b | 94% | 85% | W7 (no $M>1$ data) |
| H2c | 75% | 70% | unchanged (T1 pending) |
| H2d | 65% | 60% | widest exposure |
| H3 | 85% | 65% | W2, W3, W6 (ablation + curve + attribution) |
| H3a | 95% | 70% | W1, W2, W3 |
| H3b | 70% | 60% | T1 pending |
| H4 | 90% | 85% | W9 (scope boundary) |
| H5 | 88% | 75% | W8 (dimensional overclaim) |

Confidence drops are **expected** given rigorous counterfactual
audit. They do not indicate the hypotheses are unsound; they
indicate the prior confidence values did not account for the
measurement gaps this audit surfaces. Remediation (cross-encoder
run, LoRA ablation, $n_m$ curve) would restore most of the drop.

## Dependencies agent-25 will pull forward

- **From agent-24 (this handoff)**: weakness register, ablation
  priority list, disclosure list, updated confidence scorecard.
- **From agent-23**: hypothesis ledger + evidence map +
  falsification harness per hypothesis.
- **From agent-22**: Figure-1 arrow map, variable dictionary.
- **From agent-21**: axioms, propositions, theorems.
- **From agent-14**: Bias-05 / Bias-06 (scope).
- **From agent-17**: Theme D (single-encoder-pair) — now
  upgraded to a required remediation (W1).
- **From agent-19**: RO-8 (H1b → W5), RO-5 (H2 calibration
  hygiene), RO-10 (H3 curve → W3).

## One-paragraph summary for agent-25

> Thirteen hypothesis entries were stress-tested via six-question
> counterfactual audit (counterfactual, observable signature,
> measurement exclusion, circularity, reverse causality, spurious
> correlation). The theoretical claims (H1 signal count, H4
> collapse silence, H5 meaning compression as category) survive as
> derivations under their axioms, with wording downgrades (W5, W8,
> W9) and a "up to 91" caveat on H1a. The measured claims (H2a
> mean SECS 0.961; H3a $n_m = 50$ anchor) have two CRITICAL
> weaknesses: **W1** the Case-3 evaluation uses the same encoder as
> the guard, inducing a selection effect that makes the 0.961
> number the signature of the guard working, not independent proof
> of cross-encoder identity quality; **W2** no LoRA-only ablation
> exists, so we cannot causally attribute the 0.961 to TCT versus
> the identity-locked LoRA. One HIGH weakness **W3** notes that
> Axiom 3's $n_m \in [10, 10^2]$ range is anchored by a single
> operating point at $n_m = 50$. Agent-25 should prioritise: (1)
> cross-encoder evaluation on Case 3, (2) LoRA-only ablation on
> Case 3, (3) minimal $n_m$ ablation at Case 3. These three runs
> would transform the empirical spine from "single anchor with
> disclosure gaps" to "triangulated anchor with cross-encoder
> validation, causal attribution, and monotone curve". The paper
> is publishable with disclosure alone; remediation where cheap
> will substantially strengthen the empirical claims.

## Register rules inherited (carry forward unchanged)

- MP-12 composition-not-invention.
- R3 pool-relative scope.
- R4 projected / measured / AE tier flagging.
- V11 approximate independence (now paired with RO-8
  measurement requirement).
- V18 frozen / frozen-with-LoRA.
- V22 measured-vs-projected.
- Bias-02 through Bias-11 agent-14 bias audit.
- Agent-17 Themes A–F including Theme D cross-encoder flag
  (upgraded from hedge to remediation).

## Quality-gate status delivered

| Gate | Status |
|---|---|
| 13 hypothesis entries stress-tested | **PASS** |
| Per-H counterfactual recorded | **PASS** |
| Per-H observable signature of falsity recorded | **PASS** |
| Per-H measurement-exclusion verdict | **PASS** |
| Per-H circular-reasoning check | **PASS** |
| Per-H reverse-causality check | **PASS** |
| Per-H spurious-correlation check | **PASS** |
| Weakness register with severities | **PASS** |
| Cross-hypothesis failure-propagation DAG | **PASS** |
| Updated confidence scorecard | **PASS** |
| Ablation priority list for agent-25 | **PASS** |
| Disclosure list with section anchors | **PASS** |
| No new primitives, no new citations | **PASS** |
| All upstream hedges preserved | **PASS** |

## Confidence

**HIGH** that the weakness register is complete relative to the
locked hypothesis ledger. **HIGH** that the CRITICAL items (W1,
W2, W7) will be found by an adversarial reviewer and must be
addressed. **MEDIUM** on remediation feasibility — all proposed
runs are technically tractable but require agent-25 to budget
experimental effort.

## Changelog

- 2026-04-17 — Initial handoff by agent-24-causal-simulator.
