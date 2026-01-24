# Context Graph PRD v6.3 (GPU-First 13-Perspectives Multi-Space System)

**Platform**: Claude Code CLI | **Architecture**: GPU-First | **Hardware**: RTX 5090 32GB + CUDA 13.1

**Core Insight**: 13 embedders = 13 unique perspectives on every memory, all warm-loaded on GPU

**New in v6.3**: Embedder-first search - AI agents can now search using any of the 13 embedders as the primary perspective, not just E1

---

## 0. GPU-FIRST ARCHITECTURE

This is a **GPU-first system**. All compute-intensive operations use GPU over CPU. No CPU fallback.

### Hardware Platform
| Component | Specification |
|-----------|---------------|
| GPU | NVIDIA GeForce RTX 5090 (Blackwell GB202) |
| VRAM | 32GB GDDR7 @ 1,792 GB/s |
| Tensor Cores | 680 (5th gen, FP16/BF16/FP8/FP4) |
| CUDA Cores | 21,760 across 170 SMs |
| Compute Cap | 12.0 (CUDA 13.1) |
| CPU | AMD Ryzen 9 9950X3D (16C/32T) |
| RAM | 128GB DDR5 |

### GPU Utilization
| Workload | GPU Component | Benefit |
|----------|---------------|---------|
| Embedding Inference | Tensor Cores (FP16/BF16) | 3-5x vs CPU |
| Vector Search | faiss-gpu HNSW | Sub-millisecond ANN |
| Topic Clustering | cuML HDBSCAN | 10-50x vs sklearn |
| Batch Operations | CUDA Streams | Async overlap |
| QoS Isolation | Green Contexts | Deterministic latency |

### VRAM Budget (32GB)
| Allocation | Size | Purpose |
|------------|------|---------|
| 13 Embedders | ~10GB | Warm-loaded, FP16 weights |
| FAISS Indexes | ~8GB | Per-space HNSW |
| Batch Buffers | ~4GB | Inference batches |
| cuML Workspace | ~2GB | Clustering, analytics |
| Reserved | ~8GB | Spike headroom |

---

## 1. CORE PHILOSOPHY

Each embedder finds what OTHERS MISS. Combined = superior answers.

**Example Query**: "What databases work with Rust?"
| Embedder | Finds | Why Others Missed It |
|----------|-------|---------------------|
| E1 | Memories containing "database" or "Rust" | Semantic match only |
| E11 | Memories about "Diesel" | Knows Diesel IS a database ORM |
| E7 | Code using `sqlx`, `diesel` crates | Recognizes code patterns |
| E5 | "Migration that broke production" | Understands causal chain |
| **Combined** | All of the above | Better answer than any single embedder |

**Key Principle**: Temporal proximity ≠ semantic relationship. Working on 3 unrelated tasks in the same hour creates temporal clusters, NOT topics.

---

## 2. THE 13 EMBEDDERS

### 2.1 What Each Finds (That Others Miss)

| ID | Name | Finds | E1 Blind Spot Covered | Category | Topic Weight |
|----|------|-------|----------------------|----------|--------------|
| **E1** | V_meaning | Semantic similarity | Foundation - has blind spots | Semantic | 1.0 |
| **E5** | V_causality | Causal chains ("why X caused Y") | Direction lost in averaging | Semantic | 1.0 |
| **E6** | V_selectivity | Exact keyword matches | Diluted by dense averaging | Semantic | 1.0 |
| **E7** | V_correctness | Code patterns, function signatures | Treats code as natural language | Semantic | 1.0 |
| **E10** | V_multimodality | Same-goal work (different words) | Misses intent alignment | Semantic | 1.0 |
| **E12** | V_precision | Exact phrase matches | Token-level precision lost | Semantic | 1.0 |
| **E13** | V_keyword | Term expansions (fast→quick) | Sparse term overlap missed | Semantic | 1.0 |
| **E8** | V_connectivity | Graph structure ("X imports Y") | Relationship structure | Relational | 0.5 |
| **E11** | V_factuality | Entity knowledge ("Diesel=ORM") | Named entity relationships | Relational | 0.5 |
| **E9** | V_robustness | Noise-robust structure | Structural patterns | Structural | 0.5 |
| **E2** | V_freshness | Recency | *POST-RETRIEVAL ONLY* | Temporal | 0.0 |
| **E3** | V_periodicity | Time-of-day patterns | *POST-RETRIEVAL ONLY* | Temporal | 0.0 |
| **E4** | V_ordering | Sequence (before/after) | *POST-RETRIEVAL ONLY* | Temporal | 0.0 |

### 2.2 Technical Specs (All GPU-Accelerated)

| ID | Dim | Distance | GPU Notes |
|----|-----|----------|-----------|
| E1 | 1024 | Cosine | Matryoshka, FP16 Tensor Core inference |
| E2-E4 | 512 | Cosine | Never in similarity fusion, GPU warm |
| E5 | 768 | Asymmetric KNN | Direction matters, faiss-gpu IVF |
| E6 | ~30K sparse | Jaccard | cuSPARSE operations |
| E7 | 1536 | Cosine | AST-aware, largest model (~3GB) |
| E8 | 384 | TransE | GPU TransE distance ||h + r - t|| |
| E11 | 768 | TransE | KEPLER GPU, RoBERTa-base + TransE |
| E9 | 1024 | Hamming | GPU bitwise ops (10K→1024) |
| E10 | 768 | Cosine | Multiplicative boost, same GPU batch |
| E12 | 128D/token | MaxSim | GPU reranking ONLY |
| E13 | ~30K sparse | Jaccard | GPU Stage 1 recall ONLY |

**All 13 embedders are warm-loaded into GPU VRAM at MCP server startup. No cold-loading, no CPU fallback.**

---

## 3. RETRIEVAL PIPELINE (GPU-Accelerated)

### 3.1 How Perspectives Combine

```
Query → E13 GPU sparse (10K) → E1 GPU dense (1K) → GPU RRF (100) → cuML filter (50) → E12 GPU rerank (10)
                ↓                       ↓                    ↓
        cuSPARSE Jaccard      faiss-gpu HNSW        Tensor Core inference
```

**All pipeline stages execute on GPU. No CPU roundtrips for core retrieval path.**

**Strategy Selection**:
| Strategy | When to Use | Pipeline |
|----------|-------------|----------|
| E1Only | Simple semantic queries | E1 only |
| MultiSpace | E1 blind spots matter | E1 + enhancers via RRF |
| Pipeline | Maximum precision | E13 → E1 → E12 |
| EmbedderFirst | Explore specific perspective | Any embedder as primary |

**Enhancer Routing**:
- E5: Causal queries ("why", "what caused")
- E7: Code queries (implementations, functions)
- E10: Intent queries (same goal, similar purpose)
- E11: Entity queries (specific named things)
- E6/E13: Keyword queries (exact terms, jargon)

### 3.2 Similarity Thresholds

| Space | High (inject) | Low (divergence) |
|-------|---------------|------------------|
| E1 | > 0.75 | < 0.30 |
| E5 | > 0.70 | < 0.25 |
| E6, E13 | > 0.60 | < 0.20 |
| E7 | > 0.80 | < 0.35 |
| E8, E11 | > 0.65 | N/A |
| E9 | > 0.70 | N/A |
| E10, E12 | > 0.70 | < 0.30 |
| E2-E4 | N/A | N/A (excluded) |

---

## 4. TOPIC SYSTEM

### 4.1 Topic Formation

Topics emerge when memories cluster in **semantic** spaces (NOT temporal).

```
weighted_agreement = Σ(topic_weight × is_clustered)

is_topic = weighted_agreement >= 2.5
max_possible = 7×1.0 + 2×0.5 + 1×0.5 = 8.5
confidence = weighted_agreement / 8.5
```

**Examples**:
- 3 semantic spaces agree = 3.0 → TOPIC
- 2 semantic + 1 relational = 2.5 → TOPIC
- 5 temporal spaces = 0.0 → NOT TOPIC (excluded)

### 4.2 Topic Stability

```
TopicMetrics { age, membership_stability, centroid_stability, phase }
phase: Emerging | Stable | Declining | Merging
churn_rate: 0.0=stable, 1.0=completely new topics
```

**Consolidation Trigger**: entropy > 0.7 AND churn > 0.5

---

## 5. MEMORY SYSTEM

### 5.1 Schema

```
Memory {
  id: UUID,
  content: String,
  source: HookDescription | ClaudeResponse | MDFileChunk,
  teleological_array: [E1..E13],  // All 13 or nothing
  session_id, created_at,
  chunk_metadata: Option<{file_path, chunk_index, total_chunks}>
}
```

### 5.2 Sources & Capture

| Source | Trigger | Content |
|--------|---------|---------|
| HookDescription | Every tool use | Claude's description of action |
| ClaudeResponse | SessionEnd, Stop | Session summaries, significant responses |
| MDFileChunk | File watcher | 200 words, 50 overlap, sentence boundaries |

### 5.3 Importance Scoring

```
Importance = BM25_saturated(log(1+access_count)) × e^(-λ × days)
λ = ln(2)/45 (45-day half-life), k1=1.2
```

---

## 6. INJECTION STRATEGY

### 6.1 Priority Order

| Priority | Type | Condition | Tokens |
|----------|------|-----------|--------|
| 1 | Divergence Alerts | Low similarity in SEMANTIC spaces | ~200 |
| 2 | Topic Matches | weighted_agreement >= 2.5 | ~400 |
| 3 | Related Memories | weighted_agreement in [1.0, 2.5) | ~300 |
| 4 | Recent Context | Last session summary | ~200 |
| 5 | Temporal Badges | Same-session metadata | ~50 |

### 6.2 Relevance Score

```
score = Σ(category_weight × embedder_weight × max(0, similarity - threshold))

Category weights: SEMANTIC=1.0, RELATIONAL=0.5, STRUCTURAL=0.5, TEMPORAL=0.0
Recency factor: <1h=1.3x, <1d=1.2x, <7d=1.1x, <30d=1.0x, >90d=0.8x
```

---

## 7. HOOK INTEGRATION (GPU-Accelerated)

Native Claude Code hooks via `.claude/settings.json`:

| Hook | Action | GPU Budget |
|------|--------|------------|
| SessionStart | Warm-load 13 models to GPU, load indexes | 30000ms |
| UserPromptSubmit | GPU embed → faiss-gpu search → inject | 500ms |
| PreToolUse | GPU inject brief relevant context | 100ms |
| PostToolUse | Capture + GPU embed as HookDescription | 300ms |
| Stop | Capture response summary | 500ms |
| SessionEnd | Persist, cuML HDBSCAN, consolidate | 5000ms |

**GPU enables aggressive budgets:** SessionStart is longer (warm-loading), but all runtime hooks are 3-6x faster.

---

## 8. MCP TOOLS

### 8.1 Core Operations

| Tool | Purpose | Key Params |
|------|---------|------------|
| `search_graph` | Multi-space search | query, strategy, topK |
| `search_causes` | Causal queries (E5) | query, causalDirection |
| `search_connections` | Graph queries (E8) | query, direction |
| `search_by_intent` | Intent queries (E10) | query, blendWithSemantic |
| `store_memory` | Store with embeddings | content, importance, rationale |
| `inject_context` | Retrieval + injection | query, max_tokens |

### 8.2 Topic & Maintenance

| Tool | Purpose |
|------|---------|
| `get_topic_portfolio` | View emergent topics |
| `get_topic_stability` | Churn, entropy metrics |
| `detect_topics` | Force HDBSCAN clustering |
| `get_divergence_alerts` | Check semantic divergence |
| `trigger_consolidation` | Merge similar memories |
| `trigger_dream` | NREM replay + REM exploration |
| `merge_concepts` | Manual memory merge |
| `forget_concept` | Soft delete (30-day recovery) |

### 8.3 Embedder-First Search (NEW)

**Core Insight**: Each of the 13 embedders sees the knowledge graph from a unique perspective. By default, E1 (semantic) is the foundation, but sometimes another perspective reveals what E1 misses.

**Example**: Query "What framework does Tokio relate to?"
| Embedder | Finds | Why This Matters |
|----------|-------|------------------|
| E1 (semantic) | "async", "runtime" | Generic semantic matches |
| E11 (entity) | "Rust", "Actix" | Entity relationships via KEPLER |
| E8 (graph) | "imports", "depends on" | Structural relationships |
| E7 (code) | `tokio::spawn`, `#[tokio::main]` | Code patterns |

**Embedder-First Tools**:

| Tool | Purpose | Key Params |
|------|---------|------------|
| `search_by_embedder` | Search using any embedder (E1-E13) as primary | embedder, query, topK, includeAllScores |
| `get_embedder_clusters` | Explore clusters in a specific embedder's space | embedder, minClusterSize, topClusters |
| `compare_embedder_views` | Compare rankings from multiple embedders | query, embedders[], topK |
| `list_embedder_indexes` | List all embedder indexes with GPU stats | - |

**Use Cases**:
- **E11 search**: Find entity relationships that E1 misses ("Diesel" = database ORM for Rust)
- **E7 search**: Find code patterns and implementations
- **E5 search**: Explore causal relationships (why X caused Y)
- **E8 search**: Find structural relationships (imports, dependencies)
- **Compare views**: Understand blind spots by seeing how different embedders rank the same query

### 8.4 Per-Embedder Perspectives

| Embedder | Perspective | What It Finds |
|----------|-------------|---------------|
| E1 | Semantic | Dense semantic similarity - foundation |
| E2 | Recency | Temporal freshness - recent memories first |
| E3 | Periodic | Time-of-day patterns - daily/weekly cycles |
| E4 | Sequence | Conversation order - before/after relationships |
| E5 | Causal | Cause-effect relationships - why X caused Y |
| E6 | Keyword | Exact keyword matches - precise terminology |
| E7 | Code | Code patterns - function signatures, AST structure |
| E8 | Graph | Structural relationships - imports, dependencies |
| E9 | HDC | Noise-robust structure - typos, variations |
| E10 | Intent | Goal alignment - similar purpose, different words |
| E11 | Entity | Entity knowledge - named entities, relationships (KEPLER) |
| E12 | Precision | Exact phrase matches - token-level precision |
| E13 | Expansion | Term expansion - synonyms, related terms |

---

## 9. PERFORMANCE BUDGETS (GPU-Accelerated)

| Operation | Target | GPU Acceleration |
|-----------|--------|------------------|
| All 13 embed | <200ms | Batched Tensor Core FP16 |
| Per-space HNSW | <1ms | faiss-gpu IVF/HNSW |
| inject_context P95 | <500ms | Full GPU pipeline |
| store_memory P95 | <800ms | GPU embed + index |
| Any tool P99 | <1000ms | Worst case with GPU |
| Topic detection | <20ms | cuML HDBSCAN |
| Warm-load startup | <30s | All 13 models to VRAM |

**Comparison vs CPU-based system:**
| Operation | GPU (RTX 5090) | CPU (baseline) | Speedup |
|-----------|----------------|----------------|---------|
| All 13 embed | <200ms | ~2000ms | 10x |
| HNSW search | <1ms | ~5ms | 5x |
| HDBSCAN | <20ms | ~500ms | 25x |
| Full pipeline | <500ms | ~3000ms | 6x |

---

## 10. KEY THRESHOLDS

| Metric | Value |
|--------|-------|
| Topic threshold | weighted_agreement >= 2.5 |
| Max weighted agreement | 8.5 |
| Chunk size / overlap | 200 / 50 words |
| Cluster min size | 3 |
| Recency half-life | 45 days |
| Exploration budget | 15% (Thompson sampling) |
| Consolidation trigger | entropy > 0.7 AND churn > 0.5 |
| Duplicate detection | similarity > 0.90 |

---

## 11. ARCHITECTURAL RULES

### GPU-First Rules (Mandatory)
| Rule | Description |
|------|-------------|
| ARCH-GPU-01 | GPU is mandatory - no CPU fallback for embeddings |
| ARCH-GPU-02 | All 13 embedders warm-loaded into VRAM at startup |
| ARCH-GPU-03 | Embedding inference uses FP16/BF16 Tensor Cores |
| ARCH-GPU-04 | FAISS indexes use GPU (faiss-gpu) not CPU |
| ARCH-GPU-05 | HDBSCAN clustering runs on GPU via cuML |
| ARCH-GPU-06 | Batch operations preferred - minimize kernel launches |
| ARCH-GPU-07 | Green Contexts partition SMs: 70% inference, 30% indexing |
| ARCH-GPU-08 | CUDA streams for async embedding + indexing overlap |

### Core Rules
| Rule | Description |
|------|-------------|
| ARCH-01 | TeleologicalArray is atomic (all 13 or nothing) |
| ARCH-02 | Apples-to-apples only (E1↔E1, never E1↔E5) |
| ARCH-04 | Temporal (E2-E4) NEVER count toward topics |
| ARCH-12 | E1 is foundation - all retrieval starts with E1 |
| ARCH-17 | Strong E1 (>0.8): enhancers refine. Weak E1 (<0.4): enhancers broaden |
| ARCH-21 | Multi-space fusion uses Weighted RRF, not weighted sum |
| ARCH-25 | Temporal boosts POST-retrieval only |

**Forbidden (GPU)**:
- CPU embedding inference when GPU available
- Cold-loading embedders per-request
- CPU FAISS when GPU FAISS available
- sklearn HDBSCAN (use cuML)
- GPU→CPU→GPU transfers
- FP32 inference (use FP16/BF16)
- Per-item serialization (batch for GPU)
- Blocking sync (use CUDA streams)

**Forbidden (Core)**:
- Cross-embedder comparison (E1↔E5)
- Partial TeleologicalArray
- Temporal in similarity fusion
- E12 for initial retrieval (rerank only)
- E13 for final ranking (recall only)
- Simple weighted sum (use RRF)

---

## 12. GPU TESTING REQUIREMENTS

All tests and benchmarks MUST execute on GPU. No CPU fallback is acceptable.

### Testing Rules
| Rule | Requirement |
|------|-------------|
| Unit Tests | All embedder tests use GPU inference |
| Integration Tests | Full pipeline tests require GPU |
| Benchmarks | GPU-only, report VRAM usage + latency |
| CI/CD | Requires GPU runner (RTX 5090 or compatible) |
| Memory Profiling | Track VRAM allocation per embedder |

### Benchmark Requirements
```
cargo bench --features gpu
# MUST report:
# - GPU utilization %
# - VRAM peak usage (GB)
# - Tensor Core utilization %
# - Latency P50/P95/P99
# - Throughput (embeddings/sec)
```

### GPU Health Checks
- Startup: Verify CUDA 13.1, compute cap 12.0
- Warm-load: Confirm all 13 embedders in VRAM
- Runtime: Monitor VRAM fragmentation
- Shutdown: Clean VRAM deallocation

---

## 13. CUDA 13.1 FEATURE UTILIZATION

| Feature | Usage in Context Graph |
|---------|----------------------|
| CUDA Tile | Portable kernels for custom ops |
| Green Contexts | 70/30 SM split (inference/index) |
| Tensor Cores | FP16/BF16 embedder inference |
| CUDA Streams | Async embed + index overlap |
| cuBLAS | Batched GEMM for attention |
| cuSPARSE | E6/E13 sparse operations |
| MPS (60 clients) | Multi-session support |
