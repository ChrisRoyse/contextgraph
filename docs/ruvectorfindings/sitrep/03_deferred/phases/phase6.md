# Doc 09 Phase 6 — Disk + Memory Hygiene

> **Estimated effort (per doc 09 §7):** 3-5 days.
> **Status (2026-05-09):** DEFERRED — depends on Phase 0 RocksDB CFs +
> Phase 2 panel materializer. Foundation ✅ shipped (`context-graph-tier-compression`).

## Goal

Phase 6 wires the shipped tier-compression primitive into the panel and
checkpoint storage layers so a 300-task training run does not blow past
disk and RAM budgets. Concretely:

1. Hot-tier panels (last 7 days, batch-2 frequent access) at 8-bit
   group-wise quantization.
2. Warm-tier panels (7-30 days, occasional access) at 5-bit.
3. Cold-tier panels (>30 days, replay-only) at 3-bit.
4. Checkpoint deduplication: identical `theta_sha` chain entries reference
   a single blob via Merkle-DAG content addressing.

Per doc 09 §1.1's compiler analogy, Phase 6 is the **link-time optimizer**:
takes mature artifacts and rewrites them for size without changing
semantics.

## Why this phase matters

Per doc 09 §7 disk-budget table:

| Artifact | Per-task cost | 300-task total | Budget |
|---|---|---|---|
| Mutation corpus | ~50 KB | ~15 MB | ✅ fits |
| Per-attempt panels | ~60 KB raw | ~17 MB/attempt | ⚠️ explodes at 1k+ attempts |
| Training certs | ~250 B | ~1 MB / 3,750 steps | ✅ fits |
| Per-step model snapshots (uncompressed bf16) | ~500 MB | ~1.9 TB at 3,750 steps | ❌ NEEDS PHASE 6 |
| Witness chain | ~73 B / step | ~280 KB | ✅ fits |

Per-step snapshots are unsustainable without tier-compression. Even
8-bit hot-tier brings the 1.9 TB → ~500 GB (still a lot, so we ALSO
deduplicate via Merkle-DAG content addressing).

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa-storage/`** — wiring crate that
   adapts `context-graph-tier-compression` to the RocksDB layer.
2. **Panel storage tiering**: a background task that walks
   `CF_MEJEPA_PANELS`, retiers panels by age (and by access-frequency
   heuristic from a lightweight read counter).
3. **Checkpoint deduplication**: each per-step training cert (Phase 3)
   stores `theta_sha` etc.; the actual weight blobs land in
   `CF_MEJEPA_WEIGHT_BLOBS` keyed by SHA-256. If two steps produce
   bit-identical weights (rare but possible in a fixed-seed dry-run),
   only one blob is stored.
4. **Merkle-DAG content addressing**: a checkpoint = root hash over
   (theta_sha, xi_sha, phi_sha, optimizer_state_sha). Replay walks the
   DAG. Saves ~2× by sharing optimizer state across nearby steps.
5. **GC policy**: cold-tier panels older than 60 days with zero hits are
   dropped (their hashes remain in the witness chain for replay).

## Definition of Done (per doc 09 §7)

- After 1,000 attempts × 300 tasks, total disk usage <500 GB (vs ~1.9 TB
  uncompressed).
- Hot-tier panel decompression latency <1ms on RTX 5090.
- Cold-tier panel decompression latency <5ms.
- GC policy verified by 60-day-stale synthetic dataset: cold panels
  dropped, witness-chain hashes remain.

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-storage stats --runtime-root ./runtime
# expected:
#   total_disk_usage_gb: <500
#   panels_by_tier:
#     hot: 8000   (last 7 days)
#     warm: 25000 (7-30 days)
#     cold: 75000 (>30 days)
#   checkpoint_blobs: 1234
#   checkpoint_dedupe_ratio: 0.85  (15% of references share blobs)
#   median_decompress_latency_ms: { hot: 0.6, warm: 1.8, cold: 4.2 }
```

## Component breakdown

### A. Tier classification

```rust
enum Tier { Hot, Warm, Cold }

fn classify_panel(panel_meta: &PanelMeta, now: Instant) -> Tier {
    let age_days = (now - panel_meta.created_at).as_secs() / 86400;
    let access_count_30d = panel_meta.access_count_last_30_days;
    match (age_days, access_count_30d) {
        (0..=6, _) => Tier::Hot,
        (7..=29, n) if n >= 2 => Tier::Hot,  // promotion: hot if frequent
        (7..=29, _) => Tier::Warm,
        (_, n) if n >= 5 => Tier::Warm,      // demotion-skip: warm if hot+
        _ => Tier::Cold,
    }
}
```

### B. Background retiering task

```rust
async fn retier_loop(store: Arc<MeJepaStore>) {
    loop {
        for panel_id in store.list_panels_oldest_first(1000) {
            let meta = store.get_panel_meta(&panel_id)?;
            let target = classify_panel(&meta, Instant::now());
            if meta.current_tier != target {
                let raw = store.read_panel_raw(&panel_id)?;
                let recompressed = match target {
                    Tier::Hot => tier_compression::encode(&raw, BitWidth::Eight)?,
                    Tier::Warm => tier_compression::encode(&raw, BitWidth::Five)?,
                    Tier::Cold => tier_compression::encode(&raw, BitWidth::Three)?,
                };
                store.write_panel_compressed(&panel_id, &recompressed, target)?;
            }
        }
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
```

### C. Checkpoint deduplication

```rust
fn store_checkpoint(
    store: &MeJepaStore,
    weights: &Tensor,
) -> Result<Sha256> {
    let blob = bytes_of_tensor(weights);
    let sha = sha256(&blob);
    if !store.cf_weight_blobs.contains_key(&sha)? {
        store.cf_weight_blobs.put(&sha, &blob)?;
    }
    Ok(sha)
}

fn store_training_cert(
    store: &MeJepaStore,
    step: u64,
    theta_sha: Sha256,
    xi_sha: Sha256,
    phi_sha: Sha256,
    optimizer_sha: Sha256,
) -> Result<MerkleRoot> {
    let merkle_root = sha256_concat(&[theta_sha, xi_sha, phi_sha, optimizer_sha]);
    store.cf_train_certs.put(&step.to_be_bytes(), &TrainingCertHeader {
        merkle_root, theta_sha, xi_sha, phi_sha, optimizer_sha, ..
    }.serialize())?;
    Ok(merkle_root)
}
```

### D. GC policy

```rust
async fn gc_cold_panels(store: Arc<MeJepaStore>) {
    let cutoff = Instant::now() - Duration::from_secs(60 * 86400);
    for (panel_id, meta) in store.scan_cf_panel_meta() {
        if meta.created_at < cutoff
            && meta.tier == Tier::Cold
            && meta.access_count_last_30_days == 0
        {
            store.cf_mejepa_panels.delete(&panel_id)?;
            store.cf_panel_meta.delete(&panel_id)?;
            // The witness chain entry remains — replay can reconstruct
            // the panel from its corpus_id if needed.
        }
    }
}
```

## What's already shipped

- ✅ `crates/context-graph-tier-compression/` — 8/7/5/3-bit groupwise
  quantization with 22-byte CGTC header (magic + version + bits + n +
  min + max), `packed_len(n, bits) -> Option<usize>` overflow guard,
  `serde(try_from)` invariant gate. Used standalone.

What's missing (this phase):
- The wiring layer that calls into it from the panel & checkpoint store.
- The retiering background task.
- The Merkle-DAG checkpoint deduplication scheme.
- GC policy.

## Critical path within Phase 6

1. Phase 0 RocksDB CFs (`CF_MEJEPA_PANELS`, `CF_MEJEPA_PANEL_META`,
   `CF_MEJEPA_WEIGHT_BLOBS`, `CF_MEJEPA_TRAIN_CERTS`) — pre-requisite.
2. Phase 2 panel materializer — provides the writes that get retiered.
3. Tier classification + retiering task.
4. Checkpoint dedupe + Merkle-DAG.
5. GC policy.

## Dependencies

- ⏳ Phase 0 RocksDB CFs.
- ⏳ Phase 2 panel materializer (Sub-phase 2.A).
- ✅ `context-graph-tier-compression` (shipped 2026-05-09).
- ⏳ `context-graph-storage` extension for the new CFs.

## Implementation order (recommended)

**Sub-phase 6.A**: Wiring crate `context-graph-mejepa-storage` with
panel write/read paths. ~1 day.

**Sub-phase 6.B**: Tier classification + retiering background task.
~1.5 days.

**Sub-phase 6.C**: Checkpoint deduplication + Merkle-DAG. ~1 day.

**Sub-phase 6.D**: GC policy + 60-day-stale synthetic test. ~1 day.

## Risks (per doc 09 §8)

1. **3-bit cold-tier loses too much fidelity for replay**: tier-compression
   round-trip mean error at 3-bit is ~0.06 σ. If replay-from-cold ever
   needs to reproduce training-time exact panels, switch to 5-bit cold.
2. **Retiering thrash**: a panel that drifts hot→warm→hot every cycle
   wastes I/O. Gate retiering on a 10% hysteresis margin.
3. **Hash collision**: SHA-256 collisions are computationally
   infeasible; no mitigation needed beyond the standard primitive.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.8,
  §7`
- Shipped primitive: `02_shipped/tier1_3_tier_compression.md`
- Phase 0 dependency: `phase0.md`
- Phase 2 dependency: `phase2.md`
- Phase 3 dependency: `phase3.md` (writes the checkpoints this phase
  dedupes)
