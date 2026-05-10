# RuVector Findings — Index

> **📊 2026-05-10 HARDENING + FSV SNAPSHOT.** ME-JEPA Phases 1-5 have been re-run through manual Full State Verification with source-of-truth readbacks. Phase 1a's operational 11-language AST chunking API now lives in `context_graph_core::memory::ast` and passes FSV for Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, and PHP; the legacy `AstCodeChunker` type remains Rust-only for backward compatibility and should be converged/deprecated. Phase 1b's real model artifact control surface passes FSV: schema v1 config, 21 declared registrations, 19 active digest-verified registrations, 12 active content embedders, E5/E11 retired, RTX 5090 VRAM budget, and 143 routing rows. Phases 1c, 2, 3, 4a, 4b, and 5 are SHIPPED with FSV evidence. `compute_delta_xi` now fails closed on any non-zero target collapse with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. MCP tools `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report` are live. Current evidence ledger and blindspot register: [`15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`](./15_PHASE_1_5_HARDENING_FSV_2026_05_10.md). Authoritative status table: [`specs/_README.md`](./specs/_README.md). Per-phase deliverable status: [`specs/implementation-plan.md`](./specs/implementation-plan.md).

> **🔁 PIVOT 2026-05-09 — read [`10_MEJEPA_PIVOT_2026_05_09.md`](./10_MEJEPA_PIVOT_2026_05_09.md), [`11_MEJEPA_LEARNING_FORMULA.md`](./11_MEJEPA_LEARNING_FORMULA.md), [`12_EMBEDDER_INTEGRATION.md`](./12_EMBEDDER_INTEGRATION.md), AND [`14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`](./14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md) FIRST.** Doc 14 is load-bearing because every embedder in doc 12 must consume **AST-perfect atomic chunks** (function / class / impl / module level) from the workspace's **11 wired tree-sitter parsers** (Rust, Python, JS, TS, Go, Java, C, C++, C#, Ruby, PHP) — not raw text — for the panel materialization pipeline to produce coherent supervision signal. The previous "outer Claude + inner Qwen" two-loop architecture is retired. The system is now **a single outer reality loop around Claude Code, capturing all durable reality signals as training data for ME-JEPA-Code**. ME-JEPA is the predictive verifier; Claude Code is the patch producer (against SWE-bench Lite 300 sequentially); the reality loop's only purpose is to feed ME-JEPA. The learning formula adopts **UTML v1.0's** `L_step = ΔP × ΔK × ΔΩ × ΔΞ` (Predictive Error × Knowledge Coherence × Optimizer-State Health × Representational Non-Collapse) — four multiplicative factors, with ΔΞ being the ML-native term that has no biological analogue. **ME-JEPA is structurally collapse-immune on the target side because its target panel is produced by frozen externally-defined instruments instead of a learnable target encoder.** With Qwen3.6 removed and the 14 production embedders (E1-E14) loaded warm, **every Claude Code action yields up to 105 structured supervisory signals per input** — 14 single-embedder + 91 cross-embedder pair signals — per **Royse 2026c "Teleological Constellation Training" (DOI 10.13140/RG.2.2.24370.57288)**. The DDA configuration is **structurally outside the Shumailov 2024 collapse regime** (no generator-in-loop recursion). See docs 11 + 12.
>
> **⭐ End Goal (operator-stated 2026-05-08, reaffirmed 2026-05-09):** make code review obsolete by making AI-generated patches **self-verifiable against bytes-on-disk reality**. The system being built is a **reality compiler** — ME-JEPA-Code: a Joint Embedding Predictive Architecture that takes (code change + context) and outputs a predicted reality state, calibrated against the SWE-bench Docker oracle. When prediction-vs-oracle correlation crosses ~0.95, no human code review of AI patches is needed.
>
> **Read order for a fresh agent:** **doc 10** (post-pivot architecture) → **doc 11** (learning formula) → **doc 09** (master plan; sections that reference Qwen/inner-actor are re-framed by doc 10) → `specs/_README.md` + `specs/implementation-plan.md` (canonical status/roadmap after commit `6f72a02e` removed the legacy `sitrep/` tree). Docs 01-08 are historical reference for clean-room reimplementation rationale.
>
> Doc 09's master plan covers: the reality-compiler architecture, the code-domain instrument set (now 14 instruments — original 11 `E_AST`, `E_CFG`, `E_DataFlow`, `E_TypeGraph`, `E_Test`, `E_Trace`, `E_Diff`, `E_Witness`, `E_Oracle`, `E_Problem`, `E_CommitMsg` + 3 added by §3.5: `E_StaticAnalysis`, `E_Runtime`, `E_Reasoning`), the expanded panel layout (5,120-d, was 4,096-d), the predictor with **frozen instrument targets**, the SWE-bench Lite mutation corpus (8 categories × 300 tasks = ~2,400 labeled examples), the training procedure (VICReg loss + Huber predict + multi-objective aux-head loss), conformal calibration, OOD detection, **§3.5 maximal reward-signal capture from durable reality** (static-analysis, runtime, cross-attempt, counterfactual signals from existing harness artifacts), **§3.6 blindspots and what we'd unlock by capturing them** (adjacent-test outcomes, stack-trace embeddings, operator overrides, per-hunk attribution, outer-loop agent reasoning), **§3.7 optimal use** (multi-objective loss with α weights, hard-example mining, counterfactual training, active learning, cross-task transfer, aux-head distillation), **§3.8 real-time pipeline** (Tokio shift-log subscriber wiring `cgreality-shift-log` + harness artifacts → `mejepa.observe()` in < 5 s p50 / < 30 s p99, closing the see→predict→observe→update loop so the model is always synced with reality), self-healing (online learning + EWC++ + drift detection + A/B/C promotion), disk + memory hygiene (tiered 8/7/5/3-bit compression + nightly GC + witness-chain Merkle compression), and an **8-phase build plan** with definitions of done.

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

## Current ContextGraph implementation state (updated 2026-05-08)

This doc set is no longer purely pre-implementation. The workspace now contains:

- **`crates/context-graph-witness/` implemented** — 73-byte SHAKE-256 witness entries, chain verification, and file-backed ccreality optimizer witness tools (`optimizer_witness_chain_verify`, `optimizer_witness_chain_diff`). DynamicJEPA audit rows are also bound to RocksDB CF `dj_audit_witness_chain` with readback/tamper FSV.
- **`crates/context-graph-solver/` partially implemented** — clean-room weighted Forward-Push PPR, validated Conjugate Gradient, CSR validation, structured fail-closed errors, and manual FSV evidence at `target/context-graph-solver-fsv/manual-evidence.json`.
- **Still pending:** BMSSP/multigrid, the ccreality `optimizer_compute_influence` MCP tool, JEPA sampler integration, SONA/EWC++, tiered 8/7/5/3-bit compression, conformal prediction, retrieval diffusion, mincut, Granger, and the ME-JEPA-Code Phase 0 mutation corpus.

When this index says "reimplement," read it as either **done**, **partial**, or **pending** according to the state above and the per-doc status notes below.

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
| 03 | `03_REALITY_LOOP_APPLICATIONS.md` | Concrete clean-room reimplementation targets for ccreality. Each cites the ccreality doc + the RuVector reference crate |
| 04 | `04_JEPA_APPLICATIONS.md` | Concrete transplants to ME-JEPA / DynamicJEPA / 5090jepa with crate refs |
| 05 | `05_QUICK_WINS_VS_DEEP_INTEGRATION.md` | Triage after the locked clean-room decision; earlier path-dep assumptions are superseded |
| 06 | `06_RISKS_AND_NON_TRANSFERABLE.md` | The honest "don't take these" list — license, build complexity, branding leakage |
| 07 | `07_GIT_HISTORY_LAST_2_DAYS.md` | The 6 commits in last 2 days + what they tell us about active workstreams (sparse-mario / retrieval diffusion / FastGRNN-gated attention / sm_120-relevant) |
| 08 | `08_HONEST_SUMMARY.md` | The verdict — read first if you only have 5 minutes |
| 09 | `09_END_GOAL_REVIEW_REPLACEMENT.md` | **🎯 THE MASTER PLAN.** Build-ready specification for ME-JEPA-Code (the reality compiler). Architecture, instrument set, mutation corpus, training procedure, self-healing, hygiene, 8-phase build plan with verification protocols. **Sections referencing Qwen / inner-actor are re-framed by doc 10.** |
| 10 | `10_MEJEPA_PIVOT_2026_05_09.md` | **🔁 POST-PIVOT ARCHITECTURE — read this first.** Statement of the single-outer-loop design. The inner-actor LLM is retired; Claude Code is the patch producer; the reality loop's only purpose is to capture training data for ME-JEPA. The "three questions ME-JEPA must answer" framing. |
| 11 | `11_MEJEPA_LEARNING_FORMULA.md` | **🧠 LEARNING FORMULA.** UTML v1.0's four-factor `L_step = ΔP × ΔK × ΔΩ × ΔΞ` operationalized for ME-JEPA. ΔΞ.target_collapse = 0 by construction (frozen instruments). Predictor-side defenses (VICReg + pairwise MI + constellation guard + negative-action ablation falsifier). Per-example sampling weights, consolidation integral M(t), state-transfer T, training cycle, pruning rule, implementation roadmap. |
| 12 | `12_EMBEDDER_INTEGRATION.md` | **🧬 EMBEDDER INTEGRATION.** Inventories the **21 embedders (E1-E14 content + E15-E21 learner-state)** + the shipped LoRA / distillation / multitask / AdamW / InfoNCE training infrastructure. Maps the 21 embedders onto doc 09's 14 ME-JEPA instruments — 8 of 14 become thin heads on existing embedders (~25-day Phase 1 vs ~35 from scratch). ~29 GB VRAM free post-Qwen. Cross-language E14 BGE-M3 satisfies the Section C mandate. UTML v1.0 ΔK / ΔΩ / ΔΞ instrumentation via the embedder ensemble. This is the canonical doc-12 reference; the older sibling filename was removed. |
| 13 | `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` | **🌌 TCT / DDA / MEANING COMPRESSION (operator's published research applied).** Direct application of Royse (April 2026) — *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall* (DOI 10.13140/RG.2.2.24370.57288) — to ME-JEPA-Code. **Derived Data Abundance (DDA)**: at N=21, every input yields up to `n·(N + (N choose 2)) = n·231` structured signals (or `n·462` at the loose upper bound). **Meaning compression**: a fourth compression category (signals per raw byte). **TCT three-phase**: Phase 1 build CP centroids → Phase 2 train with `L_constellation_centroid` loss → Phase 3 per-output `Gτ` deterministic geometric guard. **Same frozen embedders in three roles** (labeller / loss target / runtime guard) — the operational form of `ΔΞ.target_collapse = 0 by construction`. Adds `Verdict::GuardRejected` variant for per-output verifiability that scalar-reward alignment (RLHF/DPO/Constitutional AI) cannot provide. ContextGraph cited in the source paper as one of two reference implementations. |
| 14 | `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` | **🌳 AST × EMBEDDER ATOMIC CHUNKING.** The repo has **11 wired tree-sitter parsers** (Rust, Python, JS, TS, Go, Java, C, C++, C#, Ruby, PHP) plus an existing `AstCodeChunker` with FSV. Wires AST-atomic chunks (function/method/class/struct/impl/module level) through the 14 frozen embedders so every code change produces panel triples *at AST-node granularity*. Multiplies signal yield by ~10× (chunks/file ratio): ~50K-200K signals per /loop turn, ~5M-20M per day per Claude Code instance. Uses the per-(mutation × entity_type × language × embedder) constellation cell math, per-chunk-type embedder routing table, roadmap, and propagation matrix. The older sibling filename was removed; this is the canonical doc-14 reference. |
| 15 | `15_PHASE_1_5_HARDENING_FSV_2026_05_10.md` | **🧾 CURRENT HARDENING LEDGER.** Source-of-truth FSV evidence for Phases 1-5, the `ΔΞ.target_collapse` root-cause fix, the training-evidence byte-readback fix, what exists now, what still needs deep integration, and blindspots that would hide critical capability gaps. Read this before making or reviewing any "Phase 1-5 complete" claim. |

---

## The four highest-leverage primitives to clean-room reimplement

1. **A sublinear solver suite (PPR + BMSSP + CG)** — **partial.** `crates/context-graph-solver/` now has Forward-Push PPR + CG + CSR validation + FSV evidence. BMSSP/multigrid and ccreality/JEPA integration are still pending. See `03 §1`, `04 §2`.

2. **A 73-byte SHAKE-256 witness chain + ed25519/ML-DSA-65 signing** — **core chain implemented.** `crates/context-graph-witness/` exists and is wired into ccreality optimizer files plus DynamicJEPA audit witnesses. ed25519/ML-DSA-65 signing, lineage, and attestation remain pending. See `03 §2`, `04 §1`.

3. **An adaptive learning engine (μLoRA + EWC++ + REINFORCE trajectory + ReasoningBank)** — **pending.** No `crates/context-graph-sona/` crate yet. See `03 §3`, `04 §3`.

4. **Access-pattern-driven 8/7/5/3-bit tier compression** — **pending.** Current PQ-8/Float8/token-pruning quantization exists elsewhere, but this access-pattern tier policy is not implemented. See `04 §4`.

The remainder are individual quality-of-life primitives ranked in `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`. **Any older import/path-dep effort estimate should be treated as historical and replaced by the current clean-room estimates in `05`.**

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

**Important:** docs 03-08 contain historical framing about "verify_ai_output gates." That framing is SUPERSEDED by doc 09's reality-compiler framing — gates are now *input features* to ME-JEPA, ME-JEPA itself is the verifier. When the supporting docs disagree with doc 09, doc 09 wins.
