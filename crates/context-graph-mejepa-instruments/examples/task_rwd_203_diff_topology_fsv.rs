use context_graph_mejepa_instruments::{
    analyze_diff_topology, DiffTopologyInput, EDiffTopologyInstrument, Instrument,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    error_code: String,
    passed: bool,
}

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/task-rwd-203-diff-topology-fsv")
        .join(format!("run-{run_id}"));
    fs::create_dir_all(&root)?;
    let diff_path = root.join("sample.diff");
    let evidence_path = root.join("diff_topology_fsv.json");
    fs::write(&diff_path, sample_diff())?;
    require("sample diff physically written", diff_path.exists())?;

    let input = DiffTopologyInput {
        diff: fs::read_to_string(&diff_path)?,
    };
    let metrics = analyze_diff_topology(&input)?;
    let vector = EDiffTopologyInstrument.encode(&input)?;
    require("vector length is E_Diff slot", vector.len() == 256)?;
    require(
        "vector has non-zero reward signal",
        vector.iter().any(|value| *value > 0.0),
    )?;
    require("file_count is 2", metrics.file_count == 2)?;
    require("hunk_count is 3", metrics.hunk_count == 3)?;
    require("added_lines is 5", metrics.added_lines == 5)?;
    require("removed_lines is 2", metrics.removed_lines == 2)?;
    require("changed_lines is 7", metrics.changed_lines == 7)?;
    require("max_hunks_per_file is 2", metrics.max_hunks_per_file == 2)?;
    require(
        "file entropy is positive",
        metrics.file_change_entropy_bits.is_finite() && metrics.file_change_entropy_bits > 0.5,
    )?;
    require(
        "intra-file entropy is positive",
        metrics.mean_intra_file_hunk_entropy_bits.is_finite()
            && metrics.mean_intra_file_hunk_entropy_bits >= 0.5,
    )?;

    let edge_cases = vec![
        edge_case("empty_diff", ""),
        edge_case(
            "no_hunks",
            "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n",
        ),
        edge_case("orphan_hunk", "@@ -1,1 +1,1 @@\n-x\n+y\n"),
        edge_case(
            "line_count_mismatch",
            "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -1,2 +1,2 @@\n x = 1\n",
        ),
        edge_case("nul_diff", "diff --git a/a.py b/a.py\0\n"),
    ];
    require(
        "all edge cases fail closed with instrument invalid-input code",
        edge_cases.iter().all(|case| case.passed),
    )?;

    let evidence = json!({
        "task": "TASK-RWD-203",
        "all_passed": true,
        "source_of_truth": {
            "root": root,
            "diff_path": diff_path,
            "evidence_path": evidence_path,
        },
        "happy_path": {
            "metrics": metrics,
            "vector_len": vector.len(),
            "vector_non_zero": vector.iter().filter(|value| **value != 0.0).count(),
            "vector_sha256": sha256_json(&vector)?,
        },
        "edge_cases": edge_cases,
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"] == true,
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn edge_case(name: &'static str, diff: &str) -> EdgeCase {
    let error_code = EDiffTopologyInstrument
        .encode(&DiffTopologyInput {
            diff: diff.to_string(),
        })
        .unwrap_err()
        .code()
        .to_string();
    EdgeCase {
        name,
        passed: error_code == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        error_code,
    }
}

fn sample_diff() -> &'static str {
    r#"diff --git a/src/a.py b/src/a.py
index 1111111..2222222 100644
--- a/src/a.py
+++ b/src/a.py
@@ -1,4 +1,5 @@
 def f():
-    return 1
+    value = 1
+    return value
 
 def g():
@@ -10,3 +11,4 @@
 def h():
-    return 2
+    value = 2
+    return value
 # end
diff --git a/tests/test_a.py b/tests/test_a.py
index 3333333..4444444 100644
--- a/tests/test_a.py
+++ b/tests/test_a.py
@@ -1,3 +1,4 @@
 from src.a import f
+from src.a import g
 
 def test_f():
"#
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json write/readback bytes equal", readback == bytes)?;
    Ok(())
}

fn sha256_json<T: Serialize>(value: &T) -> FsvResult<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn require(message: &'static str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
