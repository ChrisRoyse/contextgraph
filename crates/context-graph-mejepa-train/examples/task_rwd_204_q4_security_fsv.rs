use context_graph_mejepa::{
    infer_phase_b_surfaces, AstDiff, DiffHunk, Language, PatchBundle, SecurityConcernClass,
    TaskContext, TaskEnvironment, TaskId, TestId,
};
use context_graph_mejepa_instruments::{
    extract_q4_security_labels, run_q4_security_tools_with_timeout, write_q4_security_raw_outputs,
    EStaticAnalysisInstrument, Instrument, Q4SecurityDetector, Q4SecurityLabelStore,
    Q4SecurityRawOutputs, Q4SecurityScanPhase, Q4SecuritySignalRecord, Q4SecuritySource,
    Q4SecurityToolOutput,
};
use context_graph_mejepa_train::labels::q4_security_labels_to_phase_b_labels;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    passed: bool,
    detail: Value,
}

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/task-py-g-046-q4-security-label-fsv")
        .join(format!("run-{run_id}"));
    fs::create_dir_all(&fsv_root)?;
    let raw_root = PathBuf::from("/mnt/d/contextgraph/corpus/python-q4-security-labels-v1");
    let row_id = format!("task-rwd-204-{run_id}");
    let rocksdb_path = fsv_root.join("q4-security-rocksdb");
    let evidence_path = fsv_root.join("q4_security_label_fsv.json");

    let pre_path = fsv_root.join("pre.py");
    let post_path = fsv_root.join("post.py");
    let source = happy_source(&row_id);
    fs::write(&pre_path, source.pre_patch_source.as_bytes())?;
    fs::write(&post_path, source.post_patch_source.as_bytes())?;

    let old_path = install_security_shims(&fsv_root)?;
    let outputs =
        run_q4_security_tools_with_timeout(&pre_path, &post_path, Duration::from_secs(2))?;
    restore_path(old_path);
    require(
        "bandit shim executed",
        outputs.bandit_post.status_code == Some(1) && !outputs.bandit_post.toolchain_missing,
    )?;
    require(
        "semgrep shim executed",
        outputs.semgrep_post.status_code == Some(0) && !outputs.semgrep_post.toolchain_missing,
    )?;

    let raw_paths = write_q4_security_raw_outputs(&raw_root, &row_id, &outputs)?;
    require("bandit raw output exists", raw_paths.bandit_json.exists())?;
    require("semgrep raw output exists", raw_paths.semgrep_json.exists())?;
    let bandit_raw: Value = serde_json::from_slice(&fs::read(&raw_paths.bandit_json)?)?;
    require(
        "bandit raw wrapper preserves post phase",
        bandit_raw["post_patch"]["phase"] == "post_patch",
    )?;

    let extraction = extract_q4_security_labels(&source, &outputs)?;
    require("happy path produced security labels", !extraction.labels.is_empty())?;
    require(
        "happy path has no quarantines",
        extraction.quarantines.is_empty(),
    )?;
    require(
        "Bandit B602 is introduced by patch",
        extraction.labels.iter().any(|label| {
            label.detector == Q4SecurityDetector::Bandit
                && label.rule_id == "B602"
                && label.introduced_by_patch
                && label.class.as_str() == "command_injection"
        }),
    )?;

    let vector = EStaticAnalysisInstrument.encode(&extraction.static_analysis_input)?;
    require("E_StaticAnalysis vector len is 256", vector.len() == 256)?;
    require(
        "E_StaticAnalysis vector nonzero",
        vector.iter().any(|value| *value != 0.0),
    )?;

    let phase_b_labels = q4_security_labels_to_phase_b_labels(&extraction.labels)?;
    require(
        "training adapter emits CommandInjection label",
        phase_b_labels
            .security_concerns
            .iter()
            .any(|label| label.class == SecurityConcernClass::CommandInjection),
    )?;
    let sibling_surfaces = infer_sibling_security_surface()?;
    require(
        "sibling holdout emits predicted CommandInjection concern",
        sibling_surfaces
            .predicted_security_concerns
            .iter()
            .any(|concern| concern.class == SecurityConcernClass::CommandInjection),
    )?;

    let store = Q4SecurityLabelStore::open(&rocksdb_path)?;
    let before_count = store.count_records()?;
    let keys = store.put_extraction(&extraction)?;
    store.flush()?;
    let after_count = store.count_records()?;
    require(
        "store count moved by label count",
        after_count == before_count + extraction.labels.len(),
    )?;
    drop(store);
    let reopened = Q4SecurityLabelStore::open(&rocksdb_path)?;
    let reopened_rows = reopened.scan_records()?;
    require(
        "reopened RocksDB sees label rows",
        reopened_rows.len() == extraction.labels.len(),
    )?;
    require(
        "reopened rows are labels",
        reopened_rows
            .iter()
            .all(|(_, row)| matches!(row.signal, Q4SecuritySignalRecord::Label(_))),
    )?;
    drop(reopened);

    let edge_cases = vec![
        edge_toolchain_missing_quarantined(&rocksdb_path)?,
        edge_fixed_finding_label()?,
        edge_unchanged_finding_dropped()?,
        edge_parser_failure_quarantined()?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 security edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let evidence = json!({
        "task": "TASK-RWD-204",
        "issue": 265,
        "all_passed": true,
        "source_of_truth": {
            "raw_output_root": raw_root,
            "raw_output_paths": raw_paths,
            "rocksdb_path": rocksdb_path,
            "cf": context_graph_mejepa_cf::CF_MEJEPA_Q4_SECURITY_LABELS,
            "evidence_path": evidence_path,
        },
        "happy_path": {
            "before_state": {"cf_record_count": before_count},
            "trigger": "Run Bandit and Semgrep through deterministic PATH shims; normalize raw JSON into Q4 security delta labels; persist to CF; reopen RocksDB; feed E_StaticAnalysis and the Phase-B security label adapter.",
            "expected": {
                "bandit_rule": "B602",
                "introduced_by_patch": true,
                "cf_record_delta": extraction.labels.len(),
                "raw_files": ["bandit.json", "semgrep.json"],
                "predicted_security_concern": "command_injection"
            },
            "after_state": {
                "labels": extraction.labels,
                "quarantines": extraction.quarantines,
                "phase_b_security_labels": phase_b_labels.security_concerns,
                "sibling_predicted_security_concerns": sibling_surfaces.predicted_security_concerns,
                "cf_record_count": after_count,
                "reopened_record_count": reopened_rows.len(),
                "keys": keys,
                "vector_len": vector.len(),
                "vector_non_zero": vector.iter().filter(|value| **value != 0.0).count(),
                "vector_sha256": sha256_json(&vector)?,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "bandit_command_shape": "bandit -q -f json <file>",
            "semgrep_command_shape": "semgrep scan --json --metrics=off --disable-version-check --config <bundled local rules> <file>",
            "semgrep_network_policy": "bundled local rules avoid registry fetches in ME-JEPA action paths"
        }
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

fn happy_source(row_id: &str) -> Q4SecuritySource {
    Q4SecuritySource {
        corpus_row_id: row_id.to_string(),
        chunk_id: "chunk-q4-security-001".into(),
        logical_path: "app/security.py".into(),
        pre_patch_source: "def run(user_input):\n    print(user_input)\n".into(),
        post_patch_source: "import subprocess\n\ndef run(user_input):\n    subprocess.call(user_input, shell=True)\n".into(),
    }
}

fn infer_sibling_security_surface() -> FsvResult<context_graph_mejepa::PhaseBPredictionSurfaces> {
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: "app/security_sibling.py".into(),
                pre_sha: [0; 32],
                post_sha: [1; 32],
                before: "def run(user_input):\n    print(user_input)\n".to_string(),
                after: "import subprocess\n\ndef run(user_input):\n    subprocess.run(user_input, shell=True)\n"
                    .to_string(),
            }],
        },
        Vec::new(),
        "sibling command injection holdout".to_string(),
        [1; 32],
    )?;
    let context = TaskContext {
        task_id: TaskId("task-rwd-204-sibling".to_string()),
        session_id: [2; 16],
        language: Language::Python,
        problem_statement: "detect command injection sibling".to_string(),
        tests: vec![TestId("test_security".to_string())],
        environment: TaskEnvironment {
            repo_root: ".".into(),
            python_version: Some("3.12".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    Ok(infer_phase_b_surfaces(&patch, &context, &[0.95])?)
}

fn edge_toolchain_missing_quarantined(rocksdb_path: &Path) -> FsvResult<EdgeCase> {
    let source = happy_source("edge-toolchain-missing");
    let mut outputs = happy_outputs_for_source(&source);
    outputs.bandit_post.toolchain_missing = true;
    outputs.bandit_post.stdout.clear();
    let extraction = extract_q4_security_labels(&source, &outputs)?;
    let store = Q4SecurityLabelStore::open(rocksdb_path)?;
    let keys = store.put_extraction(&extraction)?;
    store.flush()?;
    let passed = extraction.quarantines.iter().any(|quarantine| {
        quarantine.reason_code == "Q4_SECURITY_TOOLCHAIN_MISSING"
            && quarantine.detector == Q4SecurityDetector::Bandit
    });
    Ok(EdgeCase {
        name: "toolchain_missing_quarantined",
        passed,
        detail: json!({"quarantines": extraction.quarantines, "keys": keys}),
    })
}

fn edge_fixed_finding_label() -> FsvResult<EdgeCase> {
    let source = Q4SecuritySource {
        corpus_row_id: "edge-fixed".into(),
        chunk_id: "chunk-edge-fixed".into(),
        logical_path: "app/config.py".into(),
        pre_patch_source: "API_KEY = 'abcdefghijklmnop'\n".into(),
        post_patch_source: "API_KEY = read_secret()\n".into(),
    };
    let outputs = Q4SecurityRawOutputs {
        bandit_pre: tool_output(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PrePatch,
            bandit_json(&[(
                "B105",
                "LOW",
                1,
                "Possible hardcoded password: 'abcdefghijklmnop'",
            )]),
        ),
        bandit_post: tool_output(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PostPatch,
            bandit_json(&[]),
        ),
        semgrep_pre: tool_output(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PrePatch,
            semgrep_json(&[]),
        ),
        semgrep_post: tool_output(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PostPatch,
            semgrep_json(&[]),
        ),
    };
    let extraction = extract_q4_security_labels(&source, &outputs)?;
    let passed = extraction
        .labels
        .iter()
        .any(|label| label.fixed_by_patch && label.class.as_str() == "hardcoded_secret");
    Ok(EdgeCase {
        name: "fixed_finding_label",
        passed,
        detail: json!({"labels": extraction.labels}),
    })
}

fn edge_unchanged_finding_dropped() -> FsvResult<EdgeCase> {
    let source = Q4SecuritySource {
        corpus_row_id: "edge-unchanged".into(),
        chunk_id: "chunk-edge-unchanged".into(),
        logical_path: "app/crypto.py".into(),
        pre_patch_source: "import hashlib\nhashlib.md5(b'x').hexdigest()\n".into(),
        post_patch_source: "import hashlib\nhashlib.md5(b'x').hexdigest()\n".into(),
    };
    let outputs = unchanged_outputs();
    let extraction = extract_q4_security_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "unchanged_finding_dropped",
        passed: extraction.labels.is_empty() && extraction.quarantines.is_empty(),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

fn edge_parser_failure_quarantined() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-parser");
    let mut outputs = happy_outputs_for_source(&source);
    outputs.semgrep_post.stdout = r#"{"results":[],"errors":[{"message":"parse failed"}]}"#.into();
    let extraction = extract_q4_security_labels(&source, &outputs)?;
    let passed = extraction.quarantines.iter().any(|quarantine| {
        quarantine.reason_code == "Q4_SECURITY_LABEL_PARSE_FAILURE"
            && quarantine.detector == Q4SecurityDetector::Semgrep
    });
    Ok(EdgeCase {
        name: "parser_failure_quarantined",
        passed,
        detail: json!({"quarantines": extraction.quarantines}),
    })
}

fn happy_outputs_for_source(_source: &Q4SecuritySource) -> Q4SecurityRawOutputs {
    Q4SecurityRawOutputs {
        bandit_pre: tool_output(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PrePatch,
            bandit_json(&[]),
        ),
        bandit_post: tool_output(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PostPatch,
            bandit_json(&[(
                "B602",
                "HIGH",
                4,
                "subprocess call with shell=True identified, security issue.",
            )]),
        ),
        semgrep_pre: tool_output(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PrePatch,
            semgrep_json(&[]),
        ),
        semgrep_post: tool_output(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PostPatch,
            semgrep_json(&[(
                "contextgraph.python.security.subprocess-shell-true",
                "ERROR",
                4,
                "subprocess with shell=True can route unsanitized input into a shell.",
            )]),
        ),
    }
}

fn unchanged_outputs() -> Q4SecurityRawOutputs {
    let bandit = bandit_json(&[(
        "B324",
        "MEDIUM",
        2,
        "Use of weak MD5 hash for security. Consider usedforsecurity=False",
    )]);
    Q4SecurityRawOutputs {
        bandit_pre: tool_output(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PrePatch,
            bandit.clone(),
        ),
        bandit_post: tool_output(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PostPatch,
            bandit,
        ),
        semgrep_pre: tool_output(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PrePatch,
            semgrep_json(&[]),
        ),
        semgrep_post: tool_output(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PostPatch,
            semgrep_json(&[]),
        ),
    }
}

fn install_security_shims(root: &Path) -> FsvResult<Option<OsString>> {
    let shim_dir = root.join("security-shims");
    fs::create_dir_all(&shim_dir)?;
    write_executable(&shim_dir.join("bandit"), bandit_shim())?;
    write_executable(&shim_dir.join("semgrep"), semgrep_shim())?;
    let old_path = env::var_os("PATH");
    let mut paths = vec![shim_dir];
    if let Some(old) = old_path.clone() {
        paths.extend(env::split_paths(&old));
    }
    env::set_var("PATH", env::join_paths(paths)?);
    Ok(old_path)
}

fn restore_path(old_path: Option<OsString>) {
    if let Some(value) = old_path {
        env::set_var("PATH", value);
    } else {
        env::remove_var("PATH");
    }
}

fn write_executable(path: &Path, content: &str) -> FsvResult<()> {
    fs::write(path, content.as_bytes())?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

fn bandit_shim() -> &'static str {
    r#"#!/bin/sh
target=""
for arg do
  target="$arg"
done
case "$target" in
  *post.py)
    printf '%s\n' '{"errors":[],"results":[{"filename":"app/security.py","issue_severity":"HIGH","issue_text":"subprocess call with shell=True identified, security issue.","line_number":4,"line_range":[4],"test_id":"B602","test_name":"subprocess_popen_with_shell_equals_true"}]}'
    exit 1
    ;;
  *)
    printf '%s\n' '{"errors":[],"results":[]}'
    exit 0
    ;;
esac
"#
}

fn semgrep_shim() -> &'static str {
    r#"#!/bin/sh
target=""
for arg do
  target="$arg"
done
case "$target" in
  *post.py)
    printf '%s\n' '{"errors":[],"results":[{"check_id":"contextgraph.python.security.subprocess-shell-true","path":"app/security.py","start":{"line":4,"col":5},"end":{"line":4,"col":45},"extra":{"message":"subprocess with shell=True can route unsanitized input into a shell.","severity":"ERROR","metadata":{"category":"security","vulnerability_class":["Command Injection"]}}}]}'
    ;;
  *)
    printf '%s\n' '{"errors":[],"results":[]}'
    ;;
esac
"#
}

fn tool_output(
    detector: Q4SecurityDetector,
    phase: Q4SecurityScanPhase,
    stdout: String,
) -> Q4SecurityToolOutput {
    Q4SecurityToolOutput {
        detector,
        phase,
        command: vec![detector.as_str().into()],
        status_code: Some(0),
        stdout,
        stderr: String::new(),
        runtime_exceeded: false,
        toolchain_missing: false,
    }
}

fn bandit_json(items: &[(&str, &str, u32, &str)]) -> String {
    let results = items
        .iter()
        .map(|(rule, severity, line, message)| {
            json!({
                "filename": "app/security.py",
                "issue_severity": severity,
                "issue_text": message,
                "line_number": line,
                "line_range": [line],
                "test_id": rule,
                "test_name": "test"
            })
        })
        .collect::<Vec<_>>();
    json!({"errors":[],"results":results}).to_string()
}

fn semgrep_json(items: &[(&str, &str, u32, &str)]) -> String {
    let results = items
        .iter()
        .map(|(rule, severity, line, message)| {
            json!({
                "check_id": rule,
                "path": "app/security.py",
                "start": {"line": line, "col": 1},
                "end": {"line": line, "col": 44},
                "extra": {
                    "message": message,
                    "severity": severity,
                    "metadata": {"category": "security", "vulnerability_class": ["Command Injection"]}
                }
            })
        })
        .collect::<Vec<_>>();
    json!({"errors":[],"results":results}).to_string()
}

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV requirement failed: {name}").into())
    }
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(format!("readback bytes differ for {}", path.display()).into());
    }
    Ok(())
}

fn sha256_json<T: Serialize>(value: &T) -> FsvResult<String> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
