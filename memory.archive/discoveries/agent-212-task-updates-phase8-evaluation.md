---
namespace: discoveries
agent_id: agent-212-task-updater
timestamp: 2026-05-10T00:00:00Z
status: active
tags: phase8-evaluation, task-update, fsv-protocol, mandate-block, drift-audit
---

# Discovery: Phase 8 Evaluation TASK-EVAL-001..017 updated with mandate block + state audit

## What was done

Prepended the agent-208-coordinator mandate block + a per-task `## 📋 2026-05-10 STATE AUDIT` section to all 17 TASK-EVAL-*.md files under `docs/ruvectorfindings/specs/tasks/phase8-evaluation/`. Each audit cites specific working-tree facts verified by grep/ls against HEAD.

## Key drift findings (per-task, summarized)

1. **Phase 8 eval crate location is correct.** `crates/context-graph-mejepa/src/eval/` does NOT yet exist — all 17 tasks accurately direct creation under that subtree. No "crate already shipped" surprise for Phase 8 (unlike UTML/MCP per agent-208 handoff).

2. **Type-namespace clash with mejepa-train::eval.** `crates/context-graph-mejepa-train/src/eval/{mod.rs,holdout.rs,slicing.rs,regression.rs,epoch_witness.rs}` already exists and defines its OWN local `MutationCategory` (line 14) and `Lang` (line 40 — 5-variant, NOT the 11-variant `Language`). Phase 8 eval (in `mejepa` crate) MUST use `crate::types::Language` (11 variants) and SHOULD NOT redeclare `Lang`. Flagged on every relevant task (001, 005, 006, 007).

3. **EvalReport precedent**: `mejepa-train::eval::HoldoutReport` + `EpochReport` are train-time. Phase 8 `EvalReport` (mejepa crate) is operator-facing — computes Pearson correlation per-cell (mejepa eval), not raw per-category accuracy (train eval). Don't confuse the two.

4. **Cross-task PatchSimilarityGraph already partially shipped.** `crates/context-graph-mejepa-train/src/sampler/cross_task.rs::build_patch_similarity_graph` (line 7) exists as a Phase 3 sampler helper. TASK-EVAL-010 should consider promoting it to a shared crate rather than duplicating logic (flagged in EVAL-010 audit).

5. **Regression detection precedent**: `mejepa-train::eval::regression::{PromotionDecision, detect_regression, promote_best, track_consecutive_regressions}` is train-internal. Phase 8 `check_ship_gate` (TASK-EVAL-008) week-over-week regression detection reads CF_MEJEPA_EVAL_REPORTS rolling history — distinct surface. Flagged on EVAL-008 + EVAL-012.

6. **TASK-INFER-001 dependency satisfied.** `crates/context-graph-mejepa/src/types.rs` ships `Language` (11), `TaskId` (line 52, 502 impl), `EmbedderId`, `ChunkId`, `OracleOutcome`. `MeJepaCompiler::compile` shipped in `compiler.rs` (line 628 returns `predicted_class: TctMutationCategory`).

7. **MutationCategory naming**: `mejepa::compiler` re-exports `MutationCategory as TctMutationCategory` (compiler.rs line 11). TASK-EVAL-001 must make the new `eval::types::MutationCategory` canonical without colliding.

8. **MCP handler structure**: per agent-208 handoff, existing MCP tools live in flat `mejepa_tools.rs`. TASK-EVAL-013/014 propose separate `mejepa_eval.rs`/`mejepa_active_learning.rs` handler files — flagged for reconciliation choice (split vs extend).

9. **Phase-0 helpers satisfied**: `assert_no_test_patch_leakage` (in `mejepa-corpus::split`) and `swebench::run_swebench_lite_oracle` are SHIPPED. EVAL-004 + EVAL-009 (DockerRerun LabelMethod) consume them directly.

10. **FSV-harness precedent**: `crates/context-graph-mejepa-train/examples/train_holdout_eval_fsv.rs` is SHIPPED and is the reference structure for TASK-EVAL-017 (16 evidence files at `/tmp/contextgraph-mejepa-evaluation-fsv/`).

11. **CLI cli.rs already a flat file** (not directory). TASK-EVAL-016 says `src/cli/eval.rs` — needs reconciliation (extend `cli.rs` or refactor to `cli/`).

## Post-edit SHAs

| File | After-SHA |
|------|-----------|
| TASK-EVAL-001 | 0237a67ce2f2c47b809f67f80f866bdf3939d0b16cd72bd12f195492b8ac28e1 |
| TASK-EVAL-002 | fa667332d13ad5ca505ded46cbdb3066264ddb1f42fa22845855e28940dbbde9 |
| TASK-EVAL-003 | c1090b584beb1796bb90ef10983247169769a6345274c1697e9814f9c49e32a0 |
| TASK-EVAL-004 | c557e254a86a409ba1d9be3b1bdfab928e179d4f7e377bcb2660907a737793b5 |
| TASK-EVAL-005 | 82a4fb44b5ed22f8c1501ca2a5241ccebe45a415bf6a42745058b8ba90ce6c81 |
| TASK-EVAL-006 | 33ca04fd43050bc49307995c2bc49de62738db01a262b634f1be1b9a02a76738 |
| TASK-EVAL-007 | 60cce3bb9dae03669bc09cac862ffbb7905c0144bd204b1811875da6bfe6a6dd |
| TASK-EVAL-008 | e88e10fafadb390649fe5711043fe3aa499dc2539c0e6402f36287ee6ffc7479 |
| TASK-EVAL-009 | 210387a6943488a4c0ddbb110309a0a679280cf1d978a0d5f69ec8fc680f23df |
| TASK-EVAL-010 | 83f9165e81ad85d5a5e9681bb3817a58b320bfba4f5501c9e27e7779fb9cf07b |
| TASK-EVAL-011 | 312b979e5794248f8c80afb23bcbba4227390e238729bb2a7277f06d43ffb8f1 |
| TASK-EVAL-012 | bfee2f0b3aba0ffa6e70440908870ae65018d716efbe08e9b97ad25d0104343e |
| TASK-EVAL-013 | 611108b4162e4f6f676ec57a89b36ee14ef024c12286bd742ae096ceace32b0a |
| TASK-EVAL-014 | 4d469625fd5b55033860f96140de7dd07dd381c3f490d73edae9b354ac43afa0 |
| TASK-EVAL-015 | 43a27a39342afedff294558098f212baa7344b3b1cce1b25becb54b2204f6554 |
| TASK-EVAL-016 | 807ed115d1628cc456540702cdfb826c3538dfaffc7270692ce8c223383273dd |
| TASK-EVAL-017 | d522c54347e56f3343e74286f7d36d6a3ae5aaa984aa60e61c49293fa03cc464 |
