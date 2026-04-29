# Training Subsystem

**Source files covered:**
- `crates/context-graph-core/src/training/**/*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`
- `crates/context-graph-mcp/src/tools/definitions/training.rs`
- `crates/context-graph-cli/src/commands/export_training.rs`

## 1. Scope
`TrainingRecord` v3 packages a memory's identity, content, all 14 embedding outputs, topic profile, cross-correlations, graph/causal/topic/temporal labels, and typed-edge type distribution. Persisted to `CF_TRAINING_RECORDS` (versioned bincode).

## 2. Record Shape
| Group | Fields |
| --- | --- |
| Identity | memory_id, content, importance, created_at, session_id |
| Source | source_type, source_path, content_hash |
| Dense (E1-E5, E7-E11, E14) | E1 (1024), E2-E4 (512 each), E5 (768), E7 (1536), E8 (1024), E9 (1024 projected), E10 (768), E11 (768), E14 (1024) |
| Asymmetric | E5 cause / effect, E8 source / target, E10 paraphrase / context |
| Sparse | E6 indices/values, E13 indices/values |
| Token | E12 token-level (Vec<Vec<f32>>, 128/token) |
| Fusion | 14D topic_profile, 91 cross_correlations, 6 group_alignments |
| Graph | outgoing/incoming edges, K-NN neighbors per embedder |
| Causal/topic/time | causal labels, topic_memberships, temporal_labels |
| Typed edge | 8D edge_type_distribution |
| Optional | Tucker-1 core (Phase 4, CPU) when `includeTuckerCore=true` |

## 3. Constants
| Constant | Value |
| --- | --- |
| TRAINING_RECORD_VERSION | 3 |
| NUM_CROSS_CORRELATIONS | 91 (= C(14, 2)) |
| NUM_GROUP_ALIGNMENTS | 6 |
| NUM_EDGE_TYPE_DISTRIBUTION | 8 |

## 4. MCP Tools
- `export_training_corpus` — Export to CF_TRAINING_RECORDS. Toggles: filter, filterId, maxMemories, includeEmbeddings, includeSparseVectors, includeTokenEmbeddings, includeEdges, includeCausalLabels, includeIncomingEdges, includeTemporalLabels, includeTuckerCore, clearExisting.
- `list_training_records` (limit/offset/includeShape).
- `get_training_record` (memoryId; includeVectors / includeTokenEmbeddings / includeEdges).
- `count_training_records`.

## 5. CLI
`context-graph-cli export training-corpus` — flags: `batch_size`, `limit`, `out`, `storage`.

## 6. Versioning Policy
Deserialization rejects v1/v2 records. Pre-production policy is wipe-and-re-export rather than migrate.

## 7. Related
- Learner training datasets (CF_LEARNER_TRAINING_DATASETS) — see [20_learning_subsystem.md](20_learning_subsystem.md).
- Typed-edge training records (CF_TYPED_EDGE_RECORDS) — see [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).
- DPO pair mining off training corpus — see [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md).
