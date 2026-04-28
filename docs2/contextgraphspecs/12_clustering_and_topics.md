# Clustering And Topics

**Source files covered:**
- `crates/context-graph-core/src/clustering/**/*.rs`
- `crates/context-graph-core/src/types/topic.rs`
- `crates/context-graph-mcp/src/tools/definitions/topic.rs`

## 1. Scope
Clustering/topic code builds fingerprint matrices, topic profiles, recluster results, and topic MCP/CLI surfaces.

## 2. Constants
| Constant | Value |
| --- | --- |
| MAX_WEIGHTED_AGREEMENT | 9.5 |
| NUM_EMBEDDERS | 14 |
| Topic MCP tools | 4 |

## 3. Algorithms
Fingerprint matrices aggregate per-embedder similarity with weighted, max, mean, product, or min strategies. Topic confidence uses weighted agreement divided by `MAX_WEIGHTED_AGREEMENT`. Topic tools expose portfolio, stability, detection, and divergence alerts.

## 4. Integration
Topic profiles are stored in CF_TOPIC_PROFILES, topic portfolios in CF_TOPIC_PORTFOLIO, training records carry 14D topic profiles, and constellations aggregate topic profile centroids.
