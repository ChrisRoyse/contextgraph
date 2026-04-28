---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-16T15:10:00Z
status: active
tags: phase, pipeline, agent-order
---

# Pipeline Phases — 48 Sequential Agents

Agents run **sequentially** (one at a time, each waits for prior). No parallelism.

## Phase 0 — Coordinator bootstrap (COMPLETE)
- Memory tree created, mission brief written, constraints locked, order planned.

## Phase 1 — Scoping & planning (5 agents)
1. `step-back-analyzer` — establish high-level principles
2. `ambiguity-clarifier` — resolve terminology ambiguities
3. `self-ask-decomposer` — 15–20 critical questions
4. `construct-definer` — define all constructs (DDA, meaning compression, TCT, constellation, ...)
5. `research-planner` — ReWOO full plan

## Phase 2 — Literature & evidence (8 agents)
6. `literature-mapper` — topology of the field (cs.LG/CL/CV scaling-laws + synthetic-data + multi-modal retrieval clusters)
7. `source-tier-classifier` — classify references into Tier 1/2/3
8. `citation-extractor` — harvest all citations with full metadata (already partially done in _evidence_*.md)
9. `methodology-scanner` — methodology patterns across related work
10. `theoretical-framework-analyst` — map theoretical frameworks
11. `systematic-reviewer` — PRISMA review across scaling-law + synthetic-data + multi-embedder work
12. `context-tier-manager` — organize sources hot/warm/cold
13. `quality-assessor` — CASP/JBI quality scoring

## Phase 3 — Analysis & synthesis (6 agents)
14. `bias-detector` — detect bias in cited works
15. `evidence-synthesizer` — meta-synthesis
16. `pattern-analyst` — cross-study patterns
17. `thematic-synthesizer` — thematic frameworks
18. `gap-hunter` — 15+ gaps
19. `opportunity-identifier` — novel research opportunities

## Phase 4 — Theory & model building (5 agents)
20. `interdisciplinary-pollinator` — isomorphic patterns across fields
21. `theory-builder` — theoretical framework for DDA/TCT
22. `model-architect` — conceptual/statistical model
23. `hypothesis-generator` — testable hypotheses
24. `causal-simulator` — counterfactuals, circular-reasoning check

## Phase 5 — Design (4 agents)
25. `method-designer` — research methodology
26. `sampling-strategist` — sampling strategy (for each case study)
27. `instrument-developer` — measurement instruments (SECS, DNSMOS, UTMOS, visual cosine, etc.)
28. `analysis-planner` — statistical/qualitative analysis plan

## Phase 6 — Quality & risk (6 agents)
29. `validity-guardian` — internal/external/construct/statistical validity
30. `risk-analyst` — FMEA risks
31. `ethics-reviewer` — IRB/ethics (identity-clone consent, PII, etc.)
32. `contradiction-analyzer` — contradictions (shadow search)
33. `falsification-agent` — adversarial counterexamples
34. `confidence-quantifier` — per-claim probability calibration

## Phase 7 — Writing (7 agents)
35. `introduction-writer` — §1
36. `literature-review-writer` — §2
37. `methodology-writer` — §5 (Methods / TCT protocol)
38. `results-writer` — §6 (Case study results)
39. `discussion-writer` — §7 (Discussion)
40. `conclusion-writer` — §9 (Conclusion)
41. `abstract-writer` — Abstract (last, after full draft)

## Phase 8 — Review & polish (7 agents)
42. `adversarial-reviewer` — red-team the manuscript
43. `reproducibility-checker` — verify method replicability
44. `apa-citation-specialist` — APA 7th formatting
45. `citation-validator` — verify every citation's URL + metadata
46. `file-length-manager` — split if >1500 lines
47. `graph-gardener` — prune/merge memory graph
48. `adaptive-orchestrator` — final swarm-health assessment

## After all 48 agents
- Coordinator assembles final `PAPER.md` from `sections/` + agent outputs.
- Coordinator writes `build.sh` script.
- Coordinator personally verifies every claim against source materials.
