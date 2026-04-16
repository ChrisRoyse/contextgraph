# Context Graph PRD v6.4 (GPU-First 13-Perspectives Multi-Space System)

**Platform**: Claude Code CLI | **Architecture**: GPU-First | **Hardware**: RTX 5090 32GB + CUDA 13.1

**Core Insight**: 13 embedders = 13 unique perspectives on every memory, all warm-loaded on GPU

**New in v6.4**: Code Embedding Separation - Dedicated E7 pipeline for code with AST-aware chunking, tree-sitter parsing, and separate storage from teleological memories

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

## 1. CORE PHILOSOPHY: ALL EMBEDDERS ARE SIGNAL

### 1.0 System Goal: Multi-Perspective Navigation

**GOAL**: Use all 13 embedders to navigate massive datasets from multiple angles to optimally find all answers one seeks.

Each embedder provides a unique lens on the knowledge graph. The AI model receives **FULL VISIBILITY** into all 13 embedder scores to guide intelligent exploration of the knowledge space.

| Capability | Description |
|------------|-------------|
| **Full Visibility** | AI sees all 13 embedder scores per result, categorized by SEMANTIC, RELATIONAL, STRUCTURAL, TEMPORAL |
| **Blind Spot Detection** | Highlights when enhancers (E5-E13) found what E1 missed (enhancer >= 0.5 AND E1 < 0.3) |
| **Navigation Hints** | Suggests which embedder-specific tools to explore next based on score patterns |
| **Agreement Metrics** | Shows how many embedders agree on a result's relevance (agreementCount) |

**How AI Uses This Information**:
- Identify which embedders found strong matches for different query aspects
- Navigate to specialized tools (`search_code`, `search_by_entities`, `search_causes`, etc.)
- Understand blind spots where E1 missed but other embedders found signal
- Make informed decisions about which perspective to explore next

**Example Response Structure**:
```json
{
  "fingerprintId": "uuid",
  "similarity": 0.75,
  "e1Score": 0.25,
  "embedderScores": {
    "semantic": { "E1_Semantic": 0.25, "E7_Code": 0.85, ... },
    "relational": { "E11_Entity": 0.72, ... },
    "structural": { "E9_HDC": 0.45 },
    "temporal": { "E2_Recency": 0.90, ... }
  },
  "agreementCount": 5,
  "blindSpots": [
    { "embedder": "E7_Code", "score": 0.85, "e1Score": 0.25, "finding": "E7_Code found via code patterns but E1 missed" }
  ],
  "navigationHints": ["E7 (code) found more than E1 - try search_code for code patterns"]
}
```

### 1.1 The Signal, Not Noise Principle

**FUNDAMENTAL**: Every embedder provides SIGNAL, never noise. Each of the 13 embedders captures a unique dimension of meaning that E1 (semantic) alone cannot see.

| Embedder | What E1 Sees as Noise | What It Actually Is |
|----------|----------------------|---------------------|
| E7 | Code syntax (`::", `->`, `fn`) | **Structured code signal** - patterns, signatures |
| E5 | Word order in causal statements | **Directional signal** - cause→effect preserved |
| E6 | Exact term repetition | **Precision signal** - keyword matches matter |
| E11 | Entity names without context | **Knowledge signal** - "Diesel" = database ORM |
| E10 | Different words, same meaning | **Paraphrase signal** - same meaning, different words |

**When E1 misses something that E7 or E11 finds, that's not because E1 was wrong - it's because E7/E11 captured ADDITIONAL signal that E1 cannot encode in its representation space.**

### 1.2 Parameter Tuning = Signal Definition

Each embedder's parameters are tuned to define what its signal MEANS:

| Parameter Type | Purpose | Example |
|---------------|---------|---------|
| **Blend weights** | How strongly this signal influences final ranking | E7 blend=0.4 means 40% code signal |
| **Similarity thresholds** | What constitutes meaningful signal vs. weak | E7 high=0.80 for strong code match |
| **Boost factors** | How to amplify signal in specific contexts | E10 boosts 15% when E1 is weak |
| **Topic weights** | How much this signal contributes to topic detection | E7=1.0, E2=0.0 (temporal excluded) |

### 1.3 Combined Signal > Single Embedder

Each embedder finds what OTHERS MISS. Combined = superior answers.

**Example Query**: "What databases work with Rust?"
| Embedder | Finds | Signal Type |
|----------|-------|-------------|
| E1 | Memories containing "database" or "Rust" | Semantic signal |
| E11 | Memories about "Diesel" | Entity knowledge signal (knows Diesel IS a database ORM) |
| E7 | Code using `sqlx`, `diesel` crates | Code pattern signal |
| E5 | "Migration that broke production" | Causal signal |
| **Combined** | All of the above | Multi-perspective signal = superior answer |

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
| **E10** | V_multimodality | Paraphrase detection (different words) | Cross-lingual paraphrase gaps | Semantic | 1.0 |
| **E12** | V_precision | Exact phrase matches | Token-level precision lost | Semantic | 1.0 |
| **E13** | V_keyword | Term expansions (fast→quick) | Sparse term overlap missed | Semantic | 1.0 |
| **E8** | V_connectivity | Graph structure ("X imports Y") | Relationship structure (e5-large-v2) | Relational | 0.5 |
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
| E8 | 1024 | Cosine | e5-large-v2 GPU, structural relationships |
| E11 | 768 | TransE | KEPLER GPU, RoBERTa-base + TransE |
| E9 | 1024 | Hamming | GPU bitwise ops (10K→1024) |
| E10 | 768 | Cosine | Multiplicative boost, same GPU batch |
| E12 | 128D/token | MaxSim | GPU reranking ONLY |
| E13 | ~30K sparse | Jaccard | GPU Stage 1 recall ONLY |

**All 13 embedders are warm-loaded into GPU VRAM at MCP server startup. No cold-loading, no CPU fallback.**

### 2.3 Signal Definitions and Tunable Parameters

Each embedder's signal has specific meaning defined by tunable parameters. These are NOT arbitrary - they define what each signal REPRESENTS.

#### E7 Code Signal (Benchmark Validated)
| Parameter | Value | Signal Meaning |
|-----------|-------|----------------|
| `blend_weight` | 0.4 (default) | 40% code signal in final ranking |
| `similarity_threshold.high` | 0.80 | Strong code pattern match |
| `similarity_threshold.low` | 0.35 | Weak but present code signal |
| `language_detection` | auto | Detect Rust/Python/TS from query |

**Benchmark Results**: +69% improvement for signature searches, +29% for pattern searches, +19.2% overall MRR improvement over pure E1.

#### E10 Paraphrase Signal (Multiplicative Boost)
| E1 Strength | Boost | Rationale |
|-------------|-------|-----------|
| Strong (>0.8) | 5% | E1 is confident, E10 refines |
| Medium (0.4-0.8) | 10% | Balanced enhancement |
| Weak (<0.4) | 15% | E1 uncertain, E10 broadens search |
| Multiplier clamp | [0.8, 1.2] | Never override E1 completely |

**Signal meaning**: Same goal expressed differently. E10 boosts memories with aligned intent even when words differ.

#### E5 Causal Signal (Asymmetric)
| Direction | Modifier | Signal Meaning |
|-----------|----------|----------------|
| cause→effect | 1.2x | "What did X cause?" gets boost |
| effect→cause | 0.8x | "What caused X?" dampened appropriately |

**Signal meaning**: Preserves causal direction that E1 loses through symmetric cosine similarity.

#### E11 Entity Knowledge Signal (KEPLER TransE)
| Parameter | Value | Signal Meaning |
|-----------|-------|----------------|
| `transe_score > -2.0` | Valid relation | High confidence entity relationship |
| `transe_score -2.0 to -5.0` | Uncertain | Possible but unconfirmed relation |
| `exact_match_boost` | 1.3x | Exact entity name match bonus |

**Signal meaning**: "Diesel" = database ORM for Rust. Entity relationships E1 cannot encode.

#### E2-E4 Temporal Signal (POST-RETRIEVAL ONLY)
| Parameter | Value | Usage |
|-----------|-------|-------|
| `recency_boost.under_1h` | 1.3x | Very recent, post-retrieval only |
| `recency_boost.under_1d` | 1.2x | Same-day, post-retrieval only |
| `topic_weight` | 0.0 | **NEVER** contributes to topic detection |

**Signal meaning**: Temporal context badges applied AFTER retrieval. Temporal proximity ≠ semantic relationship.

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

## 5.4 CODE EMBEDDING SEPARATION

Code entities are stored in a **separate pipeline** from the 13-embedder teleological system. E7 (Qodo-Embed-1-1.5B) serves as the primary embedder for code, enabling specialized code search without polluting the memory graph.

### Why Separate Code?

| Challenge with Mixed Storage | Solution with Separation |
|------------------------------|--------------------------|
| Code syntax treated as NL noise | E7 treats code as structured signal |
| Word-based chunking breaks functions | AST-aware chunking preserves boundaries |
| Topic detection confused by code | Code excluded from topic clustering |
| Function signatures diluted | Signature-specific search supported |

### Architecture

```
Source File → Git Watcher → Tree-sitter AST → Code Entities → E7 Embeddings → CodeStore
                                    ↓
                          (Separate from TeleologicalStore)
```

### Code Entity Types

| Type | Description | Example |
|------|-------------|---------|
| Function | Standalone function | `fn parse_config()` |
| Method | Function in impl block | `impl Config { fn load() }` |
| Struct | Struct definition | `struct Config { ... }` |
| Enum | Enum definition | `enum Status { ... }` |
| Trait | Trait definition | `trait Parser { ... }` |
| Impl | Impl block | `impl Parser for Config` |

### AST Chunking (Tree-sitter)

Per Qodo best practices and cAST paper (EMNLP 2025):

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Target size | 500 chars | Qodo recommendation for code |
| Min size | 100 chars | Avoid tiny fragments |
| Max size | 1000 chars | Prevent semantic dilution |
| Parent context | Included | Methods include parent struct |
| Imports | Included | Relevant imports in each chunk |

### Storage Schema

```
CodeStore {
  code_entities: CF        // Metadata: name, language, signature, visibility
  code_e7_embeddings: CF   // 1536D E7 vectors
  code_file_index: CF      // File path → entity IDs
  code_name_index: CF      // Entity name secondary index
  code_signature_index: CF // Signature secondary index
}
```

### Code Search

Code search uses E7 as primary, with optional E1 semantic enhancement:

| Mode | Description | Use Case |
|------|-------------|----------|
| `signature` | Match function signatures | "Find functions that take `&str`" |
| `pattern` | Match code patterns | "Find async functions with `await`" |
| `semantic` | E7 + E1 blended | "Find authentication code" |

**Default blend weight**: 0.4 (40% E1 semantic, 60% E7 code)

### GPU Memory

The E7 code model (~3GB) is loaded **separately** from the 8GB budget for the 13 teleological models:

| Allocation | Size | Notes |
|------------|------|-------|
| 13 Teleological Models | 8GB | Warm-loaded at startup |
| E7 Code Model (Separate) | 3GB | Dedicated, not shared |
| Total for Embedders | ~11GB | Leaves 21GB for indexes/buffers |

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
| `store_memory` | Store with embeddings | content, importance, rationale |
| `inject_context` | Retrieval + injection | query, max_tokens |

### 8.2 Code Search Operations

| Tool | Purpose | Key Params |
|------|---------|------------|
| `search_code` | E7-first code search | query, topK, blendWithSemantic, languageHint |
| `search_code_entities` | Search by entity type | entityType, name, signature |
| `get_code_entity` | Get entity by ID | entityId, includeEmbedding |
| `list_code_files` | List indexed code files | pathPattern, language |

**Code Search Modes**:
| Mode | Description | E7/E1 Blend |
|------|-------------|-------------|
| `signature` | Match function signatures | 80% E7, 20% E1 |
| `pattern` | Match code patterns | 70% E7, 30% E1 |
| `semantic` | Semantic code search | 60% E7, 40% E1 |

### 8.3 Topic & Maintenance

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
| E10 | Paraphrase | Paraphrase detection - same meaning, different words |
| E11 | Entity | Entity knowledge - named entities, relationships (KEPLER) |
| E12 | Precision | Exact phrase matches - token-level precision |
| E13 | Expansion | Term expansion - synonyms, related terms |

### 8.5 Autonomous Multi-Embedder Enrichment

The enrichment system automatically selects and combines multiple embedders based on query characteristics, providing richer insights without requiring separate tool calls.

#### Philosophy
- **E1 is foundation** - All retrieval starts with E1 (ARCH-12)
- **Enhancers find what E1 misses** - E5 finds causal chains, E7 finds code patterns, E11 finds entity knowledge
- **Combined = superior answers** - Agreement across embedders indicates confidence

#### Enrichment Modes

| Mode | Behavior | Use Case |
|------|----------|----------|
| `off` | E1 only (legacy) | Backwards compatibility |
| `light` | E1 + 1-2 enhancers, basic agreement metrics | Default - balanced |
| `full` | All relevant embedders, full metrics, blind spot detection | Maximum insight |

#### Query Type Detection

| Type | Pattern Examples | Selected Embedders |
|------|------------------|-------------------|
| CAUSAL | "why", "caused by", "led to" | E5 (asymmetric causal) |
| CODE | function names, imports, `fn`, `class` | E7 (code patterns) |
| ENTITY | Capitalized names, known entities | E11 (KEPLER entity knowledge) |
| PARAPHRASE | "same as", "similar to", "means" | E10 (paraphrase detection) |
| KEYWORD | Quoted terms, technical jargon | E6, E13 (sparse keyword) |
| TEMPORAL | "before", "after", "yesterday" | E2-E4 (post-retrieval only) |

#### Output Enrichment

Each result includes:
- **ScoringBreakdown**: E1 score, enhancer scores, RRF final score
- **AgreementMetrics**: Which embedders agree, weighted agreement (topic threshold: 2.5)
- **BlindSpotAlert**: Results found by enhancers but missed by E1 (E1 score < 0.3)

#### Example Usage

```json
{
  "query": "why did the authentication fail",
  "enrichMode": "light"
}
```

Response includes:
- Detected types: `["CAUSAL"]`
- E5 asymmetric causal search applied
- Agreement metrics showing E1/E5 alignment
- Blind spots: entities E11 found that E1 missed

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

### Signal Philosophy Rules (FUNDAMENTAL)
| Rule | Description |
|------|-------------|
| ARCH-SIGNAL-01 | ALL 13 embedders provide SIGNAL, never noise - each captures unique dimensions of meaning |
| ARCH-SIGNAL-02 | What E1 misses is ADDITIONAL signal, not noise - E7/E11/E5 capture what E1 cannot encode |
| ARCH-SIGNAL-03 | Parameter tuning defines signal MEANING - blend weights, thresholds, boosts define each embedder's role |
| ARCH-SIGNAL-04 | Combined signal > any single embedder - multi-perspective retrieval is always superior |
| ARCH-SIGNAL-05 | Enhancers complement E1, never compete - E10 boosts E1, doesn't replace it |

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
