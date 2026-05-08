# 01 — RuVector Architecture

> **🔒 Reference-only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — never vendor, path-dep, or npm-depend on RuVector. The architecture below is documented to inform our own clean-room reimplementations, not to enable adoption of their crates.

> **🎯 Master plan:** [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md) is the build-ready specification for **ME-JEPA-Code** — a reality compiler that predicts oracle outcomes from `(code, context)` and replaces line-by-line code review. The architecture below is the *inspirational substrate*; doc 09 is *what we build*. **Doc 09 §3.5–§3.8** specifies maximal reward-signal capture from durable reality (static-analysis + runtime + cross-attempt + counterfactual signals → expanded 5,120-d panel), the blindspots we'd unlock by capturing them, the multi-objective loss + counterfactual training that uses them, and a real-time shift-log subscriber that feeds reality back into ME-JEPA in < 5 s p50 / < 30 s p99. When this doc disagrees with doc 09, doc 09 wins.

> **Current ContextGraph state (2026-05-08):** this architecture remains a reference map. ContextGraph has clean-room implementations for the witness-chain core and a solver foundation (`Forward-Push PPR + CG`). It does **not** currently contain RuVector's full solver router, BMSSP multigrid, SONA, temporal tensor compression, retrieval diffusion, or graph-transformer modules.

## One-line definition

**RuVector is a 135-crate Rust workspace** that ships a self-learning vector database, a graph engine with Cypher + 8 graph-transformer module families, an attention library with ~50 mechanisms, a local LLM runtime, a single-file `.rvf` cognitive container format with post-quantum signatures and witness chains, an MCP gate for Claude/GPT integration, and dozens of WASM/NAPI/Hailo/FPGA/ESP32 backends.

Each crate has at minimum a `core` variant. Most have `wasm` and `node` variants too — same source tree, different feature flags + entry points.

## The crate-naming pattern

```
ruvector-<feature>            ← canonical Rust crate
ruvector-<feature>-wasm       ← WASM/browser export
ruvector-<feature>-node       ← N-API binding for Node.js
ruvector-<feature>-cli        ← stand-alone CLI
ruvector-<feature>-ffi        ← C FFI binding
```

So `ruvector-router-{core,cli,ffi,wasm}` is one logical primitive in four shipping forms. The `-wasm` and `-node` crates are mostly thin wrappers over `-core` types — the algorithm lives in the core, the wrappers do `Float32Array` ↔ `Vec<f32>` marshaling.

## The 9 layered families

Reading the `Cargo.toml` `members = [...]` list and grouping by domain:

```
                     ┌────────────────────────────────────────────┐
LAYER 9              │  Application examples                       │
                     │  refrag-pipeline · scipix · dna · openfang │
                     │  google-cloud · subpolynomial-time · delta  │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 8              │  Specialized domains                        │
                     │  prime-radiant   ruvector-consciousness     │
                     │  ruvector-nervous-system  ruQu (quantum)    │
                     │  cognitum-gate  agentic-robotics            │
                     │  neural-trader  rvAgent  rvm  ruvix         │
                     │  ruvector-kalshi  thermorust                │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 7              │  Hardware backends                          │
                     │  ruvector-hailo (Pi 5 + AI HAT+ NPU)        │
                     │  hailort-sys                                │
                     │  ruvector-fpga-transformer                  │
                     │  ruvector-mmwave (radar)                    │
                     │  ruos-thermal (Pi 5 thermal supervisor)     │
                     │  ruvllm_sparse_attention (no_std/ESP32-S3)  │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 6              │  Local LLM + reasoning                       │
                     │  ruvllm  ruvllm-cli  ruvllm-wasm             │
                     │  ruvllm_sparse_attention                     │
                     │  ruvllm_retrieval_diffusion (NEW 2026-05-08)│
                     │  ruvector-tiny-dancer-{core,node,wasm}       │
                     │  ruvector-router-{core,cli,ffi,wasm}         │
                     │  ruvector-sparse-inference                   │
                     │  cognitum-gate-{kernel,tilezero}             │
                     │  mcp-{gate,brain,brain-server}               │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 5              │  Self-learning + audit                       │
                     │  sona (ReasoningBank, EWC++, μLoRA, fed.)    │
                     │  ruvector-coherence (signal quality)         │
                     │  ruvector-domain-expansion (transfer learn.) │
                     │  ruvector-verified (proof envs, attestation) │
                     │  ruvector-temporal-tensor (8/7/5/3-bit tier) │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 4              │  Graph + attention                           │
                     │  ruvector-graph (Cypher, hyperedges)         │
                     │  ruvector-graph-transformer                  │
                     │    8 modules: physics · biological · manifold│
                     │      · temporal · economic · self-organizing │
                     │      · sublinear · proof-gated               │
                     │  ruvector-attention (~50 mechanisms)         │
                     │    flash · MLA · Mamba SSM · MoE · sparse    │
                     │    hyperbolic (Poincaré + Lorentz)           │
                     │    info-bottleneck · sheaf · transport · PDE │
                     │  ruvector-attn-mincut · ruvector-cnn         │
                     │  ruvector-gnn · ruvector-mincut-gated-trans. │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 3              │  Compute primitives                          │
                     │  ruvector-solver (8 sublinear algorithms)    │
                     │    Neumann · CG · Forward Push · BMSSP       │
                     │    TRUE solver · backward push · random walk │
                     │  ruvector-mincut (canonical, parallel,       │
                     │    link-cut, JTree, sparsifier, witness)     │
                     │  ruvector-sparsifier (spectral, PPR)         │
                     │  ruvector-math · ruvector-math-wasm          │
                     │  ruvector-decompiler (DAG analysis)          │
                     │  ruvector-dag                                │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 2              │  Single-file format + crypto                 │
                     │  rvf (workspace-excluded sub-workspace):     │
                     │    rvf-types · rvf-wire · rvf-manifest       │
                     │    rvf-runtime · rvf-quant · rvf-launch      │
                     │    rvf-crypto (SHAKE-256 · ed25519 ·         │
                     │      ML-DSA-65 · SLH-DSA-128s · 73-byte      │
                     │      WITNESS_SEG · attestation · lineage)    │
                     │    rvf-ebpf · rvf-federation · rvf-import    │
                     │    rvf-cli · rvf-server · rvf-node           │
                     │    rvf-wasm · rvf-solver-wasm · rvf-adapters │
                     │  rvlite (lightweight RVF subset)             │
                     │  rvf-cow (COW branching, 1M vec → 2.5MB)     │
                     └────────────────────────────────────────────┘

                     ┌────────────────────────────────────────────┐
LAYER 1              │  Vector storage + index                      │
                     │  ruvector-core (the canonical engine)        │
                     │    HNSW · DiskANN/Vamana · flat              │
                     │    BM25 + RRF hybrid · Graph RAG (Leiden)    │
                     │    ColBERT multi-vec · Matryoshka · OPQ      │
                     │    LSM compaction · sparse vec · MMR · CP    │
                     │  ruvector-postgres (pgrx PG extension)       │
                     │  ruvector-rabitq (RaBitQ binary quant)       │
                     │  ruvector-acorn (faceted ANN)                │
                     │  ruvector-snapshot · ruvector-collections    │
                     │  ruvector-filter · ruvector-metrics          │
                     │  ruvector-bench · ruvector-cli               │
                     │  ruvector-{cluster,raft,replication}         │
                     │  ruvector-rulake (lake-ish multi-collection) │
                     │  ruvector-{delta-core,delta-graph,           │
                     │    delta-index,delta-consensus,delta-wasm}   │
                     │  ruvector-hyperbolic-hnsw                    │
                     │  ruvector-diskann · ruvector-diskann-node    │
                     │  ruvector-dither (vector dithering)          │
                     │  ruvector-crv · ruvector-profiler            │
                     └────────────────────────────────────────────┘
```

Each layer mostly depends only on lower layers. ruvector-core is the trunk; everything else either decorates it or replaces a sub-component.

## What's actually built — workspace `members` vs `exclude`

Workspace builds 100+ crates by default but **excludes**:
- `crates/rvf/*` — its own sub-workspace; build separately when you want the RVF format.
- `crates/ruvector-postgres` — pgrx requires `cargo install cargo-pgrx --version 0.12.9` + `cargo pgrx init` first.
- `crates/micro-hnsw-wasm` — WASM-specific build path.
- `crates/ruvector-hyperbolic-hnsw{,-wasm}` — hyperbolic-HNSW research crate.
- `crates/mcp-brain-server` — separate build (server target).
- `crates/ruos-thermal` — Pi 5 thermal supervisor; rejoins workspace when daemon mode + Unix socket protocol land.
- Several `examples/` directories — `ruvLLM` family, `data`, `delta-behavior`, `edge-net`, `ruvLLM/esp32{,-flash}`.

**Implication for our clean-room work**: per the locked rule, we never path-dep or vendor any RuVector crate. The fact that `crates/rvf/*` is in workspace `exclude` is informational — it tells us the RuVector team treats `rvf-crypto` as a sub-workspace concern. We read the source from there as inspiration; we reimplement in our own crate.

## Major sub-systems in detail

### 1. `ruvector-core` — the vector trunk

`crates/ruvector-core/src/`:
- **index/** — `flat.rs`, `hnsw.rs`. Two indexes; everything else builds on these.
- **advanced/** — `hypergraph.rs`, `learned_index.rs`, `neural_hash.rs`, `tda.rs` (topological data analysis).
- **advanced_features/** — `compaction.rs` (LSM), `conformal_prediction.rs`, `diskann.rs` (Vamana), `filtered_search.rs`, `graph_rag.rs` (Leiden community detection), `hybrid_search.rs` (BM25+vector RRF), `matryoshka.rs`, `mmr.rs` (diversity rerank), `multi_vector.rs` (ColBERT), `opq.rs`, `product_quantization.rs`, `sparse_vector.rs`.
- Plus `agenticdb.rs` (the agentic-DB layer), `arena.rs`, `cache_optimized.rs`, `distance.rs`, `embeddings.rs`, `error.rs`, `lockfree.rs`, `memory.rs`, `quantization.rs`, `simd_intrinsics.rs`, `storage*.rs`, `vector_db.rs`.

This single crate has *more* retrieval variety than most production vector DBs. It is the first place to study when designing our own self-contained ANN/retrieval modules.

### 2. `sona` — the self-learning engine

`crates/sona/src/`:
- **engine.rs** — `SonaEngine` main type.
- **lora.rs** — `MicroLoRA` (rank 1-2), `BaseLoRA` (standard rank), `LoRAEngine`, `LoRALayer`.
- **ewc.rs** — `EwcPlusPlus` (Elastic Weight Consolidation++) with online Fisher info, multi-task circular buffer, automatic task-boundary detection, adaptive lambda scheduling.
- **trajectory.rs** — `TrajectoryBuffer`, `TrajectoryBuilder`, `TrajectoryIdGen`, REINFORCE gradient estimation.
- **reasoning_bank.rs** — `ReasoningBank` + `PatternConfig`, pattern extraction + clustering.
- **loops/** — `instant.rs` (sub-ms reaction), `background.rs` (offline consolidation), `coordinator.rs` (orchestration), `mod.rs`.
- **types.rs** — `LearningSignal`, `QueryTrajectory`, `TrajectoryStep`, `LearnedPattern`, `PatternType`, `SonaConfig`.
- **export/** (feature `serde-support`) — `dataset.rs`, `huggingface_hub.rs`, `pretrain.rs`, `safetensors.rs`. Yes, you can export a trained SONA state to HuggingFace Hub.
- **training/** — `factory.rs`, `federated.rs`, `metrics.rs`, `pipeline.rs`, `templates.rs`. Includes `FederatedCoordinator`, `EphemeralAgent`, `TrainingPipeline`, `BatchConfig`, `EpochStats`, `AgentExport`.
- **wasm.rs** — 718 lines of WASM bindings.
- **napi_simple.rs** — Node N-API bindings.

This is the Rust source of agentdb's `@ruvector/sona@^0.1.4` dependency. It is dramatically more capable than what agentdb exposes.

**Important defaults** (from `crates/sona/src/ewc.rs:35-48`):
- `initial_lambda: 2000.0` — described as "OPTIMIZED: Better forgetting prevention."
- `max_lambda: 15000.0` — "OPTIMIZED: Higher ceiling for multi-task."
- `fisher_ema_decay: 0.999`.
- `boundary_threshold: 2.0` — for automatic task boundary detection.

These supersede agentdb's `ewcLambda: 1000` default. If you adopted agentdb's defaults, the SONA team has since tuned them upward.

### 3. `rvf-crypto` — the cryptographic substrate

`crates/rvf/rvf-crypto/src/`:
- **hash.rs** — `shake256_128`, `shake256_256`, `shake256_hash`. SHAKE-256 throughout.
- **footer.rs** — `encode_signature_footer`, `decode_signature_footer`. Per-segment ed25519 signatures.
- **lineage.rs** — `compute_manifest_hash`, `lineage_record_{from,to}_bytes`, `lineage_witness_entry`, `verify_lineage_chain`. Parent/child derivation chains.
- **attestation.rs** — `attestation_witness_entry`, `build_attestation_witness_payload`, `decode_attestation_record`, `decode_tee_bound_key`, `QuoteVerifier`, `TeeBoundKeyRecord`. TEE attestation support.
- **witness.rs** — **the 73-byte SHAKE-256 witness chain** — `WitnessEntry`, `create_witness_chain`, `verify_witness_chain`. `ENTRY_SIZE: usize = 73` (line 25).
- **sign.rs** (feature `ed25519`) — ed25519 segment signing.

This is a **`#![cfg_attr(not(feature = "std"), no_std)]`** crate — runs on ESP32-S3, embedded targets, anywhere. Pulls in `rvf-types` for error types. Otherwise self-contained.

If you reimplement one thing from RuVector inspiration, this is it. ~700 LOC of theirs reads quickly; our clean-room equivalent will be similar size in `crates/context-graph-witness/` (or wherever fits). Drops into ccreality + DynamicJEPA without architectural changes.

### 4. `ruvector-solver` — sublinear algorithm suite

`crates/ruvector-solver/src/`:
- **lib.rs** — top-level traits.
- **traits.rs** — `SolverEngine`, `Solver` traits.
- **types.rs** — `CsrMatrix`, `ComputeBudget`, `SolveResult`.
- **bmssp.rs** — BMSSP multigrid (algebraic multigrid V-cycle, O(nnz·log n)).
- **cg.rs** — Conjugate Gradient.
- **neumann.rs** — Neumann series.
- **forward_push.rs**, **backward_push.rs** — bidirectional push for Personalized PageRank.
- **true_solver.rs** — TRUE (Truncated Random Updates) solver.
- **random_walk.rs** — Random walk sampler.
- **router.rs** — Algorithm selection / routing.
- **simd.rs** — SIMD-accelerated inner loops.
- **arena.rs** — Memory arena for solver workspaces.
- **audit.rs** — Operation audit log.
- **budget.rs** — Compute budget tracking.
- **events.rs** — Event emission.
- **validation.rs** — Result validation.

Each algorithm is `O(log n)` to `O(sqrt n)`. Used by Postgres extension (in-SQL PageRank), graph transformer (sublinear attention), and prime-radiant. If your workspace ever needs PageRank, Laplacian solve, or fast sparse linear solve — this is the canonical answer.

**ContextGraph implementation status:** `crates/context-graph-solver/` now clean-room implements weighted Forward-Push PPR and Conjugate Gradient with CSR validation and manual FSV evidence. BMSSP/multigrid, solver routing, solver audit persistence, and ccreality/JEPA integration are still pending. Do not claim "the solver suite" is complete yet.

### 5. `ruvector-mincut` — graph cuts everywhere

`crates/ruvector-mincut/src/` (28 sub-modules):
- **algorithm/** — core min-cut algorithms.
- **canonical/**, **parallel/**, **localkcut/**, **subpolynomial/** — different complexity classes.
- **linkcut/** — link-cut trees for dynamic graphs.
- **expander/** — expander-graph properties.
- **sparsify/** — graph sparsification.
- **euler/** — Eulerian decompositions.
- **jtree/** — junction tree decompositions.
- **fragment/**, **fragmentation/** — fragment-aware partitioning.
- **witness/** — cut certificates.
- **certificate/** — cut certificates with proofs.
- **integration/** — integration with attention/transformer modules.
- **monitoring/**, **optimization/**, **wasm/**, **wrapper/** — operational layers.
- **snn/** — spiking neural net mincut (! — ties into nervous-system crate).

Used by: `ruvector-mincut-gated-transformer` (dynamic attention pruning), `prime-radiant` (sheaf Laplacian via mincut), `ruvector-consciousness` (mincut Φ for IIT4), `ruQu` (quantum coherence error correction via dynamic mincut).

If DynamicJEPA needs trajectory leakage cuts (per `04 §7` in agentdb findings), this is the right primitive.

### 6. `ruvector-attention` — the attention library

`crates/ruvector-attention/src/`:
- **attention/** — `flash.rs` (FlashAttention), `kv_cache.rs`, `mla.rs` (Multi-Latent Attention), `multi_head.rs`, `scaled_dot_product.rs`, `speculative.rs`, `ssm.rs` (Mamba State Space).
- **sparse/** — `flash.rs` (FlashAttention sparse variant), `linear.rs`, `local_global.rs`, `mask.rs`.
- **hyperbolic/** — `poincare.rs`, `lorentz_cascade.rs`, `mixed_curvature.rs`, `hyperbolic_attention.rs`.
- **moe/** — Mixture-of-Experts attention.
- **info_bottleneck/**, **info_geometry/** — information-theoretic attention regularization.
- **graph/** — graph attention.
- **sheaf/** — sheaf attention (sheaf cohomology over graph).
- **transport/** — optimal-transport attention.
- **pde_attention/** — PDE-based attention (Hamiltonian / diffusion dynamics).
- **curvature/** — curvature-aware attention.
- **topology/** — TDA features in attention.
- **training/** — training utilities.

About 50 distinct mechanisms, all behind feature flags. Feature gates ("flash", "sparse", "hyperbolic", "moe", "graph", etc.) keep the build narrow.

Most relevant to JEPA: **`mla.rs`** (Multi-Latent Attention is a 2024 SoTA technique for KV-cache efficiency), **`sparse/linear.rs`** (linear-time attention), **`hyperbolic/poincare.rs`** (tree-structured retrieval).

### 7. `ruvector-graph-transformer` — the 8 verified modules

`crates/ruvector-graph-transformer/src/`:
- `physics.rs` — Hamiltonian dynamics, gauge-equivariant message passing.
- `biological.rs` — spiking attention, Hebbian/STDP, dendritic branching.
- `manifold.rs` — product manifolds S^n × H^m × R^k.
- `temporal.rs` — causal masking, Granger causality, continuous-time ODE.
- `economic.rs` — Nash equilibrium attention, Shapley attribution.
- `self_organizing.rs` — morphogenetic fields, reaction-diffusion.
- `sublinear_attention.rs` — LSH bucketing + PPR sampling + spectral sparsification → O(n log n).
- `proof_gated.rs` — `ProofGate<T>` wrapping mutations behind formal proofs.
- `verified_training.rs` — per-step invariant proofs, training certificates.

Each is `#[cfg(feature = "<name>")]`-gated. The temporal-causal layer is especially interesting for DynamicJEPA — Granger-causality extraction + continuous-time ODE on graphs is exactly the kind of dynamics primitive ME-JEPA's Stage E predictor wants.

### 8. `ruvllm` — local LLM runtime

`crates/ruvllm/src/`:
- `lib.rs`, `error.rs`, `capabilities.rs`, `autodetect.rs`.
- **backends/** — pluggable backend layer.
- **bitnet/** — 1-bit BitNet variants.
- **claude_flow/** — claude-flow integration.
- **context/** — context management (sliding window, etc.).
- **evaluation/** — quality evaluation.
- **gguf/** — GGUF format loader.
- **hub/** — HuggingFace Hub integration.
- **intelligence/** — top-level intelligence ops.
- **kernels/** — compute kernels (Metal, CUDA, WebGPU branches).
- **kv_cache.rs**, **paged_attention.rs** — KV cache management.
- **lora/** — LoRA loading.
- **metal/** — Apple Metal kernels.
- **models/** — model architecture definitions.
- **moe/** — Mixture-of-Experts.
- **optimization/** — graph-level optimizations.
- **policy_store.rs** — policy registry.
- **qat/** — Quantization-Aware Training.
- **quality/** — quality scoring.
- **quantize/** — quantization (TurboQuant 2-4 bit, `turboquant_profile.rs`).
- **reasoning_bank/** — local reasoning bank inside ruvllm.
- **reflection/** — self-reflection.
- **ruvector_integration.rs** — integration with ruvector-core.
- **serving/** — serving layer.

This is the Rust-native model runtime. Has Metal + WebGPU + CUDA branches; doesn't yet have sm_120-specific kernels (uses candle generically).

### 9. The `mcp-*` family

`crates/mcp-gate/`, `crates/mcp-brain/`, `crates/mcp-brain-server/` — Model Context Protocol integration.

The "pi-brain" referenced in the embedded `CLAUDE.md` (read at session start) lives here — Google Cloud Run deployed at `pi.ruv.io` with 1,500+ memories, 350K+ graph edges, witness chain audit, differential privacy ε=1.0. **You don't need to use this hosted service**, but the crate code is the open-source reference for "MCP-attached collective brain."

### 10. The latest commit (2026-05-08): `ruvllm_retrieval_diffusion`

PR #450, merged 2026-05-08 14:59:56 -0400. New crate `crates/ruvllm_retrieval_diffusion`:
- **Training-free retrieval LM** — autoregressive next-token retrieval via `KvCache` + `decode_step`, O(log T) per token.
- **Masked discrete diffusion** — bidirectional diffusion with MaskGIT cosine schedule. Beats AR by 6.9× on the Mario benchmark.
- Built on top of `ruvllm_sparse_attention` (a separately-versioned no_std/ESP32-S3-capable crate that landed two days earlier).

Domain plug-in: any small-vocab token domain (vocab ≤ 254) can use it as associative memory. **For DynamicJEPA's planner head**, this is a straight drop-in for "given a sequence of latent tokens, generate next-token candidates via training-free retrieval" — and it's the actively-developed workstream right now.

## Workspace stats

```
Crates listed in Cargo.toml members: ~100
Total directories under crates/:     135  (some excluded, some sub-workspaces)
ADR documents:                       206  (docs/adr/ADR-001 through ADR-206)
Total commits:                     2,492
README size:                       277 KB
CHANGELOG entries:                  v1.x → v2.0.5 → hailo-backend
Latest workspace version:           2.0.5  (last "release" CHANGELOG entry; iter-arc still active)
Latest npm published:               ruvector  (versioned independently per crate family)
```

## Comparing to ContextGraph

| | ContextGraph | RuVector |
|--|--------------|----------|
| Crates | 11 | 135 |
| Storage substrate | RocksDB-per-CF | sled / files / `.rvf` / Postgres |
| Vector engines | RuVector HNSW (you depend on theirs) | HNSW + DiskANN + flat + hyperbolic |
| Embedders | 13 (E1-E13, asymmetric for E5/E8/E10) | configurable, default Xenova MiniLM |
| Self-learning | LoRA-on-embedder | sona (μLoRA + EWC++ + REINFORCE + 3 loops) |
| Causal | E5 + LLM-validated edges | graph-transformer/temporal.rs (Granger) + sona reasoning bank |
| Attention | candle CUDA on sm_120 | ruvector-attention (~50 mechanisms; CPU/WASM/Metal/Hailo) |
| MCP server | context-graph-mcp | mcp-gate, mcp-brain, mcp-brain-server |
| Audit | CF_AUDIT_LOG | rvf-crypto witness chain (73-byte SHAKE-256) + lineage + attestation + ed25519 + ML-DSA-65 |
| Single-file format | — | `.rvf` cognitive container |
| LLM runtime | external (qwen3.6:27b via GGUF for inner actor) | ruvllm (Metal/CUDA/WebGPU built-in) |
| Domains | SWE-bench harness + DynamicJEPA design | recruiting via cognitum-one, dna, scipix, neural-trader, agentic-robotics, etc. |

ContextGraph is **purpose-built** for one workstream (the reality loop + DynamicJEPA). RuVector is **infrastructure** that sits underneath many workstreams. They're complementary; the pieces of RuVector that fit ContextGraph's purpose are exactly what these findings docs surface.

## Read next

- `02_CAPABILITIES_INVENTORY.md` for per-crate capability descriptions.
- `03_REALITY_LOOP_APPLICATIONS.md` for ccreality transplants.
- `04_JEPA_APPLICATIONS.md` for ME-JEPA / 5090jepa transplants.
