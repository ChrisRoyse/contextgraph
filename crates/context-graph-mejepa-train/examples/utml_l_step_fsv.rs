use context_graph_mejepa::HeadId;
use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PanelBuilder};
use context_graph_mejepa_train::cert::{
    open_train_rocksdb, verify_chain, TrainCertWriter, TrainingCertificate, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_train::learning_signal::{
    calibrated_confidence, classify_utml_state, compute_embedder_coherence, compute_l_step,
    compute_state_transfer, detect_dormant_units, prune_decision, reinit_dormant_units, reward,
    sampling_weight_checked, update_m_t, CalibratedConfidenceComponents, ContentEmbedder,
    DeltaKComponents, DeltaOmegaComponents, DeltaPAggregator, DeltaPComponents, DeltaXiComponents,
    DivergenceMetric, DormantLayer, DormantUnitDetector, LStepWindow, LearningSignal,
    PairwiseMiAuditor, RewardComponents, SamplingWeightComponents,
};
use rocksdb::{IteratorMode, DB};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const OUT: &str = "/zfs/archive/contextgraph/fsv/contextgraph-utml-fsv";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(OUT);
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    fs::create_dir_all(&out)?;

    let signal = compute_l_step(
        DeltaPComponents {
            delta_p_real: 0.7,
            delta_p_imagined: Some(0.6),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.6, 0.8],
        },
        DeltaKComponents {
            cos_align: 0.8,
            fisher_violation: 0.2,
            ece: 0.1,
            embedder_coherence: 0.6,
            embedder_coherence_source: "computed".to_string(),
        },
        DeltaOmegaComponents {
            effective_plasticity: 0.8,
            landscape_health: 0.7,
            stability_floor: 1.0,
            agent_state_score: 0.7,
            agent_state_source: "default_neutral_no_transcript".to_string(),
        },
        DeltaXiComponents {
            target_collapse: 0.0,
            predictor_redundancy: 0.05,
            constellation_violation_rate: 0.10,
        },
    )?;
    let cert_db_path = out.join("utml-cert-rocksdb");
    let cert_db = open_train_rocksdb(&cert_db_path)?;
    let cert_count_before = count_cf(cert_db.as_ref(), CF_MEJEPA_TRAIN_CERTS)?;
    let mut cert_writer = TrainCertWriter::new(
        cert_db.clone(),
        CF_MEJEPA_TRAIN_CERTS.to_string(),
        git_head()?,
        "utml-fsv-corpus-sha".to_string(),
        HashMap::from([(
            "utml_formula_carrier".to_string(),
            "doc11-v1-carriers".to_string(),
        )]),
        "2026-05-12T00:00:00Z".to_string(),
    )?;
    let mut cert = training_cert(0, signal.clone(), None);
    cert_writer.emit(&mut cert)?;
    let cf_handle = cert_db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or("missing CF_MEJEPA_TRAIN_CERTS")?;
    cert_db.flush_cf(cf_handle)?;
    let cert_count_after_happy = count_cf(cert_db.as_ref(), CF_MEJEPA_TRAIN_CERTS)?;
    if cert_count_after_happy != cert_count_before + 1 {
        return Err(format!(
            "expected happy-path cert count {} -> {}, observed {}",
            cert_count_before,
            cert_count_before + 1,
            cert_count_after_happy
        )
        .into());
    }
    let readback_cert = read_cert(cert_db.as_ref(), 0)?;
    assert_signal_equal(
        "persisted happy-path UTML signal",
        &readback_cert.signal,
        &signal,
    )?;
    let auditor = PairwiseMiAuditor::new(1, &out)?;
    let panels = (0..6).map(build_panel).collect::<Result<Vec<_>, _>>()?;
    let mi = auditor.run_from_panels(&panels, 42)?;
    let embedders = (0..14)
        .map(|idx| Box::new(DeterministicEmbedder { idx }) as Box<dyn ContentEmbedder>)
        .collect::<Vec<_>>();
    let corpus = (0..6)
        .map(|idx| format!("synthetic real UTML coherence document {idx}"))
        .collect::<Vec<_>>();
    let coherence = compute_embedder_coherence(&embedders, &corpus, 2, 4)?;
    let window = LStepWindow {
        signals: vec![signal.clone(); 4],
        stale_update_ratio: 0.10,
        optimizer_spike_ratio: 0.05,
        dissipating_ratio: 0.05,
    };
    let verdict = classify_utml_state(&window)?;
    let prune = prune_decision(&signal, &verdict)?;
    let m_t = update_m_t(0.6, signal.delta_omega, signal.delta_xi, 0.25)?;
    let transfer = compute_state_transfer(
        &[0.1, 0.2, 0.4, 0.8],
        &[0.1, 0.25, 0.45, 0.75],
        DivergenceMetric::Wasserstein1,
        0.5,
    )?;
    let sampling_components = SamplingWeightComponents {
        base_weight: 2.0,
        l_step: signal.l_step,
        operator_override: true,
        age_days: 3.0,
        age_decay: 0.99,
        agent_surprise_severity_score: 0.25,
        foundationality_score: 0.0,
        lambda_foundationality: 1.0,
        curiosity_score: 0.0,
        lambda_curiosity: 1.0,
    };
    let sampling_weight = sampling_weight_checked(sampling_components)?;
    let expected_sampling_weight = 2.0 * signal.l_step * 6.0 * 1.5 * 0.99_f32.powf(3.0);
    assert_close(
        "sampling_weight_checked full formula",
        sampling_weight,
        expected_sampling_weight,
        0.00001,
    )?;
    let confidence_components = CalibratedConfidenceComponents {
        raw_confidence: 0.80,
        convergence_rate: 0.90,
        strategy_agreement: 0.75,
        evidence_factor: 0.70,
        delta_omega_mean: 0.60,
        delta_xi_mean: 0.50,
    };
    let calibrated = calibrated_confidence(confidence_components)?;
    let expected_calibrated = (0.80_f32 * 0.90 * 0.75 * 0.70 * 0.60 * 0.50).clamp(0.10, 0.95);
    assert_close(
        "calibrated_confidence product clamp",
        calibrated,
        expected_calibrated,
        0.00001,
    )?;
    let reward_components = RewardComponents {
        calibrated_confidence: calibrated,
        mean_l_step_delta_xi: signal.delta_xi,
    };
    let reward_value = reward(reward_components)?;
    let expected_reward = (0.6 * calibrated + 0.4 * signal.delta_xi).clamp(0.0, 1.0);
    assert_close("reward formula", reward_value, expected_reward, 0.00001)?;
    let dormant = detect_dormant_units(
        &DormantUnitDetector {
            activation_threshold: 0.01,
            dormant_fraction_threshold: 0.25,
        },
        &DormantLayer {
            activations: vec![
                vec![0.0, 0.8, 0.0, 0.5],
                vec![0.0, 0.7, 0.0, 0.4],
                vec![0.0, 0.9, 0.0, 0.6],
            ],
        },
    )?;
    let mut weights = vec![vec![0.0; 3]; 4];
    let mut biases = vec![1.0; 4];
    let before_reinit = json!({"weights": weights, "biases": biases});
    reinit_dormant_units(&mut weights, &mut biases, &dormant.dormant_units, 123)?;
    let after_reinit = json!({"weights": weights, "biases": biases});

    let mut edge_cases = vec![
        json!({
            "name": "empty_pairwise_mi_panels",
            "before_state": {"panel_count": 0, "train_cert_count": cert_count_after_happy},
            "trigger": "PairwiseMiAuditor::run_from_panels([])",
            "after_state": {"error": auditor.run_from_panels(&[], 43).unwrap_err().code(), "train_cert_count": count_cf(cert_db.as_ref(), CF_MEJEPA_TRAIN_CERTS)?},
            "passes": true
        }),
        edge_nonfinite_component(cert_db.as_ref())?,
        edge_zero_multiplicative_gate(&mut cert_writer, cert_db.as_ref())?,
        edge_conflicting_state_transfer(cert_db.as_ref())?,
        json!({
            "name": "sampling_weight_negative_age",
            "before_state": {"age_days": -1.0, "train_cert_count": count_cf(cert_db.as_ref(), CF_MEJEPA_TRAIN_CERTS)?},
            "trigger": "sampling_weight_checked with negative age_days",
            "after_state": {"error": sampling_weight_checked(SamplingWeightComponents {
                base_weight: 1.0,
                l_step: 0.5,
                operator_override: false,
                age_days: -1.0,
                age_decay: 0.99,
                agent_surprise_severity_score: 0.0,
                foundationality_score: 0.0,
                lambda_foundationality: 1.0,
                curiosity_score: 0.0,
                lambda_curiosity: 1.0,
            }).unwrap_err().code(), "train_cert_count": count_cf(cert_db.as_ref(), CF_MEJEPA_TRAIN_CERTS)?},
            "passes": true
        }),
    ];
    let chain_before_reopen = verify_chain(&cert_db, CF_MEJEPA_TRAIN_CERTS, 0, 1)?;
    edge_cases.iter_mut().for_each(|case| {
        if case.get("passes").is_none() {
            case["passes"] = json!(true);
        }
    });
    if edge_cases
        .iter()
        .any(|case| case.get("passes").and_then(|value| value.as_bool()) != Some(true))
    {
        return Err(format!(
            "one or more UTML edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }
    drop(cert_writer);
    drop(cert_db);
    let reopened_cert_db = open_train_rocksdb(&cert_db_path)?;
    let reopened_cert_count = count_cf(reopened_cert_db.as_ref(), CF_MEJEPA_TRAIN_CERTS)?;
    if reopened_cert_count != 2 {
        return Err(format!(
            "expected 2 persisted training certs after reopen, got {reopened_cert_count}"
        )
        .into());
    }
    let reopened_cert = read_cert(reopened_cert_db.as_ref(), 0)?;
    assert_signal_equal(
        "reopened happy-path UTML signal",
        &reopened_cert.signal,
        &signal,
    )?;
    let chain_after_reopen = verify_chain(&reopened_cert_db, CF_MEJEPA_TRAIN_CERTS, 0, 1)?;

    let summary = json!({
        "all_passed": true,
        "source_of_truth": {
            "l_step_evidence": out.join("utml-l-step-evidence.json"),
            "pairwise_mi_matrix_csv": mi.path,
            "train_cert_rocksdb": cert_db_path,
            "train_cert_cf": CF_MEJEPA_TRAIN_CERTS,
            "summary": out.join("utml-fsv-summary.json")
        },
        "happy_path": {
            "before_state": {"train_cert_count": cert_count_before},
            "trigger": "compute_l_step -> TrainCertWriter::emit -> flush -> read CF_MEJEPA_TRAIN_CERTS",
            "after_state": {
                "train_cert_count": cert_count_after_happy,
                "cert_step": readback_cert.step,
                "delta_p_components": readback_cert.signal.delta_p_components,
                "delta_k_components": readback_cert.signal.delta_k_components,
                "delta_omega_components": readback_cert.signal.delta_omega_components,
                "delta_xi_components": readback_cert.signal.delta_xi_components,
                "per_head_l_step_count": readback_cert.per_head_l_step.len(),
                "chain_before_reopen": chain_before_reopen,
                "reopened_cert_count": reopened_cert_count,
                "chain_after_reopen": chain_after_reopen,
            },
            "passes": true
        },
        "signal": signal,
        "pairwise_mi": mi,
        "embedder_coherence": coherence,
        "classifier": verdict,
        "prune": prune,
        "m_t": m_t,
        "state_transfer": transfer,
        "doc11_formula_carriers": {
            "sampling_components": sampling_components,
            "sampling_weight": sampling_weight,
            "expected_sampling_weight": expected_sampling_weight,
            "calibrated_confidence_components": confidence_components,
            "calibrated_confidence": calibrated,
            "expected_calibrated_confidence": expected_calibrated,
            "reward_components": reward_components,
            "reward": reward_value,
            "expected_reward": expected_reward
        },
        "dormant": dormant,
        "before_reinit": before_reinit,
        "after_reinit": after_reinit,
        "edge_cases": edge_cases
    });
    write_json_0600(&out.join("utml-l-step-evidence.json"), &summary["signal"])?;
    write_json_0600(
        &out.join("utml-classifier-evidence.json"),
        &summary["classifier"],
    )?;
    write_json_0600(
        &out.join("utml-doc11-formula-carriers.json"),
        &summary["doc11_formula_carriers"],
    )?;
    write_json_0600(&out.join("utml-edge-cases.json"), &edge_cases)?;
    write_json_0600(&out.join("utml-fsv-summary.json"), &summary)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn count_cf(db: &DB, cf_name: &'static str) -> Result<usize, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing RocksDB column family {cf_name}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn read_cert(db: &DB, step: u64) -> Result<TrainingCertificate, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or("missing CF_MEJEPA_TRAIN_CERTS")?;
    let bytes = db
        .get_cf(cf, step.to_be_bytes())?
        .ok_or_else(|| format!("missing training certificate at step {step}"))?;
    let cert: TrainingCertificate = serde_json::from_slice(&bytes)?;
    cert.validate_phase3()?;
    if cert.step != step {
        return Err(format!("training certificate step {} != key {step}", cert.step).into());
    }
    Ok(cert)
}

fn training_cert(
    step: u64,
    signal: LearningSignal,
    training_mode: Option<&str>,
) -> TrainingCertificate {
    let conditional_description_length_bits =
        context_graph_mejepa_train::conditional_description_length_bits_from_probability(
            signal.delta_p,
        );
    TrainingCertificate {
        step,
        epoch: 0,
        per_head_l_step: HeadId::ALL
            .into_iter()
            .map(|head| (head.as_str().to_string(), signal.l_step))
            .collect(),
        delta_xi_global_min: signal.delta_xi,
        loss_components: signal.components_as_map(),
        conditional_description_length_bits: Some(conditional_description_length_bits),
        inverse_map_quality_bits: None,
        l_entropy_nats: None,
        l_entropy_weighted: None,
        l_entropy_lambda: None,
        l_entropy_sample_count: None,
        l_entropy_estimator: None,
        training_mode: training_mode.unwrap_or("utml_l_step_fsv").to_string(),
        distillation_cycle_id: None,
        distillation_loss_mean: None,
        distillation_skipped_count: None,
        counterfactual_smoothness: Some(0.0),
        counterfactual_smoothness_anomaly: Some(false),
        adversarial_mix_target_ratio: 0.20,
        adversarial_mix_count: 0,
        adversarial_mix_example_indices: Vec::new(),
        adversarial_mix_fallback_count: 0,
        cross_task_transfer_indices: Vec::new(),
        cross_task_transfer_fallback_count: 0,
        predictor_redundancy_pairwise_mi: signal.delta_xi_components.predictor_redundancy,
        predictor_redundancy_pairwise_mi_source: "utml_l_step_fsv_pairwise_mi_matrix".to_string(),
        holdout_promotion: Some(false),
        generic_only_warning: None,
        phase3_dod_passed: Some(true),
        grad_norm_running_mean: 0.0,
        parent_witness_hash: String::new(),
        self_hash: String::new(),
        merkle_root: String::new(),
        code_version: String::new(),
        corpus_sha: String::new(),
        embedder_versions: HashMap::new(),
        frozen_at: String::new(),
        signal,
    }
}

fn edge_nonfinite_component(db: &DB) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let before_count = count_cf(db, CF_MEJEPA_TRAIN_CERTS)?;
    let result = compute_l_step(
        DeltaPComponents {
            delta_p_real: f32::NAN,
            delta_p_imagined: None,
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![f32::NAN],
        },
        DeltaKComponents {
            cos_align: 0.8,
            fisher_violation: 0.2,
            ece: 0.1,
            embedder_coherence: 0.6,
            embedder_coherence_source: "computed".to_string(),
        },
        DeltaOmegaComponents {
            effective_plasticity: 0.8,
            landscape_health: 0.7,
            stability_floor: 1.0,
            agent_state_score: 0.7,
            agent_state_source: "default_neutral_no_transcript".to_string(),
        },
        DeltaXiComponents {
            target_collapse: 0.0,
            predictor_redundancy: 0.05,
            constellation_violation_rate: 0.10,
        },
    );
    let err = match result {
        Ok(_) => return Err("non-finite UTML component unexpectedly succeeded".into()),
        Err(err) => err,
    };
    let after_count = count_cf(db, CF_MEJEPA_TRAIN_CERTS)?;
    if after_count != before_count {
        return Err("non-finite component edge case mutated training cert state".into());
    }
    Ok(json!({
        "name": "nonfinite_delta_p_component",
        "before_state": {"train_cert_count": before_count, "delta_p_real": "NaN"},
        "trigger": "compute_l_step with NaN delta_p_real and per_chunk_values",
        "after_state": {"error": err.code(), "train_cert_count": after_count},
        "passes": err.code() == "UTML_NON_FINITE"
    }))
}

fn edge_zero_multiplicative_gate(
    cert_writer: &mut TrainCertWriter,
    db: &DB,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let before_count = count_cf(db, CF_MEJEPA_TRAIN_CERTS)?;
    let zero_signal = compute_l_step(
        DeltaPComponents {
            delta_p_real: 0.5,
            delta_p_imagined: Some(0.5),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.5, 0.5],
        },
        DeltaKComponents {
            cos_align: 0.8,
            fisher_violation: 0.2,
            ece: 0.1,
            embedder_coherence: 0.6,
            embedder_coherence_source: "computed".to_string(),
        },
        DeltaOmegaComponents {
            effective_plasticity: 0.8,
            landscape_health: 0.7,
            stability_floor: 0.0,
            agent_state_score: 0.7,
            agent_state_source: "default_neutral_no_transcript".to_string(),
        },
        DeltaXiComponents {
            target_collapse: 0.0,
            predictor_redundancy: 0.05,
            constellation_violation_rate: 0.10,
        },
    )?;
    if zero_signal.l_step != 0.0 {
        return Err(format!("expected zero-gate l_step 0.0, got {}", zero_signal.l_step).into());
    }
    let mut cert = training_cert(1, zero_signal.clone(), Some("utml_zero_gate_fsv"));
    cert_writer.emit(&mut cert)?;
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or("missing CF_MEJEPA_TRAIN_CERTS")?;
    db.flush_cf(cf)?;
    let after_count = count_cf(db, CF_MEJEPA_TRAIN_CERTS)?;
    let readback = read_cert(db, 1)?;
    assert_signal_equal(
        "zero-gate training cert readback signal",
        &readback.signal,
        &zero_signal,
    )?;
    Ok(json!({
        "name": "zero_multiplicative_gate_persists_zero_l_step",
        "before_state": {"train_cert_count": before_count, "stability_floor": 0.0},
        "trigger": "compute_l_step with stability_floor=0.0 -> TrainCertWriter::emit step 1 -> read CF_MEJEPA_TRAIN_CERTS",
        "after_state": {"train_cert_count": after_count, "l_step": readback.signal.l_step, "cert_step": readback.step},
        "passes": after_count == before_count + 1 && readback.signal.l_step == 0.0
    }))
}

fn edge_conflicting_state_transfer(
    db: &DB,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let before_count = count_cf(db, CF_MEJEPA_TRAIN_CERTS)?;
    let result = compute_state_transfer(&[0.1], &[0.2, 0.3], DivergenceMetric::Wasserstein1, 0.5);
    let err = match result {
        Ok(_) => return Err("conflicting state transfer input unexpectedly succeeded".into()),
        Err(err) => err,
    };
    let after_count = count_cf(db, CF_MEJEPA_TRAIN_CERTS)?;
    if after_count != before_count {
        return Err("conflicting state transfer edge case mutated training cert state".into());
    }
    Ok(json!({
        "name": "conflicting_state_transfer_cardinality",
        "before_state": {"train_cert_count": before_count, "source_len": 1, "target_len": 2},
        "trigger": "compute_state_transfer with mismatched source/target cardinality",
        "after_state": {"error": err.code(), "train_cert_count": after_count},
        "passes": err.code() == "UTML_INVALID_SIGNAL"
    }))
}

fn git_head() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --verify HEAD failed; cannot record code_version".into());
    }
    let sha = String::from_utf8(output.stdout)?.trim().to_string();
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("git HEAD was not a 40-char hex SHA: {sha:?}").into());
    }
    Ok(sha.to_ascii_lowercase())
}

fn build_panel(seed: usize) -> Result<Panel, context_graph_mejepa_instruments::InstrumentError> {
    let mut builder = PanelBuilder::new();
    for slot in InstrumentSlot::all() {
        let values = (0..slot.dim())
            .map(|idx| ((seed + idx + slot.offset()) as f32 * 0.001).sin())
            .collect::<Vec<_>>();
        builder.set_slot(slot, &values)?;
    }
    builder.build()
}

fn assert_close(
    label: &str,
    actual: f32,
    expected: f32,
    tolerance: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    if (actual - expected).abs() <= tolerance {
        Ok(())
    } else {
        Err(format!("{label}: actual {actual} differs from expected {expected}").into())
    }
}

fn assert_signal_equal(
    label: &str,
    actual: &LearningSignal,
    expected: &LearningSignal,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual_json = serde_json::to_value(actual)?;
    let expected_json = serde_json::to_value(expected)?;
    if actual_json == expected_json {
        Ok(())
    } else {
        Err(format!("{label}: persisted signal JSON differs from expected signal JSON").into())
    }
}

#[derive(Debug)]
struct DeterministicEmbedder {
    idx: usize,
}

impl ContentEmbedder for DeterministicEmbedder {
    fn id(&self) -> &str {
        const IDS: [&str; 14] = [
            "e1", "e2", "e3", "e4", "e5", "e6", "e7", "e8", "e9", "e10", "e11", "e12", "e13", "e14",
        ];
        IDS[self.idx]
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, context_graph_mejepa_train::UtmlError> {
        let base = text.len() as f32 + self.idx as f32;
        Ok(vec![
            base.sin(),
            base.cos(),
            (base * 0.5).sin(),
            (base * 0.25).cos(),
        ])
    }
}

fn write_json_0600(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?
    };
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(format!("{} readback mismatch", path.display()).into());
    }
    Ok(())
}
