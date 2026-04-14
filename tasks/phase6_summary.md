# Phase 6 — Agent E summary (File-format export CLI, Parquet only)

## Build status

`cargo build --release` (full workspace) with
`CUDARC_CUDA_VERSION=13000 CUDA_HOME=/usr/local/cuda-13.1 PATH=/usr/local/cuda-13.1/bin:$PATH`:

- Rust errors: 0
- Rust warnings: 0 (the two `warning: context-graph-cuda@0.1.0: ...` log lines
  are NVCC build-script info notes, not Rust compiler warnings)
- Clean MCP build: `cargo build --release -p context-graph-mcp` → 0 errors, 0 warnings
- Clean CLI build: `cargo build --release -p context-graph-cli` → 0 errors, 0 warnings

## Tests

### Phase 6 tests (this phase)

| Suite | Result |
| --- | --- |
| `context-graph-cli --test export_training_integration` | **5 / 5 PASS** |

Tests:
1. `roundtrip_training_corpus_parquet` — seed 3 real `TrainingRecord` rows through
   `RocksDbTeleologicalStore`, export to Parquet, read back via
   `parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder`, decode every
   `record_bytes` payload with `decode_training_record`, assert every populated
   field survives (memory_id, content, importance, session_id, content_hash,
   e1/e5/e6 vectors, topic_profile, cross_correlations [len=78], group_alignments,
   outgoing edges, causal_effects, knn_neighbors [len=NUM_EMBEDDERS], tucker_core / temporal_labels both `None`).
2. `empty_cf_produces_empty_parquet_file` — zero-row store produces a valid
   Parquet file (non-empty footer, 0 rows, 0 row groups).
3. `batch_size_respected` — 50 rows with `batch_size=10` produces exactly 5 row
   groups of 10 rows each.
4. `limit_caps_records` — 20 rows stored, `--limit 7` yields exactly 7 rows in
   the file and all 7 decode successfully.
5. `invalid_output_dir_fails_fast` — `--out /nonexistent/bogus/path/...` → `Err`
   with `"parent directory does not exist"`; no file is created.

No mocks. Every test uses a real `TempDir`-backed `RocksDbTeleologicalStore`
and a real Parquet file on disk.

### Regression suites (unchanged behaviour)

| Suite | Result |
| --- | --- |
| `context-graph-core --lib training::` | **18 / 18 PASS** |
| `context-graph-core --lib constellation::` | **19 / 19 PASS** |
| `context-graph-core --lib contrastive::` | **21 / 21 PASS** |
| `context-graph-core --lib teleological::tucker` | **8 / 8 PASS** |
| `context-graph-storage --test training_records_integration` | **15 / 15 PASS** |

Total phase-6 + regressions: 86 tests run, 86 PASS. No regressions anywhere.

## Files created

- `/home/cabdru/contextgraph/crates/context-graph-cli/src/lib.rs` — new library
  surface so integration tests can import `commands::export_training::run`.
  Declares `pub mod commands`, `pub mod error`, `pub mod mcp_client`,
  `pub mod mcp_helpers`. Does not alter the binary behaviour.
- `/home/cabdru/contextgraph/crates/context-graph-cli/src/commands/export_training.rs`
  — the Phase 6 handler (~290 LOC incl. module docs). Exposes
  `ExportTrainingArgs`, `ExportSummary`, `run`, `summary_to_exit_code`.
- `/home/cabdru/contextgraph/crates/context-graph-cli/tests/export_training_integration.rs`
  — the 5 FSV tests described above (~290 LOC).

## Files modified

- `/home/cabdru/contextgraph/crates/context-graph-cli/Cargo.toml` — added
  `parquet = { version = "58", default-features = false, features = ["arrow", "zstd", "snap"] }`,
  `arrow-array = "58"`, `arrow-schema = "58"`. Nothing else touched.
- `/home/cabdru/contextgraph/crates/context-graph-cli/src/main.rs` — swapped
  the inline `mod commands; mod error; pub mod mcp_client; pub mod mcp_helpers;`
  block for `use context_graph_cli::commands;` (the lib version), added the
  `Export { sub }` subcommand and `ExportSubcommand::TrainingCorpus` variant,
  wired the dispatch arm.
- `/home/cabdru/contextgraph/crates/context-graph-cli/src/commands/mod.rs` —
  added `pub mod export_training;`.
- `/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`
  — changed `encode_training_record` / `decode_training_record` from private to
  `pub` (with doc-comments explaining the Phase-6 rationale). No behavioural change.
- `/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs`
  — promoted `mod export;` to `pub mod export;` and re-exported
  `encode_training_record` / `decode_training_record` at the module root.
- `/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/mod.rs`
  — re-exported `encode_training_record` / `decode_training_record` from the
  teleological public surface so the CLI can import them via
  `context_graph_storage::teleological::{encode,decode}_training_record`.
- `/home/cabdru/contextgraph/tasks/todo.md` — checked off the four Phase-6 items.

Phase 1 / 2 / 3 / 4 logic is **untouched**. The only change outside the CLI
is promoting two private helpers to `pub` in the storage crate — the function
bodies are byte-for-byte identical.

## Parquet schema (the on-disk contract)

```text
Parquet file:
  column 0: memory_id    DataType::Utf8     non-nullable   (UUID hyphenated string)
  column 1: record_bytes DataType::Binary   non-nullable   ([version: u8][bincode(TrainingRecord)])
Compression: Zstd (default level)
Row groups: one per CLI --batch-size (forced by writer.flush() between batches)
```

`record_bytes` is produced by
`context_graph_storage::teleological::encode_training_record` — the **exact
same bytes** that live in RocksDB's `CF_TRAINING_RECORDS`. Consumers must
call the matching `decode_training_record` (or replicate the 2-line
`[version_byte][bincode]` contract).

## Key signatures added

```rust
// crates/context-graph-cli/src/commands/export_training.rs

#[derive(clap::Args, Debug)]
pub struct ExportTrainingArgs {
    pub kind: String,           // only "training-corpus"
    pub format: String,         // only "parquet"
    pub out: PathBuf,
    pub storage: PathBuf,       // env: CONTEXT_GRAPH_STORAGE_PATH
    pub batch_size: usize,      // default 500
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ExportSummary {
    pub total_records: usize,
    pub bytes_written: u64,
    pub row_groups: usize,
    pub elapsed_ms: u64,
}

pub async fn run(args: ExportTrainingArgs) -> anyhow::Result<ExportSummary>;
pub fn summary_to_exit_code(result: anyhow::Result<ExportSummary>) -> i32;
```

```rust
// crates/context-graph-storage/src/teleological/mod.rs  (newly pub)

pub fn encode_training_record(record: &TrainingRecord) -> CoreResult<Vec<u8>>;
pub fn decode_training_record(bytes: &[u8]) -> CoreResult<TrainingRecord>;
```

## CLI invocation form chosen

Nested subcommand, matching the existing `session` / `hooks` / `memory`
style:

```bash
# Minimal
context-graph-cli export training-corpus \
    --out ./out/corpus.parquet \
    --storage ~/.context-graph/db

# Full
context-graph-cli export training-corpus \
    --kind training-corpus \
    --format parquet \
    --out /abs/path/corpus.parquet \
    --storage $CONTEXT_GRAPH_STORAGE_PATH \
    --batch-size 500 \
    --limit 10000
```

On success prints a single-line JSON summary on stdout:
```json
{"status":"ok","total_records":3,"bytes_written":5241,"row_groups":1,"elapsed_ms":17}
```
Exit code 0 on success, 1 on any failure (missing parent dir, bad storage,
RocksDB errors, Parquet writer errors).

## Known limitations

- **Parquet only.** HDF5 and safetensors are intentionally deferred per the
  task brief + plan §8 ("Parquet-first"). The `--format` flag is a
  single-valued clap `value_parser` so other formats surface as a clap
  parse error, not a silent no-op.
- **`training-corpus` only.** Constellation and contrastive-pair CLI
  targets are deferred to a later phase. The subcommand structure is
  already shaped so adding them is one new `ExportSubcommand` variant +
  one new module — the `export_training.rs` pattern is directly copyable.
- **No streaming iterator yet.** `run()` first calls
  `list_training_record_ids()` to get the full Vec<Uuid>, then streams
  records in `batch_size` chunks via `multi_get_training_records`. For
  the 10 K-corpus target in plan §3.8 the full Vec is ~160 KB so this
  is immaterial; for 10 M+ corpora we'd want to replace the ID fetch
  with a RocksDB prefix iterator surfaced as a new store method. Not
  in scope for Phase 6.
- **One row group per batch is forced** via `writer.flush()` between
  batches. This is required by FSV-3 (`batch_size_respected`) and by
  downstream consumers that want predictable row-group-level parallelism.
  If a consumer prefers fewer, larger row groups they can increase
  `--batch-size`.
- **Missing / undecodable rows are logged (warn!) and skipped.** The
  `multi_get_training_records` path returns `Option<TrainingRecord>`;
  `None` slots are dropped from the output so the downstream consumer
  never sees an orphan UUID with no payload. Behaviour matches the
  Phase-1 contract (see `export.rs` `multi_get_training_records`).
- **Partial-file cleanup.** On any error in the write loop the
  half-written Parquet file is `remove_file`'d so a retried run does not
  start from a corrupted footer.
- **No CUDA code added.** Phase 6 is CPU-only; the CLI's `cuda` feature
  flag still enables the transitive `context-graph-mcp/cuda` and
  `context-graph-embeddings/cuda` features as before.

## FSV status (Phase 6)

| FSV | Description | Status |
| --- | --- | --- |
| **P6-1** | Parquet roundtrip preserves every `TrainingRecord` field | **PASS** — `roundtrip_training_corpus_parquet` |
| **P6-2** | Empty CF → valid empty Parquet file | **PASS** — `empty_cf_produces_empty_parquet_file` |
| **P6-3** | `batch_size` controls row-group count | **PASS** — `batch_size_respected` |
| **P6-4** | `--limit` caps row count | **PASS** — `limit_caps_records` |
| **P6-5** | Missing parent directory fails fast | **PASS** — `invalid_output_dir_fails_fast` |

## Memory for next agent (use verbatim)

Title: **IMPL-PHASE-6-DONE**

Content:

Phase 6 (File-format export CLI, Parquet only) is complete and green.

New CLI surface: `context-graph-cli export training-corpus --out <path> --storage <path> [--batch-size N] [--limit N]`.

Entry points (library-visible via the new `context-graph-cli` lib target):
- `context_graph_cli::commands::export_training::run(ExportTrainingArgs) -> anyhow::Result<ExportSummary>`
- `context_graph_cli::commands::export_training::summary_to_exit_code(...) -> i32`
- `ExportTrainingArgs { kind, format, out, storage, batch_size, limit }`
- `ExportSummary { total_records, bytes_written, row_groups, elapsed_ms }`

Parquet schema (stable contract):
- Two columns: `memory_id: Utf8` (non-nullable, UUID hyphenated) +
  `record_bytes: Binary` (non-nullable).
- `record_bytes` is `[TRAINING_RECORD_VERSION: u8][bincode(TrainingRecord)]`
  — byte-for-byte identical to what `CF_TRAINING_RECORDS` holds.
- Compression: Zstd (default level).
- One row group per CLI `--batch-size` (forced flush between batches).

Storage-crate additions (minimal, non-breaking):
- `encode_training_record` and `decode_training_record` promoted from
  private to `pub` in
  `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`.
- Re-exported at
  `context_graph_storage::teleological::{encode,decode}_training_record`.
- `rocksdb_store::export` module promoted from private `mod` to `pub mod`.
- No behavioural change, no version-byte bump.

CLI-crate additions:
- New `src/lib.rs` exposing `pub mod commands; pub mod error; pub mod mcp_client; pub mod mcp_helpers;`.
- `main.rs` now imports `use context_graph_cli::commands;` instead of declaring the modules inline.
- New top-level subcommand `Export { sub }` with `ExportSubcommand::TrainingCorpus(ExportTrainingArgs)`.

Deps added (CLI only):
- `parquet = "58"` (default-features off; explicit `arrow,zstd,snap` features)
- `arrow-array = "58"`
- `arrow-schema = "58"`

What Phase 7 (code-simplifier) can rely on:
- Error paths are `anyhow::Error` (no new error enums introduced).
- Partial files are cleaned up on any writer failure.
- `bincode` + `TRAINING_RECORD_VERSION` byte contract is consumed via the
  production `encode_training_record` — no parallel encoder to review.
- Row-group-per-batch semantics are explicitly asserted in `batch_size_respected`.
- Phase 1-5 public APIs are unchanged; only the export module re-exports grew.

Deferred (explicit non-goals of Phase 6):
- HDF5, safetensors — plan §8 says Parquet-first.
- `export constellations`, `export contrastive-pairs` CLI subcommands
  — the `ExportSubcommand` enum is already structured to accept them next.
- Streaming ID iterator — current `list_training_record_ids()` full-Vec
  path is fine for 10 K-corpus target; 10 M+ needs a new store method.

FSV: 5/5 green (roundtrip, empty, batch_size, limit, bad-out-dir).

Build: `cargo build --release` clean across the full workspace (including
`-p context-graph-mcp` and `-p context-graph-cli`). Zero Rust warnings.
