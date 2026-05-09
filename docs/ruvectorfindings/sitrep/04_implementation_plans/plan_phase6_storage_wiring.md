# Plan: Phase 6 — Storage Wiring (Tier-Compression + Merkle DAG)

> **Priority:** MEDIUM (after Phase 0 CFs + Phase 2 panel materializer).
> **Effort:** ~3-5 days.
> **Crate:** `crates/context-graph-mejepa-storage/` (new wiring crate).

## Goal

Wire the shipped `context-graph-tier-compression` primitive into the
panel and checkpoint storage layers so a 300-task training run does
not blow past disk and RAM budgets.

Specifically:
- Hot-tier panels (last 7 days, frequent access) at 8-bit groupwise.
- Warm-tier panels (7-30 days) at 5-bit.
- Cold-tier panels (>30 days) at 3-bit.
- Checkpoint deduplication: identical `theta_sha` chain entries
  reference a single blob via Merkle-DAG content addressing.

## Deliverables

1. New crate `crates/context-graph-mejepa-storage/` with:
   - `panel_store.rs` — read/write panels with automatic tier
     compression based on access patterns.
   - `tiering.rs` — background retiering task.
   - `checkpoint_dag.rs` — Merkle-DAG checkpoint deduplication.
   - `gc.rs` — cold-panel garbage collection.
2. RocksDB CFs added to `context-graph-storage`:
   - `CF_MEJEPA_PANELS` — `(attempt_id, time_step) → tier_compressed_blob`.
   - `CF_MEJEPA_PANEL_META` — `(attempt_id, time_step) → PanelMeta`
     (tier, access_count_30d, created_at, last_accessed).
   - `CF_MEJEPA_WEIGHT_BLOBS` — `theta_sha → bf16_weights_blob`
     (deduplicated).
   - `CF_MEJEPA_TRAIN_CERTS` — `step → TrainingCertHeader` referencing
     the Merkle root over the four shas.
3. 25+ unit tests + an integration test exercising 1,000 synthetic
   attempts × 3 panels each = 3,000 panel writes.
4. FSV at `/tmp/contextgraph-mejepa-storage-fsv/`.

## Definition of Done (per doc 09 §7)

- After 1,000 attempts × 300 tasks of synthetic load, total disk usage
  <500 GB (vs ~1.9 TB uncompressed).
- Hot-tier panel decompression latency <1 ms on RTX 5090.
- Cold-tier panel decompression latency <5 ms.
- GC policy verified: 60-day-stale synthetic dataset has cold panels
  dropped, witness-chain hashes preserved.
- Checkpoint dedupe ratio measured and reported.

## Verification protocol

```bash
context-graph mejepa-storage stats --runtime-root ./runtime
# expected:
#   total_disk_usage_gb: <500
#   panels_by_tier: { hot: 8000, warm: 25000, cold: 75000 }
#   checkpoint_blobs: ~1234
#   checkpoint_dedupe_ratio: ~0.85
#   median_decompress_latency_ms: { hot: 0.6, warm: 1.8, cold: 4.2 }
```

## Component breakdown

### A. Tier classification (~0.5 days)

```rust
pub enum Tier { Hot, Warm, Cold }

pub fn classify_panel(meta: &PanelMeta, now: Instant) -> Tier {
    let age_days = (now - meta.created_at).as_secs() / 86400;
    let access_30d = meta.access_count_last_30_days;
    match (age_days, access_30d) {
        (0..=6, _) => Tier::Hot,
        (7..=29, n) if n >= 2 => Tier::Hot,    // promote frequent
        (7..=29, _) => Tier::Warm,
        (_, n) if n >= 5 => Tier::Warm,         // skip-demote
        _ => Tier::Cold,
    }
}
```

Hysteresis: a panel that's just promoted hot waits 24 h before
considering demotion, to prevent thrash.

### B. Panel store + retiering (~1.5 days)

```rust
pub struct MeJepaPanelStore { db: Arc<DB>, ... }

impl MeJepaPanelStore {
    pub fn put_panel(&self, attempt_id: &AttemptId, time_step: TimeStep, panel: &Panel)
        -> Result<()>
    {
        let bits = match Tier::Hot.bit_width() {  // new panels start hot
            BitWidth::Eight => BitWidth::Eight,
            _ => unreachable!(),
        };
        let compressed = tier_compression::encode(&panel.data, bits)?;
        let key = encode_panel_key(attempt_id, time_step);
        self.db.put_cf(&self.cf_panels, &key, &compressed)?;
        self.db.put_cf(&self.cf_panel_meta, &key, &PanelMeta::new(Tier::Hot).serialize())?;
        Ok(())
    }

    pub fn get_panel(&self, attempt_id: &AttemptId, time_step: TimeStep)
        -> Result<Panel>
    {
        let key = encode_panel_key(attempt_id, time_step);
        let compressed = self.db.get_cf(&self.cf_panels, &key)?
            .ok_or(StoreError::NotFound)?;
        let panel_data = tier_compression::decode(&compressed)?;
        // bump access counter
        self.bump_access(&key)?;
        Panel::try_new(panel_data, /* mask */ 0xFFFF)  // assumes all slots
    }
}
```

Background retiering task:

```rust
pub async fn retier_loop(store: Arc<MeJepaPanelStore>) {
    loop {
        for entry in store.list_panels_by_oldest(1000) {
            let target = classify_panel(&entry.meta, Instant::now());
            if entry.meta.current_tier != target {
                // re-encode at the new bit width
                let raw = tier_compression::decode(&entry.compressed)?;
                let new_compressed = tier_compression::encode(&raw, target.bit_width())?;
                store.replace_panel(&entry.key, &new_compressed, target)?;
            }
        }
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
```

### C. Checkpoint Merkle DAG (~1 day)

```rust
pub struct WeightBlobStore { ... }

impl WeightBlobStore {
    pub fn store_weights(&self, weights: &[f32]) -> Result<Sha256> {
        let bytes = bytemuck::cast_slice(weights);
        let sha = sha2::Sha256::digest(bytes).into();
        if !self.db.key_may_exist_cf(&self.cf_weight_blobs, &sha) {
            self.db.put_cf(&self.cf_weight_blobs, &sha, bytes)?;
        }
        Ok(sha)
    }
}

pub fn build_merkle_root(theta: Sha256, xi: Sha256, phi: Sha256, opt: Sha256) -> Sha256 {
    let mut h = sha2::Sha256::new();
    h.update(&theta);
    h.update(&xi);
    h.update(&phi);
    h.update(&opt);
    h.finalize().into()
}
```

The training cert (Phase 3) stores `(merkle_root, theta_sha, xi_sha,
phi_sha, opt_sha)`; the actual weight blobs live in the dedupe CF.

### D. GC (~0.5 days)

```rust
pub async fn gc_cold_panels(store: Arc<MeJepaPanelStore>) {
    let cutoff = Instant::now() - Duration::from_secs(60 * 86400);
    for (key, meta) in store.scan_panel_meta() {
        if meta.created_at < cutoff
            && meta.tier == Tier::Cold
            && meta.access_count_last_30_days == 0
        {
            store.delete_panel(&key)?;
            // witness chain entry remains
        }
    }
}
```

## Dependencies

- ✅ `context-graph-tier-compression` (shipped 2026-05-09).
- ⏳ Phase 0 RocksDB CF migration framework (need to add the four CFs
  above).
- ⏳ Phase 2 panel materializer (Sub-phase 2.A) producing the writes.
- ⏳ Phase 3 training loop producing checkpoint writes.

## Risks

1. **3-bit cold-tier precision loss**: 0.06 σ mean error may break
   replay-from-cold for exact panel reconstruction. If replay needs
   bit-exact, switch cold to 5-bit (0.02 σ error).
2. **Retiering thrash**: 10% hysteresis margin prevents oscillation.
3. **SHA-256 collision**: cryptographically infeasible; no mitigation.

## Fresh-agent recipe

1. Read `../03_deferred/phases/phase6.md` for full context.
2. Read `02_shipped/tier1_3_tier_compression.md` for the existing
   primitive.
3. Add the 4 CFs to `context-graph-storage` via the existing CF
   migration pattern.
4. Implement the 4 sub-modules in the new crate.
5. Synthetic 1,000-attempt FSV harness.
6. Verify §7 DoD numbers.
7. Update `01_status_overview.md`.
8. End-of-turn SMAP report.

## See also

- Phase 6 phase doc: `../03_deferred/phases/phase6.md`
- Tier-compression primitive: `02_shipped/tier1_3_tier_compression.md`
- Phase 0 deferred dependency: `../03_deferred/phases/phase0.md` (CFs).
- Phase 2 dependency: `../03_deferred/phases/phase2.md` (panel writer).
- Phase 3 dependency: `../03_deferred/phases/phase3.md` (checkpoint
  writer).
