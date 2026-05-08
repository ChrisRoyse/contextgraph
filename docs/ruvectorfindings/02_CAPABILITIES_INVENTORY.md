# 02 — Capabilities Inventory

Per-crate catalog of capabilities relevant to ccreality and JEPA. Roughly ordered by load-bearing → specialty. File refs cite `crates/<name>/src/<path>` paths inside the cloned repo at `ruvector/`.

---

## A. `ruvector-core` — vector storage + retrieval (the trunk)

**Path:** `crates/ruvector-core/`

### A.1 Indexes
- **HNSW** — `src/index/hnsw.rs`. Standard hierarchical navigable small-world.
- **Flat** — `src/index/flat.rs`. Brute-force baseline, useful for tests + small corpora.
- **DiskANN/Vamana** — `src/advanced_features/diskann.rs`. SSD-backed billion-scale ANN; alpha-RNG pruning, LRU page cache, filtered traversal during search (not post-filter), <10ms latency on 1B vectors.

### A.2 Quantization
- **OPQ (Optimized Product Quantization)** — `src/advanced_features/opq.rs`. Learned rotation before PQ; 10-30% error reduction vs vanilla PQ.
- **Product Quantization** — `src/advanced_features/product_quantization.rs`. Standard PQ.
- **Sparse vectors** — `src/advanced_features/sparse_vector.rs`. Sparse representation for hybrid search.
- **In-core quantization.rs** — `src/quantization.rs`. Scalar quant primitives.

### A.3 Search variants
- **Hybrid (BM25 + vector + RRF)** — `src/advanced_features/hybrid_search.rs`. Default `vector_weight: 0.7, keyword_weight: 0.3`. Three normalization strategies (MinMax, ZScore, None). Real BM25 implementation with `idf`, `avg_doc_len`, `k1`, `b`.
- **Graph RAG (Leiden)** — `src/advanced_features/graph_rag.rs`. Entity extraction → KnowledgeGraph → community detection (Leiden-inspired) → multi-level summaries → local + global + hybrid search. 30-60% improvement on multi-hop queries.
- **ColBERT multi-vector** — `src/advanced_features/multi_vector.rs`. Per-token late interaction with MaxSim, AvgSim, SumMax scoring.
- **Matryoshka embeddings** — `src/advanced_features/matryoshka.rs`. Adaptive-dimension search; coarse-to-fine funnel + cascade modes.
- **Filtered search** — `src/advanced_features/filtered_search.rs`. Predicate evaluation during graph traversal, not post-filter.
- **MMR diversity rerank** — `src/advanced_features/mmr.rs`. Maximal Marginal Relevance.
- **Conformal prediction** — `src/advanced_features/conformal_prediction.rs`. Calibrated uncertainty quantification with `ConformalConfig.alpha` (0.1 = 90% coverage default), three nonconformity measures (Distance, InverseRank, NormalizedDistance), prediction sets with statistical coverage guarantees.

### A.4 Operational
- **LSM compaction** — `src/advanced_features/compaction.rs`. Log-Structured Merge-tree for write-heavy vector workloads with bloom filters.
- **Lock-free** — `src/lockfree.rs`. Concurrent operation primitives.
- **Cache-optimized** — `src/cache_optimized.rs`. Memory layouts tuned for L1/L2 hits.
- **SIMD** — `src/simd_intrinsics.rs`. AVX-512 / NEON intrinsics.

### A.5 Advanced
- **Hypergraph** — `src/advanced/hypergraph.rs`. n-ary relations.
- **Learned index** — `src/advanced/learned_index.rs`. ML-driven index structure.
- **Neural hash** — `src/advanced/neural_hash.rs`. Learned LSH.
- **TDA** — `src/advanced/tda.rs`. Topological data analysis (persistent homology, Betti numbers, drift detection).

**Verdict:** core engine; you'd path-dep this for any vector retrieval need that goes beyond pure cosine k-NN. Big win: conformal prediction, Graph RAG, ColBERT, Matryoshka are uncommon together in a single Rust crate.

---

## B. `sona` — Self-Optimizing Neural Architecture

**Path:** `crates/sona/`

### B.1 Engine
- **`SonaEngine`** — `src/engine.rs`. Top-level entry point. Core API: `begin_trajectory(query) -> Builder`, `Builder::add_step(activation, context_ids, reward)`, `engine.end_trajectory(builder, final_quality)`, `apply_micro_lora(input, output)`.
- **3 learning loops** — `src/loops/{instant.rs, background.rs, coordinator.rs}`:
  - Instant: sub-ms learning per trajectory close.
  - Background: offline consolidation pulled from a buffer.
  - Coordinator: orchestrates instant + background, decides when to promote.

### B.2 LoRA stack
- **MicroLoRA** — `src/lora.rs`. Ultra-low rank (1-2). Sub-ms apply.
- **BaseLoRA** — `src/lora.rs`. Standard rank for background.
- **`LoRAEngine`** — `src/lora.rs`. Compose micro + base.

### B.3 EWC++
- **`EwcPlusPlus`** — `src/ewc.rs`. Online Fisher info estimation (EMA), multi-task circular buffer, automatic task-boundary detection (via gradient mean+variance + threshold), adaptive lambda scheduling. Defaults: `initial_lambda=2000`, `max_lambda=15000`, `fisher_ema_decay=0.999`, `boundary_threshold=2.0`.

### B.4 Trajectory
- **`TrajectoryBuffer` / `TrajectoryBuilder`** — `src/trajectory.rs`. Bounded-capacity ring buffer.
- **REINFORCE gradient estimation** — `src/types.rs:69-101`. `gradient = sum((reward - baseline) * activation)`, L2-normalized.
- **`LearningSignal::scaled_gradient()`** — `src/types.rs:104-109`. Scales by quality.

### B.5 ReasoningBank
- **`ReasoningBank` + `PatternConfig`** — `src/reasoning_bank.rs`. Pattern extraction + clustering + similarity search. The semantic memory layer above raw trajectories.

### B.6 Federated training
- **`FederatedCoordinator`** — `src/training/federated.rs`. Multi-agent coordination.
- **`EphemeralAgent`** — lightweight nodes (~5 MB footprint).
- **Quality-weighted aggregation** — only high-quality agents contribute.
- **Topologies** — `FederatedTopology` enum.
- **`TrainingPipeline`** — `src/training/pipeline.rs`. End-to-end batch training.
- **`AgentFactory`, `AgentExport`** — `src/training/factory.rs`. Lifecycle.

### B.7 Export
(feature `serde-support`)
- **HuggingFace Hub** — `src/export/huggingface_hub.rs`.
- **Safetensors** — `src/export/safetensors.rs`. Rust-native safetensors writer.
- **Pretrain pipeline** — `src/export/pretrain.rs`.
- **Dataset exporter** — `src/export/dataset.rs`.

### B.8 Deployment forms
- **Native** — Rust crate.
- **WASM** — `src/wasm.rs` (718 LOC) with `WasmSonaEngine`.
- **N-API** — `src/napi_simple.rs` for Node.js.

**Verdict:** the gold-standard transplant. If you want Rust-native self-learning that ships, this is it. Wraps cleanly around any embedding source. Used by `agentdb` via its npm wrapper; here you can consume the source.

---

## C. `rvf-crypto` — cryptographic substrate

**Path:** `crates/rvf/rvf-crypto/`

### C.1 Hash
- **`shake256_128`, `shake256_256`, `shake256_hash`** — `src/hash.rs`. SHAKE-256 throughout the system.

### C.2 Signatures
- **Ed25519** — `src/sign.rs` (feature `ed25519`). Per-segment signing.
- **Footer codec** — `src/footer.rs`. `encode_signature_footer` / `decode_signature_footer` for trailing signature placement.
- **ML-DSA-65 + SLH-DSA-128s** — separate (referenced in workspace deps); post-quantum signature schemes.

### C.3 Witness chain (the killer feature)
- **`WitnessEntry`** — `src/witness.rs:13`. `{ prev_hash: [u8;32], action_hash: [u8;32], timestamp_ns: u64, witness_type: u8 }`. Total `ENTRY_SIZE: 73` (line 25).
- **`create_witness_chain(entries)`** — `src/witness.rs:66-79`. First entry's `prev_hash` is genesis (zeros); subsequent `prev_hash = SHAKE-256(prev encoded entry)`.
- **Verification** — recompute the chain, compare.

### C.4 Lineage
- **`compute_manifest_hash`** — `src/lineage.rs`. Manifest content hash.
- **`lineage_record_{from,to}_bytes`** — Lineage records with parent/child cryptographic chains.
- **`lineage_witness_entry`** — Lineage entries chained via witness pattern.
- **`verify_lineage_chain`** — Independent verification.

### C.5 Attestation
- **`QuoteVerifier`** — `src/attestation.rs`. TEE quote verification.
- **`TeeBoundKeyRecord`** — Key-binding records.
- **`build_attestation_witness_payload`, `verify_attestation_witness_payload`** — TEE attestation in the witness chain.

**Verdict:** the highest-leverage drop-in transplant in the entire RuVector tree. ~700 LOC, no_std (`#![cfg_attr(not(feature = "std"), no_std)]`), pulls only `rvf-types` for error types. Vendor or path-dep this **today**. Two consumers: ccreality optimizer audit, DynamicJEPA training-run audit.

---

## D. `ruvector-solver` — sublinear algorithms

**Path:** `crates/ruvector-solver/`

### D.1 Algorithms
- **BMSSP multigrid** — `src/bmssp.rs`. Algebraic multigrid V-cycle, O(nnz·log n) per solve. For Laplacian + SPD systems.
- **Conjugate Gradient** — `src/cg.rs`.
- **Neumann series** — `src/neumann.rs`.
- **Forward Push** — `src/forward_push.rs`. Personalized PageRank.
- **Backward Push** — `src/backward_push.rs`. Bidirectional PPR.
- **TRUE solver** — `src/true_solver.rs`. Truncated Random Updates.
- **Random Walk** — `src/random_walk.rs`. Sampling-based.

### D.2 Infrastructure
- **`SolverEngine` trait** — `src/traits.rs`. Pluggable algorithm selection.
- **`Solver` trait** — for in-place use.
- **`CsrMatrix<T>`** — `src/types.rs`. CSR sparse matrix.
- **`ComputeBudget`** — `src/types.rs`. Tracks `max_iterations`, `tolerance`, `max_time`.
- **`SolveResult`** — return type with `residual_norm`, `iterations`, `converged`.
- **Router** — `src/router.rs`. Picks algorithm based on matrix properties.
- **Audit** — `src/audit.rs`. Operation log.
- **Validation** — `src/validation.rs`. Result verification.
- **SIMD** — `src/simd.rs`.

**Verdict:** stop reinventing PageRank or Laplacian solve. Adopt this. ContextGraph already references the matrix-optimizer agent role; this is the canonical implementation.

---

## E. `ruvector-mincut` — graph cuts

**Path:** `crates/ruvector-mincut/`

28 sub-modules. Core deliverables:

### E.1 Algorithms
- Canonical mincut, Karger's, Stoer-Wagner.
- Parallel mincut.
- Local k-cut.
- Subpolynomial-time variants.
- Link-cut trees for dynamic graphs (`src/linkcut/`).
- Expander-graph properties (`src/expander/`).
- Spectral sparsification (`src/sparsify/`).
- Eulerian decomposition (`src/euler/`).
- Junction trees (`src/jtree/`).

### E.2 Operational
- **Witness certificates** — `src/witness/`, `src/certificate/`. Cuts come with proof.
- **Fragment-aware partitioning** — `src/fragment/`, `src/fragmentation/`.
- **Pool** — `src/pool/`. Reusable buffer allocation.
- **SNN integration** — `src/snn/`. Spiking-neural-net mincut.

### E.3 Bindings
- WASM, NAPI, brain-node, gated-transformer integration.

**Verdict:** the right primitive for "split this graph respecting these constraints" — ME-JEPA train/test split, DynamicJEPA trajectory leakage check, ccreality task-clustering by failure-mode similarity.

---

## F. `ruvector-attention` — attention library

**Path:** `crates/ruvector-attention/`

### F.1 Standard attention
- **`flash.rs`** — FlashAttention implementation.
- **`mla.rs`** — Multi-Latent Attention (DeepSeek-style KV-cache compression).
- **`multi_head.rs`** — Standard MHA.
- **`scaled_dot_product.rs`** — Building block.
- **`speculative.rs`** — Speculative decoding.
- **`ssm.rs`** — Mamba State Space Model attention.
- **`kv_cache.rs`** — KV cache management.

### F.2 Sparse
- **`sparse/flash.rs`** — Sparse FlashAttention.
- **`sparse/linear.rs`** — Linear-time attention.
- **`sparse/local_global.rs`** — Local + global attention split.
- **`sparse/mask.rs`** — Mask compilation.

### F.3 Hyperbolic
- **`hyperbolic/poincare.rs`** — Poincaré ball.
- **`hyperbolic/lorentz_cascade.rs`** — Lorentz model cascade.
- **`hyperbolic/mixed_curvature.rs`** — Mixed-curvature spaces.
- **`hyperbolic/hyperbolic_attention.rs`** — Top-level wrapper.

### F.4 Specialty
- **`moe/`** — Mixture-of-Experts attention.
- **`info_bottleneck/`** — Information-bottleneck regularization.
- **`info_geometry/`** — Information-geometry-aware.
- **`graph/`** — Graph attention.
- **`sheaf/`** — Sheaf attention (sheaf cohomology over graphs).
- **`transport/`** — Optimal-transport attention.
- **`pde_attention/`** — PDE-based dynamics.
- **`curvature/`** — Curvature-aware.
- **`topology/`** — TDA-features.
- **`training/`** — Training utilities.

### F.5 Bindings
- WASM (`ruvector-attention-wasm`), NAPI (`ruvector-attention-node`), CLI, unified-WASM bundle.

**Verdict:** read for ideas; you have your own candle CUDA path on RTX 5090. Specifically interesting modules: **MLA** for KV-cache compression on inner-actor model serving; **sheaf attention** for graph-structured trajectories in DynamicJEPA.

---

## G. `ruvector-graph-transformer` — 8 verified modules

**Path:** `crates/ruvector-graph-transformer/`

All feature-flagged.

### G.1 `physics` — Hamiltonian dynamics, gauge equivariance
Hamiltonian flow attention, gauge-equivariant message passing. Energy conservation by construction.

### G.2 `biological` — bio-inspired
Spiking attention, Hebbian/STDP learning, dendritic branching.

### G.3 `manifold` — curved spaces
Product manifolds S^n × H^m × R^k. Riemannian message passing.

### G.4 `temporal` — `src/temporal.rs`
- Causal masking strategies: `Strict` (t' < t), `TimeWindow { window_size }`.
- Continuous-time neural ODE integration.
- Granger-causality extraction.
- Delta-chain temporal embedding storage.
- Retrocausal-safety enforcement.
- Proof-gated mutations via `ruvector_verified`.

### G.5 `economic` — multi-agent
Nash equilibrium attention, Shapley attribution. Fair value assignment in multi-agent graphs.

### G.6 `self_organizing` — morphogenesis
Morphogenetic field attention, reaction-diffusion graph growth. Graphs that restructure themselves.

### G.7 `sublinear_attention`
LSH bucketing + PPR sampling + spectral sparsification. O(n log n) attention via subset selection.

### G.8 `proof_gated` — `src/proof_gated.rs`
- `ProofGate<T>` wrapping mutations behind formal proofs.
- `MutationLedger` — append-only with FNV-1a chain hash.
- `ProofScope` — partition-aligned scope tied to mincut boundaries.
- `SupersessionProof` — forward-only rollback via supersession.
- `EpochBoundary` — seal attestation for proof-algebra upgrades.
- `ProofRequirement`, `ComplexityBound`, `ProofClass` (Formal vs Statistical).

### G.9 `verified_training` — `src/verified_training.rs`
Per-step invariant proofs producing `TrainingCertificate`s. Delta-apply: gradients go to scratch buffer, invariants checked on proposed state, delta committed only if all invariants pass. Fail-closed by default.

Invariant tiers:
| Invariant | Tier | Formally proven? |
|-----------|------|------------------|
| LossStabilityBound | Reflex | Yes |
| WeightNormBound | Standard | Yes |
| LipschitzBound | Standard | No (statistical) |
| PermutationEquivariance | Deep | No (statistical) |
| EnergyGate | Standard | Yes |

**Verdict for JEPA:** `temporal` and `verified_training` are the two most relevant. Granger-causality extraction in `temporal` is exactly the kind of dynamics primitive ME-JEPA's trajectory predictor needs. `verified_training` is the right scaffolding for "did this gradient step actually preserve invariants?" — which is what `backupdocs/5090jepa/00_TASK.md` rule R4 demands.

---

## H. `ruvector-temporal-tensor` — tier-quantized cold storage

**Path:** `crates/ruvector-temporal-tensor/`

### H.1 Compressor
- **`TemporalTensorCompressor`** — `src/lib.rs`. Frame-by-frame push, segment emission at boundaries.
- **TierPolicy** — access-pattern-driven tier selection.

### H.2 Tier table
| Tier | Bits | Ratio vs f32 | Use case |
|------|------|-------------|----------|
| Hot | 8 | ~4.0× | Frequently accessed |
| Warm | 7 | ~4.57× | Moderately accessed |
| Warm | 5 | ~6.4× | Aggressively compressed warm |
| Cold | 3 | ~10.67× | Rarely accessed |

This is **finer-grained** than agentdb's 5-tier model (which used `none/half/pq8/pq4/binary`). Ruvector goes with bit-width tiers (8/7/5/3 bit groupwise symmetric quant).

### H.3 Sub-modules
- `compressor.rs`, `quantizer.rs`, `bitpack.rs`, `delta.rs`, `f16.rs`, `segment.rs`, `store.rs`, `tier_policy.rs`, `tiering.rs`, `persistence.rs`, `metrics.rs`, `coherence.rs`, `core_trait.rs`, `agentdb.rs` (yes, an agentdb-specific integration), `ffi.rs`, `store_ffi.rs`.

### H.4 Zero deps
"This crate has no external dependencies, making it fully WASM-compatible." (per `src/lib.rs:23`)

**Verdict:** drop-in replacement for any cold-tensor storage need. Strictly better than ContextGraph's binary "quantized? yes/no" PQ-8 path. JEPA latent storage is the killer use case.

---

## I. `ruvector-graph` — Cypher engine

**Path:** `crates/ruvector-graph/`

### I.1 Modules
- **`cypher/`** — Full Cypher query parser + executor.
- **`executor/`** — Query execution engine.
- **`graph.rs`, `node.rs`, `edge.rs`, `hyperedge.rs`** — Core graph types. Hyperedges connect 3+ nodes natively.
- **`storage.rs`** — Persistent graph store.
- **`transaction.rs`** — ACID transactions.
- **`distributed/`** — Distributed graph ops.
- **`hybrid/`** — Hybrid (graph + vector) queries.
- **`optimization/`** — Query plan optimization.
- **`index.rs`** — Graph indexes.

**Verdict:** ContextGraph already has its own E5 causal graph. This crate is the alternative: full Cypher, hyperedges, distributed transactions. Worth reading for the Cypher executor design; not a replacement.

---

## J. `ruvector-router-core` + `ruvector-tiny-dancer-core` — intent routing

**Path:** `crates/ruvector-router-core/`, `crates/ruvector-tiny-dancer-core/`

### J.1 Router-core
- **`vector_db.rs`** — In-memory VectorDB for intent vectors.
- **`index.rs`** — HNSW + SIMD intent index.
- **`storage.rs`** — Persistence.

### J.2 Tiny-Dancer
- **`api.rs`** — Top-level API: `route(query) -> handler`.
- **`model.rs`** — FastGRNN (Fast Gated Recurrent Neural Net) routing model.
- **`feature_engineering.rs`** — Feature extraction from query.
- **`circuit_breaker.rs`** — Failure isolation.
- **`metrics.rs`** — Routing telemetry.
- **`optimization.rs`** — Hot-path optimizations.

Sub-10ms intent routing without LLM call. Used internally to route requests to appropriate handlers (agentdb's `SemanticQueryRouter` is the higher-level wrapper).

**Verdict:** if your optimizer or planner ever has > 1 retrieval mode and you want to dispatch in sub-ms without an LLM call, this is the primitive. ccreality currently routes via 4 search strategies × weight profiles, which is more flexible but slower.

---

## K. `ruvllm` — local LLM runtime

**Path:** `crates/ruvllm/`

### K.1 Core
- **GGUF loader** — `src/gguf/`. Load any GGUF.
- **Backends** — `src/backends/`. Pluggable execution.
- **Kernels** — `src/kernels/`. Compute kernels.
- **Metal** — `src/metal/`. Apple Metal kernels.
- **WebGPU** — referenced in capabilities; cross-target.

### K.2 Optimization
- **Paged attention** — `src/paged_attention.rs`. vLLM-style paged KV cache.
- **KV cache** — `src/kv_cache.rs`.
- **TurboQuant** — `src/quantize/turboquant_profile.rs`. 2-4 bit asymmetric KV-cache quantization. 6-8× memory savings, <0.5% perplexity loss.
- **MoE** — `src/moe/`. Mixture-of-Experts execution.
- **BitNet** — `src/bitnet/`. 1-bit quantization variants.
- **QAT** — `src/qat/`. Quantization-aware training.

### K.3 Reasoning
- **`reasoning_bank/`** — Internal reasoning bank inside ruvllm (separate from `crates/sona`'s ReasoningBank).
- **`reflection/`** — Self-reflection loops.
- **`policy_store.rs`** — Policy registry.
- **`evaluation/`** — Quality evaluation.

### K.4 Integration
- **`ruvector_integration.rs`** — ruvector-core hooks.
- **`claude_flow/`** — claude-flow hooks.
- **`hub/`** — HuggingFace Hub integration.

**Verdict:** alternative to llama-cpp-2 if you ever want a Rust-native replacement for the qwen3.6:27b GGUF inner-actor runtime. **Doesn't have sm_120-specific kernels** — uses candle generically; you still own the sm_120a CUDA work for the inner actor. But for non-inner-actor LLM calls (verifier prompts, reasoning bank summaries), this is a viable substitute.

---

## L. `ruvllm_sparse_attention` — no_std attention

**Path:** `crates/ruvllm_sparse_attention/`

- `#![cfg_attr(not(feature = "std"), no_std)]` — runs on ESP32-S3.
- **`SubquadraticSparseAttention`** — `src/attention.rs`.
- **`KvCache`** — `src/attention.rs`.
- **`FastGrnnGate`** — `src/fastgrnn_gate.rs`. The new (2026-05-07) FastGRNN-gated near-linear attention from PR #429.
- **Tensor3** — small 3-axis tensor type with no_std-safe math.
- Optional `fp16` feature.

**Verdict:** if you ever go embedded — DynamicJEPA in `5090jepa` won't (RTX 5090), but ME-JEPA-edge-deployment scenarios might — this is the primitive.

---

## M. `ruvllm_retrieval_diffusion` — training-free retrieval LM (NEW 2026-05-08)

**Path:** `crates/ruvllm_retrieval_diffusion/`

PR #450, merged 2026-05-08 14:59:56 -0400. Latest substantive feature in the repo.

### M.1 Two pipelines from one kernel
- **`Retriever::generate_fast`** — autoregressive next-token retrieval via `KvCache` + `decode_step`. O(log T) per generated token.
- **`Diffuser::diffuse`** — bidirectional masked discrete diffusion with MaskGIT cosine schedule. **Beats AR by 6.9× on Mario benchmark.**

### M.2 Domain plug-in
Any small-vocab token domain (vocab ≤ 254) can plug in a corpus + `RetrievalConfig`:
```rust
let cfg = RetrievalConfig {
    vocab_size: 5,
    head_dim: 64,
    pos_scale: 0.5,
    mask_sentinel: 255,
    ..RetrievalConfig::default()
};
let retriever = Retriever::new(corpus, cfg, seed);
let output = retriever.generate_fast(&seed, 256, &SamplingConfig::quality(), 0xC0FFEE);
```

### M.3 Why this matters for DynamicJEPA
The `gridworld_5x5.v1` domain pack in `backupdocs/5090jepa/07_domain_pack_gridworld.md` has small action vocab. **The planner head can use `ruvllm_retrieval_diffusion::Diffuser` directly** — no training, just associative-memory retrieval with masked diffusion ranking. 6.9× speedup over AR.

For the full ME-JEPA recruiting domain (larger vocab), the AR retriever path is still applicable.

**Verdict:** the most exciting new transplant in the entire RuVector tree as of today (2026-05-08). Active workstream.

---

## N. `ruvector-coherence` + `prime-radiant` — signal quality / hallucination detection

**Path:** `crates/ruvector-coherence/`, `crates/prime-radiant/`

### N.1 Coherence
- **`metrics.rs`** — Coherence metrics (sheaf-Laplacian-derived).
- **`comparison.rs`** — Compare attention mechanisms objectively.
- **`batch.rs`** — Batched coherence computation.

### N.2 Prime Radiant
A coherence engine using sheaf Laplacian math for AI safety + hallucination detection. Sub-modules:
- `coherence/`, `cohomology/`, `attention/`, `mincut/`, `hyperbolic/`
- `learned_rho/` — Learned coherence threshold ρ.
- `neural_gate/` — Coherence-gated outputs.
- `signal/` — Signal-quality measurements.
- `sona_tuning/` — Auto-tune SONA against coherence loss.
- `gpu/`, `simd/`, `tiles/`, `substrate/`
- `governance/`, `security/`
- `events/`, `execution/`, `storage/`

### N.3 Integration with ruvllm
`ruvllm_integration/` — coherence-gated LLM responses. If a generation's coherence drops below ρ, the gate refuses.

**Verdict:** worth knowing exists. For any output you want to validate as "coherent" (the predictor's projection didn't drift, the read-out head didn't hallucinate), this is the right primitive. ME-JEPA's anti-pole logic could plug in here.

---

## O. `ruvector-domain-expansion` — cross-domain transfer

**Path:** `crates/ruvector-domain-expansion/`

Transfer learning. New tasks bootstrap from past learning automatically — projection across instrument spaces, similarity threshold, max-transfers cap.

**Verdict:** matches DynamicJEPA's "cross-domain-pack skill transfer" requirement (per `backupdocs/dynamicjepa/02_core_thesis.md`). Worth reading for the projection mechanics; ME-JEPA may want it directly.

---

## P. `ruvector-verified` — proof environments

**Path:** `crates/ruvector-verified/`

Underlies `proof_gated` + `verified_training` in graph-transformer. Provides:
- `ProofEnvironment` — environment for reasoning about proofs.
- `prove_dim_eq` — dimensional equality proofs.
- `ProofAttestation` — attestation records.
- `gated::route_proof` — proof tier routing.
- `pipeline::compose_chain` — chain proofs.
- `ProofTier`, `ProofKind`, `TierDecision`, `VerifiedOp`.
- `proof_store::create_attestation` — store attestations.

**Verdict:** if DynamicJEPA's verified-training story matters, vendor this with `ruvector-graph-transformer/temporal.rs`. ContextGraph's existing FSV is conceptually similar but proof-free; this adds formal scaffolding.

---

## Q. The "exotic" specialty crates

Listed for completeness — unlikely transplants for ccreality/JEPA but interesting:

- **`ruvector-consciousness`** — IIT4 Φ computation, mincut Φ, witness Φ, geomip, emergence, collapse. Research-grade.
- **`ruvector-nervous-system`** — 5-layer bio-inspired adaptive system with spiking nets and BTSP learning.
- **`ruQu`** — Quantum coherence error correction via dynamic mincut. Feature-gated.
- **`agentic-robotics-*`** — Robotics primitives.
- **`neural-trader-*`** — Financial trading.
- **`thermorust`** — Thermal modeling.
- **`ruvix`** — Unspecified; presumably an experimentation harness.

If any of these later become relevant to a side workstream, vendor on demand.

---

## R. The `rvf` sub-workspace

**Path:** `crates/rvf/`

NOT in the main workspace `members` (excluded). Sub-crates:

| Crate | Purpose |
|-------|---------|
| `rvf-types` | Error types, base types |
| `rvf-wire` | Wire-format encoding |
| `rvf-manifest` | RVF segment manifest |
| `rvf-runtime` | Runtime execution |
| `rvf-quant` | Quantization in RVF context |
| `rvf-launch` | Launch RVF as a service |
| **`rvf-crypto`** | **The witness chain + signatures (see §C)** |
| `rvf-ebpf` | eBPF integration |
| `rvf-federation` | Federated RVF sync |
| `rvf-import` | Import from other formats |
| `rvf-cli` | CLI |
| `rvf-server` | RVF server |
| `rvf-node` | Node.js bindings |
| `rvf-wasm` | WASM bindings |
| `rvf-solver-wasm` | Solver as WASM |
| `rvf-adapters` | Adapters to other systems |
| `rvf-cow` | COW branching (referenced separately) |
| `rvf-index` | RVF index |
| `rvf-launch` | Launch RVF as a service |

**`rvf-crypto` is the hero here.** The rest of `rvf` is the single-file format machinery; you don't need it unless you're shipping `.rvf` files.

---

## Summary table — what to take, what to read, what to skip

| Capability | Verdict | Effort to adopt |
|------------|---------|-----------------|
| `rvf-crypto` (witness chain + signatures) | **Vendor today** | <1 day |
| `ruvector-temporal-tensor` | **Path-dep** | 1 day (config) |
| `ruvector-solver` | **Path-dep** | 1 day (API surface) |
| `ruvector-mincut` | **Path-dep on use** | 1 day per use |
| `sona` | **Vendor or path-dep** | 2-3 days (integration) |
| `ruvector-core` advanced_features (graph_rag, conformal_prediction, mmr, matryoshka, opq) | **Path-dep selectively** | 1 day each |
| `ruvllm_retrieval_diffusion` | **Path-dep for JEPA planner** | 1-2 days |
| `ruvector-graph-transformer/temporal.rs` (Granger causality) | **Path-dep for JEPA** | 2 days |
| `ruvector-graph-transformer/verified_training.rs` | **Read; selectively port** | 3 days |
| `ruvector-domain-expansion` | **Path-dep when JEPA cross-pack ships** | 2 days |
| `ruvector-coherence` + `prime-radiant` | **Read; selectively use** | varies |
| `ruvector-attention` (MLA, sheaf) | **Read for ideas** | n/a |
| `ruvllm` | **Defer; you have qwen3.6:27b via llama-cpp-2** | n/a |
| `ruvector-graph` (Cypher) | **Read; you have E5** | n/a |
| `ruvector-consciousness`, `ruQu`, `nervous-system` | **Skip unless side-quest** | n/a |
| `ruvector-postgres` | **Skip** | n/a |
| `agentic-robotics`, `neural-trader`, `thermorust` | **Skip** | n/a |

The verdict-shift from agentdb is dramatic: most "port to Rust" items in the agentdb findings become **`Cargo.toml` line** items here. The only things you still port are pieces too tightly coupled to the rest of RuVector to extract cleanly.

Continue to `03_REALITY_LOOP_APPLICATIONS.md` for the named transplants.
