// Inspired by ruvnet/RuVector crates/ruvector-graph-transformer/src/temporal.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use super::*;

#[test]
fn rejects_zero_lag() {
    let err = granger_test(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0], GrangerConfig { lag: 0 })
        .expect_err("lag 0 must error");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn rejects_mismatched_lengths() {
    let err = granger_test(&[1.0, 2.0, 3.0, 4.0], &[1.0, 2.0], GrangerConfig { lag: 1 })
        .expect_err("length mismatch must error");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn rejects_insufficient_observations() {
    let err = granger_test(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0], GrangerConfig { lag: 2 })
        .expect_err("3 < 3*2+2 must error");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn rejects_raw_observation_count_that_cannot_leave_df_denominator() {
    let y = [1.0, 2.0, 3.0, 4.0];
    let x = [4.0, 3.0, 2.0, 1.0];
    let err = granger_test(&y, &x, GrangerConfig { lag: 1 })
        .expect_err("4 raw observations leaves zero denominator df");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn rejects_nan_in_series() {
    let err = granger_test(
        &[1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0],
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        GrangerConfig { lag: 1 },
    )
    .expect_err("NaN must error");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn x_granger_causes_y_when_y_lags_x() {
    let n = 200;
    let mut x = vec![0.0; n];
    let mut y = vec![0.0; n];
    for i in 0..n {
        x[i] = pseudo_normal(i as u64 + 1);
    }
    for t in 1..n {
        y[t] = 0.6 * x[t - 1] + 0.1 * pseudo_normal((t as u64) ^ 0xBEEF);
    }
    let report = granger_test(&y, &x, GrangerConfig { lag: 1 }).unwrap();
    assert!(report.f_statistic > 30.0);
    assert!(report.p_value < 1e-6);
}

#[test]
fn x_does_not_granger_cause_y_for_independent_series() {
    let n = 200;
    let mut y = vec![0.0; n];
    let mut x = vec![0.0; n];
    for i in 0..n {
        y[i] = pseudo_normal(i as u64 + 11);
        x[i] = pseudo_normal((i as u64) ^ 0x1234_5678);
    }
    let report = granger_test(&y, &x, GrangerConfig { lag: 1 }).unwrap();
    assert!(report.p_value > 0.01);
}

#[test]
fn reverse_direction_does_not_imply_x_causes_y() {
    let n = 200;
    let mut y = vec![0.0; n];
    let mut x = vec![0.0; n];
    for i in 0..n {
        y[i] = pseudo_normal(i as u64 + 7);
    }
    for t in 1..n {
        x[t] = 0.7 * y[t - 1] + 0.1 * pseudo_normal((t as u64) ^ 0xFACE);
    }
    let report = granger_test(&y, &x, GrangerConfig { lag: 1 }).unwrap();
    assert!(report.p_value > 0.01);
}

#[test]
fn ols_solves_known_2x2_system() {
    let n = 4;
    let k = 2;
    let design = vec![1.0, 0.0, 1.0, 1.0, 1.0, 2.0, 1.0, 3.0];
    let y = vec![1.0, 3.0, 5.0, 7.0];
    let beta = ols(&design, n, k, &y).unwrap();
    assert!((beta[0] - 1.0).abs() < 1e-9);
    assert!((beta[1] - 2.0).abs() < 1e-9);
}

#[test]
fn ols_rejects_singular_design() {
    let n = 4;
    let k = 2;
    let design = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
    let y = vec![1.0, 2.0, 3.0, 4.0];
    let err = ols(&design, n, k, &y).expect_err("singular must error");
    assert_eq!(err.code(), "CGSOLVER_NUMERICAL_INVARIANT");
}

#[test]
fn f_upper_tail_known_values() {
    let p = f_distribution_upper_tail(3.32, 2, 30).unwrap();
    assert!((p - 0.05).abs() < 0.005);
    let p = f_distribution_upper_tail(3.94, 1, 100).unwrap();
    assert!((p - 0.05).abs() < 0.01);
    let nan_err = f_distribution_upper_tail(f64::NAN, 1, 1).expect_err("NaN must error");
    assert_eq!(nan_err.code(), "CGSOLVER_INVALID_INPUT");
    let df_err = f_distribution_upper_tail(1.0, 0, 1).expect_err("df=0 must error");
    assert_eq!(df_err.code(), "CGSOLVER_INVALID_INPUT");
    assert_eq!(f_distribution_upper_tail(f64::INFINITY, 1, 1).unwrap(), 0.0);
    assert_eq!(f_distribution_upper_tail(0.0, 1, 1).unwrap(), 1.0);
}

#[test]
fn incomplete_beta_rejects_invalid_inputs() {
    let err = regularized_incomplete_beta(f64::NAN, 2.0, 3.0).expect_err("NaN x must error");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    let err =
        regularized_incomplete_beta(0.5, 0.0, 3.0).expect_err("non-positive shape must error");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn granger_rejects_degenerate_zero_residual_models() {
    let y = [1.0, 1.0, 1.0, 1.0, 1.0];
    let x = [2.0, 2.0, 2.0, 2.0, 2.0];
    let err = granger_test(&y, &x, GrangerConfig { lag: 1 })
        .expect_err("both models fit constant series exactly");
    assert_eq!(err.code(), "CGSOLVER_NUMERICAL_INVARIANT");
}

#[test]
fn ln_gamma_known_values() {
    assert!(ln_gamma(1.0).abs() < 1e-9);
    assert!(ln_gamma(2.0).abs() < 1e-9);
    assert!((ln_gamma(5.0) - 24.0_f64.ln()).abs() < 1e-7);
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn pseudo_normal(seed: u64) -> f64 {
    let r1 = splitmix64(seed.wrapping_mul(2)) >> 11;
    let r2 = splitmix64(seed.wrapping_mul(2).wrapping_add(1)) >> 11;
    let u1 = (r1 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
    let u2 = (r2 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = 2.0 * std::f64::consts::PI * u2;
    r * theta.cos()
}
