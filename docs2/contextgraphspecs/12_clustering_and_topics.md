# Clustering And Topics

**Source files covered:**
- `crates/context-graph-core/src/clustering/**/*.rs`
- `crates/context-graph-core/src/types/topic.rs`
- `crates/context-graph-mcp/src/tools/definitions/topic.rs`

## 1. Scope
Clustering builds fingerprint matrices, derives topic profiles, runs HDBSCAN reclustering, and exposes topic/divergence MCP+CLI surfaces.

## 2. Constants
| Constant | Value | Source |
| --- | --- | --- |
| MAX_WEIGHTED_AGREEMENT | 9.5 | clustering/manager.rs |
| NUM_EMBEDDERS | 14 | semantic/constants.rs |
| Topic MCP tools | 4 | tools/definitions/topic.rs |
| Topic emergence threshold | weighted_agreement ≥ 2.5 | clustering manager |

## 3. Algorithms
- **Fingerprint matrix**: per-embedder similarity aggregated via `weighted | max | mean | product | min`.
- **Topic confidence** = weighted_agreement / `MAX_WEIGHTED_AGREEMENT`.
- **HDBSCAN**: minimum cluster size 3 (per `clustering.parameters`).
- **Divergence alerts**: semantic embedders only (E1, E6, E7, E10, E12, E13 per AP-62). Excludes E5 (no CausalDirection) per AP-77 and E2-E4 per AP-63.

## 4. Storage Integration
| Concern | CF |
| --- | --- |
| Topic profile per memory | CF_TOPIC_PROFILES (UUID → 13 × f32 = 52B) |
| Per-session portfolio | CF_TOPIC_PORTFOLIO (key by session_id or `__latest__`) |
| Training records carry topic_profile | CF_TRAINING_RECORDS (14D) |
| Constellation centroids include topic profile aggregation | CF_CONSTELLATIONS |

## 5. MCP Tools
get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts. See [08_mcp_tools_reference.md](08_mcp_tools_reference.md).
