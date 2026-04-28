---
namespace: handoff
agent_id: agent-01-step-back-analyzer
timestamp: 2026-04-16T15:15:00Z
status: active
tags: handoff, agent-02, ambiguity-clarifier, principles, criteria
---

# Handoff: Agent-01 → Agent-02 (ambiguity-clarifier)

## Read these files FIRST, in this order

1. `/home/cabdru/contextgraph/docs/arxivpaper/memory/context/mission-brief.md` —
   source of truth for the manuscript scope.
2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/context/constraints.md` — non-negotiable rules.
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-01-step-back-analyzer--guiding-principles.md`
   — the 10 first-principles (P01–P10) you must preserve in your ambiguity resolutions.
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-01-step-back-analyzer--evaluation-criteria.md`
   — the 9 gates (G1–G9); your output must not create work that fails a gate.
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md`
   — the 10 failure modes (FM-01–FM-10); your disambiguations must not enable any of them.

## What agent-01 did

Established the principles, evaluation gates, and failure modes that govern the entire
manuscript pipeline. All downstream agents (02–48) inherit these as binding constraints.

## What agent-02 (ambiguity-clarifier) must do

Identify every ambiguous term / concept in the mission brief and the prior manuscript that
could cause downstream drift, and resolve each to a single canonical meaning. Particular
attention to:

- **DDA vs synthetic data vs data augmentation** (see FM-02). Resolve the boundary cleanly.
- **"Meaning compression" vs weight-compression vs activation-compression** (see P03). The
  orthogonality claim must be crisp.
- **Centroid vs constellation vs reference set vs anchor** (see FM-09). One term, one meaning.
- **Identity-locked generation vs style transfer vs fine-tuning** (see the three case studies).
- **"Orthogonal signals"** in the DDA count — is this formal orthogonality or approximate
  independence? This has to be settled because the theory depends on it.
- **TCT vs contrastive learning vs metric learning** — TCT overlaps both; distinguish.
- **"Third path around the data wall"** — what exactly is the first path (more data) and
  the second (synthetic generation)? Precise framing required.
- **"Frozen embedder" vs "pretrained embedder"** — some embedders are frozen throughout
  training; some are frozen during TCT but tuned in pretraining; clarify.

## Constraints on agent-02's output

- Do NOT override any locked decision in `./memory/decisions/`.
- Each resolution must include: (a) the ambiguous term, (b) the possible meanings, (c) the
  chosen canonical meaning, (d) the rationale, (e) which failure mode(s) this disambiguation
  defuses.
- Write your output to `./memory/patterns/agent-02-ambiguity-clarifier--resolutions.md`.
- Write handoff to `./memory/handoff/agent-02-to-03--ambiguity-resolutions.md` pointing
  agent-03 at (i) your resolutions, (ii) this agent-01 output.
- Write final report to `./memory/reports/agent-02-ambiguity-clarifier--final-report.md`.

## Known prior-manuscript locations agent-02 should consult

- `/home/cabdru/contextgraph/docs/researchpaper/` — all 16 section files.
- `/home/cabdru/contextgraph/docs/researchpaper/_evidence_01_data_wall.md` — DDA / synthetic-data terminology.
- `/home/cabdru/contextgraph/tmp/SITREP_2026-04-16_135543.md` — Case 3 vocabulary.
- `/home/cabdru/contextgraph/tasks/newdata/alldata.md` — measured numbers whitepaper.

## Escalation

If agent-02 finds an ambiguity that cannot be cleanly resolved without touching the mission
brief itself, write a blocker at `./memory/blockers/agent-02-ambiguity--{term}.md` with
severity HIGH and suggested coordinator action. Do not silently pick one meaning; flag it.

## Quality bar

Agent-02 must leave zero load-bearing ambiguities for agent-03 (self-ask-decomposer) and
agent-04 (construct-definer). The success criterion is: agent-03 can write 15–20 critical
questions without hitting a "wait, what does X mean here?" blocker.
