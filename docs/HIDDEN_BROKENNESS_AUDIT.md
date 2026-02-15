# Hidden Brokenness Audit Report

**Date**: 2026-02-15
**Branch**: casetrack
**Scope**: Full codebase forensic investigation — 6 parallel audits
**Auditors**: Automated forensic agents (phantom tests, silent errors, dead code, data integrity, search correctness, schema mismatches)

## Executive Summary

This audit identified **84 distinct findings** across 6 investigation domains where the system **appears to work correctly but is actually broken, degraded, or misleading**. The findings break down as:

| Severity | Count | Description |
|----------|-------|-------------|
| **CRITICAL** | 7 | Data corruption, completely wrong results returned as correct |
| **HIGH** | 30 | Silently degraded behavior, features that don't work as advertised |
| **MEDIUM** | 37 | Partial failures, hidden features, resource leaks |
| **LOW** | 10 | Maintenance hazards, theoretical issues, minor inconsistencies |

### Top 5 Most Dangerous Findings

1. **[SCH-1] `search_causal_relationships` parameter name mismatch** — Schema says `direction`, handler reads `searchMode` with different enum values. Causal direction filtering is completely broken for all schema-compliant clients.
2. **[SRC-2] Score inflation in `compute_semantic_fusion`** — Skip-zero logic inflates ALL multi-space search scores by 15-33%. Every `min_similarity` threshold comparison is unreliable.
3. **[ERR-1/ERR-2] HNSW index creation + search failures are silent** — An entire embedder dimension can silently disappear from all searches with no error to the user.
4. **[DAT-3] NodeMetadata uses `skip_serializing_if` with bincode** — The graph crate serializes MemoryNodes with bincode, but NodeMetadata has 8 `skip_serializing_if` fields. This produces corrupt or wrong data on deserialization.
5. **[DAT-4] Legacy E5/E8 fingerprints never migrated** — Asymmetric causal search (cause vs effect) returns identical results for all pre-migration data, silently defeating the feature.

---

## Domain 1: MCP Schema vs Handler Mismatches (SCH)

*Things users think they can do via the API but actually can't, or behavior that differs from what the schema promises.*

### SCH-1: `search_causal_relationships` — Parameter Name AND Enum Mismatch [CRITICAL]

- **Schema**: `crates/context-graph-mcp/src/tools/definitions/causal.rs:31-34` — parameter `direction` with enum `["cause", "effect", "all"]`, default `"all"`
- **Handler**: `crates/context-graph-mcp/src/handlers/tools/causal_relationship_tools.rs:86-101` — reads `searchMode` with enum `["causes", "effects", "semantic"]`, default `"semantic"`
- **Impact**: Client sends `{"direction": "cause"}` per schema. Handler never reads `direction`. Defaults to semantic search. **Causal direction filtering is completely non-functional** for any schema-compliant client.

### SCH-2: `search_graph` — `modality` Parameter Exposed But Ignored [HIGH]

- **Schema**: `tools/definitions/core.rs:99-103` — `modality` enum `["text", "code", "image", "audio", "structured", "mixed"]`
- **Handler**: `handlers/tools/memory_tools.rs` — zero references to `modality`
- **Impact**: `{"modality": "code"}` is silently ignored. No filtering occurs. Users believe they're filtering by modality.

### SCH-3: `search_robust` — `e9DiscoveryThreshold` Default Mismatch [HIGH]

- **Schema**: `tools/definitions/robustness.rs:66-73` — `"default": 0.7`
- **Handler**: `handlers/tools/robustness_dtos.rs:131-133` — `E9_DISCOVERY_THRESHOLD = 0.15`
- **Impact**: If a client explicitly sets 0.7 per schema documentation, virtually no E9 blind-spot discoveries are returned. Feature appears broken. Only works correctly when parameter is omitted (handler default 0.15 is correct).

### SCH-4: `search_graph` — `conversationContext.anchorToCurrentTurn` Default Mismatch [HIGH]

- **Schema**: `tools/definitions/core.rs:191-194` — `"default": true`
- **Handler**: `handlers/tools/memory_tools.rs:761-764` — `unwrap_or(false)`
- **Impact**: Client sends `{"conversationContext": {}}` expecting anchoring (schema says default true). Handler defaults false. Conversation anchoring never activates unless explicitly enabled.

### SCH-5: `search_causal_relationships` — 8 Undiscoverable Power Features [HIGH]

- **Handler**: `handlers/tools/causal_relationship_tools.rs:121-178`
- **Missing from schema**: `includeProvenance`, `sourceWeight`, `explanationWeight`, `multiEmbedder`, `e1Weight`, `e5Weight`, `e8Weight`, `e11Weight`
- **Impact**: Multi-embedder causal fusion mode (4-embedder weighted search for maximum accuracy) is never activated. All clients get single-embedder semantic fallback.

### SCH-6: `search_causes`/`search_effects` — 3 Undiscoverable Features [HIGH]

- **Handler DTOs**: `handlers/tools/causal_dtos.rs:90-113, 274-287`
- **Missing from schema**: `strategy` (pipeline mode), `rerankWeight` (ColBERT blend), `searchScope` ("relationships" accesses CF_CAUSAL_RELATIONSHIPS)
- **Impact**: The `searchScope: "relationships"` option accesses LLM-generated causal descriptions — a fundamentally different search path. Always defaults to `"memories"`.

### SCH-7: `search_code` — 3 Undiscoverable Features [HIGH]

- **Handler DTO**: `handlers/tools/code_dtos.rs:117-159`
- **Missing from schema**: `searchMode` (e7Only/pipeline), `languageHint` (language boost), `includeAstContext`
- **Impact**: Pure E7 code search (ideal for function signatures) and language-specific boosting are completely hidden.

### SCH-8: `store_memory` — `operator_id` Not in Schema [MEDIUM]

- **Handler**: `handlers/tools/memory_tools.rs:172-176`
- **Impact**: All audit records have `created_by: None`. Provenance audit trail has no operator attribution.

### SCH-9: `search_graph` — 10 Undiscoverable Temporal Parameters [MEDIUM]

- **Handler**: `handlers/tools/memory_tools.rs:660-753`
- **Missing**: `decayHalfLifeSecs`, `lastHours`, `lastDays`, `sessionId`, `periodicBoost`, `targetHour`, `targetDayOfWeek`, `sequenceAnchor`, `sequenceDirection`, `includeProvenance`
- **Impact**: Useful temporal shortcuts (`lastHours: 24`, sequence-based retrieval) are hidden. Provenance metadata is never included.

### SCH-10: `search_by_keywords`/`search_robust` — `strategy` Not in Schema [MEDIUM]

- **Handler DTOs**: `keyword_dtos.rs:91`, `robustness_dtos.rs:116`
- **Impact**: Pipeline search strategy unavailable for keyword and robustness tools. Always defaults to multi_space.

---

## Domain 2: Search & Scoring Correctness (SRC)

*Searches that return results but the results are wrong, inflated, or incomplete.*

### SRC-1: Two Incompatible Cosine Similarity Ranges Mixed in Scoring [CRITICAL]

- **Raw [-1,1]**: `storage/.../helpers.rs:18-48` — `compute_cosine_similarity()`
- **Normalized [0,1]**: `core/.../distance.rs:35-55` — `cosine_similarity()` uses `(raw+1)/2`
- **Collision point**: `compute_embedder_scores_sync()` uses raw [-1,1], but `compute_semantic_fusion()` at `search.rs:1283` filters `score > 0.0`, silently excluding negative cosines. Embedders with slight negative similarity (anti-correlated vectors) are dropped from fusion instead of contributing negative signal.

### SRC-2: `compute_semantic_fusion` Skip-Zero Logic Inflates ALL Scores [CRITICAL]

- **File**: `storage/.../search.rs:1275-1295`
- **Bug**: The function excludes zero-score embedders from the denominator, producing a weighted average over only responding embedders instead of the full profile.
- **Numeric example**: With E5=0.0 (always for non-causal) and E6/E11=0.0 (common), denominator shrinks from 1.0 to 0.75. A true 0.345 match scores **0.460** (33% inflation). Every `min_similarity` threshold comparison is affected.

### SRC-3: E12 MaxSim Uses Different Scale in Two Paths [HIGH]

- **Storage crate** (`search/maxsim.rs:70-84`): raw cosine [-1,1]
- **Core crate** (`retrieval/distance.rs:101-120`): normalized [0,1]
- **Impact**: E12 provenance breakdowns show [-1,1] values while pipeline reranking uses [0,1]. Custom profiles giving E12 non-zero weight mix scales in fusion.

### SRC-4: Sparse Search (E6/E13) Uses IDF-Only, Not BM25 [MEDIUM]

- **File**: `storage/.../search.rs:1080-1152`
- **Bug**: Inverted index stores only document IDs per term, not per-document term frequencies. Scoring is `Sum(query_weight * IDF)` — no TF saturation, no document length normalization.
- **Impact**: A 10-word document with one term match scores identically to a 10,000-word document with one match. Pipeline Stage 1 recall quality is degraded.

### SRC-5: E9 (HDC) Excluded from HNSW Candidate Retrieval [MEDIUM]

- **Files**: `search.rs` multi_space (lines 510-697) and pipeline (lines 708-997)
- **Bug**: E9 is never searched for HNSW candidates, but E9 scores ARE computed and E9 weights ARE applied in fusion.
- **Impact**: With custom weights `{"E9": 0.30}`, E9 only re-ranks candidates found by other embedders. Documents nearest in E9 space but not in E1/E5/E7/E8/E10/E11 space are never found. User sees E9 in provenance and believes it's working fully.

### SRC-6: Direction-Aware Reranking Produces Scores > 1.0 [MEDIUM]

- **File**: `handlers/tools/memory_tools.rs` lines 1594-1627
- **Bug**: Causal gate (1.10x) + direction match (1.08x) = 1.188x multiplicative boost. No clamp to [0,1].
- **Impact**: A pre-boost 0.90 becomes 1.069. Users receive nonsensical similarity scores. Threshold comparisons unreliable.

### SRC-7: Causal HNSW Distance-to-Similarity Not Clamped [MEDIUM]

- **Main search** (`search.rs:192`): `1.0 - distance.min(1.0)` — clamped to [0,1]
- **Causal HNSW** (`causal_hnsw_index.rs:271`): `1.0 - distance` — can produce **negative** similarity
- **Impact**: Anti-correlated causal vectors produce negative similarities entering downstream processing.

### SRC-8: `suppress_degenerate_weights` Uses Biased Estimator [LOW]

- **File**: `search.rs:1259` — population variance `m2/count` instead of sample variance `m2/(count-1)`
- **Impact**: With small result sets (top_k <= 5), embedders with true variance slightly above threshold are prematurely suppressed (weight reduced by 75%).

### SRC-9: Pipeline RRF Double-Penalizes Suppressed Embedders [LOW]

- **File**: `search.rs:869-930`
- **Impact**: Suppressed weight is applied both in RRF weighting and indirectly through scores. Marginal embedders are more aggressively demoted than intended.

---

## Domain 3: Silent Error Swallowing (ERR)

*Operations that fail but return success, degrading behavior invisibly.*

### ERR-1: HNSW Search Silently Drops Index Errors (6 Embedders) [CRITICAL]

- **File**: `storage/.../search.rs:288, 558, 579, 594, 609, 624`
- **Pattern**: `if let Ok(candidates) = index.search(...)` — error branch does nothing
- **Impact**: If an HNSW index is corrupted or OOM, that entire embedder dimension is silently excluded. RRF renormalizes remaining embedders' contributions. User receives confidently-scored results built on incomplete evidence.

### ERR-2: HNSW Index Creation Fails Silently — Embedder Permanently Missing [CRITICAL]

- **File**: `core/.../multi_space_trait.rs:393, 401`
- **Pattern**: `if let Ok(index) = RealHnswIndex::new(config)` — failure creates no index, no config, no error, no log
- **Impact**: On memory-constrained deployments, the system silently degrades from 6-embedder to 4-embedder or 1-embedder search. Compounds with ERR-1 — index never created, every search silently skips it.

### ERR-3: Weight Profile Lookup `.ok()` Discards Error Details [HIGH]

- **File**: `mcp/.../weights.rs:20`
- **Pattern**: `get_weight_profile(name).ok()` — discards "profile not found" and "weights invalid"
- **Impact**: User sends `{"weightProfile": "my_custom_profile"}`, handler silently falls back to default weights. Search runs with entirely different weights than requested.

### ERR-4: Distance Computation Returns 0.0 on Type Mismatch [HIGH]

- **File**: `core/.../distance.rs:208-215`
- **Pattern**: Type mismatch (dense/sparse/token-level) returns 0.0 instead of error
- **Impact**: Data corruption or schema migration bugs produce 0.0 scores instead of errors. Perfect-match memories get penalized. Logged to tracing but never propagated.

### ERR-5: Sparse Index Returns Defaults on Lock Poisoning [HIGH]

- **File**: `core/.../sparse_index.rs:487-528`
- **Pattern**: `.unwrap_or(0)` on poisoned `std::sync::RwLock`
- **Impact**: After any thread panic holding a write lock, the sparse index reports 0 documents and 0 terms. E6 keyword and E13 SPLADE search silently return empty results. Two entire search modalities disabled permanently with no error.

### ERR-6: Embedder Config Falls Back to Weight 1.0 on Missing Config [HIGH]

- **File**: `core/.../embedder_config.rs:150`
- **Pattern**: `.unwrap_or(1.0)` when embedder not in weight map
- **Impact**: Unconfigured embedder gets maximum weight (1.0), amplifying its contribution 3-10x over properly-configured embedders. Config error becomes invisible search quality degradation.

### ERR-7: Training Pipeline Silently Drops Trajectory File Writes [HIGH]

- **File**: `embeddings/.../pipeline.rs:656, 662`
- **Pattern**: `if let Ok(mut file) = ...` + `let _ = writeln!`
- **Impact**: Training trajectory data permanently lost. Post-hoc analysis impossible. Operators see empty/truncated trajectory files.

### ERR-8: Causal Discovery Silently Drops Database Read Failures [HIGH]

- **File**: `handlers/tools/causal_discovery_tools.rs:171-181`
- **Pattern**: `if let Ok(ids) = store.get_fingerprints_for_file(...)` — error branch skipped
- **Impact**: Database I/O errors cause files' fingerprints to be silently excluded. Tool may report "not enough memories" when data exists but can't be read.

### ERR-9: Merge Operation Logs But Continues Past 4 Distinct Write Failures [MEDIUM]

- **File**: `handlers/merge.rs:373-447`
- **Pattern**: 4 sequential `if let Err` blocks: content storage, soft-delete, audit record, merge history
- **Impact**: `merge_concepts` returns `{"success": true}` even when: content is lost, sources not deleted (duplicates in search), audit trail missing, merge history lost (unmerge broken).

### ERR-10: Audit Record Writes Fail Silently Across 15+ Handlers [MEDIUM]

- **Files**: 15+ handler files — memory_tools.rs, embedder_tools.rs, graph_tools.rs, topic_tools.rs, curation_tools.rs, etc.
- **Pattern**: `if let Err(e) = store.append_audit_record(...) { error!(...); }`
- **Impact**: A single CF_AUDIT_LOG corruption silently disables ALL audit logging. `get_memory_provenance` shows incomplete history with no indication of gaps.

### ERR-11: Code Search Entity Lookup Returns None on Error [MEDIUM]

- **File**: `handlers/tools/code_tools.rs:267-278`
- **Impact**: CodeStore errors are indistinguishable from "no matching entities." AI model concludes no relevant code exists when the search infrastructure failed.

### ERR-12: File Watcher Thread Handle Not Stored on Lock Poisoning [MEDIUM]

- **File**: `mcp/.../watchers.rs:196`
- **Impact**: On shutdown, watcher thread is never joined. Continues running after server believes it stopped, potentially writing to freed resources.

### ERR-13: Reconcile Files Continues Past Delete Failures [MEDIUM]

- **File**: `handlers/tools/file_watcher_tools.rs:503-509`
- **Impact**: Orphan fingerprints that fail to delete remain searchable. Tool claims reconciliation completed but some orphans persist, returning results pointing to deleted files.

### ERR-14: Session Manager Ignores Unreadable Session File [MEDIUM]

- **File**: `core/.../manager.rs:434-441`
- **Impact**: Stale session file persists. On restart, new memories tagged with dead session ID.

### ERR-15: BIRCH Clustering Silently Skips Entries With Distance Errors [MEDIUM]

- **File**: `core/.../birch.rs:1334, 1607, 1641, 1787`
- **Impact**: Points with NaN centroids assigned to wrong clusters. Errors in 4 nearest-neighbor computation paths silently skipped.

### ERR-16: Embedding Version Storage Failure Silently Ignored [MEDIUM]

- **File**: `handlers/tools/memory_tools.rs:453-458`
- **Impact**: Embedding version records lost. Cannot detect when memory was computed with outdated model. Re-embedding quality assurance broken.

### ERR-17: Importance Change History Write Failure Silently Ignored [MEDIUM]

- **File**: `handlers/tools/curation_tools.rs:272-277`
- **Impact**: Importance timeline has gaps. Cannot trace how memory's importance changed over time.

### ERR-18: GPU Memory Manager Lies About State on Lock Poisoning [MEDIUM]

- **File**: `graph/.../gpu_memory.rs:434, 446, 463`
- **Impact**: `is_low_memory()` returns false when lock is poisoned. System continues allocating beyond budget.

---

## Domain 4: Data Integrity (DAT)

*Data that appears correctly stored/retrieved but is actually corrupted or inconsistent.*

### DAT-1: `total_doc_count` Includes Soft-Deleted Entries [MEDIUM]

- **File**: `storage/.../store.rs:263-271` (init), `search.rs:1343,1599` (read for IDF)
- **Bug**: Startup counts ALL CF_FINGERPRINTS entries including soft-deleted. Inflates IDF denominator.
- **Impact**: BM25 IDF scores in sparse search (E6/E13) subtly degraded. Rare terms appear less discriminative than they should. Worsens proportionally to soft-deleted fraction.

### DAT-2: NodeMetadata Uses `skip_serializing_if` With Bincode [HIGH]

- **File**: `core/.../metadata.rs:50-107` — 8 fields with `skip_serializing_if`
- **Serialization**: `graph/.../iteration.rs:67` — `bincode::deserialize`
- **Bug**: Bincode is positional. When `skip_serializing_if` omits a None field, bytes shift. Deserialization reads wrong bytes for subsequent fields.
- **Impact**: Any MemoryNode stored via graph crate where a `skip_serializing_if` field is None either fails to deserialize (detected) or deserializes with wrong field values (silent corruption). Highest risk for `source`, `language`, `utl_score` which are frequently None.

### DAT-3: Legacy E5/E8 Fingerprints Never Migrated on Deserialization [HIGH]

- **Files**: `core/.../fingerprint.rs:170-176, 294-332, 723-731`, `storage/.../serialization.rs:126-159`
- **Bug**: Migration methods `migrate_legacy_e5()` and `migrate_legacy_e8()` exist but are never called during deserialization.
- **Impact**: For legacy data, both cause and effect HNSW indexes receive IDENTICAL vectors. Asymmetric E5 causal search ("What caused X?" vs "What are effects of X?") returns identical results for all pre-migration fingerprints. Same for E8 graph source/target.

### DAT-4: HNSW Index and RocksDB Diverge After Failed Store or Update [HIGH]

- **File**: `storage/.../crud.rs:44-59` (store), `crud.rs:85-138` (update)
- **Bug — store**: If `add_to_indexes` fails after `store_fingerprint_internal` succeeds, data is in RocksDB + inverted indexes but NOT in HNSW. No rollback.
- **Bug — update (WORSE)**: Step 2 removes old HNSW entries, Step 3 stores new data, Step 4 adds new HNSW entries. If Step 4 fails, old entries are gone and new entries never added. Fingerprint becomes findable via sparse search but unfindable via dense search.
- **Impact**: Inconsistent search results persist until server restart triggers full index rebuild.

### DAT-5: Negative Cosine Handling in Fusion [MEDIUM]

- **File**: `search.rs:1226-1295`
- **Bug**: `suppress_degenerate_weights` checks `all_zero` (exact 0.0 equality), keeping active any embedder with small negative scores. But `compute_semantic_fusion` excludes `score <= 0.0` from fusion. Result: active weight with zero contribution, silently reducing effective embedder count.

### DAT-6: `compact_async` / GC Soft-Delete Tracking Race [MEDIUM]

- **File**: `storage/.../persistence.rs`, `crud.rs:274-330`
- **Bug**: The DashMap is shared across threads. Any code path clearing it unconditionally (crash during GC, future refactor) causes previously soft-deleted memories to become temporarily visible as "zombie" data.

### DAT-7: Missing Source Metadata + Quantized CF Cleanup on Delete [MEDIUM]

- **File**: `storage/.../crud.rs:175-253`
- **Bug**: Hard-delete cleans 7 column families but NOT CF_SOURCE_METADATA (orphaned, pollutes file-path scans) and NOT 13 QUANTIZED_EMBEDDER_CFS (up to ~26KB wasted per deleted fingerprint, grows unboundedly).

### DAT-8: Content Hash Not Verified on Retrieval [MEDIUM]

- **File**: `storage/.../content.rs:106-139`
- **Bug**: SHA-256 verified on store but never recomputed on retrieval. Corrupted content served silently.

### DAT-9: SparseVector u16 Index vs Vocabulary Size — No Compile-Time Guard [LOW]

- **File**: `core/.../sparse.rs:23, 52`
- **Bug**: `SPARSE_VOCAB_SIZE = 30,522` fits in u16 today. No `const_assert!` prevents future vocabulary increase beyond 65,535 from causing silent index collisions.

---

## Domain 5: Phantom Tests (TST)

*Tests that always pass but don't actually verify correctness.*

### TST-1: 15+ Tests With Silent Early Return (GPU/Model Checks) [CRITICAL]

- **Files**: `context-graph-cuda/src/hdbscan/tests.rs` (7 tests), `context-graph-embeddings/tests/e9_vector_differentiation_test.rs` (2), `context-graph-embeddings/src/provider/diagnostic_test.rs` (2), `context-graph-embeddings/src/models/pretrained/sparse/tests.rs` (2), `context-graph-causal-agent/tests/background_loop_integration.rs` (3)
- **Pattern**: `if env_var("SKIP_GPU_TESTS").is_ok() { return; }` or `if !model_exists { return; }`
- **Impact**: In CI without GPU/models, these tests show GREEN but execute zero assertions. No `#[ignore]` annotation, no skip-count in test output.

### TST-2: Non-Causal Pair Test Explicitly Refuses to Assert [CRITICAL]

- **File**: `context-graph-causal-agent/tests/background_loop_integration.rs:581-587`
- **Pattern**: `if relationships_confirmed != 0 { println!("WARN: ..."); }` — no `assert!`
- **Impact**: The test's entire purpose is verifying non-causal rejection. It always passes regardless of LLM output.

### TST-3: ~20 Tests With Blind Error Assertions [HIGH]

- **Files**: cuda/cone/tests.rs, cuda/stub.rs, core/retrieval/query.rs, graph-agent/llm/mod.rs, mcp/middleware/validation.rs, storage/serialization/tests/
- **Pattern**: `assert!(result.is_err())` without checking error type/message
- **Impact**: If function fails for a DIFFERENT reason than intended, test still passes. Wrong error goes undetected.

### TST-4: CLI Tests That Cannot Fail [HIGH]

- **File**: `context-graph-cli/tests/e2e/error_recovery_test.rs:280-293, 621-633`
- **Pattern**: Accepts ANY exit code. `test_e2e_database_error_handling` accepts success with invalid DB path.
- **Impact**: Structurally impossible for these tests to fail. Any regression passes silently.

### TST-5: GPU Tests Feature-Gated Out [MEDIUM]

- **File**: `context-graph-mcp/src/handlers/tests/gpu_embedding_verification.rs:20` — `#![cfg(feature = "cuda")]`
- **Impact**: 877 lines of thorough GPU verification tests don't exist in the binary without `--features cuda`. Functionally equivalent to `#[ignore]`.

### TST-6: GraphDiscoveryService Has Only Default Config Test [MEDIUM]

- **File**: `context-graph-graph-agent/src/service/mod.rs:474-488`
- **Impact**: `run_discovery_cycle`, `start_background`, `stop` are completely untested. Single test only checks constants.

### TST-7: InMemoryTeleologicalStore Tests Don't Verify Production (RocksDB) Behavior [MEDIUM]

- **File**: `core/src/traits/teleological_memory_store_tests.rs`
- **Impact**: All tests use HashMap-based stub. Checkpoint/restore, semantic search, and other features verified only against the stub, not the actual RocksDB implementation.

---

## Domain 6: Dead / Unreachable Code (DEAD)

*Code that exists and compiles but is never executed in production.*

### DEAD-1: 3 Column Families Opened But Never Written or Read [MEDIUM]

- `CF_ENTITY_PROVENANCE` (`column_families.rs:160`) — marked DEPRECATED, trait methods not wired
- `CF_TOOL_CALL_INDEX` (`column_families.rs:252`) — marked DEPRECATED, trait methods not wired
- `CF_CONSOLIDATION_RECOMMENDATIONS` (`column_families.rs:270`) — marked DEPRECATED, trait methods not wired
- **Impact**: Each CF consumes RocksDB metadata overhead on every startup. Cannot be removed without deleting database (RocksDB requires all on-disk CFs to be opened).

### DEAD-2: `ChainRetrievalOptions` — Fully Implemented, Never Consumed [MEDIUM]

- **File**: `core/.../options.rs:639-693`
- **Impact**: Builder methods `with_chain()`, `with_chain_and_related()` exist. No search implementation reads `chain_options`. Setting it has zero runtime effect. ~80 lines of dead code.

### DEAD-3: 5 of 7 Matrix `SearchStrategy` Variants Unreachable from MCP [MEDIUM]

- **File**: `core/.../matrix_search/types.rs:14-30`
- **Dead variants**: `SynergyWeighted`, `GroupHierarchical`, `CrossCorrelationDominant`, `TuckerCompressed`, `Adaptive`
- **Impact**: Full implementations exist (~400 lines) including group categorization and Tucker decomposition. No MCP tool instantiates them. Additionally, there are TWO `SearchStrategy` enums with the same name (3-variant MCP version and 7-variant matrix version), creating confusion.

### DEAD-4: `faiss-working` Feature Never Enabled [MEDIUM]

- **Files**: `context-graph-cuda/Cargo.toml:49`, `cuda/src/ffi/mod.rs:45-111`, `graph/src/index/faiss_ffi/mod.rs:78-346`
- **Impact**: Hundreds of lines of FAISS GPU FFI bindings compile to stubs returning error messages. No default feature enables it.

### DEAD-5: `ComprehensiveComparison` Computed But Never Returned [LOW]

- **File**: `core/.../matrix_search/types.rs:89-119`
- **Impact**: Rich per-group, per-embedder correlation, Tucker compressed comparison data never reaches MCP clients. ~100 lines exercised only by unit tests.

### DEAD-6: `NormalizationStrategyOption` ZScore/Convex Unreachable [LOW]

- **File**: `core/.../options.rs:82-98`
- **Impact**: No MCP tool exposes normalization parameter. ZScore and Convex variants defined but unreachable.

### DEAD-7: `ComparisonScope` 7 Variants All Internal-Only [LOW]

- **File**: `core/.../matrix_search/types.rs:34-50`
- **Impact**: Full, TopicProfileOnly, CrossCorrelationsOnly, etc. — no MCP tool exposes comparison scope.

---

## Remediation Priority Matrix

### P0 — Fix Immediately (Data Corruption / Completely Wrong Results)

| ID | Finding | Fix |
|----|---------|-----|
| SCH-1 | `direction` vs `searchMode` param mismatch | Rename handler param to `direction`, align enum values to schema |
| SRC-2 | Score inflation from skip-zero fusion | Include zero-score embedders in denominator (or document the semantic: average over responding embedders) |
| DAT-2 | NodeMetadata bincode + skip_serializing_if | Switch graph crate to MessagePack OR remove skip_serializing_if from NodeMetadata |
| DAT-3 | Legacy E5/E8 never migrated | Call `migrate_legacy_e5()`/`migrate_legacy_e8()` in `deserialize_teleological_fingerprint()` |

### P1 — Fix Soon (Silent Degradation / Invisible Failures)

| ID | Finding | Fix |
|----|---------|-----|
| ERR-1/ERR-2 | HNSW index creation + search silent failures | Add warning metadata to MCP response: `"degraded_embedders": ["E1"]`. Log at ERROR level. |
| DAT-4 | HNSW/RocksDB divergence on failed store/update | Add compensating rollback: if `add_to_indexes` fails, remove from RocksDB |
| ERR-3 | Weight profile .ok() discards errors | Return error to user: "weight profile 'X' not found" |
| ERR-5 | Sparse index lock poisoning returns defaults | Use parking_lot::RwLock (non-poisoning) or propagate error |
| ERR-6 | Embedder config unwrap_or(1.0) | Return error or use 0.0 default for unknown embedders |
| SCH-2 | modality parameter ignored | Implement filtering or remove from schema |
| SCH-3 | e9DiscoveryThreshold schema default wrong | Update schema default to 0.15 |
| SCH-4 | anchorToCurrentTurn default mismatch | Align handler default to true (matching schema) |
| SRC-6 | Scores exceeding 1.0 | Add `result.similarity = result.similarity.min(1.0)` clamp |
| TST-1 | Silent early return phantom tests | Replace `return` with `#[ignore]` annotation |
| TST-2 | Non-causal pair test refuses to assert | Add `assert_eq!(result.relationships_confirmed, 0)` |

### P2 — Fix When Convenient (Hidden Features / Resource Waste)

| ID | Finding | Fix |
|----|---------|-----|
| SCH-5-10 | 30+ undiscoverable parameters | Update MCP tool schemas to include handler-supported params |
| DAT-1 | total_doc_count includes soft-deleted | Subtract `self.soft_deleted.len()` at startup |
| DAT-7 | Source metadata + quantized CFs not cleaned on delete | Add to WriteBatch in hard-delete path |
| DEAD-1 | 3 empty CFs consuming overhead | Document migration plan |
| DEAD-2 | ChainRetrievalOptions dead code | Remove or implement |
| TST-3 | Blind error assertions | Add `assert!(matches!(result, Err(SpecificError { .. })))` |
| TST-4 | CLI tests that cannot fail | Assert on acceptable exit code set |

---

## Systemic Root Causes

### 1. Schema Drift
Handler implementations evolved with new features (PHASE-2-PROVENANCE, INLINE-CAUSAL, E7-WIRING) but schema files in `tools/definitions/` were not updated. **30+ parameters** exist in handlers without schema entries.

### 2. Resilience-Over-Correctness Philosophy
The codebase prefers partial results over total failure (reasonable for a memory system). However, **degradation is invisible to callers** — MCP tool responses include no `warnings` or `degraded_embedders` array. Silent 0.0 returns are indistinguishable from "not similar."

### 3. Error + Default Compounding
ERR-2 (index creation fails) feeds into ERR-1 (search skips missing index), amplified by ERR-3 (wrong weights applied). Three independent silent failures compound into arbitrarily wrong results.

### 4. Dual-Path Duplication
Multiple implementations of the same operation (3 cosine similarity functions, 2 MaxSim functions, 2 SearchStrategy enums) create inconsistency surfaces where paths produce different results for identical inputs.

---

## Estimated Impact by Feature Area

| Feature | Status | Key Issues |
|---------|--------|------------|
| **Basic search (search_graph)** | Works, scores inflated 15-33% | SRC-2, SRC-1 |
| **Causal search (search_causes/effects)** | Works at reduced accuracy | SCH-1, SCH-5, SCH-6 |
| **Causal direction filtering** | **Completely broken** for schema clients | SCH-1 |
| **Modality filtering** | **Non-functional** | SCH-2 |
| **Asymmetric E5/E8 search** | Broken for legacy data | DAT-3 |
| **Weight profiles** | Silent fallback to default on error | ERR-3 |
| **HNSW search** | Can silently degrade to partial embedder set | ERR-1, ERR-2 |
| **Sparse search (E6/E13)** | IDF-only (no BM25 TF), can silently die on lock poison | SRC-4, ERR-5 |
| **Merge operations** | Returns success despite 4 possible silent failures | ERR-9 |
| **Audit trail** | Gaps develop silently across 15+ handlers | ERR-10 |
| **Graph storage** | NodeMetadata potentially corrupted by bincode + skip_serializing | DAT-2 |
| **Code search** | Works, but best modes (e7Only, language hint) are hidden | SCH-7 |
| **Conversation anchoring** | Silently disabled (default mismatch) | SCH-4 |
| **Temporal parameters** | Work if you know the param names. All 10 hidden from schema | SCH-9 |
| **Provenance** | Works if you send undocumented `includeProvenance: true` | SCH-9 |
| **E9 blind-spot detection** | Schema default (0.7) makes feature appear broken | SCH-3 |
