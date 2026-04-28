# Contrastive And Typed Edges

**Source files covered:**
- `crates/context-graph-core/src/contrastive/**/*.rs`
- `crates/context-graph-core/src/typed_edges/**/*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive*.rs`
- `crates/context-graph-mcp/src/tools/definitions/contrastive.rs`
- `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs`

## 1. Scope
Contrastive tools mine and read disagreement pairs in `CF_CONTRASTIVE_PAIRS`; typed-edge tools export and validate typed-edge training records.

## 2. Contrastive Defaults And Kinds
| Item | Value |
| --- | --- |
| max_pairs | 10000 |
| min_disagreement | 0.3 |
| high threshold | 0.6 |
| low threshold | 0.3 |
| top_k per anchor | 10 |
| anomaly kinds | semantic_but_not_causal, keyword_but_not_paraphrase, code_shape_but_different_intent, entity_shared_but_different_structure, hdc_robust_but_semantic_different, other |

## 3. Tools
Contrastive: mine_contrastive_pairs, list_contrastive_pairs, get_contrastive_pair, count_contrastive_pairs. Typed edge: export_typed_edges_corpus, derive_anomalies_from_edges, validate_low_confidence_edges, list_typed_edge_records.

## 4. Integration
Training exports include edge type distributions; CAMD scripts consume contrastive pairs; graph/causal docs cover typed graph storage.
