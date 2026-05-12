//! Top-level Phase 3 FSV: smoke + chain + holdout + UTML cross-check.

use context_graph_mejepa_train::learning_signal::{
    compute_l_step, DeltaKComponents, DeltaOmegaComponents, DeltaPAggregator, DeltaPComponents,
    DeltaXiComponents,
};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const FSV_DIR: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::from(0),
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(FSV_DIR)?;
    write_utml_evidence()?;
    run_example("train_smoke_fsv")?;
    run_example("train_cert_chain_fsv")?;
    run_example("train_holdout_eval_fsv")?;
    let files = [
        "training-smoke-evidence.json",
        "training-cert-chain.json",
        "holdout-eval-evidence.json",
        "utml-l-step-evidence.json",
    ];
    for file in files {
        let path = Path::new(FSV_DIR).join(file);
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
        assert!(
            value.is_object(),
            "{file} must contain JSON object evidence"
        );
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
    }
    let summary = json!({
        "phase3_fsv_passes": true,
        "evidence_files": files,
        "source_of_truths": {
            "smoke": "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv/smoke-rocksdb",
            "chain": "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv/chain-rocksdb",
            "holdout": "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv/holdout-rocksdb"
        }
    });
    write_0600(
        Path::new(FSV_DIR).join("training-fsv-summary.json"),
        &summary,
    )?;
    Ok(())
}

fn run_example(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let exe = sibling_example_path(name)?;
    let status = Command::new(&exe).status()?;
    if !status.success() {
        return Err(format!("example {} failed with status {:?}", exe.display(), status).into());
    }
    Ok(())
}

fn sibling_example_path(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut exe = std::env::current_exe()?;
    exe.set_file_name(name);
    Ok(exe)
}

fn write_utml_evidence() -> Result<(), Box<dyn std::error::Error>> {
    let signal = compute_l_step(
        DeltaPComponents {
            delta_p_real: 0.6,
            delta_p_imagined: Some(0.5),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.4, 0.8],
        },
        DeltaKComponents {
            cos_align: 0.9,
            fisher_violation: 0.1,
            ece: 0.1,
            embedder_coherence: 0.7,
            embedder_coherence_source: "computed".to_string(),
        },
        DeltaOmegaComponents {
            effective_plasticity: 0.9,
            landscape_health: 0.85,
            stability_floor: 1.0,
            agent_state_score: 0.7,
            agent_state_source: "default_neutral_no_transcript".to_string(),
        },
        DeltaXiComponents {
            target_collapse: 0.0,
            predictor_redundancy: 0.05,
            constellation_violation_rate: 0.05,
        },
    )?;
    let evidence = json!({
        "source_of_truth": "computed deterministic UTML LearningSignal persisted in this evidence file",
        "delta_p": signal.delta_p,
        "delta_k": signal.delta_k,
        "delta_omega": signal.delta_omega,
        "delta_xi": signal.delta_xi,
        "l_step": signal.l_step,
        "target_collapse": signal.delta_xi_components.target_collapse,
        "passes": signal.delta_xi_components.target_collapse == 0.0
            && (0.0..=1.0).contains(&signal.l_step)
    });
    write_0600(
        Path::new(FSV_DIR).join("utml-l-step-evidence.json"),
        &evidence,
    )
}

fn write_0600(
    path: impl AsRef<Path>,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
    let readback = fs::read(path)?;
    assert_eq!(readback, bytes);
    let reread: serde_json::Value = serde_json::from_slice(&readback)?;
    assert!(reread.is_object());
    Ok(())
}
