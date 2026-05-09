// Manual Full State Verification for Granger causality test.
// Writes JSON evidence to /tmp/contextgraph-tier2-fsv/.

use context_graph_solver::{
    f_distribution_upper_tail, granger_test, regularized_incomplete_beta, GrangerConfig,
};
use std::fs;
use std::path::PathBuf;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-tier2-fsv");
    fs::create_dir_all(&evidence_root)?;

    let n = 200usize;

    // Case 1: X strongly Granger-causes Y. Y(t) = 0.6 * X(t-1) + small noise.
    let mut x_a = vec![0.0; n];
    let mut y_a = vec![0.0; n];
    for i in 0..n {
        x_a[i] = pseudo_normal(i as u64 + 1);
    }
    for t in 1..n {
        y_a[t] = 0.6 * x_a[t - 1] + 0.1 * pseudo_normal((t as u64) ^ 0x1234_BEEF);
    }
    let report_a = granger_test(&y_a, &x_a, GrangerConfig { lag: 1 })?;

    // Case 2: independent series. p-value should be uniformly distributed —
    // far from significance.
    let mut y_b = vec![0.0; n];
    let mut x_b = vec![0.0; n];
    for i in 0..n {
        y_b[i] = pseudo_normal(i as u64 + 11);
        x_b[i] = pseudo_normal((i as u64) ^ 0x1234_5678);
    }
    let report_b = granger_test(&y_b, &x_b, GrangerConfig { lag: 1 })?;

    // Case 3: reverse-causal direction. X is built from Y; testing "does X
    // cause Y?" should NOT reject (Y is independent given its own past).
    let mut y_c = vec![0.0; n];
    let mut x_c = vec![0.0; n];
    for i in 0..n {
        y_c[i] = pseudo_normal(i as u64 + 7);
    }
    for t in 1..n {
        x_c[t] = 0.7 * y_c[t - 1] + 0.1 * pseudo_normal((t as u64) ^ 0xFACE);
    }
    let report_c = granger_test(&y_c, &x_c, GrangerConfig { lag: 1 })?;

    // F-distribution critical values from textbook tables.
    let f_2_30_at_3p32 = f_distribution_upper_tail(3.32, 2, 30)?;
    let f_1_100_at_3p94 = f_distribution_upper_tail(3.94, 1, 100)?;

    // Boundary 1: zero lag rejects.
    let zero_lag_code = match granger_test(&y_a, &x_a, GrangerConfig { lag: 0 }) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary 2: mismatched lengths reject.
    let len_mismatch_code = match granger_test(&[1.0, 2.0, 3.0], &[1.0], GrangerConfig { lag: 1 }) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary 3: NaN in series rejects.
    let nan_y = vec![1.0, f64::NAN, 3.0, 4.0, 5.0, 6.0];
    let nan_x = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let nan_code = match granger_test(&nan_y, &nan_x, GrangerConfig { lag: 1 }) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let too_short_for_df_code = match granger_test(
        &[1.0, 2.0, 3.0, 4.0],
        &[4.0, 3.0, 2.0, 1.0],
        GrangerConfig { lag: 1 },
    ) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let invalid_f_code = match f_distribution_upper_tail(f64::NAN, 1, 1) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Sanity: I_x(a, b) at endpoints equals known values.
    let beta_at_zero = regularized_incomplete_beta(0.0, 2.0, 3.0)?;
    let beta_at_one = regularized_incomplete_beta(1.0, 2.0, 3.0)?;

    let evidence = serde_json::json!({
        "case_x_causes_y_strong": {
            "lag": report_a.lag,
            "n_observations": report_a.n_observations,
            "f_statistic": report_a.f_statistic,
            "df_num": report_a.df_numerator,
            "df_den": report_a.df_denominator,
            "p_value": report_a.p_value,
            "passes": report_a.p_value < 1e-6,
            "ssr_restricted": report_a.ssr_restricted,
            "ssr_unrestricted": report_a.ssr_unrestricted,
        },
        "case_independent_series": {
            "p_value": report_b.p_value,
            "passes": report_b.p_value > 0.01,
            "f_statistic": report_b.f_statistic,
        },
        "case_reverse_direction_does_not_imply_x_causes_y": {
            "p_value": report_c.p_value,
            "passes": report_c.p_value > 0.01,
            "f_statistic": report_c.f_statistic,
        },
        "f_distribution_textbook_critical_values": {
            "F_2_30_at_3p32": {
                "expected": 0.05,
                "actual": f_2_30_at_3p32,
                "tolerance": 0.005,
                "passes": (f_2_30_at_3p32 - 0.05).abs() < 0.005,
            },
            "F_1_100_at_3p94": {
                "expected": 0.05,
                "actual": f_1_100_at_3p94,
                "tolerance": 0.01,
                "passes": (f_1_100_at_3p94 - 0.05).abs() < 0.01,
            },
        },
        "regularized_incomplete_beta_endpoints": {
            "I_0_a2_b3": beta_at_zero,
            "I_1_a2_b3": beta_at_one,
            "passes": beta_at_zero == 0.0 && beta_at_one == 1.0,
        },
        "boundary_zero_lag_rejected": {
            "before": {
                "lag": 0,
                "series_len_y": y_a.len(),
                "series_len_x": x_a.len(),
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": zero_lag_code,
            },
        },
        "boundary_length_mismatch_rejected": {
            "before": {
                "lag": 1,
                "series_len_y": 3,
                "series_len_x": 1,
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": len_mismatch_code,
            },
        },
        "boundary_nan_rejected": {
            "before": {
                "lag": 1,
                "series_len_y": nan_y.len(),
                "series_len_x": nan_x.len(),
                "contains_nan": true,
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": nan_code,
            },
        },
        "boundary_too_short_for_df_rejected": {
            "before": {
                "lag": 1,
                "series_len_y": 4,
                "series_len_x": 4,
                "minimum_required_raw_observations": 5,
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": too_short_for_df_code,
            },
        },
        "boundary_nan_f_statistic_rejected": {
            "before": {
                "f_statistic": "NaN",
                "df_num": 1,
                "df_den": 1,
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": invalid_f_code,
            },
        },
    });

    let path = evidence_root.join("granger-evidence.json");
    fs::write(&path, serde_json::to_string_pretty(&evidence)?)?;
    let readback_text = fs::read_to_string(&path)?;
    let readback: serde_json::Value = serde_json::from_str(&readback_text)?;
    assert_eq!(readback, evidence);
    println!("Granger FSV evidence: {}", path.display());
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}
