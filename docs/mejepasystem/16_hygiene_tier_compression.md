# 16 — Hygiene + Tier Compression

Source files covered: `crates/context-graph-mejepa-hygiene/src/{lib,gc,quota,quota_types,tier,tier_frozen,tier_ops,witness_compress,ops,mcp,reports,storage,entry,categories,error}.rs`, `crates/context-graph-tier-compression/src/{lib,tests}.rs`, `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`.

---

## 1. Tier compression algorithm

Source: `crates/context-graph-tier-compression/src/lib.rs`. **8/7/5/3-bit groupwise linear scalar quantization** with 22-byte magic+version header.

| Tier | Bits | Compression ratio | Cosine retention (768-d Gaussian) | Bytes for 768-d vec |
|---|---|---|---|---|
| Hot | f32 raw | 1× | 1.000 | 3,072 |
| Warm | 8-bit groupwise | 3.89× | 0.99999 | 790 |
| Cool | 5-bit groupwise | 6.12× | 0.99960 | 502 |
| Cold | 3-bit groupwise | 9.91× | 0.99192 | 310 |
| Frozen | 1-bit / mark | — | — | minimum |

Header: magic + version + per-group `(min, max, scale)` so dequantization is exact (no hardcoded `[-1, 1]` — fixed pq-8 bug per `MEMORY.md`).

`tier_for_score(access_count_30d) → Hot/Warm/Cool/Cold/Frozen`. `decode_from_tier` round-trips losslessly except for the quantization noise above.

---

## 2. Per-CF tier policy

| CF | Hot | Warm | Cool | Cold |
|---|---|---|---|---|
| `CF_MEJEPA_PANELS` | 8-bit | 7-bit | 5-bit | 3-bit |
| `CF_MEJEPA_DDA_SIGNALS` | 8-bit | 7-bit | 5-bit | 3-bit; ttl 365d |
| `CF_MEJEPA_FAILURE_EXEMPLARS` | 8-bit | 8-bit (operator-pinned) | 5-bit | 3-bit; ttl never |
| `CF_MEJEPA_AGENT_FEEDBACK` | 8-bit | 8-bit | 5-bit | never aged out without operator override |
| `CF_MEJEPA_TRAIN_CERTS` | n/a | n/a | n/a | n/a (small JSON; not tier-compressed) |
| `CF_MEJEPA_WEIGHT_BLOBS` | n/a | n/a | n/a | n/a (binary; not quantized) |
| Witness chain | n/a | n/a | n/a | Merkle compress > 90d |

---

## 3. Quota enforcement

Source: `quota.rs`. **115 GB hard cap** on `/mnt/d/contextgraph/`.

```rust
pub fn quota_status(d_root: &Path) -> Result<QuotaReport, QuotaError>;
pub fn quota_check_and_evict(d_root: &Path, target_gb: f64) -> Result<EvictionReport, QuotaError>;
```

| Threshold | Behavior |
|---|---|
| > 100 GB | Warning log |
| > 110 GB | Begin emergency eviction (`emergency_eviction_tick` @ 600s) |
| > 115 GB | Hard cap — refuse panel writes; alert operator (`MEJEPA_QUOTA_EXCEEDED`) |

Override via `CG_MEJEPA_DISK_QUOTA_GB` env var.

---

## 4. Quota allocation (115 GB cap)

| Slice | Budget | Tier policy |
|---|---|---|
| `storage/contextgraph-rocksdb/` (101 CFs) | 70 GB hot + warm; cool ~25 GB | 8/7/5/3 |
| `runtime/cgreality-shift-log/` | 5 GB rolling | per-session GC @ 30 days |
| `corpus/` | operationally large; managed outside the hot 115 GB cap when full runs active | archived after replacement verified |
| `storage/witness/` | 2 GB hot + Merkle-compressed >90d | per Phase 6 REQ-HYG-03 |
| `storage/cold/` | up to 25 GB cold blobs | 3-bit / 1-bit frozen |
| `storage/replay/` | 5 GB | replay-buffer policy |
| `logs/` | 2 GB rolling (daily rotation, 30-day retention) | logrotate |
| `fsv/` | 1 GB | per-crate FSV size cap |
| `exports/` | 2 GB rolling (90-day retention) | weekly snapshots |
| `state/` + `state/gold-labels/` | 100 MB | tiny but pinned |
| Models (`/mnt/d/cabdru/contextgraph/models/`) | separate budget | not in 115 GB cap |
| **Total cap** | **115 GB** | hard ceiling |

---

## 5. Nightly GC (`gc_run_nightly`)

Source: `gc.rs::gc_run_nightly` → `GcReport`. Runs at 03:00 local (configurable). Operations:

```
gc_run_nightly:
  1. tier_demote_all: for each CF that participates, decay scores and demote
     panels whose access_count_30d dropped
  2. compact: rocksdb::compact_range for each CF
  3. evict: drop entries below sampling_weight floor (0.05) per replay policy
  4. prune: remove shift-log JSONLs > 30 days old
  5. verify: integrity check on a sample of recent witness entries
```

Writes `GcReport` to `/mnt/d/contextgraph/exports/gc/<date>/report.json`. FSV `gc_stress_fsv` proves 10,000-entry stress run.

---

## 6. Emergency eviction (`quota_check_and_evict`)

```rust
quota_check_and_evict(d_root, target_gb):
  if current_gb > target_gb:
    1. tier_demote_all (cool blobs → cold)
    2. Drop oldest cold-tier entries by `last_accessed`
    3. Prune shift-log JSONLs > 7 days old (aggressive)
    4. If still over, drop oldest replay-buffer entries
    5. Alert operator if eviction insufficient
```

`emergency_eviction_tick` runs every 600s in daemon mode.

---

## 7. Witness Merkle compression

Source: `witness_compress.rs::witness_compress_old_segments`. See [`15_witness_chain.md §5`](15_witness_chain.md).

| Function | Purpose |
|---|---|
| `witness_compress_old_segments(db, max_age_days=90)` | Roll up ≥256-entry segments into Merkle roots |
| `verify_witness_integrity(db)` | Full chain replay including compressed segments |
| `audit_verify_old_entry(entry_id, proof)` | Single-entry verification against compressed Merkle proof |

---

## 8. MCP tools (3, all shipped)

Source: `mcp.rs`.

| Tool | Behavior |
|---|---|
| `mejepa_gc_run` | Force `gc_run_nightly`; supports `--target-cfs` + `--force-eviction` |
| `mejepa_quota_status` | Per-slice breakdown + total vs cap |
| `mejepa_witness_compress` | Force compression of segments older than `up_to_offset` |

Plus 5 lower-level ops via `ops.rs::*` (programmatic, not MCP-callable directly):
`gc_run_nightly`, `quota_check_and_evict`, `heal_detect_drift`, `heal_retrain_if_needed`, `witness_compress_old_segments`, `witness_verify_integrity`, `tier_demote`, `tier_promote`.

---

## 9. Tier transition rules

Source: `tier.rs::tier_for_score`.

```rust
fn tier_for_score(access_count_30d: u32) -> Tier {
    match access_count_30d {
        0..=5     => Tier::Cold,    // 3-bit
        6..=30    => Tier::Cool,    // 5-bit
        31..=100  => Tier::Warm,    // 7-bit  (or 8-bit operator-pinned for failure exemplars)
        _         => Tier::Hot,     // 8-bit raw
    }
}

fn decayed_score(score: f32, days: u32) -> f32;  // decay^days
fn score_after_read(score: f32) -> f32;          // bump on read
fn encode_for_tier(vec: &[f32], tier: Tier) -> Vec<u8>;
fn decode_from_tier(bytes: &[u8], tier: Tier) -> Result<Vec<f32>>;
fn tier_demote(item) -> NewTier;
fn tier_promote(item) -> NewTier;
fn tier_demote_all(db) -> DemoteReport;
fn tier_promote_all(db) -> PromoteReport;
```

`tier_frozen.rs` handles the 1-bit / `frozen=true` mark for constellation cells past freshness + locked baselines.

---

## 10. FSV evidence

| FSV | Output |
|---|---|
| Tier compression round-trip | `/tmp/contextgraph-tier-compression-fsv/blob_{8,7,5,3}.bin` (790/694/502/310 bytes; cosine 0.99999/0.99997/0.99960/0.99192) |
| 10k-entry GC stress | `/tmp/contextgraph-mejepa-hygiene-fsv/gc_stress_fsv/` mode 0600 |
| Merkle compression | `merkle_compression_fsv/` |
| Quota overflow | `quota_overflow_fsv/` |

---

## 11. Cross-references

[`04_database_schema.md`](04_database_schema.md) (CF-level tier policy), [`15_witness_chain.md`](15_witness_chain.md) (Merkle compression details), [`12_self_healing_scheduler.md`](12_self_healing_scheduler.md) (scheduler tickers), [`22_operator_runbook.md`](22_operator_runbook.md) (operator hygiene actions).
