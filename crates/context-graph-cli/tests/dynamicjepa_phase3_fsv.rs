//! Phase 3 DynamicJEPA CLI full-state verification.
//!
//! This test executes the real CLI binary against a real RocksDB database and
//! verifies persisted rows through independent read commands. It does not mock
//! storage or assert command return values without checking the source of truth.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn context_graph_bin() -> &'static str {
    env!("CARGO_BIN_EXE_context-graph")
}

fn run_cli(args: &[&str]) -> (bool, Value) {
    let output = Command::new(context_graph_bin())
        .args(args)
        .env("CONTEXT_GRAPH_ENV", "development")
        .output()
        .expect("execute context-graph binary");
    let stdout = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr is UTF-8");
    assert!(
        !stdout.trim().is_empty(),
        "CLI produced empty stdout; stderr={stderr}"
    );
    let value: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout must be one JSON object: {err}\nstdout={stdout}"));
    (output.status.success(), value)
}

fn run_ok(args: &[&str]) -> Value {
    let (success, value) = run_cli(args);
    assert!(
        success,
        "command expected success but failed:\n{}",
        serde_json::to_string_pretty(&value).expect("pretty JSON")
    );
    assert_eq!(value["status"], "ok");
    value
}

fn run_err(args: &[&str]) -> Value {
    let (success, value) = run_cli(args);
    assert!(
        !success,
        "command expected failure but succeeded:\n{}",
        serde_json::to_string_pretty(&value).expect("pretty JSON")
    );
    assert_eq!(value["status"], "error");
    value
}

fn count(value: &Value, cf: &str) -> u64 {
    value["counts"][cf]
        .as_u64()
        .or_else(|| value["after_counts"][cf].as_u64())
        .unwrap_or_else(|| panic!("count for {cf} missing in {value:#}"))
}

fn counts(value: &Value) -> Value {
    if value.get("counts").is_some() {
        value["counts"].clone()
    } else {
        value["after_counts"].clone()
    }
}

#[test]
fn dynamicjepa_phase3_cli_fsv_real_rocksdb() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let db_path = temp.path().join("dynamicjepa_phase3_rocksdb");
    let db = db_path.display().to_string();
    let root = repo_root();
    let counter = root
        .join("configs/dynamicjepa/domain_packs/counter_world.v1.toml")
        .display()
        .to_string();
    let gridworld = root
        .join("configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml")
        .display()
        .to_string();
    let evidence_dir = root.join("tmp/5090jepa_evidence/phase3");
    fs::create_dir_all(&evidence_dir).expect("create Phase 3 evidence dir");

    let mut evidence = Vec::new();

    let initial = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(count(&initial, "dj_domain_packs"), 0);
    evidence.push(json!({"step": "initial_counts", "output": initial}));

    let counter_registration = run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &db,
        "--file",
        &counter,
        "--json",
    ]);
    assert_eq!(counter_registration["created_ids"]["domain_pack_id"], "counter_world");
    assert_eq!(
        counter_registration["decoded_records"]["domain_pack"]["instrument_specs"]
            .as_array()
            .expect("instrument specs array")
            .len(),
        2
    );
    evidence.push(json!({"step": "register_counter_world", "output": counter_registration}));

    let counts_after_counter =
        run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(count(&counts_after_counter, "dj_domain_packs"), 1);
    assert_eq!(count(&counts_after_counter, "dj_domain_pack_by_nv"), 1);
    assert_eq!(count(&counts_after_counter, "dj_instrument_registry"), 2);
    assert_eq!(count(&counts_after_counter, "dj_adapter_registry"), 1);
    assert_eq!(count(&counts_after_counter, "dj_audit_log"), 1);
    evidence.push(json!({"step": "counts_after_counter_world", "output": counts_after_counter}));

    let get_counter = run_ok(&[
        "dynamicjepa",
        "get-domain",
        "--db",
        &db,
        "--id",
        "counter_world",
        "--version",
        "1.0.0",
        "--json",
    ]);
    assert_eq!(
        get_counter["decoded_records"]["domain_pack"]["source_hash"]
            .as_array()
            .expect("source hash array")
            .len(),
        32
    );
    evidence.push(json!({"step": "get_counter_world", "output": get_counter}));

    for cf in [
        "dj_domain_packs",
        "dj_domain_pack_by_nv",
        "dj_instrument_registry",
        "dj_adapter_registry",
        "dj_audit_log",
    ] {
        let inspected = run_ok(&[
            "dynamicjepa",
            "inspect-cf",
            "--db",
            &db,
            "--cf",
            cf,
            "--limit",
            "10",
            "--json",
        ]);
        assert!(
            !inspected["decoded_records"]["rows"]
                .as_array()
                .expect("decoded rows array")
                .is_empty(),
            "{cf} must contain physical decoded rows"
        );
        evidence.push(json!({"step": format!("inspect_{cf}"), "output": inspected}));
    }

    let gridworld_registration = run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &db,
        "--file",
        &gridworld,
        "--json",
    ]);
    assert_eq!(
        gridworld_registration["created_ids"]["domain_pack_id"],
        "gridworld_5x5"
    );
    assert_eq!(
        gridworld_registration["decoded_records"]["domain_pack"]["instrument_specs"]
            .as_array()
            .expect("gridworld instrument specs")
            .len(),
        3
    );
    evidence.push(json!({"step": "register_gridworld_5x5", "output": gridworld_registration}));

    let counts_after_both = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(count(&counts_after_both, "dj_domain_packs"), 2);
    assert_eq!(count(&counts_after_both, "dj_domain_pack_by_nv"), 2);
    assert_eq!(count(&counts_after_both, "dj_instrument_registry"), 5);
    assert_eq!(count(&counts_after_both, "dj_adapter_registry"), 2);
    assert_eq!(count(&counts_after_both, "dj_audit_log"), 2);
    evidence.push(json!({"step": "counts_after_both_domains", "output": counts_after_both}));

    let listed = run_ok(&[
        "dynamicjepa",
        "list-domains",
        "--db",
        &db,
        "--limit",
        "10",
        "--json",
    ]);
    assert_eq!(
        listed["decoded_records"]["domains"]
            .as_array()
            .expect("listed domains")
            .len(),
        2
    );
    evidence.push(json!({"step": "list_domains", "output": listed}));

    // Edge case 1: empty TOML input fails before any row changes.
    let before_empty = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let empty_toml = temp.path().join("empty_domain_pack.toml");
    fs::write(&empty_toml, "").expect("write empty TOML edge input");
    let empty_toml_arg = empty_toml.display().to_string();
    let empty_err = run_err(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &db,
        "--file",
        &empty_toml_arg,
        "--json",
    ]);
    let after_empty = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(counts(&before_empty), counts(&after_empty));
    evidence.push(json!({
        "step": "edge_empty_toml",
        "before": before_empty,
        "error": empty_err,
        "after": after_empty
    }));

    // Edge case 2: duplicate instrument id fails validation and leaves counts unchanged.
    let before_duplicate = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let duplicate_text = fs::read_to_string(&counter)
        .expect("read counter TOML")
        .replacen("id = \"counter_world\"", "id = \"counter_world_dup\"", 1)
        .replacen("id = \"delta_scalar\"", "id = \"counter_scalar\"", 1);
    let duplicate_toml = temp.path().join("duplicate_instrument_domain_pack.toml");
    fs::write(&duplicate_toml, duplicate_text).expect("write duplicate instrument TOML");
    let duplicate_toml_arg = duplicate_toml.display().to_string();
    let duplicate_err = run_err(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &db,
        "--file",
        &duplicate_toml_arg,
        "--json",
    ]);
    assert_eq!(duplicate_err["error_code"], "VALIDATION");
    let after_duplicate = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(counts(&before_duplicate), counts(&after_duplicate));
    evidence.push(json!({
        "step": "edge_duplicate_instrument",
        "before": before_duplicate,
        "error": duplicate_err,
        "after": after_duplicate
    }));

    // Edge case 3: strict schema rejects unknown TOML keys and leaves counts unchanged.
    let before_unknown = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let unknown_text = fs::read_to_string(&counter)
        .expect("read counter TOML")
        .replacen("id = \"counter_world\"", "id = \"counter_world_unknown\"", 1)
        .replacen(
            "schema_version = 1",
            "schema_version = 1\nunknown_phase3_key = true",
            1,
        );
    let unknown_toml = temp.path().join("unknown_key_domain_pack.toml");
    fs::write(&unknown_toml, unknown_text).expect("write unknown-key TOML");
    let unknown_toml_arg = unknown_toml.display().to_string();
    let unknown_err = run_err(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &db,
        "--file",
        &unknown_toml_arg,
        "--json",
    ]);
    let after_unknown = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(counts(&before_unknown), counts(&after_unknown));
    evidence.push(json!({
        "step": "edge_unknown_toml_key",
        "before": before_unknown,
        "error": unknown_err,
        "after": after_unknown
    }));

    // Edge case 4: invalid CF name fails before decode and leaves DB state unchanged.
    let before_bad_cf = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let bad_cf_err = run_err(&[
        "dynamicjepa",
        "inspect-cf",
        "--db",
        &db,
        "--cf",
        "dj_not_real",
        "--limit",
        "1",
        "--json",
    ]);
    let after_bad_cf = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(counts(&before_bad_cf), counts(&after_bad_cf));
    evidence.push(json!({
        "step": "edge_invalid_cf",
        "before": before_bad_cf,
        "error": bad_cf_err,
        "after": after_bad_cf
    }));

    fs::write(
        evidence_dir.join("cli_fsv.json"),
        serde_json::to_vec_pretty(&json!({
            "source_of_truth": {
                "physical_db": db_path,
                "dynamicjepa_cfs": [
                    "dj_domain_packs",
                    "dj_domain_pack_by_nv",
                    "dj_instrument_registry",
                    "dj_adapter_registry",
                    "dj_audit_log"
                ],
                "read_paths": [
                    "dynamicjepa inspect-counts",
                    "dynamicjepa inspect-cf",
                    "dynamicjepa get-domain",
                    "dynamicjepa list-domains"
                ]
            },
            "steps": evidence
        }))
        .expect("serialize evidence"),
    )
    .expect("write Phase 3 evidence log");
}
