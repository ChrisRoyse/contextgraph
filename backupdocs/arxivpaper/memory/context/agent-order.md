---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-16T15:10:00Z
status: active
tags: agent-order, pipeline, dag
---

# Agent Execution Order (48 sequential steps)

| # | agent_id | role | depends on |
|---|---|---|---|
| 1 | agent-01 | step-back-analyzer | — |
| 2 | agent-02 | ambiguity-clarifier | 1 |
| 3 | agent-03 | self-ask-decomposer | 1,2 |
| 4 | agent-04 | construct-definer | 2,3 |
| 5 | agent-05 | research-planner | 1–4 |
| 6 | agent-06 | literature-mapper | 5 |
| 7 | agent-07 | source-tier-classifier | 6 |
| 8 | agent-08 | citation-extractor | 6,7 |
| 9 | agent-09 | methodology-scanner | 6 |
| 10 | agent-10 | theoretical-framework-analyst | 6 |
| 11 | agent-11 | systematic-reviewer | 7,8,9 |
| 12 | agent-12 | context-tier-manager | 6,8 |
| 13 | agent-13 | quality-assessor | 7,8,11 |
| 14 | agent-14 | bias-detector | 13 |
| 15 | agent-15 | evidence-synthesizer | 13,14 |
| 16 | agent-16 | pattern-analyst | 15 |
| 17 | agent-17 | thematic-synthesizer | 16 |
| 18 | agent-18 | gap-hunter | 4,17 |
| 19 | agent-19 | opportunity-identifier | 18 |
| 20 | agent-20 | interdisciplinary-pollinator | 17 |
| 21 | agent-21 | theory-builder | 17,20 |
| 22 | agent-22 | model-architect | 21 |
| 23 | agent-23 | hypothesis-generator | 22 |
| 24 | agent-24 | causal-simulator | 23 |
| 25 | agent-25 | method-designer | 22,23 |
| 26 | agent-26 | sampling-strategist | 25 |
| 27 | agent-27 | instrument-developer | 26 |
| 28 | agent-28 | analysis-planner | 25,27 |
| 29 | agent-29 | validity-guardian | 25,28 |
| 30 | agent-30 | risk-analyst | 29 |
| 31 | agent-31 | ethics-reviewer | 25,26 |
| 32 | agent-32 | contradiction-analyzer | 15,17 |
| 33 | agent-33 | falsification-agent | 21,23 |
| 34 | agent-34 | confidence-quantifier | 15,29,32,33 |
| 35 | agent-35 | introduction-writer | all phase 1–6 |
| 36 | agent-36 | literature-review-writer | 15,17,10 |
| 37 | agent-37 | methodology-writer | 25,26,27,28 |
| 38 | agent-38 | results-writer | all case studies |
| 39 | agent-39 | discussion-writer | 38 |
| 40 | agent-40 | conclusion-writer | 39 |
| 41 | agent-41 | abstract-writer | 35–40 |
| 42 | agent-42 | adversarial-reviewer | 35–41 |
| 43 | agent-43 | reproducibility-checker | 37,38 |
| 44 | agent-44 | apa-citation-specialist | 8,41 |
| 45 | agent-45 | citation-validator | 44 |
| 46 | agent-46 | file-length-manager | 41 |
| 47 | agent-47 | graph-gardener | all |
| 48 | agent-48 | adaptive-orchestrator | all |

**Coordinator** (me) does NOT spawn any agent in parallel. Strictly sequential.
