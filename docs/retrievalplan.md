# Retrieval System Plan: Optimal Use of 13 Embedder Insights

## Executive Summary

The current retrieval system has **strong foundations** but **underutilizes** several embedders' unique perspectives. This plan outlines how to fully leverage all 13 embedders so that **combined insights produce superior answers**.

**Core Principle**: Each embedder finds what OTHERS MISS. The retrieval system must:
1. Query each embedder for its unique perspective
2. Combine insights intelligently (not just score fusion)
3. Surface results that NO SINGLE embedder would find alone

---

## Current State Assessment

### Embedder Integration Status

| Embedder | Purpose | Current Status | Utilization |
|----------|---------|----------------|-------------|
| **E1** | Semantic similarity | ✅ Complete | 100% - Foundation |
| **E2** | Recency | ⚠️ Partial | 30% - Post-retrieval only |
| **E3** | Periodicity | ⚠️ Partial | 20% - Stubbed |
| **E4** | Sequence | ✅ Complete | 90% - Via sequence tools |
| **E5** | Causal | ✅ Complete | 95% - Asymmetric search |
| **E6** | Keywords | ❌ Incomplete | 20% - No dedicated tool |
| **E7** | Code | ⚠️ Partial | 40% - No query detection |
| **E8** | Graph | ✅ Complete | 95% - Via graph tools |
| **E9** | Format/HDC | ⚠️ Partial | 30% - Projected only |
| **E10** | Intent | ✅ Complete | 95% - Multiplicative boost |
| **E11** | Entity | ✅ Mostly | 80% - TransE partial |
| **E12** | ColBERT | ⚠️ Partial | 40% - Stage 5 missing |
| **E13** | SPLADE | ❌ Incomplete | 20% - Stage 1 missing |

### What's Working Well
- E1 semantic foundation with multi-space RRF
- E5 causal asymmetric search (cause→effect direction)
- E8 graph connectivity tools
- E10 intent multiplicative boost (ARCH-28)
- E4 sequence tools (conversation context)

### Critical Gaps
1. **E6/E13 sparse retrieval** - No inverted index for keyword/expansion search
2. **E7 code query detection** - Treats code queries as natural language
3. **E12 Stage 5 reranking** - MaxSim exists but not integrated
4. **E11 entity inference** - TransE prediction stubbed

---

## Optimal Retrieval Architecture

### The 13-Perspectives Retrieval Model

```
Query: "What databases work with Rust?"

PERSPECTIVE GATHERING (Parallel):
┌─────────────────────────────────────────────────────────────────┐
│ E1 (Semantic)  → "database", "Rust" semantically similar        │
│ E11 (Entity)   → "Diesel" (knows Diesel IS a database ORM)      │
│ E7 (Code)      → code using sqlx, diesel, sea-orm crates        │
│ E5 (Causal)    → "why we chose PostgreSQL" discussions          │
│ E6 (Keywords)  → exact "database" OR "db" OR "orm" matches      │
│ E13 (SPLADE)   → expands to "sql", "persistence", "storage"     │
│ E8 (Graph)     → what IMPORTS diesel, what diesel CONNECTS to   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
INSIGHT COMBINATION (Not just score fusion):
┌─────────────────────────────────────────────────────────────────┐
│ 1. E1 baseline: memories semantically about databases/Rust      │
│ 2. E11 ADDS: Diesel memory (E1 missed - no "database" word)     │
│ 3. E7 ADDS: actual code implementations (E1 found docs only)    │
│ 4. E5 ADDS: WHY decisions (E1 found WHAT, not WHY)              │
│ 5. Combined via Weighted RRF preserving each contribution       │
└─────────────────────────────────────────────────────────────────┘
                              ↓
RESULT: Superior answer drawing from ALL perspectives
```

### Key Insight: Additive Value Model

Each embedder must contribute **UNIQUE results** that others miss:

| Embedder | Unique Contribution | E1 Blind Spot Filled |
|----------|---------------------|----------------------|
| E5 | Causal chains | E1 loses cause→effect direction |
| E6 | Exact keyword matches | E1 dilutes specific terms |
| E7 | Code patterns | E1 treats code as NL |
| E8 | Graph structure | E1 can't see X→Y relationships |
| E9 | Format patterns | E1 sensitive to paraphrasing |
| E10 | Intent alignment | E1 matches words, not goals |
| E11 | Entity knowledge | E1 has no "Diesel=ORM" knowledge |
| E12 | Exact phrase matches | E1 global embedding loses tokens |
| E13 | Term expansion | E1 doesn't expand vocabulary |

---

## Implementation Plan

### Phase 1: Core Pipeline Fixes (Critical)

#### 1.1 E13 SPLADE Stage 1 Recall

**Problem**: Stage 1 requires inverted index for sparse recall
**Solution**: Implement sparse inverted index in storage layer

```rust
// New: retrieval/sparse_index.rs
pub struct SparseInvertedIndex {
    // term_id -> Vec<(memory_id, weight)>
    postings: HashMap<u32, Vec<(Uuid, f32)>>,
}

impl SparseInvertedIndex {
    pub fn recall(&self, query_terms: &SparseVector, top_k: usize) -> Vec<(Uuid, f32)> {
        // Jaccard-weighted posting list union
    }
}
```

**Files to modify**:
- `crates/context-graph-core/src/retrieval/sparse_index.rs` (new)
- `crates/context-graph-core/src/storage/indexes.rs`
- `crates/context-graph-core/src/retrieval/pipeline/default.rs`

#### 1.2 E12 ColBERT Stage 5 Reranking

**Problem**: MaxSim function exists but Stage 5 integration missing
**Solution**: Store token embeddings and integrate reranking

```rust
// New: retrieval/reranker.rs
pub struct ColBERTReranker {
    token_store: TokenEmbeddingStore,
}

impl ColBERTReranker {
    pub fn rerank(&self, query_tokens: &[Vec<f32>], candidates: Vec<SearchResult>) -> Vec<SearchResult> {
        candidates.into_iter()
            .map(|c| {
                let doc_tokens = self.token_store.get(&c.id)?;
                let score = max_sim(&query_tokens, &doc_tokens);
                (c.id, score)
            })
            .sorted_by_score()
            .collect()
    }
}
```

**Files to modify**:
- `crates/context-graph-core/src/retrieval/reranker.rs` (new)
- `crates/context-graph-core/src/storage/token_store.rs` (new)
- `crates/context-graph-core/src/retrieval/pipeline/default.rs`

#### 1.3 Query Type Analyzer

**Problem**: Query understanding is in MCP handlers, not core
**Solution**: Create unified query analyzer in retrieval layer

```rust
// New: retrieval/query_analyzer.rs
pub enum QueryType {
    General,           // Use E1Only
    Causal,            // Boost E5, asymmetric
    Code,              // Boost E7, Code2Code vs NL2Code
    Entity,            // Boost E11, entity extraction
    Intent,            // Boost E10, multiplicative
    Keyword,           // Boost E6/E13, exact match
    Graph,             // Boost E8, structural
}

pub fn analyze_query(text: &str) -> QueryAnalysis {
    QueryAnalysis {
        query_type: detect_type(text),
        entities: extract_entities(text),
        keywords: extract_keywords(text),
        causal_direction: detect_causal_direction(text),
        code_language: detect_code_language(text),
    }
}
```

**Detection heuristics**:
- Causal: "why", "because", "caused", "leads to", "results in"
- Code: backticks, function names, `fn`, `def`, `class`, file extensions
- Entity: capitalized words, quoted terms, known entity patterns
- Intent: "goal", "purpose", "trying to", "want to", "need to"
- Keyword: quoted exact phrases, technical jargon, acronyms

---

### Phase 2: Missing Embedder Tools (High Priority)

#### 2.1 E6 Keyword Search Tool

```rust
// New: handlers/tools/keyword_tools.rs
pub async fn search_by_keywords(params: KeywordSearchParams) -> Vec<SearchResult> {
    // 1. Extract keywords from query
    let keywords = extract_keywords(&params.query);

    // 2. Search E6 sparse index for exact matches
    let e6_results = sparse_index.search_exact(keywords);

    // 3. Optionally expand with E13 SPLADE
    let e13_results = if params.expand {
        splade_index.search_expanded(keywords)
    } else { vec![] };

    // 4. Combine with E1 baseline via RRF
    let e1_results = semantic_search(&params.query);

    rrf_combine(vec![e1_results, e6_results, e13_results])
}
```

**MCP Tool Definition**:
```json
{
  "name": "search_by_keywords",
  "description": "Find memories with exact keyword matches. E6 finds specific terms E1 dilutes.",
  "parameters": {
    "keywords": "array of exact terms to match",
    "expand": "also search E13 SPLADE expansions (default: false)",
    "includeE1": "also include E1 semantic results (default: true)"
  }
}
```

#### 2.2 E7 Code Search Tool

```rust
// New: handlers/tools/code_tools.rs
pub async fn search_code(params: CodeSearchParams) -> Vec<SearchResult> {
    // 1. Detect query type
    let query_type = if looks_like_code(&params.query) {
        QueryType::Code2Code  // Query IS code
    } else {
        QueryType::NL2Code    // Query is about code
    };

    // 2. Encode query appropriately
    let query_embedding = match query_type {
        Code2Code => encode_as_code(&params.query),
        NL2Code => encode_as_nl_about_code(&params.query),
    };

    // 3. Search E7 code space
    let e7_results = code_index.search(&query_embedding);

    // 4. Combine with E1 for context
    let e1_results = semantic_search(&params.query);

    rrf_combine(vec![e7_results, e1_results])
}
```

**MCP Tool Definition**:
```json
{
  "name": "search_code",
  "description": "Find code implementations. E7 finds structural code patterns E1 treats as text.",
  "parameters": {
    "query": "code snippet or description of code to find",
    "language": "optional language filter (rust, python, etc.)",
    "type": "function, class, module, or any"
  }
}
```

#### 2.3 E9 Format Search Tool

```rust
// New: handlers/tools/format_tools.rs
pub async fn search_by_format(params: FormatSearchParams) -> Vec<SearchResult> {
    // 1. Encode format pattern with E9 HDC
    let format_embedding = hdc_encode_format(&params.format_pattern);

    // 2. Search E9 using Hamming distance (native) or cosine (projected)
    let e9_results = if params.use_native_hdc {
        hdc_index.search_hamming(&format_embedding)
    } else {
        hdc_index.search_cosine_projected(&format_embedding)
    };

    // 3. Combine with E1 for semantic relevance
    let e1_results = semantic_search(&params.query);

    rrf_combine(vec![e9_results, e1_results])
}
```

---

### Phase 3: Enhanced Insight Combination (Medium Priority)

#### 3.1 Additive RRF (New Algorithm)

Standard RRF combines by rank. We need **Additive RRF** that ensures each embedder's unique contributions are preserved:

```rust
pub fn additive_rrf(
    embedder_results: HashMap<Embedder, Vec<SearchResult>>,
    k: f32,
) -> Vec<CombinedResult> {
    let mut combined: HashMap<Uuid, CombinedResult> = HashMap::new();

    for (embedder, results) in embedder_results {
        for (rank, result) in results.iter().enumerate() {
            let entry = combined.entry(result.id).or_default();

            // Track which embedders found this result
            entry.found_by.insert(embedder);

            // RRF score contribution
            entry.rrf_score += 1.0 / (k + rank as f32 + 1.0);

            // Track if this is a UNIQUE find (only this embedder found it)
            if entry.found_by.len() == 1 {
                entry.unique_contribution = Some(embedder);
            }
        }
    }

    // Boost results that are unique contributions (found by only one embedder)
    // These are the "blind spot" discoveries
    for result in combined.values_mut() {
        if result.unique_contribution.is_some() {
            result.rrf_score *= 1.1; // 10% boost for unique discoveries
        }
    }

    combined.into_values().sorted_by_score().collect()
}
```

#### 3.2 Insight Annotation

Annotate results with which embedder(s) contributed them:

```rust
pub struct CombinedResult {
    pub id: Uuid,
    pub score: f32,
    pub found_by: HashSet<Embedder>,
    pub primary_embedder: Embedder,      // Highest contribution
    pub unique_contribution: bool,        // Only one embedder found this
    pub insight_annotation: String,       // Human-readable
}

// Example annotations:
// - "Found by E11 (entity): Diesel is a database ORM"
// - "Found by E5 (causal): This bug caused the crash"
// - "Found by E7 (code): Implementation of search function"
// - "Found by E1 + E10: Semantically similar with aligned intent"
```

#### 3.3 Perspective Coverage Score

Track how well the results represent all relevant perspectives:

```rust
pub struct PerspectiveCoverage {
    pub embedders_contributing: HashSet<Embedder>,
    pub coverage_score: f32,  // 0.0 to 1.0
    pub missing_perspectives: Vec<Embedder>,
}

pub fn compute_perspective_coverage(results: &[CombinedResult]) -> PerspectiveCoverage {
    let contributing: HashSet<_> = results.iter()
        .flat_map(|r| r.found_by.iter())
        .collect();

    let expected = vec![E1, E5, E7, E8, E10, E11]; // Semantic embedders
    let missing: Vec<_> = expected.iter()
        .filter(|e| !contributing.contains(e))
        .collect();

    PerspectiveCoverage {
        embedders_contributing: contributing,
        coverage_score: contributing.len() as f32 / expected.len() as f32,
        missing_perspectives: missing,
    }
}
```

---

### Phase 4: Temporal Integration (Medium Priority)

#### 4.1 Post-Retrieval Temporal Boost

Move temporal boosting from MCP handlers to retrieval core:

```rust
// New: retrieval/temporal_boost.rs
pub struct TemporalBooster {
    decay_function: DecayFunction,
    scale: TemporalScale,
}

impl TemporalBooster {
    pub fn apply_boost(&self, results: Vec<SearchResult>) -> Vec<SearchResult> {
        results.into_iter().map(|r| {
            let age = now() - r.created_at;
            let recency_boost = self.decay_function.compute(age, self.scale);

            SearchResult {
                score: r.score * (1.0 + recency_boost * 0.2), // Max 20% boost
                ..r
            }
        }).collect()
    }
}

pub enum DecayFunction {
    Linear,      // score = max(0, 1 - age/max_age)
    Exponential, // score = exp(-age * 0.693 / half_life)
    Step,        // Fresh=1.0, Recent=0.8, Today=0.5, Older=0.1
}

pub enum TemporalScale {
    Micro,    // 5 min half-life
    Meso,     // 1 hour
    Macro,    // 1 day
    Long,     // 1 week
    Archival, // 30 days
}
```

#### 4.2 E3 Periodic Pattern Matching

```rust
// New: retrieval/periodic_boost.rs
pub fn apply_periodic_boost(
    results: Vec<SearchResult>,
    target_hour: Option<u32>,
    target_day: Option<u32>,
) -> Vec<SearchResult> {
    results.into_iter().map(|r| {
        let memory_hour = r.created_at.hour();
        let memory_day = r.created_at.weekday();

        let hour_match = target_hour.map(|h| (h == memory_hour) as f32 * 0.1);
        let day_match = target_day.map(|d| (d == memory_day) as f32 * 0.1);

        let boost = hour_match.unwrap_or(0.0) + day_match.unwrap_or(0.0);

        SearchResult {
            score: r.score * (1.0 + boost),
            ..r
        }
    }).collect()
}
```

---

### Phase 5: E11 Entity Inference (Lower Priority)

#### 5.1 TransE Tail Prediction

```rust
// Implement: t̂ = h + r
pub fn predict_tail(head: &EntityEmbedding, relation: &RelationEmbedding) -> EntityEmbedding {
    head.add(relation) // Vector addition in TransE space
}

// Find entities similar to predicted tail
pub fn find_related_entities(
    head: &str,
    relation: &str,
    top_k: usize,
) -> Vec<Entity> {
    let head_emb = entity_index.get(head)?;
    let rel_emb = relation_index.get(relation)?;

    let predicted_tail = predict_tail(&head_emb, &rel_emb);

    entity_index.nearest_neighbors(&predicted_tail, top_k)
}
```

#### 5.2 Entity Linking Enhancement

```rust
// Link mentions to known entities
pub fn link_entities(text: &str) -> Vec<LinkedEntity> {
    let mentions = extract_mentions(text);

    mentions.into_iter().map(|mention| {
        // Find best matching known entity
        let candidates = entity_index.search(&mention.text, 10);
        let best = candidates.first()?;

        LinkedEntity {
            mention: mention.text,
            entity_id: best.id,
            confidence: best.score,
            entity_type: best.entity_type,
        }
    }).collect()
}
```

---

## Optimal Retrieval Flow (Final Architecture)

```
                        QUERY INPUT
                             │
                             ▼
                    ┌─────────────────┐
                    │ Query Analyzer  │
                    │ - Type detection│
                    │ - Entity extract│
                    │ - Keyword extract│
                    └────────┬────────┘
                             │
           ┌─────────────────┼─────────────────┐
           ▼                 ▼                 ▼
    ┌────────────┐   ┌────────────┐    ┌────────────┐
    │   Stage 1  │   │   Stage 1  │    │   Stage 1  │
    │ E13 SPLADE │   │ E6 Keyword │    │ E11 Entity │
    │   Recall   │   │   Recall   │    │   Recall   │
    └─────┬──────┘   └─────┬──────┘    └─────┬──────┘
          │                │                  │
          └────────────────┼──────────────────┘
                           ▼
                    ┌─────────────────┐
                    │    Stage 2      │
                    │ E1 Matryoshka   │
                    │ Dense Filter    │
                    └────────┬────────┘
                             │
                             ▼
    ┌──────────────────────────────────────────────────┐
    │                    Stage 3                        │
    │           Multi-Space RRF Fusion                  │
    │  ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┐    │
    │  │ E1  │ E5  │ E7  │ E8  │ E10 │ E11 │ ... │    │
    │  └──┬──┴──┬──┴──┬──┴──┬──┴──┬──┴──┬──┴──┬──┘    │
    │     └─────┴─────┴─────┴─────┴─────┴─────┘        │
    │                    Additive RRF                   │
    └────────────────────────┬─────────────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │    Stage 4      │
                    │ Topic Alignment │
                    │ (weighted_agr   │
                    │    >= 2.5)      │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │    Stage 5      │
                    │ E12 ColBERT     │
                    │ MaxSim Rerank   │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  Post-Retrieval │
                    │ E2 Recency Boost│
                    │ E3 Periodic Boost│
                    │ E4 Sequence Boost│
                    └────────┬────────┘
                             │
                             ▼
                      FINAL RESULTS
                   (with insight annotations)
```

---

## Implementation Priority Matrix

| Task | Phase | Effort | Impact | Priority |
|------|-------|--------|--------|----------|
| E13 SPLADE inverted index | 1 | High | High | **P0** |
| E12 ColBERT Stage 5 | 1 | High | High | **P0** |
| Query type analyzer | 1 | Medium | High | **P0** |
| E6 keyword search tool | 2 | Medium | Medium | **P1** |
| E7 code search tool | 2 | Medium | Medium | **P1** |
| Additive RRF algorithm | 3 | Low | Medium | **P1** |
| Insight annotation | 3 | Low | Medium | **P2** |
| E2 temporal boost in core | 4 | Low | Low | **P2** |
| E3 periodic matching | 4 | Low | Low | **P2** |
| E11 TransE prediction | 5 | Medium | Low | **P3** |
| E9 format tool | 2 | Low | Low | **P3** |

---

## Success Metrics

### Retrieval Quality
- **Perspective Coverage**: >80% of queries use 4+ embedder perspectives
- **Unique Discoveries**: >20% of results come from non-E1 embedders alone
- **Recall Improvement**: 15% better recall on entity/code/causal queries

### Performance
- **Stage 1 Latency**: <5ms for SPLADE recall
- **Full Pipeline Latency**: <100ms for 1M memories
- **Stage 5 Rerank**: <20ms for top-100 candidates

### Embedder Utilization
- All 13 embedders have dedicated search paths
- Each embedder contributes unique results in >30% of queries
- No embedder contributes <10% (indicates it's not differentiated enough)

---

## Files to Create/Modify

### New Files
```
crates/context-graph-core/src/retrieval/
├── sparse_index.rs      # E13 SPLADE inverted index
├── reranker.rs          # E12 ColBERT reranking
├── query_analyzer.rs    # Query type detection
├── temporal_boost.rs    # E2/E3 post-retrieval
└── insight_annotation.rs # Result annotations

crates/context-graph-mcp/src/handlers/tools/
├── keyword_tools.rs     # E6 search
├── code_tools.rs        # E7 search
└── format_tools.rs      # E9 search
```

### Modified Files
```
crates/context-graph-core/src/retrieval/
├── aggregation.rs       # Add Additive RRF
├── pipeline/default.rs  # Integrate all stages
└── multi_space.rs       # Perspective coverage

crates/context-graph-core/src/storage/
├── indexes.rs           # Add sparse/token stores
└── mod.rs               # Export new stores
```

---

## Conclusion

The current retrieval system has **75% of the infrastructure** in place but **underutilizes** several embedders' unique perspectives. The key improvements needed are:

1. **Stage 1/5 implementation** - SPLADE recall and ColBERT rerank
2. **Query understanding in core** - Not just MCP handlers
3. **Dedicated tools for E6/E7/E9** - Surface their unique contributions
4. **Additive RRF** - Preserve unique discoveries from each embedder
5. **Insight annotations** - Show users which perspectives contributed

When complete, the system will truly leverage **13 unique perspectives** to produce answers **better than any single embedder alone**.
