# Training Subsystem

**Source files covered:**
- `crates/context-graph-core/src/training/**/*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`
- `crates/context-graph-mcp/src/tools/definitions/training.rs`
- `crates/context-graph-cli/src/commands/export_training.rs`

## 1. Scope
`TrainingRecord` v3 packages one memory's identity, content, all embedding outputs, topic profile, cross-correlations, graph/causal/topic/temporal labels, and typed-edge distribution for export.

## 2. Record Shape
| Group | Fields |
| --- | --- |
| Identity | memory_id, content, importance, created_at, session_id |
| Source | source_type, source_path, content_hash |
| Dense | E1-E5, E7-E11, E14, with asymmetric E5/E8/E10 |
| Sparse | E6 and E13 indices/values |
| Token | E12 token embeddings |
| Fusion | 14D topic_profile, 91 cross_correlations, 6 group_alignments |
| Graph | outgoing/incoming edges, KNN neighbors |
| Causal/topic/time | causal labels, topic_memberships, temporal_labels |
| Typed edge | 8D edge_type_distribution |

## 3. Constants
| Constant | Value |
| --- | --- |
| TRAINING_RECORD_VERSION | 3 |
| NUM_CROSS_CORRELATIONS | 91 |
| NUM_GROUP_ALIGNMENTS | 6 |
| NUM_EDGE_TYPE_DISTRIBUTION | 8 |

## 4. Integration
MCP tools: export/list/get/count training records. CLI: `context-graph-cli export training-corpus`. Deserialization rejects version mismatches; source comments state v1/v2 require re-export.
