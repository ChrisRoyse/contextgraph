# Full State Verification Protocol

Every phase ends with FSV. No phase advances to the next until FSV passes.

## The 6-check FSV template

Adapted from the Python-side `scripts/v4_2/extended_fsv.py` pattern.

### Check 1: DISK

**What:** physical database state is correct.

**How:**
- Query: "does RocksDB have CF_E{N}_*?"
- Query: "how many records does that CF have?"
- Query: "does HNSW index for E{N} exist and contain N records?"

**Pass criteria:**
- CF exists in RocksDB file listing
- Record count == expected (e.g., 2,770 after reingestion)
- HNSW index count matches CF count

**Implementation:** direct RocksDB probe via storage crate, not MCP. We
want the source of truth, not a cached API response.

```rust
#[tokio::test]
async fn fsv_check_1_disk_e14() -> anyhow::Result<()> {
    let storage = TestStorage::open("contextgraph_data")?;
    let cf = storage.get_cf(CF_E14_BGE_M3_DENSE)?;
    let count = storage.count_cf(cf)?;
    assert!(count >= 2770, "expected ≥2770 E14 records, got {}", count);

    let registry = EmbedderIndexRegistry::new(&storage)?;
    let idx = registry.get(EmbedderIndex::E14BgeM3Dense)
        .ok_or_else(|| anyhow!("E14 HNSW not registered"))?;
    let hnsw_count = idx.len()?;
    assert_eq!(hnsw_count, count, "HNSW count {} != CF count {}", hnsw_count, count);
    Ok(())
}
```

### Check 2: SCHEMA

**What:** stored bytes have the right shape.

**How:**
- Sample 10 random records from CF
- For each, unpack and validate: dim header matches expected; bytes match
  `4 + dim * 4`; L2 norm > 0 and finite

**Pass criteria:** all 10 samples pass.

### Check 3: DETERMINISM

**What:** same input produces same output, bit-exact (or bf16-tolerance).

**How:**
- Pick 5 random memory_ids from CF
- For each, fetch content from CF_CONTENT
- Re-embed via the live model
- Compare stored vector to fresh: cos ≥ 0.9999

**Pass criteria:** all 5 pass.

### Check 4: DISCRIMINATION

**What:** the model actually does what it's supposed to.

**How:** held-out probe — real Shakespeare vs real modern prose.

- Embed Sonnet 29 (anchor, not used for constellation centroid)
- Embed Sonnet 18 (Shakespeare-style candidate)
- Embed a modern weather paragraph (non-Shakespeare candidate)
- Compute `gap = cos(anchor, sonnet) - cos(anchor, modern)`

**Pass criteria:** `gap ≥ 0.05` for dense embedders; `gap ≥ 0.08` for
sparse/per-token embedders; `gap ≥ 5.0 logit` for reranker.

These thresholds match Python-side Phase 1 smoke values.

### Check 5: EDGE CASES

**What:** the system fails cleanly, not silently.

**How:** 4 edge cases per embedder:
1. Empty text input → error raised (not zero-vector returned)
2. Missing memory_id in DB → KeyError-like structured error
3. Wrong-dim vector on write → ValidationError
4. Embedding model not loaded when embed() called → NotInitialized error

**Pass criteria:** each case produces the expected error type with a
descriptive message.

### Check 6: PHYSICAL EVIDENCE

**What:** proof-of-life log that humans can audit.

**How:** dump first 5 floats of 3 random records to log, with:
- memory_id
- content_sha256 prefix
- first 5 floats of each embedder output
- L2 norm
- reranker score vs a canonical Shakespeare query (if applicable)

**Pass criteria:** log entries are non-zero, finite, and match the
deterministic output for the content.

## Per-phase FSV extensions

### Phase 1 (E14 BGE-M3 dense) — 6 standard checks

### Phase 2 (E15 sparse + E16 ColBERT) — add check 7

**Check 7: BACKBONE SHARING**

**What:** BGE-M3 encoder weights are loaded once, not 3×.

**How:**
- Load E14, E15, E16 sequentially
- Check `Arc::strong_count(&encoder)` ≥ 2 after loading ≥2 of them
- Check VRAM usage: baseline (no BGE-M3) vs loaded (all 3) ≤ 3 GB delta
  (single copy of backbone + 3 small heads)

### Phase 3 (E17 Jina-v3) — add check 8

**Check 8: LoRA MERGE CORRECTNESS**

**What:** LoRA adapters were actually merged; output differs from base
encoder.

**How:**
- Load base Jina XLM-RoBERTa without LoRA
- Embed Sonnet 18 → baseline vector
- Load Jina with classification LoRA merged
- Embed same text → LoRA vector
- Assert: cos(baseline, LoRA) < 0.95 (LoRA meaningfully changed output)

### Phase 4 (E18 Qwen3) — add check 9

**Check 9: DECODER ARCHITECTURE CORRECTNESS**

**What:** causal mask actually works; RoPE is applied; GQA is correct.

**How:**
- Perturbation test: change token N of input, recompute. Only outputs at
  positions ≥ N should change (causal property).
- RoPE test: embed same content at different positions; only expected
  positions show the RoPE rotation pattern.
- GQA test: compare output to non-GQA reference (expand K/V heads manually);
  difference ≤ f32 tolerance.

### Phase 5 (E19 reranker) — modified FSV

**Skip check 1 (no persistent data) and check 2 (no stored records).**

Keep checks 3 (determinism), 4 (discrimination with logit gap), 5 (edges),
6 (physical evidence with logit values).

### Phase 6 (system-wide integration)

Run all per-phase FSVs again + a NEW system-wide check:

**Check 10: SYSTEM-WIDE CONSISTENCY**

- `score_against_constellation` returns exactly NUM_EMBEDDERS scores
- `mine_contrastive_pairs` produces similarity_profile of length
  NUM_EMBEDDERS
- `get_unified_neighbors` with default weights operates over all 18 views
- Python consumers (dashboard, v4_2 scripts) handle the extended vector
  correctly

## FSV automation

Every phase ships with an integration test file:

```
crates/context-graph-storage/tests/fsv_e14.rs
crates/context-graph-storage/tests/fsv_e15.rs
crates/context-graph-storage/tests/fsv_e16.rs
crates/context-graph-storage/tests/fsv_e17.rs
crates/context-graph-storage/tests/fsv_e18.rs
crates/context-graph-storage/tests/fsv_e19.rs
```

Each runs the 6-9 checks for that embedder.

Plus a cross-phase FSV:

```
crates/context-graph-storage/tests/fsv_system_wide.rs
```

Run after every phase to catch regressions.

## FSV log format

Each check emits a structured log line:

```
FSV[e14][check=disk]: PASS (cf_records=2770, hnsw_count=2770)
FSV[e14][check=schema]: PASS (10/10 samples valid)
FSV[e14][check=determinism]: PASS (5/5 cos >= 0.9999)
FSV[e14][check=discrimination]: PASS (gap=0.087 vs threshold 0.05)
FSV[e14][check=edges]: PASS (4/4 edge cases errored cleanly)
FSV[e14][check=evidence]: PASS (3 samples dumped)
FSV[e14]: ALL 6 PASS
```

Failure line:

```
FSV[e14][check=determinism]: FAIL (1/5 cos < 0.9999)
  memory_id=abc123... stored_first5=[...] recomputed_first5=[...] cos=0.9823
  Root cause: bf16 precision drift between stored and live embedding.
  Action: verify compute dtype in gpu_forward.rs:forward_batch matches load dtype.
```

No check passes silently. Every one emits a line.

## FSV runbook

To manually run full FSV for a phase N:

```bash
# 1. Unit + integration tests
cargo test --workspace -p context-graph-storage --test fsv_e{N} -- --nocapture

# 2. End-to-end through MCP
cargo run --bin context-graph-mcp -- --daemon &
python3 scripts/fsv_e{N}_mcp.py   # Python-side MCP client test

# 3. System-wide
cargo test --workspace --test fsv_system_wide -- --nocapture
```

Log output is preserved to `tmp/fsv_e{N}_$(date).log` for audit.
