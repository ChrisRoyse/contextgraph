# Embedding Subsystem

**Source files covered:**
- `crates/context-graph-core/src/types/fingerprint/semantic/*.rs`
- `crates/context-graph-core/src/types/fingerprint/teleological/*.rs`
- `crates/context-graph-embeddings/src/types/model_id/*.rs`
- `crates/context-graph-embeddings/src/config/*.rs`

## 1. Purpose
`SemanticFingerprint` stores all 14 production embedding slots without fusion. Sparse E6/E13, token E12, asymmetric E5/E8/E10, and E14 BGE-M3 are separate fields.

## 2. ModelId Table
| repr | Slot | Repository/implementation | Dimension | Max tokens |
| --- | --- | --- | --- | --- |
| 0 | E1 Semantic | intfloat/e5-large-v2 | 1024 | 512 |
| 1 | E2 TemporalRecent | custom exponential decay | 512 | unlimited |
| 2 | E3 TemporalPeriodic | custom Fourier | 512 | unlimited |
| 3 | E4 TemporalPositional | custom sinusoidal | 512 | unlimited |
| 4 | E5 Causal | nomic-ai/nomic-embed-text-v1.5 | 768 | 512 |
| 5 | E6 Sparse | naver/splade-cocondenser-ensembledistil | 30522 sparse -> 1536 | 512 |
| 6 | E7 Code | Qodo/Qodo-Embed-1-1.5B | 1536 | 32768 |
| 7 | E8 Graph | intfloat/e5-large-v2 | 1024 | 512 |
| 8 | E9 HDC | custom HDC | 10000 -> 1024 | unlimited |
| 9 | E10 Contextual | intfloat/e5-base-v2 | 768 | 512 |
| 10 | Entity legacy | all-MiniLM-L6-v2 | 384 | 512 |
| 11 | E12 LateInteraction | colbert-ir/colbertv2.0 | 128/token | 512 |
| 12 | E13 Splade | prithivida/Splade_PP_en_v1 | 30522 sparse -> 1536 | 512 |
| 13 | E11 Kepler | THU-KEG/KEPLER-Wiki5M-KE | 768 | 512 |
| 14 | E14 BgeM3Dense | BAAI/bge-m3 | 1024 | 8192 |

## 3. SemanticFingerprint Fields
| Field | Representation |
| --- | --- |
| e1_semantic | 1024D dense |
| e2_temporal_recent | 512D dense |
| e3_temporal_periodic | 512D dense |
| e4_temporal_positional | 512D dense |
| e5_causal_as_cause/effect | 768D asymmetric dense plus legacy e5_causal |
| e6_sparse | SparseVector |
| e7_code | 1536D dense |
| e8_graph_as_source/target | 1024D asymmetric dense plus legacy e8_graph |
| e9_hdc | 1024D projected dense |
| e10_multimodal_paraphrase/as_context | 768D asymmetric dense |
| e11_entity | 768D dense |
| e12_late_interaction | Vec<Vec<f32>>, 128D per token |
| e13_splade | SparseVector |
| e14_bge_m3_dense | 1024D dense, serde default |

## 4. Constants And Gaps
| Constant | Value | Note |
| --- | --- | --- |
| NUM_EMBEDDERS | 14 | array sizing |
| TOTAL_DENSE_DIMS | 9472 by expression | source comment includes older arithmetic |
| TELEOLOGICAL_VECTOR_DIM | 13 | compact vector excludes E14 |
| ModelId::all | 15 variants | includes legacy Entity and production Kepler |
| ModelId::production | 14 variants | excludes legacy Entity |

## 5. Validation Errors
DimensionMismatch, EmptyDenseEmbedding, TokenDimensionMismatch, SparseIndexOutOfBounds, and SparseIndicesValuesMismatch are defined in `semantic/fingerprint.rs`.
