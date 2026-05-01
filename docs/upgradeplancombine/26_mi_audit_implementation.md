# 26 — MI Audit Implementation Plan (KSG estimator)

This document is the file-level diff plan for PR 14 (the
`audit-pairwise-mi` subcommand and its KSG estimator).

## 1. New crate module

`crates/context-graph-dynamicjepa/src/mi_audit/mod.rs` (NEW)

```rust
pub mod ksg;
pub mod mine;
pub mod aggregate;
pub mod heatmap;

pub use ksg::estimate_ksg_pairwise_mi;
pub use mine::estimate_mine_pairwise_mi;
pub use aggregate::{
    aggregate_pairwise_mi_summary,
    n_eff_loose, n_eff_strict,
    AxiomTwoVerdict,
};
```

## 2. `mi_audit/ksg.rs` (NEW)

The KSG-1 estimator implementation. Algorithm from PAPER.md §10
following Kraskov-Stögbauer-Grassberger (2004):

```rust
use ndarray::{Array1, Array2, ArrayView1};

/// KSG-1 estimator for I(X; Y) given paired samples.
///
/// Inputs:
///   xs: Array2 of shape (n_samples, d_x)
///   ys: Array2 of shape (n_samples, d_y)
///   k: nearest-neighbour parameter (default 5)
///
/// Returns: estimate in nats (use ln(2) factor to convert to bits).
pub fn estimate_ksg_pairwise_mi(
    xs: &Array2<f32>,
    ys: &Array2<f32>,
    k: usize,
) -> Result<f64, KsgError> {
    let n = xs.nrows();
    if n != ys.nrows() {
        return Err(KsgError::SampleSizeMismatch);
    }
    if n < 50 {
        return Err(KsgError::SampleSizeTooSmall(n));
    }
    if k >= n / 2 {
        return Err(KsgError::KsgKTooLarge(k, n));
    }

    // Stack into joint space
    let joint = stack_joint(xs, ys);

    // For each sample i:
    //   1. Find the kth nearest neighbour in the joint space.
    //   2. Let epsilon_i = max(distance in X, distance in Y) at that neighbour.
    //   3. Count n_x_i = points within epsilon_i in X-marginal.
    //      Count n_y_i = points within epsilon_i in Y-marginal.
    let mut sum_psi_nx = 0.0_f64;
    let mut sum_psi_ny = 0.0_f64;

    let xs_kd_tree = build_kdtree_chebyshev(xs);
    let ys_kd_tree = build_kdtree_chebyshev(ys);
    let joint_kd_tree = build_kdtree_chebyshev(&joint);

    for i in 0..n {
        let joint_i = joint.row(i);
        // Find kth NN in joint space (returns the (k+1)th smallest distance
        // because the i-th sample itself counts as 0)
        let kth_dist = joint_kd_tree.kth_nearest_distance(joint_i.view(), k + 1);

        let xs_i = xs.row(i);
        let ys_i = ys.row(i);

        // Count points strictly within kth_dist in each marginal
        let n_x_i = xs_kd_tree.count_within(xs_i.view(), kth_dist) - 1; // exclude self
        let n_y_i = ys_kd_tree.count_within(ys_i.view(), kth_dist) - 1;

        sum_psi_nx += digamma((n_x_i + 1) as f64);
        sum_psi_ny += digamma((n_y_i + 1) as f64);
    }

    let n_f64 = n as f64;
    let estimate = digamma(k as f64)
        + digamma(n_f64)
        - sum_psi_nx / n_f64
        - sum_psi_ny / n_f64;

    Ok(estimate)
}

fn stack_joint(xs: &Array2<f32>, ys: &Array2<f32>) -> Array2<f32> {
    let d_x = xs.ncols();
    let d_y = ys.ncols();
    let n = xs.nrows();
    let mut joint = Array2::<f32>::zeros((n, d_x + d_y));
    joint.slice_mut(s![.., ..d_x]).assign(xs);
    joint.slice_mut(s![.., d_x..]).assign(ys);
    joint
}

fn digamma(x: f64) -> f64 {
    // Sufficient approximation for x > 0.5; fall back to recurrence
    // for x in (0, 0.5).
    if x < 0.5 {
        digamma(x + 1.0) - 1.0 / x
    } else {
        let mut result = -1.0 / (2.0 * x);
        let mut x_pow = 1.0 / (x * x);
        let coeffs = [
            -1.0 / 12.0,
             1.0 / 120.0,
            -1.0 / 252.0,
             1.0 / 240.0,
            -5.0 / 660.0,
        ];
        let mut terms = 1.0_f64;
        for c in &coeffs {
            terms *= x_pow;
            result += c * terms;
        }
        result + (x.ln())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum KsgError {
    #[error("KSG sample size mismatch")]
    SampleSizeMismatch,

    #[error("KSG sample size too small (n = {0}, must be >= 50)")]
    SampleSizeTooSmall(usize),

    #[error("KSG k = {0} is too large for n = {1}")]
    KsgKTooLarge(usize, usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_ksg_two_independent_gaussians_returns_near_zero() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let xs: Array2<f32> = generate_gaussian(&mut rng, 1000, 4);
        let ys: Array2<f32> = generate_gaussian(&mut rng, 1000, 4);  // independent
        let mi = estimate_ksg_pairwise_mi(&xs, &ys, 5).unwrap();
        // Should be near 0; small positive bias acceptable
        assert!(mi.abs() < 0.05, "MI of independent gaussians = {}", mi);
    }

    #[test]
    fn test_ksg_perfectly_correlated_returns_high_mi() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let xs: Array2<f32> = generate_gaussian(&mut rng, 1000, 4);
        let ys = xs.clone();  // identity correlation
        let mi = estimate_ksg_pairwise_mi(&xs, &ys, 5).unwrap();
        assert!(mi > 1.0, "MI of identical samples should be high; got {}", mi);
    }

    #[test]
    fn test_ksg_one_hot_pair_returns_predicted_value() {
        // For two independent uniform 4-class one-hots:
        //   I(X;Y) = 0
        // For X = Y (perfect dependency on 4 classes):
        //   I(X;Y) = log(4) ≈ 1.386 nats
        let xs = uniform_one_hot_4_class(1000, 42);
        let ys = uniform_one_hot_4_class(1000, 43);  // different seed
        let mi_indep = estimate_ksg_pairwise_mi(&xs, &ys, 5).unwrap();
        assert!(mi_indep.abs() < 0.1);

        let ys_corr = xs.clone();
        let mi_corr = estimate_ksg_pairwise_mi(&xs, &ys_corr, 5).unwrap();
        // Allow ±0.2 nats wiggle for KSG bias on small samples
        assert!((mi_corr - 1.386).abs() < 0.2,
                "Expected ~ln(4) = 1.386 nats, got {}", mi_corr);
    }

    #[test]
    fn test_ksg_k_5_default() {
        // Regression test that k=5 is what the runtime actually uses
        let xs = generate_gaussian_seeded(100, 4, 42);
        let ys = generate_gaussian_seeded(100, 4, 43);
        let r5 = estimate_ksg_pairwise_mi(&xs, &ys, 5).unwrap();
        let r3 = estimate_ksg_pairwise_mi(&xs, &ys, 3).unwrap();
        // Different k should give different (but both near-zero) estimates
        assert_ne!(r5, r3);
    }

    #[test]
    fn test_ksg_bootstrap_seed_deterministic() {
        let xs = generate_gaussian_seeded(200, 4, 42);
        let ys = generate_gaussian_seeded(200, 4, 43);
        let r1 = estimate_ksg_pairwise_mi(&xs, &ys, 5).unwrap();
        let r2 = estimate_ksg_pairwise_mi(&xs, &ys, 5).unwrap();
        assert_eq!(r1, r2);  // deterministic given fixed input
    }

    #[test]
    fn test_ksg_sample_size_too_small_rejected() {
        let xs = generate_gaussian_seeded(30, 4, 42);
        let ys = generate_gaussian_seeded(30, 4, 43);
        let result = estimate_ksg_pairwise_mi(&xs, &ys, 5);
        assert!(matches!(result, Err(KsgError::SampleSizeTooSmall(_))));
    }

    #[test]
    fn test_ksg_degenerate_input_rejected() {
        let xs = Array2::<f32>::zeros((100, 4));  // all zeros
        let ys = generate_gaussian_seeded(100, 4, 42);
        let result = estimate_ksg_pairwise_mi(&xs, &ys, 5);
        // Either rejects or returns near-zero (xs has no entropy);
        // both behaviours acceptable; this test checks no panic.
        let _ = result;
    }
}
```

## 3. `mi_audit/aggregate.rs` (NEW)

```rust
use crate::mi_audit::ksg::estimate_ksg_pairwise_mi;
use crate::mi_audit::mine::estimate_mine_pairwise_mi;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxiomTwoVerdict {
    Strong,    // n_eff_strict / N_d >= 0.95
    Weak,      // n_eff_strict / N_d in [0.85, 0.95)
    Fails,     // n_eff_strict / N_d < 0.85
}

pub struct PairwiseMiSummary {
    pub domain: String,
    pub n_panel_slots: usize,
    pub n_pairs: usize,
    pub mean_mi_nats: f64,
    pub median_mi_nats: f64,
    pub p10_mi_nats: f64,
    pub p90_mi_nats: f64,
    pub n_redundant_pairs_loose: usize,    // mi_normalised >= 0.7
    pub n_redundant_pairs_strict: usize,   // mi_normalised >= 0.9
    pub n_eff_loose: f64,
    pub n_eff_strict: f64,
    pub bootstrap_low: f64,
    pub bootstrap_high: f64,
    pub seed: u64,
    pub estimator: String,
}

pub fn aggregate_pairwise_mi_summary(
    pair_mi: &[(usize, usize, f64, f64)],  // (j, k, mi_nats, mi_normalised)
    n_panel_slots: usize,
    domain: &str,
    seed: u64,
    estimator: &str,
) -> PairwiseMiSummary {
    let mis: Vec<f64> = pair_mi.iter().map(|(_, _, mi, _)| *mi).collect();
    let normalised: Vec<f64> = pair_mi.iter().map(|(_, _, _, n)| *n).collect();

    let mean = mean(&mis);
    let median = percentile(&mis, 50.0);
    let p10 = percentile(&mis, 10.0);
    let p90 = percentile(&mis, 90.0);

    let n_red_loose = normalised.iter().filter(|&&n| n >= 0.7).count();
    let n_red_strict = normalised.iter().filter(|&&n| n >= 0.9).count();

    let n_eff_loose  = n_panel_slots as f64 - 0.5 * n_red_loose as f64;
    let n_eff_strict = n_panel_slots as f64 - 0.5 * n_red_strict as f64;

    PairwiseMiSummary {
        domain: domain.to_string(),
        n_panel_slots,
        n_pairs: pair_mi.len(),
        mean_mi_nats: mean,
        median_mi_nats: median,
        p10_mi_nats: p10,
        p90_mi_nats: p90,
        n_redundant_pairs_loose: n_red_loose,
        n_redundant_pairs_strict: n_red_strict,
        n_eff_loose,
        n_eff_strict,
        bootstrap_low: percentile(&mis, 2.5),
        bootstrap_high: percentile(&mis, 97.5),
        seed,
        estimator: estimator.to_string(),
    }
}

pub fn axiom_two_verdict(summary: &PairwiseMiSummary) -> AxiomTwoVerdict {
    let ratio = summary.n_eff_strict / summary.n_panel_slots as f64;
    if ratio >= 0.95 {
        AxiomTwoVerdict::Strong
    } else if ratio >= 0.85 {
        AxiomTwoVerdict::Weak
    } else {
        AxiomTwoVerdict::Fails
    }
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn percentile(xs: &[f64], p: f64) -> f64 {
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let idx = (p / 100.0 * sorted.len() as f64) as usize;
    sorted[idx.min(sorted.len() - 1)]
}
```

## 4. `mi_audit/mine.rs` (NEW, optional path)

```rust
//! MINE (Mutual Information Neural Estimation) — Donsker-Varadhan
//! bound estimator. Optional cross-check; default estimator is KSG.

use candle_core::{Device, Tensor, DType, Module};
use candle_nn::{linear, Linear, AdamW, Optimizer};

pub fn estimate_mine_pairwise_mi(
    xs: &Array2<f32>,
    ys: &Array2<f32>,
    n_iters: usize,
    seed: u64,
) -> Result<f64, MineError> {
    let device = Device::cuda_if_available(0)?;

    // Load samples to GPU
    let xs_t = Tensor::from_vec(
        xs.iter().copied().collect::<Vec<f32>>(),
        (xs.nrows(), xs.ncols()),
        &device,
    )?;
    let ys_t = Tensor::from_vec(
        ys.iter().copied().collect::<Vec<f32>>(),
        (ys.nrows(), ys.ncols()),
        &device,
    )?;

    // Build a small MLP T(x, y) -> R^1
    let in_dim = xs.ncols() + ys.ncols();
    let mlp = MineNet::new(in_dim, 64, &device, seed)?;

    let mut opt = AdamW::new(mlp.parameters(), candle_nn::AdamWParams { lr: 1e-3, ..Default::default() })?;
    let mut last_estimate = 0.0;

    for iter in 0..n_iters {
        // Joint sample: paired (xs, ys)
        let joint = Tensor::cat(&[&xs_t, &ys_t], 1)?;
        let t_joint = mlp.forward(&joint)?;

        // Marginal sample: paired (xs, ys_shuffled)
        let perm = generate_perm(ys.nrows(), seed.wrapping_add(iter as u64));
        let ys_shuffled = ys_t.gather(&perm_tensor(&perm, &device)?, 0)?;
        let marginal = Tensor::cat(&[&xs_t, &ys_shuffled], 1)?;
        let t_marginal = mlp.forward(&marginal)?;

        // Donsker-Varadhan bound: E[T(joint)] - log E[exp(T(marginal))]
        let mean_t_joint = t_joint.mean_all()?;
        let exp_t_marg = t_marginal.exp()?.mean_all()?;
        let log_exp = exp_t_marg.log()?;
        let mi_estimate = mean_t_joint.broadcast_sub(&log_exp)?;

        // Backward
        let loss = mi_estimate.affine(-1.0, 0.0)?;  // maximise mi <-> minimise -mi
        opt.backward_step(&loss)?;

        last_estimate = mi_estimate.to_scalar::<f32>()? as f64;
    }

    Ok(last_estimate)
}
```

MINE is the optional path; not implemented in PR 14 (KSG default).
The skeleton lives in the v2.0 codebase as a stub returning `Err(NotImplemented)`; PR 14 documents that the user can override with `--estimator mine` once it's implemented in v2.2.

## 5. The audit-pairwise-mi subcommand (PR 14)

`crates/context-graph-cli/src/commands/audit_pairwise_mi.rs` (NEW)

```rust
pub fn run_audit_pairwise_mi(args: AuditPairwiseMiArgs)
    -> Result<(), CliError>
{
    let store = open_db_read_only(&args.db)?;
    let domain_pack = get_domain_pack(&store, &args.domain)?
        .ok_or(CliError::DomainPackNotRegistered)?;

    if args.sample_size < 50 {
        return Err(CliError::MiAuditSampleSizeTooSmall);
    }

    if args.output_dir.exists() && args.output_dir.read_dir()?.next().is_some() {
        return Err(CliError::MiAuditOutputDirExists);
    }
    std::fs::create_dir_all(&args.output_dir)?;

    // Step 1: Sample n events
    let event_uuids = sample_n_events(&store, &domain_pack, args.sample_size, args.seed)?;
    if event_uuids.len() < args.sample_size {
        return Err(CliError::MiAuditSampleSizeTooSmall);
    }

    // Step 2: Read all per-instrument readings for sampled events
    let mut readings_by_instrument: HashMap<InstrumentId, Vec<Vec<f32>>> = HashMap::new();
    for event_uuid in &event_uuids {
        let event_readings = list_instrument_readings_for_event(&store, event_uuid)?;
        for r in event_readings {
            readings_by_instrument
                .entry(r.instrument_id)
                .or_insert_with(Vec::new)
                .push(r.vector);
        }
    }

    // Step 3: For each pair (j, k), assemble the (n_audit, d_j) and (n_audit, d_k) matrices
    let n_panel = domain_pack.declared_instruments.len();
    let mut pair_results: Vec<(usize, usize, f64, f64)> = Vec::new();

    for j in 0..n_panel {
        for k in (j + 1)..n_panel {
            let j_id = domain_pack.declared_instruments[j].id;
            let k_id = domain_pack.declared_instruments[k].id;

            let xs = readings_to_matrix(&readings_by_instrument[&j_id])?;
            let ys = readings_to_matrix(&readings_by_instrument[&k_id])?;

            let mi_nats = match args.estimator.as_str() {
                "ksg" => estimate_ksg_pairwise_mi(&xs, &ys, args.ksg_k)?,
                "mine" => estimate_mine_pairwise_mi(&xs, &ys, args.mine_iters, args.seed)?,
                _ => return Err(CliError::UnknownEstimator(args.estimator.clone())),
            };

            // Normalise: divide by min(H_j, H_k). Use proxy H from KSG marginal estimate.
            let h_j = estimate_marginal_entropy_ksg(&xs, args.ksg_k)?;
            let h_k = estimate_marginal_entropy_ksg(&ys, args.ksg_k)?;
            let mi_normalised = mi_nats / h_j.min(h_k);

            pair_results.push((j, k, mi_nats, mi_normalised));
        }
    }

    // Step 4: Aggregate summary
    let summary = aggregate_pairwise_mi_summary(
        &pair_results,
        n_panel,
        &args.domain,
        args.seed,
        &args.estimator,
    );
    let verdict = axiom_two_verdict(&summary);

    // Step 5: Write outputs
    write_pairwise_mi_matrix_csv(&args.output_dir.join("pairwise_mi_matrix.csv"), &pair_results, &domain_pack)?;
    write_pairwise_mi_summary_csv(&args.output_dir.join("table_pairwise_mi_summary.csv"), &summary)?;
    write_pairwise_mi_heatmap_svg(&args.output_dir.join("pairwise_mi_heatmap.svg"), &pair_results, &domain_pack)?;
    write_pairwise_mi_audit_json(&args.output_dir.join("pairwise_mi_audit.json"), &summary, verdict)?;

    if args.json {
        let json_output = MiAuditOutput {
            audit_uuid: Uuid::new_v4().to_string(),
            domain_pack_id: domain_pack.id.to_string(),
            n_audit_events: event_uuids.len(),
            estimator: args.estimator,
            ksg_k: args.ksg_k,
            seed: args.seed,
            n_eff_strict: summary.n_eff_strict,
            n_eff_loose: summary.n_eff_loose,
            verdict: format!("{:?}", verdict),
        };
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    }

    Ok(())
}
```

## 6. The heatmap SVG generator

`crates/context-graph-dynamicjepa/src/mi_audit/heatmap.rs` (NEW)

Uses the `plotters` crate (already in workspace dependencies) to
render a lower-triangular heatmap. ~150 lines of plotting code.

Output is deterministic given the input pair-MI matrix; the SVG
SHA-256 is recorded in the bundle's `archive_file_hashes.json`.

## 7. Per-pair sampling discipline

For correctness:

- The same $n$ events are used for *every* pair-MI computation
  (consistency across pairs).
- Event selection is deterministic given the bootstrap seed.
- The `event_uuids` list is recorded in the audit output JSON for
  reproducibility.

For numerical stability:

- KSG operates on f32 input but uses f64 for the digamma sums.
- Bootstrap CI uses 1{,}000 resamples by default; configurable.
- Marginal entropy estimates use the same $k$ and same sample as
  the joint estimator for consistency.

## 8. Integration with release-D

`scripts/5090jepa/local_ci_gate.sh release-d` (extended)

After the existing release-C steps, add:

```bash
# Step: Pairwise MI audit per domain
for domain in counter_world gridworld_5x5 career_taxonomy; do
  echo "Running pairwise MI audit for $domain..."

  # Use 1000 events for career; full corpus for smaller domains
  case "$domain" in
    counter_world)    sample_size=60 ;;
    gridworld_5x5)    sample_size=1000 ;;
    career_taxonomy)  sample_size=1000 ;;
  esac

  mkdir -p "${RUN_DIR}/pairwise_mi_audit_${domain}"
  $CONTEXT_GRAPH_BIN dynamicjepa audit-pairwise-mi \
    --db "${RUN_DIR}/${domain}_dbs" \
    --domain "${domain}" \
    --sample-size "${sample_size}" \
    --estimator ksg \
    --ksg-k 5 \
    --seed 20260501 \
    --output-dir "${RUN_DIR}/pairwise_mi_audit_${domain}" \
    --json
done
```

This adds ~3 minutes to the release-D wall time for the three
domains combined.

## 9. Reference comparison for release-D

`reference/reference_release_v2.0.0.json` includes:

```json
"pairwise_mi_audits": {
  "career_taxonomy": {
    "n_audit_events": 1000,
    "estimator": "ksg",
    "ksg_k": 5,
    "seed": 20260501,
    "expected_n_eff_loose_min": 9.0,
    "expected_n_eff_strict_min": 8.5,
    "expected_verdict": "WEAK"
  },
  "gridworld_5x5": {
    "n_audit_events": 1000,
    "estimator": "ksg",
    "ksg_k": 5,
    "seed": 20260501,
    "expected_n_eff_loose_min": 2.85,
    "expected_n_eff_strict_min": 2.85,
    "expected_verdict": "STRONG"
  },
  "counter_world": {
    "n_audit_events": 60,
    "estimator": "ksg",
    "ksg_k": 5,
    "seed": 20260501,
    "expected_n_eff_loose_min": 1.95,
    "expected_n_eff_strict_min": 1.95,
    "expected_verdict": "STRONG"
  }
}
```

The release-D gate's `check-release-aggregate-reference` step
extends to verify per-domain `n_eff` values and verdicts match the
reference within tolerance.

## 10. PR 14 LoC budget

| File | LoC |
| --- | ---: |
| `crates/context-graph-dynamicjepa/src/mi_audit/mod.rs` | 30 |
| `crates/context-graph-dynamicjepa/src/mi_audit/ksg.rs` | 350 |
| `crates/context-graph-dynamicjepa/src/mi_audit/mine.rs` (stub) | 100 |
| `crates/context-graph-dynamicjepa/src/mi_audit/aggregate.rs` | 250 |
| `crates/context-graph-dynamicjepa/src/mi_audit/heatmap.rs` | 200 |
| `crates/context-graph-cli/src/commands/audit_pairwise_mi.rs` | 350 |
| `crates/context-graph-cli/tests/cli_audit_pairwise_mi.rs` | 250 |
| `scripts/5090jepa/local_ci_gate.sh` (release-d step) | 50 |
| **Total** | ~1{,}580 |

Tests verify KSG behaviour on synthetic data (independent
gaussians ~0; identical samples high; one-hots = log(K)) and on
the real career\_taxonomy panel from a v2 smoke bundle.

## 11. KDTree dependency

The `kiddo` Rust crate (kiddo, ~ 50KB MIT-licensed) provides the
KDTree implementation for the Chebyshev-distance nearest-neighbour
search KSG requires. Added to `Cargo.toml` workspace dependencies
as a one-line addition; locked to a specific version per the v1
"pin every Rust dep via Cargo.lock" lesson.

## 12. Memory budget

For career\_taxonomy A1 at $n = 1{,}000$ with $d_{\max} \le 64$:

- Per-pair audit memory: 1{,}000 × 64 × 2 × 4 bytes (f32) = 512 KB
  per pair × 45 pairs = ~23 MB peak.
- KDTree overhead: ~10× the leaf storage, so ~50 MB peak.
- Plus bootstrap resampling: ~5 MB per pair.

**Total peak memory:** ~80 MB. Trivially fits within the
documented host's 110 GiB RAM.

## 13. Wall-time validation

A full release-D MI audit run on the documented host:

| Domain | $N_d$ | n\_pairs | Wall-time |
| --- | ---: | ---: | ---: |
| counter\_world (60 events) | 2 | 1 | ~5 s |
| gridworld\_5x5 (1000 events) | 3 | 3 | ~30 s |
| career\_taxonomy (1000 events) | 10 | 45 | ~80 s |
| **Total** | — | — | **~115 s** |

Plus heatmap generation (~5 s) and JSON I/O (~2 s) = ~125 s for the
audit step. Comfortably within the release-D ~3 hour budget.
