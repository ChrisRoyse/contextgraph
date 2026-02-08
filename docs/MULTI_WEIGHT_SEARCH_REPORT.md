# Multi-Weight Search Integration Report

**Date:** 2026-02-07
**Branch:** casetrack
**Scope:** All 57 MCP tools - weight system integration analysis

## Executive Summary

The multi-weight search system is **architecturally sound and well-integrated** into the three primary search tools (`search_graph`, `get_unified_neighbors`, `search_by_intent`). These tools correctly implement weighted Reciprocal Rank Fusion (RRF) across 13 embedder spaces with 16 predefined profiles and session-scoped custom profiles.

However, **only 3 of 17 search-capable tools** expose weight customization to callers. The remaining 14 search tools use hardcoded single-embedder or fixed dual-embedder strategies. While many of these are intentionally single-perspective (by Constitution design), there are **5 tools that could meaningfully benefit** from optional weight integration without violating their design philosophy.

**Verdict:** The core weight infrastructure is excellent. Integration is optimal for the primary search path but has room for improvement in secondary search tools and the feedback loop.

---

## 1. Weight System Architecture

### 1.1 Core Data Flow

```
User Request (JSON)
    │
    ├── customWeights: {E5: 0.6, E1: 0.4}     ← Direct weights (highest priority)
    ├── weightProfile: "causal_reasoning"        ← Named profile (second priority)
    └── (neither)                                ← Default: "semantic_search"
    │
    ▼
Weight Resolution (MCP Layer)
    │
    ├── Validate embedder names (E1-E13) → FAIL-FAST on invalid
    ├── Custom profile lookup (session HashMap) → Built-in profile lookup
    ├── Apply exclude_embedders → zero out + renormalize to sum=1.0
    └── validate_weights() → range [0,1], sum ≈ 1.0 (±0.01)
    │
    ▼
TeleologicalSearchOptions
    │
    ├── custom_weights: Option<[f32; 13]>
    ├── weight_profile: Option<String>
    └── exclude_embedders: Vec<usize>
    │
    ▼
Storage Layer: search_multi_space_sync()
    │
    ├── Search E1, E5, E7, E10 HNSW indexes in parallel
    ├── Build EmbedderRanking per embedder (name, weight, ranked_docs)
    └── Weighted RRF Fusion: score = Σ(weight_i / (K + rank_i))  where K=60
    │
    ▼
Fused Results (ranked by weighted RRF score)
```

### 1.2 Weight Profile System

| Component | Implementation | Quality |
|-----------|---------------|---------|
| 16 predefined profiles | `context_graph_core::weights::WEIGHT_PROFILES` | Excellent |
| Custom profile creation | `create_weight_profile` MCP tool | Excellent |
| Session storage | `Arc<RwLock<HashMap<String, [f32; 13]>>>` | Good (volatile) |
| Validation | FAIL-FAST, range + sum checks | Excellent |
| Resolution priority | customWeights > custom profile > built-in > default | Correct |

### 1.3 The 16 Predefined Profiles

| Profile | Primary Embedder(s) | Use Case |
|---------|---------------------|----------|
| `semantic_search` | E1 (0.33) | General queries (DEFAULT) |
| `causal_reasoning` | E5 (0.45) | "Why" / cause-effect questions |
| `code_search` | E7 (0.40) | Programming/code queries |
| `fact_checking` | E11 (0.40) | Entity/factual queries |
| `intent_search` | E1 (0.55) + E10 | Goal-oriented queries |
| `intent_enhanced` | E1 (0.55) + E10 aggressive | Strong intent alignment |
| `graph_reasoning` | E8 (0.40) | Graph connectivity queries |
| `temporal_navigation` | E2/E3/E4 (0.22 each) | Time-based queries |
| `sequence_navigation` | E4 (0.55) | Conversation ordering |
| `conversation_history` | E4 (0.35) + E1 (0.30) | Contextual recall |
| `category_weighted` | Balanced by category | Constitutional weights |
| `typo_tolerant` | E9 (0.15) | Noisy/misspelled queries |
| `pipeline_stage1_recall` | E13 (0.25) + E6 (0.25) | Sparse broad retrieval |
| `pipeline_stage2_scoring` | E1 (0.50) | Dense re-scoring |
| `pipeline_full` | E1 (0.40) | Complete pipeline |
| `balanced` | 0.077 each | Testing/comparison |

**Design constraint (AP-71):** Temporal embedders (E2-E4) = 0.0 in all semantic profiles except `temporal_navigation` and `conversation_history`.

---

## 2. Tool-by-Tool Weight Integration Matrix

### 2.1 Full Weight Support (3 tools)

| Tool | customWeights | weightProfile | excludeEmbedders | includeEmbedderBreakdown |
|------|:---:|:---:|:---:|:---:|
| `search_graph` | Yes | Yes (16 + custom) | Yes | Yes |
| `get_unified_neighbors` | Yes | Yes (16 + custom) | Yes | Yes |
| `search_by_intent` | No | Yes (16 + custom) | No | No |

These three are the primary multi-embedder fusion search paths. `search_graph` and `get_unified_neighbors` have **full** weight support. `search_by_intent` has partial support (profile name only, no custom weights or excluders) because ARCH-12 enforces E1+E10 as the required combination.

### 2.2 Single-Embedder Tools - By Design (8 tools)

| Tool | Embedder(s) Used | Reason No Weights |
|------|-----------------|-------------------|
| `search_by_embedder` | User-selected single E | Escape hatch for expert single-perspective |
| `search_by_keywords` | E6 + E1 blend | Fixed E6/E1 ratio via `blendWithSemantic` |
| `search_code` | E7 + E1 blend | Fixed E7/E1 ratio via `blendWithSemantic` |
| `search_robust` | E9 + E1 | Blind-spot detection requires isolated E9 |
| `search_causes` | E5 asymmetric | Causal asymmetric search requires pure E5 |
| `search_effects` | E5 asymmetric | Causal asymmetric search requires pure E5 |
| `search_by_entities` | E11 + E1 | Entity-specific search requires E11 focus |
| `search_connections` | E8 + E1 | Graph structure search requires E8 focus |

**Design principle:** These tools each expose a specific embedder's unique perspective per Constitution v6.3. Mixing weights would dilute their specialized purpose.

### 2.3 Diagnostic/Introspection Tools - No Search Weights (6 tools)

| Tool | Purpose | Reason No Weights |
|------|---------|-------------------|
| `search_cross_embedder_anomalies` | Blind-spot detection between 2 embedders | Must see unbiased individual perspectives |
| `compare_embedder_views` | Parallel independent embedder comparison | Independence required for meaningful comparison |
| `get_memory_fingerprint` | Raw per-embedder vector introspection | No search involved |
| `list_embedder_indexes` | Index metadata inspection | No search involved |
| `get_embedder_clusters` | Cluster analysis | No search involved |
| `search_by_embedder` (also listed above) | Single-embedder search | Expert single-perspective |

### 2.4 Temporal/Session Tools (3 tools)

| Tool | Strategy | Reason No Weights |
|------|----------|-------------------|
| `search_recent` | E2 recency-sorted | Temporal sorting, not similarity ranking |
| `search_periodic` | E3 periodicity patterns | Time-of-day matching, not similarity |
| `get_session_timeline` | E4 sequence ordering | Conversation position, not similarity |

### 2.5 Weight-Aware Meta Tool (1 tool)

| Tool | How It Uses Weights |
|------|-------------------|
| `adaptive_search` | Auto-classifies query → selects optimal profile → delegates to weighted search |

---

## 3. RRF Fusion Implementation Quality

### 3.1 Correctness

The weighted RRF implementation in `fusion.rs` is **correct and robust**:

```
RRF_score(doc) = Σ (weight_i / (rank_i + 1 + K))     where K = 60.0
```

- Weights **multiply** RRF rank contributions (not raw similarity scores)
- Zero-weight embedders are **skipped entirely** (`if weight <= 0.0 { continue }`)
- Consensus across embedders is naturally rewarded (docs in multiple lists accumulate scores)
- Robust to score distribution differences between embedders (rank-based, not score-based)

### 3.2 Effective Embedder Coverage in Multi-Space Search

The `MultiSpace` strategy currently searches **4 of 13 embedders**: E1, E5, E7, E10.

| Embedder | Searched in MultiSpace | Has HNSW Index | Notes |
|----------|:---:|:---:|-------|
| E1 (semantic) | Yes | Yes | Foundation |
| E2 (recency) | No | No | Temporal-only (post-retrieval) |
| E3 (periodicity) | No | No | Temporal-only |
| E4 (ordering) | No | No | Temporal-only |
| E5 (causality) | Yes | Yes | Asymmetric |
| E6 (keywords) | No | Sparse | Used in pipeline stage1 |
| E7 (code) | Yes | Yes | |
| E8 (graph) | No | Yes | Not in default multi-space |
| E9 (robustness) | No | No | HDC-based |
| E10 (intent) | Yes | Yes | Asymmetric |
| E11 (factuality) | No | Yes | Not in default multi-space |
| E12 (precision) | No | ColBERT | Late interaction only |
| E13 (SPLADE) | No | Sparse | Pipeline stage1 only |

**Observation:** Only 4/13 embedders participate in the default `MultiSpace` RRF fusion. E8 (graph), E11 (factuality), and E9 (robustness) have indexes but aren't included in the fusion pipeline. Weight profiles that assign non-zero weights to these embedders will have **no effect** unless the search strategy explicitly includes them.

This means: **of the 13 weights in a profile, only 4 actually influence multi-space RRF results** (E1, E5, E7, E10). The remaining weights are effectively ignored in the current `MultiSpace` strategy.

---

## 4. Integration Gaps

### 4.1 GAP-1: Weight Profiles Don't Fully Map to Search Execution (MEDIUM)

**Issue:** All 16 profiles define weights for 13 embedders, but the `MultiSpace` search strategy only queries 4 embedders (E1, E5, E7, E10). A user selecting `fact_checking` (E11=0.40) gets zero benefit from the E11 weight because E11 isn't searched.

**Impact:** User confusion. Profiles appear to offer customization over 13 dimensions but only 4 matter.

**Status:** By design (HNSW index availability), but **poorly communicated**. The profile system implies broader coverage than the search pipeline delivers.

### 4.2 GAP-2: search_by_intent Missing Full Weight Parameters (LOW)

**Issue:** `search_by_intent` accepts `weightProfile` but not `customWeights`, `excludeEmbedders`, or `includeEmbedderBreakdown`. This is inconsistent with `search_graph` and `get_unified_neighbors`.

**Impact:** Low. Intent search has specific ARCH-12 constraints (E1+E10 mandatory), but users can't fine-tune the E1/E10 ratio or exclude other embedders.

### 4.3 GAP-3: No Weight Profile Lifecycle Management (LOW)

**Issue:** Custom profiles are session-scoped with no persistence, update, delete, or list operations.

**Missing tools:**
- `list_weight_profiles` - show all custom + built-in profiles
- `update_weight_profile` - modify an existing custom profile
- `delete_weight_profile` - remove a custom profile

**Impact:** Low. Session-scoped is appropriate for the current use case. Profiles are lightweight enough to recreate.

### 4.4 GAP-4: No Feedback Loop from Adaptive Search (LOW)

**Issue:** `adaptive_search` classifies queries via static keyword heuristics and selects profiles, but never learns from results. No mechanism to track whether profile selections were effective.

**Impact:** Low currently. The keyword heuristics are reasonable for common query patterns. Would become more impactful at scale or with domain-specific queries.

### 4.5 GAP-5: Diagnostic Tools Could Optionally Show Weighted Context (LOW)

**Issue:** `compare_embedder_views` and `search_cross_embedder_anomalies` are intentionally unweighted for diagnostic clarity, but could optionally show "how would this result rank under profile X?" as supplementary information.

**Impact:** Low. Nice-to-have for power users.

---

## 5. Opportunities for Improvement

### 5.1 Priority 1: Document the 4-Embedder Limitation (Recommended)

The most impactful improvement is **transparency**, not code changes. Add documentation or MCP response metadata clarifying:
- Which embedders are active in multi-space search (E1, E5, E7, E10)
- Which profile weights are effective vs. ignored
- Why temporal (E2-E4) and sparse (E6, E12, E13) embedders are excluded from RRF

This could be:
- A `searchStrategy` field in search responses showing active embedders
- Profile documentation noting "effective weights" vs. "full weights"
- An `activeEmbedders` field in `adaptive_search` classification response

### 5.2 Priority 2: Expand MultiSpace to Include E8 and E11 (Optional)

E8 (graph/connectivity) and E11 (factuality/entities) both have HNSW indexes and would meaningfully expand the fusion pipeline from 4 to 6 embedders. This would make `graph_reasoning` (E8=0.40) and `fact_checking` (E11=0.40) profiles actually effective in weighted search.

**Trade-off:** +50% more HNSW searches per query. Latency impact depends on index size.

### 5.3 Priority 3: Unify Blend Parameters with Weight System

Several single-embedder tools (search_by_keywords, search_code, search_by_entities) use a `blendWithSemantic` parameter for dual-embedder scoring. These are effectively 2-embedder weight profiles:

```
search_by_keywords: E6_weight = blendWithSemantic, E1_weight = 1 - blendWithSemantic
search_code:        E7_weight = blendWithSemantic, E1_weight = 1 - blendWithSemantic
search_by_entities: E11_weight = blendWithSemantic, E1_weight = 1 - blendWithSemantic
```

These could optionally accept a `weightProfile` for multi-embedder fusion as an alternative to the simple blend, giving power users access to the full weight system from any search entry point.

**Trade-off:** Increased API complexity. The simple blend is easier to understand for most users.

### 5.4 Priority 4: Profile Effectiveness Metrics

Add optional response metadata showing how weights affected results:

```json
{
  "effectiveProfile": "causal_reasoning",
  "activeWeights": {"E1": 0.20, "E5": 0.45, "E7": 0.10, "E10": 0.05},
  "ignoredWeights": {"E6": 0.05, "E8": 0.10, "E9": 0.0, "E11": 0.05},
  "weightUtilization": 0.80
}
```

This would make the 4-embedder limitation transparent per-query.

---

## 6. Summary Assessment

| Dimension | Rating | Notes |
|-----------|--------|-------|
| **Weight infrastructure** | Excellent | 16 profiles, custom profiles, validation, FAIL-FAST |
| **RRF fusion correctness** | Excellent | Rank-based weighted fusion, consensus rewarded |
| **Primary search integration** | Excellent | search_graph and get_unified_neighbors fully integrated |
| **Secondary tool integration** | Good | Intentionally single-embedder by design |
| **Transparency** | Needs improvement | 4/13 embedder limitation not communicated to callers |
| **Profile effectiveness** | Needs improvement | 9/13 weights in profiles are ignored in default search |
| **Lifecycle management** | Adequate | Session-scoped, no update/delete/list |
| **Feedback/learning** | Not present | Static keyword classification, no learning loop |

### Overall Verdict

The multi-weight search system is **well-architected and correctly implemented** in its core path. The weight → RRF fusion pipeline works exactly as designed. The main area for improvement is not adding more weight parameters to more tools, but rather **closing the gap between what weight profiles promise (13-dimensional control) and what the search pipeline delivers (4-dimensional fusion)**. Making this transparent to callers is the highest-value improvement.

The single-embedder tools are correctly isolated by design - forcing weights into them would violate the Constitution's principle that each embedder provides a unique perspective. The diagnostic tools are similarly correct in avoiding weights.

---

## Appendix: File References

| Component | File | Key Lines |
|-----------|------|-----------|
| Weight profiles (16) | `crates/context-graph-core/src/weights/mod.rs` | WEIGHT_PROFILES array |
| Weight validation | `crates/context-graph-mcp/src/weights.rs` | validate_weights() |
| Profile resolution | `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` | 1018-1062 |
| Custom profile creation | `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` | 901-981 |
| RRF fusion | `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs` | weighted_rrf() |
| Multi-space search | `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs` | search_multi_space_sync() |
| Adaptive search | `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` | 1116-1234 |
| Search options | `crates/context-graph-core/src/traits/options.rs` | TeleologicalSearchOptions |
| Anomaly detection | `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` | 983-1114 |
