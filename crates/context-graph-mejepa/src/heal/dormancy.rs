use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::heal::errors::HealError;
use crate::heal::pipeline::SelfHealingPipeline;
use crate::heal::policy::{persist_policy_record, timestamped_policy_key};
use crate::types::HeadId;

const ACTIVATION_THRESHOLD: f32 = 0.05;
const DORMANT_FRACTION_THRESHOLD: f32 = 0.15;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeadDormancyDecision {
    pub head: String,
    pub unit_count: usize,
    pub dormant_units: Vec<usize>,
    pub dormant_fraction: f32,
    pub detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerHeadDormancyReport {
    pub generated_at_unix_ms: i64,
    pub sample_count: usize,
    pub activation_threshold: f32,
    pub dormant_fraction_threshold: f32,
    pub decisions: BTreeMap<String, HeadDormancyDecision>,
    pub detected_head_count: usize,
    pub reinitialized_units_by_head: BTreeMap<String, Vec<usize>>,
    pub reinitialized_unit_count: usize,
}

pub fn tick_per_head_dormancy(
    pipeline: &mut SelfHealingPipeline,
) -> Result<PerHeadDormancyReport, HealError> {
    let mut report = detect_per_head_dormancy(pipeline)?;
    let reinitialized = reinit_dormant_units_per_head(pipeline, &report)?;
    report.reinitialized_unit_count = reinitialized.values().map(Vec::len).sum();
    report.reinitialized_units_by_head = reinitialized;
    let key = timestamped_policy_key("per_head_dormancy")?;
    persist_policy_record(pipeline.storage.as_ref(), &key, &report)?;
    Ok(report)
}

pub fn detect_per_head_dormancy(
    pipeline: &SelfHealingPipeline,
) -> Result<PerHeadDormancyReport, HealError> {
    let unit_count = pipeline
        .active_embedders
        .iter()
        .find(|embedder| !embedder.weights.is_empty())
        .map(|embedder| embedder.weights.len())
        .ok_or_else(|| {
            HealError::invalid(
                "per_head_dormancy.active_embedders",
                "cannot inspect dormancy without active embedder weights",
            )
        })?;
    let sample_count = pipeline.dormant_activation_window.len();
    let mut decisions = BTreeMap::new();
    for (head_idx, head) in HeadId::ALL.into_iter().enumerate() {
        let head_name = head.as_str().to_string();
        let units = (0..unit_count)
            .filter(|idx| idx % HeadId::ALL.len() == head_idx)
            .collect::<Vec<_>>();
        if units.is_empty() {
            decisions.insert(
                head_name.clone(),
                HeadDormancyDecision {
                    head: head_name,
                    unit_count: 0,
                    dormant_units: Vec::new(),
                    dormant_fraction: 0.0,
                    detected: false,
                },
            );
            continue;
        }
        let dormant_units = if sample_count == 0 {
            Vec::new()
        } else {
            let mut dormant = Vec::new();
            for &unit in &units {
                let mut total = 0.0f32;
                for (row_idx, row) in pipeline.dormant_activation_window.iter().enumerate() {
                    if row.len() != unit_count {
                        return Err(HealError::invalid(
                            "per_head_dormancy.activation_window",
                            format!(
                                "activation row {row_idx} has {} units; expected {unit_count}",
                                row.len()
                            ),
                        ));
                    }
                    let value = row[unit];
                    if !value.is_finite() {
                        return Err(HealError::BatchNan {
                            component: format!("per_head_dormancy.activation[{row_idx}][{unit}]"),
                            witness_chain_offset: 0,
                        });
                    }
                    total += value.abs();
                }
                if total / sample_count as f32 <= ACTIVATION_THRESHOLD {
                    dormant.push(unit);
                }
            }
            dormant
        };
        let dormant_fraction = dormant_units.len() as f32 / units.len() as f32;
        let detected = sample_count > 0 && dormant_fraction > DORMANT_FRACTION_THRESHOLD;
        decisions.insert(
            head_name.clone(),
            HeadDormancyDecision {
                head: head_name,
                unit_count: units.len(),
                dormant_units,
                dormant_fraction,
                detected,
            },
        );
    }
    let detected_head_count = decisions
        .values()
        .filter(|decision| decision.detected)
        .count();
    Ok(PerHeadDormancyReport {
        generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        sample_count,
        activation_threshold: ACTIVATION_THRESHOLD,
        dormant_fraction_threshold: DORMANT_FRACTION_THRESHOLD,
        decisions,
        detected_head_count,
        reinitialized_units_by_head: BTreeMap::new(),
        reinitialized_unit_count: 0,
    })
}

pub fn reinit_dormant_units_per_head(
    pipeline: &mut SelfHealingPipeline,
    report: &PerHeadDormancyReport,
) -> Result<BTreeMap<String, Vec<usize>>, HealError> {
    let mut reinitialized = BTreeMap::new();
    for decision in report
        .decisions
        .values()
        .filter(|decision| decision.detected)
    {
        if decision.dormant_units.is_empty() {
            continue;
        }
        for embedder in &mut pipeline.active_embedders {
            for &unit in &decision.dormant_units {
                if unit >= embedder.weights.len() {
                    return Err(HealError::invalid(
                        "per_head_dormancy.reinit",
                        format!(
                            "unit {unit} exceeds embedder {} weight length {}",
                            embedder.embedder_id,
                            embedder.weights.len()
                        ),
                    ));
                }
                embedder.weights[unit] =
                    deterministic_reinit_weight(embedder.embedder_id, &decision.head, unit);
            }
        }
        reinitialized.insert(decision.head.clone(), decision.dormant_units.clone());
    }
    Ok(reinitialized)
}

fn deterministic_reinit_weight(embedder_id: u32, head: &str, unit: usize) -> f32 {
    let head_sum = head.bytes().map(u32::from).sum::<u32>() as f32;
    ((embedder_id as f32 + head_sum + unit as f32 + 1.0).sin() * 0.02).clamp(-0.02, 0.02)
}
