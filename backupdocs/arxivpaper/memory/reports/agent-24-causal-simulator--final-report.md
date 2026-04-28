---
namespace: reports
agent_id: agent-24-causal-simulator
timestamp: 2026-04-17T07:40:00Z
status: active
tags: final-report, smap, counterfactuals, weakness-register, ablations-required
depends_on:
  - memory/discoveries/agent-24-causal-simulator--counterfactual-analyses.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/handoff/agent-24-to-25--counterfactuals-ready.md
---

# Agent-24 (Causal Simulator) — Final Report (SMAP)

## S — Situation

The hypothesis ledger from agent-23 locks 9 primary hypotheses (plus
4 sub-hypotheses) across 5 families: **F1** DDA signal count, **F2**
TCT geometric alignment, **F3** sample-complexity collapse, **F4**
collapse exemption, **F5** meaning-compression category. Every
hypothesis is anchored to an axiom / proposition / theorem from
agent-21 and to a Figure-1 arrow from agent-22. Every hypothesis
carries an explicit falsification condition.

Two hypotheses are T1-MEASURED (H2a Case 3 mean SECS 0.961; H3a
Case 3 $n_m = 50$). Three are T3-PROJECTED (H2c Case 1, H2d Case 2
matrix, H3b Case 2). The remaining are derivation-plus-
instantiation supported.

The agent-23 handoff explicitly asked agent-24 to:
(a) simulate single-failure collapse branches;
(b) simulate T3 bet failure paths;
(c) build the Axiom-2 null-hypothesis comparator list;
(d) stress-test role-composition (P4);
(e) stress-test the FP/FN curve (Q2 / RO-5 hygiene);
(f) validate the measurement-vs-projection audit.

No claude-flow / CausalMemory infrastructure is available; agent-24
used file-based analytical stress testing.

## M — Mission

For each hypothesis, run a six-question counterfactual audit:

1. **Counterfactual**: what does the world look like if H is false?
2. **Observable signature of falsity**: what would our measurements
   show?
3. **Do our measurements exclude the counterfactual?**
4. **Circular reasoning check**: does the defense of H invoke H?
5. **Reverse causality check**: could the arrow run backward?
6. **Spurious correlation check**: is there a third factor?

Be especially thorough on H2 (guard) and H3 (sample-complexity
collapse) — the paper's strongest empirical claims. Produce:
(i) per-hypothesis counterfactual analyses;
(ii) weakness register;
(iii) handoff to agent-25 method-designer with ablation priorities;
(iv) this final report.

Hedge. Do not declare victory on claims whose counterfactuals
cannot be excluded.

## A — Actions

### A1. Read upstream files

Read agent-21 theoretical framework, agent-22 master architecture,
agent-23 hypothesis ledger, agent-23 evidence map, and agent-23→24
handoff in full.

### A2. Per-hypothesis counterfactual audit

Wrote `discoveries/agent-24-causal-simulator--counterfactual-
analyses.md` with all 13 hypothesis entries subject to the six-
question template. Produced per-H verdict, residual counterfactual
probability, and post-stress confidence value.

### A3. Weakness register

Wrote `discoveries/agent-24-causal-simulator--weakness-register.md`
with 10 weakness entries (3 CRITICAL, 3 HIGH, 3 MEDIUM, 1 LOW),
each with disclosure venue and remediation cost.

### A4. Cross-hypothesis failure-propagation DAG

Embedded a causal DAG in the counterfactual analyses file showing
how axiom failures / measurement failures cascade across the
hypothesis ledger.

### A5. Handoff to agent-25

Wrote `handoff/agent-24-to-25--counterfactuals-ready.md` with
(a) must-add ablations priority-ordered;
(b) must-disclose items with section anchors;
(c) updated confidence scorecard;
(d) register rules inherited.

### A6. Final report (SMAP)

This file.

## P — Production

### P1. Files written

| Path | Purpose |
|---|---|
| `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-24-causal-simulator--counterfactual-analyses.md` | Per-hypothesis six-question counterfactual analyses + cross-hypothesis DAG + summary scorecard |
| `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-24-causal-simulator--weakness-register.md` | 10 weaknesses with severity, scope, disclosure venue, remediation cost |
| `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-24-to-25--counterfactuals-ready.md` | Handoff to method-designer with ablation priority list and confidence scorecard |
| `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-24-causal-simulator--final-report.md` | This SMAP |

### P2. Hypothesis verdicts

**Survive counterfactual stress (acceptable residual):**
- H1, H1a, H1b (DDA signal count as upper bound; "up to 91"
  wording required).
- H2 as construction claim (deterministic, no aux model).
- H4 as scope argument (requires tightened language).
- H5 as semantic taxonomic claim (requires "dimensionally
  distinct" downgrade).

**Partially open (major remediation recommended):**
- H2a, H3a as empirical anchors — currently exposed to encoder-
  matched selection effect (W1) and missing LoRA-only ablation
  (W2). Survive as within-encoder-family measurements; do not
  survive as cross-encoder identity proof without remediation.
- H3 as Axiom-3-dependent claim — single operating point at $n_m
  = 50$; curve unknown (W3).
- H2b multi-modal strictness — derivational only; no $M>1$
  evidence (W7).

**Pending T1 measurement (no stress test possible):**
- H2c (Case 1), H2d (Case 2 matrix), H3b (Case 2). Agent-25 must
  keep the T3-projection disclosures explicit.

### P3. Three-sentence summary (as required by the task)

> Of the 13 hypothesis entries in agent-23's locked ledger, **H1
> (with "up to 91" wording), H4 (as scope argument with tightened
> language), H5 (as semantic taxonomic claim), and H2 as a
> construction claim (deterministic guard, no auxiliary learned
> model) survive counterfactual stress** with acceptable residual
> probability. **H2a and H3a remain partially open as empirical
> anchors** because the Case-3 guard (WavLM-base-plus-sv cosine
> $\geq 0.95$) and the Case-3 evaluator (WavLM-base-plus-sv SECS)
> are the same encoder family, inducing a selection effect that
> makes the 0.961 mean SECS the signature of the guard working
> rather than independent cross-encoder identity proof; and no
> LoRA-only ablation exists, preventing causal attribution of
> the 0.961 to TCT versus the identity-locked LoRA. **The
> strongest weakness we must disclose is W1 — encoder-matched
> evaluation on Case 3** — which can be remediated with one
> cross-encoder (ECAPA-TDNN or Resemblyzer) run on the same
> held-out set, and this single run would transform the empirical
> spine from "single anchor with circularity risk" to "cross-
> encoder triangulated".

### P4. Ablation priority for agent-25

1. Cross-encoder evaluation on Case 3 (remediates W1) — **highest
   ROI**.
2. LoRA-only vs. LoRA+TCT ablation on Case 3 (remediates W2).
3. $n_m \in \{10, 25, 50, 100\}$ ablation at Case 3 (remediates
   W3).
4. Held-out-session audit on Case 3 (remediates W4).
5. RO-8 mini-measurement on Context Graph $\Phi$ (remediates W5).

### P5. Disclosure list (no runs required)

- §3.4 "91 signals/input" → "up to 91".
- §5.2 "dimensionally distinct" → "semantically distinct".
- §3.3 scope boundary: generator-in-loop recursion specifically.
- §7 Discussion credit frozen-embedder conditioning literature.
- §5.3 guard footnote on GPU fp determinism.
- §6.3 encoder-match disclosure at the 0.961 headline.
- §6.3 LoRA-vs-TCT attribution caveat.
- §8 Limitations L-01 through L-05 (W1, W3, W4, W5, W7).

### P6. Confidence scorecard (updated)

Drops of 10–25 confidence points across the board are the
expected outcome of rigorous adversarial audit. No hypothesis
falls below 60% post-stress, which means none is fatally
compromised. The three CRITICAL weaknesses (W1, W2, W7) are
disclosable in their current state and remediable with cheap
experimental runs.

### P7. Confidence

**HIGH** that the per-H counterfactual audit is complete relative
to the locked ledger. **HIGH** that the three CRITICAL weaknesses
will be identified by any serious reviewer and must be addressed
either by disclosure or remediation. **MEDIUM** on feasibility of
all proposed ablations being runnable in the current publication
window — that is an agent-25 decision.

### P8. Files that should update when remediation runs return

If agent-25 runs the cross-encoder evaluation and the LoRA-only
ablation, then:
- `counterfactual-analyses.md` H2a / H3a verdicts should be
  revised upward to reflect actual cross-encoder data.
- `weakness-register.md` W1 / W2 severities should downgrade from
  CRITICAL to MEDIUM (or close if the new evidence is clean).

These revisions are out of scope for agent-24 and are correctly
scoped to the cycle following agent-25 experimental results.

## Summary of file paths (as required by instructions)

- `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-24-causal-simulator--counterfactual-analyses.md`
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-24-causal-simulator--weakness-register.md`
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-24-to-25--counterfactuals-ready.md`
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-24-causal-simulator--final-report.md`

## Changelog

- 2026-04-17 — Initial issue by agent-24-causal-simulator.
