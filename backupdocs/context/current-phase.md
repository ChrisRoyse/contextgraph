---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-14T16:30:00Z
status: active
tags: phase, pipeline, agents
---

# Current Phase: Full State Verification Pipeline

## Pipeline Ordering (Sequential — each waits for prior)

| # | Agent ID | Role | Depends On | Produces |
|---|----------|------|-----------|----------|
| 1 | agent-01-spec-reader | Explore | — | `memory/handoff/agent-01-to-02--system-map.md` with pre-work baseline counts extracted from 15 specs |
| 2 | agent-02-change-inventory | Explore | 01 | `memory/handoff/agent-02-to-03--change-inventory.md` listing every added/modified file with line counts |
| 3 | agent-03-source-forensics | sherlock-holmes | 01, 02 | `memory/discoveries/agent-03-*` for any code-vs-claim discrepancy; `memory/handoff/agent-03-to-04--verified-claims.md` |
| 4 | agent-04-live-mcp-fsv | coordinator (me, manual) | 03 | Live JSON-RPC handshake, tools/list count=71, store→export→read-back on synthetic fixtures |
| 5 | agent-05-edge-cases | sherlock-holmes | 04 | Edge-case audit for constellation + contrastive + training tools, before/after state proof |
| 6 | agent-06-math-identities | sherlock-holmes | 04 | Direct unit-level verification of identities through CLI/MCP with synthetic fixtures |
| 7 | agent-07-test-sweep | coordinator (manual) | 03,04,05,06 | Full `cargo test --workspace` re-run; cross-check 5473 claim |
| 8 | agent-08-code-simplifier | code-simplifier | 03,07 | Second-pass review on all new modules |
| 9 | agent-09-final-report | coordinator (manual) | all | `tasks/final_fsv_report_v2.md` replacing the prior report |

## Spawn Policy
- Agents 1–2 are independent of each other (can be parallel) but Agent 3 depends on both.
- Agents 5 and 6 both depend on Agent 4 but are mutually independent (parallel).
- Agent 8 waits for all code-verification agents to finish.
