//! BlackBoxFix Phase 8 enterprise-controls FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against real
//! SQLite ledgers and real audit export files. Assertions inspect the physical
//! source of truth directly: `ledger_records`, JSONL audit files, SIEM logs,
//! and export manifests.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase8_test";
const OTHER_TENANT: &str = "tenant_phase8_other";
const AGENT: &str = "agent_phase8_test";

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

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read JSON")).expect("parse JSON")
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let digest = Sha256::digest(&bytes);
    format!("sha256:{digest:x}")
}

fn init_ledger(ledger: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]))
}

fn enterprise_config() -> Value {
    json!({
        "tenant_id": TENANT,
        "sso": {
            "protocol": "oidc",
            "issuer": "https://idp.phase8.example.test/tenant_phase8_test",
            "client_id": "blackboxfix-phase8",
            "jwks_sha256": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "required_claims": ["iss", "sub", "aud", "exp"]
        },
        "rbac": {
            "roles": [
                {
                    "role_id": "security_admin",
                    "permissions": [
                        "dashboard.read",
                        "evidence.export",
                        "audit.export",
                        "policy.manage"
                    ]
                },
                {
                    "role_id": "viewer",
                    "permissions": ["dashboard.read"]
                }
            ]
        },
        "retention": {
            "default_class": "standard",
            "classes": [
                { "class_id": "standard", "retain_days": 365, "legal_hold": false },
                { "class_id": "incident", "retain_days": 2555, "legal_hold": true }
            ]
        },
        "deployment": {
            "mode": "private_vpc",
            "region": "us-test-1",
            "data_residency": "us",
            "kms_key_id": "kms_phase8_tenant_key_001",
            "siem_format": "rfc5424_jsonl"
        }
    })
}

fn enterprise_directory() -> Value {
    json!({
        "tenant_id": TENANT,
        "schemas": [
            "urn:ietf:params:scim:schemas:core:2.0:User",
            "urn:ietf:params:scim:schemas:core:2.0:Group"
        ],
        "users": [
            {
                "id": "user_alice",
                "userName": "alice.phase8@example.test",
                "active": true,
                "groups": ["group_secops"]
            },
            {
                "id": "user_bob",
                "userName": "bob.phase8@example.test",
                "active": true,
                "groups": ["group_viewers"]
            },
            {
                "id": "user_inactive",
                "userName": "inactive.phase8@example.test",
                "active": false,
                "groups": ["group_secops"]
            }
        ],
        "groups": [
            {
                "id": "group_secops",
                "displayName": "SecOps",
                "roles": ["security_admin"]
            },
            {
                "id": "group_viewers",
                "displayName": "Viewers",
                "roles": ["viewer"]
            }
        ]
    })
}

fn configure(ledger: &Path, input: &Path) -> Output {
    run_context_graph(&[
        "blackbox",
        "enterprise",
        "configure",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ])
}

fn sync_directory(ledger: &Path, input: &Path) -> Output {
    run_context_graph(&[
        "blackbox",
        "enterprise",
        "sync-directory",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ])
}

fn access_evaluate(ledger: &Path, principal: &str, permission: &str, resource: &str) -> Output {
    run_context_graph(&[
        "blackbox",
        "enterprise",
        "access-evaluate",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--principal-id",
        principal,
        "--permission",
        permission,
        "--resource",
        resource,
        "--agent",
        AGENT,
    ])
}

fn export_audit(ledger: &Path, principal: &str, out: &Path) -> Output {
    run_context_graph(&[
        "blackbox",
        "enterprise",
        "export-audit",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--principal-id",
        principal,
        "--out",
        out.to_str().expect("out path UTF-8"),
        "--agent",
        AGENT,
    ])
}

fn ledger_row_count(ledger: &Path) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .expect("count physical ledger rows")
}

fn tenant_row_count(ledger: &Path, tenant: &str) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE tenant_id = ?1",
        params![tenant],
        |row| row.get(0),
    )
    .expect("count tenant rows")
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
            params![record_kind],
            |row| row.get(0),
        )
        .expect("read latest payload");
    serde_json::from_str(&payload).expect("payload JSON")
}

fn jsonl_events(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("read JSONL")
        .lines()
        .map(|line| serde_json::from_str(line).expect("audit event JSON"))
        .collect()
}

fn siem_line_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .expect("read SIEM log")
        .lines()
        .count()
}

fn configured_case(temp: &TempDir) -> (PathBuf, PathBuf, PathBuf) {
    let ledger = temp.path().join("blackbox.sqlite");
    let config_path = temp.path().join("enterprise-config.json");
    let directory_path = temp.path().join("enterprise-directory.json");
    init_ledger(&ledger);
    write_json(&config_path, &enterprise_config());
    write_json(&directory_path, &enterprise_directory());
    stdout_json(&configure(&ledger, &config_path));
    stdout_json(&sync_directory(&ledger, &directory_path));
    (ledger, config_path, directory_path)
}

#[test]
fn phase8_happy_path_persists_enterprise_controls_and_exports_audit_files() {
    let temp = TempDir::new().expect("temp dir");
    let (ledger, _config_path, _directory_path) = configured_case(&temp);

    let access = stdout_json(&access_evaluate(
        &ledger,
        "user_alice",
        "audit.export",
        &format!("tenant:{TENANT}:audit/export"),
    ));
    assert_eq!(access["allowed"], true);
    let access_payload = latest_payload(&ledger, "enterprise_access_decision");
    assert_eq!(access_payload["allowed"], true);
    assert_eq!(access_payload["principal_id"], "user_alice");
    assert_eq!(access_payload["permission"], "audit.export");

    let before_export_rows = ledger_row_count(&ledger);
    let out = temp.path().join("audit-export");
    let export = stdout_json(&export_audit(&ledger, "user_alice", &out));
    assert_eq!(
        ledger_row_count(&ledger),
        before_export_rows + 1,
        "audit export must persist the authorization decision before writing files"
    );
    assert_eq!(
        export["audit_events_total"],
        tenant_row_count(&ledger, TENANT)
    );
    assert!(export["access_decision_id"].as_str().is_some());
    let export_access_payload = latest_payload(&ledger, "enterprise_access_decision");
    assert_eq!(export_access_payload["principal_id"], "user_alice");
    assert_eq!(export_access_payload["permission"], "audit.export");
    assert_eq!(export_access_payload["allowed"], true);

    let audit_path = out.join("audit.jsonl");
    let siem_path = out.join("siem-rfc5424.log");
    let controls_path = out.join("controls.json");
    let manifest_path = out.join("manifest.json");
    for path in [&audit_path, &siem_path, &controls_path, &manifest_path] {
        assert!(path.exists(), "export file must exist: {}", path.display());
    }

    let events = jsonl_events(&audit_path);
    assert_eq!(events.len() as i64, tenant_row_count(&ledger, TENANT));
    assert!(events
        .iter()
        .any(|event| event["record_kind"] == "enterprise_tenant_config"));
    assert!(events
        .iter()
        .any(|event| event["record_kind"] == "enterprise_directory_snapshot"));
    assert!(events
        .iter()
        .any(|event| event["record_kind"] == "enterprise_access_decision"));
    assert!(events.iter().all(|event| event["tenant_id"] == TENANT));

    assert_eq!(siem_line_count(&siem_path), events.len());
    let siem_text = fs::read_to_string(&siem_path).expect("read SIEM text");
    assert!(siem_text.lines().all(|line| line.starts_with("<134>1 ")));
    assert!(siem_text.contains("tenant_id=\"tenant_phase8_test\""));

    let controls = read_json(&controls_path);
    assert_eq!(controls["tenant_id"], TENANT);
    assert_eq!(controls["tenant_key_store"]["customer_managed"], true);
    assert_eq!(
        controls["tenant_key_store"]["kms_key_id"],
        "kms_phase8_tenant_key_001"
    );
    assert_eq!(controls["directory_summary"]["users_total"], 3);
    assert_eq!(controls["directory_summary"]["groups_total"], 2);
    assert_eq!(controls["authorization"]["allowed"], true);
    assert_eq!(
        controls["authorization_record"]["record_kind"],
        "enterprise_access_decision"
    );

    let manifest = read_json(&manifest_path);
    assert_eq!(
        manifest["format"],
        "blackbox_enterprise_audit_manifest_json_v1"
    );
    assert_eq!(manifest["audit_events_total"], events.len());
    assert_eq!(manifest["siem_events_total"], events.len());
    for file in manifest["files"].as_array().expect("manifest files array") {
        let rel = file["path"].as_str().expect("file path");
        let path = out.join(rel);
        assert_eq!(file["sha256"], sha256_file(&path), "manifest hash mismatch");
        assert_eq!(
            file["bytes"],
            fs::metadata(&path).expect("stat export file").len()
        );
    }

    let counts = ledger_kind_counts(&ledger);
    assert_eq!(counts.get("enterprise_tenant_config"), Some(&1));
    assert_eq!(counts.get("enterprise_directory_snapshot"), Some(&1));
    assert_eq!(counts.get("enterprise_access_decision"), Some(&2));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "enterprise_controls_v1",
        "--source",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "enterprise_controls_v1");
    assert_eq!(
        connector["readback"]["enterprise_controls"]["tenant_configs"],
        1
    );
    assert_eq!(
        connector["readback"]["enterprise_controls"]["directory_snapshots"],
        1
    );
    assert_eq!(
        connector["readback"]["enterprise_controls"]["access_decisions"],
        2
    );
}

#[test]
fn phase8_invalid_oidc_config_fails_before_persistence() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let config_path = temp.path().join("bad-config.json");
    init_ledger(&ledger);
    let mut config = enterprise_config();
    config["sso"]["required_claims"] = json!(["iss", "sub", "aud"]);
    write_json(&config_path, &config);
    let before = ledger_row_count(&ledger);

    let output = configure(&ledger, &config_path);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "ENTERPRISE_SSO_CONFIG_INVALID");
    assert_eq!(ledger_row_count(&ledger), before);
    assert_eq!(
        ledger_kind_counts(&ledger).get("enterprise_tenant_config"),
        None
    );
}

#[test]
fn phase8_directory_unknown_role_fails_before_snapshot_row() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let config_path = temp.path().join("enterprise-config.json");
    let directory_path = temp.path().join("bad-directory.json");
    init_ledger(&ledger);
    write_json(&config_path, &enterprise_config());
    stdout_json(&configure(&ledger, &config_path));
    let mut directory = enterprise_directory();
    directory["groups"][0]["roles"] = json!(["missing_role"]);
    write_json(&directory_path, &directory);
    let before = ledger_row_count(&ledger);

    let output = sync_directory(&ledger, &directory_path);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "ENTERPRISE_DIRECTORY_INVALID");
    assert_eq!(ledger_row_count(&ledger), before);
    assert_eq!(
        ledger_kind_counts(&ledger).get("enterprise_directory_snapshot"),
        None
    );
}

#[test]
fn phase8_cross_tenant_resource_persists_denied_access_decision() {
    let temp = TempDir::new().expect("temp dir");
    let (ledger, _config_path, _directory_path) = configured_case(&temp);
    let before = ledger_row_count(&ledger);

    let decision = stdout_json(&access_evaluate(
        &ledger,
        "user_alice",
        "audit.export",
        &format!("tenant:{OTHER_TENANT}:audit/export"),
    ));
    assert_eq!(decision["allowed"], false);
    assert_eq!(decision["reason_code"], "TENANT_SCOPE_DENIED");
    assert_eq!(ledger_row_count(&ledger), before + 1);
    let payload = latest_payload(&ledger, "enterprise_access_decision");
    assert_eq!(payload["allowed"], false);
    assert_eq!(payload["decision"], "denied");
    assert_eq!(payload["reason_code"], "TENANT_SCOPE_DENIED");
    assert_eq!(
        payload["resource"],
        format!("tenant:{OTHER_TENANT}:audit/export")
    );
}

#[test]
fn phase8_unauthorized_audit_export_persists_denial_without_files() {
    let temp = TempDir::new().expect("temp dir");
    let (ledger, _config_path, _directory_path) = configured_case(&temp);
    let out = temp.path().join("denied-export");
    let before = ledger_row_count(&ledger);

    let output = export_audit(&ledger, "user_bob", &out);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "ENTERPRISE_RBAC_DENIED");
    assert_eq!(ledger_row_count(&ledger), before + 1);
    let payload = latest_payload(&ledger, "enterprise_access_decision");
    assert_eq!(payload["principal_id"], "user_bob");
    assert_eq!(payload["allowed"], false);
    assert_eq!(payload["reason_code"], "RBAC_PERMISSION_DENIED");
    assert!(!out.exists(), "denied export must not create output dir");
}

#[test]
fn phase8_hash_chain_break_blocks_audit_export_before_files_are_written() {
    let temp = TempDir::new().expect("temp dir");
    let (ledger, _config_path, _directory_path) = configured_case(&temp);
    stdout_json(&access_evaluate(
        &ledger,
        "user_alice",
        "audit.export",
        &format!("tenant:{TENANT}:audit/export"),
    ));
    let out = temp.path().join("tampered-export");
    let before = ledger_row_count(&ledger);
    {
        let conn = Connection::open(&ledger).expect("open physical ledger");
        conn.execute_batch(
            "DROP TRIGGER ledger_records_no_update;
             UPDATE ledger_records
             SET payload_json = '{\"record_kind\":\"enterprise_tenant_config\",\"tampered\":true}'
             WHERE record_kind = 'enterprise_tenant_config';
             CREATE TRIGGER ledger_records_no_update
             BEFORE UPDATE ON ledger_records
             BEGIN
                 SELECT RAISE(ABORT, 'ledger_records is append-only; updates are forbidden');
             END;",
        )
        .expect("tamper ledger while restoring trigger");
    }

    let output = export_audit(&ledger, "user_alice", &out);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_HASH_CHAIN_BROKEN");
    assert_eq!(ledger_row_count(&ledger), before);
    assert!(
        !out.exists(),
        "tampered export must not create output directory"
    );
}
