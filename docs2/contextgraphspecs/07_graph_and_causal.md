# Graph And Causal

**Source files covered:**
- `crates/context-graph-core/src/graph_linking/**/*.rs`
- `crates/context-graph-storage/src/graph_edges/**/*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/causal_relationships.rs`
- `crates/context-graph-causal-agent/src/**/*.rs`
- `crates/context-graph-graph-agent/src/**/*.rs`

## 1. Stored Graph Data
| Data | Storage |
| --- | --- |
| K-NN embedder edges | `embedder_edges` CF; key `[embedder_id:u8][source_uuid:16]` (legacy) or `[embedder_id:u8][source:16][target:16]` (current 33-byte) |
| Typed edges | `typed_edges` CF; key `[source_uuid:16][target_uuid:16]` |
| Typed-edge type index | `typed_edges_by_type` CF; key `[edge_type:u8][source:16][target:16]` |
| Causal relationships | `causal_relationships` + `causal_by_source` CFs |
| Typed-edge training | `typed_edge_records` + `typed_edge_validations` CFs |

## 2. Edge Types (TypedEdge)
| Variant | Source signal |
| --- | --- |
| SemanticSimilar | E1 agreement |
| CodeRelated | E7 agreement |
| EntityShared | E11 agreement |
| CausalChain | E5 agreement (mechanism_type joined from causal_relationships) |
| GraphConnected | E8 agreement |
| ParaphraseAligned | E10 agreement |
| KeywordOverlap | E6 / E13 agreement |
| MultiAgreement | ≥ N embedder consensus |

## 3. MCP Tool Groups
| Group | Tools |
| --- | --- |
| causal | search_causes, search_effects, get_causal_chain, search_causal_relationships |
| causal_discovery (llm) | trigger_causal_discovery, get_causal_discovery_status |
| graph (base) | search_connections, get_graph_path |
| graph (llm) | discover_graph_relationships, validate_graph_link |
| graph_link | get_memory_neighbors, get_typed_edges, traverse_graph, get_unified_neighbors |
| typed_edges | export_typed_edges_corpus, derive_anomalies_from_edges, validate_low_confidence_edges, list_typed_edge_records |

## 4. Asymmetry Modifiers (AP-77)
| Direction | Multiplier |
| --- | --- |
| E5 cause→effect | 1.2× |
| E5 effect→cause | 0.8× |
| E8 source→target | 1.2× |
| E8 target→source | 0.8× |
| E10 paraphrase→context | 1.2× |
| E10 context→paraphrase | 0.8× |

`CausalDirection::{Cause, Effect, Bidirectional}` must be set when E5 participates in fusion (otherwise embedder_score=0.0 by AP-77).

## 5. Algorithms
- **K-NN edges**: built in background after store; `get_memory_neighbors` reads precomputed list.
- **Multi-hop traversal**: `get_graph_path` / `get_causal_chain` / `traverse_graph`; hop attenuation `0.9^hop`.
- **Unified neighbors**: Weighted RRF fusion across all 14 embedders (excludes temporal E2-E4 from semantic per AP-60).
- **LLM discovery** (feature `llm`): Qwen2.5-3B via Candle; produces structured CausalRelationship / GraphRelationship records with confidence; persisted with LLMProvenance (model, version, temperature, prompt hash).
- **Edge validation**: `validate_low_confidence_edges` runs LLM on edges with weight < ceiling; persists `LLMEdgeValidation` to `typed_edge_validations`.

## 6. LLM Models
| Use case | Model | Loader |
| --- | --- | --- |
| Causal discovery | Qwen2.5-3B-Instruct (FP16, ~6GB VRAM) | candle native |
| Graph link discovery | Qwen2.5-3B-Instruct (shared) | candle native |
| Generation (Shakespeare) | various GGUF | llama-cpp-2 |

## 7. K-NN Edge Bug Fixes (Recent)
- BUG-1: typed-edge type filter now direction-aware (graph_link_tools.rs).
- BUG-2: incoming edges return peer node (`edge.source()`), not self.
- BUG-3: secondary index key extended to 33 bytes `[type][source][target]` to disambiguate same-source edges; legacy 17-byte keys read-compat.
