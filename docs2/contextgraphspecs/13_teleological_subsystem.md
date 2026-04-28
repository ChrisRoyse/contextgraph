# Teleological Subsystem

**Source files covered:**
- `crates/context-graph-core/src/teleological/**/*.rs`
- `crates/context-graph-core/src/types/fingerprint/**/*.rs`
- `crates/context-graph-storage/src/teleological/**/*.rs`

## 1. Scope
The teleological subsystem combines semantic fingerprints, metadata, topic profiles, synergy/cross-correlation features, profile weights, matrix search, multi-resolution views, and storage integration.

## 2. Data Models
| Model | Fields/shape |
| --- | --- |
| SemanticFingerprint | 14 production slots with sparse/token/asymmetric fields |
| TeleologicalFingerprint | id, semantic, content_hash, timestamps, access_count, importance, last_accessed_at, optional e6_sparse |
| TopicProfile | per-embedder alignment profile |
| Training cross correlations | C(14,2)=91 |
| Group alignments | 6D |

## 3. Formulas
| Formula | Behavior |
| --- | --- |
| teleological_vector | 13 values: dense L2 norms, sparse nnz/1000, E12 avg token norm; E14 excluded |
| e6_term_overlap | merge-join sparse indices; shared/query nnz |
| importance | with_importance clamps to [0,1] |
| record_access | increments count and updates timestamps |

## 4. Current Source Gaps
`TeleologicalFingerprint` comments still refer to 13 embeddings while `SemanticFingerprint` has 14. `TELEOLOGICAL_VECTOR_DIM` remains 13.
