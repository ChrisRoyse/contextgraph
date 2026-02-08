# Context Graph Memory Audit Report

**Date**: 2026-02-07
**Branch**: casetrack (57 MCP tools)
**Scope**: Full codebase - all crates

---

## Executive Summary

This audit identifies **14 memory management issues** across the Context Graph system, ranging from critical VRAM leaks to medium-severity unbounded collections. The most impactful issues are:

1. **HNSW ghost vectors** - deleted vectors permanently consume memory in usearch indexes (15 indexes affected)
2. **Soft-deleted HashMap** - grows monotonically without eviction
3. **Multiple independent RocksDB block caches** - 3 separate caches consuming up to 576MB+
4. **CUDA `mem::forget`** - intentional tensor leak in VRAM allocator

The system's per-memory footprint is substantial: each stored memory creates ~28KB of in-memory index data across 15 HNSW indexes, which is **never reclaimable** even after deletion.

---

## Issue Severity Scale

| Level | Meaning |
|-------|---------|
| **P0 - Critical** | Active memory leak or unbounded growth causing OOM |
| **P1 - High** | Significant memory waste, degrades over hours/days |
| **P2 - Medium** | Suboptimal patterns, problematic at scale |
| **P3 - Low** | Minor inefficiency, acceptable for current scale |

---

## P0 - Critical Issues

### 1. HNSW Ghost Vectors (15 Indexes)

**Files**:
- `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/ops.rs:34-39,74-87`
- `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/types.rs:63-74`

**Problem**: usearch HNSW indexes do not support vector deletion. When a memory is deleted via `remove()`, only the UUID-to-key mapping is removed. The actual vector data remains permanently allocated inside the usearch C++ index.

```rust
// ops.rs:34-38 - insert acknowledges the problem:
// Handle duplicate - remove old mapping (usearch may not support true deletion)
if let Some(&old_key) = id_to_key.get(&id) {
    key_to_id.remove(&old_key);
    // Note: usearch doesn't support deletion, so the old vector remains in index
}

// ops.rs:78-82 - remove only clears the mapping:
if let Some(key) = id_to_key.remove(&id) {
    // Note: Vector remains in usearch index (doesn't support deletion)
    key_to_id.remove(&key);
}
```

**Memory Impact Per Deleted Memory**:

| Index | Dimension | Bytes/Vector | Count |
|-------|-----------|-------------|-------|
| E1 Semantic | 1024 | 4,096 | 1 |
| E2 Temporal Recent | 512 | 2,048 | 1 |
| E3 Temporal Periodic | 512 | 2,048 | 1 |
| E4 Temporal Positional | 512 | 2,048 | 1 |
| E5 Causal (cause) | 768 | 3,072 | 1 |
| E5 Causal (effect) | 768 | 3,072 | 1 |
| E7 Code | 1,536 | 6,144 | 1 |
| E8 Graph | 1,024 | 4,096 | 1 |
| E9 HDC | 1,024 | 4,096 | 1 |
| E10 Multimodal | 768 | 3,072 | 1 |
| E11 Entity | 768 | 3,072 | 1 |
| E1 Matryoshka | 128 | 512 | 1 |
| Causal E11 HNSW | 768 | 3,072 | variable |
| **Total** | | **~40KB** | per memory |

Plus usearch internal graph overhead (neighbor lists, ~M*16 bytes per vector where M=16 default).

**Projection**: With 100K memories and 10% deletion rate, ~400MB of VRAM/RAM is permanently leaked as ghost vectors. This grows monotonically and can only be reclaimed by rebuilding all indexes.

**Additionally**: The `reserve()` call at `ops.rs:44-53` doubles capacity when full, meaning the index allocates 2x its actual need after any growth event:
```rust
let new_capacity = (current_capacity * 2).max(1024);
```

### 2. Soft-Deleted HashMap (Unbounded Growth)

**File**: `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs:80,187`

```rust
pub(crate) soft_deleted: Arc<RwLock<HashMap<Uuid, bool>>>,
```

**Problem**: Every soft-deleted fingerprint adds a 16-byte UUID + 1-byte bool + ~80 bytes HashMap overhead to this in-memory map. Entries are **never evicted**. The map is checked on every search operation across multiple hot paths.

**Referenced in search hot path** (`search.rs`): lines 105, 195, 217, 257, 281, 294, 309, 324, 392, 474, 671, 715, 836, 1103.

**Projection**: At 10K deletions, this consumes ~1MB. At 1M deletions, ~100MB. Since soft-delete is preferred over hard-delete, this grows with every `forget_concept` call.

---

## P1 - High Issues

### 3. Multiple Independent RocksDB Block Caches

**Files**:
- `crates/context-graph-storage/src/teleological/rocksdb_store/store.rs:142` - Teleological store cache (configurable, default 256MB)
- `crates/context-graph-storage/src/code/store.rs:98` - Code store cache (hardcoded 64MB)
- `crates/context-graph-storage/src/rocksdb_backend/core.rs:224` - Backend cache (configurable)
- `crates/context-graph-storage/src/graph_edges/repository.rs:568` - Edge repo cache (256MB, test only)

**Problem**: Each `DB::open_cf_descriptors` creates its own block cache instance. The teleological store and code store open separate RocksDB databases with independent caches, consuming 256MB + 64MB = **320MB minimum** for block caches alone, without sharing.

RocksDB best practice is to share a single `Cache` instance across all DB instances to allow unified memory management. Currently, total memory is the sum of all caches rather than a shared pool.

### 4. `std::mem::forget(tensor)` in CUDA Allocator

**File**: `crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs:264`

```rust
std::mem::forget(tensor);
Ok(VramAllocation::new_protected(ptr, size_bytes, self.device_id))
```

**Problem**: The Rust tensor wrapper is intentionally leaked via `mem::forget`. While the intent is to prevent Rust from dropping the tensor (which would free the CUDA memory the `VramAllocation` now owns), this leaks the Rust-side metadata of the tensor struct (smart pointer overhead, shape info, etc.).

**Mitigating factor**: There is a corresponding `free_protected()` at line 273 that manually tracks bytes, but it only updates accounting - it doesn't actually free the forgotten tensor's Rust-side allocations. The VRAM pointer itself may be managed externally, but the Rust wrapper memory is permanently leaked.

**Impact**: ~64-128 bytes per allocation. With 13 model loads at startup, this is ~1-2KB total (low absolute impact, but the pattern is concerning).

### 5. Scanner `analyzed_pairs` HashSet (Unbounded)

**Files**:
- `crates/context-graph-causal-agent/src/scanner/mod.rs:67,164`
- `crates/context-graph-graph-agent/src/scanner/mod.rs:70,164`

```rust
analyzed_pairs: HashSet<(Uuid, Uuid)>,
```

**Problem**: Each scanner tracks which (memory_A, memory_B) pairs have been analyzed. This set grows O(n^2) with the number of memories and **never shrinks** unless `clear_analyzed()` is explicitly called. Each entry is 32 bytes (two UUIDs) + HashSet overhead.

**Projection**: 10K memories = up to 100M possible pairs = 3.2GB theoretical maximum. Practically limited by batch_size per cycle (50), but over weeks of continuous operation, reaches tens of MB.

### 6. Discovery Service Error Accumulation

**Files**:
- `crates/context-graph-causal-agent/src/service/mod.rs:115,369`
- `crates/context-graph-graph-agent/src/service/mod.rs:89,250,309,368`

**Problem**: `DiscoveryCycleResult.error_messages: Vec<String>` accumulates error messages during each discovery cycle. If stored as `last_result`, the vector from the most recent cycle is retained. While only one cycle's errors persist at a time (the last result), a cycle processing many candidates could generate thousands of error strings.

**Impact**: Medium per cycle (bounded by batch size), but error strings can be 100-500 bytes each.

### 7. Custom Weight Profiles (No Limit)

**File**: `crates/context-graph-mcp/src/handlers/core/handlers.rs:113`

```rust
pub(in crate::handlers) custom_profiles: Arc<RwLock<HashMap<String, [f32; 13]>>>,
```

**Problem**: The `create_weight_profile` tool at `embedder_tools.rs:946-948` inserts profiles with no size limit. Each profile is small (52 bytes + key string), but there's no upper bound. An automated client could create millions of profiles.

**Mitigating factor**: Session-scoped (clears on restart). Low per-entry cost.

---

## P2 - Medium Issues

### 8. HNSW Capacity Doubling Strategy

**File**: `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/ops.rs:44-53`

```rust
if current_size >= current_capacity {
    let new_capacity = (current_capacity * 2).max(1024);
    index.reserve(new_capacity)...
}
```

**Problem**: Capacity doubling means up to 50% wasted memory at any point. For 15 indexes, this amplifies. If index has 10,001 vectors, capacity jumps to 20,000 - wasting ~400KB per 1024-dim index in pre-allocated but unused HNSW graph structures.

### 9. File Hash Caches Without Pruning

**Files**:
- `crates/context-graph-core/src/memory/code_watcher.rs:187`
- `crates/context-graph-core/src/memory/watcher.rs:186`

```rust
file_hashes: Arc<RwLock<HashMap<String, ...>>>,
```

**Problem**: File watchers track hashes of watched files. When files are deleted from the filesystem, their entries remain in the HashMap. Growth is bounded by project size (typically 1K-10K files), but stale entries accumulate.

### 10. InMemoryTeleologicalStore DashMaps (Test/Stub)

**File**: `crates/context-graph-core/src/stubs/teleological_store_stub/mod.rs:66-80`

```rust
pub(crate) data: DashMap<Uuid, TeleologicalFingerprint>,
pub(crate) deleted: DashMap<Uuid, ()>,
pub(crate) content: DashMap<Uuid, String>,
pub(crate) source_metadata: DashMap<Uuid, SourceMetadata>,
pub(crate) topic_portfolios: DashMap<String, PersistedTopicPortfolio>,
pub(crate) causal_relationships: DashMap<Uuid, CausalRelationship>,
pub(crate) causal_by_source: DashMap<Uuid, Vec<Uuid>>,
pub(crate) file_index: DashMap<String, Vec<Uuid>>,
```

**Problem**: 8 unbounded DashMaps with no eviction. While this is a test/stub store, if used in production (e.g., for non-persistent mode), it would consume enormous memory. Each fingerprint stored in `data` carries ~40KB of embedding vectors.

**Mitigating factor**: Stub only. Production uses RocksDB store.

### 11. Allocation History Ring Buffer Inefficiency

**File**: `crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs:253-256`

```rust
self.allocation_history.push(history_entry);
if self.allocation_history.len() > MAX_ALLOCATION_HISTORY {
    self.allocation_history.remove(0);  // O(n) removal from front of Vec!
}
```

**Problem**: Using `Vec::remove(0)` for a ring buffer is O(n) because it shifts all elements. Should use `VecDeque` for O(1) push_back/pop_front. The history is bounded by `MAX_ALLOCATION_HISTORY`, so it doesn't grow unbounded, but the O(n) shift on each alloc/free is wasteful.

### 12. Background Graph Builder Queue (No Cap)

**File**: `crates/context-graph-storage/src/graph_edges/builder.rs:170,232-233`

```rust
pending_queue: Arc<Mutex<VecDeque<Uuid>>>,
// ...
queue.push_back(fingerprint_id); // No capacity check
```

**Problem**: The pending queue for K-NN graph building has no maximum size. If the background processor falls behind (or is paused), the queue grows unbounded. Each entry is 16 bytes (UUID), so 100K queued items = 1.6MB, but more concerning is that backpressure is invisible to the caller.

---

## P3 - Low Issues

### 13. String Cloning in 13-Embedder Pipeline

**File**: `crates/context-graph-embeddings/src/provider/multi_array.rs`

**Problem**: During `embed_all()`, the content string is cloned once per embedder (13 times) for parallel processing. For a 10KB document, this is 130KB of temporary allocations per `store_memory` call.

**Mitigating factor**: Temporary allocations freed after embedding completes. Not a leak.

### 14. `OnceLock` Globals (Permanent but Intentional)

**Files**:
- `crates/context-graph-embeddings/src/global_provider.rs:52` - Global warm provider
- `crates/context-graph-embeddings/src/gpu/device/core.rs:28-37` - GPU device/info
- `crates/context-graph-cli/src/commands/hooks/memory_cache.rs:31` - Memory cache (bounded, 10 sessions)

**Status**: These are intentional singletons with proper initialization. Not leaks. The CLI memory cache has TTL (5 min) and capacity limit (10 sessions).

---

## Memory Budget Analysis

### Baseline Memory (Empty System)

| Component | Memory | Source |
|-----------|--------|--------|
| 13 GPU embedding models (VRAM) | ~4-8 GB | Warm-loaded at startup |
| RocksDB teleological block cache | 256 MB | `store.rs:142` default |
| RocksDB code store block cache | 64 MB | `store.rs:98` hardcoded |
| RocksDB write buffers (3x 64MB per DB) | ~384 MB | 2 DBs x 3 x 64MB |
| 15 HNSW indexes (empty, 1024 initial cap) | ~60 MB | 15 x dimensions x 1024 x 4 |
| Bloom filters | ~10 MB | 10 bits/key, shared |
| **Total baseline** | **~5-9 GB** | |

### Per-Memory Growth

| Component | Per Memory | At 10K | At 100K |
|-----------|-----------|--------|---------|
| 15 HNSW vectors (in-memory) | ~28 KB | 280 MB | 2.8 GB |
| HNSW graph overhead (neighbors) | ~4 KB | 40 MB | 400 MB |
| RocksDB (on-disk, cached) | ~50 KB | Cached subset | Cached subset |
| id_to_key + key_to_id maps | ~1.2 KB | 12 MB | 120 MB |
| **Total in-memory growth** | **~33 KB** | **332 MB** | **3.3 GB** |

### Leak Growth (Ghost Vectors on Deletion)

| Scenario | Leak per Delete | At 1K deletes | At 10K deletes |
|----------|----------------|---------------|----------------|
| HNSW ghost vectors | ~28 KB | 28 MB | 280 MB |
| soft_deleted HashMap | ~100 bytes | 100 KB | 1 MB |
| **Total leaked** | **~28 KB** | **28 MB** | **281 MB** |

---

## Recommendations

### Immediate (P0)

1. **HNSW Periodic Rebuild**: Implement a background task that periodically rebuilds HNSW indexes to reclaim ghost vectors. The infrastructure exists (`rebuild_indexes_from_store`) - expose it as a scheduled maintenance operation. Trigger when `ghost_ratio = (index.size() - key_to_id.len()) / index.size() > 0.20`.

2. **Soft-Delete Eviction**: Either:
   - (a) Persist soft-delete state in a RocksDB column family (already have 54 CFs, one more is fine) and use a bloom filter for the in-memory check, OR
   - (b) Add TTL-based eviction: after 30 days, convert soft-deletes to hard-deletes

### Short-Term (P1)

3. **Shared RocksDB Block Cache**: Create a single `Cache::new_lru_cache()` instance and pass it to all DB opens. This is a ~10-line change that could save 64-256MB.

4. **Scanner Pair Reset**: Add automatic `clear_analyzed()` after every N cycles or when the set exceeds a configurable threshold (e.g., 100K pairs).

5. **Custom Profile Limit**: Add `const MAX_CUSTOM_PROFILES: usize = 100` and reject creates beyond this.

6. **Graph Builder Queue Cap**: Add `const MAX_PENDING_QUEUE: usize = 10_000` and drop oldest entries when exceeded.

### Medium-Term (P2)

7. **VecDeque for Allocation History**: Replace `Vec::remove(0)` with `VecDeque::pop_front()`.

8. **File Hash Pruning**: During `process_events()`, remove entries for files that no longer exist on disk.

9. **HNSW Capacity Strategy**: Consider 1.5x growth factor instead of 2x, or use a more granular growth curve.

### Monitoring

10. **Add memory metrics**: Expose the following via MCP tool or log:
    - `hnsw_ghost_ratio` per index
    - `soft_deleted_count`
    - `custom_profiles_count`
    - `pending_queue_depth`
    - `scanner_analyzed_pairs_count`
    - `rocksdb_block_cache_usage` per DB

---

## Architecture Notes

### What's Done Well

- **Connection limiting**: TCP connections bounded by semaphore (`server.rs:558`)
- **Batch processor channels**: Bounded `mpsc::channel(request_buffer_size)` (`batch/processor/core.rs:110`)
- **CLI memory cache**: Has TTL (5 min), capacity limit (10 sessions), eviction policy (`memory_cache.rs:22-28`)
- **JoinHandle management**: Background tasks properly tracked and awaited on shutdown
- **Arc usage**: Consistent use of `Arc::clone` instead of deep cloning for shared state
- **RocksDB per-CF block cache sharing**: Within each DB instance, all column families share one cache

### Systemic Risk

The 15 in-memory HNSW indexes are the dominant memory consumer. Each memory stored creates ~33KB of in-memory index data that persists for the lifetime of the process. At 100K memories, this is **3.3GB of RAM** just for HNSW indexes, separate from the GPU VRAM used by models. This is by-design for O(log n) search performance, but it means the system has a hard memory ceiling proportional to corpus size.

Consider implementing **disk-backed HNSW** (usearch supports memory-mapped indexes via `Index::new_with_path`) for cold data, keeping only frequently-accessed vectors in memory.
