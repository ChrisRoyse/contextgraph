# RuVector Findings — Index

> **⭐ End Goal (operator-stated 2026-05-08):** make code review obsolete by making AI-generated patches **self-verifiable against bytes-on-disk reality**. The system being built is a **reality compiler** — ME-JEPA-Code: a Joint Embedding Predictive Architecture that takes (code change + context) and outputs a predicted reality state, calibrated against the SWE-bench Docker oracle. When prediction-vs-oracle correlation crosses ~0.95, no human code review of AI patches is needed.
>
> **Read [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md) FIRST.** That is the **build-ready master plan** — a fresh AI agent can pick it up and execute. Every other doc in this set (00-08) is reference material that informs *how* primitives are clean-room reimplemented; doc 09 is *what* gets built and *why*.
>
> Doc 09's master plan covers: the reality-compiler architecture, the code-domain instrument set (`E_AST`, `E_CFG`, `E_DataFlow`, `E_TypeGraph`, `E_Test`, `E_Trace`, `E_Diff`, `E_Witness`, `E_Oracle`, `E_Problem`, `E_CommitMsg`), the panel layout (4,096-d), the predictor + EMA target encoder, the SWE-bench Lite mutation corpus (8 categories × 300 tasks = ~2,400 labeled examples), the training procedure (VICReg loss + Huber predict), conformal calibration, OOD detection, self-healing (online learning + EWC++ + drift detection + A/B/C promotion), disk + memory hygiene (tiered 8/7/5/3-bit compression + nightly GC + witness-chain Merkle compression), and an **8-phase build plan** with definitions of done.

> **🔒 LOCKED 2026-05-08 — Reference-Only, Clean-Room Reimplementation Only**
>
> Per operator decision (`memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`):
> - **Never vendor.** Do not copy any `ruvector/` source files into our tree.
> - **Never path-dep.** Do not add `path = "../ruvector/..."` to any `Cargo.toml`. The cloned tree never appears in our build graph.
> - **Never npm-depend.** Do not add `ruvector` or `@ruvector/*` packages to any `package.json`.
> - **Never track upstream.** No `git pull` of the clone; the snapshot at HEAD `ef5274c2` is the only state we'll ever look at.
> - **Reference-only.** Read the code to understand each algorithm; reimplement cleanly in our own crates with our own naming, error types, and crate boundaries.
>
> **All recommendations below that say "Vendor `<crate>`" or "Path-dep `<crate>`" are SUPERSEDED.** Read them as: "Reimplement the idea behind `<crate>` cleanly in our own code, after reading their source as inspiration." Effort estimates roughly **2-3× the original numbers** because clean-room reimplementation is more work than path-dep.
>
> Provenance comments in our source files cite their inspirational source file + commit (e.g., `// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/witness.rs at HEAD ef5274c2`). Provenance only — no code copied, no license attribution required.

**Date:** 2026-05-08
**Author:** Claude Code (outer reality-loop optimizer, doing direct research per operator preference)
**Source repo:** `ruvector/` (cloned 2026-05-08 from https://github.com/ruvnet/RuVector at HEAD `ef5274c2`; **read-only reference**, NOT a build dependency)
**Scope:** Cargo workspace audit (135 crates, 2,492 commits, 277 KB README); deep read of `crates/ruvector-core/`, `crates/sona/`, `crates/rvf/rvf-crypto/`, `crates/ruvector-attention/`, `crates/ruvector-graph-transformer/`, `crates/ruvector-solver/`, `crates/ruvector-temporal-tensor/`, `crates/ruvllm/`, `crates/ruvllm_retrieval_diffusion/`, `crates/ruvllm_sparse_attention/`, plus `docs/adr/` (206 ADRs).
**Audience:** future ccreality optimizer turns + 5090jepa/ME-JEPA fresh agents.

---

## TL;DR — what changes if you read RuVector after agentdb

RuVector is the **upstream** that agentdb wraps. Per the locked rule above, we treat it as **read-only reference** — NOT as a dependency candidate.

- Every "good idea" I flagged from agentdb (SHAKE-256 73-byte witness chain, SolverBandit, NightlyLearner, CausalRecall, MMR, ContextSynthesizer, SONA, ContrastiveTrainer, etc.) **already exists as a working Rust implementation here**. We read those implementations to understand the algorithms; we reimplement cleanly in our own crates.
- Where agentdb has a 270-line TypeScript SolverBandit, RuVector has the 312-line `ruvector-solver` Rust crate plus 8 algorithmically distinct sublinear solvers (Neumann, CG, Forward Push, BMSSP multigrid, etc.). Same primitives. We borrow the *design*, not the *code*.
- The 73-byte SHAKE-256 witness chain documented as "port from agentdb" is at `ruvector/crates/rvf/rvf-crypto/src/witness.rs:24` (`ENTRY_SIZE: usize = 73`). We read it; we write our own.
- agentdb's Reflexion + SkillLibrary + CausalMemoryGraph map onto RuVector's `sona` (ReasoningBank, EWC++, micro-LoRA, federated coordinator, training pipeline), `ruvector-graph`, `ruvector-coherence`, `prime-radiant`. These are *inspirational architecture references*, not dependency candidates.

The cloned trees stay on disk solely because reading them is faster than reading them on GitHub. They're never wired into `Cargo.toml`, `package.json`, or any build artifact.

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
| 09 | `09_END_GOAL_REVIEW_REPLACEMENT.md` | **🎯 THE MASTER PLAN — read this first.** Build-ready specification for ME-JEPA-Code (the reality compiler). Architecture, instrument set, mutation corpus, training procedure, self-healing, hygiene, 8-phase build plan with verification protocols. A fresh AI agent picks this up and executes. |

---

## The four highest-leverage primitives to clean-room reimplement

1. **A sublinear solver suite (PPR + BMSSP + CG)** — design the algorithms after reading `ruvector/crates/ruvector-solver/src/{forward_push,bmssp,cg}.rs`. Reimplement in our own `crates/context-graph-solver/` (or extend an existing crate). See `03 §1`, `04 §1`.

2. **A 73-byte SHAKE-256 witness chain + ed25519/ML-DSA-65 signing** — design after reading `ruvector/crates/rvf/rvf-crypto/src/{witness,sign,attestation,lineage}.rs`. Reimplement in our own `crates/context-graph-witness/` (or similar). Drops into both ccreality optimizer audit + DynamicJEPA training-run audit. See `03 §2`, `04 §1`.

3. **An adaptive learning engine (μLoRA + EWC++ + REINFORCE trajectory + ReasoningBank)** — design after reading `ruvector/crates/sona/src/`. Reimplement in our own `crates/context-graph-sona/`. Closest open-source reference for Rust-native trajectory learning. See `03 §3`, `04 §3`.

4. **Access-pattern-driven 8/7/5/3-bit tier compression** — design after reading `ruvector/crates/ruvector-temporal-tensor/src/`. Reimplement in our own quantization module. Strictly richer than current PQ-8 path. See `04 §4`.

The remainder are individual quality-of-life primitives ranked in `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`. **Every effort estimate in `05` should be multiplied by 2-3× to convert path-dep estimates into clean-room reimplementation estimates.**

---

## What's notably absent from RuVector

- **No SWE-bench harness.** Like agentdb, this is memory + compute infra, not a model-running harness for repair tasks.
- **No DynamicJEPA-style world-model protocol.** RuVector has predictors (sona, tiny-dancer, ruvllm) but no "domain pack → instrument panel → trajectory predictor → planner/heads" pipeline. They're orthogonal — RuVector substrates DynamicJEPA, doesn't replace it.
- **No CUDA sm_120 kernels.** Their attention/GNN crates use `candle` (which supports CUDA but not sm_120 by default) and Metal. The Hailo-8 path is NPU, not GPU. You still own the sm_120a kernel work.

---

## How to use this doc set

**For an AI agent picking up the build:**

1. Read `09_END_GOAL_REVIEW_REPLACEMENT.md` top-to-bottom — that's the master plan.
2. Pick up at the Phase plan (Section 7); execute Phase 0 first.
3. When implementing a primitive, look it up in `02_CAPABILITIES_INVENTORY.md` for the canonical Rust reference to read.
4. Sanity-check the locked clean-room rule via `06_RISKS_AND_NON_TRANSFERABLE.md` — never path-dep, never vendor, never npm-depend.

**For a human reviewer / operator:**

1. Skim `08_HONEST_SUMMARY.md` (5 min) for the verdict.
2. Read `09_END_GOAL_REVIEW_REPLACEMENT.md` Section 0-2 for the architecture, Section 7 for the build plan.
3. Use `01_RUVECTOR_ARCHITECTURE.md` and `02_CAPABILITIES_INVENTORY.md` as reference when reviewing any specific clean-room reimplementation.

**Important:** docs 03-08 contain earlier framings that talk about "vendor / path-dep" and "verify_ai_output gates." Those framings are SUPERSEDED by doc 09's reality-compiler framing — gates are now *input features* to ME-JEPA, ME-JEPA itself is the verifier. When the supporting docs disagree with doc 09, doc 09 wins.
