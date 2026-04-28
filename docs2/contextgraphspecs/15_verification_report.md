# Verification Report

**Source files covered:**
- `Cargo.toml`
- `crates/**/*.rs`
- `scripts/**/*.py`

## 1. Codebase Metrics
| Metric | Value |
| --- | --- |
| Workspace packages | 10 |
| Default workspace members | 9 |
| Rust source files under crates/*/src | 1137 |
| Rust source LOC under crates/*/src | 409735 |
| Integration test files | 76 |
| Rust test attributes | 5351 |
| Ignored test attributes | 32 |
| Python scripts under scripts | 74 |
| MCP tools with llm | 100 |
| MCP tools without llm | 96 |
| Tool constants | 100 |
| RocksDB column families | 73 |
| Production embedder slots | 14 |
| ModelId variants | 15 |
| TrainingRecord version | 3 |
| LearnerRecord version | 1 |
| LearningEvent version | 1 |

## 2. Verification Commands During Refresh
| Command | Status | Notes |
| --- | --- | --- |
| python doc generator | passed | Generated docs from checked-out source |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates` | passed | 1 test passed in `src/lib.rs`, 1 test passed in `src/main.rs`; integration target had 0 matching tests |
| `cargo test -p context-graph-mcp --no-default-features tools::definitions::tests::test_total_tool_count_and_no_duplicates` | failed before test | Compile error E0432 in `crates/context-graph-graph-agent/src/stubs.rs`: imports `context_graph_causal_agent::llm`, `CausalDiscoveryLLM`, and `crate::service` while those items are gated behind `llm` |
| cargo test --workspace | not run | Full suite not run during doc generation |

## 3. Notable Constants
| Constant | Value |
| --- | --- |
| TOTAL_COLUMN_FAMILIES | 73 |
| NUM_EMBEDDERS | 14 |
| TELEOLOGICAL_VECTOR_DIM | 13 |
| RRF_K | 60 |
| MAX_WEIGHTED_AGREEMENT | 9.5 |
| TRAINING_RECORD_VERSION | 3 |
| LEARNER_RECORD_VERSION | 1 |
| LEARNING_EVENT_VERSION | 1 |
| MAX_RECENT_SCORE_WINDOW | 1024 |
| MAX_LEARNER_STATE_VECTOR_VALUES | 16384 |
