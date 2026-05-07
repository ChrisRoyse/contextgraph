---
namespace: discoveries
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:05:00Z
status: active
tags: constellation, rebuild, orphan, storage-leak, atomic-swap
concern: C-2
---

# Concern-2: rebuildIfExists leaves old constellation orphaned in CF_CONSTELLATIONS

## Classification
**DEFECT** — documented in the storage-layer docstring but not handled by the
only caller that can trigger the loss-of-reachability.

## Severity
**Medium** — leaks storage monotonically. Not corrupting, but no caller can
find the orphan (the secondary index no longer points to it) so no tool can
clean it up other than the brute-force `delete_constellation` on the specific
orphan UUID, which requires the caller to have already captured that UUID.

## Location
- `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs:64-69`
  — `store_constellation`'s docstring **explicitly acknowledges** the issue:
  > "When the selector's canonical form maps to an index entry that already
  > points at a different constellation UUID, the index is updated to point
  > to the new UUID (the old record is **not** automatically deleted; call
  > `delete_constellation` on the stale id if needed)."
- `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs:76-97`
  — `call_compile_constellation` branches on `rebuild_if_exists`. When
  `rebuild_if_exists == true`, it proceeds to `resolve_members` / compile /
  `store_constellation` with a fresh UUID (line 320: `id: Uuid::new_v4()` in
  `finalize`) **without ever fetching or deleting the existing UUID** that
  `find_constellation_by_selector` would return.

## Root cause

`rebuild_if_exists` means "regenerate the constellation for this selector".
The user intent is clear: "overwrite the prior one". The current code
satisfies *half* of that:
- Secondary index `CF_CONSTELLATION_BY_SELECTOR` is updated (batch.put_cf at
  line 78 of constellation.rs overwrites it with the new UUID).
- Primary row `CF_CONSTELLATIONS` for the **old** UUID is never touched —
  `Uuid::new_v4()` generates a different primary key, so the WriteBatch
  `put_cf(primary_cf, c.id.as_bytes(), payload)` targets a brand-new key.

Every `rebuildIfExists=true` invocation thus adds one row to
`CF_CONSTELLATIONS` without removing any. Live-test evidence (per
coordinator's probe): `cf56b177` existed; rebuild created `660410b0`;
`cf56b177` remained in the CF until manually deleted.

## Is this DESIGN-INTENT (blame-line / audit)?

No. Two reasons:

1. **There is no audit field on `Constellation`**. No `superseded_by_id`, no
   `rebuilt_from_id`, no `deprecated_at` — no way to recover the chain. The
   only tie between old and new is the (now-overwritten) secondary index,
   which doesn't exist anymore after the rebuild.
2. **`CF_CONSTELLATIONS` has no time-travel query API**. No `list_since(ts)`,
   no `history_for_selector(sel)`. The orphan is unreachable from any public
   read path — it's storage dead weight, not an audit trail.

If audit/blame-line were the real intent, the codebase would carry an
explicit lineage field. It doesn't.

## Minimum elegant fix

Do an atomic swap inside `store_constellation` when the index already points
at a different UUID. Takes advantage of the existing `WriteBatch` in that
function.

```rust
// crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs
pub async fn store_constellation(&self, c: &Constellation) -> CoreResult<()> {
    let payload = encode_constellation(c)?;
    let primary_cf = self.cf_constellations();
    let index_cf = self.cf_constellation_by_selector();
    let index_key = selector_index_key(&c.selector);
    let new_id_bytes = *c.id.as_bytes();

    // Check the existing index pointer. If it points at a *different* UUID,
    // we are about to orphan that primary row — delete it atomically in the
    // same batch. If it points at us (re-store of the same UUID) or doesn't
    // exist, no deletion needed.
    let prior_id: Option<[u8; 16]> = match self.db.get_cf(index_cf, &index_key) {
        Ok(Some(bytes)) if bytes.len() == 16 && bytes != new_id_bytes => {
            let mut buf = [0u8; 16];
            buf.copy_from_slice(&bytes);
            Some(buf)
        }
        Ok(_) => None,
        Err(e) => return Err(TeleologicalStoreError::rocksdb_op(
            "probe_selector_index", CF_CONSTELLATION_BY_SELECTOR, None, e,
        ).into()),
    };

    let mut batch = WriteBatch::default();
    if let Some(old) = prior_id {
        batch.delete_cf(primary_cf, &old);
    }
    batch.put_cf(primary_cf, c.id.as_bytes(), &payload);
    batch.put_cf(index_cf, &index_key, c.id.as_bytes());

    self.db.write(batch).map_err(|e| {
        /* ... error mapping ... */
    })?;

    if let Some(old) = prior_id {
        debug!(
            new_id = %c.id,
            old_id = %Uuid::from_bytes(old),
            "Replaced prior constellation for selector (atomic swap)"
        );
    }
    Ok(())
}
```

Update the docstring at lines 64-69 to match:

```rust
/// ... when the selector's canonical form maps to an index entry that
/// already points at a different constellation UUID, the old primary row
/// is deleted in the same batch and the index is updated to point to the
/// new UUID — all three operations are atomic.
```

Then delete the FSV post-step that manually calls `delete_constellation`
after each rebuild (that's now redundant).

## Why this is simpler than the alternatives

1. **Keep the WriteBatch atomic** — no risk of index pointing at a deleted
   primary or vice-versa.
2. **One read + one potential delete, all in the path already taken** — the
   function already does a write; adding a conditional read + delete costs
   one extra RocksDB point-get on the cold path.
3. **No new tool, no new state field, no audit table**. The orphan stops
   happening.
4. **Matches `delete_constellation`'s existing "only delete when pointer
   still matches"** defensive pattern (constellation.rs:184-193). Symmetric.
5. **Satisfies the caller's intent**: "rebuildIfExists=true" means "replace
   the old one"; not "make a new one and silently leak the old one".

## Alternative considered and rejected

Teaching `call_compile_constellation` to fetch the old UUID before storing
the new one and call `delete_constellation` afterwards. Rejected because:

- Non-atomic: if the process dies between the two calls, the DB is still
  consistent (old UUID still pointed to by index, but compile's return
  value reported the new UUID which doesn't exist) — bad.
- Splits the invariant "at most one primary row per selector" across two
  CF-level operations in two different crates.
- Doubles the RocksDB I/O.

## Rationale

`store_constellation` is the single point that owns both the primary and the
secondary index. It's the right layer for the atomic swap. The handler
shouldn't know about storage-level orphan-prevention.
