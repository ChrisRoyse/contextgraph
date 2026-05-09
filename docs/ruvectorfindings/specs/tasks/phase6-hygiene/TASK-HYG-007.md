# TASK-HYG-007 — tier_demote + tier_promote + per-entry sharded mutex

```xml
<task_spec id="TASK-HYG-007" version="1.0">
<metadata>
  <title>tier_demote + tier_promote operations + per-entry sharded mutex map (DashMap)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <requirement_ref>REQ-HYG-02</requirement_ref>
    <requirement_ref>REQ-HYG-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-004</task_ref>
    <task_ref>TASK-HYG-005</task_ref>
    <task_ref>TASK-HYG-006</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Implements the bidirectional tier transition operations: demote (Hot → Warm → Cool → Cold →
Frozen on idle) and promote (Frozen → Cold → Cool → Warm → Hot on read). Per-entry locking
via `DashMap<EntryId, Arc<Mutex<()>>>` prevents concurrent re-encoding of the same entry
(per <note category="locking">). Both ops read entry blob + panel meta sidecar, compute new
tier via decay + threshold check, decode current blob, re-encode at new tier, write batch.
Decompression failures fail-closed with TierDecompressFail; corrupt entries are marked
`corrupt: true` in panel meta and surface to the next GC eviction (per EC-HYG-05).
</context>

<input_context_files>
  <file purpose="ops_contracts">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="tier_codec">crates/context-graph-mejepa-hygiene/src/tier.rs</file>
  <file purpose="tier_frozen">crates/context-graph-mejepa-hygiene/src/tier_frozen.rs</file>
  <file purpose="entry_meta">crates/context-graph-mejepa-hygiene/src/entry.rs</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
  <file purpose="state_diagram">../../technical/TECH-HYGIENE.md#architecture_diagram</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-004, 005, 006 shipped</check>
  <check>dashmap dep added</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/tier_ops.rs` with:
      * `pub struct TierLockMap { inner: DashMap<EntryId, Arc<Mutex<()>>> }`
        - `acquire(&self, id: &EntryId) -> Arc<Mutex<()>>` returns or inserts mutex
        - `release(&self, id: &EntryId)` (optional cleanup; can be no-op for now)
      * `pub fn tier_demote(env: &HygieneEnv, locks: &TierLockMap, entry_id: &EntryId) -> OpsResult<()>`:
        1. Acquire per-entry lock.
        2. Read `entry` blob from `env.db.get_cf(entry_id.cf_name, &entry_id.key)?`.
        3. Read panel meta (EntryAccessFrequency) from CF_MEJEPA_PANEL_META at the same key.
        4. Compute new freq via decayed_frequency(panel_meta.frequency, last_read, now).
        5. Determine current tier via tier_for_frequency(panel_meta.frequency).
        6. Determine new tier: if new freq < current_tier.threshold_lower, drop to next_colder; else no-op.
        7. If no-op: just persist updated meta with new freq + last_demote_ts.
        8. If transition: decode current blob (or treat as raw f32 if Hot); re-encode at new tier;
           write batch: replace entry with new blob; update panel meta with new tier + freq +
           transition log + last_demote_ts.
        9. Write batch atomic via `db.write(batch)`.
      * `pub fn tier_promote(env: &HygieneEnv, locks: &TierLockMap, entry_id: &EntryId) -> OpsResult<()>`:
        1. Acquire per-entry lock.
        2. Read entry + meta.
        3. new_freq = incremented_frequency(meta.frequency, last_read, now).
        4. current_tier = tier_for_frequency(meta.frequency); new_tier = tier_for_frequency(new_freq).
        5. If new_tier > current_tier (warmer): decode → re-encode at new_tier → write batch
           with updated meta (frequency, last_promote_ts, transition entry).
        6. Else: persist freq + last_read_ts only; no re-encode.
      * On decode failure: write panel meta with `corrupt: true`; return OpsError::TierDecompressFail.
        Do NOT propagate corrupt blob to a hotter tier.
      * On Frozen tier read: set `fidelity_lost: true` in updated meta after promote (the data
        was reconstructed from sign bits — magnitude is gone).
    - Re-export `tier_demote` + `tier_promote` from lib.rs at top level (consumed by ops.rs in
      TASK-HYG-021).
    - Unit tests with synthetic in-memory RocksDB:
      * Idle 1 day → freq drops below 0.6 → Hot demoted to Warm; meta records transition.
      * Read 5× rapidly → freq climbs → Cool promoted to Warm.
      * Inject blob with bad magic → TierDecompressFail; meta has corrupt=true after.
      * Frozen read → fidelity_lost=true; meta records.
      * Concurrent tier_promote on same entry: serialized by per-entry lock (no double-decode).
  </in_scope>
  <out_of_scope>
    - GC step 1 demote sweep loop (TASK-HYG-012).
    - Eviction (TASK-HYG-009 / TASK-HYG-014).
    - PanelMeta CF schema definition is OWNED by TASK-HYG-007 here — this task is the
      first to write to CF_MEJEPA_PANEL_META; downstream tasks read it.
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/tier_ops.rs">
    pub struct TierLockMap { /* DashMap inner */ }
    impl TierLockMap {
        pub fn new() -&gt; Self;
        pub fn acquire(&amp;self, id: &amp;crate::entry::EntryId) -&gt; std::sync::Arc&lt;parking_lot::Mutex&lt;()&gt;&gt;;
    }

    pub fn tier_demote(env: &amp;crate::HygieneEnv, locks: &amp;TierLockMap, entry_id: &amp;crate::entry::EntryId) -&gt; crate::error::OpsResult&lt;()&gt;;
    pub fn tier_promote(env: &amp;crate::HygieneEnv, locks: &amp;TierLockMap, entry_id: &amp;crate::entry::EntryId) -&gt; crate::error::OpsResult&lt;()&gt;;

    pub const CF_MEJEPA_PANEL_META: &amp;str = "CF_MEJEPA_PANEL_META";
    </signature>
    <signature file="crates/context-graph-mejepa-hygiene/src/entry.rs">
    // Add helpers
    impl crate::entry::EntryAccessFrequency {
        pub fn corrupt(&amp;self) -&gt; bool;
        pub fn fidelity_lost(&amp;self) -&gt; bool;
        pub fn mark_corrupt(&amp;mut self);
        pub fn mark_fidelity_lost(&amp;mut self);
    }

    // Extend struct with `pub corrupt: bool, pub fidelity_lost: bool`
    </signature>
  </signatures>

  <constraints>
    - DashMap ensures lock-free shard-level access; per-entry lock is a parking_lot::Mutex.
    - Decode failure marks corrupt=true in meta BEFORE returning the error (so subsequent
      reads / GC see the flag).
    - Atomic write_batch: entry blob + meta sidecar written together; never split.
    - No `unwrap_or` on confidence/quota paths.
    - File ≤ 500 lines.
    - clippy clean.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- tier_demote_idle` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- tier_promote_warm_up` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- tier_promote_corrupt_blob` passes
      (TierDecompressFail returned + meta.corrupt == true).
    - `cargo test -p context-graph-mejepa-hygiene -- tier_promote_concurrent` passes
      (per-entry lock serializes calls; no panic, no double-decode).
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn tier_demote(env: &amp;HygieneEnv, locks: &amp;TierLockMap, id: &amp;EntryId) -&gt; OpsResult&lt;()&gt; {
    let lk = locks.acquire(id);
    let _g = lk.lock();
    let cf = env.db.cf_handle(&amp;id.cf_name).ok_or_else(|| OpsError::internal("cf_missing"))?;
    let blob = env.db.get_cf(&amp;cf, &amp;id.key)?.ok_or_else(|| OpsError::internal("entry_missing"))?;
    let meta_cf = env.db.cf_handle(CF_MEJEPA_PANEL_META).ok_or_else(|| OpsError::internal("meta_cf_missing"))?;
    let mut meta = read_meta(&amp;env.db, &amp;meta_cf, &amp;id.key)?;
    let now = env.now().timestamp_nanos_opt().unwrap_or(0) as u64;
    let new_freq = decayed_frequency(meta.frequency, meta.last_read_ts_ns, now)?;
    let cur = tier_for_frequency(meta.frequency);
    let new = tier_for_frequency(new_freq);
    if new == cur {
        meta.frequency = new_freq;
        meta.last_demote_ts_ns = now;
        write_meta(&amp;env.db, &amp;meta_cf, &amp;id.key, &amp;meta)?;
        return Ok(());
    }
    // Transition needed
    let values = match cur {
        Tier::Hot =&gt; bytemuck::cast_slice::&lt;u8, f32&gt;(&amp;blob).to_vec(),
        _ =&gt; match decode_for_tier(&amp;blob) {
            Ok(v) =&gt; v,
            Err(e) =&gt; { meta.mark_corrupt(); write_meta(...)?; return Err(e); }
        },
    };
    let new_blob = encode_for_tier(&amp;values, new)?;
    let mut batch = WriteBatch::default();
    batch.put_cf(&amp;cf, &amp;id.key, &amp;new_blob);
    meta.frequency = new_freq;
    meta.last_demote_ts_ns = now;
    // append transition
    write_meta_into_batch(&amp;mut batch, &amp;meta_cf, &amp;id.key, &amp;meta)?;
    env.db.write(batch)?;
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/tier_ops.rs">tier_demote + tier_promote + TierLockMap</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">add `pub mod tier_ops;` + re-export tier_demote/tier_promote</file>
  <file path="crates/context-graph-mejepa-hygiene/src/entry.rs">extend EntryAccessFrequency with corrupt + fidelity_lost flags</file>
  <file path="crates/context-graph-mejepa-hygiene/Cargo.toml">add parking_lot if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>tier_demote: Hot entry idle 1 day → demoted to Warm; meta has TierTransition logged</criterion>
  <criterion>tier_promote: Cool entry read 5× → promoted to Warm</criterion>
  <criterion>Corrupt blob → TierDecompressFail + meta.corrupt=true</criterion>
  <criterion>Frozen read → fidelity_lost=true</criterion>
  <criterion>Concurrent calls on same EntryId serialized (no double re-encode)</criterion>
  <criterion>Atomic write_batch: blob + meta committed together</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_demote_idle</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_promote_warm_up</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_promote_corrupt_blob</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_promote_concurrent</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
