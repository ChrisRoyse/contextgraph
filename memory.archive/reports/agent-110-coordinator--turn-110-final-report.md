---
namespace: reports
agent_id: agent-110-coordinator
timestamp: 2026-05-08T11:33:00Z
status: active
tags: report, final, turn-110, mcp-bounced, fresh-attempt-running-on-new-binary
---

# Turn 110 — fresh attempt running with combined fixes; MCP bounced on drip

## Status

Operator restored MCP, said "continue loop". I dripped — got "MCP error -32000: Connection closed" but engine **DID launch** (same MCP-crashes-engine-survives pattern). Engine PID 1725881 etime 56:12 running the NEW binary (turn-79 4 KB cap + turn-77 MoE swap + turn-73 7200s budget) on a fresh runtime root `rust_reality_1778240223405536609_aaf099e6`.

Wait — etime 56:12 means engine was launched ~56 min ago, NOT just now from my drip. So this engine actually started during the MCP-down period (likely from a prior cron-driven action). My most-recent drip call hit a MCP server that immediately disconnected.

## First validation of all combined harness improvements

| Signal | Value |
|---|---|
| Model file loaded | `Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf` ✓ |
| GPU | 31949 MiB, 100% util |
| Rounds completed | 6 (rounds 1-6 all `open_file_window`) |
| Per-round time | ~9.3 min (vs prior MoE attempt's 12 min — turn-79 cap is working!) |
| `attempt-summary.json` | absent (in flight) |
| Candidate.patch | absent |

The 4 KB tool-result cap (turn 79) is producing measurable speedup: ~25% faster per round. Model is still over-cautious about exploration — same pattern as the previous MoE attempt where 8+ rounds went to file-reading before any apply attempt.

## Hard rules check

All ✅. No edits.

## Files touched

- `memory/reports/agent-110-coordinator--turn-110-final-report.md` — this file.

## Operator action required

`/mcp reconnect` — MCP dropped again on my drip call. Same pattern as previous turns.

## Next steps

1. After /mcp reconnect, record this and pending recommendations
2. Watch engine — at ~9 min/round, may reach apply phase by round 10-12 (within 64 min remaining budget)
3. If engine still doesn't reach apply: behavioral nudge in system prompt is needed (e.g., "commit to apply_*_edit by round 5")
