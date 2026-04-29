# Storage Layer

**Source files covered:**
- `crates/context-graph-storage/src/**/*.rs`
- `crates/context-graph-core/src/traits/**/*.rs`

## 1. Scope
The storage crate persists base memory records, teleological fingerprints, quantized per-embedder records, code entities, causal relationships, training exports, learning events, learner state, constellations, contrastive pairs, and typed-edge records. Single-writer enforced via `<db_path>/mcp.pid` Unix flock.

## 2. CF Groups
| Group | Count | Source |
| --- | --- | --- |
| Base | 11 | `column_families.rs` |
| Teleological / learner / training | 41 | `teleological/column_families.rs` |
| Quantized embedder | 14 | `teleological/column_families.rs` |
| Code | 5 | `code` module |
| Causal | 2 | `causal_relationships` module |
| **Total** | **73** | |

See [04_database_schema.md](04_database_schema.md) for every CF.

## 3. RocksDB Option Profiles
| Profile | Used by | Behavior |
| --- | --- | --- |
| nodes_options | base/nodes, fingerprints | Bloom filter, shared cache, LZ4, point-lookup, 4MB write buffer |
| edges_options | edges, embedder_edges, typed_edges | Bloom, shared cache, LZ4, prefix extractor (16/17/33 bytes), 4MB |
| embeddings_options | quantized embedder CFs | 64KB blocks, shared cache, LZ4, 8MB write buffer |
| index_options | secondary indexes (audit, file_index, etc.) | Bloom, LZ4, 16-byte prefix, 2MB |
| system_options | CF_SYSTEM | No compression, rare access |
| append_only | audit_log, merge_history, importance_history, embedding_registry | Bloom, big-endian timestamp prefix, NO update/delete |

## 4. Repository Modules (under `crates/context-graph-storage/src`)
| Module | Concern |
| --- | --- |
| `repository.rs` | Top-level CRUD, K-NN edge writes |
| `content` | CF_CONTENT, CF_SOURCE_METADATA |
| `search` | Multi-space search, fusion, sparse path |
| `export` | TrainingRecord export to CF_TRAINING_RECORDS |
| `constellation` | CF_CONSTELLATIONS + selector index |
| `contrastive*` | CF_CONTRASTIVE_PAIRS + by_kind / by_anchor |
| `learner*` | All CF_LEARNER_* and CF_FINGERPRINTS_LEARNER |
| `code` | CF_CODE_* |
| `graph_edges` | CF_TYPED_EDGES, CF_TYPED_EDGES_BY_TYPE, CF_EMBEDDER_EDGES |
| `causal_relationships` | CF_CAUSAL_RELATIONSHIPS, CF_CAUSAL_BY_SOURCE |
| `learning_events` | CF_LEARNING_EVENTS, CF_LEARNER_TRAINING_DATASETS |

## 5. Single-Writer Guard
`PidFileGuard` (in MCP main) writes `<db_path>/mcp.pid`, takes `flock(LOCK_EX|LOCK_NB)`, verifies prior PID is alive (`kill(pid, 0) == 0` AND `/proc/pid/status` not zombie). Stale locks are reclaimed.

## 6. HNSW Indexes (15)
E1, E1Matryoshka128, E2, E3, E4, E5+E5CausalCause+E5CausalEffect, E7, E8, E9, E10+E10MultimodalParaphrase+E10MultimodalContext, E11, E14. Stored in `CF_HNSW_GRAPHS`. Background persist on shutdown.

## 7. Versioning
| Record | Version |
| --- | --- |
| TrainingRecord | 3 |
| Learner records | 1 |
| LearningEvent | 1 |
| LearnerTrainingDataset | (per `LEARNER_TRAINING_DATASET_VERSION`) 1 |

Records reject mismatched version bytes; pre-production policy is to wipe DB rather than migrate.
