#![allow(dead_code)]

use context_graph_mejepa::q4_trust_gate::{Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind};
use context_graph_mejepa_instruments::{
    extract_q4_cost_labels, run_q4_cost_tools_with_timeout, Q4CostCommandSpec, Q4CostExtraction,
    Q4CostKind, Q4CostLabelKind, Q4CostLabelStore, Q4CostRawOutputs, Q4CostScanPhase, Q4CostSource,
    Q4CostToolOutput,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
pub(crate) struct EdgeCase {
    pub(crate) name: &'static str,
    pub(crate) passed: bool,
    pub(crate) detail: Value,
}

pub(crate) fn happy_source(row_id: &str) -> Q4CostSource {
    Q4CostSource {
        corpus_row_id: row_id.to_string(),
        chunk_id: "chunk-q4-cost-001".to_string(),
        logical_path: "requirements.txt".to_string(),
        cost_selector: "pytest-and-build-wheel".to_string(),
        changed_paths: vec!["requirements.txt".to_string()],
    }
}

pub(crate) fn alternate_chunk_source(row_id: &str) -> Q4CostSource {
    Q4CostSource {
        corpus_row_id: row_id.to_string(),
        chunk_id: "chunk-q4-cost-002".to_string(),
        logical_path: "pyproject.toml".to_string(),
        cost_selector: "pytest-and-build-wheel-alt-manifest".to_string(),
        changed_paths: vec!["pyproject.toml".to_string()],
    }
}

pub(crate) fn write_cost_fixture(root: &Path) -> FsvResult<PathBuf> {
    fs::create_dir_all(root)?;
    write_project_state(root, "pre", 12, 600.0, Some(10_000))?;
    write_project_state(root, "post", 14, 750.0, Some(12_048))?;
    write_project_state(root, "no_reqs_pre", 0, 60.0, None)?;
    write_project_state(root, "no_reqs_post", 0, 66.0, None)?;
    write_project_state(root, "fix_pre", 14, 60.0, None)?;
    write_project_state(root, "fix_post", 12, 60.0, None)?;
    write_failing_wheel_state(root)?;
    fs::write(
        root.join("requirements.patch"),
        "\
diff --git a/requirements.txt b/requirements.txt\n\
--- a/requirements.txt\n\
+++ b/requirements.txt\n\
@@\n\
+numpy==2.0\n\
+scipy==1.14\n",
    )?;
    let script = root.join("q4_cost_fixture.py");
    let source = r#"
import argparse
import os
import json
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parent


def emit(case: str) -> None:
    if case in {"pre", "post", "no_reqs_pre", "no_reqs_post", "fix_pre", "fix_post"}:
        emit_project(ROOT / case)
    elif case == "noisy_pre":
        payload = {"metrics": [{"kind": "ci_minutes", "value": 10.0, "stddev": 3.0}, {"kind": "dependency_count", "value": 12}]}
        print(json.dumps(payload, sort_keys=True))
    elif case == "noisy_post":
        payload = {"metrics": [{"kind": "ci_minutes", "value": 12.0, "stddev": 0.4}, {"kind": "dependency_count", "value": 14}]}
        print(json.dumps(payload, sort_keys=True))
    elif case == "wheel_fail":
        emit_failing_wheel(ROOT / "wheel_fail")
    else:
        raise ValueError(f"unknown case: {case}")


def emit_project(project: Path) -> None:
    env = tool_env()
    pytest = subprocess.run(
        [sys.executable, "-m", "pytest", "-q"],
        cwd=project,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=30,
        check=False,
    )
    print(pytest.stdout.strip())
    if pytest.returncode != 0:
        raise SystemExit(pytest.returncode)
    requirements = project / "requirements.txt"
    if requirements.exists():
        print("REQUIREMENTS_START")
        print(requirements.read_text().strip())
        print("REQUIREMENTS_END")
    dist = project / "dist"
    if dist.exists():
        shutil.rmtree(dist)
    wheel = subprocess.run(
        [
            sys.executable,
            "-m",
            "pip",
            "wheel",
            "--disable-pip-version-check",
            "--no-deps",
            "--no-build-isolation",
            ".",
            "-w",
            "dist",
        ],
        cwd=project,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=60,
        check=False,
    )
    print("WHEEL_BUILD_STDOUT_START")
    print(wheel.stdout.strip())
    print("WHEEL_BUILD_STDOUT_END")
    if wheel.returncode != 0:
        raise SystemExit(wheel.returncode)
    wheels = sorted((project / "dist").glob("*.whl"))
    if wheels:
        wheel = wheels[0]
        print("WHEEL_START")
        print(f"wheel_bytes: {wheel.stat().st_size} {wheel}")
        print("WHEEL_END")


def emit_failing_wheel(project: Path) -> None:
    env = tool_env()
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "pip",
            "wheel",
            "--disable-pip-version-check",
            "--no-deps",
            "--no-build-isolation",
            ".",
            "-w",
            "dist",
        ],
        cwd=project,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=60,
        check=False,
    )
    print(result.stdout.strip())
    raise SystemExit(result.returncode or 1)


def tool_env() -> dict[str, str]:
    env = os.environ.copy()
    if has_pytest(env):
        return env
    vendor = ROOT / ".pytest_vendor"
    if not (vendor / "pytest").exists():
        subprocess.run(
            [
                sys.executable,
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--target",
                str(vendor),
                "pytest==9.0.3",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=120,
            check=True,
        )
    existing = env.get("PYTHONPATH")
    env["PYTHONPATH"] = str(vendor) if not existing else f"{vendor}{os.pathsep}{existing}"
    if not has_pytest(env):
        raise RuntimeError("pytest toolchain bootstrap failed")
    return env


def has_pytest(env: dict[str, str]) -> bool:
    probe = subprocess.run(
        [sys.executable, "-m", "pytest", "--version"],
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=15,
        check=False,
    )
    return probe.returncode == 0


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("case")
    args = parser.parse_args()
    emit(args.case)


if __name__ == "__main__":
    main()
"#;
    fs::write(&script, source.trim_start())?;
    Ok(script)
}

pub(crate) fn run_fixture_outputs(
    fixture_root: &Path,
    fixture_script: &Path,
    pre_case: &str,
    post_case: &str,
) -> FsvResult<Q4CostRawOutputs> {
    Ok(run_q4_cost_tools_with_timeout(
        &fixture_command(fixture_root, fixture_script, pre_case),
        &fixture_command(fixture_root, fixture_script, post_case),
        Duration::from_secs(30),
    )?)
}

fn write_project_state(
    root: &Path,
    state: &str,
    dependency_count: usize,
    duration_seconds: f64,
    wheel_bytes: Option<usize>,
) -> FsvResult<()> {
    let state_root = root.join(state);
    fs::create_dir_all(&state_root)?;
    fs::write(
        state_root.join("pyproject.toml"),
        r#"[build-system]
requires = ["setuptools", "wheel"]
build-backend = "setuptools.build_meta"

[project]
name = "q4-cost-fixture"
version = "0.1.0"

[tool.setuptools.packages.find]
where = ["src"]

[tool.setuptools.package-data]
demo_cost = ["payload.bin"]
"#,
    )?;
    let package_root = state_root.join("src").join("demo_cost");
    fs::create_dir_all(&package_root)?;
    fs::write(
        package_root.join("__init__.py"),
        "from .core import workload\n",
    )?;
    let loop_count = (duration_seconds / 0.02).max(1.0) as usize;
    fs::write(
        package_root.join("core.py"),
        format!(
            "def workload():\n    total = 0\n    for idx in range({loop_count}):\n        total += (idx * 17) % 31\n    return total\n"
        ),
    )?;
    let payload_size = wheel_bytes.unwrap_or(512);
    let payload = (0..payload_size)
        .map(|idx| ((idx * 37 + payload_size) % 251) as u8)
        .collect::<Vec<_>>();
    fs::write(package_root.join("payload.bin"), payload)?;
    let tests_root = state_root.join("tests");
    fs::create_dir_all(&tests_root)?;
    fs::write(
        tests_root.join("test_cost.py"),
        "import sys\nfrom pathlib import Path\n\nsys.path.insert(0, str(Path(__file__).resolve().parents[1] / \"src\"))\nfrom demo_cost import workload\n\n\ndef test_cost():\n    assert workload() >= 0\n",
    )?;
    if dependency_count > 0 {
        let mut deps = (0..dependency_count)
            .map(|idx| format!("dep{idx}==1.0"))
            .collect::<Vec<_>>();
        if state == "post" {
            deps.truncate(12);
            deps.push("numpy==2.0".to_string());
            deps.push("scipy==1.14".to_string());
        }
        fs::write(state_root.join("requirements.txt"), deps.join("\n") + "\n")?;
    }
    Ok(())
}

fn write_failing_wheel_state(root: &Path) -> FsvResult<()> {
    let state_root = root.join("wheel_fail");
    fs::create_dir_all(&state_root)?;
    fs::write(state_root.join("pyproject.toml"), "[build-system\n")?;
    Ok(())
}

pub(crate) fn persist_extraction(
    store: &Q4CostLabelStore,
    extraction: &Q4CostExtraction,
) -> FsvResult<usize> {
    let keys = store.put_extraction(extraction)?;
    Ok(keys.len())
}

pub(crate) fn update_q4_catalog(
    path: &Path,
    producer_rows: u64,
    producer_fsv_root: &Path,
) -> FsvResult<Q4EvidenceCatalog> {
    let mut catalog = if path.exists() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        Q4EvidenceCatalog::default()
    };
    let mut heads: BTreeMap<_, _> = std::mem::take(&mut catalog.heads);
    heads.insert(
        Q4HeadKind::Cost,
        Q4HeadEvidence {
            producer_fsv_root: Some(canonical_fsv_root(producer_fsv_root)),
            producer_rows,
            calibration_fsv_root: None,
            calibration_rows: 0,
            per_slot_evidence_root: None,
            slots_with_evidence: Vec::new(),
        },
    );
    catalog.heads = heads;
    write_json_checked(path, &catalog)?;
    Ok(catalog)
}

pub(crate) fn canonical_fsv_root(path: &Path) -> String {
    let mut raw = path.display().to_string();
    if !raw.ends_with('/') {
        raw.push('/');
    }
    raw
}

pub(crate) fn edge_no_requirements_absent_dep_label(
    fixture_root: &Path,
    fixture_script: &Path,
) -> FsvResult<EdgeCase> {
    let mut source = happy_source("edge-no-requirements");
    source.logical_path = "app/module.py".to_string();
    source.changed_paths = vec!["app/module.py".to_string()];
    let outputs = run_fixture_outputs(fixture_root, fixture_script, "no_reqs_pre", "no_reqs_post")?;
    let extraction = extract_q4_cost_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "no_requirements_absent_dep_label",
        passed: !extraction
            .labels
            .iter()
            .any(|label| label.kind == Q4CostKind::DependencyCount)
            && extraction.quarantines.is_empty(),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_wheel_build_failure_quarantined(
    fixture_root: &Path,
    fixture_script: &Path,
) -> FsvResult<EdgeCase> {
    let source = happy_source("edge-wheel-build-fail");
    let outputs = run_fixture_outputs(fixture_root, fixture_script, "pre", "wheel_fail")?;
    let extraction = extract_q4_cost_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "wheel_build_failure_quarantined",
        passed: extraction.labels.is_empty()
            && extraction
                .quarantines
                .iter()
                .any(|item| item.reason_code == "Q4_COST_LABEL_BUILD_FAILED"),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_dependency_removal_improvement(
    fixture_root: &Path,
    fixture_script: &Path,
) -> FsvResult<EdgeCase> {
    let source = happy_source("edge-dependency-removal");
    let outputs = run_fixture_outputs(fixture_root, fixture_script, "fix_pre", "fix_post")?;
    let extraction = extract_q4_cost_labels(&source, &outputs)?;
    let label = extraction
        .labels
        .iter()
        .find(|label| label.kind == Q4CostKind::DependencyCount)
        .ok_or("missing dependency improvement label")?;
    Ok(EdgeCase {
        name: "dependency_removal_improvement",
        passed: label.kind == Q4CostKind::DependencyCount
            && label.delta == -2.0
            && !label.regression
            && label.label_kind == Q4CostLabelKind::Improvement,
        detail: json!({"label": label, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_noisy_walltime_only(
    fixture_root: &Path,
    fixture_script: &Path,
) -> FsvResult<EdgeCase> {
    let source = happy_source("edge-noisy-walltime");
    let outputs = run_fixture_outputs(fixture_root, fixture_script, "noisy_pre", "noisy_post")?;
    let extraction = extract_q4_cost_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "noisy_walltime_quarantines_only_ci_minutes",
        passed: extraction
            .quarantines
            .iter()
            .any(|item| item.reason_code == "Q4_COST_LABEL_UNSTABLE_WALLTIME")
            && !extraction
                .labels
                .iter()
                .any(|label| label.kind == Q4CostKind::CiMinutes)
            && extraction
                .labels
                .iter()
                .any(|label| label.kind == Q4CostKind::DependencyCount && label.regression),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn persisted_quarantine_fixture() -> FsvResult<Q4CostExtraction> {
    let source = happy_source("edge-persisted-cost-quarantines");
    let outputs = Q4CostRawOutputs {
        pre_patch: output(Q4CostScanPhase::PrePatch, r#"{"dependency_count": 12}"#),
        post_patch: output(Q4CostScanPhase::PostPatch, r#"{"ci_minutes": 2.0}"#),
    };
    let extraction = extract_q4_cost_labels(&source, &outputs)?;
    require(
        "persisted quarantine fixture emits two missing-direction rows",
        extraction.labels.is_empty() && extraction.quarantines.len() == 2,
    )?;
    Ok(extraction)
}

pub(crate) fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json readback matches write", readback == bytes)?;
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

pub(crate) fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub(crate) fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("requirement failed: {name}").into())
    }
}

fn fixture_command(cwd: &Path, script: &Path, case_name: &str) -> Q4CostCommandSpec {
    Q4CostCommandSpec {
        program: "python3".to_string(),
        args: vec![script.display().to_string(), case_name.to_string()],
        cwd: cwd.to_path_buf(),
    }
}

fn output(phase: Q4CostScanPhase, stdout: &str) -> Q4CostToolOutput {
    Q4CostToolOutput {
        phase,
        command: vec!["python3".to_string()],
        status_code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
        runtime_exceeded: false,
        toolchain_missing: false,
    }
}
