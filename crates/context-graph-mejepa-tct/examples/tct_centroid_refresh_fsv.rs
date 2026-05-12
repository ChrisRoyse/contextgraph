use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use context_graph_mejepa_instruments::{hash_f32s, InstrumentSlot, Panel, PanelBuilder};
use context_graph_mejepa_tct::*;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

type FsvResult<T> = Result<T, Box<dyn Error>>;

const OUT: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-refresh-fsv";
const PANEL_PATH: &str =
    "/mnt/d/contextgraph/fsv/contextgraph-mejepa-instruments-fsv/phase1-extended/panel-all-slots.json";
const CORPUS_INDEX_PATH: &str =
    "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const SAMPLES_PER_CELL: usize = 5;

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    complete: bool,
    corpus_sha256: String,
    entries: Vec<CorpusEntry>,
}

#[derive(Debug, Deserialize)]
struct CorpusEntry {
    category: String,
    oracle_all_passed: bool,
    patch_path: String,
    patch_sha256: String,
    repo: String,
    task_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshFsvEvidence {
    all_passed: bool,
    source_of_truth: serde_json::Value,
    happy_path: serde_json::Value,
    guard_decision_reward_signals: serde_json::Value,
    edge_cases: Vec<serde_json::Value>,
    physical_proof: serde_json::Value,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from(OUT);
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let panel_path = PathBuf::from(PANEL_PATH);
    let real_panel = read_real_phase1_panel(&panel_path)?;
    let corpus_path = PathBuf::from(CORPUS_INDEX_PATH);
    let corpus = read_corpus_index(&corpus_path)?;
    let corpus_sha = hex_digest_to_32(&corpus.corpus_sha256)?;
    let code_version = git_head()?;
    let versions = embedder_versions_from_panel(&real_panel);

    let mut entries_by_category: BTreeMap<MutationCategory, Vec<&CorpusEntry>> = BTreeMap::new();
    for entry in &corpus.entries {
        let mutation = parse_mutation_category(&entry.category)?;
        entries_by_category.entry(mutation).or_default().push(entry);
    }
    for mutation in MutationCategory::all() {
        let count = entries_by_category
            .get(&mutation)
            .map(Vec::len)
            .unwrap_or_default();
        require(
            &format!("real corpus has rows for {mutation:?}"),
            count >= SAMPLES_PER_CELL,
        )?;
    }

    let mut builder =
        ConstellationBuilder::new(corpus_sha, versions.clone(), code_version.clone())?;
    let mut entity_counts: BTreeMap<EntityType, usize> = BTreeMap::new();
    let mut observed_support: BTreeMap<
        (MutationCategory, EntityType, Language, EmbedderId),
        usize,
    > = BTreeMap::new();
    let mut ingested_panel_count = 0usize;
    let mut first_good_function: Option<(ChunkId, Panel)> = None;
    let mut first_good_entry = json!(null);

    for (mutation_idx, mutation) in MutationCategory::all().into_iter().enumerate() {
        let entries = entries_by_category
            .get(&mutation)
            .ok_or_else(|| format!("missing corpus rows for {mutation:?}"))?;
        for (entity_idx, entity_type) in EntityType::all().into_iter().enumerate() {
            for repeat in 0..SAMPLES_PER_CELL {
                let entry = entries[(entity_idx * SAMPLES_PER_CELL + repeat) % entries.len()];
                let panel = derive_panel(&real_panel, mutation_idx, entity_idx, repeat, entry)?;
                let chunk = chunk_id_from_entry(entry, entity_type, repeat)?;
                let outcome = if entry.oracle_all_passed {
                    OracleOutcome::Pass
                } else {
                    OracleOutcome::Fail
                };
                builder.ingest_corpus_entry(
                    &panel,
                    mutation,
                    outcome,
                    Language::Python,
                    entity_type,
                    &[(chunk.clone(), panel.clone())],
                )?;
                *entity_counts.entry(entity_type).or_default() += 1;
                for embedder in EmbedderId::all() {
                    *observed_support
                        .entry((mutation, entity_type, Language::Python, embedder))
                        .or_default() += 1;
                }
                ingested_panel_count += 1;
                if mutation == MutationCategory::KnownGood
                    && entity_type == EntityType::Function
                    && repeat == 0
                {
                    first_good_function = Some((chunk, panel));
                    first_good_entry = json!({
                        "repo": entry.repo,
                        "task_id": entry.task_id,
                        "patch_path": entry.patch_path,
                        "patch_sha256": entry.patch_sha256,
                    });
                }
            }
        }
    }

    require(
        "covered every mutation/entity/embedder cell",
        observed_support.len()
            == MutationCategory::all().len() * EntityType::all().len() * EmbedderId::all().len(),
    )?;

    let thresholds = Thresholds::try_new(
        EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, 0.0))
            .collect(),
        BTreeMap::new(),
    )?;
    let frozen_at = SystemTime::now();
    let (constellation, shrinkage_decisions) = builder.finalize(thresholds, frozen_at)?;
    require(
        "constellation chunk cells match support cells",
        constellation.per_chunk_type_centroids.len() == observed_support.len(),
    )?;

    let db_path = root.join("tct-refresh-rocksdb");
    let db = open_tct_rocksdb(&db_path)?;
    let store = ConstellationStore::new(db.clone())?;
    let constellation_count_before = store.count_constellations()?;
    let refresh_count_before = store.count_refresh_reports()?;
    let guard_count_before = store.count_guard_decisions()?;
    let version_id = store.persist(&constellation)?;
    let constellation_count_after = store.count_constellations()?;
    require(
        "constellation persist increments store count",
        constellation_count_after == constellation_count_before + 1,
    )?;

    let (good_chunk, good_panel) =
        first_good_function.ok_or("missing real known-good function panel sample")?;
    let bad_chunk = ChunkId::try_new(
        good_chunk.file_sha,
        good_chunk.language,
        good_chunk.entity_type,
        good_chunk.line_start + 1000,
        good_chunk.line_end + 1000,
        [9u8; 16],
    )?;
    let bad_panel = negate_slot(&good_panel, panel_slot_for_embedder(EmbedderId::E7))?;
    let chunk_diagnostics = gtau_check_chunks(
        &[
            (good_chunk.clone(), good_panel.clone()),
            (bad_chunk.clone(), bad_panel),
        ],
        MutationCategory::KnownGood,
        &constellation,
    )?;
    require(
        "diagnostic chunks reject exactly one corrupted chunk",
        !chunk_diagnostics.aggregate_satisfied && chunk_diagnostics.violating_chunks.len() == 1,
    )?;
    require(
        "diagnostic corrupted chunk is durable bad chunk",
        chunk_diagnostics.violating_chunks[0] == bad_chunk,
    )?;

    let cell_support = build_cell_support(&constellation, &observed_support, &shrinkage_decisions)?;
    let shrinkage = shrinkage_summary(&cell_support)?;
    let per_entity_support = entity_counts
        .iter()
        .map(|(entity_type, chunk_count)| EntitySupportRecord::try_new(*entity_type, *chunk_count))
        .collect::<Result<Vec<_>, _>>()?;
    let operator_diagnostics = operator_diagnostics(&chunk_diagnostics)?;
    let started_at = SystemTime::now();
    let input = ConstellationRefreshReportInput {
        started_at,
        finished_at: started_at + Duration::from_secs(1),
        constellation_version_id: version_id,
        corpus_sha,
        code_version: code_version.clone(),
        source_corpus_path: corpus_path.display().to_string(),
        source_corpus_sha256: corpus_sha,
        source_row_count: corpus.entries.len(),
        source_chunk_count: ingested_panel_count,
        ingested_panel_count,
        per_entity_support,
        cell_support,
        shrinkage,
        operator_diagnostics,
    };
    let report = ConstellationRefreshReport::try_new(input.clone())?;
    let report_id = store.persist_refresh_report(&report)?;
    require(
        "persisted report id matches report",
        report_id == report.report_id,
    )?;
    flush_tct_cfs(db.as_ref())?;
    let refresh_count_after = store.count_refresh_reports()?;
    require(
        "refresh report persist increments store count",
        refresh_count_after == refresh_count_before + 1,
    )?;
    let report_readback = store.load_refresh_report(report_id)?;
    require(
        "refresh report readback equality",
        report_readback == report,
    )?;
    let reports_for_version = store.refresh_reports_for_constellation(version_id, 10)?;
    require(
        "refresh report listed for constellation",
        reports_for_version
            .iter()
            .any(|item| item.report_id == report_id),
    )?;

    let guard_reward = record_guard_decisions(
        db.clone(),
        version_id,
        &chunk_diagnostics,
        guard_count_before,
    )?;
    flush_tct_cfs(db.as_ref())?;
    let guard_count_after = store.count_guard_decisions()?;
    require(
        "guard decision records persisted",
        guard_count_after == guard_count_before + MIN_SAMPLES_FOR_RATE,
    )?;

    drop(store);
    drop(db);

    let reopened_db = open_tct_rocksdb(&db_path)?;
    let reopened_store = ConstellationStore::new(reopened_db.clone())?;
    let reopened_report = reopened_store.load_refresh_report(report_id)?;
    let reopened_latest = reopened_store.latest_refresh_report()?;
    let reopened_constellation = reopened_store.load(version_id, &versions)?;
    let reopened_aggregator = ViolationRateAggregator::new(reopened_db.clone())?;
    let reopened_rate = reopened_aggregator
        .constellation_violation_rate(RollingWindow::try_new(30, 24)?, SystemTime::now())?
        .ok_or("missing reopened constellation violation rate")?;
    require(
        "reopened refresh report equality",
        reopened_report == report,
    );
    require(
        "reopened latest refresh report id matches",
        reopened_latest.report_id == report_id,
    )?;
    require(
        "reopened constellation id matches",
        reopened_constellation.version_id() == version_id,
    )?;
    assert_close("reopened violation rate", reopened_rate, 0.2, 0.00001)?;

    let edge_cases = run_edge_cases(
        &reopened_store,
        version_id,
        report.clone(),
        input,
        &reopened_constellation,
        reopened_aggregator.count_rows()?,
        reopened_store.count_refresh_reports()?,
    )?;
    if edge_cases
        .iter()
        .any(|case| case.get("passes").and_then(|value| value.as_bool()) != Some(true))
    {
        return Err(format!(
            "one or more TCT refresh edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let evidence = RefreshFsvEvidence {
        all_passed: true,
        source_of_truth: json!({
            "rocksdb_path": db_path,
            "constellation_cf": CF_MEJEPA_CONSTELLATION,
            "refresh_report_cf": CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
            "guard_decisions_cf": CF_MEJEPA_GUARD_DECISIONS,
            "real_phase1_panel": panel_path,
            "real_corpus_index": corpus_path,
            "evidence_file": root.join("tct-centroid-refresh-fsv-summary.json"),
        }),
        happy_path: json!({
            "before_state": {
                "constellation_rows": constellation_count_before,
                "refresh_report_rows": refresh_count_before,
                "guard_decision_rows": guard_count_before,
                "real_corpus_complete": corpus.complete,
                "real_corpus_rows": corpus.entries.len(),
                "real_panel_hash": hash_f32s(real_panel.data()),
            },
            "trigger": "real corpus index + real Phase 1 panel -> ConstellationBuilder -> ConstellationStore::persist_refresh_report -> flush -> reopen -> load_refresh_report",
            "after_state": {
                "constellation_rows": reopened_store.count_constellations()?,
                "refresh_report_rows": reopened_store.count_refresh_reports()?,
                "guard_decision_rows": reopened_store.count_guard_decisions()?,
                "constellation_version_id": hex::encode(version_id),
                "refresh_report_id": hex::encode(report_id),
                "source_chunk_count": reopened_report.source_chunk_count,
                "ingested_panel_count": reopened_report.ingested_panel_count,
                "cell_support_count": reopened_report.cell_support.len(),
                "entity_support_count": reopened_report.per_entity_support.len(),
                "shrinkage": reopened_report.shrinkage,
                "operator_diagnostics": reopened_report.operator_diagnostics,
                "reports_for_version": reports_for_version.len(),
                "first_real_entry": first_good_entry,
            },
            "passes": true
        }),
        guard_decision_reward_signals: guard_reward,
        edge_cases,
        physical_proof: json!({
            "rocksdb_files": rocksdb_files(&db_path)?,
            "reopened_refresh_report": {
                "report_id": hex::encode(reopened_report.report_id),
                "constellation_version_id": hex::encode(reopened_report.constellation_version_id),
                "source_corpus_sha256": hex::encode(reopened_report.source_corpus_sha256),
                "cell_support_count": reopened_report.cell_support.len(),
            },
            "reopened_constellation": {
                "version_id": hex::encode(reopened_constellation.version_id()),
                "per_chunk_type_centroids": reopened_constellation.per_chunk_type_centroids.len(),
            },
            "reopened_guard_rate": reopened_rate,
        }),
    };
    write_and_verify(
        &root.join("tct-centroid-refresh-fsv-summary.json"),
        &evidence,
    )?;
    write_and_verify(
        &root.join("tct-centroid-refresh-report-readback.json"),
        &reopened_report,
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn read_real_phase1_panel(path: &Path) -> FsvResult<Panel> {
    if !path.is_file() {
        return Err(format!(
            "required real Phase 1 panel missing at {}; run `cargo run --release -p context-graph-mejepa-instruments --example phase1_extended_fsv` first",
            path.display()
        )
        .into());
    }
    let text = fs::read_to_string(path)?;
    let panel: Panel = serde_json::from_str(&text)?;
    for slot in InstrumentSlot::all() {
        require(
            &format!("slot {} filled", slot.slug()),
            panel.is_filled(slot),
        )?;
    }
    Ok(panel)
}

fn read_corpus_index(path: &Path) -> FsvResult<CorpusIndex> {
    if !path.is_file() {
        return Err(format!("required real corpus index missing at {}", path.display()).into());
    }
    let text = fs::read_to_string(path)?;
    let index: CorpusIndex = serde_json::from_str(&text)?;
    require("corpus index marked complete", index.complete)?;
    require("corpus index has rows", !index.entries.is_empty())?;
    Ok(index)
}

fn parse_mutation_category(value: &str) -> FsvResult<MutationCategory> {
    let category = match value {
        "known_good" => MutationCategory::KnownGood,
        "subtle_flip" => MutationCategory::SubtleFlip,
        "off_by_one" => MutationCategory::OffByOne,
        "swap_variable" => MutationCategory::SwapVariable,
        "delete_test_call" => MutationCategory::DeleteTestCall,
        "wrong_file" => MutationCategory::WrongFile,
        "over_engineer" => MutationCategory::OverEngineer,
        "compile_error" => MutationCategory::CompileError,
        other => return Err(format!("unknown corpus mutation category {other:?}").into()),
    };
    Ok(category)
}

fn derive_panel(
    base: &Panel,
    mutation_idx: usize,
    entity_idx: usize,
    repeat: usize,
    entry: &CorpusEntry,
) -> FsvResult<Panel> {
    let seed = stable_seed(&format!(
        "{}:{}:{}:{}:{}",
        entry.task_id, entry.patch_sha256, mutation_idx, entity_idx, repeat
    ));
    let mut builder = PanelBuilder::new();
    for slot in InstrumentSlot::all() {
        let values = base
            .slot(slot)
            .iter()
            .enumerate()
            .map(|(idx, value)| {
                let phase = ((seed as usize % 997) + idx + slot.offset()) as f32 * 0.013;
                let scale = 1.0 + 0.004 * phase.sin();
                let offset = 0.0015 * (phase * 0.7).cos();
                value * scale + offset
            })
            .collect::<Vec<_>>();
        builder.set_slot(slot, &values)?;
    }
    builder.require_slots(&InstrumentSlot::all())?;
    Ok(builder.build()?)
}

fn chunk_id_from_entry(
    entry: &CorpusEntry,
    entity_type: EntityType,
    repeat: usize,
) -> FsvResult<ChunkId> {
    let file_sha = hex_digest_to_32(&entry.patch_sha256)?;
    let parent_digest = Sha256::digest(format!(
        "{}:{}:{entity_type:?}:{repeat}",
        entry.task_id, entry.patch_path
    ));
    let mut parent_chain_hash = [0u8; 16];
    parent_chain_hash.copy_from_slice(&parent_digest[..16]);
    let line_start = 1 + u32::try_from(repeat)? * 10;
    ChunkId::try_new(
        file_sha,
        Language::Python,
        entity_type,
        line_start,
        line_start + 5,
        parent_chain_hash,
    )
    .map_err(Into::into)
}

fn build_cell_support(
    constellation: &TctConstellation,
    observed_support: &BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), usize>,
    shrinkage_decisions: &[ShrinkageDecision],
) -> FsvResult<Vec<CellSupportRecord>> {
    let observed_by_cell = observed_support;
    let decision_by_cell = shrinkage_decisions
        .iter()
        .map(|decision| (decision.cell, decision))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::new();
    for (cell, centroid) in &constellation.per_chunk_type_centroids {
        let observed = *observed_by_cell
            .get(cell)
            .ok_or_else(|| format!("missing observed support for {cell:?}"))?;
        let decision = decision_by_cell
            .get(cell)
            .ok_or_else(|| format!("missing shrinkage decision for {cell:?}"))?;
        require(
            &format!("shrinkage observed count matches support for {cell:?}"),
            decision.observed_n == observed,
        )?;
        records.push(CellSupportRecord::try_new(
            cell.0,
            cell.1,
            cell.2,
            cell.3,
            observed,
            centroid.sample_count,
            centroid.origin,
        )?);
    }
    Ok(records)
}

fn shrinkage_summary(cell_support: &[CellSupportRecord]) -> FsvResult<RefreshShrinkageSummary> {
    let mut summary = RefreshShrinkageSummary {
        total_cells: cell_support.len(),
        own_cell: 0,
        language_aggregate: 0,
        entity_aggregate: 0,
        category_aggregate: 0,
    };
    for cell in cell_support {
        match cell.centroid_origin {
            ShrinkageOrigin::OwnCell => summary.own_cell += 1,
            ShrinkageOrigin::LanguageAggregate => summary.language_aggregate += 1,
            ShrinkageOrigin::EntityAggregate => summary.entity_aggregate += 1,
            ShrinkageOrigin::CategoryAggregate => summary.category_aggregate += 1,
        }
    }
    summary.validate()?;
    Ok(summary)
}

fn operator_diagnostics(output: &ChunkGtauOutput) -> FsvResult<OperatorDiagnosticSummary> {
    let mut embedders = BTreeSet::new();
    let mut worst_margin = f32::INFINITY;
    for (_chunk, gtau) in &output.per_chunk_results {
        worst_margin = worst_margin.min(gtau.min_margin);
        for violation in &gtau.violations {
            embedders.insert(violation.embedder);
        }
    }
    let summary = OperatorDiagnosticSummary {
        inspected_chunk_count: output.per_chunk_results.len(),
        rejected_chunk_count: output.violating_chunks.len(),
        violating_embedder_count: embedders.len(),
        worst_margin,
    };
    summary.validate()?;
    Ok(summary)
}

fn record_guard_decisions(
    db: Arc<DB>,
    version_id: [u8; 32],
    chunk_diagnostics: &ChunkGtauOutput,
    before_count: usize,
) -> FsvResult<serde_json::Value> {
    let aggregator = ViolationRateAggregator::new(db)?;
    let violations = chunk_diagnostics
        .per_chunk_results
        .iter()
        .flat_map(|(_chunk, gtau)| gtau.violations.clone())
        .collect::<Vec<_>>();
    require("guard rejection has violations", !violations.is_empty())?;
    let predictor = PredictorOutput::try_new(
        MutationCategory::KnownGood,
        vec![0.82, 0.11, 0.07],
        (0.74, 0.90),
        1.0,
    )?;
    let now = SystemTime::now();
    for idx in 0..MIN_SAMPLES_FOR_RATE {
        let timestamp = now - Duration::from_secs((MIN_SAMPLES_FOR_RATE - idx) as u64);
        let record = if idx < 6 {
            VerdictRecord::GuardRejected(VerdictGuardRejectedSummary {
                timestamp,
                payload: VerdictGuardRejected::try_new(
                    violations.clone(),
                    predictor.clone(),
                    chunk_diagnostics.violating_chunks.clone(),
                    version_id,
                )?,
            })
        } else {
            VerdictRecord::Approve(VerdictApproveSummary {
                timestamp,
                constellation_version_id: version_id,
            })
        };
        aggregator.record_verdict(&record)?;
    }
    let after_count = aggregator.count_rows()?;
    let window = RollingWindow::try_new(30, 24)?;
    let rate = aggregator
        .constellation_violation_rate(window, now)?
        .ok_or("expected violation rate after 30 persisted decisions")?;
    assert_close("constellation violation rate", rate, 0.2, 0.00001)?;
    Ok(json!({
        "before_state": {"guard_decision_rows": before_count},
        "trigger": "6 guard rejections + 24 approvals persisted to CF_MEJEPA_GUARD_DECISIONS, then RollingWindow readback",
        "after_state": {
            "guard_decision_rows": after_count,
            "window_size": 30,
            "rejected": 6,
            "approved": 24,
            "constellation_violation_rate": rate,
            "delta_xi_constellation_factor": (1.0_f32 - rate).powf(0.2),
            "durable_reward_signal": 1.0_f32 - rate,
        },
        "passes": after_count == before_count + MIN_SAMPLES_FOR_RATE
    }))
}

fn run_edge_cases(
    store: &ConstellationStore,
    version_id: [u8; 32],
    report: ConstellationRefreshReport,
    base_input: ConstellationRefreshReportInput,
    constellation: &TctConstellation,
    guard_count_before: usize,
    refresh_count_before: usize,
) -> FsvResult<Vec<serde_json::Value>> {
    let mut empty_cell_input = base_input.clone();
    empty_cell_input.cell_support = Vec::new();
    empty_cell_input.shrinkage = RefreshShrinkageSummary {
        total_cells: 0,
        own_cell: 0,
        language_aggregate: 0,
        entity_aggregate: 0,
        category_aggregate: 0,
    };
    let empty_cell_error = ConstellationRefreshReport::try_new(empty_cell_input)
        .expect_err("empty cell support must fail");
    let refresh_after_empty = store.count_refresh_reports()?;

    let mut corrupted = report.clone();
    corrupted.report_id = [1u8; 32];
    let corrupt_error = corrupted
        .validate_integrity()
        .expect_err("corrupted report id must fail");
    let refresh_after_corrupt = store.count_refresh_reports()?;

    let empty_chunk_error = gtau_check_chunks(&[], MutationCategory::KnownGood, constellation)
        .expect_err("empty chunk diagnostics must fail");
    let guard_after_empty_chunk = store.count_guard_decisions()?;

    let zero_limit_error = store
        .refresh_reports_for_constellation(version_id, 0)
        .expect_err("zero refresh-report limit must fail");
    let refresh_after_zero_limit = store.count_refresh_reports()?;

    Ok(vec![
        json!({
            "name": "empty_cell_support_rejected",
            "before_state": {"refresh_report_rows": refresh_count_before, "cell_support_count": report.cell_support.len()},
            "trigger": "ConstellationRefreshReport::try_new with empty cell_support",
            "after_state": {"error": empty_cell_error.code(), "refresh_report_rows": refresh_after_empty},
            "passes": refresh_after_empty == refresh_count_before
        }),
        json!({
            "name": "corrupt_report_id_rejected",
            "before_state": {"refresh_report_rows": refresh_count_before, "report_id": hex::encode(report.report_id)},
            "trigger": "ConstellationRefreshReport::validate_integrity after report_id mutation",
            "after_state": {"error": corrupt_error.code(), "refresh_report_rows": refresh_after_corrupt},
            "passes": refresh_after_corrupt == refresh_count_before
        }),
        json!({
            "name": "empty_chunk_guard_rejected",
            "before_state": {"guard_decision_rows": guard_count_before, "chunk_count": 0},
            "trigger": "gtau_check_chunks([])",
            "after_state": {"error": empty_chunk_error.code(), "guard_decision_rows": guard_after_empty_chunk},
            "passes": guard_after_empty_chunk == guard_count_before
        }),
        json!({
            "name": "zero_refresh_report_limit_rejected",
            "before_state": {"refresh_report_rows": refresh_count_before, "limit": 0},
            "trigger": "ConstellationStore::refresh_reports_for_constellation(version_id, 0)",
            "after_state": {"error": zero_limit_error.code(), "refresh_report_rows": refresh_after_zero_limit},
            "passes": refresh_after_zero_limit == refresh_count_before
        }),
    ])
}

fn embedder_versions_from_panel(panel: &Panel) -> BTreeMap<EmbedderId, [u8; 32]> {
    EmbedderId::all()
        .into_iter()
        .map(|embedder| {
            let slot = panel_slot_for_embedder(embedder);
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&sha2_digest(panel.slot(slot)));
            (embedder, bytes)
        })
        .collect()
}

fn sha2_digest(values: &[f32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

fn negate_slot(panel: &Panel, slot: InstrumentSlot) -> FsvResult<Panel> {
    let mut builder = PanelBuilder::new();
    for current in InstrumentSlot::all() {
        let mut vector = panel.slot(current).to_vec();
        if current == slot {
            for value in &mut vector {
                *value = -*value;
            }
        }
        builder.set_slot(current, &vector)?;
    }
    builder.require_slots(&InstrumentSlot::all())?;
    Ok(builder.build()?)
}

fn hex_digest_to_32(value: &str) -> FsvResult<[u8; 32]> {
    let hex_value = value.strip_prefix("sha256:").unwrap_or(value);
    let bytes = hex::decode(hex_value)?;
    if bytes.len() != 32 {
        return Err(format!("expected 32-byte sha256 digest, got {} bytes", bytes.len()).into());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn stable_seed(value: &str) -> u64 {
    let digest = Sha256::digest(value.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}

fn flush_tct_cfs(db: &DB) -> FsvResult<()> {
    for cf_name in MEJEPA_TCT_CFS {
        let cf = db
            .cf_handle(cf_name)
            .ok_or_else(|| format!("missing TCT column family {cf_name}"))?;
        db.flush_cf(cf)?;
    }
    Ok(())
}

fn rocksdb_files(path: &Path) -> FsvResult<Vec<String>> {
    let mut files = fs::read_dir(path)?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().to_string())
                .map_err(Into::into)
        })
        .collect::<FsvResult<Vec<_>>>()?;
    files.sort();
    Ok(files)
}

fn git_head() -> FsvResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --verify HEAD failed; cannot record code_version".into());
    }
    let sha = String::from_utf8(output.stdout)?.trim().to_string();
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("git HEAD was not a 40-char hex SHA: {sha:?}").into());
    }
    Ok(sha.to_ascii_lowercase())
}

fn write_and_verify<T>(path: &Path, value: &T) -> FsvResult<()>
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        OpenOptions::new().read(true).open(parent)?.sync_all()?;
    }
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    require(&format!("{} mode 0600", path.display()), mode == 0o600)?;
    let readback_bytes = fs::read(path)?;
    let readback: T = serde_json::from_slice(&readback_bytes)?;
    require(
        &format!("{} serde readback equality", path.display()),
        readback == *value,
    )?;
    require(
        &format!("{} canonical bytes equality", path.display()),
        serde_json::to_vec_pretty(&readback)? == readback_bytes,
    )?;
    Ok(())
}

fn assert_close(label: &str, actual: f32, expected: f32, tolerance: f32) -> FsvResult<()> {
    if (actual - expected).abs() <= tolerance {
        Ok(())
    } else {
        Err(format!("{label}: actual {actual} differs from expected {expected}").into())
    }
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {label}").into())
    }
}
