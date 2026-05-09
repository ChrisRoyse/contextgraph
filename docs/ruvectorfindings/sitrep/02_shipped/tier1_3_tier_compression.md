# Tier-1.3 Tier Compression (8/7/5/3-bit Groupwise Quantization)

> **Shipped:** 2026-05-09
> **Crate:** `crates/context-graph-tier-compression/`
> **Tests:** 24 passing
> **FSV evidence:** `/tmp/contextgraph-tier-compression-fsv/`

## What this primitive does

Generic groupwise scalar quantization at four bit-widths: 8, 7, 5, 3.
Used by Phase 6 to tier panel and checkpoint storage by access frequency
and age:

- **8-bit** — hot tier. Mean round-trip error ~0.002 σ. Fast decode.
- **7-bit** — middle hot/warm. Mean error ~0.005 σ.
- **5-bit** — warm tier. Mean error ~0.020 σ.
- **3-bit** — cold tier. Mean error ~0.060 σ. Replay-only.

API surface:
- `encode(values: &[f32], bits: BitWidth) -> Vec<u8>` produces a
  self-describing byte blob: 22-byte header + LSB-first packed bitstream.
- `decode(blob: &[u8]) -> Result<Vec<f32>>` recovers the original
  vector lossily. Returns explicit error on malformed header.
- `serialize(values, bits)` / `deserialize(blob)` are the JSON-friendly
  high-level wrappers.

## Why it matters

Per Phase 6 §B disk-budget table, the 1.9 TB of uncompressed per-step
checkpoints would blow past any reasonable disk budget. 8-bit hot-tier
is 4× compression; 3-bit cold-tier is ~10× compression. Combined with
checkpoint deduplication via Merkle-DAG, the 1.9 TB → ~150 GB target.

Panels at 5,120-d × 4 bytes (f32) = 20 KB each; at 3-bit = 1.92 KB. With
1,000 attempts × 300 tasks × 3 panels each = 900,000 panels, total
storage is 18 GB raw → 1.7 GB at 3-bit cold.

## Implementation notes

### 22-byte header layout
```
offset  size  field
  0      4    magic "CGTC" (ASCII bytes)
  4      1    version (currently 1)
  5      1    bits (3, 5, 7, or 8)
  6      8    n: number of f32 values (u64 LE)
 14      4    min: f32 LE (the scaling reference low)
 18      4    max: f32 LE (the scaling reference high)
```

### packed_len overflow guard
- `packed_len(n: usize, bits: u8) -> Option<usize>` uses `checked_mul`
  to detect `n * bits > usize::MAX / 8` overflow and returns `None`.
- Cascade: encode uses `.expect()` (slice-bounded so overflow is
  unreachable); decode + deserialize fail-closed via `ok_or_else`.

### Serde fail-closed
- `Panel<f32>` deserialization uses `serde(try_from = "PanelSerde")` so
  a malformed-source-of-truth blob fails at deserialize time, never
  silently producing wrong f32 values.

### LSB-first bitstream
- Packing reads bits low-to-high within each byte for portability with
  the standard "bitstream packed" convention used by zstd, Parquet, etc.

### Refinements caught during build
- **R1**: Original code had inline magic bytes at 5+ sites; refactored
  to a single `MAGIC: &[u8; 4] = b"CGTC"` constant.
- **R3**: `usize::try_from(u64)` for the `n` field with explicit error
  prevents silent narrowing on 32-bit targets.

### FSV
Physical FSV at `/tmp/contextgraph-tier-compression-fsv/`:
- `roundtrip-evidence.json` — 1,000 random f32 vectors at each bit
  width; mean absolute error per width recorded:
  - 8-bit: 0.0021 σ
  - 7-bit: 0.0050 σ
  - 5-bit: 0.0193 σ
  - 3-bit: 0.0586 σ
- `header-tamper-evidence.json` — 4 tamper cases (wrong magic, future
  version, invalid bits, oversized n) all produce explicit errors.
- `overflow-evidence.json` — synthetic `n * bits` overflow case
  produces `TIER_COMPRESSION_PACKED_LEN_OVERFLOW` error.

## What this enables downstream

- **Phase 6 panel storage tiering** — direct consumer.
- **Phase 6 checkpoint deduplication** — checkpoints get hashed and
  tier-compressed before write.
- **Future SONA training quantization** — same primitive can encode
  intermediate model weights for streaming.

## See also

- Spec: `docs/ruvectorfindings/05_TIERED_PRIMITIVE_LADDER.md §1.3`
- Phase 6 wiring plan: `04_implementation_plans/plan_phase6_storage_wiring.md`
- Memory entry: (none — direct primitive, no memory record needed)
- Source: `crates/context-graph-tier-compression/src/lib.rs`
