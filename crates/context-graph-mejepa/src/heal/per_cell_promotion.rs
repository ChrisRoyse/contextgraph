use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::eval::EvalReport;
use crate::heal::errors::HealError;
use crate::heal::policy::{persist_policy_record, policy_key};
use crate::heal::promote::{HoldoutEval, ModeWinner};
use crate::heal::store::HealRocksStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionScore {
    pub holdout_correlation: f32,
    pub latency_multiplier: f32,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerCellPromotionDecision {
    pub accepted: bool,
    pub winner: ModeWinner,
    pub mode_a: PromotionScore,
    pub mode_b: PromotionScore,
    pub mode_c: PromotionScore,
    pub regressing_cells: Vec<String>,
    pub compared_cells: BTreeMap<String, CellPromotionDelta>,
    pub source_of_truth_cf: String,
    pub source_of_truth_key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CellPromotionDelta {
    pub before: f32,
    pub after: f32,
    pub delta: f32,
    pub holds_or_improves: bool,
}

pub fn score_holdout(
    eval: &HoldoutEval,
    latency_multiplier: f32,
) -> Result<PromotionScore, HealError> {
    if !latency_multiplier.is_finite() || latency_multiplier <= 0.0 {
        return Err(HealError::invalid(
            "promotion.latency_multiplier",
            "latency multiplier must be positive and finite",
        ));
    }
    let holdout_correlation = eval.oracle_agreement;
    Ok(PromotionScore {
        holdout_correlation,
        latency_multiplier,
        score: holdout_correlation - 0.05 * latency_multiplier,
    })
}

pub fn choose_winner_by_phase_e_score(
    mode_a: &HoldoutEval,
    mode_b: &HoldoutEval,
    mode_c: &HoldoutEval,
    latency_a: f32,
    latency_b: f32,
    latency_c: f32,
) -> Result<ModeWinner, HealError> {
    let scores = [
        (ModeWinner::A, score_holdout(mode_a, latency_a)?),
        (ModeWinner::B, score_holdout(mode_b, latency_b)?),
        (ModeWinner::C, score_holdout(mode_c, latency_c)?),
    ];
    Ok(scores
        .into_iter()
        .max_by(|left, right| left.1.score.total_cmp(&right.1.score))
        .map(|(winner, _)| winner)
        .expect("non-empty candidates"))
}

pub fn persist_per_cell_promotion_decision(
    storage: &HealRocksStore,
    mut decision: PerCellPromotionDecision,
) -> Result<PerCellPromotionDecision, HealError> {
    let key = policy_key(&[
        "phase_e",
        "per-cell-promotion",
        &format!("{:020}", chrono::Utc::now().timestamp_millis()),
    ])?;
    decision.source_of_truth_key_hex = hex::encode(&key);
    persist_policy_record(storage, &key, &decision)?;
    Ok(decision)
}

pub fn evaluate_latest_report_cells(
    storage: &HealRocksStore,
    report_before: &EvalReport,
    report_after: &EvalReport,
    tolerance: f32,
    winner: ModeWinner,
    mode_a: &HoldoutEval,
    mode_b: &HoldoutEval,
    mode_c: &HoldoutEval,
) -> Result<PerCellPromotionDecision, HealError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(HealError::invalid(
            "per_cell_promotion.tolerance",
            "tolerance must be finite and non-negative",
        ));
    }
    let mut compared = BTreeMap::new();
    let mut regressing = Vec::new();
    for (cell, before) in &report_before.per_cell_correlation {
        let Some(before) = before else {
            continue;
        };
        let Some(Some(after)) = report_after.per_cell_correlation.get(cell) else {
            regressing.push(format!("{cell}:missing_after"));
            continue;
        };
        let delta = after - before;
        let holds = delta + tolerance >= 0.0;
        if !holds {
            regressing.push(cell.clone());
        }
        compared.insert(
            cell.clone(),
            CellPromotionDelta {
                before: *before,
                after: *after,
                delta,
                holds_or_improves: holds,
            },
        );
    }
    let decision = PerCellPromotionDecision {
        accepted: regressing.is_empty() && winner.is_promoted(),
        winner,
        mode_a: score_holdout(mode_a, 1.0)?,
        mode_b: score_holdout(mode_b, 1.0)?,
        mode_c: score_holdout(mode_c, 1.0)?,
        regressing_cells: regressing,
        compared_cells: compared,
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        source_of_truth_key_hex: String::new(),
    };
    persist_per_cell_promotion_decision(storage, decision)
}
