//! Top-level Phase 3 FSV: smoke + chain + holdout + UTML cross-check.

use context_graph_mejepa_train::learning_signal::{
    compute_l_step, DeltaKComponents, DeltaOmegaComponents, DeltaPComponents, DeltaXiComponents,
};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const FSV_DIR: &str = "/tmp/contextgraph-mejepa-train-fsv";

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
        assert!(value.is_object(), "{file} must contain JSON object evidence");
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
    }
    let summary = json!({
        "phase3_fsv_passes": true,
        "evidence_files": files,
        "source_of_truths": {
            "smoke": "/tmp/contextgraph-mejepa-train-fsv/smoke-rocksdb",
            "chain": "/tmp/contextgraph-mejepa-train-fsv/chain-rocksdb",
            "holdout": "/tmp/contextgraph-mejepa-train-fsv/holdout-rocksdb"
        }
    });
    write_0600(Path::new(FSV_DIR).join("training-fsv-summary.json"), &summary)?;
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
            oracle_disagreement: 0.8,
            prediction_residual: 0.4,
            uncertainty: 0.2,
        },
        DeltaKComponents {
            operator_override: 1.0,
            novel_failure_mode: 0.3,
            sparse_slice_bonus: 0.2,
        },
        DeltaOmegaComponents {
            runtime_delta: 0.1,
            warning_delta: 0.2,
            memory_delta: 0.3,
        },
        DeltaXiComponents {
            target_collapse: 0.0,
            predictor_collapse: 0.05,
            redundancy_mi: 0.05,
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
    write_0600(Path::new(FSV_DIR).join("utml-l-step-evidence.json"), &evidence)
}

fn write_0600(path: impl AsRef<Path>, value: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
    let reread: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    assert_eq!(&reread, value);
    Ok(())
}
