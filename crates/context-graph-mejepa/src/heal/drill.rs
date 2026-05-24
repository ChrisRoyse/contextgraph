use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use context_graph_mejepa_instruments::{Panel, PANEL_DIM};
use serde::{Deserialize, Serialize};

use crate::heal::drift::DriftSeverity;
use crate::heal::errors::HealError;
use crate::heal::ewc::{HessianTraceProbe, PredictorGradAccessor};
use crate::heal::integrity::{integrity_evidence_from_chain, verify, ChainIntegrityChecker};
use crate::heal::lora_refresh::{refresh, LoraRefreshEvidence};
use crate::heal::pipeline::{bootstrap_pipeline_for_path, force_lora_corpus_slice, ObserveOutput};
use crate::heal::promote::{ModeWinner, TriggerReason};
use crate::heal::readback::{
    write_readback_evidence_canonical, DriftDrillEvidence, READBACK_FILES, READBACK_ROOT,
};
use crate::types::OracleOutcome;

#[derive(ValueEnum, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InjectDrift {
    Soft,
    Hard,
    Catastrophic,
}

impl InjectDrift {
    pub fn target_coverage(self) -> f32 {
        match self {
            Self::Soft => 0.86,
            Self::Hard => 0.82,
            Self::Catastrophic => 0.75,
        }
    }

    pub fn trigger_reason(self) -> TriggerReason {
        match self {
            Self::Soft => TriggerReason::OperatorTriggered,
            Self::Hard => TriggerReason::DriftHard,
            Self::Catastrophic => TriggerReason::DriftCatastrophic,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HealDrillArgs {
    pub inject_drift: InjectDrift,
    pub output_readback: PathBuf,
    pub max_observations: u64,
    pub seed: u64,
    pub rtx_5090_budget_min: u64,
}

impl Default for HealDrillArgs {
    fn default() -> Self {
        Self {
            inject_drift: InjectDrift::Hard,
            output_readback: PathBuf::from(READBACK_ROOT),
            max_observations: 5000,
            seed: 42,
            rtx_5090_budget_min: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DrillStdoutSummary {
    pub drift_detected: DriftSeverity,
    pub observations_to_detect: u64,
    /// #690: derived from real evidence, not hardcoded.
    /// `true` iff (`weights_sha_winner != [0u8; 32]`) AND
    /// (`evaluation_summary_sha != [0u8; 32]`) AND
    /// (`mode_winner ∈ {B, C}`). Mode A is a no-promotion outcome and must
    /// not be reported as a successful retrain.
    pub retrain_triggered: bool,
    pub mode_winner: char,
    pub promotion_latency_seconds: u64,
    pub lora_refresh_succeeded: bool,
    pub integrity_violation_detected: bool,
    pub fisher_rank_after_10_boundaries: usize,
    pub fisher_full_rank: bool,
    pub readback_files_emitted: Vec<String>,
    pub all_readback_files_mode_0o600: bool,
    pub exit_code: i32,
    /// #690: AND-folds every boundary check that drives `exit_code` so an
    /// operator dashboard can rely on a single boolean to know whether
    /// Phase-5 DoD was satisfied end-to-end. `exit_code == 0` is the
    /// authoritative signal; this field is its serializable AND-fold.
    pub phase_5_dod_completed: bool,
}

pub fn run_heal_drill(args: HealDrillArgs) -> Result<DrillStdoutSummary, HealError> {
    if args.max_observations < 700 {
        return Err(HealError::invalid(
            "heal_drill.max_observations",
            "max_observations must be >= 700",
        ));
    }
    std::fs::create_dir_all(&args.output_readback)
        .map_err(|err| HealError::io("create_dir_all", &args.output_readback, err))?;
    let state_root = args.output_readback.join("state");
    if state_root.exists() {
        std::fs::remove_dir_all(&state_root)
            .map_err(|err| HealError::io("remove_dir_all", &state_root, err))?;
    }
    std::fs::create_dir_all(&state_root)
        .map_err(|err| HealError::io("create_dir_all", &state_root, err))?;
    let mut pipeline = bootstrap_pipeline_for_path(&state_root)?;
    pipeline.config.skip_below_signal_clarity = false;
    pipeline.drift_detector.hysteresis_windows = 1;
    pipeline.drift_detector.min_detection_samples = 700;

    let mut observations_to_detect = 0u64;
    let mut drift_detected = DriftSeverity::WarmupNotReady;
    let mut severity_history = Vec::new();
    let mut empirical_coverage_trajectory = Vec::new();
    for i in 0..args.max_observations {
        let panel = synthetic_panel(args.seed.wrapping_add(i), i)?;
        let actual = deterministic_oracle(args.inject_drift.target_coverage(), args.seed, i);
        match pipeline.observe(&panel, &actual, 0.9, i, "heal-drill") {
            Ok(ObserveOutput::Stepped { .. }) | Ok(ObserveOutput::Skipped { .. }) => {
                let severity = pipeline.drift_detector.last_severity;
                severity_history.push((i, severity));
                if let Some(cov) = pipeline.drift_detector.last_empirical_coverage {
                    empirical_coverage_trajectory.push(cov);
                }
                if matches_requested(severity, args.inject_drift) {
                    observations_to_detect = i + 1;
                    drift_detected = severity;
                    break;
                }
            }
            Err(HealError::DriftDetected { severity, .. }) => {
                severity_history.push((i, severity));
                if let Some(cov) = pipeline.drift_detector.last_empirical_coverage {
                    empirical_coverage_trajectory.push(cov);
                }
                observations_to_detect = i + 1;
                drift_detected = severity;
                break;
            }
            Err(err) => return Err(err),
        }
    }
    let hit_max_without_detection = observations_to_detect == 0;
    if hit_max_without_detection {
        observations_to_detect = args.max_observations;
    }
    let drift_evidence = DriftDrillEvidence {
        inject_drift: format!("{:?}", args.inject_drift).to_lowercase(),
        observations_to_detect,
        severity_history,
        empirical_coverage_trajectory,
        max_observations: args.max_observations,
        hit_max_without_detection,
    };
    write_readback_evidence_canonical(
        "drift-drill-evidence.json",
        &drift_evidence,
        Some(&args.output_readback),
    )?;

    let report = if args.inject_drift == InjectDrift::Soft {
        pipeline.trigger_abc_for_current_drift(TriggerReason::OperatorTriggered)?
    } else {
        pipeline.trigger_abc_for_current_drift(args.inject_drift.trigger_reason())?
    };
    let abc_evidence = pipeline
        .abc_promoter
        .readback_snapshot()
        .ok_or_else(|| HealError::invalid("heal_drill.abc", "missing A/B/C readback evidence"))?;
    write_readback_evidence_canonical(
        "abc-promotion-evidence.json",
        &abc_evidence,
        Some(&args.output_readback),
    )?;

    let corpus = force_lora_corpus_slice(args.seed.wrapping_add(1), 64)?;
    let lora_report = refresh(
        &mut pipeline.lora_refresher,
        7,
        &corpus,
        pipeline.storage.clone(),
    )?;
    let lora_evidence = LoraRefreshEvidence::from(&lora_report);
    write_readback_evidence_canonical(
        "lora-refresh-evidence.json",
        &lora_evidence,
        Some(&args.output_readback),
    )?;

    let mut boundary_step_trajectory = Vec::new();
    let mut lambda_trajectory = Vec::new();
    let mut fisher_rank_per_boundary = Vec::new();
    for idx in 0..10 {
        let panel = synthetic_panel(args.seed.wrapping_add(1000 + idx), idx)?;
        pipeline
            .ewc
            .update_fisher_online(&panel, &OracleOutcome::Pass, &pipeline.predictor)?;
        let loss = 100.0 + idx as f32 * 20.0 + pipeline.predictor.hessian_trace_estimate()?;
        let _ = pipeline
            .ewc
            .detect_task_boundary(loss, &pipeline.predictor)?;
        let snapshot = pipeline.ewc.snapshot_current_task(
            &pipeline.predictor.parameters_flat(),
            [idx as u8 + 1; 32],
            pipeline.storage.as_ref(),
        )?;
        boundary_step_trajectory.push(snapshot.boundary_step);
        lambda_trajectory.push(pipeline.ewc.current_lambda());
        fisher_rank_per_boundary.push(pipeline.ewc.fisher_rank());
    }
    let fisher_full_rank = pipeline.ewc.fisher_rank() == pipeline.ewc.predictor_dim;
    let ewc_evidence = crate::heal::ewc::EwcReadbackEvidence {
        boundary_step_trajectory,
        lambda_trajectory,
        fisher_rank_per_boundary,
        fisher_full_rank_after_boundary_10: fisher_full_rank,
        task_snapshots_count: pipeline.ewc.task_snapshots.len(),
        predictor_dim: pipeline.ewc.predictor_dim,
    };
    write_readback_evidence_canonical(
        "ewc-fisher-evidence.json",
        &ewc_evidence,
        Some(&args.output_readback),
    )?;

    pipeline.witness_chain.append_model_event(
        "ModelRollback",
        report.weights_sha_winner,
        report.evaluation_summary_sha,
    )?;
    let corruption_offset = context_graph_witness::WITNESS_ENTRY_SIZE;
    write_garbage_at_offset(&pipeline.witness_chain.chain_path, corruption_offset, 1)?;
    let mut checker = ChainIntegrityChecker::try_new(pipeline.witness_chain.chain_path.clone())?;
    let integrity_result = verify(
        &mut checker,
        &pipeline.witness_chain,
        pipeline.storage.as_ref(),
        &pipeline.status,
    );
    let integrity_violation_detected = match integrity_result {
        Ok(_) => false,
        Err(HealError::IntegrityViolation { .. } | HealError::WitnessQuarantined { .. }) => true,
        Err(err) => return Err(err),
    };
    let integrity_evidence = integrity_evidence_from_chain(
        &checker,
        pipeline.status.lock().unwrap().status_change,
        Some(1),
        false,
        0,
    )?;
    write_readback_evidence_canonical(
        "witness-integrity-evidence.json",
        &integrity_evidence,
        Some(&args.output_readback),
    )?;
    set_readback_files_mode_0o600(&args.output_readback)?;

    let readback_files_emitted = READBACK_FILES
        .iter()
        .map(|name| args.output_readback.join(name).display().to_string())
        .collect::<Vec<_>>();
    let all_modes = all_readback_files_mode_0o600(&args.output_readback)?;
    let mut exit_code = 0;
    if args.inject_drift == InjectDrift::Hard && !(400..=1000).contains(&observations_to_detect) {
        exit_code = 1;
    }
    if !matches!(abc_evidence.mode_winner, ModeWinner::B | ModeWinner::C) {
        exit_code = 1;
    }
    if !lora_evidence.frozen_base_byte_equal || lora_evidence.post_plasticity <= 0.4 {
        exit_code = 1;
    }
    if !integrity_violation_detected || !fisher_full_rank || !all_modes {
        exit_code = 1;
    }
    if std::env::var("MEJEPA_RTX_5090").ok().as_deref() == Some("1")
        && report.promotion_latency_seconds >= args.rtx_5090_budget_min * 60
    {
        exit_code = 1;
    }
    if drift_detected == DriftSeverity::WarmupNotReady {
        drift_detected = pipeline.drift_detector.last_severity;
    }
    let retrain_triggered = derive_retrain_triggered(
        report.weights_sha_winner,
        report.evaluation_summary_sha,
        abc_evidence.mode_winner,
    );
    let lora_refresh_succeeded = lora_evidence.post_plasticity > 0.4;
    let phase_5_dod_completed = exit_code == 0;
    Ok(DrillStdoutSummary {
        drift_detected,
        observations_to_detect,
        retrain_triggered,
        mode_winner: abc_evidence.mode_winner.as_char(),
        promotion_latency_seconds: report.promotion_latency_seconds,
        lora_refresh_succeeded,
        integrity_violation_detected,
        fisher_rank_after_10_boundaries: pipeline.ewc.fisher_rank(),
        fisher_full_rank,
        readback_files_emitted,
        all_readback_files_mode_0o600: all_modes,
        exit_code,
        phase_5_dod_completed,
    })
}

pub fn synthetic_panel(seed: u64, witness_chain_offset: u64) -> Result<Panel, HealError> {
    let mut state = seed ^ witness_chain_offset.rotate_left(17);
    let mut data = Vec::with_capacity(PANEL_DIM);
    for _ in 0..PANEL_DIM {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let v = ((state >> 40) as f32 / (u32::MAX >> 8) as f32).clamp(0.0, 1.0);
        data.push(0.05 + v * 0.10);
    }
    Panel::try_new(data, (1u16 << 15) - 1).map_err(|err| {
        HealError::invalid(
            "heal_drill.synthetic_panel",
            format!("panel invariant failed: {err}"),
        )
    })
}

pub fn deterministic_oracle(target_coverage: f32, seed: u64, idx: u64) -> OracleOutcome {
    let bucket = ((idx.wrapping_mul(1103515245).wrapping_add(seed) % 1000) as f32) / 1000.0;
    if bucket < target_coverage {
        OracleOutcome::Pass
    } else {
        OracleOutcome::Fail
    }
}

pub fn write_garbage_at_offset(
    test_chain_path: &Path,
    offset: usize,
    n_bytes: usize,
) -> Result<(), HealError> {
    if !test_chain_path.starts_with(std::env::temp_dir()) {
        return Err(HealError::invalid(
            "heal_drill.chain_path",
            "refusing to corrupt a chain outside tempdir",
        ));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .open(test_chain_path)
        .map_err(|err| HealError::io("open", test_chain_path, err))?;
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(|err| HealError::io("seek", test_chain_path, err))?;
    file.write_all(&vec![0xA5; n_bytes])
        .map_err(|err| HealError::io("write", test_chain_path, err))?;
    file.sync_all()
        .map_err(|err| HealError::io("sync", test_chain_path, err))?;
    Ok(())
}

fn matches_requested(severity: DriftSeverity, inject: InjectDrift) -> bool {
    matches!(
        (inject, severity),
        (InjectDrift::Soft, DriftSeverity::Soft)
            | (InjectDrift::Hard, DriftSeverity::Hard)
            | (InjectDrift::Catastrophic, DriftSeverity::Catastrophic)
    )
}

fn set_readback_files_mode_0o600(root: &Path) -> Result<(), HealError> {
    for name in READBACK_FILES {
        let path = root.join(name);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|err| HealError::io("chmod", &path, err))?;
    }
    Ok(())
}

fn all_readback_files_mode_0o600(root: &Path) -> Result<bool, HealError> {
    for name in READBACK_FILES {
        let path = root.join(name);
        let mode = std::fs::metadata(&path)
            .map_err(|err| HealError::io("metadata", &path, err))?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o600 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// #690: derive `retrain_triggered` from real evidence rather than a hardcoded
/// `true`. A true retrain requires (a) winner weights digest is non-zero,
/// (b) evaluation summary digest is non-zero, AND (c) the ABC bandit
/// promoted a non-control mode (B or C). Mode A is "no promotion" and must
/// not be reported as a successful retrain.
fn derive_retrain_triggered(
    weights_sha_winner: [u8; 32],
    evaluation_summary_sha: [u8; 32],
    mode_winner: ModeWinner,
) -> bool {
    let weights_sha_present = weights_sha_winner != [0u8; 32];
    let evaluation_summary_sha_present = evaluation_summary_sha != [0u8; 32];
    let mode_winner_promoted = matches!(mode_winner, ModeWinner::B | ModeWinner::C);
    weights_sha_present && evaluation_summary_sha_present && mode_winner_promoted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonzero_digest() -> [u8; 32] {
        let mut d = [0u8; 32];
        d[0] = 1;
        d
    }

    #[test]
    fn derive_retrain_triggered_false_for_mode_a_no_op() {
        // #690 regression: mode A is "no promotion" — even with non-zero
        // weights and evaluation digests, the drill must NOT report a
        // successful retrain.
        assert!(!derive_retrain_triggered(
            nonzero_digest(),
            nonzero_digest(),
            ModeWinner::A
        ));
    }

    #[test]
    fn derive_retrain_triggered_false_for_zero_weights_sha() {
        assert!(!derive_retrain_triggered(
            [0u8; 32],
            nonzero_digest(),
            ModeWinner::B
        ));
    }

    #[test]
    fn derive_retrain_triggered_false_for_zero_evaluation_summary_sha() {
        assert!(!derive_retrain_triggered(
            nonzero_digest(),
            [0u8; 32],
            ModeWinner::C
        ));
    }

    #[test]
    fn derive_retrain_triggered_true_only_when_all_three_signals_agree() {
        assert!(derive_retrain_triggered(
            nonzero_digest(),
            nonzero_digest(),
            ModeWinner::B
        ));
        assert!(derive_retrain_triggered(
            nonzero_digest(),
            nonzero_digest(),
            ModeWinner::C
        ));
    }

    #[test]
    fn synthetic_oracle_produces_target_coverage_with_tolerance() {
        let covered = (0..1000)
            .filter(|i| deterministic_oracle(0.82, 42, *i) == OracleOutcome::Pass)
            .count() as f32
            / 1000.0;
        assert!((covered - 0.82).abs() <= 0.02);
    }

    #[test]
    fn write_garbage_at_offset_rejects_non_tempdir_path() {
        assert!(write_garbage_at_offset(Path::new("/home/not-temp-chain"), 0, 1).is_err());
    }
}
