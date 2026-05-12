// Manual Full State Verification (FSV) for Tier-1 foundation gaps:
// generic MMR + conformal prediction. Writes JSON evidence to disk under
// /mnt/d/contextgraph/fsv/contextgraph-tier1-fsv/ for physical inspection.

use context_graph_solver::{
    mmr_select, mmr_select_with_matrix, ConformalConfig, ConformalPredictor, MmrConfig,
    NonconformityMeasure,
};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence_root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-tier1-fsv");
    fs::create_dir_all(&evidence_root)?;

    let mmr_evidence = run_mmr_fsv()?;
    let conformal_evidence = run_conformal_fsv()?;

    let combined = serde_json::json!({
        "fsv_run_started_at_unix_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        "mmr": mmr_evidence,
        "conformal": conformal_evidence,
    });

    let path = evidence_root.join("solver-tier1-evidence.json");
    let evidence_text = serde_json::to_string_pretty(&combined)?;
    write_bytes_durable(&path, evidence_text.as_bytes())?;
    let evidence_back = fs::read_to_string(&path)?;
    require_eq(
        "evidence_readback.sha_equivalent_text",
        evidence_back.as_str(),
        evidence_text.as_str(),
    )?;
    let evidence_json: serde_json::Value = serde_json::from_str(&evidence_back)?;
    require(
        evidence_json["mmr"]["case_jaccard_lambda_030"]["actual_order"]
            == serde_json::json!([0, 2]),
        "evidence_readback.mmr.case_jaccard_lambda_030.actual_order expected [0,2]",
    )?;
    require(
        evidence_json["conformal"]["case_rank_nonconformity"]["top_ranked_candidate_included"]
            == serde_json::json!(true),
        "evidence_readback.conformal.rank.top_ranked_candidate_included expected true",
    )?;

    println!("Solver Tier-1 FSV evidence: {}", path.display());
    println!("{}", serde_json::to_string_pretty(&combined)?);
    Ok(())
}

fn write_bytes_durable(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn run_mmr_fsv() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Synthetic case 1: closure-based MMR with known relevance + Jaccard
    // similarity. Two near-duplicates (idx 0, 1) and one unique (idx 2);
    // λ=0.30 should pick 0 first, then 2 second.
    let texts = [
        "alpha bravo charlie",
        "alpha bravo delta",
        "echo foxtrot golf",
    ];
    let relevances = vec![0.80, 0.78, 0.55];
    let case_one = mmr_select(
        &relevances,
        MmrConfig {
            lambda: 0.30,
            limit: 2,
        },
        |i, j| jaccard_words(texts[i], texts[j]),
    )?;
    let case_one_order = case_one.iter().map(|s| s.index).collect::<Vec<_>>();
    require_eq(
        "mmr.case_jaccard_lambda_030.order",
        case_one_order.as_slice(),
        &[0usize, 2],
    )?;

    // Synthetic case 2: matrix-based MMR with deterministic identity sims.
    // Pure relevance ordering at λ=1.0.
    let rels_two = vec![0.30, 0.95, 0.60, 0.10];
    let n = rels_two.len();
    let mut sim = vec![0.0; n * n];
    for i in 0..n {
        sim[i * n + i] = 1.0;
    }
    let case_two = mmr_select_with_matrix(
        &rels_two,
        &sim,
        MmrConfig {
            lambda: 1.0,
            limit: 4,
        },
    )?;
    let case_two_order = case_two.iter().map(|s| s.index).collect::<Vec<_>>();
    require_eq(
        "mmr.case_matrix_lambda_100.order",
        case_two_order.as_slice(),
        &[1usize, 2, 0, 3],
    )?;

    // Synthetic case 3: lambda=0.0 with sim matrix — picks the lowest-index
    // first (tiebreak), then the candidate with minimum max-similarity to
    // the chosen set.
    let rels_three = vec![0.5, 0.5, 0.5];
    let sim_three = vec![
        1.0, 0.9, 0.1, // 0
        0.9, 1.0, 0.2, // 1
        0.1, 0.2, 1.0, // 2
    ];
    let case_three = mmr_select_with_matrix(
        &rels_three,
        &sim_three,
        MmrConfig {
            lambda: 0.0,
            limit: 2,
        },
    )?;
    let case_three_order = case_three.iter().map(|s| s.index).collect::<Vec<_>>();
    require_eq(
        "mmr.case_pure_diversity_lambda_000.order",
        case_three_order.as_slice(),
        &[0usize, 2],
    )?;

    // Boundary 1: lambda < 0 → CGSOLVER_INVALID_INPUT.
    let bad_lambda = mmr_select(
        &[0.5],
        MmrConfig {
            lambda: -0.1,
            limit: 1,
        },
        |_, _| 0.0,
    );
    let bad_lambda_code = match bad_lambda {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    require_eq(
        "mmr.boundary_negative_lambda.error_code",
        &bad_lambda_code,
        &"CGSOLVER_INVALID_INPUT",
    )?;

    // Boundary 2: limit=0 → empty result.
    let zero_limit = mmr_select(
        &[0.5],
        MmrConfig {
            lambda: 0.5,
            limit: 0,
        },
        |_, _| 0.0,
    )?;
    require_eq("mmr.boundary_limit_zero.len", &zero_limit.len(), &0usize)?;

    // Boundary 3: NaN relevance → CGSOLVER_INVALID_INPUT.
    let bad_rel = mmr_select(
        &[0.5, f64::NAN],
        MmrConfig {
            lambda: 0.5,
            limit: 2,
        },
        |_, _| 0.0,
    );
    let bad_rel_code = match bad_rel {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    require_eq(
        "mmr.boundary_nan_relevance.error_code",
        &bad_rel_code,
        &"CGSOLVER_INVALID_INPUT",
    )?;

    // Boundary 4: similarity outside [0,1] → CGSOLVER_INVALID_INPUT.
    let bad_similarity = mmr_select(
        &[0.5, 0.4],
        MmrConfig {
            lambda: 0.5,
            limit: 2,
        },
        |_, _| 1.01,
    );
    let bad_similarity_code = match bad_similarity {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    require_eq(
        "mmr.boundary_similarity_above_one.error_code",
        &bad_similarity_code,
        &"CGSOLVER_INVALID_INPUT",
    )?;

    Ok(serde_json::json!({
        "case_jaccard_lambda_030": {
            "expected_order": [0usize, 2],
            "actual_order": case_one_order,
            "diversity_penalty_for_pick_2": case_one[1].diversity_penalty,
        },
        "case_matrix_lambda_100": {
            "expected_order": [1usize, 2, 0, 3],
            "actual_order": case_two_order,
        },
        "case_pure_diversity_lambda_000": {
            "expected_order": [0usize, 2],
            "actual_order": case_three_order,
        },
        "boundary_negative_lambda": {
            "expected_code": "CGSOLVER_INVALID_INPUT",
            "actual_code": bad_lambda_code,
        },
        "boundary_limit_zero": {
            "expected_len": 0usize,
            "actual_len": zero_limit.len(),
        },
        "boundary_nan_relevance": {
            "expected_code": "CGSOLVER_INVALID_INPUT",
            "actual_code": bad_rel_code,
        },
        "boundary_similarity_above_one": {
            "expected_code": "CGSOLVER_INVALID_INPUT",
            "actual_code": bad_similarity_code,
        },
    }))
}

fn run_conformal_fsv() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Synthetic case 1: distance measure, n=10, alpha=0.1.
    // k = ceil(11 * 0.9) = 10 → threshold = max calibration = 1.0.
    let mut p1 = ConformalPredictor::new(ConformalConfig {
        alpha: 0.1,
        measure: NonconformityMeasure::Distance,
    })?;
    let calib1 = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
    let report1 = p1.calibrate_distances(&calib1)?;
    let pred1 = p1.predict_set(&[0.05, 0.5, 0.99, 1.5, 0.85], None)?;
    require_close(
        "conformal.distance.threshold",
        report1.threshold,
        1.0,
        1e-12,
    )?;
    require_eq(
        "conformal.distance.quantile_rank",
        &report1.quantile_rank,
        &10usize,
    )?;
    require_eq(
        "conformal.distance.predicted_indices",
        pred1.indices.as_slice(),
        &[0usize, 1, 2, 4],
    )?;

    // Synthetic case 2: empirical coverage on a held-out sample.
    // Calibration and test from the same synthetic distribution → coverage
    // should land near 1-α=0.9.
    let mut p2 = ConformalPredictor::new(ConformalConfig {
        alpha: 0.1,
        measure: NonconformityMeasure::Distance,
    })?;
    let calib2: Vec<f64> = (0..200)
        .map(|i| ((i as f64 * 1234.5).sin().abs()).fract())
        .collect();
    p2.calibrate_distances(&calib2)?;
    let test2: Vec<f64> = (0..400)
        .map(|i| (((i + 200) as f64 * 1234.5).sin().abs()).fract())
        .collect();
    let cov2 = p2.empirical_coverage(&test2)?;
    require(
        (cov2 - 0.9).abs() < 0.05,
        format!("conformal.empirical_coverage expected 0.9 +/- 0.05, got {cov2}"),
    )?;

    // Boundary 1: uncalibrated → fail-closed CGSOLVER_INVALID_INPUT.
    let p3 = ConformalPredictor::new(ConformalConfig::default())?;
    let bad_uncal = p3.predict_set(&[0.1, 0.5], None);
    let bad_uncal_code = match bad_uncal {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    require_eq(
        "conformal.boundary_uncalibrated_predict.error_code",
        &bad_uncal_code,
        &"CGSOLVER_INVALID_INPUT",
    )?;

    // Boundary 2: alpha=0 → CGSOLVER_INVALID_INPUT.
    let bad_alpha = ConformalPredictor::new(ConformalConfig {
        alpha: 0.0,
        measure: NonconformityMeasure::Distance,
    });
    let bad_alpha_code = match bad_alpha {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    require_eq(
        "conformal.boundary_alpha_zero.error_code",
        &bad_alpha_code,
        &"CGSOLVER_INVALID_INPUT",
    )?;

    // Boundary 3: NormalizedDistance handles heterogeneous scales.
    // Calibration {1,2,4,8} → median 3 → normalized {1/3, 2/3, 4/3, 8/3}.
    // alpha=0.5 → k = ceil(5 * 0.5) = 3 → threshold = 4/3.
    let mut p4 = ConformalPredictor::new(ConformalConfig {
        alpha: 0.5,
        measure: NonconformityMeasure::NormalizedDistance,
    })?;
    let report4 = p4.calibrate_distances(&[1.0, 2.0, 4.0, 8.0])?;
    require_close(
        "conformal.normalized_distance.threshold",
        report4.threshold,
        4.0_f64 / 3.0,
        1e-9,
    )?;

    // Synthetic case 4: rank nonconformity. Rank 0 is the best candidate
    // and must be included when threshold allows the top three ranks.
    let mut p5 = ConformalPredictor::new(ConformalConfig {
        alpha: 0.5,
        measure: NonconformityMeasure::Rank,
    })?;
    let report5 = p5.calibrate_with_ranks(&[0, 1, 2, 3])?;
    let pred5 = p5.predict_set(&[0.0, 0.0, 0.0, 0.0], Some(&[0.3, 0.9, 0.1, 0.5]))?;
    require_close("conformal.rank.threshold", report5.threshold, 2.0, 1e-12)?;
    require_eq(
        "conformal.rank.predicted_indices",
        pred5.indices.as_slice(),
        &[0usize, 1, 3],
    )?;
    require(
        pred5.indices.contains(&1),
        "conformal.rank.top_ranked_candidate_included expected true",
    )?;

    Ok(serde_json::json!({
        "case_distance_alpha_010": {
            "expected_threshold": 1.0,
            "actual_threshold": report1.threshold,
            "expected_quantile_rank": 10,
            "actual_quantile_rank": report1.quantile_rank,
            "expected_calibration_size": 10,
            "actual_calibration_size": report1.calibration_size,
            "predicted_indices": pred1.indices,
            "predicted_count": pred1.indices.len(),
        },
        "case_empirical_coverage": {
            "stated_coverage": 0.9,
            "tolerance": 0.05,
            "actual_coverage": cov2,
            "passes": (cov2 - 0.9).abs() < 0.05,
        },
        "boundary_uncalibrated_predict": {
            "expected_code": "CGSOLVER_INVALID_INPUT",
            "actual_code": bad_uncal_code,
        },
        "boundary_alpha_zero": {
            "expected_code": "CGSOLVER_INVALID_INPUT",
            "actual_code": bad_alpha_code,
        },
        "case_normalized_distance": {
            "expected_threshold_close_to": 4.0 / 3.0,
            "actual_threshold": report4.threshold,
            "passes": (report4.threshold - 4.0_f64 / 3.0).abs() < 1e-9,
        },
        "case_rank_nonconformity": {
            "expected_threshold": 2.0,
            "actual_threshold": report5.threshold,
            "expected_indices": [0usize, 1, 3],
            "actual_indices": pred5.indices,
            "top_ranked_candidate_included": pred5.indices.contains(&1),
        },
    }))
}

fn jaccard_words(a: &str, b: &str) -> f64 {
    use std::collections::BTreeSet;
    let sa: BTreeSet<&str> = a.split_whitespace().collect();
    let sb: BTreeSet<&str> = b.split_whitespace().collect();
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, message.into()).into())
    }
}

fn require_eq<T>(label: &str, actual: &T, expected: &T) -> Result<(), Box<dyn std::error::Error>>
where
    T: PartialEq + std::fmt::Debug + ?Sized,
{
    require(
        actual == expected,
        format!("{label} expected {expected:?}, got {actual:?}"),
    )
}

fn require_close(
    label: &str,
    actual: f64,
    expected: f64,
    tolerance: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    require(
        (actual - expected).abs() <= tolerance,
        format!("{label} expected {expected} +/- {tolerance}, got {actual}"),
    )
}
