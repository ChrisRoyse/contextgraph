# Verification Report

**Source files covered:**
- `Cargo.toml`
- `crates/**/*.rs`
- `scripts/**/*.py`

## 1. Codebase Metrics (counted at refresh)
| Metric | Value |
| --- | --- |
| Workspace packages | 10 |
| Default workspace members | 9 (benchmark excluded) |
| Rust source files under `crates/*/src` | 1138 |
| Rust source LOC under `crates/*/src` | ~414,400 |
| Integration test files (`crates/*/tests`) | 274 (228 with `#[test]` attrs) |
| `#[test]` attributes (all Rust) | 4787 |
| `#[ignore]` attributes | 32 |
| Python scripts under `scripts/` | 91 |
| MCP tools (with `llm`) | 108 |
| MCP tools (without `llm`) | 104 |
| Tool name constants | 108 |
| RocksDB column families | 73 |
| Production embedder slots (E1-E14) | 14 |
| Learner-state slots (E15-E21) | 7 |
| ModelId variants (`all`) | 15 (production = 14 + legacy `Entity`) |
| TrainingRecord version | 3 |
| LearnerRecord version | 1 |
| LearningEvent version | 1 |

## 2. Verification Commands
| Command | Notes |
| --- | --- |
| `cargo build --workspace` | Builds default-members; benchmark crate excluded by default |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Asserts 108 (llm); also `test_submodule_counts` |
| `cargo test -p context-graph-mcp --no-default-features ...` | Asserts 104; may fail if stubs reference llm-gated items |
| `cargo test --workspace` | Full suite (long-running) |

## 3. Notable Constants
| Constant | Value | Source |
| --- | --- | --- |
| TOTAL_COLUMN_FAMILIES | 73 | column_families.rs + teleological/column_families.rs |
| NUM_EMBEDDERS | 14 | semantic/constants.rs |
| TELEOLOGICAL_VECTOR_DIM | 13 | teleological/types.rs |
| RRF_K (core) | 60.0 | retrieval/config.rs, fusion/mod.rs |
| RRF_K (storage) | 60 | rocksdb_store/fusion.rs |
| TOTAL_WEIGHT | 9.5 | retrieval/config.rs |
| MAX_WEIGHTED_AGREEMENT | 9.5 | clustering/manager.rs |
| TRAINING_RECORD_VERSION | 3 | training module |
| LEARNER_RECORD_VERSION | 1 | learner/mod.rs |
| LEARNING_EVENT_VERSION | 1 | learning/mod.rs |
| LEARNER_MODALITY_COUNT | 7 | learner/mod.rs |
| MAX_LEARNER_STATE_VECTOR_VALUES | 16384 | learner/mod.rs |
| MAX_LEARNER_LABEL_CHARS | 256 | learner/mod.rs |
| MAX_LEARNING_EVENT_MEMORY_IDS | 1024 | learning/mod.rs |
| MAX_LEARNING_EVENT_TEXT_CHARS | 65536 | learning/mod.rs |
| MAX_LEARNING_EVENT_LABEL_CHARS | 256 | learning/mod.rs |
| TEMPORAL_EMBEDDERS_ENABLED | true | weights/mod.rs |
| E11_ENTITY_ENABLED | false | weights/mod.rs |

## 4. Lint Configuration
- Workspace `[profile.release]` uses `lto = "thin"`, `codegen-units = 1`, `strip = true`.
- Clippy expected clean per recent audits (see CLAUDE memory).

## 5. Source-Level Inconsistencies (Tracked)
1. `TELEOLOGICAL_VECTOR_DIM = 13` while `NUM_EMBEDDERS = 14` (E14 deliberately excluded from compact vector).
2. Config schema accepts `sse` transport while MCP main rejects it.
3. `LEARNER_MODALITY_COUNT = 7` while `LearnerModality` enum has 14 variants; validation permits `+1` modalities.
4. CAMD `camd_lib.py` is 13-view; E14 is exposed via extended sidecar scripts only.
5. Some inline comments still reference older counts (13/100/96); current authoritative numbers above.

## 6. What Is Not Verified Here
- Coverage percentages (no instrumentation captured at refresh).
- Live runtime CUDA availability (CUDA constants documented but not exercised here).
- External script behavior (Python tests run separately).
