use super::errors::{CCRealityError, Result};
use super::helpers::*;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use regex::Regex;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

impl Handlers {
    pub(crate) async fn call_reality_query_ledger(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_query_ledger(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn reality_query_ledger(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let where_clause = required_str(&args, "where")?;
    let limit = optional_u64_strict(&args, "limit", 50)?.min(500);
    let where_clause = validate_ledger_where(&where_clause)?;
    let runtime_root = require_active_runtime_root().await?;
    let target = read_active_target_instance().await?;
    let ledger = find_ledger_for_run(&runtime_root, &run_id, target.as_deref())?;
    let conn =
        rusqlite::Connection::open_with_flags(&ledger, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| sqlite_err("CCREALITY_LEDGER_OPEN_FAILED", &ledger, e))?;
    let sql = format!(
        "SELECT sequence, record_kind, created_at, payload_json FROM ledger_records WHERE run_id = ?1 AND ({where_clause}) ORDER BY sequence DESC LIMIT ?2"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_PREPARE_FAILED", &ledger, e))?;
    let rows = stmt
        .query_map(rusqlite::params![&run_id, limit as i64], |row| {
            Ok(json!({
                "sequence": row.get::<_, i64>(0)?,
                "record_kind": row.get::<_, String>(1)?,
                "created_at": row.get::<_, String>(2)?,
                "payload_json": row.get::<_, String>(3)?
            }))
        })
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_QUERY_FAILED", &ledger, e))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_ROW_READ_FAILED", &ledger, e))?;
    let rows = rows
        .into_iter()
        .map(|mut row| {
            let payload_json =
                row.get("payload_json")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CCRealityError::new(
                            "CCREALITY_LEDGER_PAYLOAD_MISSING",
                            "ledger row did not contain payload_json",
                            "ledger.payload_json",
                            "inspect the ledger schema and row projection",
                            json!({"ledger": ledger.display().to_string(), "row": row}),
                            Some(format!("sqlite:{}#ledger_records", ledger.display())),
                        )
                    })?;
            let payload = serde_json::from_str::<Value>(payload_json).map_err(|e| {
                CCRealityError::new(
                    "CCREALITY_LEDGER_PAYLOAD_JSON_INVALID",
                    format!("ledger payload_json is not valid JSON: {e}"),
                    "ledger.payload_json",
                    "repair the ledger row or rebuild the attempt from source artifacts",
                    json!({
                        "ledger": ledger.display().to_string(),
                        "sequence": row.get("sequence").cloned().unwrap_or(Value::Null)
                    }),
                    Some(format!("sqlite:{}#ledger_records", ledger.display())),
                )
            })?;
            if let Value::Object(map) = &mut row {
                map.insert("payload".to_string(), payload);
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "rows": rows,
        "row_count": rows.len(),
        "run_id": run_id,
        "ledger_path": ledger.display().to_string(),
        "source_of_truth": format!("sqlite:{}#ledger_records", ledger.display())
    }))
}

fn validate_ledger_where(where_clause: &str) -> Result<String> {
    static WHERE_RE: OnceLock<Regex> = OnceLock::new();
    let re = WHERE_RE.get_or_init(|| {
        let atom = r#"(?:record_kind|created_at|payload_json)\s*(?:<=|>=|<>|!=|=|<|>|LIKE)\s*(?:'(?:[^']|'')*'|[0-9]+)"#;
        Regex::new(&format!(r#"(?i)^\s*{atom}(?:\s+AND\s+{atom})*\s*$"#))
            .expect("ledger WHERE validator regex compiles")
    });
    let trimmed = where_clause.trim();
    if trimmed.is_empty() || !re.is_match(trimmed) {
        return Err(CCRealityError::new(
            "CCREALITY_LEDGER_QUERY_REJECTED",
            "WHERE clause is outside the read-only ledger query contract",
            "reality_query_ledger.where",
            "use AND-joined comparisons on record_kind, created_at, or payload_json with quoted string or numeric literals",
            json!({"where": where_clause}),
            None,
        ));
    }
    Ok(trimmed.to_string())
}

pub(super) fn find_ledger_for_run(
    root: &Path,
    run_id: &str,
    target_instance: Option<&str>,
) -> Result<PathBuf> {
    let direct = root.join("ledger.sqlite");
    if direct.is_file() {
        return Ok(direct);
    }
    let run_dir = root.join(run_id);
    if !run_dir.is_dir() {
        return Err(CCRealityError::new(
            "CCREALITY_LEDGER_RUN_DIR_MISSING",
            "run_id directory is missing under active runtime root",
            "ledger.run_id",
            "verify run_id with reality_latest_root and rerun the engine if artifacts are missing",
            json!({"runtime_root": root.display().to_string(), "run_id": run_id, "path": run_dir.display().to_string()}),
            Some(file_sot(&run_dir)),
        ));
    }
    let mut found = Vec::new();
    collect_ledgers(&run_dir, &mut found)?;
    found.sort();
    let candidates = match target_instance {
        Some(target) => {
            let target_filtered = found
                .iter()
                .filter(|path| path_has_component(path, target))
                .cloned()
                .collect::<Vec<_>>();
            if target_filtered.is_empty() && !found.is_empty() {
                return Err(CCRealityError::new(
                    "CCREALITY_LEDGER_TARGET_MISMATCH",
                    "no ledger under the requested run_id matches the active target instance",
                    "ledger.target_instance",
                    "set the active target instance to the run's target or inspect the run directory before querying",
                    json!({
                        "runtime_root": root.display().to_string(),
                        "run_id": run_id,
                        "target_instance": target,
                        "candidates": found.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
                    }),
                    Some(file_sot(&run_dir)),
                ));
            }
            target_filtered
        }
        None => found,
    };
    match candidates.as_slice() {
        [ledger] => Ok(ledger.clone()),
        [] => Err(CCRealityError::new(
            "CCREALITY_LEDGER_MISSING",
            "no ledger.sqlite was found under requested run_id",
            "ledger.path",
            "run an attempt that initializes the ledger for this run_id",
            json!({"runtime_root": root.display().to_string(), "run_id": run_id, "target_instance": target_instance}),
            Some(file_sot(&run_dir)),
        )),
        many => Err(CCRealityError::new(
            "CCREALITY_LEDGER_AMBIGUOUS",
            "multiple ledgers match the requested run_id",
            "ledger.path",
            "narrow the active target instance or inspect the run directory before querying",
            json!({
                "runtime_root": root.display().to_string(),
                "run_id": run_id,
                "target_instance": target_instance,
                "candidates": many.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            }),
            Some(file_sot(&run_dir)),
        )),
    }
}

fn path_has_component(path: &Path, expected: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_str() == Some(expected))
}

fn collect_ledgers(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)
        .map_err(|e| fs_error("CCREALITY_LEDGER_WALK_READ_FAILED", root, e))?
    {
        let entry = entry.map_err(|e| fs_error("CCREALITY_LEDGER_WALK_ENTRY_FAILED", root, e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_ledgers(&path, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "ledger.sqlite")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn sqlite_err(code: &str, path: &Path, err: rusqlite::Error) -> CCRealityError {
    CCRealityError::new(
        code,
        format!("SQLite operation failed: {err}"),
        "ledger.sqlite",
        "inspect ledger schema and query shape",
        json!({"path": path.display().to_string(), "error": err.to_string()}),
        Some(format!("sqlite:{}#ledger_records", path.display())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_where_validator_accepts_allowlisted_comparisons() {
        assert!(validate_ledger_where("record_kind = 'agent_run'").is_ok());
        assert!(validate_ledger_where(
            "record_kind = 'tool_call' AND payload_json LIKE '%optimizer%'"
        )
        .is_ok());
        assert!(validate_ledger_where("created_at >= 1700000000").is_ok());
    }

    #[test]
    fn ledger_where_validator_rejects_sql_control_syntax() {
        for clause in [
            "1=1",
            "record_kind = 'agent_run' OR 1=1",
            "record_kind = 'agent_run'; DROP TABLE ledger_records",
            "record_kind IN ('agent_run')",
            "json_extract(payload_json, '$.status') = 'ok'",
            "record_kind = 'agent_run' --",
        ] {
            assert!(
                validate_ledger_where(clause).is_err(),
                "{clause} should be rejected"
            );
        }
    }

    #[test]
    fn ledger_locator_scopes_to_run_id_and_target_component() {
        let root = tempfile::tempdir().expect("runtime root");
        let run_a_ledger = root.path().join("run-a/tasks/target-a/model/ledger.sqlite");
        let run_b_ledger = root.path().join("run-b/tasks/target-b/model/ledger.sqlite");
        std::fs::create_dir_all(run_a_ledger.parent().expect("parent")).expect("run a parent");
        std::fs::create_dir_all(run_b_ledger.parent().expect("parent")).expect("run b parent");
        std::fs::write(&run_a_ledger, b"sqlite-a").expect("run a ledger");
        std::fs::write(&run_b_ledger, b"sqlite-b").expect("run b ledger");

        let resolved =
            find_ledger_for_run(root.path(), "run-b", Some("target-b")).expect("run b ledger");
        assert_eq!(resolved, run_b_ledger);
    }

    #[test]
    fn ledger_locator_rejects_ambiguous_run_ledgers() {
        let root = tempfile::tempdir().expect("runtime root");
        let ledger_a = root.path().join("run-a/tasks/target-a/ledger.sqlite");
        let ledger_b = root.path().join("run-a/tasks/target-b/ledger.sqlite");
        std::fs::create_dir_all(ledger_a.parent().expect("parent")).expect("target a parent");
        std::fs::create_dir_all(ledger_b.parent().expect("parent")).expect("target b parent");
        std::fs::write(&ledger_a, b"sqlite-a").expect("target a ledger");
        std::fs::write(&ledger_b, b"sqlite-b").expect("target b ledger");

        let err = find_ledger_for_run(root.path(), "run-a", None).expect_err("ambiguous");
        assert_eq!(err.error_code, "CCREALITY_LEDGER_AMBIGUOUS");

        let resolved =
            find_ledger_for_run(root.path(), "run-a", Some("target-a")).expect("target filter");
        assert_eq!(resolved, ledger_a);
    }

    #[test]
    fn ledger_locator_rejects_active_target_mismatch() {
        let root = tempfile::tempdir().expect("runtime root");
        let ledger = root.path().join("run-a/tasks/target-a/ledger.sqlite");
        std::fs::create_dir_all(ledger.parent().expect("parent")).expect("target parent");
        std::fs::write(&ledger, b"sqlite-a").expect("ledger");

        let err = find_ledger_for_run(root.path(), "run-a", Some("target-b"))
            .expect_err("target mismatch");
        assert_eq!(err.error_code, "CCREALITY_LEDGER_TARGET_MISMATCH");
    }
}
