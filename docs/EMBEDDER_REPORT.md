# Context Graph: 13-Embedder Architecture Report

## Executive Summary

The Context Graph system employs **13 specialized embedding models** (E1-E13) operating in parallel across distinct semantic dimensions. Each embedder captures a unique facet of meaning that the others cannot see, enabling multi-perspective knowledge graph navigation far beyond what any single embedding model could achieve. Every memory stored in the system receives a **SemanticFingerprint** containing vectors from all 13 embedders simultaneously, creating a rich, multi-dimensional representation.

The 13 embedders are organized into four categories:
- **Semantic** (7 embedders, weight 1.0): E1, E5, E6, E7, E10, E12, E13
- **Temporal** (3 embedders, weight 0.0): E2, E3, E4
- **Relational** (2 embedders, weight 0.5): E8, E11
- **Structural** (1 embedder, weight 0.5): E9

---

## Part 1: Individual Embedder Deep-Dives

### E1: V_meaning (Semantic Foundation)

| Property | Value |
|----------|-------|
| **Model** | e5-large-v2 (Matryoshka-capable) |
| **Dimensions** | 1024D dense |
| **Similarity Metric** | Cosine |
| **Index Type** | HNSW |
| **Category** | Semantic (weight 1.0) |
| **Teleological Purpose** | V_meaning |

**What It Does:**
E1 is the **foundation layer** for all retrieval in the system (per ARCH-12). It produces 1024-dimensional dense semantic embeddings using the e5-large-v2 model. These vectors capture general topical and conceptual similarity between texts. Every search begins with E1; all other embedders enhance E1's signal but never replace it.

**What It Finds:**
- Dense semantic similarity and topical relatedness
- Conceptual matches even when different vocabulary is used
- General meaning alignment between queries and memories

**What It Misses (Why Other Embedders Exist):**
- **Entities**: E1 treats "Diesel" as a generic word; it doesn't know it's a Rust ORM
- **Code patterns**: E1 treats `async fn` as two separate semantic tokens, losing structural meaning
- **Causal direction**: E1 treats cause-effect relationships as symmetric
- **Exact keywords**: E1 averages all token meanings together, diluting exact term matches

**Special Variant - E1Matryoshka128:**
A 128-dimensional truncation of E1 used exclusively in pipeline Stage 2 for fast candidate filtering. The Matryoshka architecture means the first 128 dimensions retain the most important semantic information, enabling 8x faster approximate scoring.

**System Integration:**
- Default primary embedder in the `semantic_search` weight profile (0.33 weight)
- Foundation for `intent_search` profile (0.55 weight, highest of any profile)
- Minimum weight of 0.12 even in the `temporal_navigation` profile
- Used as the baseline for E10's multiplicative boost (ARCH-17)

---

### E2: V_freshness (Temporal Recency)

| Property | Value |
|----------|-------|
| **Model** | Exponential decay function |
| **Dimensions** | 512D dense |
| **Similarity Metric** | N/A (post-retrieval badge) |
| **Index Type** | HNSW |
| **Category** | Temporal (weight 0.0) |
| **Teleological Purpose** | V_freshness |

**What It Does:**
E2 encodes **temporal recency** using configurable decay functions (linear, exponential, step, or no-decay). The exponential decay formula is:

```
score = exp(-age * ln(2) / half_life)
```

This means a memory at the half-life age scores 0.5, and the score approaches 0.0 for very old memories.

**What It Finds:**
- Recently created or accessed memories
- Temporal proximity to the current moment

**Critical Constraint - POST-RETRIEVAL ONLY (ARCH-25, AP-60):**
E2 has **weight 0.0** in ALL semantic search profiles. Time proximity does NOT indicate topical similarity. A document created 5 minutes ago is not more relevant than one created 5 hours ago just because it's newer. E2 is applied as a **post-retrieval badge** after similarity-based ranking is complete, allowing recency to break ties between equally relevant results but never to override semantic relevance.

**System Integration:**
- Weight 0.0 in: `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `intent_search`, `graph_reasoning`
- Weight 0.22 in: `temporal_navigation` (explicit time-based queries only)
- Weight 0.05 in: `sequence_navigation`, `conversation_history`
- Never used for divergence detection (AP-63)

---

### E3: V_periodicity (Temporal Patterns)

| Property | Value |
|----------|-------|
| **Model** | Fourier periodic components |
| **Dimensions** | 512D dense |
| **Similarity Metric** | N/A (post-retrieval badge) |
| **Index Type** | HNSW |
| **Category** | Temporal (weight 0.0) |
| **Teleological Purpose** | V_periodicity |

**What It Does:**
E3 captures **recurring temporal patterns** using Fourier decomposition. It encodes hour-of-day (0-23) and day-of-week (0-6, Sunday=0) as periodic signals, enabling the system to find memories associated with the same recurring time patterns.

**What It Finds:**
- Same time-of-week patterns ("Friday deployment reviews")
- Daily recurring patterns ("3pm standup notes")
- Cyclical temporal correlations

**What It Misses:**
- Semantic relationships (completely orthogonal to topic)
- One-time events that happened to fall at a particular time

**Critical Constraint - POST-RETRIEVAL ONLY (ARCH-25, AP-60):**
Like E2, E3 has weight 0.0 in all semantic profiles. Periodic time patterns are metadata, not semantic signals.

**System Integration:**
- Weight 0.0 in all semantic profiles
- Weight 0.22 in `temporal_navigation`
- Enables queries like "what do I usually work on at this time of week?"

---

### E4: V_ordering (Temporal Sequence)

| Property | Value |
|----------|-------|
| **Model** | Sinusoidal positional encoding (Transformer-style) |
| **Dimensions** | 512D dense |
| **Similarity Metric** | N/A (post-retrieval badge) |
| **Index Type** | HNSW |
| **Category** | Temporal (weight 0.0) |
| **Teleological Purpose** | V_ordering |

**What It Does:**
E4 encodes **sequence position** using sinusoidal positional encodings adapted from the Transformer architecture. Rather than encoding absolute time like E2 or periodic time like E3, E4 encodes the **relative ordering** of memories in conversations and document flows.

**What It Finds:**
- "What came before/after this memory?" (sequence navigation)
- Conversation flow and document position
- Temporal ordering relationships

**Unique Capability:**
E4 is the only embedder that understands sequence. When you need to traverse conversation history ("what was discussed before the database migration decision?"), E4 provides the positional signal to find adjacent memories.

**System Integration:**
- **Primary** in `sequence_navigation` profile (weight 0.55, highest of any embedder in any profile)
- Strong presence in `conversation_history` (weight 0.35)
- Weight 0.22 in `temporal_navigation`
- Weight 0.0 in all semantic profiles per AP-60
- Exposed via MCP `session_timeline` tools using E4's sequence space

---

### E5: V_causality (Causal Reasoning)

| Property | Value |
|----------|-------|
| **Model** | Longformer Structural Causal Model (SCM) |
| **Dimensions** | 768D dense |
| **Similarity Metric** | **AsymmetricCosine** (direction matters!) |
| **Index Type** | HNSW (3 indexes: E5Causal, E5CausalCause, E5CausalEffect) |
| **Category** | Semantic (weight 1.0) |
| **Teleological Purpose** | V_causality |

**What It Does:**
E5 captures **cause-effect relationships** using a Longformer-based Structural Causal Model. Unlike all other dense embedders, E5 uses **asymmetric similarity** because causal direction matters: "A caused B" is fundamentally different from "B caused A."

Each memory gets TWO E5 vectors:
- `e5_causal_as_cause`: How this memory functions as a cause
- `e5_causal_as_effect`: How this memory functions as an effect

**Direction Boost (AP-77):**
- cause->effect queries: 1.2x boost
- effect->cause queries: 0.8x boost

**Three HNSW Indexes (ARCH-15):**
- **E5CausalCause**: Contains cause vectors. Search when query seeks EFFECTS ("what happens if X?")
- **E5CausalEffect**: Contains effect vectors. Search when query seeks CAUSES ("why did X happen?")
- **E5Causal**: Legacy combined index, backward compatible

**What It Finds:**
- Causal chains: "Migration X broke production because Y"
- Root cause analysis: "Why did the authentication fail?"
- Impact prediction: "What would happen if we change the schema?"

**Why E1 Can't Do This:**
E1 treats cause and effect symmetrically. When you search "what caused the outage?", E1 finds documents mentioning outages, but E5 specifically finds the CAUSES of outages rather than their effects.

**System Integration:**
- **Primary** in `causal_reasoning` profile (weight 0.45)
- Supporting role in `semantic_search` (0.15), `conversation_history` (0.10)
- Direction-aware similarity in `MultiSpaceSimilarity::compute_similarity_with_direction()`
- Exposed via MCP `search_causes`, `search_effects`, `get_causal_chain` tools

---

### E6: V_selectivity (Sparse Keywords)

| Property | Value |
|----------|-------|
| **Model** | BM25 inverted index over BERT vocabulary |
| **Dimensions** | ~30,522 sparse (BERT vocabulary) |
| **Similarity Metric** | BM25 / Jaccard overlap |
| **Index Type** | **Inverted index** (NOT HNSW) |
| **Category** | Semantic (weight 1.0) |
| **Teleological Purpose** | V_selectivity |

**What It Does:**
E6 provides **keyword-level precision** that E1's dense embeddings dilute through token averaging. It uses a sparse inverted index over BERT's 30,522-token vocabulary, enabling exact term matching with BM25 scoring.

**What It Finds:**
- Exact term matches: error codes ("ERR_CONNECTION_REFUSED"), specific identifiers
- Domain-specific jargon that E1 might average away
- Precise keyword overlap when semantic similarity isn't enough

**Why E1 Can't Do This:**
E1 averages all token embeddings into a single 1024D vector, which means the specific presence of "ERR_CONNECTION_REFUSED" is diluted into general "connection error" semantics. E6 preserves the exact tokens.

**Difference from E13 (SPLADE):**
E6 uses raw BM25 keyword matching (exact terms only), while E13 uses learned term expansion (maps "fast" -> "quick"). E6 is for precision; E13 is for recall.

**System Integration:**
- Supporting role in most profiles: `semantic_search` (0.05), `code_search` (0.10), `fact_checking` (0.15)
- Strong presence in `pipeline_stage1_recall` (0.25, supporting E13's expansion)
- Exposed via MCP `search_by_keywords` tool

---

### E7: V_correctness (Code Intelligence)

| Property | Value |
|----------|-------|
| **Model** | Qodo-Embed-1-1.5B |
| **Dimensions** | 1536D dense |
| **Similarity Metric** | Cosine (code-aware) |
| **Index Type** | HNSW |
| **Category** | Semantic (weight 1.0) |
| **Teleological Purpose** | V_correctness |

**What It Does:**
E7 is the **code intelligence** embedder, using the Qodo-Embed-1-1.5B model (the largest model in the system at 1536 dimensions) specifically trained on source code. It understands programming language syntax, function signatures, design patterns, and implementation structures.

**Benchmark Results:**
- +69% improvement over E1 for function signature searches
- +29% improvement for code pattern searches

**What It Finds:**
- Function signatures: `async fn connect(config: &DbConfig) -> Result<Pool>`
- Code patterns: dependency injection, observer pattern, builder pattern
- Language-specific constructs: Rust's `impl Trait`, Python's `@decorator`, etc.
- Implementation-level similarities

**Why E1 Can't Do This:**
E1 treats code as natural language. It sees `async fn` as two words meaning "asynchronous" and "function" separately, losing the structural meaning of an async function declaration. E7 understands `tokio::spawn(async { ... })` as a meaningful code pattern unit.

**System Integration:**
- **Primary** in `code_search` profile (weight 0.40)
- Strong supporting role in `semantic_search` (0.20), `intent_search` (0.18)
- Exposed via MCP `search_code` tool
- Powers the code watcher's AST-based similarity analysis

---

### E8: V_connectivity (Graph Structure)

| Property | Value |
|----------|-------|
| **Model** | e5-large-v2 (adapted for structural/graph data) |
| **Dimensions** | 384D dense |
| **Similarity Metric** | **AsymmetricCosine** (direction matters!) |
| **Index Type** | HNSW |
| **Category** | Relational (weight 0.5) |
| **Teleological Purpose** | V_connectivity |

**What It Does:**
E8 captures **graph-structural relationships** like imports, dependencies, and connections. Like E5, it uses asymmetric similarity because direction matters in graphs: "A imports B" is fundamentally different from "B imports A."

Each memory gets TWO E8 vectors:
- `e8_graph_as_source`: How this memory functions as a dependency source
- `e8_graph_as_target`: How this memory functions as a dependency target

**Direction Boost:**
- source->target queries: 1.2x boost
- target->source queries: 0.8x boost

**What It Finds:**
- "What imports module X?" (search E8 source index)
- "What does module Y depend on?" (search E8 target index)
- Structural connectivity and dependency chains

**Why E1 Can't Do This:**
E1 treats import relationships as just "related code." It doesn't understand that "A imports B" and "B imports A" represent fundamentally different dependency directions.

**System Integration:**
- **Primary** in `graph_reasoning` profile (weight 0.40)
- Supporting in `causal_reasoning` (0.10)
- Exposed via MCP `search_connections`, `get_graph_path`, `discover_graph_relationships` tools
- Relational category means it contributes 0.5 to topic weighted agreement

---

### E9: V_robustness (Hyperdimensional Computing)

| Property | Value |
|----------|-------|
| **Model** | HDC (projected from 10K-bit hypervectors) |
| **Dimensions** | 1024D dense (projected from 10,000-bit binary) |
| **Similarity Metric** | Cosine (with Hamming-based structural similarity) |
| **Index Type** | HNSW |
| **Category** | Structural (weight 0.5) |
| **Teleological Purpose** | V_robustness |

**What It Does:**
E9 uses **Hyperdimensional Computing (HDC)** to create noise-robust embeddings. The original 10,000-bit binary hypervectors are projected to 1024D dense vectors for HNSW indexing. HDC operates in a geometry where character-level variations have minimal impact on similarity, making it robust to typos, misspellings, and variations.

**What It Finds:**
- Matches despite typos: "authetication" still finds "authentication"
- Character-level structural patterns preserved despite noise
- Robust retrieval when query quality is low

**Why E1 Can't Do This:**
E1's token-level embeddings treat "authetication" as an out-of-vocabulary token or break it into subwords differently from "authentication," causing a significant drop in similarity. E9's character-level Hamming-based approach handles this gracefully.

**System Integration:**
- **Primary boost** in `typo_tolerant` profile (weight 0.15)
- Minimal presence in most semantic profiles (0.02-0.05)
- Supporting role in `pipeline_stage1_recall` (0.05, helps recall with noisy queries)
- Structural category means it contributes 0.5 to topic weighted agreement

---

### E10: V_multimodality (Intent Alignment)

| Property | Value |
|----------|-------|
| **Model** | CLIP (cross-modal alignment, adapted for text intent) |
| **Dimensions** | 768D dense |
| **Similarity Metric** | **AsymmetricCosine** + **Multiplicative Boost** on E1 |
| **Index Type** | HNSW (3 indexes: E10Multimodal, E10MultimodalIntent, E10MultimodalContext) |
| **Category** | Semantic (weight 1.0) |
| **Teleological Purpose** | V_multimodality |

**What It Does:**
E10 captures **intent/goal alignment** using CLIP, originally designed for text-image alignment but adapted here for text intent-context matching. It understands whether two pieces of text serve the **same purpose** even if they use completely different vocabulary.

Each memory gets TWO E10 vectors:
- `e10_multimodal_as_intent`: The goal/purpose this memory serves
- `e10_multimodal_as_context`: The contextual background of this memory

**Critical Architecture - Multiplicative Boost (ARCH-17):**
E10 does NOT participate in linear weighted fusion like other embedders. Instead, it applies a **multiplicative boost** to E1's score:

```
boosted_score = E1_score * (1 + boost)
```

Where boost is adaptive:
- E1 > 0.8 (strong match): +5% boost (refine)
- E1 0.4-0.8 (medium): +10% boost
- E1 < 0.4 (weak): +15% boost (broaden)
- Clamped to [0.8, 1.2] multiplier range

**Critical Constraint (AP-80, AP-84):**
- E10 NEVER overrides E1. When E1_score = 0, E10 contribution = 0
- E10 weight is 0.0 in `intent_search` and `intent_enhanced` profiles (boost mechanism instead)

**System Integration:**
- Multiplicative boost in `intent_search` and `intent_enhanced` profiles
- Linear weight of 0.15 in `semantic_search` (for non-intent queries)
- Exposed via MCP `search_by_intent` tool
- Three HNSW indexes per ARCH-15 for asymmetric intent-context matching

---

### E11: V_factuality (Entity Knowledge)

| Property | Value |
|----------|-------|
| **Model** | KEPLER (RoBERTa-base + TransE knowledge graph embeddings) |
| **Dimensions** | 768D dense |
| **Similarity Metric** | Cosine (backed by TransE scoring) |
| **Index Type** | HNSW |
| **Category** | Relational (weight 0.5) |
| **Teleological Purpose** | V_factuality |

**What It Does:**
E11 combines **language understanding** (RoBERTa-base) with **structured knowledge** (TransE knowledge graph embeddings) to understand named entity relationships. It knows that "Diesel" IS a "Rust ORM," that "PostgreSQL" IS a "relational database," and that these have specific typed relationships.

**TransE Scoring:**
- Valid relationship: score > -2.0
- Uncertain: -2.0 to -5.0
- Invalid: < -5.0
- Jaccard boost: 1.3x for exact entity name matches

**What It Finds:**
- Entity-specific knowledge: "Diesel" -> "ORM" -> "Rust" -> "database"
- Fact validation: confirming or refuting entity relationships
- Named entity disambiguation

**Why E1 Can't Do This:**
E1 treats "Diesel" as a generic semantic token. It might associate it with fuel or engines. E11 knows through TransE that "Diesel" in a Rust codebase has a specific relationship to ORM and database concepts.

**System Integration:**
- **Primary** in `fact_checking` profile (weight 0.40)
- Strong in `graph_reasoning` (0.20)
- Exposed via MCP `search_by_entities`, `find_related_entities`, `get_entity_graph` tools
- Used in entity extraction pipeline via `extract_entities` tool
- Relational category contributes 0.5 to topic weighted agreement

---

### E12: V_precision (Late Interaction ColBERT)

| Property | Value |
|----------|-------|
| **Model** | ColBERT (token-level embeddings) |
| **Dimensions** | 128D per token (variable token count) |
| **Similarity Metric** | **MaxSim** (max per-token similarity) |
| **Index Type** | **Specialized MaxSim** (NOT HNSW) |
| **Category** | Semantic (weight 1.0) |
| **Teleological Purpose** | V_precision |

**What It Does:**
E12 uses **ColBERT-style late interaction** to provide token-level precision. Unlike E1 which produces a single vector for the entire text, E12 produces a separate 128D embedding for EACH token. Similarity is computed using MaxSim: for each query token, find the maximum similarity to any document token, then sum.

**Critical Constraint - RERANKING ONLY (AP-73, AP-74):**
E12 is used **exclusively in pipeline Stage 3 for reranking** the top ~100 candidates. It is NEVER used for initial retrieval because:
1. Token-level storage is expensive (N vectors per document vs 1)
2. MaxSim doesn't map to HNSW's nearest-neighbor search
3. Its precision is only valuable for distinguishing between already-similar candidates

**What It Finds:**
- Exact phrase matches at the token level
- Precise disambiguation between very similar candidates
- Token-level semantic alignment

**System Integration:**
- Weight 0.0 in ALL semantic scoring profiles (pipeline stage only)
- Used in pipeline Stage 3 via MaxSim reranking
- Contributes to topic detection as Semantic category (weight 1.0)
- Stored as `Vec<Vec<f32>>` in SemanticFingerprint (variable-length)

---

### E13: V_keyword_precision (SPLADE)

| Property | Value |
|----------|-------|
| **Model** | SPLADE v3 (Sparse Prediction of Linguistic Evidence Diffusion) |
| **Dimensions** | ~30,522 sparse (BERT vocabulary, learned expansion) |
| **Similarity Metric** | BM25 with learned term expansion |
| **Index Type** | **Inverted index** (NOT HNSW) |
| **Category** | Semantic (weight 1.0) |
| **Teleological Purpose** | V_keyword_precision |

**What It Does:**
E13 uses **SPLADE** (Sparse Prediction of Linguistic Evidence Diffusion) to produce sparse vectors where each dimension corresponds to a BERT vocabulary term, but with **learned term expansion**. Unlike E6's exact BM25, E13 learns that "fast" should also activate "quick," "rapid," and "efficient."

**Critical Constraint - STAGE 1 RECALL ONLY (AP-75):**
E13 is used **exclusively in pipeline Stage 1** to cast a wide initial net (~10,000 candidates). It is NOT used for final ranking because:
1. Learned expansions can introduce noise in precision-critical final ranking
2. Its strength is recall (finding ALL relevant candidates), not precision

**Difference from E6:**
| Feature | E6 (Sparse) | E13 (SPLADE) |
|---------|-------------|--------------|
| Term matching | Exact only | Learned expansion |
| "fast" finds "quick"? | No | Yes |
| Pipeline stage | Any | Stage 1 only |
| Strength | Precision | Recall |

**System Integration:**
- Weight 0.0 in most semantic profiles (pipeline stage only)
- Weight 0.25 in `pipeline_stage1_recall` (primary recall mechanism)
- Weight 0.07 in `pipeline_full` (mild fusion awareness)
- Stored as `SparseVector` in SemanticFingerprint

---

## Part 2: Multi-Embedder Fusion Architecture

### Weighted Reciprocal Rank Fusion (WRRF)

The system combines results from multiple embedders using **Weighted Reciprocal Rank Fusion** per ARCH-21:

```
WRRF_Score(result r) = Sum_i( weight_i / (K + rank_i(r)) )
```

Where:
- **K = 60** (RRF constant, per constitution)
- **weight_i** = embedder weight from the selected profile
- **rank_i(r)** = position of result r in embedder i's ranked list (0-indexed)

**Why WRRF Instead of Weighted Sum:**
1. **Rank-based, not score-based**: Avoids score miscalibration across embedders (cosine scores from different models aren't directly comparable)
2. **Scale-invariant**: Handles different score distributions (0-1 dense vs log-scale BM25)
3. **Consensus-favoring**: Results appearing in multiple embedder lists get naturally boosted

### Weight Profiles

The system defines 15 weight profiles, each tuning all 13 embedder weights for a specific query type:

| Profile | Primary Embedder | Key Weights | Use Case |
|---------|------------------|-------------|----------|
| `semantic_search` | E1 (0.33) | E7:0.20, E5:0.15, E10:0.15 | General queries |
| `causal_reasoning` | E5 (0.45) | E1:0.20, E7:0.10, E8:0.10 | "Why?" questions |
| `code_search` | E7 (0.40) | E1:0.20, E5:0.10, E6:0.10 | Programming queries |
| `fact_checking` | E11 (0.40) | E6:0.15, E5:0.15, E1:0.15 | Entity/fact validation |
| `intent_search` | E1 (0.55) + E10 boost | E7:0.18, E5:0.12 | Goal-based queries |
| `graph_reasoning` | E8 (0.40) | E11:0.20, E1:0.15, E6:0.10 | Structural queries |
| `temporal_navigation` | E2+E3+E4 (0.22 each) | E1:0.12 | Explicit time queries |
| `sequence_navigation` | E4 (0.55) | E1:0.20, E2:0.05 | Conversation flow |
| `conversation_history` | E4 (0.35) | E1:0.30, E5:0.10 | Contextual recall |
| `typo_tolerant` | E1 (0.30) + E9 (0.15) | E7:0.15, E10:0.12 | Noisy queries |
| `pipeline_stage1_recall` | E13 (0.25) + E6 (0.25) | E1:0.20, E7:0.10 | Wide recall |
| `pipeline_stage2_scoring` | E1 (0.50) | E7:0.15, E5:0.12 | Dense scoring |
| `pipeline_full` | E1 (0.40) | E7:0.15, E6:0.10, E5:0.10 | Complete pipeline |
| `category_weighted` | Constitution-compliant | Category-based | Formal compliance |
| `balanced` | Equal (0.077 each) | All equal | Testing/comparison |

**All profiles enforce these invariants:**
- Weights sum to 1.0 (validated at compile time)
- Temporal embedders (E2-E4) = 0.0 in all semantic profiles (AP-60)
- E12 (ColBERT) and E13 (SPLADE) = 0.0 in most profiles (pipeline-stage only)

---

## Part 3: Search Pipeline Architecture

### 5-Stage Pipeline

For maximum precision, the system supports a full 5-stage search pipeline:

```
Stage 1 (Recall)     E13 SPLADE inverted index     ~10,000 candidates
       |
Stage 2 (Scoring)    E1 Matryoshka128 fast filter   ~1,000 candidates
       |
Stage 3 (Ranking)    Multi-space WRRF fusion         ~100 candidates
       |
Stage 4 (Reranking)  E12 ColBERT MaxSim              ~top_k results
       |
Stage 5 (Validation) Final relevance check            Final results
```

**Stage 1 - E13 Recall:** SPLADE's learned term expansion casts the widest possible net, finding candidates that exact keyword matching would miss.

**Stage 2 - E1 Matryoshka128:** The 128D truncation of E1 provides fast approximate semantic scoring to eliminate obviously irrelevant candidates from Stage 1.

**Stage 3 - Multi-Space WRRF:** Full 13-embedder fusion with the selected weight profile. This is where the system's multi-perspective advantage materializes.

**Stage 4 - E12 Reranking:** ColBERT's token-level MaxSim precisely disambiguates the top candidates, ensuring the final ordering reflects exact phrase-level matches.

**Stage 5 - Validation:** Final relevance threshold check and post-retrieval temporal badges (E2-E4) applied.

### Simpler Search Modes

Not every query needs the full pipeline:

1. **E1-Only Search**: Single HNSW query (~1-2ms). For simple semantic queries.
2. **Multi-Space Search**: Parallel HNSW queries + WRRF (~5-10ms). For when E1 blind spots matter.
3. **Embedder-First Search**: Any single embedder as primary. For specialized perspectives.
4. **Full Pipeline**: All 5 stages (~20-50ms). For maximum precision.

---

## Part 4: Topic Detection and Divergence

### Topic Detection via Weighted Agreement

Topics are detected using **HDBSCAN clustering** across all 13 embedding spaces with weighted agreement:

```
weighted_agreement = Sum(topic_weight_i * is_clustered_i)
```

Where `is_clustered_i` = 1 if the memory is clustered with the topic in embedder i's space.

**Topic Threshold (ARCH-09):** weighted_agreement >= 2.5

**Maximum Possible:** 8.5 = (7 * 1.0) + (2 * 0.5) + (1 * 0.5) + (3 * 0.0)

**Examples from Constitution:**
| Scenario | Score | Result |
|----------|-------|--------|
| 3 semantic spaces agreeing | 3.0 | TOPIC |
| 2 semantic + 1 relational | 2.5 | TOPIC |
| 2 semantic spaces only | 2.0 | NOT TOPIC |
| 5 temporal spaces | 0.0 | NOT TOPIC |
| 1 semantic + 3 relational | 2.5 | TOPIC |

### Divergence Detection (ARCH-10)

Divergence (topic drift) is detected using **SEMANTIC embedders only**:
- Checks: E1, E5, E6, E7, E10, E12, E13 (7 spaces)
- Ignores: E2-E4 (temporal), E8/E11 (relational), E9 (structural)
- Triggers when score falls BELOW low threshold in any semantic space

**Severity Levels:**
- **High**: score < 0.10 (surfaced to user)
- **Medium**: 0.10 <= score < 0.20 (logged only)
- **Low**: score >= 0.20 (logged only)

---

## Part 5: Similarity Thresholds Per Embedder

Each embedder has calibrated high (relevance) and low (divergence) thresholds:

| Embedder | High Threshold | Low Threshold | Notes |
|----------|---------------|---------------|-------|
| E1 Semantic | 0.75 | 0.30 | Standard dense similarity |
| E2 Temporal Recent | 0.70 | 0.30 | Post-retrieval only |
| E3 Temporal Periodic | 0.70 | 0.30 | Post-retrieval only |
| E4 Temporal Positional | 0.70 | 0.30 | Post-retrieval only |
| E5 Causal | 0.70 | 0.25 | Lower low threshold (causal harder to match) |
| E6 Sparse | 0.60 | 0.20 | Lower thresholds (exact keyword harder) |
| E7 Code | 0.80 | 0.35 | Higher thresholds (code similarity stricter) |
| E8 Graph | 0.70 | 0.30 | Standard |
| E9 HDC | 0.70 | 0.30 | Standard |
| E10 Multimodal | 0.70 | 0.30 | Standard |
| E11 Entity | 0.70 | 0.30 | Standard |
| E12 Late Interaction | 0.70 | 0.30 | Pipeline stage only |
| E13 SPLADE | 0.60 | 0.20 | Lower thresholds (sparse expansion) |

---

## Part 6: MCP Tool Exposure

The embedder system is exposed through these MCP tools:

### Embedder-Specific Tools
| Tool | Primary Embedder(s) | Description |
|------|---------------------|-------------|
| `search_by_embedder` | Any (E1-E13) | Search from any embedder's perspective |
| `list_embedder_indexes` | All | List available HNSW/inverted indexes |
| `get_embedder_clusters` | Any | Find clusters in a specific embedder space |
| `compare_embedder_views` | All | Compare how different embedders see same query |

### Domain-Specific Tools
| Tool | Primary Embedder(s) | Description |
|------|---------------------|-------------|
| `search_graph` | E1 (default) + profile | Multi-space semantic search |
| `search_robust` | E1 + E9 (typo tolerance) | Noise-robust search |
| `search_by_intent` | E1 + E10 boost | Intent-aligned search |
| `search_by_keywords` | E6 | Exact keyword search |
| `search_code` | E7 | Code pattern search |
| `search_causes` / `search_effects` | E5 (asymmetric) | Causal chain search |
| `search_connections` | E8 (asymmetric) | Graph structure search |
| `search_by_entities` | E11 | Entity knowledge search |
| `search_recent` | E2 (post-retrieval) | Recency-boosted search |
| `search_periodic` | E3 (post-retrieval) | Periodic pattern search |
| `get_session_timeline` | E4 | Sequence-based navigation |

### Analysis Tools
| Tool | Embedder(s) | Description |
|------|-------------|-------------|
| `detect_topics` | All 13 (HDBSCAN) | Topic portfolio detection |
| `get_topic_stability` | All 13 | Weighted agreement stability |
| `get_divergence_alerts` | 7 semantic only | Topic drift detection |

---

## Part 7: Synergy and Optimization

### Synergy Matrix

The system maintains a 13x13 **SynergyMatrix** that quantifies how well each pair of embedders cooperates. Preset matrices include:
- **semantic_focused**: Boosts E1+E5, E1+E11, E1+E12 synergies
- **code_heavy**: Boosts E7+E5, E7+E8, E7+E13 synergies
- **temporal_focused**: Boosts E2+E3, E2+E4 synergies

### Quantization and Memory

| Embedder | Dimensions | Storage Type | VRAM Estimate |
|----------|-----------|--------------|---------------|
| E1 Semantic | 1024D | Dense float32 | ~2.0 GB |
| E1 Matryoshka128 | 128D | Dense float32 | (shared) |
| E2 Temporal Recent | 512D | Dense float32 | ~0.5 GB |
| E3 Temporal Periodic | 512D | Dense float32 | ~0.5 GB |
| E4 Temporal Positional | 512D | Dense float32 | ~0.5 GB |
| E5 Causal (x3 indexes) | 768D | Dense float32 | ~1.5 GB |
| E6 Sparse | ~30K sparse | Inverted index | ~0.5 GB |
| E7 Code | 1536D | Dense float32 | ~2.0 GB |
| E8 Graph | 384D | Dense float32 | ~1.0 GB |
| E9 HDC | 1024D | Dense float32 | ~1.0 GB |
| E10 Multimodal (x3 indexes) | 768D | Dense float32 | ~1.5 GB |
| E11 Entity | 768D | Dense float32 | ~1.5 GB |
| E12 Late Interaction | 128D/token | Token-level | ~0.5 GB |
| E13 SPLADE | ~30K sparse | Inverted index | ~0.5 GB |
| **Total** | | | **~12-14 GB** |

### Index Architecture

- **HNSW Indexes**: 15 total (11 embedders + 4 asymmetric variants for E5, E8, E10)
- **Inverted Indexes**: 2 (E6 sparse, E13 SPLADE)
- **Specialized Indexes**: 1 (E12 ColBERT MaxSim)
- **Total Index Count**: 18

---

## Part 8: Architectural Invariants

These rules are enforced throughout the system:

| Rule | Description | Enforced By |
|------|-------------|-------------|
| **ARCH-12** | E1 is the semantic foundation; all retrieval starts with E1 | Weight profiles, search strategies |
| **ARCH-15** | Asymmetric embedders (E5, E8, E10) have cause/effect and source/target indexes | SemanticFingerprint struct, HNSW config |
| **ARCH-17** | E10 uses multiplicative boost on E1, NOT linear blending | Intent search handler |
| **ARCH-21** | Multi-embedder fusion uses WRRF, NOT weighted sum | Retrieval pipeline |
| **ARCH-25** | Temporal (E2-E4) applied POST-RETRIEVAL only | Weight profiles (0.0), search pipeline |
| **AP-60** | Temporal embedders weight 0.0 in semantic profiles | Weight validation, category system |
| **AP-73** | E12 ColBERT is reranking ONLY (pipeline Stage 4) | Pipeline stage enforcement |
| **AP-74/75** | E13 SPLADE is Stage 1 recall ONLY | Pipeline stage enforcement |
| **AP-77** | E5 cause->effect boost = 1.2x, effect->cause = 0.8x | Asymmetric similarity engine |
| **AP-80/84** | E10 never overrides E1; when E1=0, E10 contribution=0 | Multiplicative boost clamping |
| **ARCH-09** | Topic threshold: weighted_agreement >= 2.5 | Topic detection engine |
| **ARCH-10** | Divergence uses SEMANTIC embedders only (7 of 13) | DivergenceDetector |

---

## Part 9: Performance Characteristics

| Operation | Latency | Notes |
|-----------|---------|-------|
| Query embedding (all 13 spaces) | ~500ms | GPU, async batched |
| E1-Only HNSW search | <50ms | Single index, 1M vectors |
| Multi-Space WRRF (5-8 embedders) | 50-150ms | Parallel HNSW + fusion |
| Full Pipeline (3 stages) | 150-300ms | Recall + scoring + rerank |
| Consensus computation | <10ms | 13-embedder agreement check |
| E12 MaxSim reranking | <15ms | 50 candidates, token-level |

## Part 10: Storage Architecture

### SemanticFingerprint Size

Each memory's fingerprint stores all 13 embeddings unfused (~46KB typical):

| Field | Size | Type |
|-------|------|------|
| `e1_semantic` | 4,096 bytes | 1024 x f32 |
| `e2_temporal_recent` | 2,048 bytes | 512 x f32 |
| `e3_temporal_periodic` | 2,048 bytes | 512 x f32 |
| `e4_temporal_positional` | 2,048 bytes | 512 x f32 |
| `e5_causal_as_cause` + `_as_effect` | 6,144 bytes | 2 x 768 x f32 |
| `e6_sparse` | ~940 bytes | ~235 active terms of 30,522 |
| `e7_code` | 6,144 bytes | 1536 x f32 |
| `e8_graph_as_source` + `_as_target` | 3,072 bytes | 2 x 384 x f32 |
| `e9_hdc` | 4,096 bytes | 1024 x f32 |
| `e10_multimodal_as_intent` + `_as_context` | 6,144 bytes | 2 x 768 x f32 |
| `e11_entity` | 3,072 bytes | 768 x f32 |
| `e12_late_interaction` | ~5,120 bytes | ~40 tokens x 128 x f32 |
| `e13_splade` | ~940 bytes | ~235 active terms |
| **Total per memory** | **~46 KB** | |

### Key Column Families

| CF Name | Purpose | Index Type |
|---------|---------|------------|
| `CF_FINGERPRINTS` | Full 13-embedding fingerprint storage | RocksDB KV |
| `CF_E1_MATRYOSHKA_128` | 128D truncated E1 for Stage 2 | HNSW |
| `CF_E13_SPLADE_INVERTED` | term_id -> Vec\<Uuid\> for Stage 1 recall | Inverted |
| `CF_E6_SPARSE_INVERTED` | E6 keyword inverted index | Inverted |
| `CF_E12_LATE_INTERACTION` | Token-level embeddings for MaxSim | Specialized |
| `CF_TOPIC_PROFILES` | 13D topic profile vectors (52 bytes each) | RocksDB KV |
| `CF_SYNERGY_MATRIX` | 13x13 co-occurrence patterns | RocksDB KV |

### Source Code Locations

| Component | Location |
|-----------|----------|
| SemanticFingerprint struct | `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` |
| Embedder enum (13 variants) | `crates/context-graph-core/src/teleological/embedder.rs` |
| Weight profiles (15 presets) | `crates/context-graph-core/src/weights/mod.rs` |
| Embedder categories | `crates/context-graph-core/src/embeddings/category.rs` |
| Similarity thresholds | `crates/context-graph-core/src/similarity/config.rs` |
| Multi-space similarity | `crates/context-graph-core/src/retrieval/multi_space.rs` |
| Divergence detector | `crates/context-graph-core/src/retrieval/detector.rs` |
| RRF fusion engine | `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs` |
| Multi-space search | `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs` |
| Pipeline stages | `crates/context-graph-storage/src/teleological/search/pipeline/` |
| HNSW index config | `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs` |
| MCP search tools | `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` |
| Embedder-first tools | `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` |
| Column families | `crates/context-graph-storage/src/teleological/column_families.rs` |
| Synergy matrix presets | `crates/context-graph-core/src/teleological/synergy_matrix/presets.rs` |

---

## Conclusion

The 13-embedder architecture provides the Context Graph with capabilities that no single embedding model could achieve:

1. **E1** provides the semantic foundation that everything builds upon
2. **E5, E8, E10** add directional/asymmetric understanding (cause-effect, graph structure, intent-context)
3. **E6, E13** provide keyword-level precision and recall that dense models dilute
4. **E7** understands code as code, not as prose
5. **E9** handles the messy reality of typos and noisy input
6. **E11** brings structured knowledge graph facts into the retrieval loop
7. **E12** provides final-stage precision for disambiguating close matches
8. **E2, E3, E4** add temporal intelligence as post-retrieval signals

The Weighted Reciprocal Rank Fusion system and configurable weight profiles allow the system to dynamically emphasize different embedder perspectives based on query type, while the 5-stage pipeline architecture ensures each embedder is used at the stage where it provides maximum value.
