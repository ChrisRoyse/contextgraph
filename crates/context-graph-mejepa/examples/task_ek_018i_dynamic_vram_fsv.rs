use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::{
    gate_dynamic_embedder_promotion, DynamicEmbedderPromotionGateRequest, HealRocksStore,
};
use context_graph_mejepa::{
    check_dynamic_embedder_promotion_vram_budget, current_unix_ms,
    deactivate_dynamic_embedder_registry_sync_readback, estimate_dynamic_embedder_artifact_vram,
    open_infer_rocksdb, plan_dynamic_embedder_evictions, read_dynamic_embedder_registry_snapshot,
    write_dynamic_embedder_registry_sync_readback, write_json_0600, DynamicEmbedderKind,
    DynamicEmbedderRegistryRecord, DynamicEmbedderUtilityScore, DynamicEmbedderVramBudget,
    RuntimeEmbedderId, RuntimeRoutingTable, MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED,
};
use context_graph_mejepa_cf::CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY;
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018i-dynamic-vram-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let infer_db_path = run_root.join("infer-rocksdb");
    let heal_db_path = run_root.join("heal-rocksdb");
    let fsv_path = run_root.join("dynamic_vram_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018I FSV run dir")?;

    let fit_artifact = artifact_path(started_at_unix_ms, 10, "fit_candidate");
    let exceed_artifact = artifact_path(started_at_unix_ms, 11, "exceed_candidate");
    write_bytes(&fit_artifact, 128).context("write fit artifact")?;
    write_bytes(&exceed_artifact, 512).context("write exceed artifact")?;
    let fit_id = RuntimeEmbedderId::dynamic(10, "fit_candidate")?;
    let exceed_id = RuntimeEmbedderId::dynamic(11, "exceed_candidate")?;
    let fit_estimate = estimate_dynamic_embedder_artifact_vram(
        fit_id.clone(),
        DynamicEmbedderKind::LearnedHead,
        &fit_artifact,
    )
    .context("estimate fit candidate VRAM")?;
    let exceed_estimate = estimate_dynamic_embedder_artifact_vram(
        exceed_id.clone(),
        DynamicEmbedderKind::LearnedHead,
        &exceed_artifact,
    )
    .context("estimate exceed candidate VRAM")?;

    let budget = DynamicEmbedderVramBudget {
        static_required_bytes: 512,
        budget_bytes: 1024,
    };
    let table = RuntimeRoutingTable::new(2, vec![registry_record(1, "existing", 256, true)?])
        .context("construct promotion budget table")?;
    let fit_budget = check_dynamic_embedder_promotion_vram_budget(
        &table,
        budget,
        fit_estimate.required_vram_bytes,
    )
    .context("fit promotion budget")?;
    let exceed_budget = check_dynamic_embedder_promotion_vram_budget(
        &table,
        budget,
        exceed_estimate.required_vram_bytes,
    )
    .context("exceed promotion budget")?;

    let heal_store = HealRocksStore::open(&heal_db_path).context("open heal store")?;
    let fit_gate = gate_dynamic_embedder_promotion(
        heal_store.as_ref(),
        gate_request(&fit_id, fit_budget.accepted, "fit"),
    )
    .context("gate fit promotion")?;
    let exceed_gate = gate_dynamic_embedder_promotion(
        heal_store.as_ref(),
        gate_request(&exceed_id, exceed_budget.accepted, "exceed"),
    )
    .context("gate exceed promotion")?;

    let db = open_infer_rocksdb(&infer_db_path).context("open infer DB")?;
    for record in [
        registry_record(1, "low_lru", 400, true)?,
        registry_record(2, "low_contrib", 300, true)?,
        registry_record(3, "high_value", 200, true)?,
    ] {
        write_dynamic_embedder_registry_sync_readback(db.as_ref(), &record)
            .context("write eviction registry record")?;
    }
    let eviction_before =
        read_dynamic_embedder_registry_snapshot(db.as_ref()).context("snapshot before eviction")?;
    let eviction_budget = DynamicEmbedderVramBudget {
        static_required_bytes: 500,
        budget_bytes: 850,
    };
    let eviction_plan = plan_dynamic_embedder_evictions(
        &eviction_before,
        eviction_budget,
        &[
            utility(1, "low_lru", 50, 0.02)?,
            utility(2, "low_contrib", 100, 0.01)?,
            utility(3, "high_value", 200, 0.50)?,
        ],
    )
    .context("plan dynamic embedder evictions")?;
    let mut deactivated = Vec::new();
    for id in &eviction_plan.evicted_ids {
        deactivated.push(
            deactivate_dynamic_embedder_registry_sync_readback(db.as_ref(), id, 3)
                .context("deactivate dynamic embedder")?,
        );
    }
    let eviction_after =
        read_dynamic_embedder_registry_snapshot(db.as_ref()).context("snapshot after eviction")?;
    let eviction_after_required = eviction_after
        .required_vram_bytes(eviction_budget.static_required_bytes)
        .context("eviction after required VRAM")?;
    drop(db);
    let reopened = open_infer_rocksdb(&infer_db_path).context("reopen infer DB")?;
    let reopened_snapshot =
        read_dynamic_embedder_registry_snapshot(reopened.as_ref()).context("reopened snapshot")?;
    let reopened_required = reopened_snapshot
        .required_vram_bytes(eviction_budget.static_required_bytes)
        .context("reopened required VRAM")?;

    let fit_accepted = fit_budget.accepted
        && fit_budget.projected_required_bytes == 896
        && fit_gate.may_promote_now
        && !fit_gate.approval_required
        && fit_gate.block_reasons.is_empty();
    let exceed_rejected = !exceed_budget.accepted
        && exceed_budget.reason_code.as_deref()
            == Some(MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED)
        && !exceed_gate.may_promote_now
        && !exceed_gate.approval_required
        && exceed_gate.block_reasons
            == vec![MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED.to_string()];
    let eviction_frees_budget = eviction_plan.eviction_required
        && eviction_plan.budget_satisfied
        && eviction_plan.before_required_bytes == 1400
        && eviction_plan.after_required_bytes == 700
        && eviction_plan.evicted_ids
            == vec![
                RuntimeEmbedderId::dynamic(2, "low_contrib")?,
                RuntimeEmbedderId::dynamic(1, "low_lru")?,
            ]
        && deactivated.iter().all(|record| !record.active)
        && eviction_after.active_dynamic_count() == 1
        && eviction_after_required == 700
        && reopened_snapshot.active_dynamic_count() == 1
        && reopened_required == 700;
    let all_passed = fit_accepted && exceed_rejected && eviction_frees_budget;

    let evidence = json!({
        "task": "TASK-EK-018I",
        "issue": 353,
        "all_passed": all_passed,
        "sot": {
            "registry_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
            "infer_db_path": infer_db_path,
            "heal_db_path": heal_db_path,
            "fsv_path": fsv_path,
            "inner_llm_invoked": false
        },
        "artifact_estimates": {
            "fit": fit_estimate,
            "exceed": exceed_estimate
        },
        "promotion_budget": {
            "budget": budget,
            "fit_decision": fit_budget,
            "exceed_decision": exceed_budget,
            "fit_gate": fit_gate,
            "exceed_gate": exceed_gate,
            "fit_accepted": fit_accepted,
            "exceed_rejected": exceed_rejected
        },
        "eviction": {
            "budget": eviction_budget,
            "before_active_dynamic_count": eviction_before.active_dynamic_count(),
            "plan": eviction_plan,
            "deactivated_records": deactivated,
            "after_active_dynamic_count": eviction_after.active_dynamic_count(),
            "after_required_bytes": eviction_after_required,
            "reopened_active_dynamic_count": reopened_snapshot.active_dynamic_count(),
            "reopened_required_bytes": reopened_required,
            "frees_budget": eviction_frees_budget
        }
    });
    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018I FSV JSON")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018I FSV JSON")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "fsv_sha256": fsv_sha256,
            "fit_accepted": fit_accepted,
            "exceed_rejected": exceed_rejected,
            "eviction_frees_budget": eviction_frees_budget,
            "eviction_after_required": eviction_after_required,
            "reopened_required": reopened_required,
        }))?
    );
    ensure!(all_passed, "TASK-EK-018I dynamic VRAM FSV failed");
    Ok(())
}

fn gate_request(
    candidate_id: &RuntimeEmbedderId,
    within_vram_budget: bool,
    suffix: &str,
) -> DynamicEmbedderPromotionGateRequest {
    DynamicEmbedderPromotionGateRequest {
        candidate_id_slug: candidate_id.slug().into_owned(),
        candidate_name: format!("candidate_{suffix}"),
        proposal_id_hex: sha256_hex(candidate_id.slug().as_bytes())[..32].to_string(),
        falsification_passed: true,
        redundant_with_existing: false,
        within_vram_budget,
        first_dynamic_promotion: false,
        adversarial_injection_risk: false,
        would_cross_ship_gate: false,
        modifies_below_threshold_cell: false,
        heldout_global_delta: 0.006,
        min_cell_delta: 0.0,
        reason: format!("TASK-EK-018I dynamic VRAM gate {suffix}"),
    }
}

fn registry_record(
    sequence: u32,
    name: &str,
    required_vram_bytes: u64,
    active: bool,
) -> Result<DynamicEmbedderRegistryRecord> {
    let record = DynamicEmbedderRegistryRecord {
        id: RuntimeEmbedderId::dynamic(sequence, name)?,
        registry_version: 2,
        kind: DynamicEmbedderKind::LearnedHead,
        dimension: 128,
        route_languages: vec!["python".to_string()],
        route_entity_types: vec!["*".to_string()],
        forward_artifact_path: format!(
            "/mnt/d/contextgraph/models/dynamic/edynamic_{sequence}_{name}/forward.bin"
        ),
        forward_artifact_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_string(),
        required_vram_bytes,
        active,
        promoted_at_unix_ms: 1_779_100_000_000 + i64::from(sequence),
    };
    record.validate()?;
    Ok(record)
}

fn utility(
    sequence: u32,
    name: &str,
    last_used_unix_ms: i64,
    mean_per_cell_contribution: f32,
) -> Result<DynamicEmbedderUtilityScore> {
    let utility = DynamicEmbedderUtilityScore {
        id: RuntimeEmbedderId::dynamic(sequence, name)?,
        last_used_unix_ms,
        mean_per_cell_contribution,
    };
    utility.validate()?;
    Ok(utility)
}

fn artifact_path(started_at_unix_ms: i64, sequence: u32, name: &str) -> PathBuf {
    PathBuf::from(format!(
        "/mnt/d/contextgraph/models/dynamic/edynamic_{sequence}_{name}/vram-fsv-{started_at_unix_ms}/forward.bin"
    ))
}

fn write_bytes(path: &Path, len: usize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create artifact parent {}", parent.display()))?;
    }
    let bytes = (0..len).map(|idx| (idx % 251) as u8).collect::<Vec<_>>();
    fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    let readback = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    ensure!(readback == bytes, "{} readback mismatch", path.display());
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
