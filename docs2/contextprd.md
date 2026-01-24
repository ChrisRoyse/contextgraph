# Context Graph PRD v6.1 (13-Perspectives Multi-Space System)

**Platform**: Claude Code CLI | **Core Insight**: 13 embedders = 13 unique perspectives on every memory

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

### 2.2 Technical Specs

| ID | Dim | Distance | Special Notes |
|----|-----|----------|---------------|
| E1 | 1024 | Cosine | Matryoshka (truncatable) |
| E2-E4 | 512 | Cosine | Never in similarity fusion |
| E5 | 768 | Asymmetric KNN | Direction matters (cause→effect 1.2x) |
| E6 | ~30K sparse | Jaccard | 5% active dimensions |
| E7 | 1536 | Cosine | AST-aware |
| E8, E11 | 384 | TransE | ||h + r - t|| |
| E9 | 1024 | Hamming | HDC (10K→1024) |
| E10 | 768 | Cosine | Multiplicative boost on E1 |
| E12 | 128D/token | MaxSim | Reranking ONLY |
| E13 | ~30K sparse | Jaccard | Stage 1 recall ONLY |

---

## 3. RETRIEVAL PIPELINE

### 3.1 How Perspectives Combine

```
Query → E13 sparse recall (10K) → E1 dense ANN (1K) → RRF fusion (100) → Topic filter (50) → E12 rerank (10)
                ↓                       ↓                    ↓
        "fast" finds "quick"    Semantic core       E5,E7,E10,E11 contribute
```

**Strategy Selection**:
| Strategy | When to Use | Pipeline |
|----------|-------------|----------|
| E1Only | Simple semantic queries | E1 only |
| MultiSpace | E1 blind spots matter | E1 + enhancers via RRF |
| Pipeline | Maximum precision | E13 → E1 → E12 |

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

## 7. HOOK INTEGRATION

Native Claude Code hooks via `.claude/settings.json`:

| Hook | Action | Budget |
|------|--------|--------|
| SessionStart | Load portfolio, warm indexes | 5000ms |
| UserPromptSubmit | Embed → search → inject context | 2000ms |
| PreToolUse | Inject brief relevant context | 500ms |
| PostToolUse | Capture + embed as HookDescription | 3000ms |
| Stop | Capture response summary | 3000ms |
| SessionEnd | Persist, cluster, consolidate | 30000ms |

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

---

## 9. PERFORMANCE BUDGETS

| Operation | Target | Notes |
|-----------|--------|-------|
| All 13 embed | <1000ms | Sequential on single GPU |
| Per-space HNSW | <5ms | FAISS lookup |
| inject_context P95 | <2000ms | Full pipeline |
| store_memory P95 | <2500ms | Embed + store + index |
| Any tool P99 | <3000ms | Worst case |
| Topic detection | <100ms | HDBSCAN batch |

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

| Rule | Description |
|------|-------------|
| ARCH-01 | TeleologicalArray is atomic (all 13 or nothing) |
| ARCH-02 | Apples-to-apples only (E1↔E1, never E1↔E5) |
| ARCH-04 | Temporal (E2-E4) NEVER count toward topics |
| ARCH-12 | E1 is foundation - all retrieval starts with E1 |
| ARCH-17 | Strong E1 (>0.8): enhancers refine. Weak E1 (<0.4): enhancers broaden |
| ARCH-21 | Multi-space fusion uses Weighted RRF, not weighted sum |
| ARCH-25 | Temporal boosts POST-retrieval only |

**Forbidden**:
- Cross-embedder comparison (E1↔E5)
- Partial TeleologicalArray
- Temporal in similarity fusion
- E12 for initial retrieval (rerank only)
- E13 for final ranking (recall only)
- Simple weighted sum (use RRF)
