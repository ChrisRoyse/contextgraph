---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-05-07T13:50:00Z
status: active
tags: constraints, rules, build, test, governed-paths, generic-only
---

# Constraints

## Authoritative

`/home/cabdru/contextgraph/CLAUDE.md` is the canonical project-instruction file. **Read it before doing any work.** The rules below are highlights, not a replacement.

## Hard behavioral deny set (still binding even after gate removal 2026-05-07)

Do NOT edit:

- `.env` files (any path)
- `swebench/**`
- `tests/oracle_**`
- `crates/context-graph-benchmark/datasets/swe_bench_*`
- `crates/*/src/reward_specs/**`
- `crates/*/src/verifiers/**`
- The official SWE-bench Docker evaluator

These were enforced by a hook gate that was removed 2026-05-07 (the gate denied legitimate edits like memory writes, blocking the loop from solving problems). The pre-edit hook now records SHAs but no longer denies. The deny set above is now **behavioral** — Claude must honor it.

## GENERIC-ONLY harness rule

Every harness change must apply to ANY SWE-bench task / Python repo / bug class.

Mental check before merging: *"would this same change help if the inner actor were attempting `psf__requests-2317` / `django__django-12286` / `sympy__sympy-13895`?"* If no, reformulate as a general principle. **Per-task knowledge belongs in the inner actor's reasoning, not the harness.**

## Architecture

- Domain-Driven Design with bounded contexts.
- Files **under 500 lines** (split via `file-length-manager` agent if exceeding).
- Typed interfaces for all public APIs.
- TDD London School (mock-first) for new code.
- Topology: hierarchical-mesh, max 15 agents.
- Memory backend: hybrid AgentDB + RocksDB.

## Build & test

```bash
cargo build --release -p context-graph-cli      # the engine + harness CLI
cargo build --release -p context-graph-mcp      # the protocol shim
cargo test                                      # full suite
cargo clippy                                    # lint
```

After every `changed` recommendation that touches `crates/context-graph-cli/`, `cargo build --release -p context-graph-cli` is sufficient — the MCP→CLI shim re-resolves the binary on every harness call. NO operator `/mcp` reconnect needed for harness-CLI changes (commits 4072dcf3 / d48102e6).

After a `changed` recommendation that touches `crates/context-graph-mcp/src/handlers/`, `protocol.rs`, `tools/`, `main.rs`, etc., the operator MUST `/mcp` reconnect to load the new MCP binary.

## Reality-shift principle (core invariant)

No alteration without current state. No alteration without a delta back into perception. Before any alter call, perceive (`open_file_window` → SHA). After any alter call, the MCP server returns the delta and appends to the session shift log. See `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`.

## Single-writer rule

Never spawn another writer subagent in parallel with `cc-reality-harness-editor`. **Cap each turn at ONE harness-editor invocation.** If diagnosis suggests multiple separate edits, surface the additional ones in `next_inner_prompt_delta` or `remaining_risks` and let the next iteration handle the follow-up.

## Cost cap

One `/loop` turn is expected to consume **<$0.50** in API cost. If a turn approaches that ceiling, stop and record `blocked` with `cost_cap_reached`.

## Git safety

Never use destructive git commands (`push --force`, `-f`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`). Plain `git push` (fast-forward) is allowed.

## SMAP discipline

This project uses the Shared Mind Agent Protocol. Every agent that touches `./memory/` must:

1. **READ** `./memory/` first (see `mission-brief.md` for read order).
2. **WRITE** `.md` files into the appropriate namespace as artifacts emerge — write early, write often, never hide failures.
3. **REPORT** to `./memory/reports/` before finishing — this is the LAST action.

Frontmatter fields required on every file: `namespace`, `agent_id`, `timestamp`, `status`, `tags`.
