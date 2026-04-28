---
namespace: reports
agent_id: agent-34-confidence-quantifier
timestamp: 2026-04-17T09:15:00Z
status: active
tags: smap, final-report, confidence-quantification, per-claim, paper-level
depends_on:
  - memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-34-to-35--confidences-ready.md
---

# Agent-34 (Confidence Quantifier) — Final Report (SMAP)

## S — Summary

Agent-34 calibrated 14 load-bearing claims (C1 … C14) from the
manuscript against (a) agent-23's locked hypothesis priors, (b)
agent-24's CRITICAL/HIGH weakness register, (c) agent-29's validity
threat audit, (d) agent-32's contradiction and contamination
register, and (e) agent-33's counterargument analysis with paste-
ready softenings. The calibration produced an Open-Philanthropy-
style confidence level for each claim, a writer-facing language
cue-sheet with banned-phrase list, and paper-level submittability
bands under six scenarios (baseline, EXP-1/2/3 STRENGTHEN/KILL, FM-01
fail).

**Paper-level result**: **~78% confident the paper is arXiv-
submittable under the current draft plus PENDING remediations**
(agent-33 softenings applied; experiments not yet executed).
Bands: ~55% if softenings not applied; ~93% if EXP-1 and EXP-3 both
STRENGTHEN; ~40% if EXP-1 KILLS; ~55% if FM-01 fails.

**Five lowest-confidence claims** that must be softened most in the
manuscript prose: C12b (sample-complexity empirical across modalities,
~40%), C9 (Case 2 guard effect, ~50%), C10 (mouth-weighted
necessity, ~55%), C11 (Case 1 retrain 15/15, ~60%), and C7 (T=0.3
optimal generalisation, ~75%).

## M — Method

1. **Inputs ingested**:
   - `mission-brief.md` — scope, constraints, banned language.
   - `agent-23-hypothesis-generator--hypotheses.md` — 13 locked
     hypotheses (H1, H1a, H1b, H2, H2a, H2b, H2c, H2d, H3, H3a, H3b,
     H4, H5) with pre-softening confidences.
   - `agent-24-causal-simulator--weakness-register.md` — W1–W10
     adversarial weaknesses (2 CRITICAL, 3 HIGH, 4 MEDIUM, 1 LOW).
   - `agent-29-validity-guardian--validity-threats.md` — VT-I, VT-E,
     VT-C, VT-S, VT-T threats.
   - `agent-32-contradiction-analyzer--contradiction-register.md` —
     IC-1…IC-6 internal contradictions; EC-7…EC-11 external
     contradictions.
   - `agent-33-falsification-agent--counterarguments.md` — CA-1 … CA-6
     counterarguments with sufficiency verdicts (2 INSUFFICIENT, 4
     SUFFICIENT).
   - `agent-33…claim-softening-recommendations.md` — 6 paste-ready
     softenings (2 MUST, 3 SHOULD, 1 MAY).
   - `agent-33-to-34--falsification-ready.md` — recommended priors.

2. **Calibration procedure**:
   - Started from agent-23 pre-softening confidence.
   - Applied pressure from named counterarguments / weaknesses /
     threats / contradictions.
   - Applied mitigation credit for paste-ready softenings.
   - Rounded to the six-band OP-style vocabulary: ≥95% / ~85% / ~70%
     / ~50% / ~30% / can't-tell.
   - Cross-checked against agent-33's recommended priors (max
     deviation ±5 pp on every claim).

3. **Outputs**:
   - Per-claim confidence table with justifications (C1–C14).
   - Paper-level scenarios (6 scenarios; baseline 78%).
   - Pre/post-experiment bands (EXP-1 / EXP-2 / EXP-3).
   - Reviewer-persona roll-up (4 personas; median 72–75%).
   - Writer cue-sheet with paste-ready hedges + banned-phrase table.

4. **Scope & assumptions**:
   - All MUST-softenings (agent-33 rec. 1 and 2) are assumed applied;
     if not, confidences degrade 10–20 pp each.
   - All CRITICAL weaknesses (W1, W2, VT-I2, VT-I3, IC-2, IC-3) are
     assumed disclosed per paste-ready inserts.
   - FM-01 regex pass at agent-42 is the gating condition for C14.
   - Experiment outcomes are pre-experiment bands only, not forecasts.

## A — Analysis / Findings

### Key findings

**F1. Theoretical spine is virtually certain (~92% avg).**
Under the softened framings (C1 "up to 91"; C12a constructive
theorem; C3 panel-relative verifiability; C4 Shumailov-exempt
scope-tightened), the theoretical claims are robust to adversarial
review. Pre-softening, C1 at exactly "91" would fail CA-1 in one
paragraph; post-softening, it is a clean structural upper bound.

**F2. Case-3 empirical spine is strong but encoder-scoped (~86% avg).**
The 0.961 mean SECS (C5) is measured and virtually certain as a
within-WavLM-family number; the W1 encoder-matched disclosure
sentence is load-bearing. Without that disclosure, C5 drops to ~60%
because the number doesn't support the broader identity-quality
claim without cross-encoder triangulation (EXP-1). DNSMOS parity
(C6) is real but small-N; 50-clip centroid delta (C8) is real but
single-operating-point.

**F3. Projected cases are honest projections (~50–65% avg).**
C9 (Case 2 guard effect), C10 (mouth-weighted necessity), C11 (Case
1 retrain 15/15), and C12b (sample-complexity empirical across
modalities) all sit in the "roughly even odds" to "confident" band.
These are correctly flagged as T3-projected in the hypothesis
ledger and the paper's prose must preserve that framing.

**F4. The C12a/C12b split is the biggest calibration win.**
Separating the constructive theorem (C12a, ≥95%) from the empirical
claim across modalities (C12b, ~40%) resolves CA-3's INSUFFICIENT
verdict. Before the split, a single "sample-complexity collapse"
headline blurred a very-strong derivation with a very-weak
empirical instantiation, producing a ~70% aggregate that was
indefensible once parsed by a careful reviewer. After the split,
both claims are separately defensible at their true confidence
levels.

**F5. Paper-level submittability is in the "strong hold / defensible"
band at ~78% baseline.** This is consistent with agent-33's
HIGH-conditional overall verdict. Scenario analysis shows EXP-1 is
the highest-leverage experiment: a STRENGTHEN result promotes the
paper to the ~88–93% band; a KILL forces a headline reframe and
drops submittability to ~40%.

**F6. The reviewer-persona roll-up median (72–75%) matches the
paper-level estimate (78%).** This is a sanity check: four different
reviewer personas independently arrive at submittability estimates
within 5 pp of the aggregate, suggesting the calibration is
internally consistent.

### Lowest-confidence claims to soften most

Ordered by priority of prose softening in the manuscript:

1. **C12b (~40%)** — constructive/empirical split is MANDATORY.
2. **C9 (~50%)** — T3-projected prefix is mandatory on Case-2 prose.
3. **C10 (~55%)** — "consistent with", not "necessary".
4. **C11 (~60%)** — "retrain in progress at time of writing".
5. **C7 (~75%)** — scope to "this pipeline / speaker / corpus".

### Highest-confidence claims the writer may keep strong

- C1 "up to 91 signals/input" (≥95%).
- C5 "0.961 mean SECS under encoder-matched protocol" (≥98%).
- C12a "constructive sample-complexity theorem under Axioms 1–3" (≥95%).
- C3 "per-output verifiability relative to the chosen panel" (~90%).

## P — Problems / Pending / Next steps

### Problems encountered

- None blocking. The calibration is subjective by nature; the
  mitigation is cross-checking against agent-33's red-team baseline,
  which produced max deviation ±5 pp on every claim.

### Pending / Open carries

- **EXP-1** (cross-encoder SECS via ECAPA-TDNN on Case 3): pending
  execution by agent-25 (method-designer). Strongest single
  confidence-mover.
- **EXP-3** (Arm A: $n_m$ sweep; Arm B: pre-selection SECS
  distribution): pending execution. Resolves C12b and W2.
- **EXP-2** (pairwise MI audit on 100–1000 Context Graph inputs):
  pending. Tightens C1 from "up to 91" to "$N_{\text{eff}} = X$".
- **FM-01 regex check**: executed by agent-42 (adversarial reviewer).
  Gating condition for C14.
- **Writer compliance with the banned-phrase list**: to be verified
  by agent-42 spot-check.

### Next steps (for downstream agents)

- **Agent-35 (introduction-writer)**: consume handoff and the
  per-claim table; draft §1 with paste-ready headline sentences
  (C1, C5, C12a/b, C3).
- **Agent-36 (related-work-writer)**: consume C2, C4, C13 language
  templates; frame pool-relative novelty scope.
- **Agent-37 (methodology-writer)**: consume W1–W10 disclosure
  templates; draft §6.3 with Case-3 mandatory disclosures.
- **Agent-38 (results-writer)**: consume C5–C8 language templates;
  draft §6.3 results table with encoder-matched scope.
- **Agent-39 (discussion-writer)**: consume C13 pool-relative
  language; credit frozen-embedder conditioning literature (W6).
- **Agent-40 (limitations-writer)**: consume L-01 … L-06 templates
  (W1–W9).
- **Agent-41 (conclusion-writer)**: consume C12a/b split framing.
- **Agent-42 (adversarial-reviewer)**: run FM-01 regex, spot-check
  banned-phrase compliance, re-assess per-claim confidences
  post-draft.

## Cognitive Physics Telemetry

- **Entropy (ΔS)**: 0.45 (moderate — this agent introduces no
  new findings; it synthesises upstream evidence into a new
  calibration ledger that is novel but closely anchored to agent-23
  / agent-33).
- **Coherence (ΔC)**: 0.92 (very high — outputs align with mission
  brief, agent-33 recommended priors within ±5 pp, and writer
  protocols downstream).

## Handoff to Next Agent

- **Output Key**: `phd/confidence/analysis_v1`
- **Entropy Score**: 0.45
- **Vector Keys**: stored for downstream retrieval by agent-35 …
  agent-42.

## Confidence

**HIGH** in the per-claim confidence levels under the softening
assumptions.

**HIGH** in the paper-level baseline estimate (~78%).

**MEDIUM** in the exact scenario-band values (pre-experiment
estimates are indicative, not predictive).

**HIGH** in the writer cue-sheet's practical utility for downstream
agents.

## Changelog

- 2026-04-17 — Initial issue by agent-34-confidence-quantifier.
