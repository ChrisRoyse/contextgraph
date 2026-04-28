---
namespace: patterns
agent_id: agent-39-discussion-writer
timestamp: 2026-04-16T22:45:00Z
status: active
locked: false
supersedes: agent-39-discussion-writer--writing-decisions.md (prior 2026-04-17T14:15:00Z revision)
tags: writing-decisions, discussion, limitations, section-7-seven-subsections, section-8-eight-limitations, hedges-carried, banned-phrases-avoided
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/discoveries/agent-30-risk-analyst--fmea-risk-register.md
  - memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md
  - memory/discoveries/agent-33-falsification-agent--counterarguments.md
  - memory/patterns/agent-33-falsification-agent--claim-softening-recommendations.md
  - memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-38-to-39--results-ready.md
downstream_consumers:
  - agent-40-limitations-writer (informational; §8 assigned to me per the agent-39 brief)
  - agent-41-conclusion-writer
  - agent-42-adversarial-reviewer
---

# Agent-39 Writing Decisions (Revised) — §7 Discussion + §8 Limitations

## 0. Scope and revision note

This file supersedes the earlier 2026-04-17 revision. The earlier draft
used a six-subsection §7 (§7.1–§7.6) and a twelve-item §8 at
`sections/10_limitations.md`. The authoritative agent-39 brief that
drove this run specifies a **seven-subsection §7** (§7.1–§7.7, adding
a "§7.7 Honest failure modes" pass on the Case 2 flow-matching
sign-bug and the Case 1 echo-bug as evidence that forensic tooling is
load-bearing) and an **eight-item §8** (L1–L8) at
`sections/09_limitations.md`, with a mandatory forward-looking closer
naming EXP-1 (cross-encoder SECS), EXP-2 (pairwise MI), and EXP-3
(n_m sweep) and their expected confidence deltas. The prior
`10_limitations.md` is removed; the current authoritative limitations
file is `09_limitations.md`.

## 1. Arc of §7 Discussion (7 subsections, 2,386 words)

- **§7.1 Interpreting the Case 3 anchor.** What the 0.961 mean SECS
  supports (encoder-matched, single speaker, N=10) vs. what it does
  not support (cross-encoder triangulation; multi-speaker; multi-modal
  conjunction at M>1; sample-complexity curve). Three pending companion
  measurements named (ECAPA cross-encoder, pre-selection distribution,
  n_m curve).
- **§7.2 DDA counting bound as upper bound, not floor.** Empirical
  embedder overlap; pairwise MI audit (EXP-2) is the correct
  falsification test; blind-spot evidence puts the effective count
  strictly above N but not necessarily at N+C(N,2).
- **§7.3 TCT vs. scalar-reward alignment — different property, not
  benchmark contest.** Four-axis structural contrast. Explicit "We
  are not claiming TCT outperforms RLHF on RLHF benchmarks." Pool-
  relative (R3) on the 88-citation master pool. Per-output
  verifiability stated as panel-relative and predicate-level.
- **§7.4 Collapse-silence scope (DDA not contradicting Shumailov).**
  P2 categorical-scope argument. Two explicit boundary cases held
  out of scope (centroid recursion; embedder-mediated data curation).
  Inference-time selection vs. training-time recursion made the
  load-bearing distinction.
- **§7.5 Sample-complexity — constructive vs. empirical split.**
  C12a constructive theorem (≥95%) vs. C12b empirical (~40%) anchored
  at one operating point. Attribution discipline (W6): TCT
  operationalises the collapse implicit in well-conditioned frozen
  embedders.
- **§7.6 Implications.** Four readings (identity-locked generation;
  scaling-law policy; meaning compression as fourth accounting axis;
  enterprise data strategy — brief, non-marketing). EXT not BRK of
  compression-as-intelligence frame.
- **§7.7 Honest failure modes.** Case 2 flow-matching sign-bug and
  Case 1 training-data echo-bug as evidence that forensic tooling
  with the multi-embedder, multi-role panel is load-bearing at
  diagnosis time.

## 2. Arc of §8 Limitations (8 items, 1,398 words)

Eight enumerated limitations matching the brief's L1–L8 structure.

- **L1** Case 3 encoder-matched protocol (single encoder) — EXP-1
  remediation.
- **L2** Case 3 single speaker, one language — future multi-subject
  replication out of scope for v1.
- **L3** Cases 1 and 2 PROJECTED — two-track drafting (A/B)
  absorbs.
- **L4** Embedder independence is empirical, not proven — EXP-2
  remediation; V11 hedge.
- **L5** τ calibration per-panel and per-application — not universal;
  centroid stability vs. threshold calibration distinction (IC-4).
- **L6** Constellation degeneration risk under non-diverse references.
- **L7** Compute cost of inference-time posture — not production-ready
  for low-latency contexts.
- **L8** Commercial COI — author operates Teleox.ai / Leapable.ai;
  disclosed in Acknowledgements; no vendor-funded benchmarks.

Closing paragraph: EXP-1, EXP-2, EXP-3 with expected confidence
deltas (C5 0.98→0.90/reframe; C1 0.95→measured N_eff; C12b 0.40→
partial curve). This is the brief-mandated forward-looking sentence.

## 3. Hedges carried verbatim from agent-34 cue-sheet

| Template | Applied in |
|---|---|
| C1 "up to 91" / "up to 28" | §7.2, §7.6, L4 |
| C3 "per-output verifiable relative to the chosen panel" | §7.1, §7.3 |
| C4 "structurally not subject to Shumailov-2024 regime" | §7.4 |
| C5 "under the encoder-matched protocol" | every §7.1 mention; L1 |
| C6 "parity within evaluator noise, N=14" | §7.1 |
| C7 not applied (T=0.3 not discussed in §7/§8) | — |
| C12a/C12b constructive/empirical split | §7.5 |
| C13 pool-relative on RLHF contrast | §7.3 |
| C14 "research contribution, not marketing" | L8 |
| V11 "approximately independent; strict formal orthogonality open" | §7.2, L4 |
| V18 "frozen" | §7.4, L4 |

## 4. Banned phrases audited clean

Grep-audited on both files against the agent-34 §4 list. Zero matches
for: breakthrough, revolutionary, game-changing, state-of-the-art,
world-class, dimensionally distinct, T=0.3 is optimal, indistinguishable
from real speech (bare), proven collapse-free, TCT causes, proven novel,
statistically equivalent, solves the data wall, solves AGI. The string
"outperforms RLHF" appears exactly once in §7.3 inside the required
disclaimer sentence ("**We are not claiming TCT outperforms RLHF, DPO,
or Constitutional AI on RLHF benchmarks.**"); this is the permitted
C13 negated form, not a positive claim. "91 signals per input" always
carries the "up to" prefix. "0.961" always appears with encoder-matched
scope or inside an explicitly scoped clause.

## 5. Agent-30 top-10 risk coverage in §7 / §8

| Risk | Where addressed |
|---|---|
| R1-03 N=1 reviewer attack | §8 L2 + §7.1 does-not-demonstrate clauses |
| R1-04 encoder-matched circular | §7.1 + §7.3 panel-relative + §8 L1 |
| R2-02 Case 2 T1 pending | §7.1 M>1 caveat + §7.5 + §8 L3 |
| R2-01 Case 1 T1 pending | §8 L3 |
| R2-05 vendor weight drift | §8 L3 (two-track absorbs) |
| R1-05 closed artefact | §8 L8 |
| R1-06 meaning-compression rebrand | §7.6 EXT-not-BRK framing |
| R2-03 W1 cross-encoder < 0.95 | §8 L1 remediation clause |
| R2-04 W2 TCT-delta small | §7.5 attribution discipline |
| R2-06 pairwise-MI demand | §7.2 + §8 L4 |

## 6. Contradiction-register resolutions carried

- IC-1 third-path vs. composition-not-invention — §7.6 "in a strictly
  composition-not-invention sense".
- IC-2 sample-complexity headline vs. Case 2 pending — §7.5 C12a/C12b
  split.
- IC-3 per-output guarantee vs. encoder-matched — §7.1 + §7.3
  panel-relative/predicate-level + §8 L1.
- IC-4 frozen centroids vs. calibrated thresholds — §8 L5.
- IC-5 DDA collapse-silent vs. Phase-3 uses TTS — §7.4 inference-time
  selection vs. training-time recursion.
- IC-6 release protocol vs. withhold artefacts — §8 L8.
- EC-7 meaning compression vs. Delétang — §7.6 EXT-not-BRK.
- EC-8 Shumailov silent — §7.4.
- EC-9 RLHF — §7.3.
- EC-10 VALL-E 2 — not §7/§8 direct concern (handled §1 + §6.3).
- EC-11 commercial COI — §8 L8.

## 7. Agent-33 counterargument softenings applied

- CA-1 ("up to 91") — §7.2, §7.6, §8 L4.
- CA-2 (guard verifiability composition novelty) — §7.3.
- CA-3 (sample-complexity collapse from n=1) — §7.5 C12a/C12b split.
- CA-4 (selection ≠ training) — §7.4.
- CA-5 (meaning compression as fourth category, W8 softening) — §7.6
  says "a fourth accounting axis ... measured in a semantically distinct
  unit at a different pipeline stage".
- CA-6 (marketing ad-hominem) — §8 L8 ethics-motivated disclosure.

## 8. Agent-29 CONFIRMED critical/high validity threats addressed

- VT-I2 LoRA/Qwen3-TTS confound — §7.5 "selection operator" + §8 L1
  pending pre-selection distribution (EXP-1).
- VT-I3 WavLM instrumentation circularity — §7.1 + §8 L1.
- VT-I6 forensic-fix confound — §7.7 + §8 L3.
- VT-E1 single subject — §8 L2.
- VT-E2 single language — §8 L2.
- VT-C1 circular construct — §8 L1 + §7.3 predicate-level.
- VT-C2 DNSMOS/UTMOS proxy — §7.1 "within evaluator noise, N=14".
- VT-C6 DDA independence — §7.2 + §8 L4.
- VT-T1 N(N+1)/2 count under Axiom 2 — §7.2 + §8 L4.
- VT-T2 threshold calibration portability — §8 L5.
- VT-T3 single operating point — §7.5 + §8 L3.
- VT-T5 multi-modal M>1 absent — §7.1 does-not-demonstrate + §8 L3.

## 9. What §7 does NOT do

- Does not re-narrate §6 numbers. References 0.961, 0.975, 9/10,
  +0.015, 3.93/3.93, 3.012/2.997 with scope tags attached.
- Does not introduce new claims. Only reads H2/H3/H4/H5 against §6.
- Does not present RLHF-vs-TCT benchmark comparisons. Structural
  four-axis contrast only.
- Does not claim empirical collapse across modalities. C12a/C12b split.
- Does not claim cross-encoder identity verification from Case 3. L1
  scope carries throughout.

## 10. What §8 does NOT do

- Does not defend softened claims. Softening is in §3 / §5 / §6 / §7;
  §8 states the limitation bluntly and names remediation.
- Does not soften the headline numbers themselves; it states the scope
  within which they are meaningful.
- Does not promise a tighter count in camera-ready. "Up to 91" carries
  permanently until EXP-2 (RO-8) lands.
- Does not promise open-source release of Context Graph or ClipCannon;
  that is a commercial decision outside the manuscript's scope.

## Confidence

**HIGH** that §7 and §8 carry every agent-34 hedge where applicable,
address every agent-30 top-10 risk, resolve every agent-32
internal/external contradiction, apply every agent-33 counterargument
softening, and address every agent-29 CONFIRMED critical/high validity
threat.

**HIGH** that §7 contains the brief-mandated §7.7 failure-modes pass
(Case 2 flow-matching sign-bug + Case 1 echo-bug as evidence that
forensic tooling is load-bearing).

**HIGH** that §8 contains exactly eight limitations L1–L8 matching the
brief's enumeration and a closing sentence naming EXP-1, EXP-2, EXP-3
with expected confidence deltas (the brief-mandated forward-looking
closer).

**HIGH** that word counts fall within the brief targets: §7 at 2,386
words (brief target 1,800–2,400); §8 at 1,398 words (brief target
900–1,400).

## Changelog

- 2026-04-16 (revised) — Rewritten to match the authoritative agent-39
  brief: §7 restructured into seven subsections (adds §7.7); §8
  restructured into eight L1–L8 items; output file for limitations
  moved from `10_limitations.md` to `09_limitations.md`.
- 2026-04-17 (prior) — Initial issue of agent-39 writing decisions
  under an earlier brief using six subsections and twelve limitations;
  superseded.
