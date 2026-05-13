use context_graph_mejepa::eval::render_pairwise_mi_heatmap;
use context_graph_mejepa::eval::report::write_json_0600;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-mi-heatmap-fsv";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root)?;

    let csv_path = run_root.join("pairwise_mi_matrix.csv");
    fs::write(
        &csv_path,
        "slot,E_AST,E_CFG,E_Test\nE_AST,1.0,0.25,0.75\nE_CFG,0.25,1.0,0.50\nE_Test,0.75,0.50,1.0\n",
    )?;
    let markdown_path = run_root.join("pairwise_mi_matrix.md");
    let png_path = run_root.join("pairwise_mi_matrix.png");
    let render = render_pairwise_mi_heatmap(&csv_path, &markdown_path, &png_path)?;
    let markdown = fs::read_to_string(&markdown_path)?;
    let png = fs::read(&png_path)?;
    let happy_pass = render.readback_equal
        && markdown.contains("| E_AST | 1.000 | 0.250 | 0.750 |")
        && png.starts_with(b"\x89PNG\r\n\x1a\n")
        && render.max_off_diagonal == 0.75;

    let empty_csv = run_root.join("empty.csv");
    fs::write(&empty_csv, "")?;
    let empty_error = render_pairwise_mi_heatmap(
        &empty_csv,
        run_root.join("empty.md"),
        run_root.join("empty.png"),
    )
    .expect_err("empty CSV must fail closed");

    let ragged_csv = run_root.join("ragged.csv");
    fs::write(&ragged_csv, "slot,A,B\nA,1.0,0.2\nB,0.2\n")?;
    let ragged_error = render_pairwise_mi_heatmap(
        &ragged_csv,
        run_root.join("ragged.md"),
        run_root.join("ragged.png"),
    )
    .expect_err("ragged CSV must fail closed");

    let out_of_range_csv = run_root.join("out_of_range.csv");
    fs::write(&out_of_range_csv, "slot,A,B\nA,1.0,1.2\nB,1.2,1.0\n")?;
    let out_of_range_error = render_pairwise_mi_heatmap(
        &out_of_range_csv,
        run_root.join("out_of_range.md"),
        run_root.join("out_of_range.png"),
    )
    .expect_err("out-of-range CSV must fail closed");

    let label_mismatch_csv = run_root.join("label_mismatch.csv");
    fs::write(&label_mismatch_csv, "slot,A,B\nA,1.0,0.2\nC,0.2,1.0\n")?;
    let label_mismatch_error = render_pairwise_mi_heatmap(
        &label_mismatch_csv,
        run_root.join("label_mismatch.md"),
        run_root.join("label_mismatch.png"),
    )
    .expect_err("label mismatch CSV must fail closed");

    let boundary_cases = vec![
        json!({
            "case": "empty_csv_fails_closed",
            "expected": "MEJEPA_EVAL_INVALID_INPUT",
            "actual": empty_error.code(),
            "pass": empty_error.code() == "MEJEPA_EVAL_INVALID_INPUT"
        }),
        json!({
            "case": "ragged_csv_fails_closed",
            "expected": "MEJEPA_EVAL_INVALID_INPUT",
            "actual": ragged_error.code(),
            "pass": ragged_error.code() == "MEJEPA_EVAL_INVALID_INPUT"
        }),
        json!({
            "case": "out_of_range_value_fails_closed",
            "expected": "MEJEPA_EVAL_INVALID_INPUT",
            "actual": out_of_range_error.code(),
            "pass": out_of_range_error.code() == "MEJEPA_EVAL_INVALID_INPUT"
        }),
        json!({
            "case": "row_label_mismatch_fails_closed",
            "expected": "MEJEPA_EVAL_INVALID_INPUT",
            "actual": label_mismatch_error.code(),
            "pass": label_mismatch_error.code() == "MEJEPA_EVAL_INVALID_INPUT"
        }),
    ];
    let artifacts = vec![
        artifact_state(&csv_path)?,
        artifact_state(&markdown_path)?,
        artifact_state(&png_path)?,
    ];
    let all_passed = happy_pass && boundary_cases.iter().all(|case| case["pass"] == true);
    let evidence = json!({
        "fsv_root": fsv_root,
        "task_id": "TASK-OBS-002",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short(),
        "happy_path": [{
            "case": "render_pairwise_mi_markdown_and_png",
            "sot": "file:/mnt/d/contextgraph/fsv/phase-f-mi-heatmap-fsv/run-*/pairwise_mi_matrix.{md,png}",
            "before": null,
            "trigger": "cargo run --release -p context-graph-mejepa --example phase_f_mi_heatmap_fsv",
            "after": {
                "markdownPath": markdown_path,
                "pngPath": png_path,
                "slotCount": render.slot_count,
                "maxOffDiagonal": render.max_off_diagonal,
                "pngSignature": hex::encode(&png[..8])
            },
            "expected": "markdown table and PNG heatmap render from pairwise_mi_matrix.csv",
            "actual": happy_pass,
            "pass": happy_pass,
            "evidence_path": run_root
        }],
        "boundary_cases": boundary_cases,
        "all_passed": all_passed,
        "readback_equal": render.readback_equal,
        "physical_artifacts": artifacts
    });
    let evidence_path = fsv_root.join("mi_heatmap_renderer_fsv.json");
    write_json_0600(&evidence_path, &evidence)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    if readback != evidence {
        return Err("MI heatmap evidence readback mismatch".into());
    }
    if !all_passed {
        return Err("TASK-OBS-002 FSV failed".into());
    }
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn artifact_state(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(json!({
        "path": path,
        "bytes": bytes.len(),
        "sha256": hex::encode(hasher.finalize())
    }))
}

fn git_head_short() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
