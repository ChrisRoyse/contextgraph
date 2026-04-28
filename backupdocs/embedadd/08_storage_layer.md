# Storage Layer — CF + HNSW Integration

Specifies exactly how each new embedder plugs into the storage layer.

## New column families (summary)

| CF Name | Phase | Key | Value | Purpose |
|---------|-------|-----|-------|---------|
| `CF_E14_BGE_M3_DENSE` | 1 | memory_id (16B) | dim_hdr(4B) + 1024×f32 | Dense vectors |
| `CF_E15_BGE_M3_SPARSE_STORED` | 2 | memory_id | bincode Vec<(u32,f32)> | Sparse vector storage |
| `CF_E15_BGE_M3_SPARSE_INVERTED` | 2 | token_id(4B) + memory_id | weight(4B) | Inverted index |
| `CF_E16_BGE_M3_COLBERT` | 2 | memory_id | seq_len(4B) + seq_len×1024×f32 | Per-token vectors |
| `CF_E17_JINA_V3` | 3 | memory_id | dim_hdr(4B) + 1024×f32 | Full 1024D |
| `CF_E17_JINA_V3_MATRYOSHKA_128` | 3 | memory_id | dim_hdr(4B) + 128×f32 | 128D truncated |
| `CF_E18_QWEN3_EMBEDDING` | 4 | memory_id | dim_hdr(4B) + 1024×f32 | Full 1024D |
| _(none for E19)_ | 5 | — | — | Reranker is runtime-only |

**Total new CFs: 7**

## CF declaration template

Add to `crates/context-graph-storage/src/teleological/column_families.rs`:

```rust
/// E14: BGE-M3 dense vectors (1024D)
/// Stored as: u32 dim header + 1024 × f32 little-endian
pub const CF_E14_BGE_M3_DENSE: &str = "e14_bge_m3_dense";
```

Maintain alphabetical order within embedder block, matching existing
convention.

## RocksDB open sequence

All CFs must be listed at open time. File:
`crates/context-graph-storage/src/teleological/repository/open.rs` (or
wherever `DB::open_cf_descriptors` is called).

Add each new CF as a `ColumnFamilyDescriptor`. Miss any one and the DB will
refuse to open.

**Migration behavior:** if opening an existing DB that lacks new CFs,
create them automatically on first open. RocksDB's `open_cf_descriptors`
with `create_missing_column_families=true` handles this. **Verify this flag
is set in the existing code.**

## Per-record serialization detail

### Dense embedders (E14, E17 full, E17 matryoshka, E18)

```
Offset  Size   Content
------  ----   -------
0       4      little-endian u32 dim (sanity check)
4       4N     f32 array, little-endian, N floats
```

Serialize helper in `crates/context-graph-storage/src/teleological/serialization/dense.rs`:

```rust
pub fn pack_dense_vector(v: &[f32], expected_dim: usize) -> StorageResult<Vec<u8>> {
    if v.len() != expected_dim {
        return Err(StorageError::DimensionMismatch { ... });
    }
    let mut buf = Vec::with_capacity(4 + v.len() * 4);
    buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
    for f in v {
        buf.extend_from_slice(&f.to_le_bytes());
    }
    Ok(buf)
}

pub fn unpack_dense_vector(data: &[u8], expected_dim: usize) -> StorageResult<Vec<f32>> {
    if data.len() != 4 + expected_dim * 4 {
        return Err(StorageError::CorruptRecord { ... });
    }
    let stored_dim = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    if stored_dim != expected_dim {
        return Err(StorageError::DimensionMismatch { expected: expected_dim, got: stored_dim });
    }
    let mut out = Vec::with_capacity(expected_dim);
    for i in 0..expected_dim {
        let off = 4 + i * 4;
        out.push(f32::from_le_bytes(data[off..off+4].try_into().unwrap()));
    }
    Ok(out)
}
```

(Note: this matches the Python-side `_pack_dense`/`_unpack_dense` in
`scripts/v4_2/extended_store.py` — enabling Rust ↔ Python compat if ever
needed.)

### ColBERT per-token (E16)

```
Offset  Size   Content
------  ----   -------
0       4      seq_len (little-endian u32)
4       4     dim_per_token (= 1024)
8       4*seq_len*1024  flattened row-major f32 array
```

### Sparse (E15)

Same pattern as existing `CF_E13_SPLADE_INVERTED`. Use bincode for
`Vec<(u32, f32)>`:

```rust
let bytes = bincode::serialize(&sparse_vec)?;
```

Plus inverted-index entries for lookup.

## HNSW index schema

New HNSW indexes required:

| EmbedderIndex | Phase | Dim | M | ef_construction | ef_search | Max elements |
|---------------|-------|-----|---|-----------------|-----------|--------------|
| E14BgeM3Dense | 1 | 1024 | 16 | 200 | 100 | 2,000,000 |
| E16BgeM3ColbertPooled | 2 (optional) | 1024 | 16 | 200 | 100 | 2M |
| E17JinaV3 | 3 | 1024 | 16 | 200 | 100 | 2M |
| E17JinaV3Matryoshka128 | 3 | 128 | 16 | 200 | 100 | 2M |
| E18Qwen3Embedding | 4 | 1024 | 16 | 200 | 100 | 2M |

**Not indexed (by design):**
- E15 sparse (inverted index only, like E6/E13)
- E16 per-token (pooled variant optional; raw per-token via MaxSim at runtime)
- E19 reranker (runtime scorer, no stored vectors)

All HNSW registered in
`crates/context-graph-storage/src/teleological/indexes/registry.rs`.

## EmbedderIndex enum extension

File: `crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs`

```rust
pub enum EmbedderIndex {
    E1Semantic,
    E1Matryoshka128,
    // ... existing ...
    E13Splade,

    // Phase 1
    E14BgeM3Dense,

    // Phase 2
    E16BgeM3ColbertPooled,   // optional

    // Phase 3
    E17JinaV3,
    E17JinaV3Matryoshka128,

    // Phase 4
    E18Qwen3Embedding,

    // (E15 sparse, E16 per-token, E19 reranker do NOT appear here)
}
```

All methods on this enum (`dimension`, `is_dense`, `column_family_name`,
etc.) must be extended. Compiler catches omissions via exhaustive match.

## Index initialization at startup

When the daemon starts, `EmbedderIndexRegistry::new()` iterates all
`EmbedderIndex` variants and creates empty HNSW structures. On record
insert, the relevant indexes get the new vector.

For migration (re-embedding existing 2,770 records), the HNSW indexes get
populated by the reembedder tool.

## Size budget

Adding 7 new CFs + 5 HNSW indexes storage impact:

Per record on-disk cost (1 record embedded in all new embedders):
- E14 dense: 4100 bytes
- E15 sparse (avg 500 tokens): ~6 KB
- E16 ColBERT (avg 1000 tokens × 1024 × 4): ~4 MB **per record** — huge
- E17 full: 4100 bytes
- E17 matryoshka: 516 bytes
- E18: 4100 bytes

**Total per record: ~18 KB + ~4 MB for ColBERT = ~4 MB dominated by E16.**

For 2,770 records × 4 MB = ~11 GB just for ColBERT per-token storage. LMDB
sidecar was taking 3.6 GB; Rust RocksDB will be bigger due to WAL +
compaction overhead.

**Disk budget:** need 15+ GB free for full integration. Currently 175 GB
free — plenty.

## Dimension mismatch safety

Every packer/unpacker MUST validate the stored dim matches the expected dim
for that CF. If they drift (e.g., we swap BGE-M3 for a 768D variant
accidentally), reads return DimensionMismatch errors rather than silently
returning the wrong-size vector.

## Backups / snapshots

Existing `contextgraph_data/` dir is the source of truth. Before any CF
addition:

```bash
cp -a contextgraph_data contextgraph_data.pre_e14_backup_$(date +%s)
```

This is non-negotiable since CF changes are not trivially reversible.
Document in runbook.
