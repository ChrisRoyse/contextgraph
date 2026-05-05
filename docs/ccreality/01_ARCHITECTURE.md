# Architecture: Claude Code as Outer Reality-Loop Operator

## One picture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                CLAUDE CODE                                   │
│  (operator session — interactive, /loop, or `claude -p` bursts)              │
│                                                                              │
│   ┌────────── reality drip via hooks ──────────┐                             │
│   │ SessionStart    : inject runtime root      │                             │
│   │ UserPromptSubmit: inject latest trigger    │                             │
│   │ PreToolUse Edit : inject path policy + SHA │                             │
│   │ PostToolUse Edit: inject cargo/diff result │                             │
│   │ Stop            : require recommendation   │                             │
│   └────────────────────────────────────────────┘                             │
│                                                                              │
│   ┌────────── ALWAYS-ON via MCP ───────────────────────────────────────────┐ │
│   │  context-graph-mcp  (mode: reality-loop)                               │ │
│   │  ╭──ANALYZE──╮  ╭──INTERACT──╮  ╭──ALTER──╮                            │ │
│   │  │ reality_* │  │ reality_run│  │ harness_*│                           │ │
│   │  │ search_*  │  │ targeted_* │  │ store_*  │                           │ │
│   │  │ traverse_*│  │ recompile_*│  │ boost_*  │                           │ │
│   │  │ audit_*   │  │ replay_*   │  │ forget_* │                           │ │
│   │  ╰───────────╯  ╰────────────╯  ╰──────────╯                           │ │
│   └────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │ MCP stdio (JSON-RPC)
                                   │
┌──────────────────────────────────▼──────────────────────────────────────────┐
│                       CONTEXTGRAPH (Rust workspace)                          │
│                                                                              │
│   ┌─── reality engine (binary) ───┐    ┌─── memory store (RocksDB) ────┐   │
│   │ context-graph-cli reality-loop│    │ 13 embedders, HNSW, ledger,    │   │
│   │   attempt   — one Gemma run    │    │ audit log, provenance chain,   │   │
│   │   smoke     — one synthetic   │    │ causal graph, harness deltas   │   │
│   │   replay    — replay artifact │    └────────────────────────────────┘   │
│   └────────────────┬───────────────┘                                         │
│                    │                                                         │
│                    ▼                                                         │
│   ┌─── durable reality artifacts ────────────────────────────────────────┐   │
│   │ /tmp/contextgraph-rust-reality-loop/<run-id>/<task>/attempt-N/       │   │
│   │   problem-reality-packet.json                                         │   │
│   │   reality-signal-packet.json                                          │   │
│   │   harness-transitions/<round-NN>.json                                 │   │
│   │   official-evidence/<instance>.json                                   │   │
│   │   attempt-summary.json                                                │   │
│   │   blackbox-ledger.sqlite (append-only)                                │   │
│   └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   ┌─── Gemma inner loop (unchanged from current design) ──────────────────┐ │
│   │  ollama gemma4:e4b → governed agent tools → tool-mediated edit        │ │
│   │  → static patch checks → official Docker SWE-bench eval               │ │
│   └──────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Three roles, sharply separated

### Gemma — inner SWE-bench repair actor
Owns: reading the SWE-bench task, walking the cloned repo through governed tools, proposing structured edits, iterating until the official Docker evaluator places the instance in `resolved_ids`. Unchanged.

### ContextGraph — deterministic reality substrate
Owns:
- Running Gemma attempts (`reality-loop attempt`).
- Running official SWE-bench Docker eval per attempt.
- Writing every artifact (problem reality, reality signal, harness transitions, official evidence, attempt summary, BlackBox ledger entries).
- Exposing the durable reality plus the memory graph through the **MCP server** that Claude Code consumes.
- Refusing to touch the oracle, benchmark tests, reward specs, or `.env` — enforced by hooks AND by MCP tool denylists, not by trust.

### Claude Code — outer harness optimizer
Owns:
- Reading durable reality through MCP `analyze` tools.
- Diagnosing why Gemma failed.
- Editing the harness, prompts, tool surface, reward features, telemetry through MCP `alter` tools (which write to disk + readback + diff).
- Triggering re-attempts through MCP `interact` tools.
- Producing a structured recommendation conforming to a JSON schema, persisted via `optimizer_record_recommendation`.

Claude Code is **never** the source of truth. Its recommendation is a model-generated artifact that links to the source-of-truth files it consulted.

## Why MCP is the spine, not `claude -p` shelling

Earlier draft (in `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md`) had ContextGraph spawning `claude -p` per trigger. We're rejecting that for production for two reasons:

1. **Reality must be queryable, not just delivered.** A spawned subprocess gets one prompt then dies. With MCP, Claude Code can ask follow-ups: "show me attempt 3 of run X", "diff the harness-transition rounds 4 vs 5", "search memory for similar past failures". Mid-session reality access is what makes the optimizer better than DeepSeek's one-shot tool loop.
2. **Mutation must close the loop.** When Claude Code edits the harness via MCP, the same MCP server reads it back, runs `cargo check`, writes a `harness-transition` record, and ContextGraph immediately knows what changed. No fragile JSON parsing of stdout, no race between Rust spawn and Claude's edit.

The fallback path (`claude -p` invoked by ContextGraph for unattended overnight runs) is still supported — see [14_TRIGGER_AND_INVOCATION_POLICY](14_TRIGGER_AND_INVOCATION_POLICY.md) — but its job is just to start a Claude Code session that then uses MCP for everything substantive.

## Why this is bigger than swapping models

The ContextGraph reality loop is a **meta-harness** in the Karpathy AutoResearch sense: an agent that improves its own training/repair loop by editing harness code, time-boxed, with durable scoring (here: official `resolved_ids`, plus the multi-signal reward packet). The model behind the meta-agent matters less than the harness's ability to feed reality back to it.

By putting Claude Code on the operator side and ContextGraph on the engine side — connected by MCP — we get:
- **Continuous reality**: hooks inject reality at every prompt and every tool call.
- **Random-access reality**: MCP analyze tools let Claude pull any artifact on demand.
- **Verified mutation**: MCP alter tools close the read-write-readback loop in one place.
- **Audit-able cognition**: every Claude turn produces a structured recommendation tied to the artifacts it read and the artifacts it wrote — the loop's own provenance chain.

## Reality-shift principle (the core invariant)

See [01b_REALITY_SHIFT_PRINCIPLE](01b_REALITY_SHIFT_PRINCIPLE.md) for the full doc. In one sentence: **no alteration without current state, no alteration without a delta back into perception**. The agent must always be able to answer "what is reality now? what has shifted since I started? did anything drift without me knowing?" — using only its tools and hooks. Every other mechanism in this directory exists to serve that.

## Reality-drip principle

Claude Code is **always reading reality during an optimization turn**, not just at the start. There are six injection points (detailed in [02_REALITY_DRIP_PROTOCOL](02_REALITY_DRIP_PROTOCOL.md)):

1. SessionStart — inject runtime root, target instance, success predicate.
2. UserPromptSubmit — inject latest trigger decision + reality-signal compact.
3. PreToolUse on Edit/Write — inject current file SHA + path policy.
4. PostToolUse on Edit/Write — inject cargo check + git diff + write a harness-transition.
5. Stop — refuse to end turn until a structured recommendation is recorded.
6. PreCompact — preserve runtime root + last trigger decision through context compaction.

Plus the always-on MCP channel: Claude can call `reality_*` tools at any moment to pull any artifact.

## Compatibility with existing ContextGraph

This plan **does not** break:
- The MCP server's existing tool surface (memory_tools, graph_tools, causal_tools, etc.). The `reality_loop` module is additive.
- The Gemma inner loop, governed agent tools, problem-reality packet builder, reality-signal packet builder, harness-transition recorder, BlackBox ledger.
- The official SWE-bench Docker bridge.

This plan **does** remove (per [04_EXCISION_PLAN](04_EXCISION_PLAN.md)):
- All DeepSeek/OpenAI HTTP code in `reality-loop.rs`.
- All `openai_optimizer_*`, `deepseek_*` flags, helpers, structs, tests.
- The trigger gate that currently calls `run_openai_loop_optimizer` from inside the attempt loop.
- The `openai-loop-optimizer/` artifact subdirectory and its contents.

Replaced by:
- A new MCP module `reality_loop` in `crates/context-graph-mcp/`.
- `.claude/` scaffolding (settings, hooks, agents, skills, loop prompt, project memory addendum).
- `scripts/cc/` hook scripts that push reality into Claude.
