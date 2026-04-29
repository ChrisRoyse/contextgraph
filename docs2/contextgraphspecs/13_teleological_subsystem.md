# Teleological Subsystem

**Source files covered:**
- `crates/context-graph-core/src/teleological/**/*.rs`
- `crates/context-graph-core/src/types/fingerprint/**/*.rs`
- `crates/context-graph-storage/src/teleological/**/*.rs`

## 1. Scope
The teleological subsystem combines `SemanticFingerprint`, metadata, topic profiles, synergy/cross-correlation features, weight profiles, multi-space search, and storage integration.

## 2. Data Models
| Model | Fields/Shape |
| --- | --- |
| SemanticFingerprint | 14 production slots; sparse / token / asymmetric fields (see [05](05_embedding_subsystem.md)) |
| TeleologicalFingerprint | id, semantic, content_hash, timestamps, access_count, importance, last_accessed_at, optional e6_sparse |
| TopicProfile | per-embedder alignment profile; 13D compact (E14 excluded) |
| Cross correlations | C(14, 2) = 91 pairwise |
| Group alignments | 6D semantic group buckets |

## 3. Compact Vector Layout (`teleological_vector` → 13 entries)
1-9: dense L2 norms for E1, E2, E3, E4, E5, E7, E8, E9, E10
10: e6_sparse nnz / 1000
11: E11 dense L2 norm
12: e12 average per-token norm
13: e13 sparse nnz / 1000
(E14 dense norm intentionally excluded; tracked separately in training records.)

## 4. Formulas
| Formula | Behavior |
| --- | --- |
| `e6_term_overlap` | Merge-join sparse indices; shared / query nnz |
| `with_importance` | Clamp to [0.0, 1.0] |
| `record_access` | Increment count + update timestamps |
| Topic confidence | weighted_agreement / 9.5 |

## 5. Constants
| Constant | Value |
| --- | --- |
| TELEOLOGICAL_VECTOR_DIM | 13 |
| NUM_EMBEDDERS | 14 |
| MAX_WEIGHTED_AGREEMENT | 9.5 |
| TOTAL_WEIGHT | 9.5 |

## 6. Source Notes
- `TeleologicalFingerprint` carries 14 embeddings (via SemanticFingerprint) but its compact `teleological_vector` is 13D.
- `record_access` mutates `last_accessed_at`; access patterns drive importance heuristics.
- Storage integration handled by `storage/teleological/rocksdb_store/*.rs`.
