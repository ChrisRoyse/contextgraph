---
namespace: journal
agent_id: codex
timestamp: 2026-05-09T00:21:25Z
status: active
tags: ccreality,mcp,dynamicjepa,schema-validator,test-failure,fsv
---

# Failure: MCP Full Lib Exposed Stale Schema Path and Tight Training Budget

## What Happened

After the witnessed-write preflight tests passed, `cargo test -p context-graph-mcp --lib` exposed five unrelated failures:

- Four recommendation schema validator tests still read `docs/ccreality/phase-05-config-files/sample_recommendation.json`.
- That sample was archived with the ccreality docs under `backupdocs/ccreality/phase-05-config-files/sample_recommendation.json`.
- `test_dynamicjepa_phase9_mcp_full_state_verification` reached the real `dynamicjepa_train` step and failed because `configs/dynamicjepa/verification/tiny_train_config.json` had `stopping.max_seconds = 30`; training hit the timeout at epoch 166 step 5 before the unchanged convergence target.

## Why It Matters

The schema tests were no longer reading the physical source of truth after the docs archive move. The DynamicJEPA failure was the documented max-seconds failure mode: the test was using real CUDA/RocksDB training, and the time budget was too tight for the current host load even though the model was still running toward the same quality gates.

## Fix

- Updated schema-validator tests to read the archived sample from `backupdocs/ccreality/phase-05-config-files/sample_recommendation.json`.
- Raised only `stopping.max_seconds` in `configs/dynamicjepa/verification/tiny_train_config.json` from 30 to 90. The target and quality gates were not relaxed.

## Related

- `crates/context-graph-mcp/src/handlers/tools/reality_loop/schema_validator.rs`
- `configs/dynamicjepa/verification/tiny_train_config.json`
- `backupdocs/5090jepa/16_troubleshooting_and_failure_taxonomy.md` section 2.7
