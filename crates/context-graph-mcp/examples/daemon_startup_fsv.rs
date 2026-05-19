use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use context_graph_mcp::{daemon, daemon_validate};
use serde_json::json;

fn main() -> Result<()> {
    let data_root = context_graph_paths::data_root()
        .context("MEJEPA_D_DRIVE_UNAVAILABLE: failed to resolve CONTEXTGRAPH_DATA_ROOT")?;
    let paths = daemon_validate::validate_daemon_root(&data_root)?;
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .context("MEJEPA_DAEMON_FSV_REPO_ROOT_INVALID")?
        .to_path_buf();
    let systemd_service = repo_root.join("scripts/systemd/mejepa-daemon.service");
    if !systemd_service.is_file() {
        return Err(anyhow::anyhow!(
            "MEJEPA_DAEMON_SYSTEMD_TEMPLATE_MISSING: {}",
            systemd_service.display()
        ));
    }

    let evidence_path = paths.fsv_dir.join("startup.json");
    let evidence = json!({
        "source_of_truth": evidence_path,
        "pid": std::process::id(),
        "validated_aiwonder_paths": paths,
        "supervised_task_names": daemon::supervised_task_names(),
        "systemd_service": systemd_service,
        "validated_at_unix_seconds": chrono::Utc::now().timestamp(),
    });
    let expected = serde_json::to_vec_pretty(&evidence)?;
    write_sync_readback(&evidence_path, &expected)?;
    let readback = fs::read(&evidence_path)?;
    if readback != expected {
        return Err(anyhow::anyhow!(
            "MEJEPA_DAEMON_FSV_READBACK_MISMATCH: {}",
            evidence_path.display()
        ));
    }

    println!("{}", String::from_utf8(readback)?);
    Ok(())
}

fn write_sync_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
