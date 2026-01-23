# Benchmark Optimization Implementation Plan (Revised)

**Generated:** 2026-01-22
**Revised:** 2026-01-22
**Status:** Pending Implementation

## Architectural Principle

> **E1 (Semantic) is the PRIMARY embedder. E2-E13 are SUPPORTING embedders that add specialized intelligence on top of E1, not compete with it.**

The goal is to maximize the UNIQUE VALUE each specialized embedder adds to the overall fingerprint system. E5 (Causal) should add directional causal intelligence that E1 cannot provide, not dilute E1's semantic strength.

---

## Executive Summary

| Task | Priority | Impact | Approach |
|------|----------|--------|----------|
| 1. Remove intervention overlap | HIGH | +15.9% E5 accuracy | Simplify formula |
| 2. Add scientific causal patterns | HIGH | +10-50% detection rate | Expand pattern lists |
| 3. Verify auto-profile switching | HIGH | Ensure E5 activates | Test causal detection |
| 4. Run GPU benchmarks | HIGH | Validate changes | Real embeddings |
| 5. Create E8 graph benchmark | MEDIUM | E8 validation | New binary |

**Key Change from Original Plan:** We are NOT increasing E5 weight in `semantic_search` profile. E1 remains dominant for general queries. E5's value comes through asymmetric reranking when causal queries are detected.

---

## Task 1: Remove Intervention Overlap from E5 Formula

### Problem
Intervention overlap hurts performance by -15.9% (benchmark correlation of only 0.063).

### Solution
Disable intervention overlap by setting `overlap_factor = 1.0`. This simplifies the asymmetric similarity formula to:

```
sim = base_cos × direction_mod
```

Instead of:
```
sim = base_cos × direction_mod × (0.7 + 0.3 × intervention_overlap)
```

### Files to Modify

**`crates/context-graph-core/src/causal/asymmetric.rs`** (line ~290)

```rust
// BEFORE:
let overlap_factor = 0.7 + 0.3 * intervention_overlap;

// AFTER:
// Intervention overlap disabled per benchmark analysis (correlation 0.063, -15.9% impact)
// The overlap calculation remains available for future use if needed
let overlap_factor = 1.0;
```

### Rationale
- The intervention overlap was designed to match intervention variables between query and document
- In practice, most queries don't have explicit intervention contexts
- The 0.7 baseline dampens E5's contribution even when overlap is neutral (0.5)
- Removing this allows E5's direction modifiers (1.2x/0.8x) to have full effect

### Verification
```bash
cargo test -p context-graph-core causal
```

---

## Task 2: Add Scientific Causal Patterns

### Problem
Direction detection rate on academic text is only 16-27% due to missing scientific patterns.

### Solution
Expand cause and effect indicator patterns in two synchronized files.

### Files to Modify

**1. `crates/context-graph-core/src/causal/asymmetric.rs`** (lines 492-661)

Add to `cause_indicators` array:
```rust
// Scientific/Academic patterns - Mechanism understanding
"mechanism underlying", "pathways leading to", "factors influencing",
"variables affecting", "predictors of", "correlates of",
"we hypothesize that", "our hypothesis is", "posit that",
"the etiology of", "pathogenesis of", "molecular basis of",
"regulatory mechanisms", "signaling cascade", "feedback loop",
"upstream regulator", "transcriptional control", "epigenetic modification",
```

Add to `effect_indicators` array:
```rust
// Scientific/Academic patterns - Outcome understanding
"phenotypic outcome", "downstream target", "end result",
"clinical manifestation", "observable effect", "measurable outcome",
"functional consequence", "biological response", "physiological change",
"statistically significant", "p-value indicates", "confidence interval",
"dose-response relationship", "therapeutic effect", "adverse outcome",
```

**2. `crates/context-graph-embeddings/src/models/pretrained/causal/marker_detection.rs`** (CAUSE_INDICATORS and EFFECT_INDICATORS)

Mirror the same patterns for token-level marker detection (keep synchronized).

### Verification
```bash
cargo test -p context-graph-core causal
cargo test -p context-graph-embeddings marker
```

---

## Task 3: Verify Auto-Profile Switching (Revised)

### Original Plan (REJECTED)
The original plan suggested increasing E5 weight from 15% to 25% in `semantic_search` profile.

### Why This Was Wrong
- E5 shouldn't compete with E1 in general semantic search
- E1 is the PRIMARY semantic embedder - diluting it hurts general retrieval
- E5's value is in CAUSAL queries, where it already has 45% weight in `causal_reasoning` profile

### Revised Approach
Verify that the auto-profile switching works correctly:

1. When a causal query is detected (via `detect_causal_query_intent`), the system should automatically switch to `causal_reasoning` profile
2. This gives E5 its 45% weight where it matters
3. E1 remains at 35% in `semantic_search` for general queries

### Verification Points

**Location:** `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` (lines 809-811)

```rust
// This auto-switching should trigger for causal queries:
(None, CausalDirection::Cause | CausalDirection::Effect, _) => {
    debug!("Auto-selecting 'causal_reasoning' profile for causal query");
    Some("causal_reasoning".to_string())
}
```

### Test Cases
```bash
# These queries should trigger causal_reasoning profile:
"why does the server crash?"           # → CausalDirection::Cause
"what causes memory leaks?"            # → CausalDirection::Cause
"what happens if I delete this file?"  # → CausalDirection::Effect

# These should stay on semantic_search:
"show me the authentication code"      # → CausalDirection::Unknown
"list all API endpoints"               # → CausalDirection::Unknown
```

### Files to Review (No Changes Needed)
- `crates/context-graph-mcp/src/weights.rs` - Keep existing weights
- `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` - Verify auto-switching logic

---

## Task 4: Run GPU Benchmarks with Real Embeddings

### Purpose
Validate that Tasks 1-3 improve E5's contribution without hurting E1's baseline.

### Commands

**E5 Causal Benchmark:**
```bash
cargo run --release -p context-graph-benchmark \
    --bin causal-realdata-bench \
    --features real-embeddings \
    -- --data-dir data/hf_benchmark \
       --max-chunks 2000 \
       --output-dir benchmark_results
```

**Embedder Stress Test (all embedders):**
```bash
cargo run --release -p context-graph-benchmark \
    --bin embedder-stress \
    --features real-embeddings \
    -- --format markdown
```

### Success Criteria

| Metric | Before | Target | Validates |
|--------|--------|--------|-----------|
| E5 Direction Detection Rate | 16-27% | >50% | Task 2 (patterns) |
| E5 Asymmetry Ratio | ~1.0 | ~1.5 | Task 1 (overlap removal) |
| E5 Contribution (causal queries) | 0% | >5% | Tasks 1-3 combined |
| E1 MRR (general queries) | baseline | maintain | E1 not diluted |

---

## Task 5: Create E8 Graph Benchmark Binary

### Problem
E8 Graph has no dedicated benchmark binary like E5 (causal_realdata_bench) or E4 (temporal_realdata_bench).

### Solution
Create `graph_bench.rs` following the causal_realdata_bench pattern.

### Files to Create

**`crates/context-graph-benchmark/src/bin/graph_bench.rs`**

Structure:
```rust
// Phase 1: Load dataset with graph relationships (code dependencies, imports)
// Phase 2: Embed with real E8 asymmetric vectors
// Phase 3: Run graph benchmarks:
//   - Direction detection (source vs target in dependencies)
//   - Asymmetric retrieval (caller→callee vs callee→caller)
//   - Path traversal (multi-hop dependencies)
//   - Centrality detection (hub nodes like utils/shared code)
// Phase 4: Generate report
```

**`crates/context-graph-benchmark/src/metrics/graph.rs`**

```rust
pub struct GraphMetrics {
    pub direction_accuracy: f64,
    pub asymmetry_ratio: f64,
    pub path_recall_at_k: HashMap<usize, f64>,
    pub centrality_correlation: f64,
}
```

### Update Cargo.toml

```toml
[[bin]]
name = "graph-bench"
path = "src/bin/graph_bench.rs"
required-features = ["bin", "real-embeddings"]
```

---

## Implementation Order

```
Step 1: Remove intervention overlap (Task 1)
    ↓
Step 2: Add scientific patterns (Task 2)
    ↓
Step 3: Verify auto-profile switching (Task 3)
    ↓
Step 4: Run GPU benchmarks (Task 4) - validates Tasks 1-3
    ↓
Step 5: Create E8 benchmark (Task 5) - if E5 results good
```

---

## Architecture Summary

### How E5 Adds Value (Not Competes)

```
Query: "why does the server crash?"

┌─────────────────────────────────────────────────────────────────┐
│ Step 1: Query Intent Detection                                  │
│   detect_causal_query_intent() → CausalDirection::Cause         │
│   Auto-switch to causal_reasoning profile                       │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ Step 2: Stage 1 Retrieval (Multi-Space HNSW)                    │
│   E1: 20% (semantic baseline)                                   │
│   E5: 45% (causal PRIMARY in this profile)                      │
│   E7: 10% (code context)                                        │
│   E8: 10% (graph structure)                                     │
│   Fetch 3x candidates for reranking                             │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ Step 3: Asymmetric E5 Reranking (E5's UNIQUE VALUE)             │
│   For each candidate:                                           │
│     - Compute: query.e5_as_effect vs doc.e5_as_cause            │
│     - Apply direction_mod: 1.2x (cause→effect amplified)        │
│     - Blend: 0.55×original + 0.45×asymmetric×direction_mod      │
│   Re-sort by adjusted scores                                    │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ Result: Causally-relevant documents ranked higher               │
│   Doc about "memory leak in handler" → 1.2x boost (explains)    │
│   Doc about "server crashed yesterday" → 0.8x (doesn't explain) │
└─────────────────────────────────────────────────────────────────┘
```

### What E5 Provides That E1 Cannot

| Capability | E1 (Semantic) | E5 (Causal) |
|------------|---------------|-------------|
| Topic similarity | Yes | No |
| Directional asymmetry | No | **Yes** (1.2x/0.8x) |
| Cause vs Effect encoding | No | **Yes** (dual vectors) |
| "Why" query understanding | Limited | **Specialized** |
| Causal chain traversal | No | **Possible** |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Pattern changes break existing detection | Additive only - no removal of working patterns |
| Overlap removal hurts edge cases | Feature flag allows re-enabling if needed |
| Auto-profile switching misdetects | Add confidence threshold (only switch if score > 0.7) |
| E1 accidentally diluted | No weight changes to semantic_search profile |

---

## Files Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `crates/context-graph-core/src/causal/asymmetric.rs` | Modify | Remove overlap factor, add patterns |
| `crates/context-graph-embeddings/src/models/pretrained/causal/marker_detection.rs` | Modify | Sync patterns |
| `crates/context-graph-mcp/src/weights.rs` | **NO CHANGE** | Keep E1 dominant in semantic_search |
| `crates/context-graph-benchmark/src/bin/graph_bench.rs` | Create | E8 benchmark |
| `crates/context-graph-benchmark/src/metrics/graph.rs` | Create | E8 metrics |
| `crates/context-graph-benchmark/Cargo.toml` | Modify | Add graph-bench binary |
