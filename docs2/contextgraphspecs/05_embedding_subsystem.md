# Embedding Subsystem

**Source files covered:**
- `crates/context-graph-core/src/types/fingerprint/semantic/*.rs`
- `crates/context-graph-core/src/types/fingerprint/teleological/*.rs`
- `crates/context-graph-embeddings/src/types/model_id/*.rs`
- `crates/context-graph-embeddings/src/config/*.rs`

## 1. Purpose
`SemanticFingerprint` stores all 14 production embedding slots without fusion. Sparse (E6/E13), token-level (E12), asymmetric (E5/E8/E10), and dense (E14) are separate fields. `LearnerFingerprint` (separate type) holds E15-E21 learner-state vectors.

## 2. Production ModelId Table (E1-E14, NUM_EMBEDDERS=14)
| Index | Slot | Repository | Native Dim | Stored Dim | Max tokens |
| --- | --- | --- | --- | --- | --- |
| 0 | E1_Semantic | intfloat/e5-large-v2 | 1024 | 1024 | 512 |
| 1 | E2_TemporalRecent | sinusoidal PE of timestamp | — | 512 | n/a |
| 2 | E3_TemporalPeriodic | Fourier periodic | — | 512 | n/a |
| 3 | E4_TemporalPositional | sinusoidal positional | — | 512 | n/a |
| 4 | E5_Causal | nomic-ai/nomic-embed-text-v1.5 (asymmetric cause/effect) | 768 | 768 | 512 |
| 5 | E6_Sparse | naver/splade-cocondenser-ensembledistil | 30522 sparse | sparse | 512 |
| 6 | E7_Code | Qodo/Qodo-Embed-1-1.5B | 1536 | 1536 | 32768 |
| 7 | E8_Graph | intfloat/e5-large-v2 (asymmetric source/target) | 1024 | 1024 | 512 |
| 8 | E9_HDC | custom hyperdimensional | 10000 binary | 1024 projected | n/a |
| 9 | E10_Multimodal | intfloat/e5-base-v2 (asymmetric paraphrase/context) | 768 | 768 | 512 |
| 10 | E11_Entity | THU-KEG/KEPLER-Wiki5M-KE (Kepler/TransE) | 768 | 768 | 512 |
| 11 | E12_LateInteraction | colbert-ir/colbertv2.0 | 128/token | 128/token | 512 |
| 12 | E13_SPLADE | prithivida/Splade_PP_en_v1 | 30522 sparse | sparse | 512 |
| 13 | E14_BgeM3Dense | BAAI/bge-m3 (multilingual dense) | 1024 | 1024 | 8192 |

`ModelId::all` returns 15 variants (legacy `Entity`/MiniLM-384 retained for tests); `ModelId::production` returns 14 variants used in storage/HNSW.

## 3. Learner-State Slots (E15-E21)
| Slot | Source | Purpose |
| --- | --- | --- |
| E15 | OpenFace 3.0 MTL | Affect / facial action units |
| E16 | speech embeddings | Affect / paralinguistic |
| E17 | PaPaGei (Zenodo) | PPG / cardiac |
| E18 | EDA model | Galvanic skin response |
| E19 | WESAD stress head | Stress regulation |
| E20 | EEG model | EEG |
| E21 | EEG artifact-robust | EEG (motion-robust) |

E15-E21 do not participate in content fusion; they back `LearnerStateVector` and goal-distance computations.

## 4. SemanticFingerprint Fields
| Field | Representation |
| --- | --- |
| e1_semantic | 1024D dense |
| e2_temporal_recent / e3 / e4 | 512D dense each |
| e5_causal_as_cause / e5_causal_as_effect / legacy e5_causal | 768D asymmetric dense |
| e6_sparse | SparseVector (indices + values) |
| e7_code | 1536D dense |
| e8_graph_as_source / e8_graph_as_target / legacy e8_graph | 1024D asymmetric dense |
| e9_hdc | 1024D projected dense |
| e10_multimodal_paraphrase / e10_multimodal_as_context | 768D asymmetric dense |
| e11_entity | 768D dense |
| e12_late_interaction | Vec<Vec<f32>>, 128D per token |
| e13_splade | SparseVector |
| e14_bge_m3_dense | 1024D dense (serde default = vec![]) |

## 5. Constants
| Constant | Value | Source |
| --- | --- | --- |
| NUM_EMBEDDERS | 14 | semantic/constants.rs |
| E1_DIM..E14_DIM | 1024,512,512,512,768,—,1536,1024,1024,768,768,128,—,1024 | semantic/constants.rs |
| E6_SPARSE_VOCAB / E13_SPLADE_VOCAB | 30522 | semantic/constants.rs |
| TOTAL_DENSE_DIMS | sum of dense dims | semantic/constants.rs |
| TELEOLOGICAL_VECTOR_DIM | 13 (E14 excluded from compact vector) | teleological/types.rs |
| MODEL_COUNT (embeddings crate) | 15 (production = 14 + legacy Entity) | embeddings/types/model_id |

## 6. Validation Errors
`DimensionMismatch`, `EmptyDenseEmbedding`, `TokenDimensionMismatch`, `SparseIndexOutOfBounds`, `SparseIndicesValuesMismatch` defined in `semantic/fingerprint.rs`.

## 7. Integration Points
- HNSW: 15 indexes (E1, E1Matryoshka128, E2-E4, E5+E5CausalCause+E5CausalEffect, E7-E11, E10MultimodalParaphrase+E10MultimodalContext, E14).
- E6/E13: sparse inverted indexes in CF_E6_SPARSE_INVERTED / CF_E13_SPLADE_INVERTED.
- E12: late-interaction MaxSim path; data in CF_E12_LATE_INTERACTION.
- Quantized per-embedder CFs CF_EMB_0..CF_EMB_13 (PQ-8, Float8E4M3, Binary, SparseNative, TokenPruning).
