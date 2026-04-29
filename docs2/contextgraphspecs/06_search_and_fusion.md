# Search And Fusion

**Source files covered:**
- `crates/context-graph-core/src/retrieval/*.rs`
- `crates/context-graph-core/src/weights/mod.rs`
- `crates/context-graph-core/src/fusion/mod.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs`

## 1. Aggregation Strategies
| Strategy | Formula |
| --- | --- |
| RRF | `sum 1/(k + rank + 1)` per ranked list; `RRF_K = 60` |
| Weighted RRF | `sum weight/(k + rank + 1)` |
| Weighted Average | `Σ sim·weight / Σ weight`; can require all embedders |
| Max Pooling | `max(sim_i)` |
| Additive RRF | RRF + unique-embedder boost |

## 2. Search Strategies
| Strategy | Behavior |
| --- | --- |
| `e1_only` | E1 dense recall only |
| `multi_space` (default) | Per-embedder recall fused via Weighted RRF |
| `pipeline` | Stage 1 sparse recall (E13/E6) → Stage 2 dense rerank (E1) → Stage 3 token MaxSim (E12) |
| `embedder_first` | Single-embedder recall using `embedder` parameter |

## 3. Weight Profiles (17 total, all sum to 1.0; length 14)
| Profile | Primary embedders | Use case |
| --- | --- | --- |
| semantic_search | E1 (0.33), E7 (0.20), E5/E10 (0.15) | General queries |
| causal_reasoning | E1 (0.40), E7 (0.15), E5/E8/E10/E11 (0.10) | "Why" / cause-effect |
| code_search | E7 (0.40), E1 (0.20), E5/E6/E10/E11 (0.10) | Programming queries |
| fact_checking | E11 (0.40), E1/E5/E6 (0.15) | Named entities |
| graph_reasoning | E8 primary | Structural connectivity |
| temporal_navigation | E2/E3/E4 weighted | Time-anchored retrieval |
| sequence_navigation | E4 primary | Conversation flow |
| conversation_history | sequence-aware | Session continuity |
| category_weighted | balanced semantic | Multi-domain |
| typo_tolerant | E9 (HDC) elevated | Noisy queries |
| pipeline_full | sparse + dense + late-interaction | Stage-balanced |
| balanced | uniform | Baseline |
| multilingual_search | E14 elevated | BGE-M3 multilingual |
| long_context | E7 / E14 | Long documents |
| translation_finder | E10 / E14 | Cross-lingual paraphrase |
| affect_repair | learner-state-routed | Stress floor ≥ 0.60 |
| affect_priming | learner-state-routed | Valence ≤ -0.30 |
| affect_neutral | learner-state-routed | Stress floor ≥ 0.80 or stress+arousal high |

`get_weight_profile(name)`, `get_profile_names()`, `get_effective_weight_profile(name)` (applies E11 disable when `E11_ENTITY_ENABLED=false`), `apply_e11_disable(weights)`, `validate_weights(weights)`.

## 4. State-Conditioned Profile Selection
`select_state_conditioned_weight_profile(base_profile, components)` routes by `LearnerStateComponents`:
| Condition | Selected profile |
| --- | --- |
| `stress_floor >= 0.80` OR (`stress_floor >= 0.60` AND `arousal >= 0.65`) | `affect_neutral` |
| `stress_floor >= 0.60` | `affect_repair` |
| `valence <= -0.30` | `affect_priming` |
| else | `base_profile` (default `semantic_search`) |

Exposed via MCP `resolve_learner_retrieval_policy`. Fail-fast on unknown base profile or invalid state.

## 5. Constants
| Constant | Value | Source |
| --- | --- | --- |
| RRF_K (core) | 60.0 (f32) | retrieval/config.rs, fusion/mod.rs |
| RRF_K (storage) | 60 (i32) | rocksdb_store/fusion.rs |
| TEMPORAL_EMBEDDERS_ENABLED | true | weights/mod.rs |
| E11_ENTITY_ENABLED | false | weights/mod.rs |
| TOTAL_WEIGHT | 9.5 | retrieval/config.rs |
| MAX_WEIGHTED_AGREEMENT | 9.5 | clustering/manager.rs |
| WEIGHT_PROFILE array length | 14 | weights/mod.rs |
| NUM_EMBEDDERS | 14 | semantic/constants.rs |

## 6. Storage Search Flow
1. Resolve weights via `get_effective_weight_profile`.
2. Apply `excludeEmbedders`, renormalize so remaining ≈ 1.0.
3. Per-embedder search: HNSW for dense, sparse inverted for E6/E13, MaxSim path for E12.
4. Filter soft-deleted IDs unless requested.
5. Fuse ranked lists via selected aggregation.
6. Optional rerank: enable_rerank, enable_asymmetric_e5, enable_query_expansion.
7. Optional post-retrieval temporal/periodic boost (AP-25; not in fusion).

## 7. Source Notes
- `MULTI_SPACE_MAX_EMBEDDERS = 10` historic; current code paths fuse all 14 with weights.
- `TELEOLOGICAL_VECTOR_DIM = 13` (compact vector excludes E14) while training/search profiles use 14D arrays.
