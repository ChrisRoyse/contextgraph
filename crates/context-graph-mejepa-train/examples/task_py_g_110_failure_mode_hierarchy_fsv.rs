use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL1,
    CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS,
    CF_MEJEPA_NAMED_PREDICTIONS, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::ability_resolver::{
    prediction_label_context_from_ability_context, resolve_ability_context, AbilityResolverRequest,
    LiveChunkInput,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, read_all_chunk_skill_membership_rows, read_all_level2_skill_rows,
    read_all_skill_reverse_index_rows, write_skill_materialization_sync_readback,
};
use context_graph_mejepa_train::failure_mode_hierarchy::{
    materialize_failure_mode_hierarchy, read_all_level1_failure_mode_group_rows,
    read_all_level3_constellation_rows, read_all_named_prediction_rows,
    write_failure_mode_hierarchy_sync_readback,
};
use context_graph_mejepa_train::label_bridge::load_label_learning_bridge;
use context_graph_mejepa_train::skill_corpus_materialization::{
    build_skill_corpus_materialization, SkillCorpusMaterializationConfig,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_LABEL_ROOT: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split";
const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse()?;
    let created_at = now_ms()?;
    if contains_mntd(&args.fsv_root) || contains_mntd(&args.label_root) {
        return Err("TASK-PY-G-110 hierarchy FSV paths must not use /mnt/d".into());
    }
    fs::create_dir_all(&args.fsv_root)?;
    let run_root = args.fsv_root.join(format!("run-{created_at}"));
    fs::create_dir_all(&run_root)?;
    let before_tree = tree_state(&run_root)?;

    let mut config = SkillCorpusMaterializationConfig {
        max_chunk_rows: args.max_chunk_rows,
        max_materialized_skills: args.max_materialized_skills,
        ..Default::default()
    };
    config.discovery.min_support = args.min_support;
    config.discovery.min_lift_over_cell_baseline = args.min_lift;
    config.discovery.min_confidence = args.min_confidence;

    let build = build_skill_corpus_materialization(&args.label_root, config, created_at)?;
    let db_path = run_root.join("failure-mode-hierarchy.rocksdb");
    let db = open_db(&db_path)?;
    let before_counts = cf_counts(&db)?;
    write_skill_materialization_sync_readback(&db, &build.materialization)?;

    let level2_rows = read_all_level2_skill_rows(&db)?;
    let level2_memberships = read_all_chunk_skill_membership_rows(&db)?;
    let reverse_rows = read_all_skill_reverse_index_rows(&db)?;
    let hierarchy = materialize_failure_mode_hierarchy(
        &level2_rows,
        &level2_memberships,
        &reverse_rows,
        created_at,
    )?;
    write_failure_mode_hierarchy_sync_readback(&db, &hierarchy)?;
    let after_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_db(&db_path)?;
    let reopened_counts = cf_counts(&readback_db)?;
    let level1_readback = read_all_level1_failure_mode_group_rows(&readback_db)?;
    let level3_readback = read_all_level3_constellation_rows(&readback_db)?;
    let named_readback = read_all_named_prediction_rows(&readback_db)?;
    let all_membership_readback = read_all_chunk_skill_membership_rows(&readback_db)?;
    let sample_level3_membership = hierarchy
        .level3_memberships
        .first()
        .ok_or("missing Level-3 membership")?;
    let bridge = load_label_learning_bridge(&args.label_root)?;
    let ability_context = resolve_ability_context(
        &readback_db,
        AbilityResolverRequest {
            code_state_key: sample_level3_membership.code_state_key.clone(),
            chunks: vec![LiveChunkInput {
                chunk_id: sample_level3_membership.chunk_id.clone(),
                file_path: Some(sample_level3_membership.file_path.clone()),
                accepted_label_ids: sample_level3_membership.source_accepted_label_ids.clone(),
            }],
        },
    )?;
    let prediction_label_context =
        prediction_label_context_from_ability_context(&bridge, &ability_context, Vec::new())?;
    prediction_label_context.validate()?;

    let expected_named_count =
        build.materialization.level2_skills.len() + hierarchy.level3_constellations.len();
    let expected_membership_count =
        build.materialization.chunk_memberships.len() + hierarchy.level3_memberships.len();
    let readback_pass = reopened_counts == after_counts
        && level1_readback.len() == hierarchy.level1_groups.len()
        && level3_readback.len() == hierarchy.level3_constellations.len()
        && named_readback.len() == expected_named_count
        && all_membership_readback.len() == expected_membership_count;
    let hierarchy_pass = !hierarchy.level1_groups.is_empty()
        && !hierarchy.level3_constellations.is_empty()
        && !hierarchy.level3_memberships.is_empty()
        && !hierarchy.named_predictions.is_empty()
        && hierarchy
            .named_predictions
            .iter()
            .all(|row| row.live_input_allowed && !row.target_labels_used_as_live_inputs)
        && hierarchy
            .level3_constellations
            .iter()
            .all(|row| row.live_input_allowed && row.target_outcomes_used_for_calibration);
    let runtime_pass = ability_context
        .active_higher_ability_ids
        .contains(&sample_level3_membership.skill_id)
        && !ability_context.active_skill_ids.is_empty()
        && !ability_context.flat_vector_concat_used
        && !ability_context.target_side_labels_used_as_live_inputs
        && prediction_label_context
            .active_higher_ability_ids
            .contains(&sample_level3_membership.skill_id)
        && prediction_label_context
            .source_membership_keys
            .contains(&sample_level3_membership.membership_key);
    let substrate_pass = build.report.chunk_rows_streamed > 0
        && build.report.chunk_rows_with_live_skill_labels == build.report.chunk_rows_streamed
        && build.report.slot_identity_preserved
        && !build.report.flat_vector_concat_used
        && !build.report.target_labels_used_as_live_inputs
        && !build.report.mnt_d_active_reads_or_writes;
    let all_passed = readback_pass && hierarchy_pass && runtime_pass && substrate_pass;

    let report_path = run_root.join("task_py_g_110_failure_mode_hierarchy_fsv.json");
    let report = json!({
        "artifact_kind": "task_py_g_110_failure_mode_hierarchy_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-110",
        "passes": all_passed,
        "run_root": run_root,
        "label_root": args.label_root,
        "db_path": db_path,
        "before": {"tree": before_tree, "cf_counts": before_counts},
        "after": {"tree": tree_state(&run_root)?, "cf_counts": after_counts},
        "readback": {
            "cf_counts": reopened_counts,
            "level1Rows": level1_readback.len(),
            "level3Rows": level3_readback.len(),
            "namedPredictionRows": named_readback.len(),
            "allMembershipRows": all_membership_readback.len()
        },
        "production_materialization": build.report,
        "hierarchy_materialization": {
            "level1GroupCount": hierarchy.level1_groups.len(),
            "level2SkillCount": build.materialization.level2_skills.len(),
            "level3ConstellationCount": hierarchy.level3_constellations.len(),
            "level3MembershipCount": hierarchy.level3_memberships.len(),
            "namedPredictionCount": hierarchy.named_predictions.len(),
            "expectedNamedPredictionCount": expected_named_count,
            "expectedMembershipCount": expected_membership_count
        },
        "runtime_resolver": {
            "sampleAbilityId": sample_level3_membership.skill_id,
            "sampleMembershipKey": sample_level3_membership.membership_key,
            "activeSkillIds": ability_context.active_skill_ids,
            "activeHigherAbilityIds": ability_context.active_higher_ability_ids,
            "sourceMembershipKeys": ability_context.source_membership_keys,
            "predictionLabelContext": prediction_label_context
        },
        "checks": {
            "drop_reopen_readback_rows_agree": readback_pass,
            "hierarchy_rows_materialized": hierarchy_pass,
            "runtime_resolver_consumes_level3_membership": runtime_pass,
            "substrate_slot_identity_and_live_label_policy_preserved": substrate_pass
        },
        "active_path_policy": {
            "mnt_d_active_reads_or_writes": false,
            "aiwonder_label_root": true,
            "flat_vector_concat_used": false,
            "target_labels_used_as_live_inputs": false
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let final_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if final_report
        .get("passes")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err(format!(
            "TASK-PY-G-110 hierarchy FSV failed at {}",
            report_path.display()
        )
        .into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "passes": true,
            "report_path": report_path,
            "report_sha256": format!("sha256:{}", hex::encode(Sha256::digest(fs::read(&report_path)?))),
            "cf_counts": final_report.pointer("/after/cf_counts"),
            "hierarchy_materialization": final_report.get("hierarchy_materialization"),
            "runtime_resolver": {
                "sampleAbilityId": final_report.pointer("/runtime_resolver/sampleAbilityId"),
                "activeHigherAbilityIds": final_report.pointer("/runtime_resolver/activeHigherAbilityIds")
            }
        }))?
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct Args {
    label_root: PathBuf,
    fsv_root: PathBuf,
    max_chunk_rows: Option<usize>,
    max_materialized_skills: usize,
    min_support: u64,
    min_lift: f64,
    min_confidence: f64,
}

impl Args {
    fn parse() -> Result<Self, Box<dyn std::error::Error>> {
        let mut args = env::args().skip(1);
        let mut out = Self {
            label_root: PathBuf::from(DEFAULT_LABEL_ROOT),
            fsv_root: PathBuf::from(DEFAULT_FSV_ROOT),
            max_chunk_rows: None,
            max_materialized_skills: 128,
            min_support: 8,
            min_lift: 0.05,
            min_confidence: 0.55,
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--label-root" => {
                    out.label_root = PathBuf::from(args.next().ok_or("missing --label-root value")?)
                }
                "--fsv-root" => {
                    out.fsv_root = PathBuf::from(args.next().ok_or("missing --fsv-root value")?)
                }
                "--max-chunk-rows" => {
                    out.max_chunk_rows = Some(
                        args.next()
                            .ok_or("missing --max-chunk-rows value")?
                            .parse()?,
                    )
                }
                "--max-materialized-skills" => {
                    out.max_materialized_skills = args
                        .next()
                        .ok_or("missing --max-materialized-skills value")?
                        .parse()?
                }
                "--min-support" => {
                    out.min_support = args.next().ok_or("missing --min-support value")?.parse()?
                }
                "--min-lift" => {
                    out.min_lift = args.next().ok_or("missing --min-lift value")?.parse()?
                }
                "--min-confidence" => {
                    out.min_confidence = args
                        .next()
                        .ok_or("missing --min-confidence value")?
                        .parse()?
                }
                _ => return Err(format!("unknown argument {arg}").into()),
            }
        }
        Ok(out)
    }
}

fn open_db(path: &Path) -> Result<DB, Box<dyn std::error::Error>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = [
        CF_MEJEPA_FAILURE_MODE_LEVEL1,
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
        CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS,
        CF_MEJEPA_NAMED_PREDICTIONS,
        CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
        CF_MEJEPA_SKILL_REVERSE_INDEX,
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
    ]
    .into_iter()
    .map(|name| ColumnFamilyDescriptor::new(name, Options::default()))
    .collect::<Vec<_>>();
    Ok(DB::open_cf_descriptors(&opts, path, descriptors)?)
}

fn cf_counts(db: &DB) -> Result<BTreeMap<String, u64>, Box<dyn std::error::Error>> {
    Ok(BTreeMap::from([
        (
            "level1Groups".to_string(),
            count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL1)?,
        ),
        (
            "level2Skills".to_string(),
            count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)?,
        ),
        (
            "level3Constellations".to_string(),
            count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS)?,
        ),
        (
            "namedPredictions".to_string(),
            count_cf_rows(db, CF_MEJEPA_NAMED_PREDICTIONS)?,
        ),
        (
            "chunkSkillMembership".to_string(),
            count_cf_rows(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)?,
        ),
        (
            "skillReverseIndex".to_string(),
            count_cf_rows(db, CF_MEJEPA_SKILL_REVERSE_INDEX)?,
        ),
        (
            "skillLifecycleAudit".to_string(),
            count_cf_rows(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?,
        ),
    ]))
}

fn tree_state(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut entries = Vec::new();
    if path.exists() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "is_dir": metadata.is_dir(),
                "len": metadata.len()
            }));
        }
    }
    entries.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    Ok(json!({"path": path, "entries": entries}))
}

fn contains_mntd(path: &Path) -> bool {
    let value = path.to_string_lossy();
    value == "/mnt/d" || value.starts_with("/mnt/d/")
}

fn now_ms() -> Result<i64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
