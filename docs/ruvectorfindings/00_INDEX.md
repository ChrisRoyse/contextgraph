# RuVector Findings — Index

**Date:** 2026-05-08
**Author:** Claude Code (outer reality-loop optimizer, doing direct research per operator preference)
**Source repo:** `ruvector/` (cloned this turn from https://github.com/ruvnet/RuVector at HEAD `ef5274c2`)
**Scope:** Cargo workspace audit (135 crates, 2,492 commits, 277 KB README); deep read of `crates/ruvector-core/`, `crates/sona/`, `crates/rvf/rvf-crypto/`, `crates/ruvector-attention/`, `crates/ruvector-graph-transformer/`, `crates/ruvector-solver/`, `crates/ruvector-temporal-tensor/`, `crates/ruvllm/`, `crates/ruvllm_retrieval_diffusion/`, `crates/ruvllm_sparse_attention/`, plus `docs/adr/` (206 ADRs).
**Audience:** future ccreality optimizer turns + 5090jepa/ME-JEPA fresh agents.

---

## TL;DR — what changes if you read RuVector after agentdb

RuVector is the **upstream**. agentdb is a TypeScript wrapper that *consumes* `@ruvector/*` npm packages. This means:

- Every "good idea" I flagged from agentdb (SHAKE-256 73-byte witness chain, SolverBandit, NightlyLearner, CausalRecall, MMR, ContextSynthesizer, SONA, ContrastiveTrainer, etc.) **already exists in Rust here**, in production form, with proper tests.
- The agentdb findings doc said "port to Rust" for ~5 days of work. The RuVector findings replace that with: **vendor or path-dep these crates**, far less work.
- Where agentdb has a 270-line TypeScript SolverBandit, RuVector has the 312-line `ruvector-solver` Rust crate plus 8 algorithmically distinct sublinear solvers (Neumann, CG, Forward Push, BMSSP multigrid, etc.), Personalized PageRank with budgets, witness audit trails, SIMD acceleration. Same primitive, an order of magnitude more capability.
- agentdb's Reflexion + SkillLibrary + CausalMemoryGraph are SQL schema + TS controllers. RuVector's are crate-level: `sona` (ReasoningBank, EWC++, micro-LoRA, federated coordinator, training pipeline), `ruvector-graph` (Cypher engine, hyperedges), `ruvector-coherence` (signal quality / hallucination detection), `prime-radiant` (sheaf Laplacian coherence engine, sona_tuning).
- The 73-byte SHAKE-256 witness chain that I documented as "port from agentdb" is in fact **right here at `crates/rvf/rvf-crypto/src/witness.rs:24` (`ENTRY_SIZE: usize = 73`)** — no_std, ed25519, post-quantum signatures (ML-DSA-65, SLH-DSA-128s), full lineage chain, attestation records.

So the operator's question "give me the same assessment" reframes after reading RuVector: this isn't an idea source, it's a **dependency candidate**. Several primitives could land in your workspace as `Cargo.toml` entries this afternoon.

---

## What RuVector actually is

A 135-crate Rust workspace shipping:

- A **vector database** (`ruvector-core`) with HNSW + DiskANN/Vamana + flat indexes, hybrid search (BM25 + dense + RRF), Graph RAG (Leiden community detection), ColBERT multi-vector retrieval, Matryoshka embeddings, OPQ, LSM compaction, MMR diversity reranker, conformal prediction, sparse vectors.
- A **self-learning engine** (`sona`) with micro-LoRA + EWC++ + REINFORCE-trajectory learning + reasoning bank pattern extraction + federated training coordinator + safetensors/HuggingFace export + 3 learning loops (instant, background, coordination).
- A **graph engine** (`ruvector-graph`) with full Cypher executor + hyperedges + distributed transactions, plus `ruvector-graph-transformer` with 8 verified module families (physics, biological, manifold, temporal-causal, economic, self-organizing, sublinear, proof-gated).
- A **solver suite** (`ruvector-solver`) with 8 sublinear algorithms (Neumann series, conjugate gradient, forward/backward push PageRank, BMSSP multigrid, TRUE solver, random walk).
- A **mincut subsystem** (`ruvector-mincut`) with canonical, parallel, link-cut, sparsifier, JTree, witness — used everywhere ("mincut-gated transformer", "consciousness via mincut Φ", "quantum coherence via mincut").
- An **attention library** (`ruvector-attention`) with FlashAttention, MLA, Mamba SSM, hyperbolic attention (Poincaré + Lorentz cascade + mixed curvature), graph attention, info-bottleneck, info-geometry, sparse, sheaf, transport, PDE attention, MoE attention, speculative decoding, KV-cache management. About 50 mechanisms total.
- A **local LLM runtime** (`ruvllm`) with GGUF loading, BitNet, MoE, paged attention, quantization (TurboQuant 2-4 bit KV-cache), policy store, reasoning bank integration, reflection, Metal/CUDA/WebGPU.
- A **single-file format** (`crates/rvf/`) — RVF cognitive containers with COW branching, witness chains (SHAKE-256 73-byte entries), eBPF integration, ML-DSA-65/SLH-DSA-128s post-quantum signatures, lineage records, attestation records, federated sync.
- An **MCP gate** (`mcp-gate`, `mcp-brain`, `mcp-brain-server`) for Claude/GPT integration.
- A **tiny-dancer router** (FastGRNN-based, sub-10ms intent routing).
- **Specialized hardware backends**: Hailo-8 NPU (Pi 5 + AI HAT+, 9.6× speedup), FPGA transformer, ESP32-S3 (`ruvllm_sparse_attention` is no_std), mmWave radar, hailort-sys.
- **Application examples**: `examples/dna` (genomics, k-mer HNSW), `examples/scipix` (LaTeX OCR), `examples/refrag-pipeline`, `examples/google-cloud`, `examples/subpolynomial-time`.
- **Latest (2026-05-08) addition**: `ruvllm_retrieval_diffusion` — training-free retrieval LM + masked discrete diffusion (MaskGIT cosine schedule) on top of `ruvllm_sparse_attention`. PR #450.

Workspace version: `2.0.5` (per CHANGELOG); npm distribution `ruvector` package; crates.io publishes `ruvector-core@2.0.5`, `ruvector-gnn@2.0.5`, etc.

---

## Document map (read in this order)

| # | File | What it covers |
|---|------|----------------|
| 00 | `00_INDEX.md` (this file) | Map, scope, headline framing of "RuVector is the upstream, not an idea source" |
| 01 | `01_RUVECTOR_ARCHITECTURE.md` | The 135-crate workspace tour: crate families, dependency layering, what's WASM/NAPI/native, CLAUDE.md vs ours |
| 02 | `02_CAPABILITIES_INVENTORY.md` | Per-crate capability catalog (75+ features). What's production, what's research-grade |
| 03 | `03_REALITY_LOOP_APPLICATIONS.md` | Concrete vendor/path-dep transplants to ccreality. Each cites the ccreality doc + the RuVector crate |
| 04 | `04_JEPA_APPLICATIONS.md` | Concrete transplants to ME-JEPA / DynamicJEPA / 5090jepa with crate refs |
| 05 | `05_QUICK_WINS_VS_DEEP_INTEGRATION.md` | Triage. *Different from agentdb's triage* — many "L effort" agentdb ports become "S effort" path-deps here |
| 06 | `06_RISKS_AND_NON_TRANSFERABLE.md` | The honest "don't take these" list — license, build complexity, branding leakage |
| 07 | `07_GIT_HISTORY_LAST_2_DAYS.md` | The 6 commits in last 2 days + what they tell us about active workstreams (sparse-mario / retrieval diffusion / FastGRNN-gated attention / sm_120-relevant) |
| 08 | `08_HONEST_SUMMARY.md` | The verdict — read first if you only have 5 minutes |

---

## The four highest-leverage transplants (revised after reading RuVector)

1. **Path-dep `ruvector-solver` directly.** Stops you from rolling your own Thompson Sampling bandit OR Neumann/CG/BMSSP solvers. ContextGraph already has matrix-optimizer agent and a need for sublinear solvers; `ruvector-solver` is the canonical implementation. See `03 §1`, `04 §1`.

2. **Path-dep `crates/rvf/rvf-crypto` for witness chains + post-quantum signatures.** Gives you the 73-byte SHAKE-256 chain, ML-DSA-65, SLH-DSA-128s, ed25519 — independent of any other RVF code you don't want. ~700 LOC sub-crate. Drops straight into ccreality optimizer audit + DynamicJEPA training-run audit. See `03 §2`, `04 §2`.

3. **Vendor or path-dep `crates/sona`** if you want the full ReasoningBank + EWC++ + micro-LoRA + REINFORCE trajectory + 3 learning loops in Rust. This is the closest thing the open-source ecosystem has to "agentdb but in Rust." See `03 §3`, `04 §3`.

4. **Path-dep `ruvector-temporal-tensor`** for the access-pattern-driven 8/7/5/3-bit tier policy compression. Strictly better than your existing PQ-8 path. See `04 §4`.

The remainder are individual quality-of-life wins ranked in `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`.

---

## What's notably absent from RuVector

- **No SWE-bench harness.** Like agentdb, this is memory + compute infra, not a model-running harness for repair tasks.
- **No DynamicJEPA-style world-model protocol.** RuVector has predictors (sona, tiny-dancer, ruvllm) but no "domain pack → instrument panel → trajectory predictor → planner/heads" pipeline. They're orthogonal — RuVector substrates DynamicJEPA, doesn't replace it.
- **No CUDA sm_120 kernels.** Their attention/GNN crates use `candle` (which supports CUDA but not sm_120 by default) and Metal. The Hailo-8 path is NPU, not GPU. You still own the sm_120a kernel work.

---

## How to use this doc set

1. Skim `08_HONEST_SUMMARY.md` (5 min).
2. Read `02_CAPABILITIES_INVENTORY.md` to find a crate by capability.
3. Decide path-dep vs vendor vs port using `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`.
4. Read the relevant `03/04 §` for the concrete integration shape.
5. Sanity-check via `06_RISKS_AND_NON_TRANSFERABLE.md` before committing.
6. Write the locked decision to `./memory/decisions/ruvector-<dep>.md` per CLAUDE.md §6.

These docs **complement** the agentdb findings (`docs/agentdbfindings/`); they don't replace them. agentdb's findings cover the *high-level patterns* worth copying. RuVector's findings cover *the canonical Rust implementation* of those same patterns — which is what you actually want as a Rust workspace.
