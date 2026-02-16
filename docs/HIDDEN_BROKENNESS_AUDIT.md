# Hidden Brokenness Audit Report

**Date:** 2026-02-15
**Branch:** casetrack
**Scope:** Full codebase forensic investigation across 5 dimensions
**Method:** 5 parallel Sherlock Holmes forensic agents, each examining a different subsystem

---

## Executive Summary

Despite 5,416 passing tests and zero compiler warnings, this audit uncovered **52 distinct findings** where code appears to work correctly but is actually broken, degraded, or deceptive. The findings break down as:

| Severity | Count | Description |
|----------|-------|-------------|
| CRITICAL | 1 | Zero MCP E2E test coverage without CUDA feature |
| HIGH | 14 | Data corruption, silent failures, misleading results |
| MEDIUM | 25 | Degraded behavior, masked errors, inconsistent handling |
| LOW | 12 | Edge cases, maintenance hazards, minor inconsistencies |

### Top 5 Most Dangerous Findings

1. **Double pagination bug** in `get_session_timeline` -- any non-zero offset returns wrong/empty results (MCP-6)
2. **Sparse cosine normalization mismatch** -- E6/E13 scores in [-1,1] while all other embedders use [0,1], causing systematic under-weighting (SEARCH-1)
3. **MemoryStore session index race condition** -- concurrent writes to same session can silently lose index entries (DATA-6)
4. **Consolidation on empty content** -- batch fetch failure substitutes empty strings, merging unrelated memories (ERR-3)
5. **All MCP E2E tests gated behind `#[cfg(feature = "cuda")]`** -- zero handler coverage in standard CI (TEST-7)

---

## Section 1: Data Integrity Issues (6 findings)

### DATA-1: Non-Atomic MemoryStore Write [MEDIUM]
**Files:** `crates/context-graph-core/src/memory/store.rs:209-306`

The `store()` method performs 3 separate RocksDB writes without WriteBatch:
1. `put_cf(cf_memories, ...)` -- writes the memory
2. `put_cf(cf_session_index, ...)` -- updates session index
3. `put_cf(cf_file_index, ...)` -- updates file index

A crash between writes creates **orphan memories** -- stored but invisible to session/file queries. The code acknowledges this as "Phase 1" debt (lines 199-203). The `delete()` method (lines 514-578) has the reverse problem: deleting memory before updating session index can leave **dangling index references**.

**Mitigation in place:** Read path skips orphaned references silently (line 474).

### DATA-2: Non-Atomic delete_causal_relationship [MEDIUM]
**Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/causal_relationships.rs:992-1029`

Three independent operations without WriteBatch:
1. `self.db.delete_cf(cf, key)` -- deletes from primary CF
2. `self.remove_from_causal_by_source_index(...)` -- updates secondary index
3. `self.causal_e11_index.remove(id)` -- removes from HNSW

Contrast: `store_causal_relationship()` (lines 150-166) correctly uses WriteBatch. The asymmetry means the store path is atomic but the delete path is not.

### DATA-3: Incomplete Rollback in store_async [MEDIUM]
**Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs:44-78`

On HNSW insertion failure, rollback only deletes from `CF_FINGERPRINTS`. But `store_fingerprint_internal()` writes to **5 column families** via WriteBatch (CF_FINGERPRINTS, CF_E1_MATRYOSHKA_128, CF_E13_SPLADE_INVERTED, CF_E6_SPARSE_INVERTED, CF_E12_LATE_INTERACTION). Rollback leaves orphaned data in 4 CFs.

### DATA-4: CodeStore Non-Atomic Multi-CF Writes [MEDIUM]
**Files:** `crates/context-graph-storage/src/code/store.rs:170-229`

5 separate `put_cf` calls (entity, fingerprint, file index, name index, signature index) without WriteBatch. A crash mid-store can leave entities without fingerprints (unsearchable) or present but invisible to queries.

### DATA-5: HNSW Compaction Race Condition [LOW]
**Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs:819-845`

`compact_hnsw_if_needed()` calls `rebuild_indexes_from_store()` which iterates all fingerprints and re-inserts into HNSW. Concurrent `store_async()` during rebuild creates duplicate usearch entries. Already documented in MEMORY.md.

### DATA-6: MemoryStore Session Index Race Condition [HIGH]
**Files:** `crates/context-graph-core/src/memory/store.rs:247-298`

Read-modify-write cycle on session index with NO locking:
1. Read existing session_ids from cf_session_index
2. Push new memory ID
3. Write updated list back

Two concurrent stores to the same session both read the same initial list, both append, both write back -- second writer **overwrites** first writer's addition. Classic lost-update problem. The TeleologicalStore has `secondary_index_lock` (parking_lot::Mutex) for this exact reason -- MemoryStore has no equivalent.

Same race exists in `update_file_index()` (lines 327-369) and `delete()` (lines 544-574).

---

## Section 2: Search & Embedding Pipeline Issues (7 findings)

### SEARCH-1: SparseVector cosine_similarity Returns [-1,1] While Dense Returns [0,1] [HIGH]
**Files:** `crates/context-graph-core/src/types/fingerprint/sparse.rs:234-244`

`SparseVector::cosine_similarity()` returns raw cosine in [-1, 1]:
```rust
dot / (norm_self * norm_other)  // Returns [-1, 1], NOT [0, 1]
```

Called directly in `compute_embedder_scores_sync` (search.rs:403, 410) for E6 and E13. All other embedder scores in the same array are normalized to [0, 1] via `(raw + 1.0) / 2.0`. This means E6/E13 contribute LESS to fusion than their weights suggest -- a cosine of 0.8 stays at 0.8 for E6/E13 but becomes 0.9 for E1-E5, E7-E11.

In practice, SPLADE terms are non-negative so raw cosine stays in [0, 1], but orthogonal items score 0.0 (raw) instead of 0.5 (normalized), making E6/E13 appear less discriminating.

### SEARCH-2: E9 (HDC) Has HNSW Index But Is Never Queried for Candidates [MEDIUM]
**Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs:521+`

`search_multi_space_sync` explicitly searches E1, E5, E7, E8, E10, E11 -- but NOT E9. E9 has `uses_hnsw() = true` and non-zero weights in `typo_tolerant` (0.15) and `semantic_search` (0.02) profiles. E9 scores are computed after candidate retrieval, so it can modify scores but never discover new candidates.

### SEARCH-3: DEFAULT_SEMANTIC_WEIGHTS Differs from semantic_search Profile [LOW]
**Files:** `search.rs:1212-1226` vs `weights/mod.rs:122-137`

Two "default" weight configs disagree:
- E1: 0.35 (default constant) vs 0.33 (named profile)
- E9: 0.0 (default constant) vs 0.02 (named profile)

MCP handler uses the constant (no profile specified), not the named profile.

### SEARCH-4: HNSW Failures Silently Degrade Results [MEDIUM]
**Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs:609-614`

When HNSW search fails for E7, E10, E8, or E11, the error is logged but search continues with fewer embedders. The response contains no signal that results are degraded. No `degraded_embedders` field exists.

### SEARCH-5: Pipeline RRF Hardcodes 6 Embedders, Excludes E6/E13 [LOW]
**Files:** `search.rs:934-941`

Pipeline stage 2 WeightedRRF/ScoreWeightedRRF only uses E1, E5, E7, E8, E10, E11. E6 and E13 are excluded even when `pipeline_full` profile gives them non-zero weights (E6=0.10, E13=0.07). They DO contribute in WeightedSum mode but not RRF mode.

### SEARCH-6: Pipeline Re-reads Fingerprints From RocksDB Instead of Carrying Forward [MEDIUM]
**Files:** `search.rs:1036-1046`

Final results construction discards the `SemanticFingerprint` from scoring (`_semantic`) and re-reads the full `TeleologicalFingerprint` from RocksDB. Performance: 2x reads per result. Integrity: potential TOCTOU if concurrent writes occur.

### SEARCH-7: E8/E10 Asymmetric Vectors Not Direction-Aware in Search [LOW]
**Files:** `search.rs:620, 643`

E5 has full direction-aware routing (Cause -> search Effect index). E8 always searches as "source" (never "target"), E10 always uses paraphrase (never context). The dual vectors exist in the data model but multi-space search ignores direction.

---

## Section 3: MCP Tool Handler Issues (8 findings)

### MCP-1: store_memory -- modality and tags Silently Ignored [HIGH]
**Files:**
- Schema: `tools/definitions/core.rs:42-50`
- Handler: `handlers/tools/memory_tools.rs` (zero matches for "modality" or "tags")

Schema advertises `modality` (enum: text/code/image/audio/structured/mixed) and `tags` (string array). Handler never reads, validates, or stores either parameter. User sends tags/modality, gets no error, data silently vanishes.

### MCP-2: store_memory -- importance Silently Clamped [MEDIUM]
**Files:** `handlers/tools/memory_tools.rs:154-158`

Schema says `minimum: 0, maximum: 1`. Handler clamps `importance: 5.0` to `1.0` silently instead of rejecting. User believes importance was set to 5.0 but it was stored as 1.0.

### MCP-3: search_graph -- minSimilarity Not Validated [MEDIUM]
**Files:** `handlers/tools/memory_tools.rs:541-544`

Schema declares `minimum: 0, maximum: 1`. Handler does no bounds check. `minSimilarity: 5.0` filters out all results. `minSimilarity: -999.0` does nothing. Contrast: `topK` IS explicitly validated (lines 514-536).

### MCP-4: search_graph -- decayFunction/temporalScale Undocumented [MEDIUM]
**Files:** `handlers/tools/memory_tools.rs:669-681, 737-749`

Handler parses and validates `decayFunction` (linear/exponential/step/none/no_decay) and `temporalScale` (micro/meso/macro/long/archival). Neither appears in the JSON schema. MCP clients can never discover these parameters from `tools/list`.

### MCP-5: get_topic_portfolio -- format Parameter Never Used [HIGH]
**Files:**
- Schema: `tools/definitions/topic.rs:28-33`
- Handler: `handlers/tools/topic_tools.rs:193-269`

Schema offers `format: brief|standard|verbose`. DTO validates. Handler builds identical response regardless of format value. Brief, standard, and verbose all return the same data.

### MCP-6: get_session_timeline -- Double Pagination Bug [HIGH]
**Files:** `handlers/tools/sequence_tools.rs:428-431, 503-508`

`skip(offset).take(limit)` applied TWICE:
1. Lines 428-431: on session_fingerprints when converting to results
2. Lines 503-508: on results_with_seq after sorting

With `offset: 10, limit: 5`: first pagination skips 10 and takes 5, second tries to skip 10 more from those 5 results -- yields **zero results**. Only works correctly when offset is 0.

### MCP-7: get_session_timeline -- sourceTypes Silently Ignored [MEDIUM]
**Files:**
- Schema: `tools/definitions/sequence.rs:96-101`
- Handler: `handlers/tools/sequence_tools.rs` (zero matches for "sourceTypes")

Schema declares `sourceTypes` filter (HookDescription/ClaudeResponse/Manual/MDFileChunk). Handler never reads the parameter.

### MCP-8: trigger_consolidation -- Bounds Not Validated [MEDIUM]
**Files:** `handlers/tools/consolidation.rs:172-249`

Schema declares `max_memories: min=1 max=10000` and `min_similarity: min=0 max=1`. Handler parses via serde_json with no validation step. `max_memories: 0` fetches nothing, `max_memories: 999999999` risks OOM.

---

## Section 4: Error Handling Issues (15 findings)

### ERR-1: HNSW Rollback Silently Discards Failures [HIGH]
**Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs:162-166`

```rust
let _ = self.store_fingerprint_internal(&old_fp);
let _ = self.add_to_indexes(&old_fp);
```

During fingerprint update, if HNSW index add fails, rollback attempts to restore old data. Both `let _ =` discard Results. If rollback also fails, system is left with new fingerprint in RocksDB but no HNSW entry -- search silently misses entries.

### ERR-2: Daemon Server Fire-and-Forget [HIGH]
**Files:** `crates/context-graph-mcp/src/main.rs:653-657`

```rust
tokio::spawn(async move {
    if let Err(e) = server.run_tcp().await {
        error!("Daemon server error: {}", e);
    }
});
```

JoinHandle never stored or awaited. If TCP server crashes after initial health check, stdio proxy forwards to dead socket indefinitely. No recovery path.

### ERR-3: Consolidation Degrades to Empty Content [HIGH]
**Files:** `crates/context-graph-mcp/src/handlers/tools/consolidation.rs:280-286`

When `get_content_batch()` fails, substitutes `vec![None; fp_ids.len()]`. Consolidation then compares empty strings against each other, finding them identical, potentially merging unrelated memories. User sees "consolidation complete" with no indication it operated on phantom data.

### ERR-4: Causal Discovery Silently Discards Fetch Errors [HIGH]
**Files:** `crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs:237-256`

`Err(_)` discards error details entirely -- not even logged. `Ok(None) | Err(_)` conflates "content never stored" with "storage layer broke". If 8/10 memories fail to fetch, LLM runs causal analysis on 20% of data with no indication.

### ERR-5: .unwrap() on Early-Return Serde Paths [MEDIUM]
**Files:** `crates/context-graph-mcp/src/handlers/tools/temporal_tools.rs:93-107, 284-297`

Main response path correctly uses `match serde_json::to_value()`. Early-return "empty results" paths use `.unwrap()`. Can panic if struct ever gains a field producing NaN/Infinity.

### ERR-6: HNSW Restore Failure Silently Falls Back to Rebuild [MEDIUM]
**Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs:295-303`

`Ok(false) | Err(_)` triggers full O(n) rebuild. The `Err(_)` discards the specific error from `try_load_hnsw_indexes()`. Persistent HNSW corruption causes O(n) rebuild on every startup with no operator visibility.

### ERR-7: File Watcher Errors Return Ok(false) Instead of Err [MEDIUM]
**Files:** `crates/context-graph-mcp/src/server/watchers.rs:64-87`

`Ok(false)` returned for: 60-second timeout, model loading failure, AND missing provider. Caller cannot distinguish "disabled by config" from "system is broken."

### ERR-8: Batch Comparator .ok() Drops Errors [MEDIUM]
**Files:** `crates/context-graph-core/src/teleological/comparator/batch.rs:127-137`

`.ok()` makes failed comparisons indistinguishable from below-threshold comparisons. During consolidation, this could prevent highly-similar memories from being detected as duplicates.

### ERR-9: JoinHandle Panic Discarded [MEDIUM]
**Files:** `crates/context-graph-mcp/src/main.rs:631`

`let _ = stdout_task.await;` -- if spawned task panicked, JoinError is discarded. `stdio_proxy_run` returns `Ok(())` even though proxy is broken. MCP clients hang indefinitely.

### ERR-10: Unknown CLI Arguments Silently Ignored [MEDIUM]
**Files:** `crates/context-graph-mcp/src/main.rs:183`

`_ => {}` ignores unknown arguments. Typos like `--deamon` or `--daemon_port` silently fall through to defaults.

### ERR-11: Epoch Fallback on Clock Error [LOW]
**Files:** `crates/context-graph-core/src/teleological/services/profile_manager/manager.rs:368`

`unwrap_or_default()` on SystemTime gives timestamp 0 (1970). Corrupts usage tracking silently.

### ERR-12: Pipeline Builder Creates Zero-Vectors for Missing Queries [MEDIUM]
**Files:** `crates/context-graph-storage/src/teleological/search/pipeline/builder.rs:73-76`

Missing `query_semantic` defaults to `vec![0.0; 1024]`. Cosine similarity against zero vector is mathematically undefined. Can produce NaN, 0.0, or panic depending on implementation.

### ERR-13: Poisoned Mutex Returns None Silently [LOW]
**Files:** `crates/context-graph-cli/src/commands/hooks/session_state.rs:81-83`

`SESSION_CACHE.lock().ok()?.clone()` -- poisoned mutex (from prior panic) returns None, indistinguishable from "no session."

### ERR-14: Unknown Weight Categories Silently Dropped [LOW]
**Files:** `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs:2108`

Match on category names drops unknowns via `_ => {}`. New embedder categories silently omitted from weight breakdown response.

### ERR-15: Code Watcher Background Loop Has No Circuit Breaker [MEDIUM]
**Files:** `crates/context-graph-mcp/src/server/watchers.rs:402-413`

Failed `process_events()` logs error every 5 seconds forever. No backoff, no max error count, no self-shutdown. Disk-full scenario generates ~17,280 error entries/day while accomplishing nothing.

---

## Section 5: Test Suite Integrity Issues (14 findings)

### TEST-1: Tautological Constant Tests [MEDIUM]
**Files:** `fusion.rs:236`, `multi_array.rs:1584`, 5x `definitions/*.rs`

Tests like `assert_eq!(RRF_K, 60.0)` and `assert_eq!(definitions().len(), 1)` test that constants equal themselves. Provide zero behavioral verification.

### TEST-2: Assertion-Free "Evidence" Test [HIGH]
**Files:** `mcp_protocol_e2e_test.rs:903-926`

`evidence_of_e2e_test_coverage()` only asserts a hardcoded array has 11 elements (`assert_eq!(11, 11)` in disguise). Does NOT verify tools are registered. This is the ONLY non-CUDA e2e test.

### TEST-3: Tests With No Assertions on Result [HIGH]
**Files:** `teleological_memory_store_tests.rs:283-301`, `pipeline/tests.rs:91-118`

`test_min_similarity_filter` calls search with `min_similarity=0.999` but never checks the filter worked. `println!("[VERIFIED]")` but nothing was verified. `test_timing_breakdown` prints timing data but never asserts any field is non-zero.

### TEST-4: is_err() Without Error Type Verification [MEDIUM]
**Files:** 8+ locations across `retrieval/tests.rs`, `query.rs`, `config_tests.rs`

Tests assert `result.is_err()` but never verify WHAT error was returned. Any error (including wrong errors) passes the test.

### TEST-5: Overly Permissive Upper-Bound Assertions [HIGH]
**Files:** `retrieval/tests.rs:231,251,527`, `teleological_store_stub/tests.rs:93`

`assert!(result.results.len() <= 5)` is true when 0 results returned. A broken search returning nothing passes as "working."

### TEST-6: Stub-Only Tests Masquerading as Integration Tests [HIGH]
**Files:** `crates/context-graph-core/src/retrieval/tests.rs` (entire file)

`StubMultiArrayProvider` generates hash-based embeddings. Tests verify pipeline machinery doesn't crash but verify NOTHING about search quality, ranking, or semantic relevance.

### TEST-7: Feature-Gated Tests That Never Run in CI [CRITICAL]
**Files:** `mcp_protocol_e2e_test.rs` (6 tests), `search_periodic_test.rs`

ALL 6 MCP E2E tests gated behind `#[cfg(feature = "cuda")]`. Without GPU hardware, zero MCP handler E2E coverage.

### TEST-8: Tests That Test Setup, Not Code [LOW]
**Files:** `pipeline/tests.rs:121-133`

Constructs `PipelineHealth { is_healthy: true }` then asserts `is_healthy` is true. Tests struct initialization, not actual health checking.

### TEST-9: Display Trait Tautologies [LOW]
**Files:** `executor.rs:216`, `pressure.rs:213`, `profile/tests.rs:111`

Tests that `format!("{}", IndexType::Hnsw) == "HNSW"`. Cosmetic concern, zero safety benefit.

### TEST-10: Send/Sync Compile-Time Tests [LOW]
**Files:** `memex_impl.rs:452-461`

Compile-time check disguised as runtime test. Either compiles and passes, or doesn't compile at all. Can never fail at runtime.

### TEST-11: Latency Test With No Assertion [HIGH]
**Files:** `search/single/tests/search.rs:345-360`

Prints latency value and says "RESULT: PASS" but has zero assertions. Latency could be 0, garbage, or anything.

### TEST-12: Conditional Assertions That Skip Verification [HIGH]
**Files:** `search_periodic_test.rs:389, 449`

```rust
if !results.is_empty() { /* assertions */ } else { println!("No results (may be expected)"); }
```

With stub embeddings (the common case), results are empty and ALL assertions skipped.

### TEST-13: Unnecessary Async Tests [LOW]
**Files:** 10+ tests across `retrieval/tests.rs`

Tests marked `#[tokio::test]` that never `.await` anything meaningful. Creates false impression of async behavior testing.

### TEST-14: "Evidence of Success" Printf Pattern [MEDIUM]
**Files:** 8 files across `context-graph-embeddings/src/models/`

Tests with ~10:1 ratio of `println!` to `assert!`. Some print `"ALL CHECKS PASSED"` BEFORE assertions run. Misleading if test panics on a later assertion.

---

## Remediation Priority Matrix

### P0 -- Fix Immediately (data corruption / zero coverage)

| ID | Finding | Effort |
|----|---------|--------|
| MCP-6 | Double pagination bug in get_session_timeline | Small -- remove second skip/take |
| SEARCH-1 | SparseVector cosine normalization mismatch | Small -- add `(raw+1)/2` normalization |
| DATA-6 | MemoryStore session index race condition | Medium -- add Mutex like TeleologicalStore |
| ERR-3 | Consolidation on empty content | Small -- return error instead of empty strings |
| TEST-7 | All MCP E2E tests CUDA-gated | Medium -- create non-CUDA E2E test suite |

### P1 -- Fix Soon (silent failures / misleading results)

| ID | Finding | Effort |
|----|---------|--------|
| ERR-1 | HNSW rollback discards failures | Small -- log + propagate |
| ERR-2 | Daemon server fire-and-forget | Medium -- store JoinHandle, detect crash |
| ERR-4 | Causal discovery discards fetch errors | Small -- log errors, add warning to response |
| MCP-1 | store_memory modality/tags ignored | Medium -- implement storage or remove from schema |
| MCP-5 | get_topic_portfolio format never used | Small -- implement or remove parameter |
| DATA-3 | Incomplete rollback in store_async | Medium -- rollback all 5 CFs |
| SEARCH-4 | HNSW failures silently degrade results | Medium -- add degraded_embedders to response |
| TEST-3 | Tests with no assertions on result | Small -- add actual assertions |
| TEST-5 | Overly permissive upper-bound assertions | Small -- change `<=` to range check |

### P2 -- Fix When Convenient (edge cases / maintenance hazards)

| ID | Finding | Effort |
|----|---------|--------|
| DATA-1 | Non-atomic MemoryStore write | Medium -- convert to WriteBatch |
| DATA-2 | Non-atomic delete_causal_relationship | Medium -- convert to WriteBatch |
| DATA-4 | CodeStore non-atomic multi-CF writes | Medium -- convert to WriteBatch |
| ERR-6 | HNSW restore error swallowed | Small -- add warn! log |
| ERR-7 | Watcher errors return Ok(false) | Small -- return Err for failures |
| ERR-10 | Unknown CLI args silently ignored | Small -- add warn! for unknown args |
| ERR-15 | Code watcher no circuit breaker | Medium -- add backoff + max retries |
| MCP-2 | importance silently clamped | Small -- reject out-of-range |
| MCP-3 | minSimilarity not validated | Small -- add bounds check |
| MCP-4 | decayFunction/temporalScale undocumented | Small -- add to schema |
| MCP-7 | sourceTypes silently ignored | Medium -- implement or remove |
| MCP-8 | consolidation bounds unvalidated | Small -- add range checks |
| SEARCH-2 | E9 HNSW never queried for candidates | Medium -- add E9 to multi-space when weight > 0 |
| SEARCH-3 | DEFAULT_SEMANTIC_WEIGHTS vs named profile | Small -- unify |
| SEARCH-5 | Pipeline RRF hardcodes 6 embedders | Medium -- make dynamic |
| SEARCH-6 | Pipeline re-reads fingerprints | Medium -- carry forward from stage 2 |

### P3 -- Low Priority (cosmetic / unlikely edge cases)

| ID | Finding | Effort |
|----|---------|--------|
| DATA-5 | HNSW compaction race (acknowledged) | Large -- snapshot iterator |
| ERR-5 | .unwrap() on serde early-return | Small |
| ERR-8 | Batch comparator .ok() drops errors | Small |
| ERR-9 | JoinHandle panic discarded | Small |
| ERR-11 | Epoch fallback timestamp | Small |
| ERR-12 | Zero-vector defaults in pipeline builder | Medium |
| ERR-13 | Poisoned mutex returns None | Small |
| ERR-14 | Unknown weight categories dropped | Small |
| SEARCH-7 | E8/E10 direction not used in search | Medium |
| TEST-1 | Tautological constant tests | Small |
| TEST-2 | Assertion-free evidence test | Small |
| TEST-4 | is_err() without type check | Small |
| TEST-6 | Stub-only integration tests | Large |
| TEST-8 | Tests that test setup | Small |
| TEST-9 | Display trait tautologies | Small |
| TEST-10 | Send/Sync compile-time tests | None (informational) |
| TEST-11 | Latency test no assertion | Small |
| TEST-12 | Conditional assertions skip verification | Medium |
| TEST-13 | Unnecessary async tests | Small |
| TEST-14 | Evidence-of-success printf pattern | Small |

---

## Methodology

Five forensic investigation agents ran in parallel, each examining a different dimension:

1. **Test Suite Integrity** -- Examined 30+ test files for tautological tests, missing assertions, and deceptive coverage
2. **Error Handling Paths** -- Searched all .rs files for `unwrap_or`, `let _ =`, `.ok()`, `_ => {}` patterns
3. **Data Integrity** -- Examined RocksDB operations, atomicity, serialization, concurrency, and cache coherence
4. **MCP Tool Handlers** -- Cross-referenced all 55 tool schemas against handler implementations
5. **Search & Embedding Pipeline** -- Audited scoring, fusion, normalization, and candidate retrieval across 13 embedders

Each agent performed line-by-line code inspection with specific file:line citations. Findings rated INNOCENT were excluded from this report.

---

## Conclusion

The codebase has strong engineering in its core paths -- the TeleologicalStore uses WriteBatch and secondary_index_lock correctly, E5 asymmetric search is properly implemented, MaxSim normalization is consistent, and bincode/JSON serialization boundaries are well-maintained. However, the MemoryStore, CodeStore, and several MCP handlers appear to be from earlier development phases that never received the same hardening treatment. The test suite inflates coverage numbers through tautological tests and CUDA-gated tests that never run in standard CI. The 5 P0 items represent real bugs that produce incorrect results in normal operation.
