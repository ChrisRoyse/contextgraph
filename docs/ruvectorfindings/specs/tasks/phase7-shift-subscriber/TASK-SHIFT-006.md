## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-10 STATE AUDIT (agent-211-task-updater)

**Audit pass against current tree on 2026-05-10:**

- ✅ **Crate `context-graph-mejepa-shift-subscriber` does NOT exist yet** — `InstrumentCache` body lands in the same not-yet-existing crate.
- ✅ `lru ≥ 0.12` and `parking_lot` workspace deps available; declared in TASK-SHIFT-002.
- ✅ `SlotKey` + `InstrumentCache` skeletons (declared in TASK-SHIFT-002) match the body signatures defined in this task — no signature drift to reconcile.
- ✅ `InstrumentSlot` enum is shipped at `crates/context-graph-mejepa-instruments/src/lib.rs` and is the canonical slot identifier used in the cache key.
- No drift in `<files_to_create>` paths.

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
