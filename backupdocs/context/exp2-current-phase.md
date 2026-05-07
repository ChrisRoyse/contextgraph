---
namespace: context
agent_id: coordinator-exp2-verification
timestamp: 2026-04-28T22:50:00Z
status: active
tags: exp2, phase, sequential-pipeline
---

# EXP-2 Verification — Current Phase Plan

## Sequential Agent Pipeline (synchronous, dependency-ordered)

```
Phase 0 (coordinator): SMAP context written → ./memory/context/exp2-{mission,constraints,current-phase}.md
                       (THIS PHASE — DONE)
       ↓
Phase 1 (agent-01-static-audit): Static audit of scripts/exp2/*.py against docs/exp2/*.md
       ↓
Phase 2 (agent-02-tester): pytest + synthetic Gaussian validation (T-013 substitute)
       ↓
Phase 3 (agent-03-sherlock): Live forensic on running MCP daemon, RocksDB CFs, env verifier
       ↓
Phase 4 (agent-04-simplifier): Review of agents 1-3 + refinement proposals
       ↓
Phase 5 (coordinator): Personal verification + final synthesis to operator
```

## Why Sequential, Not Parallel

Each agent's input is the previous agent's `./memory/handoff/` output. Parallelism would force premature work without prior context. (Boris flow §2: "One tack per subagent for focused execution.")

## Tracking Table

| # | Agent ID | Status | Report |
|---|---|---|---|
| 0 | coordinator-exp2 | running | (this campaign) |
| 1 | agent-01-static-audit | pending | — |
| 2 | agent-02-tester | pending | — |
| 3 | agent-03-sherlock | pending | — |
| 4 | agent-04-simplifier | pending | — |

## Pipeline-Complete Criteria

- Every agent reported "Task Completed: yes | partial" with rationale.
- Every claimed memory file exists and is readable.
- Every claim about code or state is independently verified by coordinator before being relayed to operator.
- A consolidated truth table of what works, what doesn't, and the GPU blocker is in `./memory/reports/`.
