# Phase 1 Pilot — Atomic Task Breakdown

Every task in Phase 1 (BGE-M3 dense / E14) broken into atomic, testable
units. Order matters — later tasks depend on earlier ones.

## Pre-flight (must be done before Task 1)

- [ ] Back up existing RocksDB: `cp -a contextgraph_data contextgraph_data.pre_e14.bak`
- [ ] Verify `HF_TOKEN` env is valid (can download BAAI/bge-m3)
- [ ] Verify GPU free: ≥ 10 GB VRAM free (for model loading + encoder
      inference)
- [ ] Verify `cargo build --workspace` passes on clean main (baseline)
- [ ] Verify `cargo test --workspace` passes on clean main (baseline)

If any fails, resolve before starting.

## Phase 1 tasks

### Task 1 — Verify XLM-RoBERTa tokenizer support

**Purpose:** fail-fast check that the existing `tokenizers` crate can load
BGE-M3's SentencePiece tokenizer. If not, the entire phase is blocked.

**Action:**
- Write a standalone test binary that downloads `BAAI/bge-m3/tokenizer.json`
  and tokenizes "Shall I compare thee to a summer's day?"
- Expected: produces token ids, no errors

**Acceptance:**
- Tokenization succeeds
- Token count is reasonable (12-15 tokens for that string)

**File:** `crates/context-graph-embeddings/tests/bge_m3_tokenizer_probe.rs`

**Estimate:** 30 min

### Task 2 — Add `TokenizerFamily::XlmRobertaSentencePiece` variant

**Purpose:** enable the new tokenizer family to be referenced from `ModelId`.

**Action:**
- Edit `crates/context-graph-embeddings/src/types/model_id/tokenizer.rs`
- Add variant
- Update all exhaustive matches (compiler will list)

**Acceptance:**
- `cargo check --workspace` passes

**Estimate:** 30 min

### Task 3 — Add `ModelId::BgeM3Dense = 14` variant

**Purpose:** register the new embedder in the canonical enum.

**Action:**
- Edit `crates/context-graph-embeddings/src/types/model_id/core.rs`:
  - Add `BgeM3Dense = 14` to enum
  - Update `all()` array
  - Update `production()` array
  - Add match arms in `dimension()`, `projected_dimension()`,
    `is_custom()`, `is_pretrained()` (inherited), `max_tokens()`,
    `tokenizer_family()`, `as_str()`, `latency_budget_ms()`
- Run `cargo check --workspace` — will emit errors for every incomplete
  match arm elsewhere in codebase. Fix each as instructed.

**Acceptance:**
- `cargo check --workspace` passes with no errors
- `cargo test --workspace -p context-graph-embeddings --lib` passes

**Estimate:** 1-2 hours (many exhaustive matches to extend — this is where
the "938 ModelId references" pain concentrates)

### Task 4 — Update `NUM_EMBEDDERS` constant

**Purpose:** grow fixed-size arrays.

**Action:**
- Edit `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs`:
  - `NUM_EMBEDDERS: usize = 14`
- Run `cargo check --workspace`
- Update tests that assert `== 13`:
  - `crates/context-graph-embeddings/tests/search_test.rs:34`
  - `crates/context-graph-embeddings/tests/benchmark_test.rs`

**Acceptance:**
- All tests that asserted 13 now assert 14 (or are explicitly kept at 13
  with comment explaining why)
- `cargo test --workspace` passes

**Estimate:** 1 hour

### Task 5 — Scaffold `bge_m3_dense/` module

**Purpose:** create empty module structure matching `semantic/`.

**Action:**
- Create directory `crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/`
- Create empty file stubs: `mod.rs`, `types.rs`, `constants.rs`, `model.rs`,
  `loader.rs`, `tokenizer.rs`, `attention.rs`, `embeddings.rs`, `encoder.rs`,
  `ffn.rs`, `layer_norm.rs`, `pooling.rs`, `gpu_forward.rs`, `trait_impl.rs`
- Export from `models/pretrained/mod.rs`
- Each stub has at minimum `pub struct {Name};` so it compiles

**Acceptance:**
- `cargo check --workspace` passes
- Module tree compiles

**Estimate:** 30 min

### Task 6 — Copy `semantic/` as starting point for `bge_m3_dense/`

**Purpose:** maximize reuse; we'll modify incrementally.

**Action:**
- Copy contents of `semantic/attention.rs`, `embeddings.rs`, `encoder.rs`,
  `ffn.rs`, `layer_norm.rs`, `types.rs`, `constants.rs`, `loader.rs`,
  `pooling.rs`, `gpu_forward.rs`, `model.rs`, `trait_impl.rs` into
  `bge_m3_dense/` equivalents
- s/Semantic/BgeM3Dense/ throughout
- s/SemanticModel/BgeM3DenseModel/ throughout

**Acceptance:**
- `cargo check --workspace` passes
- Stubs now contain the full encoder code; only tokenizer + pooling need
  adjustment

**Estimate:** 1 hour

### Task 7 — Replace BERT tokenizer with XLM-RoBERTa SentencePiece

**Purpose:** BGE-M3 tokenizes differently.

**Action:**
- Edit `bge_m3_dense/tokenizer.rs`: load `tokenizer.json` from HF cache;
  use `tokenizers::Tokenizer::from_file()`
- Handle special tokens: `<s>` (bos), `</s>` (eos), `<pad>`
- Max length 8192

**Acceptance:**
- Unit test `tokenizer_roundtrip_unicode` passes
- Unit test `tokenizer_max_length_truncates` passes

**Estimate:** 2 hours

### Task 8 — Adjust embeddings (remove segment embeddings)

**Purpose:** XLM-RoBERTa has no token_type_ids (unlike BERT).

**Action:**
- Edit `bge_m3_dense/embeddings.rs`: remove segment embedding lookup; keep
  token + position only

**Acceptance:**
- `cargo test --package context-graph-embeddings --lib bge_m3_dense` passes
  unit tests

**Estimate:** 1 hour

### Task 9 — Adjust pooling to CLS

**Purpose:** BGE-M3 uses CLS pooling, not mean pooling.

**Action:**
- Edit `bge_m3_dense/pooling.rs`: extract `[:, 0, :]` + L2 normalize

**Acceptance:**
- Unit test `pooling_uses_cls_token` passes

**Estimate:** 30 min

### Task 10 — Update loader to handle XLM-RoBERTa safetensors layout

**Purpose:** tensor names differ from BERT e5-large-v2.

**Action:**
- Edit `bge_m3_dense/loader.rs`: map XLM-RoBERTa weight names to internal
  struct fields:
  - `roberta.embeddings.word_embeddings.weight` → token_embeddings
  - `roberta.embeddings.position_embeddings.weight` → position_embeddings
  - `roberta.encoder.layer.{N}.attention.self.query.weight` → q_proj
  - etc.
- Handle `pooler` exists but not needed (we use CLS from last layer)
- Fail-fast if any expected tensor is missing

**Acceptance:**
- Unit test `loader_loads_safetensors_happy_path` passes
- Test can load `BAAI/bge-m3` weights without error
- `loader_rejects_wrong_shape_weights` unit test passes (synthetic corrupt
  file)

**Estimate:** 3-4 hours (largest task in Phase 1)

### Task 11 — Register in factory + registry

**Action:**
- Edit `crates/context-graph-embeddings/src/models/factory/default_factory.rs`:
  add match arm for `ModelId::BgeM3Dense`
- Edit `crates/context-graph-embeddings/src/models/registry/loader.rs`:
  route load requests

**Acceptance:**
- Integration test `factory_creates_bge_m3_dense` passes
- Manual: `ModelRegistry::get(ModelId::BgeM3Dense)` returns working model

**Estimate:** 1 hour

### Task 12 — End-to-end forward pass smoke test

**Action:**
- Write integration test `bge_m3_dense_forward_smoke` that:
  - Loads the model
  - Embeds "Shall I compare thee to a summer's day?"
  - Asserts output is 1024D, finite, L2 norm ≈ 1.0

**Acceptance:**
- Test passes
- Inference latency ≤ 15ms/text (on warm model, RTX 5090)

**Estimate:** 1 hour

### Task 13 — Python parity test

**Purpose:** Rust output matches HF Python reference.

**Action:**
- Write test that:
  - Embeds 10 Sonnet lines via Rust
  - Runs Python comparison (saves inputs to JSON, Python reads+embeds+saves,
    Rust reads back and compares)
  - Asserts cos ≥ 0.995 per-record

**Acceptance:**
- All 10 pass

**Estimate:** 2 hours

### Task 14 — Add CF + storage integration

**Action:**
- Edit `column_families.rs`: `CF_E14_BGE_M3_DENSE`
- Edit `repository/open.rs`: register CF
- Edit `serialization/`: pack/unpack dense vector

**Acceptance:**
- Unit test: `pack + unpack` roundtrip
- Integration test: `storage_roundtrip_e14` — write vector, read back,
  verify bit-identical

**Estimate:** 1.5 hours

### Task 15 — Add HNSW EmbedderIndex

**Action:**
- Edit `hnsw_config/embedder.rs`: add `E14BgeM3Dense` variant + all match
  arms
- Edit `indexes/registry.rs`: register HNSW at startup
- Config: M=16, ef_construction=200, ef_search=100

**Acceptance:**
- Integration test: `hnsw_e14_insert_and_search` — insert 100 random
  1024D vectors, search for one, recover it in top-10

**Estimate:** 1.5 hours

### Task 16 — Update `EmbedderId` DTO + recognize "E14" strings

**Action:**
- Edit `embedder_dtos.rs`: add variant, `from_str` handles "E14",
  "E14_BGE_M3_DENSE", "bge_m3_dense"
- Update existing test that asserted `from_str("E14") == None`

**Acceptance:**
- MCP test: `search_by_embedder(embedder="E14", query="...")` returns
  results (or empty list if no records yet)

**Estimate:** 30 min

### Task 17 — Update MCP handlers

**Action:**
- `embedder_tools.rs`: dispatch E14 queries to new HNSW index
- `memory_tools.rs::score_against_constellation`: returns
  `per_embedder_cosine` of length 14 after compile

**Acceptance:**
- Integration test through JSON-RPC surface:
  `score_against_constellation` returns 14-element array for a Sonnets
  constellation query

**Estimate:** 1.5 hours

### Task 18 — Migration CLI

**Action:**
- Create `crates/context-graph-cli/src/commands/migrate.rs`
- `migrate reembed --embedder E14` command
- Resumable via `.reembed_state_E14.json`

**Acceptance:**
- Unit test: migration handles empty content gracefully
- Unit test: migration is resumable (kill mid-way, restart, completes)

**Estimate:** 2.5 hours

### Task 19 — Run migration on live DB

**Action:**
- Back up `contextgraph_data`
- Run `migrate reembed --embedder E14`
- Verify: all 2,770+ records have E14 vectors in CF_E14_BGE_M3_DENSE

**Acceptance:**
- CF record count == live memory count
- Elapsed time ≤ 25 min
- No hard errors

**Estimate:** 15-25 min wall-clock (migration) + 5 min setup/verify

### Task 20 — Run 6-check FSV for E14

**Action:**
- Write `crates/context-graph-storage/tests/fsv_e14.rs` with all 6 checks
- Run: `cargo test --test fsv_e14 -- --nocapture`

**Acceptance:**
- All 6 checks PASS

**Estimate:** 2 hours (writing tests)

### Task 21 — Update Python consumers

**Action:**
- Edit `scripts/v4_2/camd_lib.py`: extend `EMB` dict with `E14` index
- Edit `scripts/shakespeare_chat_dashboard.py`: extend
  `STYLE_EMBEDDER_WEIGHTS` if desired (or leave E14 at weight 0 initially)
- Verify: dashboard still starts + serves

**Acceptance:**
- Dashboard starts and accepts a request; returns output
- camd_lib.py smoke test passes

**Estimate:** 1 hour

### Task 22 — code-simplifier review

**Action:**
- Invoke `code-simplifier` subagent on all Phase 1 new code
- Apply or reject each suggestion with rationale

**Acceptance:**
- Review complete, changes committed

**Estimate:** 1 hour

### Task 23 — Documentation

**Action:**
- Update `CLAUDE.md` if it mentions embedder count
- Update `docs/SHAKESPEARE_LORA_TRAINING_SPEC.md`
- Add changelog entry

**Acceptance:**
- Docs reflect 14-embedder state

**Estimate:** 30 min

## Phase 1 total estimate

22 tasks × average ~1-2 hours = **~25-35 hours focused work.**

## Merge gate

Phase 1 is complete when:

- [ ] All 23 tasks above checked off
- [ ] `cargo test --workspace` passes with zero failures
- [ ] 6-check FSV passes
- [ ] Python consumers still work
- [ ] Python parity test: cos ≥ 0.995 on 10 Sonnets
- [ ] Re-embedding complete with no hard errors
- [ ] Backup exists at `contextgraph_data.pre_e14.bak`

On green, ready to start Phase 2 (E15 + E16).

## What to do if stuck

- Tokenizer issues: check `tokenizers` crate version compatibility with
  `tokenizer.json` format version
- Loader issues: use HF's `transformers` Python to dump tensor names
  (`model.state_dict().keys()`), then map to Rust field names
- Dim mismatches: verify config.json values match what code expects
- Latency: profile with `cargo flamegraph`; likely culprit is naive
  attention (optimize to flash-attn if needed)
- FSV determinism failures: check compute dtype consistency (bf16 vs fp32
  must match between load and forward)

## Runbook TODOs for user

When executing each task:
1. Announce the task start
2. Show the code changes + test output
3. Run acceptance criteria
4. Only proceed to next task if green
5. Commit per task or per logical group
