use candle_core::{DType, Device, Tensor, Var};
use context_graph_mejepa_corpus::prng::SplitMix64;
use context_graph_mejepa_train::loss::sigreg::{
    sigreg_loss, sigreg_stats, SigregConfig, SigregStats,
};
use context_graph_mejepa_train::TrainerErrorCode;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-fp-101-sigreg-fsv";

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::from(0)
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let run_id = format!("run-{}", chrono::Utc::now().timestamp_millis());
    let run_dir = Path::new(FSV_ROOT).join(run_id);
    let output_path = run_dir.join("sigreg_fsv.json");
    let output_existed_before = output_path.exists();
    fs::create_dir_all(&run_dir)?;

    let device = Device::Cpu;
    let config = SigregConfig {
        num_slices: 48,
        num_points: 17,
        projection_seed: 0x3141_0101,
        ..SigregConfig::default()
    };

    let gaussian = gaussian_rows(192, 8, 0x5151);
    let collapsed = vec![0.0f32; gaussian.len()];
    let mut anisotropic = gaussian.clone();
    for row in anisotropic.chunks_mut(8) {
        row[0] *= 5.0;
        row[1] *= 0.1;
    }

    let before = json!({
        "fsvRoot": FSV_ROOT,
        "outputExistedBeforeWrite": output_existed_before
    });

    let gaussian_stats = stats(&gaussian, &device, config)?;
    let collapsed_stats = stats(&collapsed, &device, config)?;
    let anisotropic_stats = stats(&anisotropic, &device, config)?;

    let trajectory = [0.0f32, 0.25, 0.5, 0.75, 1.0]
        .into_iter()
        .map(|scale| {
            let values = gaussian
                .iter()
                .map(|value| value * scale)
                .collect::<Vec<_>>();
            stats(&values, &device, config).map(|stats| {
                json!({
                    "scale": scale,
                    "sigregLoss": stats.loss,
                    "informationContentProxy": scale
                })
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

    let small_batch = Tensor::zeros((1, 8), DType::F32, &device)?;
    let small_batch_error = sigreg_stats(&small_batch, config).expect_err("batch<2 must fail");
    let non_finite = Tensor::from_slice(&[0.0f32, f32::NAN, 1.0, 2.0], (2, 2), &device)?;
    let non_finite_error = sigreg_stats(&non_finite, config).expect_err("NaN must fail");

    let collapsed_rejected = collapsed_stats.loss > gaussian_stats.loss * 2.0;
    let anisotropic_rejected = anisotropic_stats.loss > gaussian_stats.loss;
    let monotonic = strictly_descends(
        &trajectory
            .iter()
            .map(|row| row["sigregLoss"].as_f64().unwrap() as f32)
            .collect::<Vec<_>>(),
    );
    let fail_closed = small_batch_error.code()
        == TrainerErrorCode::MejepaTrainConfigInvalid.as_screaming_snake()
        && non_finite_error.code() == TrainerErrorCode::MejepaTrainLossNan.as_screaming_snake();
    let gradient = gradient_case(&gaussian, &device, config)?;
    let gradient_flow_present = gradient["passes"].as_bool().unwrap_or(false);

    let evidence_without_hash = json!({
        "task": "TASK-FP-101",
        "sourceOfTruth": {
            "artifact": run_dir.join("sigreg_fsv.json").display().to_string(),
            "implementation": "crates/context-graph-mejepa-train/src/loss/sigreg.rs",
            "lossComposition": "crates/context-graph-mejepa-train/src/loss/mod.rs",
            "primaryReference": "https://arxiv.org/abs/2511.08544"
        },
        "before": before,
        "action": "compute SIGReg characteristic-function sketch scores for known synthetic embedding distributions",
        "config": config,
        "cases": [
            {
                "name": "collapsed_batch_scores_worse_than_gaussian_like_batch",
                "expected": "collapsed loss > 2x gaussian-like loss",
                "gaussianLike": gaussian_stats,
                "collapsed": collapsed_stats,
                "passes": collapsed_rejected
            },
            {
                "name": "anisotropic_batch_scores_worse_than_isotropic_batch",
                "expected": "anisotropic loss > isotropic gaussian-like loss",
                "anisotropic": anisotropic_stats,
                "isotropic": gaussian_stats,
                "passes": anisotropic_rejected
            },
            {
                "name": "monotonic_information_content_trajectory",
                "expected": "SIGReg loss strictly descends as scale approaches unit-variance gaussian-like embeddings",
                "trajectory": trajectory,
                "passes": monotonic
            },
            {
                "name": "invalid_inputs_fail_closed",
                "expected": "batch<2 and NaN inputs fail before producing score",
                "batchTooSmallError": small_batch_error.code(),
                "nonFiniteError": non_finite_error.code(),
                "passes": fail_closed
            },
            {
                "name": "sigreg_loss_backpropagates_to_predicted_latent",
                "expected": "training loss tensor has finite non-zero gradient at predicted latent",
                "gradient": gradient,
                "passes": gradient_flow_present
            }
        ],
        "allPassed": collapsed_rejected && anisotropic_rejected && monotonic && fail_closed && gradient_flow_present
    });
    assert_eq!(evidence_without_hash["allPassed"], true);

    let payload_sha256 = sha256_json(&evidence_without_hash)?;
    let mut evidence = evidence_without_hash;
    evidence["payloadSha256WithoutHashField"] = json!(payload_sha256);
    write_json_0600(&output_path, &evidence)?;

    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&output_path)?)?;
    assert_eq!(readback["allPassed"], true);
    assert_eq!(
        readback["payloadSha256WithoutHashField"],
        evidence["payloadSha256WithoutHashField"]
    );
    assert_eq!(readback["cases"].as_array().map(Vec::len), Some(5));
    Ok(output_path)
}

fn stats(
    values: &[f32],
    device: &Device,
    config: SigregConfig,
) -> Result<SigregStats, Box<dyn std::error::Error>> {
    let tensor = Tensor::from_slice(values, (values.len() / 8, 8), device)?;
    Ok(sigreg_stats(&tensor, config)?)
}

fn gradient_case(
    values: &[f32],
    device: &Device,
    config: SigregConfig,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let sample = values.iter().take(16 * 8).copied().collect::<Vec<_>>();
    let predicted = Var::from_slice(&sample, (16, 8), device)?;
    let loss = sigreg_loss(predicted.as_tensor(), config)?;
    let grads = loss.backward()?;
    let grad = grads
        .get(&predicted)
        .ok_or("missing SIGReg predicted-latent gradient")?;
    let grad_values = grad.flatten_all()?.to_vec1::<f32>()?;
    let grad_l1 = grad_values.iter().map(|value| value.abs()).sum::<f32>();
    let finite = grad_values.iter().all(|value| value.is_finite());
    Ok(json!({
        "loss": loss.to_scalar::<f32>()?,
        "gradShape": grad.dims(),
        "gradL1": grad_l1,
        "finite": finite,
        "passes": finite && grad_l1 > 0.0
    }))
}

fn gaussian_rows(batch: usize, dim: usize, seed: u64) -> Vec<f32> {
    let mut rng = SplitMix64::new(seed);
    let mut values = Vec::with_capacity(batch * dim);
    while values.len() < batch * dim {
        let (a, b) = standard_normal_pair(&mut rng);
        values.push(a);
        if values.len() < batch * dim {
            values.push(b);
        }
    }
    values
}

fn standard_normal_pair(rng: &mut SplitMix64) -> (f32, f32) {
    let u1 = rng.next_unit_f32().clamp(1e-7, 1.0);
    let u2 = rng.next_unit_f32();
    let radius = (-2.0 * u1.ln()).sqrt();
    let theta = std::f32::consts::TAU * u2;
    (radius * theta.cos(), radius * theta.sin())
}

fn strictly_descends(values: &[f32]) -> bool {
    values.windows(2).all(|pair| pair[1] < pair[0])
}

fn write_json_0600(
    path: &Path,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
    Ok(())
}

fn sha256_json(value: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
