use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::{
    all_dynamic_embedder_promotions, pending_dynamic_embedder_promotions, HealRocksStore,
};
use context_graph_mejepa::{
    adversarial_embedder_injection_cases, count_cf, evaluate_adversarial_embedder_injection,
    open_infer_rocksdb, read_embedder_proposal_rejections, write_json_0600,
    EmbedderFalsificationGate,
};
use context_graph_mejepa_cf::{CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS, CF_MEJEPA_MODEL_PROMOTIONS};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018j-adversarial-injection-fsv";
const THREAT_MODEL_DOC: &str = "docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md";

fn main() -> Result<()> {
    let started_at_unix_ms = context_graph_mejepa::current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let infer_db_path = run_root.join("infer-rocksdb");
    let heal_db_path = run_root.join("heal-rocksdb");
    let fsv_path = run_root.join("adversarial_injection_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018J FSV run dir")?;

    let infer_db = open_infer_rocksdb(&infer_db_path).context("open TASK-EK-018J infer DB")?;
    let heal_store = HealRocksStore::open(&heal_db_path).context("open TASK-EK-018J heal DB")?;
    let cases = adversarial_embedder_injection_cases(started_at_unix_ms)
        .context("build adversarial injection cases")?;
    let report = evaluate_adversarial_embedder_injection(
        infer_db.as_ref(),
        heal_store.as_ref(),
        &cases,
        EmbedderFalsificationGate::default(),
    )
    .context("evaluate adversarial injection cases")?;

    let rejection_rows =
        read_embedder_proposal_rejections(infer_db.as_ref()).context("rejection rows")?;
    let pending_promotions =
        pending_dynamic_embedder_promotions(heal_store.as_ref()).context("pending dynamic rows")?;
    let all_promotions =
        all_dynamic_embedder_promotions(heal_store.as_ref()).context("all dynamic rows")?;

    drop(infer_db);
    drop(heal_store);

    let reopened_infer =
        open_infer_rocksdb(&infer_db_path).context("reopen TASK-EK-018J infer DB")?;
    let reopened_heal =
        HealRocksStore::open(&heal_db_path).context("reopen TASK-EK-018J heal DB")?;
    let reopened_rejection_count = count_cf(
        reopened_infer.as_ref(),
        CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
    )
    .context("reopened rejection count")?;
    let reopened_promotions = all_dynamic_embedder_promotions(reopened_heal.as_ref())
        .context("reopened dynamic promotions")?;
    let docs_lint = threat_model_doc_lint(Path::new(THREAT_MODEL_DOC))
        .context("TASK-EK-018J threat model doc lint")?;
    let reopened_readback_equal = reopened_rejection_count == report.rejection_rows_after
        && reopened_promotions == all_promotions;

    let all_passed = report.all_passed
        && reopened_readback_equal
        && docs_lint
        && rejection_rows.len() == 3
        && pending_promotions.len() == 1
        && all_promotions.len() == 1;

    let evidence = json!({
        "task": "TASK-EK-018J",
        "issue": 351,
        "all_passed": all_passed,
        "sot": {
            "rejection_cf": CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
            "approval_cf": CF_MEJEPA_MODEL_PROMOTIONS,
            "infer_db_path": infer_db_path,
            "heal_db_path": heal_db_path,
            "fsv_path": fsv_path,
            "threat_model_doc": THREAT_MODEL_DOC,
            "inner_llm_invoked": false
        },
        "report": report,
        "readback": {
            "rejection_rows": rejection_rows,
            "pending_dynamic_promotions": pending_promotions,
            "all_dynamic_promotions": all_promotions,
            "reopened_rejection_count": reopened_rejection_count,
            "reopened_promotions": reopened_promotions,
            "reopened_readback_equal": reopened_readback_equal
        },
        "docs_lint": {
            "path": THREAT_MODEL_DOC,
            "task_ek_018j_threat_model_present": docs_lint
        }
    });

    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018J FSV JSON")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018J FSV JSON")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "fsv_sha256": fsv_sha256,
            "falsification_rejected_count": evidence["report"]["falsificationRejectedCount"],
            "operator_gate_caught_count": evidence["report"]["operatorGateCaughtCount"],
            "unexpected_survivor_count": evidence["report"]["unexpectedSurvivorCount"],
            "reopened_readback_equal": reopened_readback_equal,
            "docs_lint": docs_lint
        }))?
    );
    ensure!(all_passed, "TASK-EK-018J adversarial injection FSV failed");
    Ok(())
}

fn threat_model_doc_lint(path: &Path) -> Result<bool> {
    let body = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(body.contains("Adversarial-embedder-injection threat model")
        && body.contains("TASK-EK-018J")
        && body.contains("Synthetic OOD chunks")
        && body.contains("Falsification gate")
        && body.contains("Operator-in-the-loop gate"))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}
