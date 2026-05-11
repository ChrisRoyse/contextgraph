use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PanelBuilder};
use context_graph_mejepa_train::learning_signal::{
    calibrated_confidence, classify_utml_state, compute_embedder_coherence, compute_l_step,
    compute_state_transfer, detect_dormant_units, prune_decision, reinit_dormant_units, reward,
    sampling_weight_checked, update_m_t, CalibratedConfidenceComponents, ContentEmbedder,
    DeltaKComponents, DeltaOmegaComponents, DeltaPAggregator, DeltaPComponents, DeltaXiComponents,
    DivergenceMetric, DormantLayer, DormantUnitDetector, LStepWindow, PairwiseMiAuditor,
    RewardComponents, SamplingWeightComponents,
};
use serde::Serialize;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const OUT: &str = "/tmp/contextgraph-utml-fsv";

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
    };
    let sampling_weight = sampling_weight_checked(sampling_components)?;
    let expected_sampling_weight = 2.0 * signal.l_step * 6.0 * 1.25 * 0.99_f32.powf(3.0);
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

    let edge_cases = vec![
        json!({
            "name": "empty_pairwise_mi_panels",
            "before": {"panel_count": 0},
            "after": {"error": auditor.run_from_panels(&[], 43).unwrap_err().code()}
        }),
        json!({
            "name": "target_collapse_gradient_leak",
            "before": {"target_collapse": 0.5},
            "after": {"error": compute_l_step(
                DeltaPComponents::default(),
                DeltaKComponents::default(),
                DeltaOmegaComponents::default(),
                DeltaXiComponents { target_collapse: 0.5, predictor_redundancy: 0.0, constellation_violation_rate: 0.0 },
            ).unwrap_err().code()}
        }),
        json!({
            "name": "sampling_weight_negative_age",
            "before": {"age_days": -1.0},
            "after": {"error": sampling_weight_checked(SamplingWeightComponents {
                base_weight: 1.0,
                l_step: 0.5,
                operator_override: false,
                age_days: -1.0,
                age_decay: 0.99,
                agent_surprise_severity_score: 0.0,
            }).unwrap_err().code()}
        }),
        json!({
            "name": "invalid_state_transfer_lambda",
            "before": {"lambda": 0.0},
            "after": {"error": compute_state_transfer(&[0.1], &[0.2], DivergenceMetric::Wasserstein1, 0.0).unwrap_err().code()}
        }),
    ];

    let summary = json!({
        "source_of_truth": {
            "l_step_evidence": out.join("utml-l-step-evidence.json"),
            "pairwise_mi_matrix_csv": mi.path,
            "summary": out.join("utml-fsv-summary.json")
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
