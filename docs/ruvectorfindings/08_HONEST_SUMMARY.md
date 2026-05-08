# 08 — Honest Summary

The 5-minute version. Read this first if you only have time for one of these docs.

---

## What RuVector is, in one sentence

A 135-crate Rust workspace shipping a self-learning vector database (`ruvector-core`), an adaptive learning engine (`sona`), a single-file cognitive container format with cryptographic witness chains (`rvf`), an attention library (~50 mechanisms), a graph engine with Cypher + 8 verified module families, sublinear graph solvers (`ruvector-solver`), mincut everywhere (`ruvector-mincut`), local LLM runtime (`ruvllm`), and as of yesterday a training-free retrieval LM with masked discrete diffusion (`ruvllm_retrieval_diffusion`).

It's the upstream that agentdb is a TypeScript wrapper of.

---

## What changes if you read RuVector after agentdb

agentdb said: "here are 8 good ideas, plan to port them in Rust over ~10 days." RuVector says: **"these ideas are already in Rust here, vendor or path-dep them in ~7 days."**

Specifically: the 73-byte SHAKE-256 witness chain that agentdb's findings documented as "port from agentdb" is in fact in `ruvector/crates/rvf/rvf-crypto/src/witness.rs:24` (`ENTRY_SIZE: usize = 73`). agentdb consumed it via `@ruvector/rvf-crypto` npm package; you can consume it via `Cargo.toml` path-dep.

Same story for SONA (the trajectory + LoRA + EWC++ engine), Thompson Sampling (in `ruvector-solver`'s policy router), MMR (in `ruvector-core::advanced_features::mmr`), the 4-tier cold storage (in `ruvector-temporal-tensor`, but with finer 8/7/5/3-bit quantization), Granger causality (in `ruvector-graph-transformer/temporal.rs`).

You don't port. You vendor or path-dep.

---

## What's genuinely good

1. **`rvf-crypto`** — 700 LOC, no_std, ed25519 + ML-DSA-65 + SLH-DSA-128s post-quantum signatures + 73-byte SHAKE-256 witness chain + lineage records + TEE attestation. Single highest-leverage transplant.

2. **`sona`** — Rust-native trajectory-based learning. Micro-LoRA + EWC++ + REINFORCE gradient + 3 learning loops + ReasoningBank pattern extraction + federated coordinator + safetensors export. The closest thing the open-source ecosystem has to "agentdb but in Rust."

3. **`ruvector-solver`** — 8 sublinear algorithms (Neumann, CG, Forward Push, Backward Push, BMSSP multigrid, TRUE, Random Walk, plus router), SIMD, audit log, witness-grade. Stops you from re-implementing PageRank or Laplacian solve.

4. **`ruvector-mincut`** — 28-module mincut suite. Canonical, parallel, link-cut, JTree, sparsifier, witness, certificate, expander, subpolynomial. The right primitive for "split this graph respecting these constraints."

5. **`ruvector-temporal-tensor`** — 8/7/5/3-bit access-pattern-driven cold-tier compression. Zero deps, WASM-compatible. 4-10× storage reduction.

6. **`ruvector-core::advanced_features`** — MMR, Matryoshka funnel, Graph RAG (Leiden communities), DiskANN/Vamana, ColBERT multi-vector, OPQ, conformal prediction, hybrid (BM25 + dense + RRF). Path-dep selectively.

7. **`ruvllm_retrieval_diffusion`** — *yesterday's* (2026-05-08) addition. Training-free retrieval LM + masked discrete diffusion. 6.9× speedup over autoregressive on Mario benchmark. Drop-in for the DynamicJEPA gridworld planner head.

8. **`ruvector-graph-transformer/temporal.rs`** — Granger causality + causal masking + retrocausal-safety enforcement. Right primitive for ME-JEPA Stage F.

9. **`ruvector-graph-transformer/verified_training.rs`** — Per-step training certificates with formal/statistical invariants. Fail-closed. Right primitive for FSV-grade gradient-step audit.

That's the takeaway list. The remaining ~120 crates are decoration, specialty, or alternate-target builds.

---

## What's not worth taking

- **The full `crates/rvf/` single-file format** beyond `rvf-crypto` (RocksDB-FSV mismatch).
- **`ruvllm` runtime** (no sm_120 kernels; you have llama-cpp-2 + custom CUDA path).
- **`ruvector-attention`** Flash/MLA/Mamba/sheaf etc. (no sm_120 either; read for ideas).
- **`ruvector-graph` Cypher engine** (you have E5 causal graph; not a replacement).
- **`ruvector-postgres`** (RocksDB-based workspace).
- **`ruvector-consciousness`, `ruQu`, `nervous-system`** (research-grade, no alignment).
- **Specialty domains** (`agentic-robotics`, `neural-trader`, `thermorust`, `ruvix`, `ruvector-kalshi`).
- **`mcp-brain*` hosted-pi.ruv.io flavor** (external HTTP service; CLAUDE.md §11.9 forbids).
- **Marketing perf claims** (synthetic benchmarks; "150× faster," "+36%," "8.2× faster" — bad baselines).

Full list with reasoning is `06_RISKS_AND_NON_TRANSFERABLE.md`.

---

## The verdict for ccreality

**Path-dep `ruvector-solver`, vendor `rvf-crypto`, vendor `sona`** — three transplants, ~6-7 days.

After this sprint:
- Optimizer's decision stream has a SHAKE-256 witness chain. `optimizer_witness_chain_verify` returns `valid: true` cryptographically.
- Optimizer can answer "which past attempts most influenced today's failure?" via PPR over the (failure_tag, recommendation) graph in 1 tool call.
- Optimizer learns across turns via SONA trajectory + REINFORCE + EWC++. The bandit (proposed in `agentdbfindings/03 §2`) sits on top.

Tier 2 adds ~10 days for `verified_training` certificates per gradient step, MMR + Matryoshka + Graph RAG retrieval, mincut for A/B splits, conformal prediction.

Full priority order in `03_REALITY_LOOP_APPLICATIONS.md` and `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`.

---

## The verdict for ME-JEPA / DynamicJEPA / 5090jepa

**Vendor `rvf-crypto`, path-dep `ruvllm_retrieval_diffusion`, vendor `sona`** — three transplants, ~7-8 days.

After this sprint:
- Every training run + verification run has a cryptographic witness chain. `dynamicjepa verify-witness-chain --run <run_id>` satisfies `5090jepa/00_TASK.md` rules R4 + R5 cryptographically.
- Gridworld planner head uses `ruvllm_retrieval_diffusion::Diffuser` — training-free, 6.9× over AR baseline. Drop-in.
- Planner learns across episodes via SONA. EWC++ prevents counter_world ↔ gridworld interference. ReasoningBank surfaces stable patterns as candidate skills.

Tier 2 adds ~10 days: Stage 0 + Stage F certificates, `verified_training` per-step certificates, PPR for trajectory importance, temporal-tensor cold storage, mincut for leakage check + actor split.

Tier 3 (~15 days) is cross-pack transfer + Granger causality + manifold geometry — substantial, defer until Tier 1+2 stable.

Full priority order in `04_JEPA_APPLICATIONS.md` and `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`.

---

## The two big takeaways (revised after reading RuVector)

### 1. agentdb's "port these primitives" reduces to RuVector's "vendor these crates"

The agentdb findings recommended ~10-12 days of Rust port work for Tier 1+2. RuVector findings recommend ~7-8 days of vendor + path-dep for the equivalent capability — but the capability is dramatically richer (8 sublinear solvers vs 1 SolverBandit, full SONA vs simplified bandit, post-quantum signatures vs SHAKE-only, etc.).

The reduction is because **someone already wrote it**. Specifically, `rvf-crypto` is what `@ruvector/rvf-crypto` is in agentdb's npm dep tree — same code, but exposed as a Rust crate you can vendor instead of a Node module you wrap in NAPI.

### 2. The 2026-05-08 retrieval-diffusion crate is uniquely well-timed for DynamicJEPA

`ruvllm_retrieval_diffusion` (PR #450) merged yesterday. It's a near-perfect drop-in for `gridworld_5x5.v1` and `counter_world.v1` planner heads (vocab ≤ 254). 6.9× speedup over autoregressive on a comparable benchmark. Training-free. No autograd, no Python toolchain. The 5090jepa demo Phase 1 needs exactly this primitive.

This isn't a hypothetical fit. It's an exact match landing 0 days before you read it.

---

## The vendor-vs-path-dep decision

Recommended per crate (full reasoning in `06`):

| Crate | Choice | Why |
|-------|--------|-----|
| `rvf-types` + `rvf-crypto` | Vendor | Critical-path security; want stability + license-clean inclusion. |
| `sona` | Vendor | Long-running learning state; you don't want upstream churn changing trajectory semantics mid-experiment. |
| `ruvector-solver` | Path-dep | Pure algorithms; upstream improvements are wins. |
| `ruvector-temporal-tensor` | Path-dep | Zero-deps crate; minimal churn risk. |
| `ruvector-mincut` | Path-dep | Same. |
| `ruvector-core::advanced_features::*` | Path-dep | Each module small; track upstream. |
| `ruvllm_retrieval_diffusion` | Path-dep | Brand new (PR #450); will see active churn. Stay current for bugfixes. |
| `ruvector-graph-transformer` | Selective port | Only Granger + causal masking + verified_training. Avoid the full feature surface. |
| `prime-radiant` (if adopted) | Vendor | Heavy tree; pinning a snapshot keeps your build deterministic. |

---

## The license is fine

MIT. Permissive. Copy + clean-room ports both fine. When vendoring: preserve the LICENSE file alongside, add per-file attribution headers, run `cargo license` on the resulting tree to surface any non-MIT/Apache transitive deps.

---

## Reading order if you're going to act on this

1. Skim this file (you're done).
2. **`02_CAPABILITIES_INVENTORY.md`** for the per-crate capability list with file refs.
3. **`05_QUICK_WINS_VS_DEEP_INTEGRATION.md`** for the prioritized triage and a concrete one-week sprint plan.
4. Pick from Tier 1 (witness chain + sublinear solvers + SONA + temporal-tensor + MMR + retrieval-diffusion + conformal prediction).
5. Read the relevant section in `03_REALITY_LOOP_APPLICATIONS.md` or `04_JEPA_APPLICATIONS.md` for the concrete shape.
6. Read `06_RISKS_AND_NON_TRANSFERABLE.md` for what to NOT do and vendor-vs-path-dep tradeoffs.
7. Implement.
8. Write the locked decision to `./memory/decisions/ruvector-<dep>.md` per CLAUDE.md §6.

`07_GIT_HISTORY_LAST_2_DAYS.md` is for ongoing tracking — the active workstream is "subquadratic / training-free retrieval LMs," and PR #450 on 2026-05-08 is the most recent commit.

`01_RUVECTOR_ARCHITECTURE.md` is the deep architecture reference if you want to understand the 9-layer crate stack.

---

## Bottom line

Where the agentdb findings doc said "port these 8 ideas in 10-12 days," the RuVector findings doc says **"vendor 2 sub-crates + path-dep 6 crates + 500 LOC of glue, in 7-8 days, for substantially more capability."**

The four highest-leverage transplants:

1. **Vendor `rvf-crypto`** — witness chain + post-quantum signatures + lineage + attestation. Two consumers (ccreality + DynamicJEPA). ~2 days.
2. **Path-dep `ruvector-solver`** — 8 sublinear algorithms with SIMD + audit. ~1 day.
3. **Vendor `sona`** — full Rust-native trajectory learning + EWC++ + ReasoningBank. ~3-4 days.
4. **Path-dep `ruvllm_retrieval_diffusion`** — training-free planner head for gridworld. Yesterday's release. ~1-2 days.

Total: 7-9 days. After that:
- ccreality optimizer has cryptographic audit + sublinear-PPR-aware retrieval + REINFORCE-trajectory learning + EWC++ memory.
- DynamicJEPA gridworld planner is a 6.9× faster training-free retrieval-LM with witness-chain audit.
- ME-JEPA Stage 0 + Stage F can ship Merkle-rooted certificates built from `rvf-crypto` primitives in 2 more days.

The remaining 50+ candidate transplants in `02` are individually useful but not load-bearing.

The honest summary is: **agentdb is a wrapper, RuVector is the substrate.** When you want the substrate, take it.
