---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-05-07T13:50:00Z
status: active
tags: mission, ccreality, swebench, qwen, reality-loop, smap
---

# Mission: ccreality Outer Reality-Loop Optimizer

## What we are building

ContextGraph's "reality loop" — a self-driving harness that runs the full 300-instance SWE-bench Lite suite under one generic harness, with **Qwen3.6-27B Q6_K_XL GGUF** as the inner repair actor and **Claude Code** as the outer optimizer.

The outer optimizer iterates the harness based on per-task evidence; the inner actor edits code under the harness's tool surface and is evaluated by the **official SWE-bench Docker scorer**.

## Success predicate

The official SWE-bench report places every targeted instance in `resolved_ids`, achieved by running the FULL 300-task suite under the SAME harness in one shot.

## How the loop runs

A 15-minute cron fires `/loop` in Claude Code (cron id `2fdb2d44`, schedule `7,22,37,52 * * * *`). Each tick is one optimizer turn:

1. Drip a fresh `mcp__cgreality__reality_run_attempt` against the next unresolved task in `tasks/swebench-lite/*.json`.
2. Diagnose any failure via `cc-reality-diagnostician` (Sonnet, read-only).
3. Edit the harness via `cc-reality-harness-editor` (Opus, single writer) — **GENERIC-ONLY**, no per-task overfit.
4. Verify with `cc-reality-source-of-truth-readback`.
5. Record `mcp__cgreality__optimizer_record_recommendation` (the Stop hook enforces this).

## Authoritative documents (read in this order)

1. `/home/cabdru/contextgraph/CLAUDE.md` — project instructions; **takes precedence** over anything in `./memory/` if they conflict.
2. `/home/cabdru/contextgraph/.claude/loop.md` — the `/loop` skill text the cron actually fires.
3. `/home/cabdru/contextgraph/docs/ccreality/` — full design rationale (21 docs).
4. `/home/cabdru/contextgraph/docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md` — the recommendation JSON schema.
5. `/home/cabdru/leapablememory/docs2/SHARED-MIND-AGENT-PROTOCOL.md` — the SMAP protocol governing this `./memory/` directory.

## Key invariants

- **Reality-shift principle**: no alteration without current state; no alteration without a delta back into perception. (`docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`)
- **GENERIC-ONLY harness**: every harness change must apply to ANY SWE-bench task / Python repo / bug class. Never overfit to the current candidate.
- **Single-writer rule**: only `cc-reality-harness-editor` performs harness source edits, one invocation per turn maximum.
- **No HTTP from the optimizer**: the MCP server is the only network/IO surface. No `curl`, no `wget`.

## Inner actor identity

- Model: `qwen3.6:27b` (MCP identifier).
- GGUF: Unsloth Q6_K_XL, ~25.6 GB on disk, ~22 GB GPU resident.
- Pin key: `qwen3_6_27b_q6_k_xl` in `config/ccreality_model_pins.json`.
- Effective context: 262,144 tokens (256K).
