# TASK-PRED-011 — Forward-pass FSV harness `examples/predictor_fsv.rs` (forward-pass-evidence + vicreg-loss-evidence)

```xml
<task_spec id="TASK-PRED-011" version="1.0">

<metadata>
  <title>Forensic Source Verification harness: write/read/assert forward-pass-evidence.json + vicreg-loss-evidence.json + adversarial cases</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-PRED-15">FSV evidence at /tmp/contextgraph-mejepa-predictor-fsv/{forward-pass-evidence.json, vicreg-loss-evidence.json}; mode 0o600</item>
    <item ref="REQ-PRED-03">forward_latency_ms_p50 measured via warm-call loop</item>
    <item ref="REQ-PRED-04">Adversarial NaN injection verifies MEJEPA_PRED_NAN_DETECTED fires</item>
    <item ref="REQ-PRED-05">Determinism: SHA-256 of two consecutive eval-mode forwards is byte-equal</item>
    <item ref="REQ-PRED-10">Adversarial gradient violation verifies MEJEPA_PRED_FROZEN_TARGET_GRAD fires</item>
    <item ref="REQ-PRED-11">FrozenTargetAdapter encode_target byte-equal across calls</item>
    <item ref="REQ-PRED-13">VRAM resident_bytes recorded in evidence</item>
    <item ref="REQ-PRED-14">Adversarial over-allocation verifies MEJEPA_PRED_VRAM_EXCEEDED fires</item>
    <item ref="component:PredictorFsvHarness">examples/predictor_fsv.rs binary</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-005</task_ref>
    <task_ref>TASK-PRED-006</task_ref>
    <task_ref>TASK-PRED-007</task_ref>
    <task_ref>TASK-PRED-008</task_ref>
    <task_ref>TASK-PRED-009</task_ref>
    <task_ref>TASK-PRED-010</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (example binary), 1 modified (lib.rs to expose FSV writer helpers)</estimated_files>
</metadata>

<context>
The Phase 2 FSV harness is the forensic verifier: it constructs a `MeJepaPredictor` with default
config + a (possibly empty) FrozenTargetAdapter, runs forward passes, computes VICReg loss,
probes VRAM, and writes evidence JSON to `/tmp/contextgraph-mejepa-predictor-fsv/` (mode 0o600
per SEC-MEJEPA-07). It then **re-reads each file and asserts** all thresholds — this is the
constitutional FSV protocol (`<fsv_protocol>`: "write evidence → re-read → assert").

The harness covers nominal AND adversarial cases:

  Nominal:
    - forward-pass-evidence.json: 100 warm forward calls; record p50/p99 latency; SHA-256 of
      output bytes; vram_resident_bytes; dod_pass.
    - vicreg-loss-evidence.json: one vicreg_loss call on (predicted, target) panels; record
      l_predict / l_variance / l_covariance / l_invariance / l_total; formula_check; finite.
    - determinism-evidence.json: two eval-mode forwards on same inputs → byte-equal SHA-256.
    - frozen-target-determinism-evidence.json: two encode_target calls → byte-equal target SHA.

  Adversarial (each failure case writes its own evidence file with `expected_error_code` and
  `actual_error_code` fields; the file is REQUIRED so the operator can inspect even when the
  case correctly fails):
    - adversarial-nan-input-evidence.json: panel_t0 with NaN → MEJEPA_PRED_NAN_DETECTED
    - adversarial-dim-mismatch-evidence.json: panel_t0 wrong dim → MEJEPA_PRED_DIM_MISMATCH
    - adversarial-frozen-grad-evidence.json: synthetic InstrumentGradHandle violation → MEJEPA_PRED_FROZEN_TARGET_GRAD
    - adversarial-vram-exceeded-evidence.json: synthetic over-allocation → MEJEPA_PRED_VRAM_EXCEEDED
      (gracefully skipped on hosts where 9 GB+ allocation is unsafe; logged)

Iron Rule observed: the harness only invokes APIs already shipped by TASK-PRED-001..010. It
does NOT call the DoD CLI subcommand (TASK-PRED-012; that's a separate consumer of the same
artifacts). Synthetic panels are random (no real corpus required for Phase 2 dry-run).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (fsv_protocol; testing_requirements fsv_evidence_paths; security_requirements SEC-MEJEPA-07 mode 0o600)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-15; AC-PRED-06-01..02; EC-PRED-08 sm_120a unavailable; AC-PRED-03-01..02 frozen target adversarial)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (component_contracts PredictorFsvHarness; api_contracts ForwardPassEvidence + VicregLossEvidence; implementation_notes "phase2-dod-binary", "testing-fsv")</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (ForwardPassEvidence + VicregLossEvidence DTO definitions)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-005.md (forward + forward_dryrun + trainable_parameters + architecture_summary)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-006.md (FrozenTargetAdapter::empty_for_test + encode_target + target_sha256)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-007.md (vicreg_loss + vicreg_loss_with_variance_count)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-008.md (OracleHead::predict_logits + predict_probabilities)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-009.md (run_gradient_hook + GradientHookReport)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-010.md (check_vram_steady_state + VramReport)</file>
  <file purpose="reference_pattern">crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs (sibling FSV harness pattern)</file>
  <file purpose="reference_pattern">memory/project_phase0_mejepa_mutation_corpus.md (FSV evidence write→re-read→assert pattern)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-001..010 complete</check>
  <check>cargo build --release -p context-graph-mejepa --features cuda succeeds on RTX 5090</check>
  <check>cargo test -p context-graph-mejepa shows ≥ 61 tests passing</check>
  <check>/tmp directory writable</check>
  <check>code_version helper available — read CARGO_PKG_VERSION + git SHA via std::process or build-script env var (use `option_env!("VERGEN_GIT_SHA")` if vergen is in workspace; else fallback to `env!("CARGO_PKG_VERSION")` + a placeholder "unknown")</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/examples/predictor_fsv.rs` binary (clap-style argv parsing OK; can be flag-free with hardcoded defaults)
    - Implement `fn main() -> Result<(), Box<dyn std::error::Error>>` that:
        1. Construct MeJepaPredictor (default config; FrozenTargetAdapter::empty_for_test; num_tests=30; Device::Cuda(0))
        2. Generate synthetic random panels (panel_t0, panel_t1) shape (64, 5120) bf16 cuda
        3. **Warmup loop** (10 calls, discarded)
        4. **Timing loop** (100 calls, record per-call latency_ms via Instant::now)
        5. Compute p50, p99 from sorted latencies
        6. Run vicreg_loss(predicted, target=encode_target(synthetic_panel_t2)) once
        7. Probe VRAM via check_vram_steady_state
        8. Build ForwardPassEvidence + VicregLossEvidence
        9. Write JSON to /tmp/contextgraph-mejepa-predictor-fsv/{forward-pass-evidence.json, vicreg-loss-evidence.json} with `OpenOptions::new().mode(0o600).create_new(true).write(true).open(...)`
        10. **Re-read each file** and `serde_json::from_reader` parse; assert thresholds
        11. Run determinism check: two forward calls → SHA-256 byte-equal; write `determinism-evidence.json`
        12. Run frozen-target determinism: two encode_target → SHA-256 byte-equal; write `frozen-target-determinism-evidence.json`
        13. Run adversarial cases sequentially (each writes its own evidence file):
            - inject NaN into panel_t0 → expect Err(NanDetected)
            - panel_t0 shape (64, 4096) → expect Err(DimMismatch)
            - synthetic FrozenTargetAdapter with InstrumentGradHandle that registers a TensorId with grad norm 1.0 → run_gradient_hook expect Err(FrozenTargetGrad)
            - synthetic over-allocation: try to allocate (64, 100_000_000) bf16 tensor (~12 GB), then probe VRAM; expect Err(VramExceeded) — gracefully skip on hosts that OOM-protect this
        14. Print structured stdout: `forward_latency_ms_p50`, `_p99`, `vram_resident_bytes`, `l_total`, `l_predict`, `l_variance`, `l_covariance`, `l_invariance`, `dod_pass`
        15. Exit 0 on full DoD pass; exit 2 on first failing nominal threshold
    - Add a public helper `pub fn write_evidence_file<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()>` in a new `crates/context-graph-mejepa/src/fsv_writer.rs` module (mode 0o600; create_new=true to fail if file already exists)
    - Add a public helper `pub fn read_evidence_file<T: DeserializeOwned>(path: &Path) -> std::io::Result<T>` in the same module
    - Add helper `pub fn ensure_fsv_dir() -> std::io::Result<()>` that creates `/tmp/contextgraph-mejepa-predictor-fsv/` if absent + clears any pre-existing evidence files (idempotent harness reruns)
    - 4 integration tests (cuda-gated where required) verifying the harness binary's outputs are well-formed (run via `std::process::Command::new("cargo run --release ...")` OR refactor logic into a library function the test calls directly — pick the latter for hermeticity)
  </in_scope>
  <out_of_scope>
    - bin/mejepa.rs `train --dry-run` CLI (TASK-PRED-012; the FSV harness is a separate `examples/` binary per TECH-PREDICTOR `<note category="phase2-dod-binary">`)
    - Real corpus loading (TASK-PRED-012 may load first 64 corpus examples; this task uses synthetic random panels)
    - Phase 3 training-loop dry-run with backward + optimizer.step (Phase 3)
    - Conformal calibration / OOD scoring (Phase 4)
    - Embedder ensemble loading (Phase 1b — not required since FrozenTargetAdapter::empty_for_test has zero instruments)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/examples/predictor_fsv.rs">
fn main() -&gt; Result&lt;(), Box&lt;dyn std::error::Error&gt;&gt;;

// Internal helpers (not pub):
fn run_nominal_forward_loop(predictor: &amp;MeJepaPredictor, batches: usize, batch_size: usize) -&gt; Result&lt;ForwardLatencies, ...&gt;;
fn run_vicreg_loss_case(predictor: &amp;MeJepaPredictor, lambdas: VicregLambdas) -&gt; Result&lt;LossOutputs, ...&gt;;
fn run_determinism_check(predictor: &amp;MeJepaPredictor) -&gt; Result&lt;DeterminismEvidence, ...&gt;;
fn run_frozen_target_determinism_check(adapter: &amp;FrozenTargetAdapter, ...) -&gt; Result&lt;...&gt;;
fn run_adversarial_nan_input(predictor: &amp;MeJepaPredictor) -&gt; Result&lt;AdversarialEvidence, ...&gt;;
fn run_adversarial_dim_mismatch(predictor: &amp;MeJepaPredictor) -&gt; Result&lt;AdversarialEvidence, ...&gt;;
fn run_adversarial_frozen_grad(...) -&gt; Result&lt;AdversarialEvidence, ...&gt;;
fn run_adversarial_vram_exceeded(...) -&gt; Result&lt;Option&lt;AdversarialEvidence&gt;, ...&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/fsv_writer.rs">
use std::path::Path;
use serde::{Serialize, de::DeserializeOwned};

pub const FSV_DIR: &amp;str = "/tmp/contextgraph-mejepa-predictor-fsv";

pub fn ensure_fsv_dir() -&gt; std::io::Result&lt;()&gt;;

/// Write evidence file with mode 0o600 (owner-only) per SEC-MEJEPA-07.
/// Uses `create_new(true)` to fail closed on stale leftovers; caller invokes
/// `clear_fsv_dir()` first when reruns are intended.
pub fn write_evidence_file&lt;T: Serialize&gt;(path: &amp;Path, value: &amp;T) -&gt; std::io::Result&lt;()&gt;;

pub fn read_evidence_file&lt;T: DeserializeOwned&gt;(path: &amp;Path) -&gt; std::io::Result&lt;T&gt;;

pub fn clear_fsv_dir() -&gt; std::io::Result&lt;()&gt;;
    </signature>
  </signatures>
  <constraints>
    - **Output directory**: `/tmp/contextgraph-mejepa-predictor-fsv/` per constitution `<testing_requirements>` `fsv_evidence_paths`
    - **File mode**: 0o600 owner-only per SEC-MEJEPA-07 — use `OpenOptions::new().mode(0o600).create_new(true).write(true).open(path)` (the `mode` setter is `unix-only`; gate behind `#[cfg(unix)]`; on non-unix the harness emits a single `tracing::warn!` and uses default permissions)
    - **Idempotency**: `clear_fsv_dir()` removes ALL `*.json` files under FSV_DIR at the start of `main()`; subsequent `write_evidence_file` calls succeed because `create_new` finds no leftover
    - **Latency measurement**: `std::time::Instant` (NOT `chrono`); convert duration to f32 ms via `.as_secs_f64() * 1000.0`
    - **Warmup**: 10 calls discarded; **timing**: 100 calls; sort latencies; p50 = sorted[49]; p99 = sorted[98]
    - **Synthetic panels**: `Tensor::randn(0.0, 1.0, (batch=64, PANEL_DIM), &device)?.to_dtype(BF16)?` (gaussian unit-variance; seed not pinned at this layer — determinism tests use fixed-seed inputs constructed via `Tensor::from_slice` of a hardcoded f32 vector)
    - **For determinism tests**: use `Tensor::from_slice` with a hardcoded `Vec<f32>` of length `64 * 5120` (synthesize via deterministic procedure: e.g. `(0..n).map(|i| (i as f32 / n as f32 * 2.0 - 1.0)).collect()`); cast to bf16; same input bytes ⇒ same output bytes
    - **SHA-256**: hash the bf16 byte representation of the predicted tensor; cast to f32 for portability per TASK-PRED-006 `target_sha256` convention; record both raw_dtype="bf16" and hash_dtype="f32" in evidence
    - **CUBLAS_WORKSPACE_CONFIG**: print a warning + recommendation if not set when starting; do not enforce (env var is the operator's responsibility per TECH-PREDICTOR `<note category="determinism">`)
    - **Adversarial-VRAM-graceful-skip**: if the synthetic over-allocation candle call returns Err (CUDA OOM), capture the Err and write `adversarial-vram-exceeded-evidence.json` with `expected_error_code=MEJEPA_PRED_VRAM_EXCEEDED, actual_error_code="CANDLE_OOM_OR_MAPPED"` and `skipped=true` rather than crashing the harness
    - **DoD pass criteria** (the binary's exit code reflects these):
        forward_latency_ms_p50 &lt; 50.0
        l_total finite AND in [10.0, 1000.0]
        l_predict / l_variance / l_covariance / l_invariance all finite
        vram_resident_bytes &lt; VRAM_STEADY_STATE_TARGET_BYTES (8 GB)
        no NaN/Inf in nominal forward
        determinism: SHA-256 of two forwards is byte-equal
        adversarial cases all return their EXPECTED error codes (no false-pass through)
    - **stdout**: human-readable structured output (one key=value per line); plus a final line `dod_pass=true|false`
    - **Exit codes**: 0 if dod_pass; 2 with first-failing-metric on stderr otherwise; 3 if a non-DoD harness assertion (e.g. file write/re-read) fails
    - File length stays &lt; 500 lines per file (split by helper into `examples/predictor_fsv.rs` + `src/fsv_writer.rs` + optionally `examples/predictor_fsv_helpers.rs` if approaching limit)
    - No `unwrap()` in non-test code; map every fallible call to a propagated error
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa --example predictor_fsv --features cuda succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 65 tests passing (this task adds 4 integration tests)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - On RTX 5090 host: `cargo run --release -p context-graph-mejepa --example predictor_fsv --features cuda` exits 0 and writes:
        /tmp/contextgraph-mejepa-predictor-fsv/forward-pass-evidence.json (mode 0o600, parseable, contains forward_latency_ms_p50 &lt; 50.0)
        /tmp/contextgraph-mejepa-predictor-fsv/vicreg-loss-evidence.json (formula_check == true; all components finite)
        /tmp/contextgraph-mejepa-predictor-fsv/determinism-evidence.json (sha256_a == sha256_b)
        /tmp/contextgraph-mejepa-predictor-fsv/frozen-target-determinism-evidence.json
        /tmp/contextgraph-mejepa-predictor-fsv/adversarial-nan-input-evidence.json (actual_error_code == "MEJEPA_PRED_NAN_DETECTED")
        /tmp/contextgraph-mejepa-predictor-fsv/adversarial-dim-mismatch-evidence.json
        /tmp/contextgraph-mejepa-predictor-fsv/adversarial-frozen-grad-evidence.json
        /tmp/contextgraph-mejepa-predictor-fsv/adversarial-vram-exceeded-evidence.json (or skipped=true)
    - Each evidence file is re-readable and the harness's own assertions pass before exit
  </verification>
</definition_of_done>

<pseudo_code>
fsv_writer.rs:
    pub const FSV_DIR: &str = "/tmp/contextgraph-mejepa-predictor-fsv";
    pub fn ensure_fsv_dir() -&gt; io::Result&lt;()&gt; { fs::create_dir_all(FSV_DIR) }
    pub fn clear_fsv_dir() -&gt; io::Result&lt;()&gt; {
        for entry in fs::read_dir(FSV_DIR)? {
            let p = entry?.path();
            if p.extension().map_or(false, |e| e == "json") { fs::remove_file(p)?; }
        }
        Ok(())
    }
    pub fn write_evidence_file&lt;T: Serialize&gt;(path, value) -&gt; io::Result&lt;()&gt; {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)] { opts.mode(0o600); }
        let file = opts.open(path)?;
        serde_json::to_writer_pretty(file, value)?;
        Ok(())
    }
    pub fn read_evidence_file&lt;T: DeserializeOwned&gt;(path) -&gt; io::Result&lt;T&gt; {
        let file = File::open(path)?;
        Ok(serde_json::from_reader(file)?)
    }

examples/predictor_fsv.rs (top-level main):
    fn main() -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt; {
        ensure_fsv_dir()?;
        clear_fsv_dir()?;
        let device = Device::cuda_if_available(0)?;
        let cfg = PredictorConfig::default();
        let adapter = FrozenTargetAdapter::empty_for_test();
        let predictor = MeJepaPredictor::new(cfg, adapter, device.clone(), 30)?;

        let mut all_dod_pass = true;

        // 1. Nominal forward loop
        let latencies = run_nominal_forward_loop(&predictor, 100, 64)?;  // (warmup 10) + 100 timed
        let p50 = latencies.percentile(50);
        let p99 = latencies.percentile(99);
        let predicted_sha = sha256_of_bf16(&latencies.last_output);
        let vram = check_vram_steady_state(&device).unwrap_or(VramReport{...zero_filled});

        // 2. VICReg loss (synthetic target via empty FrozenTargetAdapter encoding zeros panel)
        let target = synthetic_target_panel_zeros(&device, BF16);  // (64, 5120)
        let lambdas = VicregLambdas::default();
        let loss = vicreg_loss(&latencies.last_output, &target, lambdas)?;
        let l_total_in_range = (10.0..=1000.0).contains(&loss.l_total);

        // 3. Build evidence
        let fwd_evidence = ForwardPassEvidence {
            panel_dim: PANEL_DIM, batch_size: 64,
            forward_latency_ms_p50: p50, forward_latency_ms_p99: p99,
            output_finite: true, output_sha256: predicted_sha.clone(),
            vram_resident_bytes: vram.resident_bytes,
            device: format!("{:?}", device),
            code_version: env!("CARGO_PKG_VERSION").to_string(),
            dod_pass: p50 &lt; 50.0 && vram.dod_pass_8gb && l_total_in_range && loss.is_finite_decomposition(),
        };
        let vicreg_evidence = VicregLossEvidence {
            batch_size: 64, lambdas,
            l_predict: loss.l_predict, l_variance: loss.l_variance,
            l_covariance: loss.l_covariance, l_invariance: loss.l_invariance, l_total: loss.l_total,
            components_finite: loss.is_finite_decomposition(),
            formula_check: (loss.l_total - (loss.l_predict + 25.0*loss.l_variance + loss.l_covariance + 25.0*loss.l_invariance)).abs() &lt; 1e-3,
            dod_pass: l_total_in_range,
        };
        write_evidence_file(&path("forward-pass-evidence.json"), &fwd_evidence)?;
        write_evidence_file(&path("vicreg-loss-evidence.json"), &vicreg_evidence)?;

        // 4. Re-read + assert
        let re_fwd: ForwardPassEvidence = read_evidence_file(&path("forward-pass-evidence.json"))?;
        assert_eq!(re_fwd.panel_dim, PANEL_DIM);
        assert!(re_fwd.dod_pass || !fwd_evidence.dod_pass, "dod_pass round-trip");
        let re_vicreg: VicregLossEvidence = read_evidence_file(&path("vicreg-loss-evidence.json"))?;
        assert!(re_vicreg.formula_check);

        // 5. Determinism
        let det_a = run_one_forward(&predictor, &deterministic_panels())?;
        let det_b = run_one_forward(&predictor, &deterministic_panels())?;
        let sha_a = sha256_of_bf16(&det_a);
        let sha_b = sha256_of_bf16(&det_b);
        let det_evidence = json!({"sha256_a": sha_a, "sha256_b": sha_b, "byte_equal": sha_a == sha_b});
        write_evidence_file(&path("determinism-evidence.json"), &det_evidence)?;
        if sha_a != sha_b { all_dod_pass = false; }

        // 6. Frozen-target determinism (encode same Panel twice)
        let panel_t2 = synthetic_panel_t2();
        let sha_t1 = adapter.target_sha256(&panel_t2, &device, BF16)?;
        let sha_t2 = adapter.target_sha256(&panel_t2, &device, BF16)?;
        write_evidence_file(&path("frozen-target-determinism-evidence.json"),
            &json!({"sha256_a": sha_t1, "sha256_b": sha_t2, "byte_equal": sha_t1 == sha_t2}))?;

        // 7. Adversarial cases
        let adv_nan = run_adversarial_nan_input(&predictor)?;
        write_evidence_file(&path("adversarial-nan-input-evidence.json"), &adv_nan)?;

        let adv_dim = run_adversarial_dim_mismatch(&predictor)?;
        write_evidence_file(&path("adversarial-dim-mismatch-evidence.json"), &adv_dim)?;

        let adv_frozen = run_adversarial_frozen_grad(&device)?;
        write_evidence_file(&path("adversarial-frozen-grad-evidence.json"), &adv_frozen)?;

        let adv_vram = run_adversarial_vram_exceeded(&device).ok();  // graceful Option
        if let Some(ev) = adv_vram { write_evidence_file(&path("adversarial-vram-exceeded-evidence.json"), &ev)?; }

        // 8. Print + exit
        println!("forward_latency_ms_p50={:.3}", p50);
        println!("forward_latency_ms_p99={:.3}", p99);
        println!("vram_resident_bytes={}", vram.resident_bytes);
        println!("l_total={}", loss.l_total);
        println!("l_predict={}", loss.l_predict);
        println!("l_variance={}", loss.l_variance);
        println!("l_covariance={}", loss.l_covariance);
        println!("l_invariance={}", loss.l_invariance);
        println!("dod_pass={}", all_dod_pass && fwd_evidence.dod_pass);
        std::process::exit(if all_dod_pass && fwd_evidence.dod_pass { 0 } else { 2 })
    }

    fn run_adversarial_nan_input(predictor) -&gt; Result&lt;AdversarialEvidence&gt; {
        let mut t0 = randn_panel();
        // poke a NaN into element [0, 0]
        t0 = inject_nan_at(t0, 0, 0)?;
        let result = predictor.forward_dryrun(&t0, &randn_panel());
        let (passed, actual_code) = match result {
            Err(PredictorError::NanDetected{..}) =&gt; (true, "MEJEPA_PRED_NAN_DETECTED"),
            Err(e) =&gt; (false, e.code()),
            Ok(_) =&gt; (false, "OK_UNEXPECTED"),
        };
        Ok(AdversarialEvidence { case: "nan_input", expected_code: "MEJEPA_PRED_NAN_DETECTED", actual_code: actual_code.to_string(), passed })
    }
    // ... similar pattern for dim_mismatch, frozen_grad, vram_exceeded

tests:
    test_fsv_writer_creates_file_mode_0o600  (unix-only)
    test_fsv_writer_create_new_fails_on_leftover
    test_clear_fsv_dir_removes_only_json
    test_main_writes_all_expected_evidence_files  (cuda-gated; spawns the example via Command or refactors to lib fn)
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/fsv_writer.rs">FSV_DIR const + ensure_fsv_dir + clear_fsv_dir + write_evidence_file (mode 0o600 unix) + read_evidence_file + 3 unit tests</file>
  <file path="crates/context-graph-mejepa/examples/predictor_fsv.rs">FSV harness binary: nominal forward + VICReg + determinism + frozen-target + 4 adversarial cases; write+re-read+assert</file>
  <file path="crates/context-graph-mejepa/src/evidence_extra.rs">Optional: AdversarialEvidence + DeterminismEvidence + ForwardLatencies helper structs (Serialize); kept separate from `evidence.rs` to avoid bloating that file</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">`pub mod fsv_writer; pub mod evidence_extra;` and re-exports</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">Add `[[example]] name = "predictor_fsv"` entry; `tracing` workspace dep already added in TASK-PRED-010</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa --example predictor_fsv --features cuda succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 65 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>On RTX 5090: `cargo run --release -p context-graph-mejepa --example predictor_fsv --features cuda` exits 0 with dod_pass=true</criterion>
  <criterion>All 8 evidence files exist under /tmp/contextgraph-mejepa-predictor-fsv/ (mode 0o600 unix)</criterion>
  <criterion>forward-pass-evidence.json: forward_latency_ms_p50 &lt; 50.0; vram_resident_bytes &lt; 8 GB; output_finite == true</criterion>
  <criterion>vicreg-loss-evidence.json: formula_check == true; components_finite == true</criterion>
  <criterion>determinism-evidence.json: byte_equal == true (REQ-PRED-05)</criterion>
  <criterion>frozen-target-determinism-evidence.json: byte_equal == true (REQ-PRED-11)</criterion>
  <criterion>adversarial-nan-input-evidence.json: actual_code == "MEJEPA_PRED_NAN_DETECTED" AND passed == true</criterion>
  <criterion>adversarial-dim-mismatch-evidence.json: actual_code == "MEJEPA_PRED_DIM_MISMATCH" AND passed == true</criterion>
  <criterion>adversarial-frozen-grad-evidence.json: actual_code == "MEJEPA_PRED_FROZEN_TARGET_GRAD" AND passed == true</criterion>
  <criterion>adversarial-vram-exceeded-evidence.json: actual_code in {"MEJEPA_PRED_VRAM_EXCEEDED", "CANDLE_OOM_OR_MAPPED"} OR skipped == true</criterion>
  <criterion>FSV files are re-read by the harness itself before exit (write→re-read→assert protocol)</criterion>
  <criterion>File mode 0o600 verified (unix-only test)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa --example predictor_fsv</command>
  <command>cargo build --release -p context-graph-mejepa --example predictor_fsv --features cuda</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>CUBLAS_WORKSPACE_CONFIG=:4096:8 cargo run --release -p context-graph-mejepa --example predictor_fsv --features cuda</command>
  <command>ls -la /tmp/contextgraph-mejepa-predictor-fsv/</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
