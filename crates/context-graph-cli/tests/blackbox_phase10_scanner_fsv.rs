//! BlackBoxFix Phase 10 scanner artifact FSV tests.
//!
//! These tests do not mock scanner state. They write real OSV JSON,
//! Cargo.lock, and CycloneDX JSON artifacts to disk, run the real CLI, and
//! query the physical SQLite ledger to verify persisted evidence.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase10_scanner_test";
const AGENT: &str = "agent_phase10_scanner_test";
const VULN_ID: &str = "GHSA-m5pq-gvj9-9vr8";

struct Phase10Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    run_id: String,
    tool_call_id: String,
    clean_report: PathBuf,
    vulnerable_report: PathBuf,
    invalid_json: PathBuf,
    lockfile: PathBuf,
    sbom: PathBuf,
    invalid_sbom: PathBuf,
    missing_report: PathBuf,
    missing_sbom: PathBuf,
}

fn context_graph_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_context-graph") {
        return PathBuf::from(path);
    }
    let current = std::env::current_exe().expect("current test executable path");
    current
        .parent()
        .and_then(|deps| deps.parent())
        .map(|target_debug| target_debug.join("context-graph"))
        .expect("target/debug path")
}

fn run_context_graph(args: &[&str]) -> Output {
    Command::new(context_graph_bin())
        .args(args)
        .output()
        .expect("spawn context-graph binary")
}

fn stdout_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "command failed\nstatus={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout JSON")
}

fn stderr_json(output: &Output) -> Value {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stderr).expect("stderr JSON")
}

fn write_json(path: &Path, value: &Value) {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("serialize JSON"),
    )
    .expect("write JSON file");
}

fn write_text(path: &Path, text: &str) {
    fs::write(path, text).expect("write text file");
}

fn init_ledger(ledger: &Path) {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
}

fn record_run(ledger: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "run",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--task",
        "phase10 scanner artifact source-of-truth test",
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--risk-tier",
        "high",
    ]))
}

fn record_tool_call(ledger: &Path, run_id: &str, input: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "tool-call",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--tool",
        "phase10_scanner_read",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase10.scanner.read",
    ]))
}

fn new_phase10_case() -> Phase10Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let clean_report = root.join("osv-clean.json");
    let vulnerable_report = root.join("osv-vulnerable.json");
    let invalid_json = root.join("osv-invalid.json");
    let lockfile = root.join("Cargo.lock");
    let sbom = root.join("bom.json");
    let invalid_sbom = root.join("invalid-bom.json");
    let missing_report = root.join("missing-osv.json");
    let missing_sbom = root.join("missing-bom.json");

    let clean_osv = json!({
        "results": [{
            "source": { "path": lockfile, "type": "lockfile" },
            "packages": [{
                "package": {
                    "name": "regex",
                    "version": "1.10.3",
                    "ecosystem": "crates.io"
                },
                "vulnerabilities": [],
                "groups": []
            }]
        }]
    });
    let vulnerable_osv = json!({
        "results": [{
            "source": { "path": lockfile, "type": "lockfile" },
            "packages": [{
                "package": {
                    "name": "regex",
                    "version": "1.5.1",
                    "ecosystem": "crates.io"
                },
                "vulnerabilities": [{
                    "id": VULN_ID,
                    "aliases": ["CVE-2022-24713", "RUSTSEC-2022-0013"]
                }],
                "groups": [{
                    "ids": [VULN_ID, "CVE-2022-24713", "RUSTSEC-2022-0013"]
                }]
            }]
        }]
    });
    let cyclonedx = json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "version": 1,
        "components": [{
            "type": "library",
            "name": "regex",
            "version": "1.10.3",
            "bom-ref": "pkg:cargo/regex@1.10.3",
            "purl": "pkg:cargo/regex@1.10.3"
        }]
    });
    let invalid_cyclonedx = json!({
        "bomFormat": "SPDX",
        "specVersion": "1.6",
        "version": 1,
        "components": []
    });

    write_json(&clean_report, &clean_osv);
    write_json(&vulnerable_report, &vulnerable_osv);
    write_text(&invalid_json, "{not valid json");
    write_text(
        &lockfile,
        r#"# This file is automatically @generated by Cargo.
version = 3

[[package]]
name = "regex"
version = "1.10.3"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "phase10regexchecksum"

[[package]]
name = "serde"
version = "1.0.203"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "phase10serdechecksum"
"#,
    );
    write_json(&sbom, &cyclonedx);
    write_json(&invalid_sbom, &invalid_cyclonedx);

    init_ledger(&ledger);
    let tool_input = root.join("tool-input.json");
    write_json(
        &tool_input,
        &json!({
            "operation": "phase10-scanner-artifact-read",
            "clean_report": clean_report,
            "vulnerable_report": vulnerable_report,
            "lockfile": lockfile,
            "sbom": sbom
        }),
    );
    let run = record_run(&ledger);
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase10Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
        clean_report,
        vulnerable_report,
        invalid_json,
        lockfile,
        sbom,
        invalid_sbom,
        missing_report,
        missing_sbom,
    }
}

fn verifier_spec(case: &Phase10Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn run_verify(case: &Phase10Case, spec: Value) -> Output {
    let spec_path = case.root.join("verify-spec.json");
    write_json(&spec_path, &spec);
    run_context_graph(&[
        "blackbox",
        "verify",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        spec_path.to_str().expect("spec path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ])
}

fn ledger_kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let mut stmt = conn
        .prepare(
            "SELECT record_kind, COUNT(*)
             FROM ledger_records
             GROUP BY record_kind
             ORDER BY record_kind",
        )
        .expect("prepare record kind count");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query record kind count");
    rows.map(|row| row.expect("record kind count row"))
        .collect()
}

fn latest_payload(ledger: &Path, record_kind: &str) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = ?1
             ORDER BY sequence DESC
             LIMIT 1",
            [record_kind],
            |row| row.get(0),
        )
        .expect("read latest ledger payload");
    serde_json::from_str(&payload).expect("ledger payload JSON")
}

fn assert_kind_count(ledger: &Path, kind: &str, expected: i64) {
    let counts = ledger_kind_counts(ledger);
    assert_eq!(
        counts.get(kind).copied().unwrap_or(0),
        expected,
        "ledger count for {kind}: {counts:?}"
    );
}

fn assert_latest_verifier(
    ledger: &Path,
    verifier_id: &str,
    status: &str,
    error_code: Option<&str>,
) -> Value {
    let payload = latest_payload(ledger, "verifier_result");
    assert_eq!(payload["verifier_id"], verifier_id);
    assert_eq!(payload["status"], status);
    if let Some(error_code) = error_code {
        assert_eq!(payload["error_code"], error_code);
    } else {
        assert!(payload["error_code"].is_null());
    }
    payload
}

fn assert_edge(
    case: &Phase10Case,
    verifier_id: &str,
    source: Value,
    expected: Value,
    status: &str,
    error_code: &str,
    readback_delta: i64,
    incident_delta: i64,
) -> Value {
    let before = ledger_kind_counts(&case.ledger);
    let output = run_verify(case, verifier_spec(case, verifier_id, source, expected));
    assert_eq!(stderr_json(&output)["error_code"], error_code);
    let after = ledger_kind_counts(&case.ledger);
    assert_eq!(
        after.get("expected_delta").copied().unwrap_or(0),
        before.get("expected_delta").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("readback_snapshot").copied().unwrap_or(0),
        before.get("readback_snapshot").copied().unwrap_or(0) + readback_delta
    );
    assert_eq!(
        after.get("verifier_result").copied().unwrap_or(0),
        before.get("verifier_result").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("evidence_packet").copied().unwrap_or(0),
        before.get("evidence_packet").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("incident_packet").copied().unwrap_or(0),
        before.get("incident_packet").copied().unwrap_or(0) + incident_delta
    );
    assert_latest_verifier(&case.ledger, verifier_id, status, Some(error_code))
}

fn source_report(path: &Path) -> Value {
    json!({
        "report": path,
        "report_format": "osv-json"
    })
}

fn source_lockfile(path: &Path, format: &str) -> Value {
    json!({
        "lockfile": path,
        "lockfile_format": format
    })
}

fn source_sbom(path: &Path) -> Value {
    json!({
        "sbom": path,
        "sbom_format": "cyclonedx-json"
    })
}

#[test]
fn phase10_scanner_artifact_fsv() {
    let case = new_phase10_case();

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "scanner_artifact_v1"));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "scanner_artifact_v1",
        "--source",
        case.vulnerable_report.to_str().expect("report path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "scanner_artifact_v1");
    assert_eq!(
        connector["readback"]["normalized"]["vulnerabilities_total"],
        1
    );

    let vuln_absent = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "scanner_vulnerability_absent_v1",
            source_report(&case.clean_report),
            json!({
                "vulnerability_id": VULN_ID,
                "package": "regex",
                "ecosystem": "crates.io",
                "version": "1.10.3"
            }),
        ),
    ));
    assert_eq!(vuln_absent["verifier_status"], "passed");
    assert_eq!(vuln_absent["actual_readback"]["matched_count"], 0);

    let lockfile = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "scanner_lockfile_pinned_to_version_v1",
            source_lockfile(&case.lockfile, "cargo"),
            json!({
                "package": "regex",
                "version": "1.10.3"
            }),
        ),
    ));
    assert_eq!(lockfile["verifier_status"], "passed");
    assert_eq!(lockfile["actual_readback"]["matching_count"], 1);

    let sbom = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "scanner_sbom_artifact_present_v1",
            source_sbom(&case.sbom),
            json!({
                "minimum_components": 1,
                "component_name": "regex",
                "component_version": "1.10.3"
            }),
        ),
    ));
    assert_eq!(sbom["verifier_status"], "passed");
    assert_eq!(sbom["actual_readback"]["components_total"], 1);

    assert_kind_count(&case.ledger, "agent_run", 1);
    assert_kind_count(&case.ledger, "tool_call", 1);
    assert_kind_count(&case.ledger, "expected_delta", 3);
    assert_kind_count(&case.ledger, "readback_snapshot", 3);
    assert_kind_count(&case.ledger, "verifier_result", 3);
    assert_kind_count(&case.ledger, "evidence_packet", 3);
    assert_kind_count(&case.ledger, "incident_packet", 0);
    assert_eq!(
        latest_payload(&case.ledger, "readback_snapshot")["source_of_truth"]["kind"],
        "scanner_artifact_v1"
    );

    let present = assert_edge(
        &case,
        "scanner_vulnerability_absent_v1",
        source_report(&case.vulnerable_report),
        json!({ "vulnerability_id": VULN_ID }),
        "failed",
        "SCANNER_VULNERABILITY_STILL_PRESENT",
        1,
        1,
    );
    assert_eq!(present["actual_readback"]["matched_count"], 1);

    let missing_report = assert_edge(
        &case,
        "scanner_vulnerability_absent_v1",
        source_report(&case.missing_report),
        json!({ "vulnerability_id": VULN_ID }),
        "failed",
        "SCANNER_REPORT_NOT_FOUND",
        1,
        1,
    );
    assert_eq!(
        missing_report["actual_readback"]["artifact"]["exists"],
        false
    );

    let invalid_json = assert_edge(
        &case,
        "scanner_vulnerability_absent_v1",
        source_report(&case.invalid_json),
        json!({ "vulnerability_id": VULN_ID }),
        "blocked",
        "SCANNER_ARTIFACT_JSON_INVALID",
        1,
        0,
    );
    assert!(invalid_json["actual_readback"]["parse_error"].is_string());

    let wrong_version = assert_edge(
        &case,
        "scanner_lockfile_pinned_to_version_v1",
        source_lockfile(&case.lockfile, "cargo"),
        json!({
            "package": "regex",
            "version": "9.9.9"
        }),
        "failed",
        "SCANNER_LOCKFILE_VERSION_MISMATCH",
        1,
        1,
    );
    assert_eq!(wrong_version["actual_readback"]["mismatched_count"], 1);

    let missing_package = assert_edge(
        &case,
        "scanner_lockfile_pinned_to_version_v1",
        source_lockfile(&case.lockfile, "cargo"),
        json!({
            "package": "missing-package",
            "version": "1.0.0"
        }),
        "failed",
        "SCANNER_LOCKFILE_PACKAGE_NOT_FOUND",
        1,
        1,
    );
    assert_eq!(missing_package["actual_readback"]["matching_count"], 0);

    assert_edge(
        &case,
        "scanner_lockfile_pinned_to_version_v1",
        source_lockfile(&case.lockfile, "npm"),
        json!({
            "package": "regex",
            "version": "1.10.3"
        }),
        "blocked",
        "SCANNER_ARTIFACT_FORMAT_UNSUPPORTED",
        0,
        0,
    );

    let missing_sbom = assert_edge(
        &case,
        "scanner_sbom_artifact_present_v1",
        source_sbom(&case.missing_sbom),
        json!({
            "minimum_components": 1,
            "component_name": "regex",
            "component_version": "1.10.3"
        }),
        "failed",
        "SCANNER_SBOM_NOT_FOUND",
        1,
        1,
    );
    assert_eq!(missing_sbom["actual_readback"]["artifact"]["exists"], false);

    let invalid_sbom = assert_edge(
        &case,
        "scanner_sbom_artifact_present_v1",
        source_sbom(&case.invalid_sbom),
        json!({
            "minimum_components": 1,
            "component_name": "regex",
            "component_version": "1.10.3"
        }),
        "blocked",
        "SCANNER_SBOM_SCHEMA_DRIFT",
        1,
        0,
    );
    assert!(invalid_sbom["actual_readback"]["schema_error"].is_string());

    assert_edge(
        &case,
        "scanner_sbom_artifact_present_v1",
        source_sbom(&case.sbom),
        json!({
            "minimum_components": 0,
            "component_name": "regex",
            "component_version": "1.10.3"
        }),
        "blocked",
        "SCANNER_SBOM_MINIMUM_COMPONENTS_INVALID",
        0,
        0,
    );

    assert_kind_count(&case.ledger, "expected_delta", 12);
    assert_kind_count(&case.ledger, "readback_snapshot", 10);
    assert_kind_count(&case.ledger, "verifier_result", 12);
    assert_kind_count(&case.ledger, "evidence_packet", 12);
    assert_kind_count(&case.ledger, "incident_packet", 5);
}
