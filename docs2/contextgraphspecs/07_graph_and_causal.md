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
| K-NN embedder edges | embedder_edges, key `[embedder_id][source_uuid]` |
| Typed edges | typed_edges, key `[source_uuid][target_uuid]` |
| Typed edge type index | typed_edges_by_type |
| Causal relationships | causal_relationships and causal_by_source |
| Typed-edge training | typed_edge_records and typed_edge_validations |

## 2. MCP Tool Groups
| Group | Tools |
| --- | --- |
| causal | search_causes, search_effects, get_causal_chain, search_causal_relationships |
| causal_discovery | trigger_causal_discovery, get_causal_discovery_status; llm feature |
| graph | search_connections, get_graph_path, discover_graph_relationships, validate_graph_link |
| graph_link | get_memory_neighbors, get_typed_edges, traverse_graph, get_unified_neighbors |
| typed_edges | export_typed_edges_corpus, derive_anomalies_from_edges, validate_low_confidence_edges, list_typed_edge_records |

## 3. Algorithms
Handlers read graph/causal records from storage, apply tool parameters such as source/target/depth/limit, and return relationship, path, or neighborhood structures. LLM graph and causal discovery paths are feature-gated by `llm`.
