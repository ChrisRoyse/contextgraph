# AgentDB Findings — Index

> **🔒 LOCKED 2026-05-08 — Reference-Only, Clean-Room Reimplementation Only**
>
> Per operator decision (`memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`):
> - **Never vendor.** Do not copy any `agentdb/` source files into our tree.
> - **Never npm-depend.** Do not add `agentdb` or `@ruvector/*` packages to any `package.json`.
> - **Never path-dep.** The cloned `agentdb/` directory does not appear in our build graph.
> - **Never track upstream.** No `git pull` of the clone; the snapshot we have is the only one we'll ever look at.
> - **Reference-only.** Read the code to understand each algorithm; reimplement cleanly in our own crates.
>
> This index doc happens to already use "port" / "steal" language that is consistent with this rule. The recommendations below stay valid; just interpret every "steal" as "clean-room reimplement against the cloned read-only reference."

**Date:** 2026-05-08
**Author:** Claude Code (outer reality-loop optimizer, doing direct research per operator preference)
**Source repo:** `agentdb/` (cloned 2026-05-08 from https://github.com/ruvnet/agentdb at HEAD `a478ab3`; **read-only reference**, NOT a build dependency)
**Scope:** Full read of `README`, `CHANGELOG`, `package.json`, `src/` (controllers, backends, mcp, services), `examples/`, `simulation/`, `docs/adrs/`, `docs/architecture/`, `docs/research/`. Git history reviewed since 2026-05-06 (entire history — repo was init'd that day).
**Audience for these docs:** future ccreality optimizer turns + 5090jepa/ME-JEPA fresh agents.

---

## Why this document set exists

You asked me to read agentdb cover-to-cover and tell you how it could greatly enhance:

1. **The reality loop** (`docs/ccreality/` — Claude Code as outer optimizer, Qwen3.6-27B as inner SWE-bench repair actor, ContextGraph as deterministic substrate).
2. **The ME-JEPA / 5090jepa system** (`backupdocs/mejepa/`, `backupdocs/5090jepa/`, `backupdocs/dynamicjepa/` — RTX 5090–native auditable predictor with universal domain packs).

agentdb is essentially a *single-file, self-learning, episodic-memory + skill-library + causal-graph + Thompson-Sampling + Reflexion-style* runtime that exposes 41+ MCP tools. Several of its load-bearing ideas — **Reflexion episodes**, **CausalRecall utility reranker**, **Thompson Sampling solver with witness chain**, **NightlyLearner**, **MMR diversity rerank**, **MerkleRoot provenance certificates**, **TemporalCompressor tiered quantization**, **GraphRoPE**, **federated SONA aggregation** — map almost 1:1 onto pieces of work you have either *already half-built* or *explicitly listed in the open-question docs*. Several of them you've already independently built better. A few of them you should steal verbatim.

The honest summary is in `08_HONEST_SUMMARY.md` — read that one first if you only have 5 minutes.

---

## Document map (read in this order)

| # | File | What it covers |
|---|------|----------------|
| 00 | [`00_INDEX.md`](./00_INDEX.md) (this file) | Map of the doc set, scope, file ordering |
| 01 | [`01_AGENTDB_ARCHITECTURE.md`](./01_AGENTDB_ARCHITECTURE.md) | What agentdb actually is — the core class, controller layout, RVF backend stack, MCP surface, the SONA/RuVector/RVF-solver dependency tree |
| 02 | [`02_CAPABILITIES_INVENTORY.md`](./02_CAPABILITIES_INVENTORY.md) | Catalog of every reusable capability with file refs and provenance — Reflexion, SkillLibrary, CausalMemoryGraph, CausalRecall, ExplainableRecall, NightlyLearner, AttentionService, SemanticQueryRouter, TemporalCompressor, ContrastiveTrainer, SolverBandit, FederatedLearning, MMR, ContextSynthesizer, MetadataFilter, HNSWIndex, SparsificationService, MincutService, ReasoningBank |
| 03 | [`03_REALITY_LOOP_APPLICATIONS.md`](./03_REALITY_LOOP_APPLICATIONS.md) | Concrete, named applications to ccreality. Each idea cites the ccreality doc(s) it would change, the agentdb capability it borrows, and the expected effect on resolved-instance throughput |
| 04 | [`04_JEPA_APPLICATIONS.md`](./04_JEPA_APPLICATIONS.md) | Concrete, named applications to ME-JEPA / DynamicJEPA / 5090jepa. Each idea cites the JEPA doc(s) it would change, the agentdb capability it borrows |
| 05 | [`05_QUICK_WINS_VS_DEEP_INTEGRATION.md`](./05_QUICK_WINS_VS_DEEP_INTEGRATION.md) | Triage — which ideas are 1-day wins, which are 1-week, which are full ADR-grade integrations. Prioritized list with effort estimates and risk |
| 06 | [`06_RISKS_AND_NON_TRANSFERABLE.md`](./06_RISKS_AND_NON_TRANSFERABLE.md) | The honest "don't steal these" list. agentdb is TypeScript/sql.js-centric; some of its ideas are unsuitable for Rust+RocksDB+CUDA. Includes JS/TS leak-prone patterns, license clauses, missing claims |
| 07 | [`07_GIT_HISTORY_AND_VERSIONING.md`](./07_GIT_HISTORY_AND_VERSIONING.md) | Last-2-day commit review (repo only has 7 commits, all 2026-05-06), package versioning context (`3.0.0-alpha.14` published), and what that means for stability of any code you'd port |
| 08 | [`08_HONEST_SUMMARY.md`](./08_HONEST_SUMMARY.md) | Direct verdict — what's actually worth doing, what's a distraction, what to read in their docs that I won't reproduce here |

---

## TL;DR — the four highest-leverage transplants

1. **Reflexion episode pattern → ccreality `attempt-summary` index.** agentdb's `Episode { task, input, output, critique, reward, success }` is a clean schema for exactly the per-attempt records you already write. Add a "critique" string and you can build `agentdb_critique_summary` / `agentdb_success_strategies` queries on top of `mcp__cgreality__attempts_history_query` without any new column families. See `03_REALITY_LOOP_APPLICATIONS.md §1`.

2. **CausalRecall utility reranker (α·sim + β·uplift − γ·latency) → ccreality recommendation ranking.** When you decide which past harness recommendation to revisit on a parked task, score by similarity-to-current-failure × historical-uplift − rerun-cost. Direct port: `src/controllers/CausalRecall.ts:90–120`. See `03_REALITY_LOOP_APPLICATIONS.md §3` and `04_JEPA_APPLICATIONS.md §4`.

3. **SHAKE-256 witness chain (ADR-010) → DynamicJEPA training-run audit chain.** RVF-solver's `73-byte/entry` tamper-evident chain is exactly the kind of physical-readback evidence you require for verification runs. Already a perfect fit for `11_full_state_verification_and_evaluation.md` and `11_training_runs_and_artifacts.md`. See `04_JEPA_APPLICATIONS.md §1`.

4. **Thompson Sampling SolverBandit with 18 context buckets → ccreality task-routing across the 300-instance Lite suite AND DynamicJEPA candidate-action ranking.** Same primitive, two uses. The 18-bucket model (range × distractor × noise) maps onto SWE-bench task-difficulty buckets and onto JEPA action-candidate buckets. Already implemented end-to-end in `src/backends/rvf/SolverBandit.ts`. See `03_REALITY_LOOP_APPLICATIONS.md §2` and `04_JEPA_APPLICATIONS.md §2`.

---

## Things I deliberately did NOT propose

- **Adopting agentdb wholesale.** It's TypeScript with a Rust optional-dep tree (`@ruvector/*`). You're a Rust workspace with RocksDB + candle + 13 embedders. Embedding it would import a JS/Node runtime into a CUDA path that already has too many moving parts. Take the *ideas*, not the *code*. Documented in `06_RISKS_AND_NON_TRANSFERABLE.md`.
- **Replacing your 13-embedder pipeline with their MiniLM-default.** You have E1–E13 with E5/E8/E10 asymmetric. They have one Xenova MiniLM by default. Their architecture is *worse* than yours on the embedding axis.
- **Replacing your causal graph (E5 causal asymmetric, CAUSAL_THRESHOLD=0.04) with their CausalMemoryGraph.** Theirs uses simple cosine + uplift; yours has structural causal markers + dual cause/effect vectors + LLM-validated edges. Theirs is a regression unless you specifically want do-calculus uplift estimation, which is a separate idea worth stealing (covered in `03 §3`).

---

## How to use this doc set in a future turn

Future agents should read these in order:

1. `08_HONEST_SUMMARY.md` — get the verdict.
2. `02_CAPABILITIES_INVENTORY.md` — find the capability by name.
3. `03_REALITY_LOOP_APPLICATIONS.md` or `04_JEPA_APPLICATIONS.md` — find the proposed transplant.
4. `05_QUICK_WINS_VS_DEEP_INTEGRATION.md` — pick the right size of transplant.
5. `06_RISKS_AND_NON_TRANSFERABLE.md` — sanity-check before committing.

These docs are static research artifacts. They don't replace `./memory/` SMAP — when a transplant is actually adopted, write the locked decision to `./memory/decisions/` per CLAUDE.md §6.
