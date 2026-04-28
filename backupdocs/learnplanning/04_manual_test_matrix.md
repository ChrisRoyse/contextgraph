# Manual Test Matrix

## E14 Happy Path

Trigger:

- Load BGE-M3 model from `models/bge-m3-dense`.
- Embed text `"The quick brown fox."`.

Expected:

- Output model id `BgeM3Dense`.
- Vector length 1024.
- L2 norm approximately 1.0.

Observed:

```text
len=1024
norm=1.000000
latency_us=104432
first5=[-0.03992623, 0.036425337, -0.060839016, -0.004773454, -0.050047856]
```

## E14 Quantized Storage Happy Path

Trigger:

- Store synthetic E14 PQ8 bytes in RocksDB `emb_13`.
- Close store.
- Reopen store.
- Read `load_embedder(id, 13)`.

Expected:

- Method `PQ8`.
- `original_dim=1024`.
- Bytes match exactly.

Observed:

```text
data=[0, 37, 74, 111, 148, 185, 222, 3]
loaded_full.embeddings.len()=14
```

## Edge Case 1 - Legacy JSON Missing E14

Before:

```text
legacy JSON has e14_bge_m3_dense? = false
```

After:

```text
restored.e14_bge_m3_dense.len() = 0
```

Outcome:

- Pass. Legacy records remain readable.

## Edge Case 2 - Empty E14 Quantization

Before:

```text
fp.e14_bge_m3_dense.is_empty() = true
```

After:

```text
q.e14_bge_m3_dense.data.len() = 0
is_empty() = true
```

Outcome:

- Pass. Empty legacy E14 stays empty and does not fake a vector.

## Edge Case 3 - Invalid Embedder Index

Trigger:

```text
load_embedder(id, 14)
```

Expected:

- Fail fast. Valid range is 0-13.

Observed:

```text
STORAGE ERROR: Invalid embedder index 14. Valid range: 0-13.
```

Outcome:

- Pass.

## Edge Case 4 - HNSW Wrong Dimension

Trigger:

- Query E14 HNSW with a vector that is not 1024D.

Expected:

- Reject without silent padding/truncation.

Observed:

```text
wrong-dim search -> error
```

Outcome:

- Pass.

## Edge Case 5 - Backfill Dry Run On Empty Live Store

Trigger:

```bash
target/debug/context-graph-cli backfill-e14 --storage contextgraph_data --dry-run --scan-batch 2000 --log-every 1000
```

Expected:

- If live store has no fingerprints, count remains zero.

Observed:

```text
backfill-e14: 0 seen, 0 already had E14, 0 backfilled, 0 skipped (missing content), 144ms
```

Outcome:

- Pass.

## Boundary Test Requirements For Future UTL Work

Every new UTL table/vector must include at least:

- Empty event stream.
- One valid event.
- Invalid dimension.
- Maximum label/vector length.
- Close/reopen readback.
- Delete/cleanup verification.

