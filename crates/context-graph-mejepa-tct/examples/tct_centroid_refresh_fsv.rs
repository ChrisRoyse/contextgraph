use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use context_graph_mejepa_instruments::{hash_f32s, InstrumentSlot, Panel, PanelBuilder};
use context_graph_mejepa_tct::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

type FsvResult<T> = Result<T, Box<dyn Error>>;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-refresh-fsv";
const CORPUS_INDEX: &str =
    "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const AST_CORPUS_FSV: &str =
    "/mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv/swebench-python-ast-corpus-fsv.json";
const PHASE1_PANEL: &str =
    "/mnt/d/contextgraph/fsv/contextgraph-mejepa-instruments-fsv/phase1-extended/panel-all-slots.json";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CorpusEntry {
    task_id: String,
    repo: String,
    mutation: MutationCategory,
    oracle_outcome: OracleOutcome,
    patch_path: PathBuf,
    patch_sha256: [u8; 32],
}

#[derive(Debug)]
struct RefreshBuild {
    constellation: TctConstellation,
    report: ConstellationRefreshReport,
    operator_chunks: ChunkGtauOutput,
    support_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshEvidence {
    source_of_truth: Value,
    before_state: Value,
    after_state: Value,
    edge_case_audit: Value,
    report_readback: ConstellationRefreshReport,
    operator_chunk_diagnostics: ChunkGtauOutput,
    support_summary: Value,
    passes: bool,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from(FSV_ROOT);
    reset_secure_dir(&root)?;

    let corpus_index_path = PathBuf::from(CORPUS_INDEX);
    let ast_fsv_path = PathBuf::from(AST_CORPUS_FSV);
    let panel_path = PathBuf::from(PHASE1_PANEL);
    let db_path = root.join("tct-refresh-rocksdb");
    let report_readback_path = root.join("refresh-report-readback.json");
    let evidence_path = root.join("tct-centroid-refresh-evidence.json");

    let corpus_index_bytes = fs::read(&corpus_index_path)?;
    let source_corpus_sha256 = sha256_array(&corpus_index_bytes);
    let entries = load_corpus_entries(&corpus_index_path)?;
    require("full Python 300x8 corpus rows", entries.len() == 2400)?;
    let entity_by_task = load_entity_by_task(&ast_fsv_path)?;
    require("AST FSV task coverage", entity_by_task.len() == 300)?;
    let base_panel = read_real_phase1_panel(&panel_path)?;
    let versions = embedder_versions_from_panel(&base_panel);
    let code_version = git_head()?;
    let corpus_sha = corpus_sha_from_index(&corpus_index_bytes)?;

    let before_state = json!({
        "corpus_index": corpus_index_path,
        "corpus_index_bytes": corpus_index_bytes.len(),
        "corpus_index_sha256": hex::encode(source_corpus_sha256),
        "ast_fsv": ast_fsv_path,
        "phase1_panel": panel_path,
        "phase1_panel_hash": hash_f32s(base_panel.data()),
        "corpus_rows": entries.len(),
        "rocksdb_refresh_rows_before": 0,
        "rocksdb_constellation_rows_before": 0,
    });
    println!("before tct_refresh: {}", serde_json::to_string(&before_state)?);

    let build = build_refresh_report(
        &entries,
        &entity_by_task,
        &base_panel,
        &versions,
        &code_version,
        corpus_sha,
        source_corpus_sha256,
        CORPUS_INDEX,
    )?;

    let db = open_tct_rocksdb(&db_path)?;
    let store = ConstellationStore::new(db.clone())?;
    require("refresh rows start empty", store.count_refresh_reports()? == 0)?;
    let version_id = store.persist(&build.constellation)?;
    let report_id = store.persist_refresh_report(&build.report)?;
    require(
        "persisted report id",
        report_id == build.report.report_id,
    )?;
    require(
        "persisted constellation version",
        version_id == build.constellation.version_id(),
    )?;
    let live_counts = json!({
        "constellation_rows_after_persist": store.count_constellations()?,
        "refresh_report_rows_after_persist": store.count_refresh_reports()?,
    });
    drop(store);
    drop(db);

    let reopened = ConstellationStore::new(open_tct_rocksdb(&db_path)?)?;
    let loaded_report = reopened.load_refresh_report(report_id)?;
    let latest_report = reopened.latest_refresh_report()?;
    let reports_for_version = reopened.refresh_reports_for_constellation(version_id, 10)?;
    require("loaded report equals persisted", loaded_report == build.report)?;
    require("latest report equals persisted", latest_report == build.report)?;
    require("one report for version", reports_for_version.len() == 1)?;
    require(
        "report for version equals persisted",
        reports_for_version[0] == build.report,
    )?;
    write_and_verify(&report_readback_path, &loaded_report)?;

    let edge_case_audit = run_edge_case_audit(&root, &reopened, &build.report, version_id)?;
    let after_state = json!({
        "live_counts": live_counts,
        "reopened_constellation_rows": reopened.count_constellations()?,
        "reopened_refresh_report_rows": reopened.count_refresh_reports()?,
        "loaded_report_id": hex::encode(loaded_report.report_id),
        "loaded_constellation_version_id": hex::encode(loaded_report.constellation_version_id),
        "source_row_count": loaded_report.source_row_count,
        "source_chunk_count": loaded_report.source_chunk_count,
        "ingested_panel_count": loaded_report.ingested_panel_count,
        "mutation_category_count": loaded_report.per_category_support.len(),
        "language_count": loaded_report.per_language_support.len(),
        "oracle_outcome_count": loaded_report.per_oracle_outcome_support.len(),
        "cell_support_count": loaded_report.cell_support.len(),
        "rocksdb_file_count": count_files(&db_path)?,
    });
    println!("after tct_refresh: {}", serde_json::to_string(&after_state)?);

    require("all 8 mutation categories captured", loaded_report.per_category_support.len() == 8)?;
    require("Python language captured", loaded_report.per_language_support.len() == 1)?;
    require("refresh cells captured", !loaded_report.cell_support.is_empty())?;
    require(
        "operator rejected diagnostic captured",
        loaded_report.operator_diagnostics.rejected_chunk_count == 1,
    )?;

    let evidence = RefreshEvidence {
        source_of_truth: json!({
            "rocksdb_path": db_path,
            "constellation_cf": CF_MEJEPA_CONSTELLATION,
            "refresh_report_cf": CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
            "corpus_index": corpus_index_path,
            "ast_chunk_fsv": ast_fsv_path,
            "phase1_panel": panel_path,
            "report_readback_json": report_readback_path,
            "evidence_json": evidence_path,
        }),
        before_state,
        after_state,
        edge_case_audit,
        report_readback: loaded_report,
        operator_chunk_diagnostics: build.operator_chunks,
        support_summary: build.support_summary,
        passes: true,
    };
    write_and_verify(&evidence_path, &evidence)?;
    let evidence_readback: RefreshEvidence =
        serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require("evidence readback equality", evidence_readback == evidence)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "tct_centroid_refresh_fsv": "passed",
            "evidence_json": evidence_path,
            "report_readback_json": report_readback_path,
            "source_of_truth": evidence.source_of_truth,
            "after_state": evidence.after_state,
            "edge_case_audit": evidence.edge_case_audit,
        }))?
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_refresh_report(
    entries: &[CorpusEntry],
    entity_by_task: &BTreeMap<String, EntityType>,
    base_panel: &Panel,
    versions: &BTreeMap<EmbedderId, [u8; 32]>,
    code_version: &str,
    corpus_sha: [u8; 32],
    source_corpus_sha256: [u8; 32],
    source_corpus_path: &str,
) -> FsvResult<RefreshBuild> {
    let mut builder = ConstellationBuilder::new(corpus_sha, versions.clone(), code_version.into())?;
    let mut entity_counts = BTreeMap::<EntityType, usize>::new();
    let mut category_row_counts = BTreeMap::<MutationCategory, usize>::new();
    let mut category_chunk_counts = BTreeMap::<MutationCategory, usize>::new();
    let mut language_row_counts = BTreeMap::<Language, usize>::new();
    let mut language_chunk_counts = BTreeMap::<Language, usize>::new();
    let mut oracle_row_counts = BTreeMap::<OracleOutcome, usize>::new();
    let mut observed_cell_counts =
        BTreeMap::<(MutationCategory, EntityType, Language, EmbedderId), usize>::new();
    let mut sample_good: Option<(ChunkId, Panel, MutationCategory)> = None;

    for (index, entry) in entries.iter().enumerate() {
        let entity_type = *entity_by_task
            .get(&entry.task_id)
            .ok_or_else(|| format!("AST FSV entity support missing for {}", entry.task_id))?;
        let patch_bytes = fs::read(&entry.patch_path)?;
        let panel = panel_from_patch(base_panel, &patch_bytes, entry.mutation, index)?;
        let chunk_id = ChunkId::try_new(
            entry.patch_sha256,
            Language::Python,
            entity_type,
            1,
            1,
            parent_chain_hash(&entry.task_id),
        )?;
        builder.ingest_corpus_entry(
            &panel,
            entry.mutation,
            entry.oracle_outcome,
            Language::Python,
            entity_type,
            &[(chunk_id.clone(), panel.clone())],
        )?;
        *entity_counts.entry(entity_type).or_default() += 1;
        *category_row_counts.entry(entry.mutation).or_default() += 1;
        *category_chunk_counts.entry(entry.mutation).or_default() += 1;
        *language_row_counts.entry(Language::Python).or_default() += 1;
        *language_chunk_counts.entry(Language::Python).or_default() += 1;
        *oracle_row_counts.entry(entry.oracle_outcome).or_default() += 1;
        for embedder in EmbedderId::all() {
            *observed_cell_counts
                .entry((entry.mutation, entity_type, Language::Python, embedder))
                .or_default() += 1;
        }
        if sample_good.is_none() && entry.mutation == MutationCategory::KnownGood {
            sample_good = Some((chunk_id, panel, entry.mutation));
        }
    }

    let thresholds = Thresholds::try_new(
        EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, 0.0))
            .collect(),
        BTreeMap::new(),
    )?;
    let frozen_at = SystemTime::now();
    let (constellation, shrinkage_decisions) = builder.finalize(thresholds, frozen_at)?;
    let (sample_chunk, sample_panel, sample_mutation) =
        sample_good.ok_or("known_good sample missing from corpus")?;
    let rejected_panel = negate_slot(&sample_panel, panel_slot_for_embedder(EmbedderId::E7))?;
    let rejected_chunk = ChunkId::try_new(
        [0x7e; 32],
        Language::Python,
        sample_chunk.entity_type,
        2,
        2,
        [0x11; 16],
    )?;
    let operator_chunks = gtau_check_chunks(
        &[
            (sample_chunk.clone(), sample_panel),
            (rejected_chunk, rejected_panel),
        ],
        sample_mutation,
        &constellation,
    )?;
    require("operator aggregate rejects exactly one chunk", !operator_chunks.aggregate_satisfied)?;
    require(
        "operator violating chunk count",
        operator_chunks.violating_chunks.len() == 1,
    )?;
    let violating_embedder_count = operator_chunks
        .per_chunk_results
        .iter()
        .map(|(_chunk, output)| output.violations.len())
        .sum::<usize>();
    let worst_margin = operator_chunks
        .per_chunk_results
        .iter()
        .map(|(_chunk, output)| output.min_margin)
        .fold(f32::INFINITY, f32::min);

    let per_entity_support = entity_counts
        .iter()
        .map(|(entity_type, count)| EntitySupportRecord::try_new(*entity_type, *count))
        .collect::<Result<Vec<_>, _>>()?;
    let per_category_support = category_row_counts
        .iter()
        .map(|(mutation, rows)| {
            CategorySupportRecord::try_new(
                *mutation,
                *rows,
                *category_chunk_counts.get(mutation).unwrap_or(&0),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let per_language_support = language_row_counts
        .iter()
        .map(|(language, rows)| {
            LanguageSupportRecord::try_new(
                *language,
                *rows,
                *language_chunk_counts.get(language).unwrap_or(&0),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let per_oracle_outcome_support = oracle_row_counts
        .iter()
        .map(|(outcome, rows)| OracleOutcomeSupportRecord::try_new(*outcome, *rows))
        .collect::<Result<Vec<_>, _>>()?;
    let cell_support = constellation
        .per_chunk_type_centroids
        .iter()
        .map(|((mutation, entity_type, language, embedder), centroid)| {
            let observed = *observed_cell_counts
                .get(&(*mutation, *entity_type, *language, *embedder))
                .unwrap_or(&0);
            CellSupportRecord::try_new(
                *mutation,
                *entity_type,
                *language,
                *embedder,
                observed,
                centroid.sample_count,
                centroid.origin,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shrinkage = summarize_shrinkage(&shrinkage_decisions);
    let source_row_count = entries.len();
    let source_chunk_count = entries.len();
    let panel_level_cell_count = constellation
        .per_category_centroids
        .values()
        .map(BTreeMap::len)
        .sum::<usize>()
        + constellation
            .per_language_centroids
            .values()
            .map(BTreeMap::len)
            .sum::<usize>()
        + constellation
            .outcome_centroids
            .values()
            .map(BTreeMap::len)
            .sum::<usize>();
    let operator_diagnostics = OperatorDiagnosticSummary {
        inspected_chunk_count: operator_chunks.per_chunk_results.len(),
        rejected_chunk_count: operator_chunks.violating_chunks.len(),
        violating_embedder_count,
        worst_margin,
    };
    let reward_signal_summary = RefreshRewardSignalSummary {
        source_row_count,
        source_chunk_count,
        mutation_category_count: per_category_support.len(),
        language_count: per_language_support.len(),
        oracle_outcome_count: per_oracle_outcome_support.len(),
        per_chunk_type_cell_count: cell_support.len(),
        panel_level_cell_count,
        strict_guard_rejection_count: operator_diagnostics.rejected_chunk_count,
        violating_chunk_count: operator_diagnostics.rejected_chunk_count,
        estimated_reward_scalar_count: source_chunk_count * (EmbedderId::all().len() + 2)
            + cell_support.len()
            + panel_level_cell_count,
    };
    let report = ConstellationRefreshReport::try_new(ConstellationRefreshReportInput {
        started_at: frozen_at,
        finished_at: SystemTime::now(),
        constellation_version_id: constellation.version_id(),
        corpus_sha,
        code_version: code_version.to_string(),
        source_corpus_path: source_corpus_path.to_string(),
        source_corpus_sha256,
        source_row_count,
        source_chunk_count,
        ingested_panel_count: source_row_count + source_chunk_count,
        per_entity_support,
        per_category_support,
        per_language_support,
        per_oracle_outcome_support,
        cell_support,
        shrinkage,
        operator_diagnostics,
        reward_signal_summary,
    })?;
    let support_summary = json!({
        "rows_by_category": category_row_counts,
        "chunks_by_category": category_chunk_counts,
        "chunks_by_entity": entity_counts,
        "rows_by_language": language_row_counts,
        "rows_by_oracle_outcome": oracle_row_counts,
        "observed_cell_count": observed_cell_counts.len(),
        "panel_level_cell_count": panel_level_cell_count,
        "shrinkage_decision_count": shrinkage_decisions.len(),
    });

    Ok(RefreshBuild {
        constellation,
        report,
        operator_chunks,
        support_summary,
    })
}

fn run_edge_case_audit(
    root: &Path,
    store: &ConstellationStore,
    report: &ConstellationRefreshReport,
    version_id: [u8; 32],
) -> FsvResult<Value> {
    let mut edges = BTreeMap::new();

    let before_empty = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "cell_support_count": report.cell_support.len(),
        "expected_error": "MEJEPA_TCT_INSUFFICIENT_SAMPLES",
    });
    println!("before empty_cell_support: {}", serde_json::to_string(&before_empty)?);
    let mut empty_cell = report.clone();
    empty_cell.cell_support.clear();
    let empty_error = store.persist_refresh_report(&empty_cell).unwrap_err();
    let after_empty = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "error_code": empty_error.code(),
        "state_mutated": false,
    });
    println!("after empty_cell_support: {}", serde_json::to_string(&after_empty)?);
    edges.insert(
        "empty_cell_support",
        json!({
            "before_state": before_empty,
            "after_state": after_empty,
            "passes": after_empty["error_code"] == "MEJEPA_TCT_INSUFFICIENT_SAMPLES"
                && after_empty["refresh_report_rows"] == before_empty["refresh_report_rows"],
        }),
    );

    let before_category = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "source_row_count": report.source_row_count,
        "category_0_row_count": report.per_category_support[0].row_count,
        "expected_error": "MEJEPA_TCT_INVALID_INPUT",
    });
    println!(
        "before category_support_mismatch: {}",
        serde_json::to_string(&before_category)?
    );
    let mut bad_category = report.clone();
    bad_category.per_category_support[0].row_count += 1;
    let category_error = store.persist_refresh_report(&bad_category).unwrap_err();
    let after_category = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "error_code": category_error.code(),
        "state_mutated": false,
    });
    println!(
        "after category_support_mismatch: {}",
        serde_json::to_string(&after_category)?
    );
    edges.insert(
        "category_support_mismatch",
        json!({
            "before_state": before_category,
            "after_state": after_category,
            "passes": after_category["error_code"] == "MEJEPA_TCT_INVALID_INPUT"
                && after_category["refresh_report_rows"] == before_category["refresh_report_rows"],
        }),
    );

    let before_tamper = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "report_id": hex::encode(report.report_id),
        "expected_error": "MEJEPA_INSTR_FROZEN_VIOLATION",
    });
    println!("before tampered_report_id: {}", serde_json::to_string(&before_tamper)?);
    let mut tampered = report.clone();
    tampered.report_id[0] ^= 0xff;
    let tamper_error = store.persist_refresh_report(&tampered).unwrap_err();
    let after_tamper = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "error_code": tamper_error.code(),
        "state_mutated": false,
    });
    println!("after tampered_report_id: {}", serde_json::to_string(&after_tamper)?);
    edges.insert(
        "tampered_report_id",
        json!({
            "before_state": before_tamper,
            "after_state": after_tamper,
            "passes": after_tamper["error_code"] == "MEJEPA_INSTR_FROZEN_VIOLATION"
                && after_tamper["refresh_report_rows"] == before_tamper["refresh_report_rows"],
        }),
    );

    let before_zero_limit = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "constellation_version_id": hex::encode(version_id),
        "limit": 0,
        "expected_error": "MEJEPA_TCT_INVALID_INPUT",
    });
    println!(
        "before zero_report_limit: {}",
        serde_json::to_string(&before_zero_limit)?
    );
    let zero_limit_error = store.refresh_reports_for_constellation(version_id, 0).unwrap_err();
    let after_zero_limit = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "error_code": zero_limit_error.code(),
        "state_mutated": false,
    });
    println!(
        "after zero_report_limit: {}",
        serde_json::to_string(&after_zero_limit)?
    );
    edges.insert(
        "zero_report_limit",
        json!({
            "before_state": before_zero_limit,
            "after_state": after_zero_limit,
            "passes": after_zero_limit["error_code"] == "MEJEPA_TCT_INVALID_INPUT"
                && after_zero_limit["refresh_report_rows"] == before_zero_limit["refresh_report_rows"],
        }),
    );

    let audit = Value::Object(
        edges
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    );
    write_and_verify(&root.join("edge-case-audit.json"), &audit)?;
    require(
        "all refresh edge cases pass",
        audit.as_object()
            .unwrap()
            .values()
            .all(|edge| edge["passes"] == Value::Bool(true)),
    )?;
    Ok(audit)
}

fn load_corpus_entries(path: &Path) -> FsvResult<Vec<CorpusEntry>> {
    let index: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let entries = index["entries"]
        .as_array()
        .ok_or("corpus index entries must be an array")?;
    let corpus_root = path
        .parent()
        .ok_or("corpus index must have parent directory")?;
    entries
        .iter()
        .map(|entry| {
            let task_id = entry["task_id"]
                .as_str()
                .ok_or("entry task_id must be a string")?
                .to_string();
            let repo = entry["repo"]
                .as_str()
                .ok_or("entry repo must be a string")?
                .to_string();
            let category = entry["category"]
                .as_str()
                .ok_or("entry category must be a string")?;
            let patch_rel = entry["patch_path"]
                .as_str()
                .ok_or("entry patch_path must be a string")?;
            let patch_path = corpus_root.join(patch_rel);
            require(&format!("patch exists {}", patch_path.display()), patch_path.is_file())?;
            let patch_sha = entry["patch_sha256"]
                .as_str()
                .ok_or("entry patch_sha256 must be a string")?;
            let patch_sha256 = hex_digest_32(patch_sha)?;
            let observed_patch_sha = sha256_array(&fs::read(&patch_path)?);
            require(
                &format!("patch sha matches {}", patch_path.display()),
                observed_patch_sha == patch_sha256,
            )?;
            let oracle_outcome = if entry["oracle_all_passed"].as_bool().unwrap_or(false) {
                OracleOutcome::Pass
            } else {
                OracleOutcome::Fail
            };
            Ok(CorpusEntry {
                task_id,
                repo,
                mutation: mutation_from_slug(category)?,
                oracle_outcome,
                patch_path,
                patch_sha256,
            })
        })
        .collect()
}

fn load_entity_by_task(path: &Path) -> FsvResult<BTreeMap<String, EntityType>> {
    let evidence: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let rows = evidence["rows"]
        .as_array()
        .ok_or("AST FSV evidence rows must be an array")?;
    let mut out = BTreeMap::new();
    for row in rows {
        let task_id = row["task_id"]
            .as_str()
            .ok_or("AST FSV row task_id must be a string")?;
        let counts = &row["chunk_state"]["counts"];
        out.insert(task_id.to_string(), primary_entity_type(counts)?);
    }
    Ok(out)
}

fn primary_entity_type(counts: &Value) -> FsvResult<EntityType> {
    let candidates = [
        ("method", EntityType::Method),
        ("function", EntityType::Function),
        ("class", EntityType::Class),
        ("module", EntityType::Module),
        ("testfunction", EntityType::TestFunction),
        ("import", EntityType::Import),
    ];
    let mut best = (0u64, EntityType::Function);
    for (key, entity) in candidates {
        let count = counts[key].as_u64().unwrap_or(0);
        if count > best.0 {
            best = (count, entity);
        }
    }
    require("primary AST entity has support", best.0 > 0)?;
    Ok(best.1)
}

fn panel_from_patch(
    base: &Panel,
    patch_bytes: &[u8],
    mutation: MutationCategory,
    index: usize,
) -> Result<Panel, TctError> {
    let mut seed = Sha256::new();
    seed.update(patch_bytes);
    seed.update([mutation_index(mutation)]);
    seed.update(index.to_le_bytes());
    let digest: [u8; 32] = seed.finalize().into();
    let category_bias = mutation_index(mutation) as f32 * 0.0007;
    let mut builder = PanelBuilder::new();
    for slot in InstrumentSlot::all() {
        let mut values = base.slot(slot).to_vec();
        for (idx, value) in values.iter_mut().enumerate() {
            let byte = digest[(idx + slot.offset()) % digest.len()] as f32;
            let centered = (byte / 255.0) - 0.5;
            let wave = (((idx + index + slot.offset()) % 23) as f32 - 11.0) * 0.00015;
            *value += centered * 0.018 + category_bias + wave;
        }
        builder.set_slot(slot, &values)?;
    }
    builder.require_slots(&InstrumentSlot::all())?;
    Ok(builder.build()?)
}

fn summarize_shrinkage(decisions: &[ShrinkageDecision]) -> RefreshShrinkageSummary {
    let mut summary = RefreshShrinkageSummary {
        total_cells: decisions.len(),
        own_cell: 0,
        language_aggregate: 0,
        entity_aggregate: 0,
        category_aggregate: 0,
    };
    for decision in decisions {
        match decision.origin {
            ShrinkageOrigin::OwnCell => summary.own_cell += 1,
            ShrinkageOrigin::LanguageAggregate => summary.language_aggregate += 1,
            ShrinkageOrigin::EntityAggregate => summary.entity_aggregate += 1,
            ShrinkageOrigin::CategoryAggregate => summary.category_aggregate += 1,
        }
    }
    summary
}

fn read_real_phase1_panel(path: &Path) -> FsvResult<Panel> {
    if !path.is_file() {
        return Err(format!("required Phase 1 panel missing at {}", path.display()).into());
    }
    let text = fs::read_to_string(path)?;
    let panel: Panel = serde_json::from_str(&text)?;
    for slot in InstrumentSlot::all() {
        require(&format!("slot {} filled", slot.slug()), panel.is_filled(slot))?;
    }
    Ok(panel)
}

fn embedder_versions_from_panel(panel: &Panel) -> BTreeMap<EmbedderId, [u8; 32]> {
    EmbedderId::all()
        .into_iter()
        .map(|embedder| {
            let slot = panel_slot_for_embedder(embedder);
            (embedder, sha256_array_from_f32(panel.slot(slot)))
        })
        .collect()
}

fn negate_slot(panel: &Panel, slot: InstrumentSlot) -> Result<Panel, TctError> {
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

fn mutation_from_slug(value: &str) -> FsvResult<MutationCategory> {
    match value {
        "known_good" => Ok(MutationCategory::KnownGood),
        "subtle_flip" => Ok(MutationCategory::SubtleFlip),
        "off_by_one" => Ok(MutationCategory::OffByOne),
        "swap_variable" => Ok(MutationCategory::SwapVariable),
        "delete_test_call" => Ok(MutationCategory::DeleteTestCall),
        "wrong_file" => Ok(MutationCategory::WrongFile),
        "over_engineer" => Ok(MutationCategory::OverEngineer),
        "compile_error" => Ok(MutationCategory::CompileError),
        _ => Err(format!("unknown mutation category slug {value:?}").into()),
    }
}

fn mutation_index(value: MutationCategory) -> u8 {
    match value {
        MutationCategory::KnownGood => 0,
        MutationCategory::SubtleFlip => 1,
        MutationCategory::OffByOne => 2,
        MutationCategory::SwapVariable => 3,
        MutationCategory::DeleteTestCall => 4,
        MutationCategory::WrongFile => 5,
        MutationCategory::OverEngineer => 6,
        MutationCategory::CompileError => 7,
    }
}

fn corpus_sha_from_index(index_bytes: &[u8]) -> FsvResult<[u8; 32]> {
    let index: Value = serde_json::from_slice(index_bytes)?;
    let digest = index["corpus_sha256"]
        .as_str()
        .ok_or("corpus_sha256 must be present in corpus index")?;
    hex_digest_32(digest)
}

fn hex_digest_32(value: &str) -> FsvResult<[u8; 32]> {
    let hex_value = value.strip_prefix("sha256:").unwrap_or(value);
    let bytes = hex::decode(hex_value)?;
    require("digest length 32", bytes.len() == 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parent_chain_hash(value: &str) -> [u8; 16] {
    let digest = Sha256::digest(value.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn sha256_array(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_array_from_f32(values: &[f32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

fn git_head() -> FsvResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --verify HEAD failed".into());
    }
    let sha = String::from_utf8(output.stdout)?.trim().to_string();
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("git HEAD was not a 40-char hex SHA: {sha:?}").into());
    }
    Ok(sha.to_ascii_lowercase())
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
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
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    require(&format!("{} mode 0600", path.display()), mode == 0o600)?;
    let readback_bytes = fs::read(path)?;
    let readback: T = serde_json::from_slice(&readback_bytes)?;
    require(
        &format!("{} serde readback equality", path.display()),
        readback == *value,
    )?;
    Ok(())
}

fn count_files(path: &Path) -> FsvResult<usize> {
    let mut count = 0usize;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_file() {
            count += 1;
        } else if file_type.is_dir() {
            count += count_files(&entry.path())?;
        }
    }
    Ok(count)
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("TCT centroid refresh FSV assertion failed: {label}").into())
    }
}

fn _unix_secs(value: SystemTime) -> u64 {
    value
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
