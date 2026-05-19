use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

fn main() -> Result<(), Box<dyn Error>> {
    let fsv_root = env::var("CG_PRODUCTION_ROOT_GUARD_FSV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                "/zfs/archive/contextgraph/fsv/task-aiwonder-014-production-root-guard-fsv",
            )
        });
    let fsv_root = context_graph_paths::require_production_durable_root(&fsv_root, "fsv_root")?;
    fs::create_dir_all(&fsv_root)?;

    let started_at = unix_ms();
    let prior_data_root = env::var(context_graph_paths::ENV_DATA_ROOT).ok();
    let prior_local_smoke = env::var(context_graph_paths::ENV_LOCAL_SMOKE_ISSUE).ok();

    env::remove_var(context_graph_paths::ENV_LOCAL_SMOKE_ISSUE);
    let reject_local = expect_error_code(
        context_graph_paths::require_production_durable_root_or_named_local_smoke(
            Path::new("/mnt/d/contextgraph/fsv/task-aiwonder-014"),
            "durable_root",
        ),
        "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER",
    );

    let accept_archive = context_graph_paths::require_production_durable_root(
        Path::new("/zfs/archive/contextgraph/fsv/task-aiwonder-014"),
        "durable_root",
    )?;
    let accept_hot = context_graph_paths::require_production_hot_root(
        Path::new("/zfs/hot/contextgraph/runtime/task-aiwonder-014"),
        "hot_root",
    )?;
    let accept_scratch = context_graph_paths::require_production_hot_or_explicit_scratch_root(
        Path::new("/home/croyse/.cache/contextgraph/task-aiwonder-014"),
        "hot_root",
    )?;
    let reject_local_scratch = expect_error_code(
        context_graph_paths::require_production_hot_or_explicit_scratch_root(
            Path::new("/home/cabdru/.cache/contextgraph/task-aiwonder-014"),
            "hot_root",
        ),
        "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER",
    );

    env::set_var(context_graph_paths::ENV_DATA_ROOT, "/mnt/d/contextgraph");
    env::remove_var(context_graph_paths::ENV_LOCAL_SMOKE_ISSUE);
    let env_reject_local = expect_error_code(
        context_graph_paths::production_data_root(),
        "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER",
    );

    env::set_var(
        context_graph_paths::ENV_DATA_ROOT,
        context_graph_paths::AIWONDER_DURABLE_ROOT,
    );
    let env_accept_archive = context_graph_paths::production_data_root()?;

    env::set_var(
        context_graph_paths::ENV_LOCAL_SMOKE_ISSUE,
        "issue-365-local-smoke-fsv",
    );
    let local_smoke_reject = expect_error_code(
        context_graph_paths::require_production_durable_root_or_named_local_smoke(
            Path::new("/mnt/d/contextgraph/fsv/task-aiwonder-014-local-smoke"),
            "durable_root",
        ),
        "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER",
    );

    restore_env(context_graph_paths::ENV_DATA_ROOT, prior_data_root);
    restore_env(
        context_graph_paths::ENV_LOCAL_SMOKE_ISSUE,
        prior_local_smoke,
    );

    let cases = vec![
        json!({"case": "reject_local_durable_root", "passes": reject_local}),
        json!({"case": "accept_aiwonder_archive_root", "passes": accept_archive.starts_with(context_graph_paths::AIWONDER_DURABLE_ROOT), "path": accept_archive}),
        json!({"case": "accept_aiwonder_hot_root", "passes": accept_hot.starts_with(context_graph_paths::AIWONDER_HOT_ROOT), "path": accept_hot}),
        json!({"case": "accept_aiwonder_explicit_scratch_root", "passes": accept_scratch.starts_with(context_graph_paths::AIWONDER_EXPLICIT_SCRATCH_ROOT), "path": accept_scratch}),
        json!({"case": "reject_local_scratch_root", "passes": reject_local_scratch}),
        json!({"case": "env_var_rejects_local_durable_root", "passes": env_reject_local}),
        json!({"case": "env_var_accepts_aiwonder_archive_root", "passes": env_accept_archive.starts_with(context_graph_paths::AIWONDER_DURABLE_ROOT), "path": env_accept_archive}),
        json!({"case": "named_local_smoke_still_rejects_legacy_root", "passes": local_smoke_reject}),
    ];
    let passes = cases
        .iter()
        .all(|case| case.get("passes").and_then(|v| v.as_bool()) == Some(true));
    let evidence = json!({
        "schema_version": 1,
        "task_id": "TASK-AIWONDER-014",
        "artifact_kind": "production_root_guard_fsv",
        "started_at_unix_ms": started_at,
        "created_at_unix_ms": unix_ms(),
        "source_of_truth": {
            "fsv_root": fsv_root,
            "durable_root": context_graph_paths::AIWONDER_DURABLE_ROOT,
            "hot_root": context_graph_paths::AIWONDER_HOT_ROOT,
            "explicit_scratch_root": context_graph_paths::AIWONDER_EXPLICIT_SCRATCH_ROOT,
            "legacy_local_root": context_graph_paths::LEGACY_LOCAL_DATA_ROOT
        },
        "cases": cases,
        "passes": passes
    });
    let fsv_path = fsv_root.join("production_root_guard_fsv.json");
    write_json_readback(&fsv_path, &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    if !passes {
        return Err("production root guard FSV did not pass".into());
    }
    Ok(())
}

fn expect_error_code<T>(result: context_graph_paths::Result<T>, expected: &'static str) -> bool {
    matches!(result, Err(err) if err.code == expected)
}

fn write_json_readback(path: &Path, value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let data = serde_json::to_vec_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &data)?;
    if fs::read(&tmp)? != data {
        return Err(format!("temp readback mismatch for {}", tmp.display()).into());
    }
    fs::rename(&tmp, path)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    if &readback != value {
        return Err(format!("final readback mismatch for {}", path.display()).into());
    }
    Ok(())
}

fn restore_env(name: &'static str, prior: Option<String>) {
    if let Some(value) = prior {
        env::set_var(name, value);
    } else {
        env::remove_var(name);
    }
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_millis()
}
