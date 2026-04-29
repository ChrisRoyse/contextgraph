# Test Suite

**Source files covered:**
- `crates/*/tests/**/*.rs`
- `crates/*/src/**/*.rs`
- `Cargo.toml`

## 1. Metrics
| Metric | Value |
| --- | --- |
| Integration test files (in `tests/`) | 274 (228 contain `#[test]`/`#[tokio::test]`) |
| All Rust files under `crates/` | 1236 |
| `#[test]` attributes in all Rust files | 4787 |
| `#[ignore]` attributes | 32 |

## 2. Test Categories
| Category | Pattern | Notes |
| --- | --- | --- |
| Unit | `#[test]` inside `mod tests {}` blocks | Embedded in source modules |
| Integration | `crates/*/tests/*.rs` | Run by Cargo per-crate |
| FSV (Full State Verification) | `*_fsv.rs`, `*_full_state_verification.rs` | Physical RocksDB readback + state diff |
| E2E | `crates/context-graph-cli/tests/e2e/` | End-to-end session/error/recovery |
| Roundtrip | `*_roundtrip*` / `storage_roundtrip_test/` | Encode→decode parity |
| Real-data | `full_integration_real_data/` | Real embeddings on real corpus |

## 3. Run Commands
| Command | Purpose |
| --- | --- |
| `cargo test --workspace` | Default workspace tests |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Verify 108 tools (llm) |
| `cargo test -p context-graph-mcp --no-default-features tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Verify 104 tools (no-llm) — may fail to compile if `graph-agent/stubs.rs` references llm-gated items |
| `cargo test -p context-graph-storage` | Storage tests (CF init, recovery, FSV) |
| `cargo test -p context-graph-core` | Core (fingerprints, weights, retrieval, learning) |
| `cargo test -p context-graph-embeddings` | Embedder dimension/quantization/serialization tests |
| `cargo test -p context-graph-cli` | CLI integration + E2E |

## 4. Notable FSV Targets
| Target | Coverage |
| --- | --- |
| `e14_fsv.rs` (storage) | E14 BGE-M3 end-to-end |
| `e14_phase_b_fsv.rs` (cli) | E14 ingest → search → consistency |
| `e14_phase_b_full_state_verification.rs` (cli) | Full state diff |
| `full_14_embedder_fsv.rs` (storage) | All 14 embedders end-to-end |
| `learning_events_fsv.rs` | Learning event readback |
| `utl_learner_state_fsv.rs` | Learner state CFs |
| `utl_learner_embedders_fsv.rs` | E15-E21 slot listing |
| `e2e/full_session_test.rs` | Session lifecycle |
| `crash_recovery_tests.rs` | Open after dirty shutdown |

## 5. FSV Python Scripts (`scripts/v4_2/`)
| Script | Coverage |
| --- | --- |
| `bigunlock_fsv_lib.py` | BigUnlock FSV harness |
| `learner_state_bigunlock_fsv.py` | Learner-state end-to-end |
| `shakespeare_corpus_fsv.py` | Shakespeare corpus |
| `shakespeare_state_conditioned_fsv.py` | State-conditioned retrieval |
| `shakespeare_learner_training_fsv.py` | Learner training cycle |
| `shakespeare_learning_event_fsv.py` | Learning event ingest |
| `shakespeare_phase6_policy_benchmark.py` | Phase 6 retrieval policy benchmark |
| `unified_realdata_fsv.py` | Unified real-data |
| `extended_fsv.py` | Extended CAMD sidecar |
| `cuda13_wsl_fsv.py` | CUDA 13 / WSL Blackwell |
| `conceptual_capabilities_fsv.py` | Capability matrix readback |

## 6. Coverage
Coverage percentage is not determined from source.
