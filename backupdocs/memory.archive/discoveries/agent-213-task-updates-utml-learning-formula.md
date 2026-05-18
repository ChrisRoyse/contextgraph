---
namespace: discoveries
agent_id: agent-213-task-updater
timestamp: 2026-05-10T00:00:00Z
status: active
tags: utml, task-update, state-audit, mejepa-train, supersession, learning-signal
---

# Discovery — UTML learning-formula task updates (per-file audit)

## Context

Applied agent-208-coordinator's AI-agent mandate block to all 10 TASK-UTML-*.md files in `docs/ruvectorfindings/specs/tasks/utml-learning-formula/` AND prepended a per-file STATE AUDIT block flagging current shipping status against the live tree at `crates/context-graph-mejepa-train/src/`.

## Key finding

The host crate `context-graph-mejepa-train` ALREADY EXISTS with the full module tree (`aux/`, `bin/`, `cert/`, `cli/`, `distill/`, `eval/`, `loss/`, `optim/`, `sampler/`, `trainer/` + `learning_signal.rs`, `error.rs`, `lib.rs`, `checkpoint.rs`, `config.rs`, `lora_wiring.rs`). TASK-UTML-001's "create new crate" plan is OUT OF DATE.

The compute-function shipping status against `src/learning_signal.rs`:

| Function / Struct | Line | Status | Drift? |
|---|---|---|---|
| `UtmlErrorCode`, `UtmlError` | `error.rs` L8/L15 | SHIPPED | parity OK |
| `SkipReason` | L57 | SHIPPED | parity OK |
| `DeltaPComponents` | L69 | SHIPPED | verify fields |
| `DeltaKComponents` | L76 | SHIPPED | verify fields |
| `DeltaOmegaComponents` | L83 | SHIPPED | verify fields |
| `DeltaXiComponents` | L90 | SHIPPED | verify fields |
| `LearningSignal` | L97 + `components_as_map` L111 + `validate` L125 | SHIPPED | parity OK |
| `TrainingCertificate` | `cert/mod.rs` L46 + `validate_phase3` L73 | SHIPPED | parity OK |
| `compute_delta_p` | L159 | SHIPPED | mean-of-fields vs spec weighted form |
| `compute_delta_k` | L166 | SHIPPED | mean vs spec `0.5/0.3/0.15/0.05` weighting |
| `compute_delta_omega` | L177 | SHIPPED | mean vs spec geometric `^0.4·^0.4·^0.1·^0.1` |
| `compute_delta_xi` | L184 | SHIPPED | mean vs spec `(1−tc)^0.5·(1−r)^0.3·(1−v)^0.2` + **MISSING `target_collapse == 0.0` HARD ASSERTION** |
| `compute_l_step` | L191 | SHIPPED | parity OK (multiplicative product + clamp01) |
| `sampling_weight` | L218 | SHIPPED | verify `0.995^age_days` form |
| `should_drop` | L226 | SHIPPED | parity OK |
| `should_force_include` | L230 | SHIPPED | parity OK |
| `pairwise_mi_audit` | L234 | SHIPPED (scalar) | spec wants CSV matrix writer too |
| `UtmlClassifier::classify` | — | PENDING | nothing in tree |
| `prune_decision` | — | PENDING | nothing in tree |
| `Consolidation::{update_m_t,detect_dissipating}` | — | PENDING | nothing in tree |
| `StateTransfer::compute_t` | — | PENDING | nothing in tree |
| `ContinualBackprop::reinit_dormant_units` | — | PENDING | nothing in tree |
| `compute_embedder_coherence` / `EmbedderCoherence` | — | PENDING | nothing in tree |
| `update_grad_ema` | — | PENDING | nothing in tree |
| `compute_delta_p_aggregate` / `DeltaPAggregator` | — | PENDING | nothing in tree |
| `UtmlState` / `ClassifierVerdict` / `Intervention` | — | PENDING | nothing in tree |
| `utml_l_step_fsv` binary | — | PENDING | `examples/train_full_fsv.rs` exists and writes `utml-l-step-evidence.json` (L34, L111) |

## Per-file pre/post SHAs + supersession status

| File | Pre-SHA | Post-SHA | Supersession | Audit verified |
|---|---|---|---|---|
| TASK-UTML-001 | `b8b1d41f7228ad15c21b3438f8eb41240e6a0c5a1ab21c1374b8bdacf0d112d6` | `2f05396e5742c0265b0b87e33db96b5a099e4bdcc1bfa30edee6430efc5508d0` | YES (full) | `UtmlError`/`UtmlErrorCode` @ `error.rs` L8/L15; `LearningSignal`/`SkipReason`/`Delta{P,K,Omega,Xi}Components` @ `learning_signal.rs` L57–L107; `TrainingCertificate` @ `cert/mod.rs` L46 |
| TASK-UTML-002 | `cacc0305dbc0a142847999e7489f885c03b32519197fdd06f7bbf21f33b17f32` | `753e0a1eacb69b2b958d67f10c50664ac26c480d3306836acb9b6fb8cd145144` | PARTIAL | `SkipReason` shipped; `UtmlState`/`ClassifierVerdict`/`DeltaPAggregator`/`Intervention` PENDING |
| TASK-UTML-003 | `5d93dc564bf6b47f9a94649c54e2b63401111ee8602093ddac1cd63d888884a6` | `e0023b043e165a9b676df8c1688ac645b10b7f6cf0e0ba6c6bc6d255328be19a` | PARTIAL | `compute_delta_p` shipped @ L159 (drift: mean vs spec weighted); `compute_delta_p_aggregate` PENDING |
| TASK-UTML-004 | `d27738ce51909a5777b7b620d18c5f405806b15ae3067bb7028399e3b3ae8ce3` | `2c30f52add23fc8ba3bb029be91c015080e9afaf1447d135dd704eeb9fe77ee1` | PARTIAL | `compute_delta_k` shipped @ L166 (drift: mean vs spec 0.5/0.3/0.15/0.05); `update_grad_ema` PENDING |
| TASK-UTML-005 | `90d660f34e60a60f7ba5f54c2bf97754b057d9f82348ccd25eafb273b16ba103` | `2bd8e8e96ea14d627367c1e318a826f0d9cdc839fe775bb4e33b13d7867ca2b5` | PARTIAL + SEMANTIC DRIFT | `compute_delta_omega` shipped @ L177 (mean of `runtime_delta`/`warning_delta`/`memory_delta` vs spec geometric product); 4 sub-functions PENDING |
| TASK-UTML-006 | `29ded7dfe91b5207649222e6a8fbe4fd1cc2e2024b9f3162d2c7d1365abe2ca3` | `5cb9d8dd48b1840ee370c542db574dcaa9063b31ab372d3ae66226a78ce6f861` | PARTIAL + CRITICAL DRIFT | `compute_delta_xi` shipped @ L184 (mean, MISSING `target_collapse == 0.0` HARD ASSERTION — safety-critical gradient-leak detection) |
| TASK-UTML-007 | `fc76bab485e2be79008557ec4090432007c08fc3d5ff588e983165ba0be06794` | `8a34ec07d117e267d4079d40024c2cfe08a92f4d3e2b24d21b1c31efa44d35cf` | YES (full) | `compute_l_step` L191 + `sampling_weight` L218 + `should_drop` L226 + `should_force_include` L230 all shipped; consumed by `sampler/mod.rs` L103+L116+L118 |
| TASK-UTML-008 | `d679847d900f01e9994cb1746ced7a601b99b2dbfce80f91e87d5a9d96617c6e` | `1d5a0fcb110e7ae072bc8834d99527e56547ef455a36b71dd6fa1f5e200ee0ef` | PARTIAL | `pairwise_mi_audit` shipped @ L234 (scalar form, consumed by `eval/holdout.rs` L160); CSV matrix writer + `compute_embedder_coherence` + `bootstrap_neutral` PENDING |
| TASK-UTML-009 | `205091166af89122465fd3103607a9a267888f36fecfe4b7fe1c3014971d732d` | `e9cb63b7dec0931347cd8e3c3275023aeaf018c607832d967ba3185af408cb49` | NO (clean PENDING) | `UtmlClassifier`/`prune_decision`/`Consolidation::*`/`StateTransfer::compute_t`/`ContinualBackprop::reinit_dormant_units` — full grep returns zero hits across the crate |
| TASK-UTML-010 | `d50f70ed61a0c08671558ecb4cf77b94523d72c2b8665d61ee570f0bc9d01522` | `59d72eb99c68c7994d5635805a150ad66f1bf5ba42f6ed4ccbaa69bcff29b615` | PARTIAL | `examples/train_full_fsv.rs` exists and emits `utml-l-step-evidence.json` (L34, L111); `utml_l_step_fsv` binary rename + `utml-classifier-evidence.json` + `utml-pairwise-mi-matrix.csv` PENDING (blocked by TASK-UTML-008/-009) |

## Implications

1. The Phase-3 trainer host crate is FURTHER along than the task graph suggests — `aux/`, `bin/`, `cli/`, `distill/`, `eval/holdout.rs`, `lora_wiring.rs`, `checkpoint.rs` all exist and are wired into a working training loop (`train_holdout_eval_fsv.rs`, `train_cert_chain_fsv.rs`, etc.). Future agents should read `lib.rs` to see what other modules exist before writing new code.
2. The drift on TASK-UTML-{004,005,006} is consistent: the shipped `compute_delta_*` functions all degenerate to arithmetic mean over their `Delta*Components` carrier, ignoring the weighted-sum / geometric-product / power-weighted-product semantics in their task titles. Either the specs need to relax or the shipped bodies need to be rewritten. **TASK-UTML-006's missing `target_collapse == 0.0` hard assertion is safety-critical** — without it, gradient leakage from the EMA target encoder can silently corrupt ME-JEPA training.
3. The pending block (TASK-UTML-009 + the missing pieces of -002/-003/-004/-008) covers the active-learning trigger (REQ-UTML-08), continual-backprop dormant-unit re-init (REQ-UTML-07), and the pairwise-MI CSV matrix audit (REQ-UTML-06) listed in Phase 3+ status as "pending".

## Source of truth

`file:/home/cabdru/contextgraph/docs/ruvectorfindings/specs/tasks/utml-learning-formula/TASK-UTML-*.md` (post-SHAs above).
`file:/home/cabdru/contextgraph/crates/context-graph-mejepa-train/src/learning_signal.rs` (313 lines).
`file:/home/cabdru/contextgraph/crates/context-graph-mejepa-train/src/error.rs` (135 lines).
`file:/home/cabdru/contextgraph/crates/context-graph-mejepa-train/src/cert/{mod,chain,writer,invariant_scan}.rs`.
