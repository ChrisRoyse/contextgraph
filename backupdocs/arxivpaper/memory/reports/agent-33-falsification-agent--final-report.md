---
namespace: reports
agent_id: agent-33-falsification-agent
timestamp: 2026-04-17T08:05:00Z
status: active
locked: false
supersedes: none
tags: smap, final-report, falsification, adversarial, red-team, summary
depends_on:
  - memory/discoveries/agent-33-falsification-agent--counterarguments.md
  - memory/discoveries/agent-33-falsification-agent--falsification-experiments.md
  - memory/patterns/agent-33-falsification-agent--claim-softening-recommendations.md
  - memory/handoff/agent-33-to-34--falsification-ready.md
---

# Agent-33 Falsification-Agent — Final Report (SMAP)

## Situation

Agent-33 entered the arXiv-manuscript pipeline as the **epistemic
red team**. Prior agents had built the theoretical framework
(agent-21), the hypothesis ledger (agent-23), the weakness register
(agent-24), the contradiction register (agent-32), and the ethics
posture (agent-31). The manuscript's spine was structurally coherent
but had not yet been subjected to adversarial stress testing
specifically designed to find the **strongest possible reviewer
attack** for each load-bearing claim.

The mission brief specified six counterarguments to construct and
answer:
1. Against H1 (DDA signal count): independence is assumed not
   measured.
2. Against H2 (TCT guard): cosine threshold is classical, not novel.
3. Against H3 (sample-complexity collapse): theorem from one data
   point.
4. Against H4 (collapse silence): best-of-$K$ is a recursion.
5. Against H5 (fourth compression category): reframing multi-modal
   or augmentation.
6. Against the overall paper: commercial marketing dressed as
   research.

And to propose three experiments that would kill or strengthen the
core claims.

## Mission

Four deliverables, all written to `./memory/`:

1. An adversarial counter-argument register (CA-1 … CA-6) with
   steelman evidence, our response, and a sufficiency verdict.
2. Three pre-registered falsification experiments with kill /
   strengthen / qualify criteria.
3. A paste-ready claim-softening recommendation set (MUST / SHOULD
   / MAY).
4. A handoff to agent-34 (confidence-quantifier) with pre-softening
   and post-softening confidence priors.

Plus this SMAP final report.

## Actions

### 1. Counterargument construction (CA-1 … CA-6)

For each of the six counterarguments, agent-33:
- **Steelmanned** the attack with the strongest evidence a hostile
  reviewer could marshal (shared pretraining corpora; classical
  speaker verification; $n=1$ extrapolation; selection-as-recursion;
  reframing-of-augmentation; FM-01 rejection risk).
- **Responded** with the best available defence: qualitative blind-
  spot arguments + future-work MI; compositional novelty (multi-modal
  + frozen + runtime); constructive vs. empirical theorem separation
  + Axiom 3 anchor; selection-is-not-training + inference-time scope;
  unit analysis + no-new-inputs + multiple frozen encoders; explicit
  COI disclosure + FM-01 regex + ethics-motivated withholding.
- **Rendered a verdict** per counter:
  - CA-1 (H1 signal count): **INSUFFICIENT** as stated.
  - CA-2 (H2 guard): **SUFFICIENT** with scope tightening.
  - CA-3 (H3 sample-complexity): **INSUFFICIENT** as stated.
  - CA-4 (H4 collapse silence): **SUFFICIENT** with scope tightening.
  - CA-5 (H5 fourth category): **SUFFICIENT** with W8 language fix.
  - CA-6 (overall commercial framing): **SUFFICIENT** conditional on
    FM-01 regex pass.

### 2. Falsification-experiment design (EXP-1 … EXP-3)

Three high-leverage experiments, each with pre-registered kill /
strengthen / qualify criteria:

- **EXP-1 (P0): Cross-encoder SECS triangulation on Case 3.** Run
  ECAPA-TDNN, Resemblyzer, WavLM-large-sv alongside the incumbent
  WavLM-base-plus-sv on the same 10 held-out sentences. KILL if
  $\mu_{ECAPA} < 0.80$; STRENGTHEN if $\mu_{ECAPA} \geq 0.85$ and
  $\mu_{Resemblyzer} \geq 0.85$. Closes W1 CRITICAL and IC-3
  CRITICAL. ~3 days engineer + trivial GPU.
- **EXP-2 (P1): Pairwise MI audit on N=13 panel.** KSG estimate on
  64-D PCA projections of 1,000 production inputs; report
  $N_{\text{eff}}$. KILL if $N_{\text{eff}} < 20$; STRENGTHEN if
  $N_{\text{eff}} \geq 50$. Closes W5 HIGH; runs RO-8 at tractable
  scope. ~1 week engineer + modest CPU.
- **EXP-3 (P0): $n_m$ sweep + LoRA-only baseline.** Arm A ($n_m \in
  \{10, 25, 50, 100\}$); Arm B (LoRA-only mean pre-selection SECS);
  Arm C (best-of-$K$ without threshold). KILL Arm B if LoRA-only
  $\geq 0.93$; STRENGTHEN if LoRA-only $\leq 0.85$. Closes W2
  CRITICAL and W3 HIGH. ~1 week engineer + ~2 GPU-days.

### 3. Claim-softening recommendations (6 edits)

Paste-ready language edits for section writers:
- **MUST 1**: H1 / H1a "91 signals" → "up to 91 signals" (§3.4, §4.1,
  §1).
- **MUST 2**: H3 / Theorem 1 "collapses" → "constructive bound
  (unconditional) + empirical at one T1 operating point" (§1, §3.6,
  §5.4, §6.3, §9).
- **SHOULD 3**: H2 / Theorem 2 "novel predicate" → "novel
  composition of three properties" (§5.1, §5.3, §5.4).
- **SHOULD 4**: H5 "dimensionally distinct" → "semantically distinct
  at a different pipeline stage" (§3.1, §5.2).
- **SHOULD 5** (→MUST if FM-01 regex fails): research-register body
  prose + §Competing Interests disclosure enforcement (§4, §9, §CI).
- **MAY 6**: H4 "exempt" → "collapse-silent under training-recursion
  scope only" (§3.3, §8 L-05, §10).

### 4. Handoff to agent-34

Pre-softening and post-softening confidence priors for each
hypothesis (H1 through H5 and secondaries), with the rationale and
the experimental-contingency bands (KILL / STRENGTHEN per EXP-1, -2,
-3). Agent-34 uses these as calibration inputs for its confidence
roll-up methodology.

## Performance

### What was delivered

- **Five files** under `./memory/` as specified in the task brief
  (counterarguments, falsification-experiments, softening-
  recommendations, handoff, final report).
- **Six counterarguments** constructed at steelman strength.
- **Three experiments** with pre-registered decision criteria and
  cost estimates.
- **Six paste-ready softening recommendations** anchored to manuscript
  sections.
- **Handoff content** sufficient for agent-34 to run confidence
  quantification with explicit experimental contingencies.

### What was NOT delivered (scope discipline)

- No new primitives (MP-12); no new citations (R3).
- No modifications to locked constructs (agent-04), axioms (agent-
  21), or hypotheses (agent-23).
- No execution of the three experiments (they are designs; agent-25
  owns execution).
- No modifications to Figure 1 / Figure 2 / Figure 3 / Figure 4
  captions (agent-22 scope; agent-33 flagged at handoff).
- No formal confidence quantification (agent-34's scope; agent-33
  provided priors).

### Quality-gate verification

| Gate | Required | Delivered |
|---|---|---|
| Counterargument for each of six claims | 6 | 6 (CA-1 … CA-6) |
| Steelman evidence for each | YES | YES |
| Our response for each | YES | YES |
| Sufficiency verdict for each | YES | YES (INSUFFICIENT × 2, SUFFICIENT × 4) |
| Three experimental designs | 3 | 3 (EXP-1, EXP-2, EXP-3) |
| Pre-registered criteria per experiment | YES | YES (KILL / STRENGTHEN / QUALIFY) |
| Cost estimate per experiment | YES | YES |
| Softening recommendations | YES | 6 (2 MUST, 3 SHOULD, 1 MAY) |
| Paste-ready paragraphs | YES | YES (§-anchored) |
| Handoff to agent-34 with priors | YES | YES (pre-/post-softening; EXP contingencies) |
| No new primitives (MP-12) | YES | YES |
| No new citations (R3) | YES | YES |
| Consistency with agent-32 resolution inserts | YES | YES |
| Consistency with agent-24 weakness register | YES | YES |

## Key findings worth highlighting

### Two non-negotiable claim softenings

1. **H1 / H1a: "91 signals/input" becomes "up to 91"**. Without this
   edit, any reviewer who takes V11 seriously one-sentences the
   manuscript: "They assumed 78 pairwise cross-features carry
   non-redundant information but never measured MI. RO-8 is future
   work. The headline is an order-of-magnitude upper bound. The
   paper doesn't own its own hedge."
2. **H3 / Theorem 1: "sample-complexity collapse" becomes
   "constructive bound + one empirical operating point"**. Without
   this edit, a reviewer reads "theorem from n=1" and rejects; the
   theorem's constructive nature must be explicit, and Axiom 3 must
   be explicitly the non-trivial empirical content, with Case 3 as
   the one measured operating point and RO-10 as the open extension.

### Most important experiment to run before submission

**EXP-1** (cross-encoder SECS triangulation) is the single highest-
leverage experiment. It closes the W1 CRITICAL and IC-3 CRITICAL
exposures simultaneously, costs ~3 days of engineer time, and
converts the 0.961 headline from "within-WavLM-family agreement"
back into "identity-level agreement" (if it strengthens) or
discloses the encoder-matched artefact (if it kills). Either way the
paper gains integrity.

### Open conditional on FM-01 regex

CA-6 / EC-11 (commercial-framing attack) is neutralised if the body
text passes the FM-01 regex scan at agent-42. If it fails, claim-
softening recommendation 5 escalates from SHOULD to MUST, and
body-text rewrites follow. This is a submission-gating check agent-42
owns.

## Aggregate paper health after agent-33

With the six softening recommendations applied:

- **Zero real unresolvable contradictions** (carried from agent-32).
- **Zero undefended counterarguments**: every CA has a response and
  a sufficiency verdict; where response is INSUFFICIENT, the
  softening fixes it.
- **Three pre-registered empirical extensions** (EXP-1, EXP-2,
  EXP-3) that would resolve the remaining HIGH exposures.
- **Four load-bearing claims** defensible under scope discipline:
  H2 (compositional novelty); H4 (training-recursion scope); H5
  (fourth category at dataset-prep stage); overall paper (research-
  register body + COI disclosure).
- **Two load-bearing claims** softened to their defensible form:
  H1 (upper bound with RO-8 future work); H3 (constructive bound +
  one empirical point with RO-10 future work).

## Dependencies agent-33 consumed

- mission-brief.md (six counterarguments specified).
- agent-21-theory-builder--theoretical-framework.md (axioms,
  propositions, theorems; the object $C_P$).
- agent-23-hypothesis-generator--hypotheses.md (H1–H5 + secondaries;
  evidence tiers; falsification conditions).
- agent-24-causal-simulator--weakness-register.md (W1–W10).
- agent-32-contradiction-analyzer--contradiction-register.md (IC-1
  … IC-6; EC-7 … EC-11).
- agent-32-to-33--contradictions-ready.md (task specification).

## Dependencies agent-33 passes forward

Agent-34 (confidence-quantifier):
- counterargument register + sufficiency verdicts.
- claim-softening recommendations (pre-/post-softening priors).
- three experimental contingency bands.

Agent-37 (methodology-writer) / agent-39 (discussion-writer):
- six paste-ready language edits anchored to specific sections.
- three experimental designs to cite as named future work (if not
  executed in time).

Agent-42 (adversarial-reviewer):
- counterargument register as a pre-flight checklist.
- FM-01 regex gate on EC-11 / CA-6.

Agent-43 (reproducibility-checker):
- three experimental protocols are pre-registered and ready for
  manifest-level documentation.

## Confidence

**HIGH** that the six counterarguments are the strongest reviewer
attacks in the space and that the sufficiency verdicts are correct.

**HIGH** that the two MUST softenings (Recs 1 and 2) are required
for publication-grade integrity.

**HIGH** that the three experimental designs would decisively
resolve the open exposures if executed.

**HIGH** that agent-34 has sufficient calibration material to
produce a rigorous confidence roll-up.

**MEDIUM** on whether agent-25 / the author will schedule EXP-1 and
EXP-3 before submission (resource decision outside agent-33's
scope).

**LOW** residual risk of an uncaught counterargument not covered by
CA-1 … CA-6. The six counterarguments were drawn from the task brief
and from the full upstream context; any additional counterargument
would almost certainly reduce to a variant of one of the six.

## Changelog

- 2026-04-17 — Initial final report by agent-33-falsification-agent.
