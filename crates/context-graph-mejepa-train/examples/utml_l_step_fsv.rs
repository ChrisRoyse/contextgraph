use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PanelBuilder};
use context_graph_mejepa_train::learning_signal::{
    classify_utml_state, compute_embedder_coherence, compute_l_step, compute_state_transfer,
    detect_dormant_units, prune_decision, reinit_dormant_units, update_m_t, ContentEmbedder,
    DeltaKComponents, DeltaOmegaComponents, DeltaPComponents, DeltaXiComponents, DivergenceMetric,
    DormantLayer, DormantUnitDetector, LStepWindow, PairwiseMiAuditor,
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
            oracle_disagreement: 0.9,
            prediction_residual: 0.7,
            uncertainty: 0.5,
        },
        DeltaKComponents {
            operator_override: 0.2,
            novel_failure_mode: 0.8,
            sparse_slice_bonus: 0.5,
        },
        DeltaOmegaComponents {
            runtime_delta: 0.8,
            warning_delta: 0.6,
            memory_delta: 0.7,
        },
        DeltaXiComponents {
            target_collapse: 0.0,
            predictor_collapse: 0.05,
            redundancy_mi: 0.10,
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
                DeltaPComponents { oracle_disagreement: 0.1, prediction_residual: 0.1, uncertainty: 0.1 },
                DeltaKComponents { operator_override: 0.1, novel_failure_mode: 0.1, sparse_slice_bonus: 0.1 },
                DeltaOmegaComponents { runtime_delta: 0.1, warning_delta: 0.1, memory_delta: 0.1 },
                DeltaXiComponents { target_collapse: 0.5, predictor_collapse: 0.0, redundancy_mi: 0.0 },
            ).unwrap_err().code()}
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
