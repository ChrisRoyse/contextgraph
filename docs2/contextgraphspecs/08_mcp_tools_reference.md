# MCP Tools Reference

**Source files covered:**
- `crates/context-graph-mcp/src/tools/names.rs`
- `crates/context-graph-mcp/src/tools/definitions/*.rs`
- `crates/context-graph-mcp/src/handlers/tools/**/*.rs`

## 1. Registry
`get_tool_definitions()` aggregates per-category modules. Test `tools::definitions::tests::test_total_tool_count_and_no_duplicates` asserts **108 tools with `llm`, 104 without**. `names.rs` defines 108 tool constants. Categories with `*` denote required top-level params.

## 2. Category Counts
| Category | Count | Notes |
| --- | --- | --- |
| capability | 1 | get_capability_matrix |
| core | 5 | store_memory, store_memories, get_memetic_status, search_graph, trigger_consolidation |
| merge | 1 | |
| curation | 2 | |
| topic | 4 | |
| file_watcher | 4 | |
| sequence | 4 | E4 |
| causal | 4 | E5 |
| causal_discovery | 0/2 | LLM-gated |
| keyword | 1 | E6 |
| code | 1 | E7 |
| graph | 2/4 | LLM adds 2 |
| robustness | 1 | E9 |
| entity | 6 | E11/TransE |
| embedder | 7 + 2 standalone | E1-E14 cross-cutting + search_by_tokens / search_by_expansion |
| temporal | 2 | E2/E3 |
| graph_link | 4 | K-NN, typed edges, traversal |
| maintenance | 1 | repair_causal_relationships |
| provenance | 3 | audit, merge, chain |
| daemon | 1 | daemon_status |
| training | 4 | TrainingRecord v3 export + read |
| learning | 12 | events, signal embedders, outcome estimate, training datasets |
| utl | 18 | learner-state measurement substrate + retrieval policy |
| constellation | 6 | compile/derive + read/score/delete |
| contrastive | 4 | mine + read |
| typed_edges | 4 | export + LLM validate + read |

## 3. Protocol Methods
| Method | Behavior |
| --- | --- |
| initialize | MCP session metadata |
| tools/list | Returns ToolDefinition[] (108 with llm, 104 without) |
| tools/call | Dispatches via handlers/tools/dispatch.rs |

## 4. Tool Inventory (Name → One-Line)
### capability
- **get_capability_matrix** — Compact MCP capability map: E1-E14 content embedders, E15-E21 learner slots, tool groups, RocksDB CFs, optional runtime counts.

### core
- **store_memory*** — Store one memory directly without UTL processing. Params: `content*`, `rationale`, `importance`, `sessionId`, `operatorId`.
- **store_memories*** — Batch store an array of memories. Params: `memories*` (array of {content, rationale, importance, sessionId, operatorId}).
- **get_memetic_status** — Fingerprint count, embedder count (14), storage backend/size, layer status.
- **search_graph*** — Multi-space search across 14 embedders. Params: `query*`, `topK`, `minSimilarity`, `strategy`, `weightProfile`, `customWeights`, `excludeEmbedders`, `enableRerank`, `enableAsymmetricE5`, `causalDirection`, `temporalWeight`, `lastHours`, `lastDays`, `sessionId`, `decayFunction`, `temporalScale`, ... .
- **trigger_consolidation** — Identify merge candidates (does NOT auto-merge). Params: `strategy`, `min_similarity`, `max_memories`.

### merge / curation
- **merge_concepts*** — Merge ≥2 nodes; `union | intersection | weighted_average`; reversal_hash for 30-day undo. Params: `source_ids*`, `target_name*`, `merge_strategy`, `rationale*`, `force_merge`.
- **forget_concept*** — Soft-delete (30-day recovery) or permanent. Params: `node_id*`, `soft_delete`, `operator_id`, `reason`.
- **boost_importance*** — Adjust importance by delta; clamps [0.0, 1.0]. Params: `node_id*`, `delta*`, `operator_id`.

### topic
- **get_topic_portfolio** — All discovered topics + stability metrics. Param: `format`.
- **get_topic_stability** — Portfolio churn/entropy/phase breakdown. Param: `hours`.
- **detect_topics** — Force HDBSCAN recalculation (min 3 memories). Params: `force`, `max_memories`.
- **get_divergence_alerts** — Divergence vs recent activity using semantic embedders only. Param: `lookback_hours`.

### file_watcher
- **list_watched_files** — Files with embeddings + chunk counts.
- **get_file_watcher_stats** — Aggregate file watcher stats.
- **delete_file_content*** — Delete embeddings for a path; soft-delete supported. Params: `file_path*`, `soft_delete`.
- **reconcile_files** — Find orphaned files vs disk; `dry_run` to preview.

### sequence (E4)
- **get_conversation_context** — Memories around current turn via E4 ordering.
- **get_session_timeline** — Ordered memory timeline.
- **traverse_memory_chain*** — Multi-hop sequence traversal from `anchorId*`.
- **compare_session_states*** — Compare state at two sequence points. Params: `beforeSequence*`, `afterSequence*`, `sessionId`.

### causal (E5)
- **search_causal_relationships*** — E5 asymmetric similarity + LLM 1-3 paragraph descriptions.
- **search_causes*** — Abductive: find causes of effects (0.8× dampening).
- **search_effects*** — Predictive: find effects of causes (1.2× boost).
- **get_causal_chain*** — Transitive chains from anchor; hop attenuation 0.9^hop.

### causal_discovery (llm)
- **trigger_causal_discovery** — Run Qwen2.5 causal discovery; FP16 on RTX 5090.
- **get_causal_discovery_status** — Agent status, last cycle, VRAM usage.

### keyword / code / robustness
- **search_by_keywords*** — E6 sparse + optional SPLADE expansion.
- **search_code*** — E7 dense (1536D) + optional E1 blend.
- **search_robust*** — E9 HDC typo-tolerant.

### entity (E11 / TransE)
- **extract_entities*** — Pattern + KB canonicalization.
- **search_by_entities*** — E11 + Jaccard hybrid; `any` / `all` modes.
- **infer_relationship*** — TransE r̂ = t - h.
- **find_related_entities*** — TransE outgoing/incoming.
- **validate_knowledge*** — Score (h, r, t) triple via TransE.
- **get_entity_graph** — Inferred relationship graph with weights.

### embedder (cross-cutting + standalone)
- **search_by_embedder*** — Single-embedder primary search. Params: `embedder*`, `query*`, ... .
- **get_embedder_clusters*** — Per-embedder clusters.
- **compare_embedder_views*** — Side-by-side rankings.
- **list_embedder_indexes** — All 14 indexes + stats.
- **get_memory_fingerprint*** — Per-embedder vectors for one memory.
- **create_weight_profile*** — Named custom weight profile (rejects built-in names). Params: `name*`, `weights*`, `description`.
- **search_cross_embedder_anomalies*** — High-score one embedder, low-score another.
- **search_by_tokens*** — E12 ColBERT MaxSim.
- **search_by_expansion*** — E13 SPLADE expansion.

### temporal (E2/E3)
- **search_recent*** — E2 freshness boost (post-retrieval).
- **search_periodic*** — E3 hour/day-of-week match.

### graph (base + llm)
- **search_connections*** — E8 asymmetric source/target connections.
- **get_graph_path*** — Multi-hop E8 paths.
- **discover_graph_relationships*** (llm) — Qwen2.5-3B; 20 relationship types.
- **validate_graph_link*** (llm) — LLM verdict on (source, target).

### graph_link
- **get_memory_neighbors*** — Pre-computed K-NN; embedder_id selectable.
- **get_typed_edges*** — Typed edges (8 types) by direction/edge_type.
- **traverse_graph*** — Multi-hop typed-edge traversal.
- **get_unified_neighbors*** — Weighted RRF fusion across all 14 embedders.

### maintenance / provenance / daemon
- **repair_causal_relationships** — Delete corrupt CF_CAUSAL_RELATIONSHIPS rows.
- **get_audit_trail** — Audit log query.
- **get_merge_history*** — Merge lineage for fingerprint.
- **get_provenance_chain*** — Full embedding→source chain.
- **daemon_status** — Connections, models, uptime, PID.

### training
- **export_training_corpus** — TrainingRecord v3 → CF_TRAINING_RECORDS. Params include `includeEmbeddings`, `includeSparseVectors`, `includeTokenEmbeddings`, `includeEdges`, `includeCausalLabels`, `includeIncomingEdges`, `includeTemporalLabels`, `includeTuckerCore`, `clearExisting`.
- **list_training_records** / **get_training_record*** / **count_training_records**.

### learning (Learning-as-UTL)
- **record_learning_event** — Persist event with before/after 14D topic profile, 91D cross-correlations, outcome label, signals.
- **list_learning_events** / **get_learning_event*** / **count_learning_events**.
- **list_learning_signal_embedders** — Available learner-signal embedder IDs.
- **compute_learning_signals** — Compute deterministic learning features from inputs.
- **embed_learning_event_signals** — Embed signals for a stored event.
- **estimate_learning_outcome*** — Nearest-neighbor outcome prediction over CF_LEARNING_EVENTS.
- **export_learner_training_dataset** — Compile rows into row-major f32 matrix in CF_LEARNER_TRAINING_DATASETS.
- **list_learner_training_datasets** / **get_learner_training_dataset*** / **count_learner_training_datasets**.

### utl (Learner-State Substrate)
- **register_learner** — Profile + consent + modalities.
- **record_session_observation*** — Persist learner-session fingerprint and state vector.
- **compute_delta_s** / **compute_delta_c** / **compute_delta_e** — Components of L.
- **compute_L** — L = ΔS·ΔC·ΔE; optionally persist delta log + M trace.
- **get_learner_M*** — Read persisted M(t).
- **next_review_for_trace*** — Next review timestamp for M trace.
- **list_learner_embedders** — E1-E14 + E15-E21 matrix.
- **preflight_learner_assets** — Verify cached E15-E21 model assets and calibration data.
- **count_learner_state** — Row counts across learner CFs.
- **get_learner_state*** — Profile, fingerprint, state vector, delta log, k_sleep, optional M trace.
- **record_learner_k_sleep*** — Sleep-gated consolidation multiplier.
- **record_learner_retrieval*** — Retrieval-practice row.
- **upsert_goal_centroid*** — Expert goal centroid per (skill, modality).
- **get_goal_distance*** — Compare session embedding to goal centroid.
- **compile_learner_constellation*** — Persist regulated-state baseline.
- **resolve_learner_retrieval_policy*** — Resolve state-conditioned weight profile (affect_repair / affect_priming / affect_neutral / base).

### constellation
- **compile_constellation*** — Aggregate centroids over selector. Selectors: topic / session / tag / time_range / explicit_ids.
- **list_constellations** / **get_constellation*** — Read.
- **score_against_constellation*** — Per-embedder cosine + in_spread_p95.
- **derive_constellation*** — Operations: `interpolate / add / difference / anti_pole`. Persists derived anchor.
- **delete_constellation*** — Idempotent.

### contrastive
- **mine_contrastive_pairs** — Scan for cross-embedder anomaly pairs into CF_CONTRASTIVE_PAIRS.
- **list_contrastive_pairs** / **get_contrastive_pair*** / **count_contrastive_pairs**.

### typed_edges (training)
- **export_typed_edges_corpus** — Each typed edge → TypedEdgeTrainingRecord with similarity profile, optional content/source/mechanism/LLM-validation join.
- **derive_anomalies_from_edges** — Classify edges into 5 AnomalyKind patterns; persists to CF_CONTRASTIVE_PAIRS.
- **validate_low_confidence_edges** — Run LLM on low-weight edges; persists to CF_TYPED_EDGE_VALIDATIONS (skipped without `llm`).
- **list_typed_edge_records** — Paginated read.

## 5. Tool Aliases
| Alias | Canonical |
| --- | --- |
| consolidate_memories | trigger_consolidation |
| inject_context | store_memory |

## 6. AnomalyKinds (contrastive / typed_edges)
- semantic_but_not_causal
- keyword_but_not_paraphrase
- code_shape_but_different_intent
- entity_shared_but_different_structure
- hdc_robust_but_semantic_different
- other (sentinel for unmatched)

## 7. Common Side Effects
| Tool group | Writes |
| --- | --- |
| store_memory(_ies) | CF_FINGERPRINTS, CF_CONTENT, CF_SOURCE_METADATA, all CF_EMB_*, audit log |
| forget_concept | tombstone in CF_SYSTEM, CF_AUDIT_LOG |
| merge_concepts | CF_FINGERPRINTS (target), CF_MERGE_HISTORY, CF_AUDIT_LOG |
| boost_importance | CF_FINGERPRINTS, CF_IMPORTANCE_HISTORY, CF_AUDIT_LOG |
| trigger_causal_discovery | CF_CAUSAL_RELATIONSHIPS, CF_CAUSAL_BY_SOURCE |
| export_training_corpus | CF_TRAINING_RECORDS |
| record_learning_event | CF_LEARNING_EVENTS |
| compile_constellation / derive_constellation | CF_CONSTELLATIONS, CF_CONSTELLATION_BY_SELECTOR |
| mine_contrastive_pairs / derive_anomalies_from_edges | CF_CONTRASTIVE_PAIRS, CF_CONTRASTIVE_BY_KIND/ANCHOR |
| validate_low_confidence_edges | CF_TYPED_EDGE_VALIDATIONS |
| record_session_observation / record_learner_* | CF_FINGERPRINTS_LEARNER, CF_LEARNER_STATE_HISTORY, CF_LEARNER_DELTA_LOG, etc. |
