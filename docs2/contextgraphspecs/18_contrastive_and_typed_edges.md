# Contrastive And Typed Edges

**Source files covered:**
- `crates/context-graph-core/src/contrastive/**/*.rs`
- `crates/context-graph-core/src/typed_edges/**/*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edges*.rs`
- `crates/context-graph-mcp/src/tools/definitions/contrastive.rs`
- `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs`

## 1. Scope
- **Contrastive pairs** mine cross-embedder disagreements into `CF_CONTRASTIVE_PAIRS` (+ by_kind / by_anchor secondary indexes).
- **Typed-edge training records** snapshot every typed edge with full similarity profile + optional joins to `CF_TYPED_EDGE_VALIDATIONS` (LLM verdict).

## 2. AnomalyKinds
| Kind | Pattern |
| --- | --- |
| semantic_but_not_causal | High E1, low E5 |
| keyword_but_not_paraphrase | High E6/E13, low E10 |
| code_shape_but_different_intent | High E7, low E1 |
| entity_shared_but_different_structure | High E11, low E8 |
| hdc_robust_but_semantic_different | High E9, low E1 |
| other | Sentinel (unmatched) |

## 3. Defaults
| Param | Default |
| --- | --- |
| max_pairs | 10000 |
| min_disagreement | 0.3 |
| high_threshold | 0.6 |
| low_threshold | 0.3 |
| topKCandidatesPerAnchor | 10 |
| candidatePoolSize | 100 |

## 4. Storage Layout
| CF | Key | Value |
| --- | --- | --- |
| CF_CONTRASTIVE_PAIRS | `[anchor:16][negative:16]` (32B) | bincode ContrastivePair (similarity_profile + texts + metadata) |
| CF_CONTRASTIVE_BY_KIND | `[kind:u8] + composite` | Pointer for kind prefix-scan |
| CF_CONTRASTIVE_BY_ANCHOR | `[anchor:16] + composite` | Pointer for anchor prefix-scan |
| CF_TYPED_EDGES | `[source:16][target:16]` (32B) | bincode TypedEdge |
| CF_TYPED_EDGES_BY_TYPE | `[type:u8][source:16][target:16]` (33B) | target UUID |
| CF_TYPED_EDGE_RECORDS | `[source:16][target:16][type:u8]` (33B) | bincode TypedEdgeTrainingRecord |
| CF_TYPED_EDGE_VALIDATIONS | `[source:16][target:16][type:u8]` (33B) | bincode LLMEdgeValidation |

## 5. Tools
| Tool | Behavior |
| --- | --- |
| mine_contrastive_pairs | Scan for anomaly pairs (bounded candidate pool) |
| list_contrastive_pairs | Paginated read; `kind` / `anchorId` filters |
| get_contrastive_pair | Full record by composite key |
| count_contrastive_pairs | Row count (optional kind prefix) |
| export_typed_edges_corpus | Export every CF_TYPED_EDGES row as TypedEdgeTrainingRecord; idempotent on composite key |
| derive_anomalies_from_edges | Classify typed edges into 5 AnomalyKind patterns; persist via standard contrastive write path |
| validate_low_confidence_edges | LLM verdict on edges with weight < ceiling; persists to CF_TYPED_EDGE_VALIDATIONS; skipped without `llm` |
| list_typed_edge_records | Paginated read |

## 6. Generators (provenance field on records)
| Generator | Source tool |
| --- | --- |
| miner | mine_contrastive_pairs |
| edge_derived | derive_anomalies_from_edges |

## 7. Integration
- TrainingRecord includes `edge_type_distribution` (8D) over outgoing typed edges.
- CAMD-DPO pair miners ([19](19_camd_dpo_subsystem.md)) consume contrastive pairs and constellation scores.
- LLM validation feeds typed-edge weight gating used by graph traversal scoring.
