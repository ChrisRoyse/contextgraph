<task_spec id="TASK-SHIFT-006" version="1.0">
<metadata>
  <title>Logic — InstrumentCache 4 GB LRU + per-instrument hit/miss metrics</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-SHIFT-08">LRU keyed by (file_sha, slot_id, code_version); 4 GB RAM budget</item>
    <item ref="REQ-SHIFT-12">cache hit_rate / capacity / entries surfaced to subscriber_status</item>
    <item ref="EC-SHIFT-08">Undersized cache surfaces to status</item>
    <item ref="AC-SHIFT-08-01">LRU evicts; overflow → MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[InstrumentCache]">5 methods</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (cache.rs body) + tests</estimated_files>
</metadata>

<context>
Implements the read-heavy LRU cache that backs panel-slot materialization. Hits are lock-free via `RwLock<LruCache>` upgraded only on insert. RAM budget enforced after `compute()` returns; oversized inserts evict LRU until under budget; impossible insertions surface `InstrumentCacheOverflow`. Hit-rate, capacity, entry-count metrics consumed by `mejepa_subscriber_status`.
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[InstrumentCache]</file>
  <file purpose="cache_skeleton">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
  <file purpose="error_taxonomy">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-003.md</file>
  <file purpose="hygiene_cross_cut">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md (REQ-SHIFT-08 cross-cutting with SPEC-HYGIENE)</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-002 + TASK-SHIFT-003 complete</check>
  <check>`lru` crate ≥ 0.12 dependency added in TASK-SHIFT-002</check>
  <check>`parking_lot` workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `InstrumentCache::new`, `get_or_insert_with<F>`, `hit_rate`, `entries`, `resize`
    - RAM accounting: each `Vec<f32>` size = `4 * len` bytes + small overhead; tracked via `AtomicU64::ram_used_bytes`
    - Read-then-upgrade pattern: read lock for hit; drop lock; compute outside lock; write lock to insert
    - Eviction loop after insert: while `ram_used_bytes > ram_budget_bytes`, `lru.pop_lru()`; subtract evicted size from accountant
    - Overflow handling: if newly-computed value alone exceeds `ram_budget_bytes`, drop value and return `SubscriberError::InstrumentCacheOverflow`
    - `resize(new_capacity)` clamped to [256, 16_384]; out-of-bound returns `ConfigOutOfBounds`
    - Tests: insert 5,000 distinct keys (above default capacity 4,096) → entries stabilizes at 4,096; LRU policy verified by access order
    - Tests: hit-rate computation: 1,000 lookups with 70% hit pattern → reported hit_rate ∈ [0.69, 0.71]
    - Tests: oversized insert (a vec exceeding budget) → InstrumentCacheOverflow
  </in_scope>
  <out_of_scope>
    - Tier-compression eviction policy (SPEC-HYGIENE)
    - Disk-backed cache spillover (out of scope; cache is RAM-only)
    - Cache-key collision detection (collisions are structurally impossible because `code_version` is in the key)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/cache.rs">
      use crate::error::SubscriberError;
      use std::sync::Arc;

      impl InstrumentCache {
          pub fn new(capacity: usize, ram_budget_bytes: u64) -&gt; Self;

          pub fn get_or_insert_with&lt;F&gt;(
              &amp;self,
              key: SlotKey,
              compute: F,
          ) -&gt; Result&lt;Arc&lt;Vec&lt;f32&gt;&gt;, SubscriberError&gt;
          where
              F: FnOnce() -&gt; Result&lt;Vec&lt;f32&gt;, SubscriberError&gt;;

          pub fn hit_rate(&amp;self) -&gt; f32;
          pub fn entries(&amp;self) -&gt; usize;
          pub fn ram_used_bytes(&amp;self) -&gt; u64;
          pub fn capacity(&amp;self) -&gt; usize;

          pub fn resize(&amp;self, new_capacity: usize) -&gt; Result&lt;(), SubscriberError&gt;;

          // Internal:
          fn entry_size_bytes(value: &amp;[f32]) -&gt; u64;
          fn evict_until_under_budget(&amp;self);
      }
    </signature>
  </signatures>

  <constraints>
    - `Arc<Vec<f32>>` for lock-free clone on hit (no copy)
    - `compute()` SHALL run OUTSIDE the write lock to avoid blocking other readers (release read lock before compute; reacquire write lock for insert)
    - Recheck-after-relock: another thread may have inserted while compute was running; if so, return that other value (drop the freshly-computed one)
    - Eviction is strictly LRU; `lru` crate's `pop_lru()` is the only eviction primitive used
    - Overflow detection: compute `entry_size_bytes(&value)`; if `value alone > ram_budget_bytes` → `InstrumentCacheOverflow` (no insert; drop value)
    - `resize`: bounds [256, 16_384] per REQ-SHIFT-17; out of bounds → ConfigOutOfBounds
    - Hit-rate calculation guards against div-by-zero (returns 0.0 on no lookups yet); always finite
    - File under 500 lines
  </constraints>

  <verification>
    - 5,000-distinct-key insert test: final `entries() == 4_096`
    - LRU correctness: insert k0..k4095; access k0; insert k4096; assert k1 evicted (k0 was MRU)
    - Hit rate test: 700 hits over 1,000 lookups → `hit_rate()` ∈ [0.69, 0.71]
    - Overflow test: 5 GB synthetic vec on 4 GB budget → InstrumentCacheOverflow
    - Concurrent test: 16 threads × 1,000 lookups on 100 keys → no panics, hit_rate &gt; 0.9 (warm cache)
    - `cargo clippy` clean
  </verification>
</definition_of_done>

<pseudo_code>
get_or_insert_with(key, compute):
  // Hit fast path
  {
    let lru = self.lru.read();
    if let Some(v) = lru.peek(&key) {
      self.hits.fetch_add(1, Ordering::Relaxed);
      // promote-to-MRU happens via separate write lock to keep peek read-only
      drop(lru);
      let mut wlock = self.lru.write();
      wlock.get(&key); // promotes
      return Ok(Arc::clone(wlock.peek(&key).unwrap()));
    }
  }
  self.misses.fetch_add(1, Ordering::Relaxed);
  // Compute outside lock
  let value = compute()?;
  let size = Self::entry_size_bytes(&value);
  if size > self.ram_budget_bytes {
    return Err(SubscriberError::InstrumentCacheOverflow{ requested_bytes: size, current_bytes: self.ram_used_bytes(), capacity_bytes: self.ram_budget_bytes });
  }
  // Re-check + insert under write lock
  let mut wlock = self.lru.write();
  if let Some(v) = wlock.get(&key) {
    return Ok(Arc::clone(v)); // race: someone else inserted
  }
  let arc = Arc::new(value);
  wlock.put(key, Arc::clone(&arc));
  self.ram_used_bytes.fetch_add(size, Ordering::Relaxed);
  drop(wlock);
  self.evict_until_under_budget();
  Ok(arc)
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/cache_integration.rs">5K-key insert + LRU + concurrency drills</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/cache.rs">implement 5 public methods + private helpers</file>
</files_to_modify>

<validation_criteria>
  <criterion>LRU eviction correct (test: most-recently-used not evicted; least-recently-used evicted)</criterion>
  <criterion>RAM budget honored; overflow surfaces structured error</criterion>
  <criterion>Hit-rate calculation finite, in [0,1], div-by-zero safe</criterion>
  <criterion>Concurrent 16-thread test finishes without panic; hit rate &gt; 0.9 after warmup</criterion>
  <criterion>resize bounds enforced</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber cache::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test cache_integration</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
