---
namespace: decisions
agent_id: agent-140-coordinator
timestamp: 2026-05-08T19:24:50Z
status: active
tags: agentdb, ccreality, outer-loop, fsv, witness-chain, bandit, reflexion
---

# Decision — Selective agentdb primitives are adopted as Rust-native reality-loop evidence tools

## Locked rule

ContextGraph adopts the useful agentdb recommendations as Rust-native ccreality primitives only:

- Reflexion-style attempt episode fields on `attempt-summary.json` and `reality-optimizer/attempts.jsonl`.
- A persisted Thompson-sampling SolverBandit for repeated outer-loop decisions.
- Utility recall over real optimizer recommendations with provenance hashes.
- Deterministic attempt recall with metadata filters, MMR diversity, critique summaries, success strategies, and synthesized context.
- A 73-byte SHAKE-256 witness chain over optimizer writes.

ContextGraph must not import agentdb wholesale, add a Node/TypeScript runtime, add `.rvf` storage, add optional dependency fallbacks, or port agentdb's toy RL / attention / GNN claims into the CUDA/Rust path.

## Source-of-truth files

- Bandit state: `<runtime_root>/<run_id>/reality-optimizer/solver-bandit.json`
- Attempt episodes: `<runtime_root>/<run_id>/reality-optimizer/attempts.jsonl`
- Optimizer recommendation recall: `recommendation-turn-*.json` under the active run
- Witness chain: `<runtime_root>/<run_id>/claude-code-optimizer/witness-chain.bin`
- Recommendation ledger mirror: BlackBox SQLite `ledger_records`

Every write path must mutate, fsync where applicable, re-read the source of truth, and return the physical pointer plus hash/entry count. Errors must be structured and fail closed; no fallback implementation may hide a missing or corrupt artifact.

## Why

The outer loop is not trying to make code review nicer. Its target is stronger: an optimizer whose claims are checkable against durable reality. The inner model attempts SWE-bench tasks; the outer loop learns from real attempt artifacts, reward rows, recommendation files, and witness-chain readbacks. These agentdb-derived primitives help because they organize and rank that evidence without replacing the existing ccreality source-of-truth discipline.

## Consequences

Future loop changes should extend the persisted evidence system, not introduce another memory substrate. A local validation pass is not success; only official SWE-bench resolved evidence sets `success = true`. If the official result is absent, the system must say so and carry the incomplete state forward as evidence.
