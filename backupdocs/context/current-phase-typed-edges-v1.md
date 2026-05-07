---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-14T23:10:00Z
status: active
tags: phase, pipeline, agents, typed-edges
---

# Current Phase: Typed-Edge Training-Data Factory

## Pipeline Ordering (Sequential — each waits for prior)

| # | Agent ID | Role | Depends On | Produces |
|---|----------|------|-----------|----------|
| 1 | agent-01-architect | architect | — | `memory/handoff/agent-01-to-02--implementation-spec.md` with concrete file-by-file plan |
| 2 | agent-02-coder-core | coder | 01 | TrainingRecord v2 + LLMEdgeValidation + TypedEdgeTrainingRecord types in core crate |
| 3 | agent-03-coder-storage | coder | 01,02 | CF_TYPED_EDGE_RECORDS + CF_TYPED_EDGE_VALIDATIONS + derivation helpers in storage crate; update `export_training_corpus` to compute `edge_type_distribution` |
| 4 | agent-04-coder-mcp | coder | 01,02,03 | 4 new MCP tools wired via `tool_dispatch!` |
| 5 | agent-05-tests | tester | 02,03,04 | Integration tests for all 4 features, synthetic-fixture FSV |
| 6 | agent-06-fsv | coordinator (manual, me) | 05 | Live MCP FSV, physical CF inspection, synthetic happy-path + edge-case proof |
| 7 | agent-07-simplifier | code-simplifier | all | Second-pass review of all new modules |
| 8 | agent-08-final-report | coordinator (manual, me) | all | `tasks/typed_edges_fsv_report.md` + commit + push |

## Spawn Policy
- Sequential only. Each agent waits for the prior agent's handoff.
- I verify each agent's work (compile, inspect files, read code) before spawning the next.
- If any agent produces a blocker, I stop the pipeline and re-plan per borisflow Rule 1.

## Memory Namespace Reservation
All typed-edge work uses filenames prefixed `agent-{N}-*--typed-edges-*` to avoid name collisions with prior FSV pipeline files.
