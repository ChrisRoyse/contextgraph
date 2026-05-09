// Inspired by ruvnet/RuVector crates/ruvector-graph-transformer/src/temporal.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Manual Full State Verification for causal masking and retrocausal safety.
// Writes JSON evidence to /tmp/contextgraph-tier2-fsv/.

use context_graph_solver::{
    apply_mask_to_scores, build_causal_mask, retrocausal_safety_check, softmax_row, MaskStrategy,
};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-tier2-fsv");
    fs::create_dir_all(&evidence_root)?;

    let timestamps = [10_u64, 20, 35, 50];
    let expected_time_window = vec![
        true, false, false, false, //
        true, true, false, false, //
        false, true, true, false, //
        false, false, true, true,
    ];
    let mask = build_causal_mask(&timestamps, MaskStrategy::TimeWindow { window_size: 15 })?;
    require_eq(
        "time_window.mask",
        mask.as_slice(),
        expected_time_window.as_slice(),
    )?;

    let mut scores = vec![
        0.1_f32, 0.2, 0.3, 0.4, //
        0.5, 0.6, 0.7, 0.8, //
        0.9, 1.0, 1.1, 1.2, //
        1.3, 1.4, 1.5, 1.6,
    ];
    let scores_before_mask = scores.clone();
    apply_mask_to_scores(&mut scores, &mask, timestamps.len())?;
    require_eq("masked.future_score", scores[2], f32::NEG_INFINITY)?;
    let row_zero_probs = softmax_row(&scores[0..4])?;
    require_eq(
        "softmax.masked_future_probability",
        row_zero_probs[1],
        0.0_f32,
    )?;
    require_eq(
        "softmax.second_masked_future_probability",
        row_zero_probs[2],
        0.0_f32,
    )?;
    require_eq(
        "softmax.third_masked_future_probability",
        row_zero_probs[3],
        0.0_f32,
    )?;

    let empty_code = match build_causal_mask(&[], MaskStrategy::CausalWithSelf) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let mask_short = build_causal_mask(&[10, 20], MaskStrategy::CausalWithSelf)?;
    let mut short_scores = vec![0.1_f32, 0.2, 0.3];
    let short_scores_before = short_scores.clone();
    let shape_code = match apply_mask_to_scores(&mut short_scores, &mask_short, 2) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let mut inf_scores = vec![0.1_f32, f32::INFINITY, 0.3, 0.4];
    let inf_scores_before = inf_scores.clone();
    let inf_code = match apply_mask_to_scores(&mut inf_scores, &mask_short, 2) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let retro_code =
        match retrocausal_safety_check(&[10, 20, 30], 1, 2, MaskStrategy::CausalWithSelf) {
            Err(e) => e.code(),
            Ok(_) => "UNEXPECTED_OK",
        };

    require_eq(
        "boundary.empty_timestamps",
        empty_code,
        "CGSOLVER_INVALID_INPUT",
    )?;
    require_eq(
        "boundary.shape_mismatch",
        shape_code,
        "CGSOLVER_INVALID_INPUT",
    )?;
    require_eq(
        "boundary.non_finite_scores",
        inf_code,
        "CGSOLVER_NUMERICAL_INVARIANT",
    )?;
    require_eq(
        "boundary.retrocausal_future_attention",
        retro_code,
        "CGSOLVER_INVALID_INPUT",
    )?;
    require_eq(
        "boundary.shape_mismatch_preserves_scores",
        short_scores.as_slice(),
        short_scores_before.as_slice(),
    )?;
    require_eq(
        "boundary.non_finite_preserves_scores",
        inf_scores.as_slice(),
        inf_scores_before.as_slice(),
    )?;

    let evidence = serde_json::json!({
        "case_time_window_mask": {
            "timestamps": timestamps,
            "window_size": 15,
            "expected_mask": expected_time_window,
            "actual_mask": mask,
            "passes": true,
        },
        "case_apply_mask_and_softmax": {
            "before": {
                "scores": scores_as_json(&scores_before_mask),
                "mask_shape": [timestamps.len(), timestamps.len()],
            },
            "after": {
                "masked_scores": scores_as_json(&scores),
                "row_zero_softmax": row_zero_probs,
                "future_probabilities_are_zero": true,
            },
        },
        "boundary_empty_timestamps_rejected": {
            "before": {
                "timestamps_len": 0,
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": empty_code,
            },
        },
        "boundary_shape_mismatch_rejected": {
            "before": {
                "n": 2,
                "scores_len": 3,
                "scores": scores_as_json(&short_scores_before),
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": shape_code,
                "scores_preserved": scores_as_json(&short_scores),
            },
        },
        "boundary_non_finite_scores_rejected_before_mutation": {
            "before": {
                "n": 2,
                "scores": ["0.1", "+inf", "0.3", "0.4"],
            },
            "after": {
                "expected_code": "CGSOLVER_NUMERICAL_INVARIANT",
                "actual_code": inf_code,
                "scores_preserved": ["0.1", "+inf", "0.3", "0.4"],
            },
        },
        "boundary_retrocausal_future_attention_rejected": {
            "before": {
                "timestamps": [10, 20, 30],
                "query_idx": 1,
                "key_idx": 2,
                "strategy": "CausalWithSelf",
            },
            "after": {
                "expected_code": "CGSOLVER_INVALID_INPUT",
                "actual_code": retro_code,
            },
        },
    });

    let path = evidence_root.join("causal-mask-evidence.json");
    let readback = write_json_evidence_readback(&path, &evidence)?;
    println!("Causal mask FSV evidence: {}", path.display());
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn scores_as_json(scores: &[f32]) -> Vec<Value> {
    scores
        .iter()
        .map(|score| {
            if score.is_infinite() && score.is_sign_negative() {
                Value::String("-inf".to_string())
            } else if score.is_infinite() && score.is_sign_positive() {
                Value::String("+inf".to_string())
            } else if score.is_nan() {
                Value::String("NaN".to_string())
            } else {
                serde_json::json!(score)
            }
        })
        .collect()
}

fn write_json_evidence_readback(
    path: &Path,
    evidence: &Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let text = serde_json::to_string_pretty(evidence)?;
    let mut file = File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    let readback_text = fs::read_to_string(path)?;
    let readback: Value = serde_json::from_str(&readback_text)?;
    require("evidence_json_readback_matches", readback == *evidence)?;
    Ok(readback)
}

fn require(label: &str, condition: bool) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, format!("{label} expected true")).into())
    }
}

fn require_eq<T>(label: &str, actual: T, expected: T) -> Result<(), Box<dyn std::error::Error>>
where
    T: PartialEq + std::fmt::Debug,
{
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{label} expected {expected:?}, got {actual:?}"),
        )
        .into())
    }
}
