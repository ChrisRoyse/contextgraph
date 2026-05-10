use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use candle_core::{DType, Device, Tensor};
use context_graph_mejepa::{
    enforce_vram_report, panel_source, run_gradient_hook, AdversarialEvidence, DeterminismEvidence,
    ForwardPassEvidence, FrozenTargetAdapter, GradStore, InstrumentGradHandle, MeJepaPredictor,
    NoOpGradHandle, PredictorConfig, TargetProvenance, TensorId, VicregLambdas, VicregLossEvidence,
    VramReport, PANEL_DIM, VRAM_STEADY_STATE_TARGET_BYTES,
};
use serde_json::json;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const WARMUP_CALLS: usize = 10;
const MEASURED_CALLS: usize = 100;
const BATCH_SIZE: usize = 64;

fn main() -> FsvResult<()> {
    let _ = tracing_subscriber::fmt::try_init();
    let panel_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(panel_source::DEFAULT_PHASE1_PANEL_PATH));
    let fsv_dir = context_graph_mejepa::fsv_writer::ensure_fsv_dir()?;
    context_graph_mejepa::fsv_writer::clear_fsv_dir()?;

    let panel = panel_source::read_phase1_panel(&panel_path)?;
    let device = Device::new_cuda(0)?;
    let provenance = TargetProvenance::new(
        "phase1-panel-fsv",
        BTreeMap::from([(
            "phase1-instruments".to_string(),
            "real-panel-json".to_string(),
        )]),
        0,
        Some(context_graph_mejepa_instruments::hash_f32s(panel.data())),
    );
    let adapter = FrozenTargetAdapter::new(provenance);
    let predictor = MeJepaPredictor::new(PredictorConfig::default(), adapter, device.clone(), 30)?;

    let panel_t0 = panel_source::panel_batch_tensor(
        &panel,
        BATCH_SIZE,
        panel_source::PanelBatchView::T0,
        &device,
        DType::BF16,
    )?;
    let panel_t1 = panel_source::panel_batch_tensor(
        &panel,
        BATCH_SIZE,
        panel_source::PanelBatchView::T1,
        &device,
        DType::BF16,
    )?;
    let panel_t2 = panel_source::panel_batch_tensor(
        &panel,
        BATCH_SIZE,
        panel_source::PanelBatchView::T2,
        &device,
        DType::BF16,
    )?;

    let forward = run_forward_evidence(&predictor, &panel_t0, &panel_t1, &panel_path, &fsv_dir)?;
    let loss = run_loss_evidence(
        &predictor,
        &panel_t0,
        &panel_t1,
        &panel_t2,
        &panel_path,
        &fsv_dir,
    )?;
    let determinism = run_determinism(&predictor, &panel_t0, &panel_t1, &fsv_dir)?;
    let frozen_target = run_frozen_target_determinism(&predictor, &panel, &fsv_dir)?;
    let adversarial = vec![
        run_nan_case(&predictor, &panel_t0, &panel_t1, &device, &fsv_dir)?,
        run_dim_case(&predictor, &panel_t1, &device, &fsv_dir)?,
        run_frozen_grad_case(&fsv_dir)?,
        run_vram_case(&fsv_dir)?,
    ];

    let aggregate = json!({
        "source_of_truth": {
            "phase1_panel": panel_path.display().to_string(),
            "fsv_dir": fsv_dir.display().to_string()
        },
        "after_state": {
            "panel_first_8": panel_source::panel_first_values(&panel, 8),
            "forward": forward,
            "loss": loss,
            "determinism": determinism,
            "frozen_target": frozen_target,
            "adversarial": adversarial
        },
        "passes": forward.dod_pass
            && loss.dod_pass
            && determinism.passes
            && frozen_target.passes
            && adversarial.iter().all(|case| case.passes)
    });
    let aggregate_path = fsv_dir.join("aggregate-evidence.json");
    context_graph_mejepa::fsv_writer::write_evidence_file(&aggregate_path, &aggregate)?;
    let readback: serde_json::Value =
        context_graph_mejepa::fsv_writer::read_evidence_file(&aggregate_path)?;
    require(
        "aggregate evidence readback",
        readback.get("passes") == Some(&serde_json::Value::Bool(true)),
    )?;

    println!("forward_latency_ms_p50={}", forward.forward_latency_ms_p50);
    println!("forward_latency_ms_p99={}", forward.forward_latency_ms_p99);
    println!("vram_resident_bytes={}", forward.vram_resident_bytes);
    println!("l_total={}", loss.outputs.l_total);
    println!("dod_pass=true");
    Ok(())
}

fn run_forward_evidence(
    predictor: &MeJepaPredictor,
    panel_t0: &Tensor,
    panel_t1: &Tensor,
    panel_path: &Path,
    fsv_dir: &Path,
) -> FsvResult<ForwardPassEvidence> {
    let dryrun = predictor.forward_dryrun(panel_t0, panel_t1)?;
    let output_sha256 = panel_source::tensor_sha256_f32(&dryrun.tensor)?;
    let mut latencies = Vec::with_capacity(MEASURED_CALLS);
    for _ in 0..WARMUP_CALLS {
        let _ = predictor.forward(panel_t0, panel_t1)?;
        predictor.device().synchronize()?;
    }
    for _ in 0..MEASURED_CALLS {
        let started = Instant::now();
        let _ = predictor.forward(panel_t0, panel_t1)?;
        predictor.device().synchronize()?;
        latencies.push(started.elapsed().as_secs_f64() as f32 * 1_000.0);
    }
    latencies.sort_by(|a, b| a.total_cmp(b));
    let vram_resident_bytes = context_graph_mejepa::vram_resident_bytes(predictor.device())?;
    let evidence = ForwardPassEvidence {
        source_of_truth: fsv_dir
            .join("forward-pass-evidence.json")
            .display()
            .to_string(),
        input_panel_path: panel_path.display().to_string(),
        panel_dim: PANEL_DIM,
        batch_size: BATCH_SIZE,
        warmup_calls: WARMUP_CALLS,
        measured_calls: MEASURED_CALLS,
        forward_latency_ms_p50: latencies[MEASURED_CALLS / 2],
        forward_latency_ms_p99: latencies[MEASURED_CALLS - 2],
        output_sha256_f32: output_sha256,
        output_finite: true,
        output_dtype: dryrun.dtype,
        vram_resident_bytes,
        dod_pass: latencies[MEASURED_CALLS / 2] < 50.0
            && vram_resident_bytes < VRAM_STEADY_STATE_TARGET_BYTES,
    };
    let path = fsv_dir.join("forward-pass-evidence.json");
    let readback = context_graph_mejepa::fsv_writer::write_readback_assert(&path, &evidence)?;
    require("forward DoD", readback.dod_pass)?;
    Ok(readback)
}

fn run_loss_evidence(
    predictor: &MeJepaPredictor,
    panel_t0: &Tensor,
    panel_t1: &Tensor,
    panel_t2: &Tensor,
    panel_path: &Path,
    fsv_dir: &Path,
) -> FsvResult<VicregLossEvidence> {
    let predicted = predictor.forward_dryrun(panel_t0, panel_t1)?;
    let lambdas = VicregLambdas::default();
    let outputs = context_graph_mejepa::vicreg_loss(&predicted.tensor, panel_t2, lambdas)?;
    let evidence = VicregLossEvidence {
        source_of_truth: fsv_dir
            .join("vicreg-loss-evidence.json")
            .display()
            .to_string(),
        input_panel_path: panel_path.display().to_string(),
        lambdas,
        finite: outputs.finite(),
        total_in_dod_band: outputs.l_total >= 10.0 && outputs.l_total <= 1_000.0,
        dod_pass: outputs.finite()
            && outputs.formula_check
            && outputs.l_total >= 10.0
            && outputs.l_total <= 1_000.0,
        outputs,
    };
    let path = fsv_dir.join("vicreg-loss-evidence.json");
    let readback = context_graph_mejepa::fsv_writer::write_readback_assert(&path, &evidence)?;
    require("loss DoD", readback.dod_pass)?;
    Ok(readback)
}

fn run_determinism(
    predictor: &MeJepaPredictor,
    panel_t0: &Tensor,
    panel_t1: &Tensor,
    fsv_dir: &Path,
) -> FsvResult<DeterminismEvidence> {
    let a = predictor.forward_dryrun(panel_t0, panel_t1)?;
    let b = predictor.forward_dryrun(panel_t0, panel_t1)?;
    let sha_a = panel_source::tensor_sha256_f32(&a.tensor)?;
    let sha_b = panel_source::tensor_sha256_f32(&b.tensor)?;
    let evidence = DeterminismEvidence {
        source_of_truth: fsv_dir
            .join("determinism-evidence.json")
            .display()
            .to_string(),
        sha256_a: sha_a.clone(),
        sha256_b: sha_b.clone(),
        byte_equal: sha_a == sha_b,
        passes: sha_a == sha_b,
    };
    let path = fsv_dir.join("determinism-evidence.json");
    Ok(context_graph_mejepa::fsv_writer::write_readback_assert(
        &path, &evidence,
    )?)
}

fn run_frozen_target_determinism(
    predictor: &MeJepaPredictor,
    panel: &context_graph_mejepa_instruments::Panel,
    fsv_dir: &Path,
) -> FsvResult<DeterminismEvidence> {
    let target_a = predictor.frozen_target_adapter().encode_target(
        panel,
        predictor.device(),
        predictor.dtype(),
    )?;
    let target_b = predictor.frozen_target_adapter().encode_target(
        panel,
        predictor.device(),
        predictor.dtype(),
    )?;
    let sha_a = panel_source::tensor_sha256_f32(&target_a.tensor)?;
    let sha_b = panel_source::tensor_sha256_f32(&target_b.tensor)?;
    let evidence = DeterminismEvidence {
        source_of_truth: fsv_dir
            .join("frozen-target-determinism-evidence.json")
            .display()
            .to_string(),
        sha256_a: sha_a.clone(),
        sha256_b: sha_b.clone(),
        byte_equal: sha_a == sha_b,
        passes: sha_a == sha_b,
    };
    let path = fsv_dir.join("frozen-target-determinism-evidence.json");
    Ok(context_graph_mejepa::fsv_writer::write_readback_assert(
        &path, &evidence,
    )?)
}

fn run_nan_case(
    predictor: &MeJepaPredictor,
    panel_t0: &Tensor,
    panel_t1: &Tensor,
    device: &Device,
    fsv_dir: &Path,
) -> FsvResult<AdversarialEvidence> {
    let mut values = panel_t0
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    values[0] = f32::NAN;
    let nan_t0 =
        Tensor::from_slice(&values, (BATCH_SIZE, PANEL_DIM), device)?.to_dtype(DType::BF16)?;
    let actual = predictor
        .forward_dryrun(&nan_t0, panel_t1)
        .err()
        .map(|err| err.code().to_string())
        .unwrap_or_else(|| "NO_ERROR".to_string());
    write_adversarial(
        fsv_dir,
        "adversarial-nan-input-evidence.json",
        "nan_input",
        json!({"panel_t0_first_value": "NaN"}),
        json!({"actual_error_code": actual}),
        "MEJEPA_PRED_NAN_DETECTED",
        &actual,
    )
}

fn run_dim_case(
    predictor: &MeJepaPredictor,
    panel_t1: &Tensor,
    device: &Device,
    fsv_dir: &Path,
) -> FsvResult<AdversarialEvidence> {
    let wrong = Tensor::zeros((BATCH_SIZE, PANEL_DIM - 1), DType::BF16, device)?;
    let actual = predictor
        .forward_dryrun(&wrong, panel_t1)
        .err()
        .map(|err| err.code().to_string())
        .unwrap_or_else(|| "NO_ERROR".to_string());
    write_adversarial(
        fsv_dir,
        "adversarial-dim-mismatch-evidence.json",
        "dim_mismatch",
        json!({"panel_t0_shape": [BATCH_SIZE, PANEL_DIM - 1], "panel_t1_shape": panel_t1.dims()}),
        json!({"actual_error_code": actual}),
        "MEJEPA_PRED_DIM_MISMATCH",
        &actual,
    )
}

fn run_frozen_grad_case(fsv_dir: &Path) -> FsvResult<AdversarialEvidence> {
    struct SyntheticGradHandle {
        ids: Vec<TensorId>,
    }
    impl InstrumentGradHandle for SyntheticGradHandle {
        fn instrument_id(&self) -> &str {
            "synthetic_frozen_target"
        }
        fn tensor_ids(&self) -> &[TensorId] {
            &self.ids
        }
    }
    let adapter = FrozenTargetAdapter::with_grad_handles(
        TargetProvenance::new("adversarial", BTreeMap::new(), 0, None),
        vec![
            Box::new(NoOpGradHandle::with_label("clean")),
            Box::new(SyntheticGradHandle {
                ids: vec![TensorId("frozen.synthetic.weight".to_string())],
            }),
        ],
    );
    let mut grads = GradStore::default();
    grads.insert_norm(TensorId("frozen.synthetic.weight".to_string()), 1.0)?;
    let actual = run_gradient_hook(&adapter, &grads)
        .err()
        .map(|err| err.code().to_string())
        .unwrap_or_else(|| "NO_ERROR".to_string());
    write_adversarial(
        fsv_dir,
        "adversarial-frozen-grad-evidence.json",
        "frozen_grad",
        json!({"grad_norm": 1.0, "threshold": context_graph_mejepa::GRAD_NORM_NOISE_FLOOR}),
        json!({"actual_error_code": actual}),
        "MEJEPA_PRED_FROZEN_TARGET_GRAD",
        &actual,
    )
}

fn run_vram_case(fsv_dir: &Path) -> FsvResult<AdversarialEvidence> {
    let gib = 1024 * 1024 * 1024;
    let report = VramReport::from_measurement(0, 10 * gib, "synthetic-over-budget".to_string());
    let actual = enforce_vram_report(report)
        .err()
        .map(|err| err.code().to_string())
        .unwrap_or_else(|| "NO_ERROR".to_string());
    write_adversarial(
        fsv_dir,
        "adversarial-vram-exceeded-evidence.json",
        "vram_exceeded",
        json!({"resident_bytes": 10 * gib, "threshold_bytes": 9 * gib}),
        json!({"actual_error_code": actual}),
        "MEJEPA_PRED_VRAM_EXCEEDED",
        &actual,
    )
}

fn write_adversarial(
    fsv_dir: &Path,
    file_name: &str,
    case_name: &str,
    before_state: serde_json::Value,
    after_state: serde_json::Value,
    expected: &str,
    actual: &str,
) -> FsvResult<AdversarialEvidence> {
    let evidence = AdversarialEvidence {
        case_name: case_name.to_string(),
        before_state,
        after_state,
        expected_error_code: expected.to_string(),
        actual_error_code: actual.to_string(),
        passes: actual == expected,
    };
    let path = fsv_dir.join(file_name);
    let readback = context_graph_mejepa::fsv_writer::write_readback_assert(&path, &evidence)?;
    require(case_name, readback.passes)?;
    Ok(readback)
}

fn require(label: &str, passes: bool) -> FsvResult<()> {
    if passes {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {label}").into())
    }
}
