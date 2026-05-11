use super::error::{EvalError, EvalErrorCode};
use super::types::{
    language_slug, ConformalHealthEntry, EvalConfig, EvalObservation, MutationCategory,
    RegressionCheck,
};
use crate::types::{Language, OracleOutcome};
use std::collections::BTreeMap;

pub type CorrelationBreakdown = (
    Option<f32>,
    BTreeMap<MutationCategory, Option<f32>>,
    BTreeMap<Language, Option<f32>>,
    BTreeMap<String, Option<f32>>,
);

pub fn pearson_correlation(xs: &[f32], ys: &[f32]) -> Result<Option<f32>, EvalError> {
    if xs.len() != ys.len() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("pearson length mismatch: {} vs {}", xs.len(), ys.len()),
        ));
    }
    if xs.len() < 2 {
        return Ok(None);
    }
    for (idx, (x, y)) in xs.iter().zip(ys).enumerate() {
        if !x.is_finite() || !y.is_finite() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("pearson non-finite at {idx}: {x}, {y}"),
            ));
        }
    }
    let n = xs.len() as f32;
    let mean_x = xs.iter().sum::<f32>() / n;
    let mean_y = ys.iter().sum::<f32>() / n;
    let mut cov = 0.0f32;
    let mut var_x = 0.0f32;
    let mut var_y = 0.0f32;
    for (x, y) in xs.iter().zip(ys) {
        let dx = x - mean_x;
        let dy = y - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    if var_x <= f32::EPSILON || var_y <= f32::EPSILON {
        return Ok(None);
    }
    Ok(Some((cov / (var_x.sqrt() * var_y.sqrt())).clamp(-1.0, 1.0)))
}

pub fn oracle_target(outcome: OracleOutcome) -> Result<f32, EvalError> {
    match outcome {
        OracleOutcome::Pass => Ok(1.0),
        OracleOutcome::Fail => Ok(0.0),
        OracleOutcome::OutOfDistribution | OracleOutcome::Abstain => Err(EvalError::new(
            EvalErrorCode::OracleMissing,
            format!("actual oracle outcome {outcome:?} cannot be reduced to pass/fail"),
        )),
    }
}

pub fn oracle_target_opt(outcome: OracleOutcome) -> Option<f32> {
    match outcome {
        OracleOutcome::Pass => Some(1.0),
        OracleOutcome::Fail => Some(0.0),
        OracleOutcome::OutOfDistribution | OracleOutcome::Abstain => None,
    }
}

pub fn compute_correlations(
    observations: &[EvalObservation],
    config: &EvalConfig,
) -> Result<CorrelationBreakdown, EvalError> {
    let mut predicted = Vec::with_capacity(observations.len());
    let mut actual = Vec::with_capacity(observations.len());
    for obs in observations {
        if let Some(target) = oracle_target_opt(obs.actual_oracle) {
            predicted.push(obs.prediction.predicted_oracle_pass);
            actual.push(target);
        }
    }
    let overall = pearson_correlation(&predicted, &actual)?;

    let mut per_category = BTreeMap::new();
    for category in MutationCategory::all() {
        let filtered = observations
            .iter()
            .filter(|obs| obs.mutation_category == category)
            .collect::<Vec<_>>();
        per_category.insert(category, slice_correlation(&filtered, config)?);
    }

    let mut per_language = BTreeMap::new();
    for language in languages_in(observations) {
        let filtered = observations
            .iter()
            .filter(|obs| obs.language == language)
            .collect::<Vec<_>>();
        per_language.insert(language, slice_correlation(&filtered, config)?);
    }

    let mut per_cell = BTreeMap::new();
    for category in MutationCategory::all() {
        for language in languages_in(observations) {
            let filtered = observations
                .iter()
                .filter(|obs| obs.mutation_category == category && obs.language == language)
                .collect::<Vec<_>>();
            let key = format!("{}::{}", category.slug(), language_slug(language));
            per_cell.insert(key, slice_correlation(&filtered, config)?);
        }
    }
    Ok((overall, per_category, per_language, per_cell))
}

pub fn conformal_health(
    observations: &[EvalObservation],
    config: &EvalConfig,
) -> Result<BTreeMap<Language, ConformalHealthEntry>, EvalError> {
    let mut out = BTreeMap::new();
    for language in languages_in(observations) {
        let lang_obs = observations
            .iter()
            .filter(|obs| obs.language == language)
            .collect::<Vec<_>>();
        if lang_obs.is_empty() {
            continue;
        }
        let mut covered = 0usize;
        for obs in &lang_obs {
            if obs
                .prediction
                .outcome_set
                .outcomes
                .contains(&obs.actual_oracle)
            {
                covered += 1;
            }
        }
        let empirical = covered as f32 / lang_obs.len() as f32;
        out.insert(
            language,
            ConformalHealthEntry {
                expected_coverage: config.conformal_expected_coverage,
                empirical_coverage: empirical,
                sample_count: lang_obs.len(),
                within_band: empirical + config.conformal_band
                    >= config.conformal_expected_coverage,
            },
        );
    }
    Ok(out)
}

pub fn ood_auc_by_language(
    observations: &[EvalObservation],
) -> Result<BTreeMap<Language, Option<f32>>, EvalError> {
    let mut out = BTreeMap::new();
    for language in languages_in(observations) {
        let positives = observations
            .iter()
            .filter(|obs| {
                obs.language == language && obs.actual_oracle == OracleOutcome::OutOfDistribution
            })
            .map(|obs| obs.prediction.ood_score)
            .collect::<Vec<_>>();
        let negatives = observations
            .iter()
            .filter(|obs| {
                obs.language == language && obs.actual_oracle != OracleOutcome::OutOfDistribution
            })
            .map(|obs| obs.prediction.ood_score)
            .collect::<Vec<_>>();
        out.insert(language, auc_pairwise(&positives, &negatives)?);
    }
    Ok(out)
}

pub fn gtau_pass_rate_by_language(
    observations: &[EvalObservation],
) -> Result<BTreeMap<Language, f32>, EvalError> {
    let mut out = BTreeMap::new();
    for language in languages_in(observations) {
        let lang_obs = observations
            .iter()
            .filter(|obs| obs.language == language)
            .collect::<Vec<_>>();
        if lang_obs.is_empty() {
            continue;
        }
        let passed = lang_obs.iter().filter(|obs| obs.gtau_passed).count();
        out.insert(language, passed as f32 / lang_obs.len() as f32);
    }
    Ok(out)
}

pub fn bayesian_shrinkage(
    raw: &BTreeMap<String, Option<f32>>,
    global: Option<f32>,
    prior_strength: f32,
) -> Result<BTreeMap<String, f32>, EvalError> {
    if !prior_strength.is_finite() || prior_strength < 0.0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("prior_strength must be finite and non-negative; got {prior_strength}"),
        ));
    }
    let prior = global.unwrap_or(0.0).clamp(-1.0, 1.0);
    let mut out = BTreeMap::new();
    for (key, value) in raw {
        let observed = value.unwrap_or(prior).clamp(-1.0, 1.0);
        let shrunk =
            ((observed + prior_strength * prior) / (1.0 + prior_strength)).clamp(-1.0, 1.0);
        out.insert(key.clone(), (shrunk + 1.0) / 2.0);
    }
    Ok(out)
}

pub struct ShipGateInputs<'a> {
    pub observations: &'a [EvalObservation],
    pub report_overall: Option<f32>,
    pub conformal: &'a BTreeMap<Language, ConformalHealthEntry>,
    pub ood: &'a BTreeMap<Language, Option<f32>>,
    pub gtau: &'a BTreeMap<Language, f32>,
    pub regression_checks: &'a [RegressionCheck],
    pub q1_pass_rate: f32,
    pub q3_side_effect_agreement: Option<f32>,
    pub config: &'a EvalConfig,
}

pub fn ship_gate_failures(inputs: ShipGateInputs<'_>) -> Vec<String> {
    let mut failures = Vec::new();
    if inputs.observations.is_empty() {
        failures.push("empty_holdout".to_string());
    }
    match inputs.report_overall {
        Some(value) if value >= inputs.config.correlation_min => {}
        Some(value) => failures.push(format!(
            "overall_correlation {value:.6} < {:.6}",
            inputs.config.correlation_min
        )),
        None => failures.push("overall_correlation unavailable".to_string()),
    }
    for (language, health) in inputs.conformal {
        if !health.within_band {
            failures.push(format!("conformal_coverage {:?} outside band", language));
        }
    }
    for (language, auc) in inputs.ood {
        if let Some(value) = auc {
            if *value < inputs.config.ood_auc_min {
                failures.push(format!("ood_auc {:?} {value:.6} below threshold", language));
            }
        }
    }
    for (language, value) in inputs.gtau {
        if *value < inputs.config.gtau_pass_rate_min {
            failures.push(format!(
                "gtau_pass_rate {:?} {value:.6} below threshold",
                language
            ));
        }
    }
    if inputs.q1_pass_rate < inputs.config.q1_pass_rate_min {
        failures.push(format!(
            "q1_pass_rate {:.6} below threshold {:.6}",
            inputs.q1_pass_rate, inputs.config.q1_pass_rate_min
        ));
    }
    match inputs.q3_side_effect_agreement {
        Some(value) if value >= inputs.config.q3_side_effect_min => {}
        Some(value) => failures.push(format!(
            "q3_side_effect_agreement {value:.6} below threshold {:.6}",
            inputs.config.q3_side_effect_min
        )),
        None => failures.push("q3_side_effect_agreement unavailable".to_string()),
    }
    for check in inputs.regression_checks {
        if !check.passed {
            failures.push(format!(
                "regression_check {} drop {:.6} exceeds threshold {:.6}",
                check.name, check.drop, inputs.config.regression_max_drop
            ));
        }
    }
    failures
}

fn slice_correlation(
    observations: &[&EvalObservation],
    config: &EvalConfig,
) -> Result<Option<f32>, EvalError> {
    if observations.len() < config.min_samples_per_slice {
        return Ok(None);
    }
    let mut predicted = Vec::with_capacity(observations.len());
    let mut actual = Vec::with_capacity(observations.len());
    for obs in observations {
        if let Some(target) = oracle_target_opt(obs.actual_oracle) {
            predicted.push(obs.prediction.predicted_oracle_pass);
            actual.push(target);
        }
    }
    pearson_correlation(&predicted, &actual)
}

fn auc_pairwise(positives: &[f32], negatives: &[f32]) -> Result<Option<f32>, EvalError> {
    if positives.is_empty() || negatives.is_empty() {
        return Ok(None);
    }
    let mut score = 0.0f32;
    let mut pairs = 0usize;
    for pos in positives {
        for neg in negatives {
            if !pos.is_finite() || !neg.is_finite() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    "AUC received non-finite score",
                ));
            }
            score += if pos > neg {
                1.0
            } else if (*pos - *neg).abs() <= f32::EPSILON {
                0.5
            } else {
                0.0
            };
            pairs += 1;
        }
    }
    Ok(Some(score / pairs as f32))
}

fn languages_in(observations: &[EvalObservation]) -> Vec<Language> {
    let mut languages = observations
        .iter()
        .map(|obs| obs.language)
        .collect::<Vec<_>>();
    languages.sort();
    languages.dedup();
    languages
}
