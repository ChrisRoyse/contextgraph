// Manual Full State Verification for Stoer-Wagner global minimum cut.
// Writes JSON evidence to disk under /mnt/d/contextgraph/fsv/contextgraph-tier2-fsv/.

use context_graph_mincut::{stoer_wagner, weights_from_edges, StoerWagnerConfig};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence_root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-tier2-fsv");
    fs::create_dir_all(&evidence_root)?;

    // Case 1: Stoer-Wagner paper Figure 1 — known mincut weight = 4.
    let edges_paper = [
        (0, 1, 2.0),
        (0, 4, 3.0),
        (1, 2, 3.0),
        (1, 4, 2.0),
        (1, 5, 2.0),
        (2, 3, 4.0),
        (2, 6, 2.0),
        (3, 6, 2.0),
        (3, 7, 2.0),
        (4, 5, 3.0),
        (5, 6, 1.0),
        (6, 7, 3.0),
    ];
    let weights_paper = weights_from_edges(8, edges_paper.iter().copied())?;
    let cut_paper = stoer_wagner(&weights_paper, 8, StoerWagnerConfig::default())?;

    // Case 2: dumbbell graph — mincut should isolate at the bridge of weight 1.
    let edges_dumbbell = [
        (0, 1, 10.0),
        (0, 2, 10.0),
        (1, 2, 10.0),
        (2, 3, 1.0), // bridge
        (3, 4, 10.0),
        (3, 5, 10.0),
        (4, 5, 10.0),
    ];
    let weights_dumbbell = weights_from_edges(6, edges_dumbbell.iter().copied())?;
    let cut_dumbbell = stoer_wagner(&weights_dumbbell, 6, StoerWagnerConfig::default())?;

    // Case 3: balanced clusters — three-edge weak link between two K3s.
    let edges_clusters = [
        (0, 1, 5.0),
        (0, 2, 5.0),
        (1, 2, 5.0),
        (3, 4, 5.0),
        (3, 5, 5.0),
        (4, 5, 5.0),
        (0, 3, 1.0),
        (1, 4, 0.5),
        (2, 5, 0.5),
    ];
    let weights_clusters = weights_from_edges(6, edges_clusters.iter().copied())?;
    let cut_clusters = stoer_wagner(&weights_clusters, 6, StoerWagnerConfig::default())?;

    // Boundary 1: disconnected graph rejects.
    let weights_disconnected = weights_from_edges(4, [(0, 1, 1.0), (2, 3, 1.0)].iter().copied())?;
    let disconnected_code =
        match stoer_wagner(&weights_disconnected, 4, StoerWagnerConfig::default()) {
            Err(e) => e.code(),
            Ok(_) => "UNEXPECTED_OK",
        };

    // Boundary 2: negative weight rejects.
    let weights_negative = vec![0.0, -1.0, -1.0, 0.0];
    let neg_code = match stoer_wagner(&weights_negative, 2, StoerWagnerConfig::default()) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary 3: graph with 1 vertex rejects.
    let too_small_code = match stoer_wagner(&[0.0], 1, StoerWagnerConfig::default()) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let oob_edge_code = match weights_from_edges(3, [(0, 3, 1.0)].iter().copied()) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let self_loop_edge_code = match weights_from_edges(3, [(1, 1, 1.0)].iter().copied()) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Determinism: re-run and assert identical result.
    let cut_paper_again = stoer_wagner(&weights_paper, 8, StoerWagnerConfig::default())?;
    let deterministic = cut_paper == cut_paper_again;

    require_close("paper_fig_1.cut_weight", cut_paper.cut_weight, 4.0, 1e-9)?;
    require_close("dumbbell.cut_weight", cut_dumbbell.cut_weight, 1.0, 1e-9)?;
    require_close(
        "balanced_clusters.cut_weight",
        cut_clusters.cut_weight,
        2.0,
        1e-9,
    )?;
    require_eq(
        "boundary_disconnected_graph.error_code",
        disconnected_code,
        "MINCUT_GRAPH_DISCONNECTED",
    )?;
    require_eq(
        "boundary_negative_weight.error_code",
        neg_code,
        "MINCUT_WEIGHT_INVALID",
    )?;
    require_eq(
        "boundary_too_small_graph.error_code",
        too_small_code,
        "MINCUT_GRAPH_TOO_SMALL",
    )?;
    require_eq(
        "boundary_oob_edge.error_code",
        oob_edge_code,
        "MINCUT_EDGE_ENDPOINT_OUT_OF_BOUNDS",
    )?;
    require_eq(
        "boundary_self_loop_edge.error_code",
        self_loop_edge_code,
        "MINCUT_SELF_LOOP_EDGE",
    )?;
    require("deterministic_across_runs", deterministic)?;

    let evidence = serde_json::json!({
        "case_stoer_wagner_paper_fig_1": {
            "expected_cut_weight": 4.0,
            "actual_cut_weight": cut_paper.cut_weight,
            "small_side": cut_paper.small_side,
            "large_side": cut_paper.large_side,
            "passes": (cut_paper.cut_weight - 4.0).abs() < 1e-9,
        },
        "case_dumbbell_isolates_at_bridge": {
            "expected_cut_weight": 1.0,
            "actual_cut_weight": cut_dumbbell.cut_weight,
            "small_side": cut_dumbbell.small_side,
            "large_side": cut_dumbbell.large_side,
            "passes": (cut_dumbbell.cut_weight - 1.0).abs() < 1e-9,
        },
        "case_balanced_clusters": {
            "expected_cut_weight": 2.0,
            "actual_cut_weight": cut_clusters.cut_weight,
            "small_side": cut_clusters.small_side,
            "large_side": cut_clusters.large_side,
            "passes": (cut_clusters.cut_weight - 2.0).abs() < 1e-9,
        },
        "boundary_disconnected_graph_rejected": {
            "before": {
                "n": 4,
                "edges": [[0, 1, 1.0], [2, 3, 1.0]],
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "MINCUT_GRAPH_DISCONNECTED",
                "actual_code": disconnected_code,
            },
        },
        "boundary_negative_weight_rejected": {
            "before": {
                "n": 2,
                "weights": weights_negative,
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "MINCUT_WEIGHT_INVALID",
                "actual_code": neg_code,
            },
        },
        "boundary_too_small_graph_rejected": {
            "before": {
                "n": 1,
                "weights": [0.0],
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "MINCUT_GRAPH_TOO_SMALL",
                "actual_code": too_small_code,
            },
        },
        "boundary_oob_edge_rejected": {
            "before": {
                "n": 3,
                "edges": [[0, 3, 1.0]],
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "MINCUT_EDGE_ENDPOINT_OUT_OF_BOUNDS",
                "actual_code": oob_edge_code,
            },
        },
        "boundary_self_loop_edge_rejected": {
            "before": {
                "n": 3,
                "edges": [[1, 1, 1.0]],
                "expected_mutation": "none",
            },
            "after": {
                "expected_code": "MINCUT_SELF_LOOP_EDGE",
                "actual_code": self_loop_edge_code,
            },
        },
        "deterministic_across_runs": deterministic,
    });

    let path = evidence_root.join("mincut-evidence.json");
    let readback = write_json_evidence_readback(&path, &evidence)?;
    println!("Mincut FSV evidence: {}", path.display());
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
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

fn require_close(
    label: &str,
    actual: f64,
    expected: f64,
    tolerance: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    if (actual - expected).abs() <= tolerance {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{label} expected {expected} +/- {tolerance}, got {actual}"),
        )
        .into())
    }
}
