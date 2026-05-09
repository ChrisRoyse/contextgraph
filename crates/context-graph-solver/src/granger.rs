// Inspired by ruvnet/RuVector crates/ruvector-graph-transformer/src/temporal.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Bivariate Granger causality F-test.
//
// Algorithm:
//   For two stationary time series Y and X with lag order p, compare:
//     Restricted   :  Y(t) = α₀ + Σᵢ αᵢ Y(t-i)                     + ε    (no X)
//     Unrestricted :  Y(t) = α₀ + Σᵢ αᵢ Y(t-i) + Σⱼ βⱼ X(t-j)       + ε    (with X)
//   Compute residual sum of squares for each (SSR_r, SSR_u) over n
//   usable observations (n = T - p where T = series length). The test
//   statistic is:
//     F = ((SSR_r - SSR_u) / p) / (SSR_u / (n - 2p - 1))
//   Under the null hypothesis "X does NOT Granger-cause Y," F follows the
//   F-distribution with (p, n - 2p - 1) degrees of freedom. We compute the
//   p-value as the upper-tail probability and let the caller compare it
//   against their chosen significance level (typical α = 0.05).
//
// References:
//   Granger, "Investigating causal relations by econometric models and
//   cross-spectral methods," Econometrica 37(3) 1969.
//   Press, Teukolsky, Vetterling & Flannery, "Numerical Recipes 3rd ed.,"
//   §6.4 for the regularized incomplete beta function used in the p-value
//   computation.
//
// Limitations and caveats (caller responsibility):
//   - Series MUST be stationary; non-stationary series produce inflated F's.
//   - Lag p MUST be chosen on a-priori grounds or via AIC/BIC; we do not
//     auto-select p.
//   - The test detects linear Granger causality only.

use crate::error::{SolverError, SolverResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GrangerConfig {
    /// Number of lags to include (must be ≥ 1).
    pub lag: usize,
}

impl Default for GrangerConfig {
    fn default() -> Self {
        Self { lag: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrangerReport {
    pub lag: usize,
    /// Number of usable observations after lagging (n = T - lag).
    pub n_observations: usize,
    /// Residual sum of squares for the restricted (no-X) model.
    pub ssr_restricted: f64,
    /// Residual sum of squares for the unrestricted (with-X) model.
    pub ssr_unrestricted: f64,
    /// F-statistic of the joint nullity test on β coefficients.
    pub f_statistic: f64,
    /// Numerator degrees of freedom = lag.
    pub df_numerator: usize,
    /// Denominator degrees of freedom = n - 2*lag - 1.
    pub df_denominator: usize,
    /// Upper-tail p-value: P(F_(p, n-2p-1) ≥ f_statistic | H₀).
    pub p_value: f64,
}

/// Test whether `x` Granger-causes `y` at the given lag.
/// Both series must have the same length and at least `3 * lag + 2`
/// raw observations. After dropping the first `lag` rows, this leaves
/// `n_observations >= 2*lag + 2`, so the unrestricted regression has
/// positive residual degrees of freedom.
pub fn granger_test(y: &[f64], x: &[f64], config: GrangerConfig) -> SolverResult<GrangerReport> {
    let lag = config.lag;
    if lag == 0 {
        return Err(SolverError::invalid(
            "lag",
            "Granger lag must be ≥ 1",
            "set lag to at least 1",
        ));
    }
    if y.len() != x.len() {
        return Err(SolverError::invalid(
            "series",
            format!("y len {} != x len {}", y.len(), x.len()),
            "pass parallel time series of identical length",
        ));
    }
    let t = y.len();
    let min_required = lag
        .checked_mul(3)
        .and_then(|v| v.checked_add(2))
        .ok_or_else(|| {
            SolverError::invalid(
                "lag",
                format!("lag {lag} is too large to compute required observation count"),
                "choose a smaller lag",
            )
        })?;
    if t < min_required {
        return Err(SolverError::invalid(
            "series",
            format!("Granger requires ≥ 3*lag+2 = {min_required} observations; got {t}"),
            "supply a longer time series or reduce lag",
        ));
    }
    for (idx, val) in y.iter().chain(x.iter()).enumerate() {
        if !val.is_finite() {
            return Err(SolverError::invalid(
                "series",
                format!("non-finite series value at flat index {idx}: {val}"),
                "filter or impute non-finite values before testing",
            ));
        }
    }

    let n = t - lag; // usable observations after lagging
    let p = lag;

    // Build restricted design matrix: [1, Y(t-1), ..., Y(t-p)] over rows
    // t = lag .. T-1. Columns: 1 + p.
    let cols_r = 1 + p;
    let mut design_r = vec![0.0; n * cols_r];
    let mut response = vec![0.0; n];
    for (row, t_idx) in (lag..t).enumerate() {
        design_r[row * cols_r] = 1.0;
        for i in 1..=p {
            design_r[row * cols_r + i] = y[t_idx - i];
        }
        response[row] = y[t_idx];
    }
    let beta_r = ols(&design_r, n, cols_r, &response)?;
    let ssr_restricted = ssr(&design_r, n, cols_r, &response, &beta_r);

    // Build unrestricted design matrix: [1, Y(t-1), ..., Y(t-p), X(t-1), ..., X(t-p)].
    let cols_u = 1 + 2 * p;
    let mut design_u = vec![0.0; n * cols_u];
    for (row, t_idx) in (lag..t).enumerate() {
        design_u[row * cols_u] = 1.0;
        for i in 1..=p {
            design_u[row * cols_u + i] = y[t_idx - i];
        }
        for j in 1..=p {
            design_u[row * cols_u + p + j] = x[t_idx - j];
        }
    }
    let beta_u = ols(&design_u, n, cols_u, &response)?;
    let ssr_unrestricted = ssr(&design_u, n, cols_u, &response, &beta_u);

    let df_num = p;
    let unrestricted_params = p
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| {
            SolverError::invalid(
                "lag",
                format!("lag {p} is too large to compute unrestricted parameter count"),
                "choose a smaller lag",
            )
        })?;
    let df_den = n.saturating_sub(unrestricted_params);
    if df_den == 0 {
        return Err(SolverError::invalid(
            "series",
            format!("denominator degrees of freedom is 0; need n > 2*lag+1 (n={n}, lag={p})"),
            "supply more observations or reduce lag",
        ));
    }

    let ssr_delta = ssr_restricted - ssr_unrestricted;
    let tolerance = 1.0e-10 * (1.0 + ssr_restricted.abs().max(ssr_unrestricted.abs()));
    if ssr_delta < -tolerance {
        return Err(SolverError::invariant(
            "OLS",
            format!(
                "unrestricted SSR exceeded restricted SSR by {}; restricted={}, unrestricted={}",
                -ssr_delta, ssr_restricted, ssr_unrestricted
            ),
            "check the design matrix conditioning and input stationarity",
        ));
    }
    let numerator = ssr_delta.max(0.0) / df_num as f64;
    let denom = ssr_unrestricted / df_den as f64;
    let f_statistic = if denom == 0.0 {
        if numerator == 0.0 {
            return Err(SolverError::invariant(
                "OLS",
                "restricted and unrestricted residuals are both exactly zero; F statistic is undefined",
                "use a less degenerate series or add noise before testing",
            ));
        }
        // Unrestricted residuals are exactly zero and restricted residuals are not:
        // perfect incremental fit. F → ∞; p-value 0.
        f64::INFINITY
    } else {
        numerator / denom
    };

    let p_value = f_distribution_upper_tail(f_statistic, df_num, df_den)?;

    Ok(GrangerReport {
        lag,
        n_observations: n,
        ssr_restricted,
        ssr_unrestricted,
        f_statistic,
        df_numerator: df_num,
        df_denominator: df_den,
        p_value,
    })
}

/// Solve the OLS normal equations (Xᵀ X) β = Xᵀ y by Gaussian elimination
/// with partial pivoting. Suitable for the small (k = 1 + 2*lag) systems
/// typical of Granger tests; for larger problems we would route through
/// the conjugate-gradient solver.
fn ols(design: &[f64], n: usize, k: usize, y: &[f64]) -> SolverResult<Vec<f64>> {
    let xtx = mat_mat_t(design, n, k);
    let xty = mat_t_vec(design, n, k, y);
    gaussian_solve(xtx, k, xty)
}

fn ssr(design: &[f64], n: usize, k: usize, y: &[f64], beta: &[f64]) -> f64 {
    let mut total = 0.0;
    for row in 0..n {
        let mut yhat = 0.0;
        for j in 0..k {
            yhat += design[row * k + j] * beta[j];
        }
        let resid = y[row] - yhat;
        total += resid * resid;
    }
    total
}

/// Compute Xᵀ X for an n×k row-major matrix X. Returns a k×k row-major matrix.
fn mat_mat_t(design: &[f64], n: usize, k: usize) -> Vec<f64> {
    let mut out = vec![0.0; k * k];
    for row in 0..n {
        for i in 0..k {
            let xi = design[row * k + i];
            for j in 0..k {
                out[i * k + j] += xi * design[row * k + j];
            }
        }
    }
    out
}

/// Compute Xᵀ y for an n×k row-major matrix X. Returns a k-vector.
fn mat_t_vec(design: &[f64], n: usize, k: usize, y: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; k];
    for row in 0..n {
        for j in 0..k {
            out[j] += design[row * k + j] * y[row];
        }
    }
    out
}

/// Solve A x = b by in-place Gaussian elimination with partial pivoting.
/// Fail-closed if A is singular within tolerance.
fn gaussian_solve(mut a: Vec<f64>, k: usize, mut b: Vec<f64>) -> SolverResult<Vec<f64>> {
    for col in 0..k {
        // Partial pivot: find the row with max |a[r,col]| for r ≥ col.
        let mut pivot = col;
        let mut pivot_abs = a[col * k + col].abs();
        for row in (col + 1)..k {
            let v = a[row * k + col].abs();
            if v > pivot_abs {
                pivot_abs = v;
                pivot = row;
            }
        }
        if pivot_abs < 1e-12 {
            return Err(SolverError::invariant(
                "OLS",
                format!("normal-equation matrix is singular at column {col}"),
                "the design matrix has collinear columns; reduce lag or check stationarity",
            ));
        }
        if pivot != col {
            for j in 0..k {
                a.swap(col * k + j, pivot * k + j);
            }
            b.swap(col, pivot);
        }
        // Eliminate below pivot.
        for row in (col + 1)..k {
            let factor = a[row * k + col] / a[col * k + col];
            for j in col..k {
                a[row * k + j] -= factor * a[col * k + j];
            }
            b[row] -= factor * b[col];
        }
    }
    // Back-substitution.
    let mut x = vec![0.0; k];
    for row in (0..k).rev() {
        let mut sum = b[row];
        for j in (row + 1)..k {
            sum -= a[row * k + j] * x[j];
        }
        x[row] = sum / a[row * k + row];
    }
    Ok(x)
}

/// Upper-tail p-value of the F-distribution with given degrees of freedom:
///   P(F ≥ f) = 1 - I_{(d1*f) / (d1*f + d2)}(d1/2, d2/2)
/// We compute the regularized incomplete beta function via continued
/// fraction (Lentz's method) — standard textbook approach. Fail-closed:
/// returns Err on `betacf` non-convergence.
///
/// Edge cases:
/// - `f` NaN → returns NaN (propagates).
/// - `f` infinite → returns 0.0 (upper-tail at +∞).
/// - `f` ≤ 0     → returns 1.0 (upper-tail at 0 or below).
pub fn f_distribution_upper_tail(f: f64, df_num: usize, df_den: usize) -> SolverResult<f64> {
    if df_num == 0 || df_den == 0 {
        return Err(SolverError::invalid(
            "degrees_of_freedom",
            format!(
                "F distribution requires positive degrees of freedom; got ({df_num}, {df_den})"
            ),
            "pass positive numerator and denominator degrees of freedom",
        ));
    }
    if f.is_nan() {
        return Err(SolverError::invalid(
            "f",
            "F statistic is NaN",
            "check upstream regression residuals before computing a p-value",
        ));
    }
    if f.is_infinite() {
        return Ok(0.0);
    }
    if f <= 0.0 {
        return Ok(1.0);
    }
    let d1 = df_num as f64;
    let d2 = df_den as f64;
    let x = (d1 * f) / (d1 * f + d2);
    Ok(1.0 - regularized_incomplete_beta(x, d1 / 2.0, d2 / 2.0)?)
}

/// Regularized incomplete beta function I_x(a, b).
/// Uses the continued fraction expansion documented in Numerical Recipes
/// (Section 6.4, "Incomplete Beta Function"). For x small relative to
/// (a+1)/(a+b+2) we evaluate in the lower-tail orientation; otherwise in
/// the symmetric form 1 - I_{1-x}(b, a). Fail-closed if Lentz's
/// continued-fraction iteration does not converge within
/// `BETACF_MAX_ITER` steps.
pub fn regularized_incomplete_beta(x: f64, a: f64, b: f64) -> SolverResult<f64> {
    if !x.is_finite() || !(0.0..=1.0).contains(&x) {
        return Err(SolverError::invalid(
            "x",
            format!("regularized incomplete beta requires finite x in [0,1]; got {x}"),
            "clamp or reject invalid probability inputs before calling",
        ));
    }
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 {
        return Err(SolverError::invalid(
            "shape",
            format!("regularized incomplete beta requires positive finite a,b; got ({a}, {b})"),
            "pass positive finite shape parameters",
        ));
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    if x == 1.0 {
        return Ok(1.0);
    }
    // bt = x^a * (1-x)^b / B(a,b), computed in log space for numerical safety.
    let log_bt = a * x.ln() + b * (1.0 - x).ln() - ln_beta_complete(a, b);
    let bt_exp = log_bt.exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        Ok(bt_exp * betacf(x, a, b)? / a)
    } else {
        Ok(1.0 - bt_exp * betacf(1.0 - x, b, a)? / b)
    }
}

const BETACF_MAX_ITER: usize = 200;
const BETACF_EPS: f64 = 3.0e-15;
const BETACF_FPMIN: f64 = 1.0e-300;

/// Continued-fraction expansion for the incomplete beta function I_x(a,b).
/// Implements Lentz's modified method to avoid underflow. Fail-closed:
/// returns Err if the iteration does not converge within
/// `BETACF_MAX_ITER` steps.
fn betacf(x: f64, a: f64, b: f64) -> SolverResult<f64> {
    let max_iter = BETACF_MAX_ITER;
    let eps = BETACF_EPS;
    let fpmin = BETACF_FPMIN;
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
        let m_f = m as f64;
        let m2 = 2.0 * m_f;
        // Even step
        let aa = m_f * (b - m_f) * x / ((qam + m2) * (a + m2));
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
        // Odd step
        let aa = -(a + m_f) * (qab + m_f) * x / ((a + m2) * (qap + m2));
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
        if (del - 1.0).abs() < eps {
            return Ok(h);
        }
    }
    Err(SolverError::DidNotConverge {
        message: format!(
            "betacf Lentz iteration did not converge within {max_iter} steps (a={a}, b={b}, x={x})"
        ),
        remediation: "increase BETACF_MAX_ITER or check inputs for pathological values",
    })
}

/// ln B(a, b) = ln Γ(a) + ln Γ(b) - ln Γ(a+b), via Lanczos lgamma.
fn ln_beta_complete(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// Lanczos approximation of ln Γ(x). Coefficients from Numerical Recipes
/// 3rd ed. §6.1, accurate to about 1e-10 for x > 0.
fn ln_gamma(x: f64) -> f64 {
    let coefs = [
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let g = 7.0;
    if x < 0.5 {
        // Reflection formula: Γ(x)Γ(1-x) = π/sin(πx)
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = 0.999_999_999_999_809_9;
    let t = x + g + 0.5;
    for (i, c) in coefs.iter().enumerate() {
        a += c / (x + (i as f64 + 1.0));
    }
    let half_ln_2pi = 0.5 * (2.0 * std::f64::consts::PI).ln();
    half_ln_2pi + (x + 0.5) * t.ln() - t + a.ln()
}

#[cfg(test)]
#[path = "granger_tests.rs"]
mod granger_tests;
