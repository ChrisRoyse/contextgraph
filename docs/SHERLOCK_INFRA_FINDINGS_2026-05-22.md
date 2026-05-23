# Sherlock Holmes — Infrastructure / MCP / Trainer / Witness / TCT forensic findings (2026-05-22)

Scope: infra, MCP product surface, observability, hook scripts, trainer-side audit honesty. Slice complementary to the 5-Sherlock parallel audit (#629 closeout: #594-#609). All findings here are *new* unless explicitly cross-referenced as already filed (#621 / #622 / #627 / #656 / #608 / #625).

Methodology: Holmesian Protocol — Cold Read → Contradiction Engine → Source-of-Truth readback. Code is **guilty until proven innocent**. I read source bytes before reading the issue ledger that claims a surface works.

## Summary

- **6 new silent-success / hardcoded-telemetry / dead-code findings filed as GitHub issues** (#671-#676). Severity skews high because each is gate-relevant or operator-trust-relevant.
- The most damaging defect: **`crates/context-graph-mejepa-train/src/aux/mod.rs:28-32`** — `AuxHeadEnsemble::forward_logits` returns a **zero tensor regardless of input**. Anyone wiring a real distillation cycle through `distill::AuxHeadDistiller::run_cycle` (still publicly exported) would compute KL of zero-zero and report `mean_loss = 0.0` as "training". This is the same class as #596 / #597 but the *building block* survived the producer cleanup.
- The next most damaging: **`crates/context-graph-mejepa-train/src/lora_wiring.rs:58-60`** — `pub fn detach_loras(_state: LoraState) -> Result<(), TrainerError> { Ok(()) }` accepts state, discards it, returns `Ok(())`. Anyone trusting "loras detached → safe to checkpoint" sees a green stamp on a no-op. Combined with `attach_loras` having zero callers and the rust-analyzer evidence — **the entire LoRA wiring trait surface is dead but reachable as public API**. #622 already covers `lora_wiring + distill + aux` as a tech-debt cluster; this report quantifies the specific silent-no-op risk surface.
- **`crates/context-graph-mejepa-train/src/trainer/mod.rs:362-375`** — `EpochSummary` hardcodes `best_category: KnownGood`, `worst_category: CompileError`, `best_language: Python`, `worst_language: Rust` without computing them from data. A downstream consumer reading the EpochReport from `CF_MEJEPA_EPOCH_WITNESS` would believe these were measured. The `training_semantics: DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE` field correctly tells the truth elsewhere; these four fields contradict it.
- **`crates/context-graph-mejepa/src/heal/drill.rs:282-296`** — `DrillStdoutSummary { retrain_triggered: true, ... }` is a literal. The drill never actually triggers a retrain; it calls `trigger_abc_for_current_drift` and reports `retrain_triggered: true` regardless of A/B/C outcome (the actual mode-winner check is *separate* and feeds `exit_code` only). Anyone reading `summary.retrain_triggered` as ground truth is misled.
- **`scripts/cc/hook_*.sh`** universally exit `0`, including on `CCREALITY_HARNESS_TRANSITION_RECORD_FAILED` (`hook_post_tool_use_edit.sh:131`) and `CCREALITY_PREEDIT_STATE_MISSING` (line 47). Per Claude Code's 2026 hook spec ([thepromptshelf.dev](https://thepromptshelf.dev/blog/claude-code-hooks-complete-reference-2026/), [thinkingthroughcode.medium.com](https://thinkingthroughcode.medium.com/the-silent-failure-mode-in-claude-code-hook-every-dev-should-know-about-0466f139c19f)), PostToolUse exit 0 is "success/proceed" and exit 1 is "non-blocking error visible to Claude". Using exit 0 for every error path means **Claude never learns the shift-log capture failed**, and the durable shift log silently misses real Edit events whenever the cli binary is missing or the runtime root is unset. Downstream Q5 `mejepa_reality_impact` then has no ground truth to match against, but reports `not_yet_observed` not `evidence_missing`.
- **Phase H absence-checks advertised in CLAUDE.md §14 and `01_overview.md` do not exist on disk.** Both docs state "9 `scripts/ci/phase_h_*.sh` absence-checks" enforce inner-actor LLM retirement. The `feature = "llm"` `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13-14` is real. The 9 scripts are not. Operator-trusted guard surface is fictional.
- **`docs2/AICodingAgentSuperPrompt.md`-mandated FSV evidence claim about `cert_fsv.rs` orphaned target binaries is real** — `target/debug/incremental/cert_fsv-*` and `target/debug/examples/cert_fsv-*` exist as stale compiled artifacts from a source file already removed (no `cert_fsv*.rs` anywhere under `crates/`). These are inert (no Cargo target wires them) but the operator-facing `target/` dir gives a false impression that `cert_fsv` is still a runtime FSV. Cosmetic but worth a one-line `cargo clean` note in operator runbook.

## Silent-success bugs found

### F-1 (P0) — `AuxHeadEnsemble::forward_logits` returns zeros regardless of input

- **File:line:** `crates/context-graph-mejepa-train/src/aux/mod.rs:28-32`
- **What the code claims:** "Forward logits for the auxiliary head ensemble" — public method on a public struct that the trainer constructs at line 131 of `trainer/mod.rs`.
- **What it actually does:**
  ```rust
  pub fn forward_logits(&self, hidden: &Tensor) -> Result<Tensor, TrainerError> {
      let batch = *hidden.dims().first().unwrap_or(&1);
      Tensor::zeros((batch, self.head_names.len()), DType::F32, &Device::Cpu)
          .map_err(TrainerError::from)
  }
  ```
  The `hidden` input is read only for batch-shape, then a zero tensor is returned. There is no model — only shape-passthrough.
- **Downstream impact:** `distill::AuxHeadDistiller::run_cycle` (still publicly exported in `crate::distill::*`) calls `aux_heads_frozen_snapshot.forward_logits(&hidden)` and `aux_heads_trainable.forward_logits(&hidden)`. Both return zeros. `kl_from_probs_and_logits(zeros, zeros) ≈ 0.0` (after softmax both become uniform 1/N; KL of uniform-to-uniform is 0). `DistillationResult { mean_loss: 0.0, ... excessive_skips: false }`. Any future caller wiring a distillation cycle would log "distillation cycle complete, mean_loss=0.0" — interpretable as "the student matched the teacher perfectly" when the truth is "neither did anything".
- **Severity:** P1. The producer track is currently dead (no live callers), so the false-green is latent. The risk is that a future agent (per **#406 mistake loop** path) wires aux distillation through this surface trusting it. Per CLAUDE.md §6.13, mistake-loop is the only online weight update path. If `aux/mod.rs` gets wired in, mistake loop silently no-ops. **Recommendation:** make `forward_logits` an explicit `Err(MejepaTrainNotImplemented)` until a real ensemble exists, or delete entirely with #622.

### F-2 (P0) — `detach_loras(_state)` is a hard-coded no-op disguised as cleanup

- **File:line:** `crates/context-graph-mejepa-train/src/lora_wiring.rs:58-60`
- **What the code claims:** `pub fn detach_loras(_state: LoraState) -> Result<(), TrainerError>` — operates on `LoraState` and returns success.
- **What it actually does:**
  ```rust
  pub fn detach_loras(_state: LoraState) -> Result<(), TrainerError> {
      Ok(())
  }
  ```
  Discards state, returns Ok. No tensor unhook, no parameter sync, no Fisher diff write — nothing.
- **Downstream impact:** Anyone calling `attach_loras(...)?; checkpoint(...); detach_loras(state)?;` sees the green stamp on the last call and trusts cleanup. In reality, **LoRA layers are leaked**. If LoRA dropout tensors are tracked in the autograd graph (per the existing MEMORY.md note "LoRA dropout increases VRAM"), this is a silent memory leak. Zero current callers (companion to F-1 / #622) so latent for now.
- **Severity:** P1. Same class as F-1.
- **Recommendation:** make this `Err(...)` or delete the whole module.

### F-3 (P1) — `EpochSummary` hardcoded best/worst category and language

- **File:line:** `crates/context-graph-mejepa-train/src/trainer/mod.rs:367-370`
- **What the code claims:** `EpochSummary { best_category, worst_category, best_language, worst_language, ... }` — fields that look like they were computed from training data.
- **What it actually does:**
  ```rust
  best_category: MutationCategory::KnownGood,
  worst_category: MutationCategory::CompileError,
  best_language: Lang::Python,
  worst_language: Lang::Rust,
  ```
  These are literal constants in the constructor. There is no per-category or per-language scan in `train_one_epoch_inner`. The values are written into the EpochSummary, the EpochReport, and the witness chain via `epoch_witness.append(summary.clone())`.
- **Downstream impact:** `CF_MEJEPA_EPOCH_WITNESS` rows produced by the diagnostic-only trainer carry these constants. The trainer correctly self-labels via `training_semantics: DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE` and `trained_predictor: false` in the `TrainingResult`, but **`EpochSummary` itself carries no such warning**. A downstream consumer iterating `CF_MEJEPA_EPOCH_WITNESS` and computing "worst language across all epochs" would treat `Rust` as a real signal and conclude the trainer is bad at Rust — when in fact the trainer measured no language.
- **Severity:** P1.
- **Recommendation:** wrap these fields in `Option<>` and emit `None` for diagnostic-only mode; OR add a non-Optional `epoch_semantics: String` mirroring `training_semantics`.

### F-4 (P1) — `mejepa-heal-drill` reports `retrain_triggered: true` regardless of actual retrain

- **File:line:** `crates/context-graph-mejepa/src/heal/drill.rs:282-296`
- **What the code claims:** `DrillStdoutSummary { retrain_triggered: true, ... }` — a field whose name implies a binary truth claim about retraining.
- **What it actually does:** the constructor hardcodes `retrain_triggered: true`. The drill does call `pipeline.trigger_abc_for_current_drift(...)` and gets back a `report`, but it never verifies that any predictor weights changed. The variable `report.weights_sha_winner` is only used to append a `ModelRollback` witness entry; nothing reads "did training actually run."
- **Downstream impact:** the bin (`mejepa-heal-drill.rs`) emits the summary as JSON to stdout. Any operator/script reading `summary.retrain_triggered` as "the heal pipeline actually retrained" is misled. The drill is documented as "Phase 5 DoD verification harness" (`drill.rs:8` doc); operator trust in DoD verification is precisely the surface that matters.
- **Severity:** P1. Adjacent to #656 (the conformal-overcoverage early-exit) — both undermine drill credibility.
- **Recommendation:** derive `retrain_triggered` from `report.evaluation_summary_sha != [0u8; 32]` AND `report.weights_sha_winner != [0u8; 32]` AND ABC promoter wrote real winner state. Default to `false` and require positive evidence to flip.

### F-5 (P0) — Claude Code hooks always exit 0, including on capture failure

- **File:line:** `scripts/cc/hook_post_tool_use_edit.sh:131` (and 23 other `exit 0` paths across `hook_*.sh`)
- **What the code claims:** "hook records harness transition; emits additional context to PostToolUse". Per Claude Code's 2026 hook semantics (cited above), exit code 0 = success/proceed, exit code 1 = non-blocking warning visible to Claude, exit code 2 = blocking.
- **What it actually does:**
  - `CCREALITY_PREEDIT_STATE_MISSING` → emit context to stdout → `exit 0`
  - `CCREALITY_CONTEXT_GRAPH_CLI_MISSING` (cli binary not built) → emit context → `exit 0`
  - `CCREALITY_HARNESS_TRANSITION_RECORD_FAILED` (record-harness-transition returned non-zero) → emit context → `exit 0`
  - `MEJEPA_VERIFICATION_BIND_FAILED` (post_tool_use_test_capture binding to predictions failed) → emit warning to stderr → `exit 0`
  All errors are reported as text only, never as exit codes. Claude Code only blocks on exit 2; it surfaces exit 1 as a non-blocking warning in the conversation. Exit 0 means "the hook ran fine."
- **Downstream impact:** when the binary is missing, the runtime root is unset, or the cli sub-command fails — no shift-log row is appended. `mejepa_reality_impact` Q5 thus has nothing to match predictions against, but its output **labels the missing data as `not_yet_observed`** (a normal state) rather than `evidence_missing` (an error state). The product-surface health metric (`Q5 surfaces=`) hides hook failures as "data not in yet."
- **Severity:** P0. This silently degrades Q5 quality and the operator never sees it.
- **Recommendation:** for recoverable errors (cli missing, runtime root missing) → `exit 1` so Claude surfaces the warning in conversation. Reserve `exit 0` for actual success. Add a `Q5_SHIFT_CAPTURE_FAILURE_RATE` operational metric so dashboards expose the silent-failure rate.

### F-6 (P2) — Phase H absence-check scripts referenced as guards do not exist

- **File:line:** `CLAUDE.md §2` ("9 `scripts/ci/phase_h_*.sh` absence-checks") and `docs/futurebuild/README.md` Phase H reference.
- **What the docs claim:** Inner-actor LLM retirement is enforced by "9 `scripts/ci/phase_h_*.sh` absence-checks" plus the compile-error guard.
- **What exists on disk:** `find . -path ./target -prune -o -name 'phase_h*' -print` returns nothing. `ls scripts/ci/` shows `check_no_c_drive_paths.sh hook_script_lint.sh phase_f_deprecation_call_sites.md pinned-action-shas.md runbook_coverage.sh`. **Zero phase_h scripts.**
- **What still works:** `crates/context-graph-mcp/src/lib.rs:13-14` `#[cfg(feature = "llm")] compile_error!` is real; building the workspace with `--features llm` would fail. That's the only real guard.
- **Downstream impact:** operator/agent reading CLAUDE.md trusts that 9 absence-checks enforce the Qwen/Hermes/Ollama/llama.cpp deletion. They do not exist. The compile-error catches the most damaging case (re-enabling the llm feature), but absence of e.g. dropped safetensors, retired CLI flags, or smuggled-in HTTP code paths is unverified.
- **Severity:** P2 (documentation drift, not silent-success runtime). But for the doctrine-trust-chain it matters.
- **Recommendation:** either (a) write the 9 absence-check scripts, or (b) update CLAUDE.md / `01_overview.md` to remove the "9 scripts" claim and replace with "1 compile-error guard at lib.rs:13". The current doc is fictional in a load-bearing place.

## Dead code masquerading as live

### D-1 — `crate::distill::*` + `crate::lora_wiring::*` + `crate::aux::*` (mejepa-train)

(Already partially covered by #622; I quantify the public-API attack surface.)

- **`AuxHeadDistiller::run_cycle`** (`distill/mod.rs:52-91`) — zero non-test callers anywhere in workspace.
- **`AuxHeadDistiller::new`** (`distill/mod.rs:38-46`) — zero non-test callers.
- **`AuxHeadDistiller::should_run`** (line 48-50) — zero non-test callers.
- **`PredictorTrunk` trait** (line 23-26) — zero implementors anywhere.
- **`attach_loras`** (`lora_wiring.rs:23-56`) — zero callers.
- **`detach_loras`** (`lora_wiring.rs:58-60`) — zero callers + silently no-op (F-2).
- **`LoraAttachable` trait** — zero implementors.
- **`AuxHeadEnsemble::forward_logits`** (`aux/mod.rs:28-32`) — only used by F-1's stub distiller.
- **`AuxHeadEnsemble`** owned at `Trainer::aux_heads` (`trainer/mod.rs:38, 131`) — constructed but never invoked anywhere.

All of these are `pub` and re-exported from `mejepa-train::lib.rs` (lines 13, 21, 29) — accessible as `context_graph_mejepa_train::aux::*`, `::distill::*`, `::lora_wiring::*` to every other crate in the workspace.

### D-2 — `crate::eval::fixtures::{build_eval_compiler, synthetic_holdout, synthetic_patch_embeddings}` (mejepa)

- **Re-export:** `crates/context-graph-mejepa/src/eval/mod.rs:47` — `pub use fixtures::{build_eval_compiler, synthetic_holdout, synthetic_patch_embeddings};`
- **Status:** despite #594 closing "MCP eval-run still persists fixture EvalReports", the fixture entry points are still publicly exported. The MCP path now fails closed with `MEJEPA_EVAL_FIXTURE_PATH_DISABLED`, but the *building blocks* remain as `pub` API.
- **Risk:** identical to #625 (`DeterministicPredictor` re-export). A future caller can construct a fixture compiler and produce a deterministic `EvalReport` row that bypasses the MCP fail-closed gate.
- **Recommendation:** mark fixtures with `pub(crate)` visibility and move callers (regression tests, runner internal tests) under the same crate. The current crate-boundary leak is structurally identical to #625.

### D-3 — `target/debug/{incremental,examples}/cert_fsv-*` orphaned binaries

- **Status:** 6+ `cert_fsv-*` artifacts under `target/debug/` despite zero `cert_fsv*.rs` source files anywhere under `crates/`.
- **Risk:** rust-analyzer surfaces "not included in any crates" warnings; operator could mistakenly believe a `cert_fsv` FSV harness is still wired.
- **Severity:** cosmetic. Solved by `cargo clean`.

## Hardcoded telemetry

### T-1 — `gtau_pass_rate: 1.0` literal in HoldoutReport

(Already #621.)

- **File:line:** `crates/context-graph-mejepa-train/src/eval/holdout.rs:248`
- **Consumer:** the trainer writes `HoldoutReport` rows to `CF_MEJEPA_HOLDOUT_REPORTS`. **No MCP tool currently reads this CF in production**, but the field is part of the schema. The risk is that any future operator surface or weekly-rollup script that reads `CF_MEJEPA_HOLDOUT_REPORTS` and aggregates `gtau_pass_rate` will see a hardcoded 1.0 telling it "the constellation guard never rejected anything during holdout." That's the same Goodhart-victim metric #621 documents.

### T-2 — `EpochSummary` constants (F-3 above, restated as telemetry concern)

- **File:line:** `crates/context-graph-mejepa-train/src/trainer/mod.rs:367-370`
- **Consumer:** `CF_MEJEPA_EPOCH_WITNESS` rows; potentially any future trainer dashboard.
- **Reason it's telemetry, not just a code smell:** the fields are typed as `MutationCategory` and `Lang`, which look authoritative. A future consumer cannot distinguish "computed from data" from "hardcoded constant" without reading source.

### T-3 — `retrain_triggered: true` (F-4 above)

- **File:line:** `crates/context-graph-mejepa/src/heal/drill.rs:285`
- **Consumer:** stdout JSON from `mejepa-heal-drill` bin; operator readback.

### T-4 — `"passes": True` and `"all_passed": True` literals in 117 sites under `scripts/swebench_lite_ops/`

- **Pattern:** producer writes `"passes": True` as a literal, then `require(final_readback.get("passes") is True, ...)` reads back the *same literal* and asserts equality. The read-back tests serialization integrity, **not** the underlying claim. Most cases are defensible because `require()` calls upstream guarantee the data conditions were met before the literal was written. The pattern is misleading-by-design because nobody reading the artifact can distinguish "passes because real conditions held" from "passes because somebody wrote True".
- **Most egregious example:** `scripts/swebench_lite_ops/code_constellation_schema.py:689` — `require(final_readback.get("passes") is True, "PY_CONSTELLATION_READBACK_READBACK_FAILED", ...)`. The check is tautological: it verifies that the literal `True` survived a JSON roundtrip.
- **Better example of correct use:** `scripts/swebench_lite_ops/python_failure_fingerprint_materializer.py:586-587` — `readback["passes"] = bool(readback["passes"] and all(case["pass"] for case in readback["boundary_cases"]))`. Here `passes` is *derived* before being checked.
- **Recommendation:** convert the producer literal to a derived computation everywhere, OR rename the field to `claim` or `claim_after_require_passed` to make the semantic clearer. The current naming invites mis-trust.

### T-5 — `holdout_agreement: self.best_holdout_agreement` in `EpochSummary` always 0.0 from the diagnostic trainer

- **File:line:** `crates/context-graph-mejepa-train/src/trainer/mod.rs:366`
- The trainer constructs `best_holdout_agreement: 0.0` (line 145) and **never updates it** in `train_one_epoch_inner`. The `EpochSummary` field emits 0.0 every epoch. The trainer's `TrainingResult` correctly marks itself as `trained_predictor: false`, but the EpochSummary on the witness chain doesn't carry that warning.
- **Severity:** P2 — same class as T-2.

## Heal-drill / shift-subscriber / witness chain blast radius

Triaging #656 + #627 + adjacent.

### #627 blast radius (shift-subscriber)

- **What's broken:** `cargo test -p context-graph-mejepa-shift-subscriber` fails on `run_until_idle_consumes_jsonl_and_persists_readback_state` with `MissingCentroid { detail: "no persisted TCT constellations" }`.
- **Why it matters in production:** the shift-subscriber's `process_shift` → compiler.compile → Gτ guard requires a TCT centroid for each `(MutationCategory, Language, EntityType, EmbedderId)` cell. If centroids are not pre-seeded in a fresh deployment, the subscriber **fails on the first shift** with `MEJEPA_TCT_MISSING_CENTROID`. This is correctly fail-closed (no silent success). But it means **a fresh ME-JEPA deployment cannot serve Q5 until centroids are seeded**, and the seeding bootstrap is not documented in `06_operations_and_fsv.md` operator runbook.
- **Downstream:** if the subscriber dies on first shift, `mejepa_subscriber_status` MCP tool reports `lag_alert_active: true` (after lag_alert_sustain_seconds), but **does NOT explicitly surface "fail-closed on missing centroid"** — the operator sees `error_code: MEJEPA_SHIFT_SUBSCRIBER_PROCESS_PANIC` or similar but has to dig through logs to find the root cause.
- **Recommendation:** add an explicit `mejepa_subscriber_status.bootstrap_required: bool` field that becomes `true` when constellation CFs are empty, AND document seeding in `06_operations_and_fsv.md §runbook/freshdeploy`.

### #656 blast radius (heal-drill)

- **What's broken:** `mejepa-heal-drill` fails before complete readback on conformal overcoverage.
- **Heal-drill is the only Phase 5 DoD verification harness.** If it can't complete, the heal pipeline's promotion-latency/integrity-violation/Fisher-rank evidence chain breaks. None of these evidences feed the predictor's serving path directly, so the **predictor serving path doesn't silently lose protection because of #656 alone** — the EWC++ Fisher protection is in `heal/ewc.rs` and is exercised by the live heal pipeline, not by drill. The drill is a verification harness for the DoD claim.
- However, combined with F-4 (`retrain_triggered: true` hardcoded), a partially-completed drill could report success for the parts it did run, including a hardcoded `retrain_triggered: true`, and the operator might believe heal-pipeline DoD is intact.
- **Recommendation:** when #656 completes the drill end-to-end (or stops early on overcoverage), the summary should include an explicit `phase_5_dod_completed: bool` derived from `(drift_detected != WarmupNotReady) AND (mode_winner in {B,C}) AND (lora_refresh_succeeded) AND (integrity_violation_detected) AND (fisher_full_rank) AND (all_readback_files_mode_0o600)`. Each of those is already in `DrillStdoutSummary` — surfacing the AND-fold as a single field would close the gap.

### Witness chain (#608 closed) — re-verified

- `optimizer_witness_chain_verify` now properly fails with `CCREALITY_WITNESS_CHAIN_ABSENT` when `witness-chain.bin` does not exist (`witness_chain.rs:282-285, 309-318`). #608 closure verified.
- `valid: true` literal at line 131 is fine because it's only constructed *after* `verify_chain()` returns `Ok`, and `verify_chain()` fails on missing file or tamper.
- `verify_audit_witness_command` (CLI, `dynamicjepa.rs:1970`) — also fine. Same pattern as MCP. Verification result bubbles via `?` before the `valid: true` literal is constructed.

## Critical infra jumps

Three to five fixes that would materially improve gate-evidence honesty.

### J-1 — Make `forward_logits` and `detach_loras` fail-closed or delete them

Wrap F-1 and F-2 into a single PR with #622. The minimum honest contract is: any caller of a stub function gets an error, not a green-stamp no-op.

```rust
// aux/mod.rs
pub fn forward_logits(&self, _hidden: &Tensor) -> Result<Tensor, TrainerError> {
    Err(TrainerError::new(
        TrainerErrorCode::MejepaTrainNotImplemented,
        "AuxHeadEnsemble::forward_logits is a stub; aux distillation is not wired",
    ))
}
// lora_wiring.rs
pub fn detach_loras(_state: LoraState) -> Result<(), TrainerError> {
    Err(TrainerError::new(
        TrainerErrorCode::MejepaTrainNotImplemented,
        "lora_wiring::detach_loras is a stub; no real detach path exists",
    ))
}
```

### J-2 — Hook scripts: use exit 1 for recoverable errors

Replace `exit 0` after every `CCREALITY_*_FAILED` / `MEJEPA_*_FAILED` code path with `exit 1`. Add a per-hook `*_FAILURE_RATE` operational metric so dashboards expose the silent-failure rate. Per Claude Code 2026 hook spec, this surfaces a non-blocking warning to Claude without breaking the tool call.

### J-3 — `Trainer::EpochSummary` honesty pass

Convert `best_category`/`worst_category`/`best_language`/`worst_language` to `Option<MutationCategory>` / `Option<Lang>` and emit `None` from the diagnostic-only path. Add an explicit `epoch_semantics: String` field mirroring `training_semantics`. (5-line PR.)

### J-4 — Retire the fixture compiler from public API

Move `build_eval_compiler`, `synthetic_holdout`, `synthetic_patch_embeddings` to `pub(crate)` visibility. Same pattern as #625 (DeterministicPredictor). Future agents can't accidentally wire them into production code paths.

### J-5 — Bootstrap-required surface on `mejepa_subscriber_status`

Add a `bootstrap_required: bool` field derived from `ConstellationStore::is_empty()`. Document the seeding step in `06_operations_and_fsv.md`. Pairs with #627 resolution.

## Web-research notes

- Claude Code's PostToolUse exit-code semantics for non-blocking errors are documented at [thepromptshelf.dev "Claude Code Hooks: The Complete 2026 Production Reference"](https://thepromptshelf.dev/blog/claude-code-hooks-complete-reference-2026/) and the "silent failure mode" anti-pattern is dissected at [thinkingthroughcode.medium.com](https://thinkingthroughcode.medium.com/the-silent-failure-mode-in-claude-code-hook-every-dev-should-know-about-0466f139c19f). The pattern matches our `hook_post_tool_use_edit.sh` exit-0-on-failure smell exactly.
- Daubert standard "testability + known error rate" — these would mark the current trainer's EpochSummary hardcoded constants as inadmissible if held to a forensic standard. The fix (Option wrapping + epoch_semantics) makes the report testable.

## Open questions

1. **Is the trainer DIAGNOSTIC_CERTIFICATE_ONLY mode the only mode?** I see references to "checkpointed-train" (#597) being retired. Is there a real-training trainer path post-#597 fix? If yes, where is it and does it set `best_category`/`worst_category`/etc honestly? If no, the entire `train_one_epoch_inner` writes diagnostic certificates only — should the dead AUx/LoRA/distill modules be deleted now rather than later? (Probably yes per #622.)
2. **Is the shift-subscriber bootstrap procedure documented anywhere?** I searched for "ConstellationStore::persist" + "centroid" + "bootstrap" in `06_operations_and_fsv.md` and found nothing. Operator needs an explicit "seed N centroids before starting the subscriber" runbook step.
3. **Should we file F-6 (Phase H absence-check docs drift) as #672 or fold it into the documentation pass that #633 is gating?** Filing it as a `type:discovery` issue is conservative.
4. **The 117 `"passes": True` literals in `scripts/swebench_lite_ops/` — are they all benign because `require()` guarantees the data conditions, or are there cases where the require() is downstream of a possibly-empty data set?** A grep showed `python_failure_fingerprint_materializer.py:586` does the right pattern (derive `passes`); a deeper audit is needed to confirm the others. Filing as a single tech-debt issue rather than 117 separate ones.
