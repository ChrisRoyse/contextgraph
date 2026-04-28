# Search And Fusion

**Source files covered:**
- `crates/context-graph-core/src/retrieval/*.rs`
- `crates/context-graph-core/src/weights/mod.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs`

## 1. Aggregation Strategies
| Strategy | Steps/formula |
| --- | --- |
| RRF | sum `1/(k+rank+1)` for each ranked list; default k=60 |
| Weighted RRF | sum `weight/(k+rank+1)` |
| WeightedAverage | sum sim*weight divided by weight sum; can require all embedders |
| MaxPooling | max similarity |
| Additive RRF | tracks unique embedder contributions and applies unique boost |

## 2. Weight Profiles
| Profile | Source |
| --- | --- |
| semantic_search | crates/context-graph-core/src/weights/mod.rs |
| causal_reasoning | crates/context-graph-core/src/weights/mod.rs |
| code_search | crates/context-graph-core/src/weights/mod.rs |
| fact_checking | crates/context-graph-core/src/weights/mod.rs |
| graph_reasoning | crates/context-graph-core/src/weights/mod.rs |
| temporal_navigation | crates/context-graph-core/src/weights/mod.rs |
| sequence_navigation | crates/context-graph-core/src/weights/mod.rs |
| conversation_history | crates/context-graph-core/src/weights/mod.rs |
| category_weighted | crates/context-graph-core/src/weights/mod.rs |
| typo_tolerant | crates/context-graph-core/src/weights/mod.rs |
| pipeline_stage1_recall | crates/context-graph-core/src/weights/mod.rs |
| pipeline_stage2_scoring | crates/context-graph-core/src/weights/mod.rs |
| pipeline_full | crates/context-graph-core/src/weights/mod.rs |
| balanced | crates/context-graph-core/src/weights/mod.rs |
| multilingual_search | crates/context-graph-core/src/weights/mod.rs |
| long_context | crates/context-graph-core/src/weights/mod.rs |
| translation_finder | crates/context-graph-core/src/weights/mod.rs |

## 3. Constants
| Constant | Value |
| --- | --- |
| RRF_K | 60.0 core / 60 storage |
| TEMPORAL_EMBEDDERS_ENABLED | true |
| E11_ENTITY_ENABLED | false |
| SPACE_WEIGHTS length | 14 |
| TOTAL_WEIGHT | 9.5 |
| MULTI_SPACE_MAX_EMBEDDERS | 10 |

## 4. Storage Search Flow
Resolve weights, apply E11 disable, apply exclusions and renormalize, run per-embedder searches, filter soft-deleted ids unless requested, then fuse ranked lists. E6/E13 use sparse paths and E12 uses late-interaction reranking paths.

## 5. Current Source Gaps
`MULTI_SPACE_MAX_EMBEDDERS` is 10 while `NUM_EMBEDDERS` is 14. `TELEOLOGICAL_VECTOR_DIM` is 13 while training/search profiles use 14D arrays.
