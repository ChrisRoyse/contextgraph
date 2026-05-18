---
namespace: reports
agent_id: agent-77-coordinator
timestamp: 2026-05-08T03:20:00Z
status: active
tags: report, final, turn-77, model-swap-shipped-this-session, engine-at-budget-cap, no-patch-submitted
---

# Turn 77 — model swap shipped; old engine hits 7200s cap with no patch

## Status

`no_change` — recording the wait state. Engine PID 742443 at exactly 7200s (the turn-73 wallclock budget). Model swap to Qwen3-Coder-30B-A3B-Q4 already committed earlier this session (recorded in `decisions/agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md`). Next drip will exercise the new model.

## Engine state

| Signal | Value |
|---|---|
| PID 742443 etime | 02:00:36 (= 7200s, exactly at budget) |
| Last completed round | round-07 (`search_repo`) |
| Round trajectory | rounds 1-4: read_problem_reality + 3x open_file_window; rounds 5-7: 3x search_repo |
| `apply_*_edit` reached? | NO |
| GPU memory | 31924 MiB (still loaded — old model resident) |
| `attempt-summary.json` | absent (will appear at cap fire) |

## Empirical validation: dense 27B is too slow on this hardware

7 rounds × ~17 min/round = 119 min ≈ exactly the 7200s budget. The dense model spent the ENTIRE budget on exploration (read_problem_reality + 3x open_file_window + 3x search_repo) without reaching any apply_*_edit. This is concrete validation of turn-77's swap rationale: even with the raised 7200s budget, dense 27B Q6 cannot complete a useful trajectory.

## Model swap recap (already done this session)

| Component | Old | New |
|---|---|---|
| Model | Qwen3.6-27B-Instruct | Qwen3-Coder-30B-A3B-Instruct |
| Architecture | Dense 27B | MoE 30B/3B-active |
| Quant | UD-Q6_K_XL | UD-Q4_K_XL |
| Size | 24 GB | 17.67 GB |
| SHA256 | 8746881d… | 2841aa31… |
| Pin key | qwen3_6_27b_q6_k_xl | qwen3_coder_30b_a3b_q4_k_xl |
| MCP id | qwen3.6:27b | qwen3-coder:30b-a3b |
| GPU preload threshold | 25600 MB | 18432 MB |
| Old GGUF on disk | gone | replaced |

Binary mtime: 22:15:15 local (just rebuilt). cargo check + 38/38 tests + cargo build --release all PASS. Engine PID 742443 still uses old binary AND old (now-deleted) model via mmap pages — safe; pages release when process exits.

## Decisions

1. **No drip THIS turn** — engine in flight (about to terminate).
2. **No harness edit** — single-writer cap used at turn 77 (the model swap).
3. **No engine kill** — let cap fire naturally; produces clean attempt-summary.

## Hard rules check

All ✅. Single harness-editor cap: ONE used (model swap).

## Files touched

- `memory/decisions/agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md` — NEW (locked) — written earlier in turn cycle
- `memory/reports/agent-77-coordinator--turn-77-final-report.md` — this file

## Source-of-truth readbacks

- `file:./memory/decisions/agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md`
- `file:./memory/decisions/agent-73-harness-editor--wallclock-budget-raise.md`
- `file:/home/cabdru/contextgraph/config/ccreality_model_pins.json`
- `file:/home/cabdru/contextgraph/models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf`

## Ledger reference

- BlackBox sequence: 40
- Record id: `bbx_rec_33919ea954164313a7083124b4831bf7`
- Shift id: `01J6C1FDE70E3FD42D48B4E`

## Next-turn priorities

1. **HIGHEST**: probe whether PID 742443 has terminated. If yes, drip a fresh attempt with the new MoE model.
2. **First-drip validation**: confirm Qwen3-Coder chat template loads cleanly + tool grammar matches + GPU footprint under 18 GB preload. If any of these fail → blocker, possibly rollback to dense 27B.
3. **If new model works**: track per-round time. Expect 3-5 min vs prior 14-26 min. If achieved, the budget is overly generous → can lower back from 7200s to 3600s in a future turn.
4. **Then prompt-bloat fix**: strip `<think>` blocks from prior assistant messages in conversation history (operator's secondary directive from earlier this session).
