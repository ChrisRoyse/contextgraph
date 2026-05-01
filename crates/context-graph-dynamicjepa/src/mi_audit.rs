use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MI_AUDIT_MIN_SAMPLE_SIZE: usize = 50;
pub const MI_AUDIT_LOOSE_THRESHOLD: f64 = 0.7;
pub const MI_AUDIT_STRICT_THRESHOLD: f64 = 0.9;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairwiseMiPairReport {
    pub instrument_j: String,
    pub instrument_k: String,
    pub instrument_j_name: String,
    pub instrument_k_name: String,
    pub d_j: usize,
    pub d_k: usize,
    pub n_audit: usize,
    pub estimator: String,
    pub ksg_k: usize,
    pub mi_nats: f64,
    pub mi_bits: f64,
    pub mi_normalised: f64,
    pub bootstrap_low: f64,
    pub bootstrap_high: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairwiseMiSummary {
    pub domain: String,
    pub n_panel_slots: usize,
    pub n_pairs: usize,
    pub mean_mi_nats: f64,
    pub median_mi_nats: f64,
    pub p10_mi_nats: f64,
    pub p90_mi_nats: f64,
    pub n_redundant_pairs_loose: usize,
    pub n_redundant_pairs_strict: usize,
    pub n_eff_loose: f64,
    pub n_eff_strict: f64,
    pub verdict: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KsgEstimate {
    pub mi_nats: f64,
    pub mi_bits: f64,
    pub mi_normalised: f64,
    pub x_entropy_nats: f64,
    pub y_entropy_nats: f64,
    #[serde(skip)]
    pub local_mi_nats: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn gen_index(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        let upper_u64 = upper as u64;
        let zone = u64::MAX - (u64::MAX % upper_u64);
        loop {
            let value = self.next_u64();
            if value < zone {
                return (value % upper_u64) as usize;
            }
        }
    }
}

pub fn estimate_ksg_1(
    instrument_j: &str,
    xs: &[Vec<f32>],
    instrument_k: &str,
    ys: &[Vec<f32>],
    k: usize,
) -> DynamicJepaResult<KsgEstimate> {
    validate_pair_samples(instrument_j, xs, instrument_k, ys, k)?;
    let local_mi_nats = ksg_1_local_mi_contributions(instrument_j, xs, instrument_k, ys, k)?;
    let mi_nats = local_mi_nats.iter().sum::<f64>() / local_mi_nats.len() as f64;
    if !mi_nats.is_finite() {
        return Err(DynamicJepaError::MiAuditDegenerateInput {
            instrument_id: format!("{instrument_j}/{instrument_k}"),
            reason: "KSG estimate was not finite".to_string(),
        });
    }
    let x_entropy_nats = empirical_entropy_nats(instrument_j, xs)?;
    let y_entropy_nats = empirical_entropy_nats(instrument_k, ys)?;
    let normalizer = x_entropy_nats.min(y_entropy_nats);
    if normalizer <= 0.0 || !normalizer.is_finite() {
        return Err(DynamicJepaError::MiAuditDegenerateInput {
            instrument_id: format!("{instrument_j}/{instrument_k}"),
            reason: "entropy normalizer was zero or non-finite".to_string(),
        });
    }
    let mi_normalised = (mi_nats.max(0.0) / normalizer).clamp(0.0, 1.0);
    Ok(KsgEstimate {
        mi_nats,
        mi_bits: mi_nats / std::f64::consts::LN_2,
        mi_normalised,
        x_entropy_nats,
        y_entropy_nats,
        local_mi_nats,
    })
}

pub fn bootstrap_ksg_1_ci(
    instrument_j: &str,
    xs: &[Vec<f32>],
    instrument_k: &str,
    ys: &[Vec<f32>],
    k: usize,
    bootstrap_iters: usize,
    seed: u64,
) -> DynamicJepaResult<(f64, f64)> {
    if bootstrap_iters == 0 {
        return Err(DynamicJepaError::MiAuditBootstrapDegenerate {
            instrument_j: instrument_j.to_string(),
            instrument_k: instrument_k.to_string(),
            bootstrap_iters,
        });
    }
    validate_pair_samples(instrument_j, xs, instrument_k, ys, k)?;
    let local_mi_nats = ksg_1_local_mi_contributions(instrument_j, xs, instrument_k, ys, k)?;
    let n = local_mi_nats.len();
    let mut rng = SplitMix64::new(seed);
    let mut estimates = Vec::with_capacity(bootstrap_iters);
    for _ in 0..bootstrap_iters {
        let mut total = 0.0f64;
        for _ in 0..n {
            let idx = rng.gen_index(n);
            total += local_mi_nats[idx];
        }
        estimates.push(total / n as f64);
    }
    estimates.sort_by(|left, right| left.total_cmp(right));
    let low = percentile_sorted(&estimates, 0.025);
    let high = percentile_sorted(&estimates, 0.975);
    if !low.is_finite() || !high.is_finite() {
        return Err(DynamicJepaError::MiAuditBootstrapDegenerate {
            instrument_j: instrument_j.to_string(),
            instrument_k: instrument_k.to_string(),
            bootstrap_iters,
        });
    }
    Ok((low, high))
}

fn ksg_1_local_mi_contributions(
    instrument_j: &str,
    xs: &[Vec<f32>],
    instrument_k: &str,
    ys: &[Vec<f32>],
    k: usize,
) -> DynamicJepaResult<Vec<f64>> {
    validate_pair_samples(instrument_j, xs, instrument_k, ys, k)?;
    let n = xs.len();
    let constant = digamma(k as f64) + digamma(n as f64);
    let mut local = Vec::with_capacity(n);
    for i in 0..n {
        let mut joint_distances = Vec::with_capacity(n.saturating_sub(1));
        for j in 0..n {
            if i == j {
                continue;
            }
            let dx = chebyshev_distance(&xs[i], &xs[j]);
            let dy = chebyshev_distance(&ys[i], &ys[j]);
            joint_distances.push((dx.max(dy), j));
        }
        joint_distances.sort_by(|(left_dist, left_idx), (right_dist, right_idx)| {
            left_dist
                .total_cmp(right_dist)
                .then_with(|| left_idx.cmp(right_idx))
        });
        let epsilon = joint_distances[k - 1].0;
        if !epsilon.is_finite() {
            return Err(DynamicJepaError::MiAuditDegenerateInput {
                instrument_id: format!("{instrument_j}/{instrument_k}"),
                reason: "non-finite KSG radius".to_string(),
            });
        }
        let mut nx = 0usize;
        let mut ny = 0usize;
        for j in 0..n {
            if i == j {
                continue;
            }
            if chebyshev_distance(&xs[i], &xs[j]) < epsilon {
                nx += 1;
            }
            if chebyshev_distance(&ys[i], &ys[j]) < epsilon {
                ny += 1;
            }
        }
        let contribution = constant - digamma((nx + 1) as f64) - digamma((ny + 1) as f64);
        if !contribution.is_finite() {
            return Err(DynamicJepaError::MiAuditDegenerateInput {
                instrument_id: format!("{instrument_j}/{instrument_k}"),
                reason: "non-finite KSG local contribution".to_string(),
            });
        }
        local.push(contribution);
    }
    Ok(local)
}

pub fn summarize_pairwise_mi(
    domain: impl Into<String>,
    n_panel_slots: usize,
    rows: &[PairwiseMiPairReport],
) -> DynamicJepaResult<PairwiseMiSummary> {
    if n_panel_slots < 2 {
        return Err(DynamicJepaError::validation(
            "pairwise_mi.n_panel_slots",
            format!("n_panel_slots must be >= 2, got {n_panel_slots}"),
            "register a domain with at least two instruments before auditing pairwise MI",
        ));
    }
    if rows.is_empty() {
        return Err(DynamicJepaError::validation(
            "pairwise_mi.rows",
            "cannot summarize zero pairwise MI rows",
            "audit at least one instrument pair before writing pairwise MI summary",
        ));
    }
    let mut values = rows.iter().map(|row| row.mi_nats).collect::<Vec<_>>();
    values.sort_by(|left, right| left.total_cmp(right));
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let n_redundant_pairs_loose = rows
        .iter()
        .filter(|row| row.mi_normalised >= MI_AUDIT_LOOSE_THRESHOLD)
        .count();
    let n_redundant_pairs_strict = rows
        .iter()
        .filter(|row| row.mi_normalised >= MI_AUDIT_STRICT_THRESHOLD)
        .count();
    let n_eff_loose = n_panel_slots as f64 - 0.5 * n_redundant_pairs_loose as f64;
    let n_eff_strict = n_panel_slots as f64 - 0.5 * n_redundant_pairs_strict as f64;
    let strict_ratio = n_eff_strict / n_panel_slots as f64;
    let verdict = if strict_ratio >= 0.95 {
        "STRONG"
    } else if strict_ratio >= 0.85 {
        "WEAK"
    } else {
        "FAILS"
    }
    .to_string();
    Ok(PairwiseMiSummary {
        domain: domain.into(),
        n_panel_slots,
        n_pairs: rows.len(),
        mean_mi_nats: mean,
        median_mi_nats: percentile_sorted(&values, 0.5),
        p10_mi_nats: percentile_sorted(&values, 0.1),
        p90_mi_nats: percentile_sorted(&values, 0.9),
        n_redundant_pairs_loose,
        n_redundant_pairs_strict,
        n_eff_loose,
        n_eff_strict,
        verdict,
    })
}

fn validate_pair_samples(
    instrument_j: &str,
    xs: &[Vec<f32>],
    instrument_k: &str,
    ys: &[Vec<f32>],
    k: usize,
) -> DynamicJepaResult<()> {
    if xs.len() != ys.len() {
        return Err(DynamicJepaError::validation(
            "pairwise_mi.sample_count",
            format!(
                "paired sample count mismatch: {instrument_j} has {}, {instrument_k} has {}",
                xs.len(),
                ys.len()
            ),
            "build both instrument sample matrices from the same event UUID set",
        ));
    }
    if xs.len() < MI_AUDIT_MIN_SAMPLE_SIZE {
        return Err(DynamicJepaError::MiAuditSampleSizeTooSmall {
            requested: xs.len(),
            available: xs.len(),
            minimum: MI_AUDIT_MIN_SAMPLE_SIZE,
        });
    }
    if k == 0 || k >= xs.len() {
        return Err(DynamicJepaError::validation(
            "pairwise_mi.ksg_k",
            format!("ksg_k must be in 1..sample_count, got k={k} n={}", xs.len()),
            "use a positive KSG neighbor count below the audited sample count",
        ));
    }
    validate_matrix(instrument_j, xs)?;
    validate_matrix(instrument_k, ys)?;
    Ok(())
}

fn validate_matrix(instrument_id: &str, matrix: &[Vec<f32>]) -> DynamicJepaResult<()> {
    let Some(first) = matrix.first() else {
        return Err(DynamicJepaError::MiAuditDegenerateInput {
            instrument_id: instrument_id.to_string(),
            reason: "empty matrix".to_string(),
        });
    };
    if first.is_empty() {
        return Err(DynamicJepaError::MiAuditDegenerateInput {
            instrument_id: instrument_id.to_string(),
            reason: "zero-dimensional instrument output".to_string(),
        });
    }
    let dim = first.len();
    let mut distinct = BTreeMap::<Vec<u32>, usize>::new();
    for (row_idx, row) in matrix.iter().enumerate() {
        if row.len() != dim {
            return Err(DynamicJepaError::MiAuditDegenerateInput {
                instrument_id: instrument_id.to_string(),
                reason: format!(
                    "inconsistent output dimension at row {row_idx}: expected {dim}, got {}",
                    row.len()
                ),
            });
        }
        let mut key = Vec::with_capacity(row.len());
        for (col_idx, value) in row.iter().enumerate() {
            if !value.is_finite() {
                return Err(DynamicJepaError::MiAuditDegenerateInput {
                    instrument_id: instrument_id.to_string(),
                    reason: format!("non-finite value at row {row_idx} col {col_idx}: {value}"),
                });
            }
            key.push(value.to_bits());
        }
        *distinct.entry(key).or_insert(0) += 1;
    }
    if distinct.len() < 2 {
        return Err(DynamicJepaError::MiAuditDegenerateInput {
            instrument_id: instrument_id.to_string(),
            reason: "all audited rows are identical".to_string(),
        });
    }
    Ok(())
}

fn empirical_entropy_nats(instrument_id: &str, matrix: &[Vec<f32>]) -> DynamicJepaResult<f64> {
    validate_matrix(instrument_id, matrix)?;
    let mut counts = BTreeMap::<Vec<u32>, usize>::new();
    for row in matrix {
        *counts
            .entry(row.iter().map(|value| value.to_bits()).collect())
            .or_insert(0) += 1;
    }
    let n = matrix.len() as f64;
    let entropy = counts
        .values()
        .map(|count| {
            let p = *count as f64 / n;
            -p * p.ln()
        })
        .sum::<f64>();
    if entropy <= 0.0 || !entropy.is_finite() {
        return Err(DynamicJepaError::MiAuditDegenerateInput {
            instrument_id: instrument_id.to_string(),
            reason: "empirical entropy was zero or non-finite".to_string(),
        });
    }
    Ok(entropy)
}

fn chebyshev_distance(left: &[f32], right: &[f32]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .fold(0.0, f64::max)
}

fn digamma(mut x: f64) -> f64 {
    let mut result = 0.0;
    while x < 8.0 {
        result -= 1.0 / x;
        x += 1.0;
    }
    let inv = 1.0 / x;
    let inv2 = inv * inv;
    result + x.ln() - 0.5 * inv - inv2 * (1.0 / 12.0 - inv2 * (1.0 / 120.0 - inv2 / 252.0))
}

fn percentile_sorted(sorted: &[f64], q: f64) -> f64 {
    debug_assert!(!sorted.is_empty());
    let clamped = q.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * clamped).round() as usize;
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ksg_mi_reports_higher_score_for_copied_signal_than_independent_signal() {
        let x = (0..80)
            .map(|idx| vec![(idx % 8) as f32, (idx / 8) as f32])
            .collect::<Vec<_>>();
        let copied = x.clone();
        let independent = (0..80)
            .map(|idx| vec![((idx * 37 + 11) % 17) as f32])
            .collect::<Vec<_>>();
        let copied_mi = estimate_ksg_1("x", &x, "copied", &copied, 3)
            .expect("copied signal estimate must be finite");
        let independent_mi = estimate_ksg_1("x", &x, "independent", &independent, 3)
            .expect("independent signal estimate must be finite");
        assert!(copied_mi.mi_normalised > independent_mi.mi_normalised);
    }

    #[test]
    fn ksg_mi_rejects_degenerate_signal() {
        let x = vec![vec![1.0f32]; MI_AUDIT_MIN_SAMPLE_SIZE];
        let y = (0..MI_AUDIT_MIN_SAMPLE_SIZE)
            .map(|idx| vec![idx as f32])
            .collect::<Vec<_>>();
        let err =
            estimate_ksg_1("constant", &x, "varying", &y, 3).expect_err("constant input must fail");
        assert_eq!(err.code(), "MI_AUDIT_DEGENERATE_INPUT");
    }
}
