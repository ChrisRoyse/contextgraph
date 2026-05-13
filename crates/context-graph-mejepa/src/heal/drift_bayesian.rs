use serde::{Deserialize, Serialize};

use crate::heal::errors::HealError;

const LANCZOS_COEFFICIENTS: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BayesianDriftDecision {
    pub successes: u64,
    pub trials: u64,
    pub threshold: f32,
    pub confidence: f32,
    pub alpha_post: f64,
    pub beta_post: f64,
    pub posterior_mean: f32,
    pub upper_credible_bound: f32,
    pub fires: bool,
}

pub fn jeffreys_posterior_below_threshold(
    successes: u64,
    trials: u64,
    threshold: f32,
    confidence: f32,
) -> Result<BayesianDriftDecision, HealError> {
    if trials == 0 {
        return Err(HealError::invalid(
            "bayesian_drift.trials",
            "trials must be greater than zero",
        ));
    }
    if successes > trials {
        return Err(HealError::invalid(
            "bayesian_drift.successes",
            format!("successes {successes} exceeds trials {trials}"),
        ));
    }
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(HealError::invalid(
            "bayesian_drift.threshold",
            format!("threshold must be finite in [0,1], got {threshold}"),
        ));
    }
    if !confidence.is_finite() || !(0.5..1.0).contains(&confidence) {
        return Err(HealError::invalid(
            "bayesian_drift.confidence",
            format!("confidence must be finite in [0.5,1), got {confidence}"),
        ));
    }
    let alpha_post = successes as f64 + 0.5;
    let beta_post = (trials - successes) as f64 + 0.5;
    let upper = beta_inverse_cdf(confidence as f64, alpha_post, beta_post)?;
    let posterior_mean = (alpha_post / (alpha_post + beta_post)) as f32;
    Ok(BayesianDriftDecision {
        successes,
        trials,
        threshold,
        confidence,
        alpha_post,
        beta_post,
        posterior_mean,
        upper_credible_bound: upper as f32,
        fires: upper < threshold as f64,
    })
}

fn beta_inverse_cdf(p: f64, a: f64, b: f64) -> Result<f64, HealError> {
    if !(0.0..=1.0).contains(&p) || !p.is_finite() {
        return Err(HealError::invalid(
            "beta_inverse_cdf.p",
            format!("p must be finite in [0,1], got {p}"),
        ));
    }
    if p == 0.0 {
        return Ok(0.0);
    }
    if p == 1.0 {
        return Ok(1.0);
    }
    let mut lo = 0.0f64;
    let mut hi = 1.0f64;
    for _ in 0..96 {
        let mid = (lo + hi) * 0.5;
        let cdf = regularized_incomplete_beta(mid, a, b)?;
        if cdf < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok((lo + hi) * 0.5)
}

fn regularized_incomplete_beta(x: f64, a: f64, b: f64) -> Result<f64, HealError> {
    if !x.is_finite() || !(0.0..=1.0).contains(&x) {
        return Err(HealError::invalid(
            "regularized_incomplete_beta.x",
            format!("x must be finite in [0,1], got {x}"),
        ));
    }
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 {
        return Err(HealError::invalid(
            "regularized_incomplete_beta.shape",
            format!("a and b must be positive finite values, got ({a}, {b})"),
        ));
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    if x == 1.0 {
        return Ok(1.0);
    }

    let log_bt = a * x.ln() + b * (1.0 - x).ln() - ln_beta(a, b);
    let bt = log_bt.exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        Ok((bt * beta_continued_fraction(x, a, b)? / a).clamp(0.0, 1.0))
    } else {
        Ok((1.0 - bt * beta_continued_fraction(1.0 - x, b, a)? / b).clamp(0.0, 1.0))
    }
}

fn beta_continued_fraction(x: f64, a: f64, b: f64) -> Result<f64, HealError> {
    let max_iter = 200;
    let eps = 3.0e-14;
    let fpmin = f64::MIN_POSITIVE / eps;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < fpmin {
        d = fpmin;
    }
    d = 1.0 / d;
    let mut h = d;

    for m in 1..=max_iter {
        let m2 = 2 * m;
        let mut aa = m as f64 * (b - m as f64) * x
            / ((qam + m2 as f64) * (a + m2 as f64));
        d = 1.0 + aa * d;
        if d.abs() < fpmin {
            d = fpmin;
        }
        c = 1.0 + aa / c;
        if c.abs() < fpmin {
            c = fpmin;
        }
        d = 1.0 / d;
        h *= d * c;

        aa = -((a + m as f64) * (qab + m as f64) * x)
            / ((a + m2 as f64) * (qap + m2 as f64));
        d = 1.0 + aa * d;
        if d.abs() < fpmin {
            d = fpmin;
        }
        c = 1.0 + aa / c;
        if c.abs() < fpmin {
            c = fpmin;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() <= eps {
            return Ok(h);
        }
    }
    Err(HealError::invalid(
        "regularized_incomplete_beta.convergence",
        format!("continued fraction did not converge for a={a}, b={b}, x={x}"),
    ))
}

fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

fn ln_gamma(z: f64) -> f64 {
    if z < 0.5 {
        return std::f64::consts::PI.ln()
            - (std::f64::consts::PI * z).sin().ln()
            - ln_gamma(1.0 - z);
    }
    let z = z - 1.0;
    let mut x = LANCZOS_COEFFICIENTS[0];
    for (idx, coefficient) in LANCZOS_COEFFICIENTS.iter().enumerate().skip(1) {
        x += coefficient / (z + idx as f64);
    }
    let t = z + 7.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (z + 0.5) * t.ln() - t + x.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jeffreys_small_window_does_not_fire_on_noisy_seven_of_ten() {
        let decision = jeffreys_posterior_below_threshold(7, 10, 0.85, 0.95).unwrap();
        assert!(!decision.fires);
        assert!(decision.upper_credible_bound > 0.85);
    }

    #[test]
    fn jeffreys_small_window_fires_on_zero_of_ten() {
        let decision = jeffreys_posterior_below_threshold(0, 10, 0.80, 0.95).unwrap();
        assert!(decision.fires);
        assert!(decision.upper_credible_bound < 0.80);
    }

    #[test]
    fn jeffreys_rejects_impossible_counts() {
        assert!(jeffreys_posterior_below_threshold(11, 10, 0.80, 0.95).is_err());
    }
}
