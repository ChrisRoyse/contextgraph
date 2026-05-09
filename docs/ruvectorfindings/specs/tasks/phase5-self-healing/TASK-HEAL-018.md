# TASK-HEAL-018 — CLI heal-drill binary (`cargo run --release -p context-graph-mejepa -- heal-drill --inject-drift {soft|hard|catastrophic}`)

```xml
<task_spec id="TASK-HEAL-018" version="1.0">

<metadata>
  <title>Phase 5 DoD verification CLI in `crates/context-graph-mejepa/src/bin/mejepa-heal-drill.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>18</sequence>
  <implements>
    <item ref="REQ-HEAL-15">mejepa-heal-drill CLI; --inject-drift {soft|hard|catastrophic}; emits 5 FSV evidence files at /tmp/contextgraph-mejepa-self-healing-fsv/</item>
    <item ref="REQ-HEAL-16">Real-data tests only — synthetic but real corpus + real RocksDB + real witness chain (no mocks)</item>
    <item ref="REQ-HEAL-19">Performance budgets: observe p50&lt;5s; A/B/C promotion latency &lt;60min on RTX 5090</item>
    <item ref="AC-HEAL-04">Drift drill assertions: drift_detected=hard ~700 obs; mode_winner∈{B,C}; promotion_latency&lt;60min</item>
    <item ref="AC-HEAL-15">Phase 5 DoD ship-gate: all 5 FSV files exist + 0o600 mode + drill exit 0</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-006</task_ref>
    <task_ref>TASK-HEAL-009</task_ref>
    <task_ref>TASK-HEAL-013</task_ref>
    <task_ref>TASK-HEAL-014</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (src/bin/mejepa-heal-drill.rs); 1 created (src/bin/heal_drill_synthetic.rs as helper module); 1 modified (Cargo.toml [[bin]] entry)</estimated_files>
</metadata>

<context>
Surface 3 of 4. The canonical Phase 5 DoD verification harness. Drives the SelfHealingPipeline
through a deterministic synthetic stress test that exercises every shipped Phase 5 primitive in
sequence: observe() through drift detection → A/B/C promotion → LoRA refresh → witness-chain
integrity verification → 10 task boundaries (Fisher full-rank assertion). The drill binary is THE
entry point reviewers run to confirm Phase 5 ships; its exit code (0 on success, non-zero on any
gate failure) is the Phase 5 DoD ship-gate. The output FSV evidence files are read by TASK-HEAL-019
to assemble the master FSV report.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<utml_v1_invariants>`; `<failure_taxonomy>`; `<performance_budgets>`; CLAUDE.md §11 fail-closed errors + real-data-only)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-15, REQ-HEAL-16, REQ-HEAL-19; AC-HEAL-04, AC-HEAL-15; TC-HEAL-04 drift drill end-to-end)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts HealDrillCli::main 10-step body; api_contracts heal-drill CLI shape)</file>
  <file purpose="prior_task">TASK-HEAL-006 (EwcPlusPlus methods + Fisher full-rank assertion)</file>
  <file purpose="prior_task">TASK-HEAL-009 (AbcPromoter::retrain_and_promote + AbcFsvEvidence)</file>
  <file purpose="prior_task">TASK-HEAL-013 (ChainIntegrityChecker::verify + IntegrityEvidence)</file>
  <file purpose="prior_task">TASK-HEAL-014 (LoraRefresher::refresh + LoraRefreshEvidence + frozen-base byte-equality invariant)</file>
  <file purpose="prior_task">TASK-HEAL-008 (SelfHealingPipeline orchestrator that the drill drives)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-006 + 008 + 009 + 013 + 014 complete (the drill end-to-end exercises all of them)</check>
  <check>Synthetic mock-oracle generator that deterministically produces requested empirical_coverage (e.g., 0.70 for hard drift) — small helper module heal_drill_synthetic.rs</check>
  <check>Synthetic CorpusSlice + Panel fixtures that drive observe() / A/B/C / LoRA refresh end-to-end without depending on Phase 0 SWE-bench Docker oracle (TC-HEAL-04 explicitly allows synthetic)</check>
  <check>Per-binary entry in `crates/context-graph-mejepa/Cargo.toml`: `[[bin]] name = "mejepa-heal-drill" path = "src/bin/mejepa-heal-drill.rs"`</check>
  <check>TEST chain dir (separate from production chain dir) so corruption-injection (step 7) NEVER touches the real chain</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/bin/mejepa-heal-drill.rs`:
      - `fn main() -> Result<(), HealError>` per component_contracts (10-step body):
        1. **Parse args** via clap: `--inject-drift {soft|hard|catastrophic}` (required), `--output-fsv {PATH}` (default `/tmp/contextgraph-mejepa-self-healing-fsv/`), `--max-observations {N}` (default 5000), `--seed {u64}` (default 42 for determinism), `--rtx-5090-budget-min {N}` (default 60 — soft assertion; warn-only on commodity hardware)
        2. **Bootstrap synthetic environment**: create a tempdir-rooted RocksDB; create a fresh test witness chain (NEVER touch the real one); load synthetic CorpusSlice + Panel fixtures + a deterministic mock-oracle that produces empirical_coverage = `match inject_drift { Soft => 0.86, Hard => 0.82, Catastrophic => 0.75 }`
        3. **Run observe() loop** with the synthetic stream until `drift_detector.detect_drift()` returns the requested band OR `max_observations` hit; record `observations_to_detect = i` at the moment of detection
        4. **Emit drift-drill-evidence.json**: `{ inject_drift, observations_to_detect, severity_history: Vec<(step, DriftSeverity)>, empirical_coverage_trajectory, max_observations, hit_max_without_detection: bool }`
        5. **Trigger A/B/C**: spawn `abc_promoter.retrain_and_promote(TriggerReason::DriftHard, ...)` (or DriftCatastrophic per band); wait for HealReport; emit `abc-promotion-evidence.json` from `AbcFsvEvidence` (TASK-HEAL-009): `{ mode_a_score, mode_b_score, mode_c_score, mode_c_weights, mode_winner, theta_sha_winner, evaluation_summary_sha, witness_chain_offset, promotion_latency_seconds }`
        6. **Force LoRA refresh**: synthesize a corpus slice that drives ΔΩ.effective_plasticity below 0.4; invoke `lora_refresher.refresh(embedder_id, fresh_corpus_slice, ...)`; emit `lora-refresh-evidence.json` from `LoraRefreshEvidence` (TASK-HEAL-014): `{ embedder_id, pre_base_sha, post_base_sha, pre_lora_sha, post_lora_sha, pre_plasticity, post_plasticity, num_epochs_run, fine_tune_duration_ms, frozen_base_byte_equal: bool }`
        7. **Force witness-chain integrity break + recovery**: write garbage bytes at a known offset of the TEST chain (NEVER the production chain — assert `chain_path.starts_with(tempdir)` before write); call `integrity_checker.verify(...)`; assert returns `Err(IntegrityViolation { kind: BrokenAt(known_offset, ...) })`; emit `witness-integrity-evidence.json` from `IntegrityEvidence` (TASK-HEAL-013): `{ entry_count, first_entry_sha, last_entry_sha, replay_duration_ms, status_change_after, rollback_target_sha, broken_at_offset, blocker_path }` PLUS the success-path counterpart: re-run verify on a clean chain and capture the success evidence side-by-side
        8. **Force 10 task boundaries**: invoke `ewc.detect_task_boundary()` 10 times (synthetically inducing loss-curvature shifts via the mock-oracle's coverage transitions); after each call, invoke `ewc.snapshot_current_task()`; assert Fisher rank == predictor_dim AFTER the 10th boundary; emit `ewc-fisher-evidence.json`: `{ boundary_step_trajectory, lambda_trajectory, fisher_rank_per_boundary, fisher_full_rank_after_boundary_10: bool, task_snapshots_count }`
        9. **Set FSV file modes to 0o600** per SEC-MEJEPA-07: for each emitted file, `fs::set_permissions(path, fs::Permissions::from_mode(0o600))`; verify via stat() before exit
        10. **Print stdout JSON summary**: `{ drift_detected: severity, observations_to_detect, retrain_triggered: true, mode_winner: 'B'|'C'|'A', promotion_latency_seconds, lora_refresh_succeeded: bool, integrity_violation_detected: bool, fisher_rank_after_10_boundaries: usize, fisher_full_rank: bool, fsv_files_emitted: Vec<String>, all_fsv_files_mode_0o600: bool, exit_code: 0|1 }`
        11. **Exit gates** (any false → exit 1):
            - `observations_to_detect ∈ [400, 1000]` for inject_drift=hard (700 ± 300 tolerance per AC-HEAL-04)
            - `mode_winner ∈ {B, C}` (NEVER A_unmodified per Phase 5 DoD)
            - `promotion_latency_seconds < rtx_5090_budget_min * 60` (soft on commodity hardware: warn-only with `tracing::warn!`; assert only when running on RTX 5090 — detect via env var `MEJEPA_RTX_5090=1`)
            - `lora_refresh_succeeded == true` AND `frozen_base_byte_equal == true` (BR-HEAL-06)
            - `integrity_violation_detected == true` (the injected break MUST be caught)
            - `fisher_full_rank == true` after 10 boundaries
            - All 5 FSV files exist + mode 0o600
      - Helper module `heal_drill_synthetic.rs`:
        - `pub fn deterministic_mock_oracle(target_coverage: f32, seed: u64) -> impl OracleHandle` — returns a mock that produces actual-vs-predicted matches at the target_coverage rate
        - `pub fn synthetic_corpus_slice(seed: u64, num_samples: usize) -> CorpusSlice` — deterministic synthetic corpus
        - `pub fn synthetic_panel(seed: u64, witness_chain_offset: u64) -> Panel`
        - `pub fn write_garbage_at_offset(test_chain_path: &Path, offset: usize, n_bytes: usize) -> Result<(), HealError>` — corruption injection helper; INVARIANT: `test_chain_path.starts_with(env::temp_dir())`
    - Tests:
      - `cli_parses_inject_drift_soft_hard_catastrophic`
      - `cli_rejects_invalid_inject_drift_value`
      - `cli_default_output_fsv_path_under_tmp`
      - `cli_default_max_observations_5000`
      - `cli_seed_default_42_deterministic_run`
      - `synthetic_oracle_produces_target_coverage_with_tolerance` (0.82 → ±0.02 over 1000 samples)
      - `synthetic_corpus_slice_deterministic` (same seed → byte-equal slice)
      - `write_garbage_at_offset_rejects_non_tempdir_path` (safety invariant; refuses to corrupt anything outside tempdir)
      - `drill_detects_hard_drift_within_700_plus_or_minus_300_observations` (AC-HEAL-04)
      - `drill_emits_all_5_fsv_files_at_default_path`
      - `drill_sets_all_fsv_files_to_mode_0o600` (SEC-MEJEPA-07)
      - `drill_mode_winner_is_b_or_c_never_a_unmodified`
      - `drill_lora_refresh_frozen_base_byte_equal_invariant_holds`
      - `drill_integrity_violation_detected_at_injected_offset`
      - `drill_fisher_full_rank_after_10_boundaries`
      - `drill_exit_code_zero_on_success`
      - `drill_exit_code_nonzero_on_lora_refresh_failure`
      - `drill_exit_code_nonzero_on_fisher_rank_deficient_after_10_boundaries`
      - `drill_test_chain_dir_isolated_from_production` (assert chain path starts with env::temp_dir())
      - End-to-end with `--inject-drift hard`: full drill completes; all gates pass; exit 0
      - End-to-end with `--inject-drift catastrophic`: status_change=paused observed; full retrain triggered (vs A/B/C-only); exit 0
    - Stdout summary JSON schema validated (additionalProperties: false on the printed structure)
  </in_scope>
  <out_of_scope>
    - The 5 underlying primitives' internals (TASK-HEAL-006/009/013/014 already shipped them)
    - The master FSV report assembly (TASK-HEAL-019 reads these emitted files)
    - Production RTX 5090 latency assertion (warn-only on commodity hardware; only hard-asserted when `MEJEPA_RTX_5090=1`)
    - The MCP tool surfaces (TASK-HEAL-016 + TASK-HEAL-017)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/bin/mejepa-heal-drill.rs">
use clap::Parser;
use context_graph_mejepa::heal::{errors::HealError, /* etc */};

#[derive(Parser, Debug, Clone)]
#[command(name = "mejepa-heal-drill", about = "Phase 5 DoD verification harness")]
pub struct HealDrillArgs {
    #[arg(long, value_enum)] pub inject_drift: InjectDrift,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-self-healing-fsv/")] pub output_fsv: std::path::PathBuf,
    #[arg(long, default_value_t = 5000)] pub max_observations: u64,
    #[arg(long, default_value_t = 42)] pub seed: u64,
    #[arg(long, default_value_t = 60)] pub rtx_5090_budget_min: u64,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum InjectDrift { Soft, Hard, Catastrophic }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrillStdoutSummary {
    pub drift_detected: DriftSeverity,
    pub observations_to_detect: u64,
    pub retrain_triggered: bool,
    pub mode_winner: char,
    pub promotion_latency_seconds: u64,
    pub lora_refresh_succeeded: bool,
    pub integrity_violation_detected: bool,
    pub fisher_rank_after_10_boundaries: usize,
    pub fisher_full_rank: bool,
    pub fsv_files_emitted: Vec&lt;String&gt;,
    pub all_fsv_files_mode_0o600: bool,
    pub exit_code: i32,
}

fn main() -&gt; Result&lt;(), HealError&gt;;
    </signature>
  </signatures>
  <constraints>
    - REQ-HEAL-16: real-data ONLY (no mock structs that bypass the production paths). Synthetic-but-real fixtures are acceptable; binary mocks of HealError / WitnessChain / RocksDB are NOT
    - SEC-MEJEPA-07: all 5 FSV files mode 0o600 (verified via stat() before exit)
    - Chain corruption MUST happen ONLY in tempdir (write_garbage_at_offset asserts before write)
    - Determinism: same `--seed` MUST produce byte-equal observations_to_detect, mode_winner, theta_sha_winner across runs
    - Exit code 0 on ALL gates pass; exit 1 on any gate failure (single-bit ship-gate result)
    - tracing::warn! for soft-failures (commodity hardware can't meet RTX 5090 latency); tracing::error! for hard-failures (LoRA, Fisher, integrity)
    - Stdout JSON summary additionalProperties: false (validated against the DrillStdoutSummary schema)
    - File &lt; 500 lines (split helper into heal_drill_synthetic.rs)
    - SCREAMING_SNAKE error codes propagated from inner primitives (no swallow)
    - Cargo [[bin]] entry: `name = "mejepa-heal-drill"` so `cargo run --release -p context-graph-mejepa -- heal-drill ...` works (note: clap subcommand `heal-drill` + binary `mejepa-heal-drill` both supported per technical spec api_contracts)
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa --bin mejepa-heal-drill` shows ≥ 18 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - `cargo run --release -p context-graph-mejepa --bin mejepa-heal-drill -- --inject-drift hard` exits 0 + emits 5 FSV files at /tmp/contextgraph-mejepa-self-healing-fsv/ + all 5 are mode 0o600
    - observations_to_detect ∈ [400, 1000] for hard drift
    - mode_winner ∈ {B, C}; never A
    - LoRA refresh succeeded with frozen_base_byte_equal=true
    - Witness chain integrity violation detected at injected offset
    - Fisher full-rank after 10 task boundaries
    - Exit code is non-zero when ANY gate fails (forced via fault-injection in tests)
  </verification>
</definition_of_done>

<pseudo_code>
fn main() -> Result<(), HealError> {
    let args = HealDrillArgs::parse();
    let tmp = tempfile::tempdir().unwrap();
    let storage = bootstrap_synthetic_rocksdb(tmp.path())?;
    let test_chain_path = tmp.path().join("test_witness_chain.bin");
    let witness_chain = bootstrap_test_witness_chain(&test_chain_path)?;
    let oracle = deterministic_mock_oracle(coverage_for(args.inject_drift), args.seed);
    let mut pipeline = SelfHealingPipeline::new(/* ... */)?;

    // Step 3-4: drift detection
    let mut observations_to_detect = 0_u64;
    let mut severity_history = vec![];
    for i in 0..args.max_observations {
        let panel = synthetic_panel(args.seed.wrapping_add(i), i);
        let outcome = oracle.outcome_for(&panel);
        let signal_clarity = 0.9_f32; // synthetic high clarity
        match pipeline.observe(&panel, &outcome, signal_clarity, i, "drill") {
            Err(HealError::DriftDetected { severity, .. }) if matches_band(severity, args.inject_drift) => {
                observations_to_detect = i;
                severity_history.push((i, severity));
                break;
            }
            other => severity_history.push((i, /* current */ DriftSeverity::Healthy)),
        }
    }
    write_drift_drill_evidence(&args.output_fsv, observations_to_detect, &severity_history)?;

    // Step 5: A/B/C
    let report = pipeline.abc_promoter.retrain_and_promote(/* ... */)?;
    write_abc_promotion_evidence(&args.output_fsv, &pipeline.abc_promoter.fsv_snapshot())?;

    // Step 6: LoRA refresh
    let fresh_slice = synthetic_corpus_slice(args.seed.wrapping_add(1), 100);
    let lora_report = pipeline.lora_refresher.refresh(EmbedderId(0), &fresh_slice, storage.clone(), /* ... */)?;
    write_lora_refresh_evidence(&args.output_fsv, &LoraRefreshEvidence::from(&lora_report))?;

    // Step 7: integrity break + recovery
    assert!(test_chain_path.starts_with(tmp.path()));
    write_garbage_at_offset(&test_chain_path, 500, 10)?;
    let integrity_result = pipeline.integrity_checker.verify(/* ... */);
    let integrity_violation_detected = matches!(integrity_result, Err(HealError::IntegrityViolation { kind: IntegrityViolationKind::BrokenAt { .. } }));
    write_witness_integrity_evidence(&args.output_fsv, &IntegrityEvidence { /* from integrity_result */ })?;

    // Step 8: 10 task boundaries
    let mut boundary_steps = vec![];
    let mut lambda_trajectory = vec![];
    let mut fisher_rank_per_boundary = vec![];
    for k in 0..10 {
        induce_loss_curvature_shift(&mut pipeline);
        if pipeline.ewc.detect_task_boundary() {
            pipeline.ewc.snapshot_current_task()?;
            boundary_steps.push(pipeline.ewc.step_count());
            lambda_trajectory.push(pipeline.ewc.current_lambda());
            fisher_rank_per_boundary.push(pipeline.ewc.fisher_rank());
        }
    }
    let fisher_full_rank = pipeline.ewc.fisher_rank() == pipeline.predictor.read().unwrap().param_count();
    write_ewc_fisher_evidence(&args.output_fsv, &EwcFisherEvidence { boundary_steps, lambda_trajectory, fisher_rank_per_boundary, fisher_full_rank, task_snapshots_count: 10 })?;

    // Step 9: 0o600 mode
    set_fsv_files_mode_0o600(&args.output_fsv)?;

    // Step 10-11: exit gates
    let summary = build_summary(observations_to_detect, &report, lora_report.post_plasticity > 0.4, integrity_violation_detected, fisher_full_rank, &args);
    println!("{}", serde_json::to_string_pretty(&summary)?);
    if !all_gates_pass(&summary, &args) {
        std::process::exit(1);
    }
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/bin/mejepa-heal-drill.rs">main + HealDrillArgs + DrillStdoutSummary + 4 evidence-emit functions + tests</file>
  <file path="crates/context-graph-mejepa/src/bin/heal_drill_synthetic.rs">deterministic_mock_oracle + synthetic_corpus_slice + synthetic_panel + write_garbage_at_offset + tempdir-invariant assertion</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `[[bin]] name = "mejepa-heal-drill" path = "src/bin/mejepa-heal-drill.rs"`; add `clap = { workspace = true, features = ["derive"] }` + `tempfile = { workspace = true }`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa --bin mejepa-heal-drill shows ≥ 18 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>cargo run --release -p context-graph-mejepa --bin mejepa-heal-drill -- --inject-drift hard exits 0</criterion>
  <criterion>5 FSV files emitted at /tmp/contextgraph-mejepa-self-healing-fsv/: drift-drill-evidence.json + abc-promotion-evidence.json + lora-refresh-evidence.json + witness-integrity-evidence.json + ewc-fisher-evidence.json</criterion>
  <criterion>All 5 files mode 0o600 (verified via stat())</criterion>
  <criterion>observations_to_detect ∈ [400, 1000] for hard drift (AC-HEAL-04 ±300 tolerance on ~700)</criterion>
  <criterion>mode_winner ∈ {B, C} (never A_unmodified)</criterion>
  <criterion>LoRA refresh succeeded; frozen_base_byte_equal=true (BR-HEAL-06)</criterion>
  <criterion>Integrity violation detected at injected offset 500</criterion>
  <criterion>Fisher full-rank after 10 boundaries</criterion>
  <criterion>Same --seed → byte-equal results across runs (determinism)</criterion>
  <criterion>Test chain corruption confined to tempdir (production chain untouched)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa --bin mejepa-heal-drill</command>
  <command>cargo run --release -p context-graph-mejepa --bin mejepa-heal-drill -- --inject-drift hard --seed 42</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
