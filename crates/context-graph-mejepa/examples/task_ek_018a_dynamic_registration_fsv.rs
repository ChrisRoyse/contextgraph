use anyhow::{ensure, Context, Result};
use context_graph_core::memory::ast::{EntityType, Language};
use context_graph_mejepa::{
    count_cf, current_unix_ms, open_infer_rocksdb, read_dynamic_embedder_provenance,
    read_dynamic_embedder_registry, read_dynamic_embedder_registry_snapshot,
    write_dynamic_embedder_provenance_sync_readback, write_dynamic_embedder_registry_sync_readback,
    write_json_0600, DynamicEmbedderKind, DynamicEmbedderProvenanceRecord,
    DynamicEmbedderRegistryRecord, RuntimeEmbedderId, RuntimeRoutingTable,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE, CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
};
use context_graph_mejepa_embedders::{dda_signal_count_for_chunks, upper_triangle_len, EmbedderId};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018a-dynamic-registration-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("rocksdb");
    let fsv_path = run_root.join("dynamic_registration_fsv.json");

    let old_snapshot = RuntimeRoutingTable::new(1, Vec::new())
        .context("construct old in-flight routing snapshot")?;
    let old_route = old_snapshot
        .route_for_entity_type(EntityType::Function, Language::Python)
        .context("route old snapshot")?;
    let old_snapshot_for_thread = old_snapshot.clone();
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let in_flight = thread::spawn(move || -> Result<bool, String> {
        ready_rx
            .recv()
            .map_err(|err| format!("in-flight route signal failed: {err}"))?;
        let route = old_snapshot_for_thread
            .route_for_entity_type(EntityType::Function, Language::Python)
            .map_err(|err| err.to_string())?;
        Ok(route.embedders.contains(
            &RuntimeEmbedderId::dynamic(1, "corpus_transe_v1").map_err(|err| err.to_string())?,
        ))
    });

    let db = open_infer_rocksdb(&db_path).context("open TASK-EK-018A FSV RocksDB")?;
    let record = registry_record(started_at_unix_ms)?;
    let provenance = provenance_record(record.id.clone(), started_at_unix_ms + 1)?;

    let before_registry_count = count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY)
        .context("count registry before")?;
    let before_provenance_count = count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
        .context("count provenance before")?;

    write_dynamic_embedder_registry_sync_readback(db.as_ref(), &record)
        .context("write/readback registry")?;
    write_dynamic_embedder_provenance_sync_readback(db.as_ref(), &provenance)
        .context("write/readback provenance")?;

    let direct_registry = read_dynamic_embedder_registry(db.as_ref(), &record.id)
        .context("direct registry read")?
        .context("missing direct registry row")?;
    let direct_provenance =
        read_dynamic_embedder_provenance(db.as_ref(), &record.id, record.registry_version)
            .context("direct provenance read")?
            .context("missing direct provenance row")?;
    ensure!(
        direct_registry == record,
        "direct registry readback mismatch"
    );
    ensure!(
        direct_provenance == provenance,
        "direct provenance readback mismatch"
    );

    let live_snapshot =
        read_dynamic_embedder_registry_snapshot(db.as_ref()).context("read live snapshot")?;
    ready_tx
        .send(())
        .context("signal in-flight old-snapshot route")?;
    let in_flight_old_snapshot_saw_dynamic = in_flight
        .join()
        .map_err(|_| anyhow::anyhow!("in-flight route thread panicked"))?
        .map_err(anyhow::Error::msg)?;
    let live_route = live_snapshot
        .route_for_entity_type(EntityType::Function, Language::Python)
        .context("route live snapshot")?;
    let static_count = EmbedderId::content().len();
    let before_pairwise = upper_triangle_len(static_count).context("before pairwise len")?;
    let after_pairwise = live_snapshot
        .pairwise_mi_upper_triangle_len(static_count)
        .context("after pairwise len")?;
    let dda_identity =
        dda_signal_count_for_chunks(10, live_snapshot.active_embedder_count(static_count), 1)
            .context("DDA identity")?;
    let required_vram_bytes = live_snapshot
        .required_vram_bytes(18 * 1024 * 1024 * 1024)
        .context("dynamic VRAM accounting")?;

    drop(db);
    let reopened = open_infer_rocksdb(&db_path).context("reopen TASK-EK-018A FSV RocksDB")?;
    let reopened_registry = read_dynamic_embedder_registry(reopened.as_ref(), &record.id)
        .context("reopened registry read")?
        .context("missing reopened registry row")?;
    let reopened_provenance =
        read_dynamic_embedder_provenance(reopened.as_ref(), &record.id, record.registry_version)
            .context("reopened provenance read")?
            .context("missing reopened provenance row")?;
    let reopened_snapshot =
        read_dynamic_embedder_registry_snapshot(reopened.as_ref()).context("reopened snapshot")?;
    let registry_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY)
        .context("count registry after")?;
    let provenance_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
        .context("count provenance after")?;
    let raw_registry_sha = raw_cf_value_sha256(
        reopened.as_ref(),
        CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
        record.id.slug().as_bytes(),
    )
    .context("raw registry sha")?;

    let duplicate_rejected =
        RuntimeRoutingTable::new(2, vec![record.clone(), record.clone()]).is_err();
    let invalid_id_rejected = RuntimeEmbedderId::dynamic(0, "bad").is_err();
    let old_snapshot_unchanged = !old_route.embedders.contains(&record.id);
    let live_route_contains_dynamic = live_route.embedders.contains(&record.id);
    let reopened_equal = reopened_registry == record && reopened_provenance == provenance;

    let all_passed = before_registry_count == 0
        && before_provenance_count == 0
        && registry_count_after == 1
        && provenance_count_after == 1
        && old_snapshot_unchanged
        && !in_flight_old_snapshot_saw_dynamic
        && live_route_contains_dynamic
        && reopened_equal
        && reopened_snapshot.active_dynamic_count() == 1
        && before_pairwise == 66
        && after_pairwise == 78
        && dda_identity == 920
        && required_vram_bytes == 18 * 1024 * 1024 * 1024 + 64 * 1024 * 1024
        && duplicate_rejected
        && invalid_id_rejected;

    let evidence = json!({
        "task": "TASK-EK-018A",
        "issue": 342,
        "all_passed": all_passed,
        "db_path": db_path,
        "sot": {
            "registry_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
            "provenance_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
            "fsv_path": fsv_path,
        },
        "registry": {
            "before_count": before_registry_count,
            "after_reopen_count": registry_count_after,
            "raw_registry_value_sha256": raw_registry_sha,
            "reopened_equal": reopened_registry == record,
            "record": record,
        },
        "provenance": {
            "before_count": before_provenance_count,
            "after_reopen_count": provenance_count_after,
            "reopened_equal": reopened_provenance == provenance,
            "record": provenance,
        },
        "routing": {
            "old_snapshot_version": old_snapshot.registry_version,
            "live_snapshot_version": live_snapshot.registry_version,
            "reopened_snapshot_version": reopened_snapshot.registry_version,
            "old_snapshot_unchanged": old_snapshot_unchanged,
            "in_flight_old_snapshot_saw_dynamic": in_flight_old_snapshot_saw_dynamic,
            "live_route_contains_dynamic": live_route_contains_dynamic,
            "live_route_embedder_count": live_route.embedders.len(),
        },
        "cardinality": {
            "static_content_embedder_count": static_count,
            "active_dynamic_count": live_snapshot.active_dynamic_count(),
            "before_pairwise_upper_triangle": before_pairwise,
            "after_pairwise_upper_triangle": after_pairwise,
            "dda_identity_10_chunks_foundationality_1": dda_identity,
        },
        "vram": {
            "base_required_bytes": 18 * 1024 * 1024 * 1024u64,
            "dynamic_required_bytes": 64 * 1024 * 1024u64,
            "total_required_bytes": required_vram_bytes,
        },
        "edges": {
            "duplicate_rejected": duplicate_rejected,
            "invalid_id_rejected": invalid_id_rejected,
        }
    });

    write_json_0600(&fsv_path, &evidence).context("write FSV JSON")?;
    let bytes = fs::read(&fsv_path).context("read FSV JSON")?;
    let fsv_sha256 = sha256_hex(&bytes);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "fsv_sha256": fsv_sha256,
            "registry_count_after": registry_count_after,
            "provenance_count_after": provenance_count_after,
            "live_route_contains_dynamic": live_route_contains_dynamic,
            "before_pairwise": before_pairwise,
            "after_pairwise": after_pairwise,
            "required_vram_bytes": required_vram_bytes,
        }))?
    );
    ensure!(all_passed, "TASK-EK-018A dynamic-registration FSV failed");
    Ok(())
}

fn registry_record(created_at: i64) -> Result<DynamicEmbedderRegistryRecord> {
    let record = DynamicEmbedderRegistryRecord {
        id: RuntimeEmbedderId::dynamic(1, "corpus_transe_v1")?,
        registry_version: 2,
        kind: DynamicEmbedderKind::Algorithmic,
        dimension: 128,
        route_languages: vec!["python".to_string()],
        route_entity_types: vec!["Function".to_string()],
        forward_artifact_path:
            "/mnt/d/contextgraph/models/dynamic/edynamic_1_corpus_transe_v1/forward.so".to_string(),
        forward_artifact_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_string(),
        required_vram_bytes: 64 * 1024 * 1024,
        active: true,
        promoted_at_unix_ms: created_at,
    };
    record.validate()?;
    Ok(record)
}

fn provenance_record(
    id: RuntimeEmbedderId,
    created_at: i64,
) -> Result<DynamicEmbedderProvenanceRecord> {
    let record = DynamicEmbedderProvenanceRecord {
        id,
        registry_version: 2,
        residual_signal_ref: "unknown_cluster:fixture-cluster-a".to_string(),
        architecture_generator: "task-ek-018a-fsv-deterministic-fixture".to_string(),
        training_cert_chain_hash:
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        heldout_global_delta: 0.006,
        heldout_min_cell_delta: 0.0,
        operator_approval_id: Some("first-promotion-approval-fixture".to_string()),
        forward_pass_artifact_sha256:
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        created_at_unix_ms: created_at,
    };
    record.validate()?;
    Ok(record)
}

fn raw_cf_value_sha256(db: &rocksdb::DB, cf_name: &str, key: &[u8]) -> Result<String> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    let value = db
        .get_cf(cf, key)
        .with_context(|| format!("read CF {cf_name}"))?
        .with_context(|| format!("missing key in CF {cf_name}"))?;
    Ok(sha256_hex(&value))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
