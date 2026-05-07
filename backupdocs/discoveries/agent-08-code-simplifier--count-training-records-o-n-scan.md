---
namespace: discoveries
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:20:00Z
status: active
tags: performance, rocksdb, count, o-n-scan
---

# count_{training_records, constellations, contrastive_pairs} perform O(N) full-CF iterator scans

## Classification
**NIT / SIMPLIFICATION** — duplicates prior review's L-4 with one extra
callsite (count_contrastive_pairs) that the prior review did not call out.

## Severity
**Low** — correctness is fine; callers that use `count_*` on a large CF pay
a full-keyspace scan per call.

## Location

All three Phase-1/2/3 CFs adopt the same O(N) implementation:

1. `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs:137-140`
   ```rust
   pub async fn count_training_records(&self) -> CoreResult<usize> {
       let cf = self.cf_training_records();
       Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
   }
   ```
2. `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs:153-157`
   ```rust
   pub async fn count_constellations(&self) -> CoreResult<usize> {
       let cf = self.cf_constellations();
       Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
   }
   ```
3. `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs`
   — `count_contrastive_pairs` and `count_contrastive_pairs_by_kind` also
   use iterator-scan semantics (full or prefix).

## Issue

RocksDB exposes `property_int_value_cf(cf, "rocksdb.estimate-num-keys")` in
O(1) — it returns an estimate that is accurate to within compaction lag
(usually ±few-% on stable databases). The training-data MCP tools call
`count_training_records` once per invocation today; at 10k rows this is
sub-millisecond, but at 10M rows it becomes a ~second-long scan per
request.

The prior review L-4 flagged 2 of the 3 sites; this adds `contrastive` as
the third.

## Minimum fix

A small shared helper in `teleological/rocksdb_store/mod.rs`:

```rust
pub(crate) fn approx_count_cf(
    db: &DB, cf: &ColumnFamily, exact_fallback: impl FnOnce() -> usize,
) -> usize {
    db.property_int_value_cf(cf, "rocksdb.estimate-num-keys")
        .ok()
        .flatten()
        .map(|v| v as usize)
        .unwrap_or_else(exact_fallback)
}
```

Then each `count_*` becomes:

```rust
pub async fn count_training_records(&self) -> CoreResult<usize> {
    let cf = self.cf_training_records();
    Ok(approx_count_cf(&self.db, cf, || {
        self.db.iterator_cf(cf, IteratorMode::Start).count()
    }))
}
```

Update the docstring to say "returns the RocksDB-estimated key count; may
lag behind writes by up to one compaction cycle".

## Recommendation

Optional polish. Only worth landing if the FSV report will surface
"approximate" as a user-visible tradeoff. For internal tooling at current
scale (≤100k rows) the iterator scan is fine.

If touched, use the helper to fix all three at once so future CFs inherit
the same O(1) convention by default.

## Rationale

Writing a shared helper upfront is cheaper than chasing down the same
pattern in Phase-7+ CFs; each new CF tends to copy the existing shape.
