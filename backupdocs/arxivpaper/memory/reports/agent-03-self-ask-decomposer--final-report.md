---
namespace: reports
agent_id: agent-03-self-ask-decomposer
timestamp: 2026-04-16T15:45:00Z
status: complete
tags: final-report, smap, self-ask, decomposition, arxiv
---

# Final Report — Agent-03 Self-Ask Decomposer

## Status

COMPLETE. 20 essential questions locked. Routing table locked. Knowledge-gap register
locked. Handoff to agent-04 written.

## Files written (all absolute paths)

1. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-03-self-ask-decomposer--essential-questions.md`
2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-03-self-ask-decomposer--question-to-section-map.md`
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-03-self-ask-decomposer--knowledge-gaps.md`
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-03-to-04--questions-locked.md`
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-03-self-ask-decomposer--final-report.md` (this file)

## Inputs consumed

- `memory/context/mission-brief.md`
- `memory/context/constraints.md`
- `memory/context/agent-order.md`
- `memory/patterns/agent-01-step-back-analyzer--guiding-principles.md`
- `memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md`
- `memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md`
- `memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md`
- `memory/discoveries/agent-02-ambiguity-clarifier--ambiguities-resolved.md`
- `memory/handoff/agent-02-to-03--vocabulary-locked.md`

## What was done (SMAP)

**Strategy.** Treat every load-bearing claim in the mission brief as a node; for each
node, generate the question(s) a skeptical cs.LG reviewer must see answered; route each
question to exactly one primary section and primary owner agent; enumerate every
question for which the answer is partial or missing, and call those out as gaps for the
downstream gap-hunter.

**Method.** Derived the 20-question set from the four core claims (DDA, meaning
compression, TCT, sample-complexity collapse) × three case studies × cross-cutting
concerns (reproducibility, ethics, forensics, related work, limitations, future work).
Cross-walked against glossary locks and failure modes to ensure no question tripped a
vocabulary violation (V1–V8) or invited a failure mode (FM-01…FM-10).

**Artifacts.** One locked decisions file (the question set itself), one patterns file
(question-to-section routing and section-indexed view), one discoveries file (nine
named gaps by severity), one handoff file to agent-04 (construct-definer), and this
report.

**Protocols followed.** P01 (evidence-tier tags per question), P03 (canonical
vocabulary only), P04 (T1–T4 labels), P05 (questions mapped to arXiv-canonical sections
§0–§9), P07 (every related-work question binds a specific novelty-delta), P09 (no
marketing, no AGI), P10 (Q17 explicitly covers forensic failures as assets). Vocabulary
rules V1–V25 respected throughout.

## 3-sentence summary

1. The three most load-bearing questions are **Q04 (DDA formal derivation of
   $n(N + \binom{N}{2})$ supervisory signals)**, **Q06 (TCT's runtime constellation
   guard as the structural delta vs. RLHF/DPO scalar-reward alignment)**, and **Q12
   (Case 3's T1-measured WavLM SECS 0.961 on 10 held-out sentences, the paper's only
   fully-measured study)** — every other question derives its force from one of these.
2. The most at-risk gap is **G03 (formal embedder-independence measurement for the 13
   Context Graph embedders)**: the DDA $\binom{N}{2}$ pairwise-interaction count sits
   on a V11 hedge ("approximately independent; strict orthogonality open") that cannot
   be closed with current evidence and must be carried into §8 Limitations and §9
   Future Work.
3. Four HIGH-severity gaps remain (G01 threshold calibration for Cases 1 and 2, G02
   T3-pending metrics for Cases 1 and 2, G03 formal embedder independence, G04 Case 3
   sample-size / cross-encoder breadth) — none block submission if honored with V22
   projected-flag discipline, but G02 is the closest thing to a camera-ready hinge.

## Confidence

HIGH.

## Reason for confidence

- Every question names (a) its primary section per the arXiv-canonical layout locked in
  P05, (b) its evidence source (either the four evidence briefs, the SITREP, the
  inspect / forensic files, or a canonical glossary entry), (c) its owner agent from the
  agent-order DAG, (d) its evidence tier T1–T4, and (e) an explicit answerability label.
- Coverage matrix (appendix of the decisions file) shows every core claim and every
  case study is supported by ≥ 2 questions; no orphaned claim and no orphaned question.
- Vocabulary conformance: the 20 questions use only canonical glossary terms; no
  "prototype" / "anchor" / "identity-preserving" / bare "SECS" / "semantic compression"
  / forbidden superlative appears anywhere.
- Gaps are enumerated separately and severity-labeled; the gap-hunter (agent-18) has a
  prioritized chase list rather than having to reverse-engineer one from prose.
- Handoff to agent-04 is specific: exactly four questions (Q04, Q05, Q06, Q15) bind
  agent-04's definition file; the rest route elsewhere, preventing scope creep.

## Residual risks and how they are mitigated

- **Risk: agent-04 re-opens a definition rather than reading glossary #1–#4.**
  Mitigation: the handoff file names the specific glossary entries to quote verbatim
  and forbids silently extending vocabulary (must file a blocker instead).
- **Risk: downstream writers hide T3-pending numbers.** Mitigation: Q13 is the only
  Case 1 / Case 2 numeric question and is explicitly tagged PARTIAL; the routing table
  places §6.1 / §6.2 on agent-38 who must apply V22 flags per P01.
- **Risk: §2 Related Work explodes into a literature survey.** Mitigation: Q02 / Q03 /
  Q09 each specify a single novelty delta per prior-work line per P07; the question
  set bounds the section's scope.

## Suggested next action for coordinator

Dispatch agent-04 (construct-definer) with the handoff file
`memory/handoff/agent-03-to-04--questions-locked.md` as its primary input. Agent-04's
output will bind agent-21 (theory-builder) and agent-25 (method-designer), which in
turn unblock the §3 / §5 writers.
