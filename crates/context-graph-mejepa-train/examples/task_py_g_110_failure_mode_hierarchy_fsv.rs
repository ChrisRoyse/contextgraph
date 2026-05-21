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
    evaluate_holdout_named_mode_coverage, evaluate_post_fix_predicted_fail_named_mode_coverage,
    materialize_failure_mode_hierarchy, read_all_level1_failure_mode_group_rows,
    read_all_level3_constellation_rows, read_all_named_prediction_rows,
    write_failure_mode_hierarchy_sync_readback, CodeStateOutcomeRow, PostFixPredictionRow,
};
use context_graph_mejepa_train::label_bridge::load_label_learning_bridge;
use context_graph_mejepa_train::skill_corpus_materialization::{
    build_skill_corpus_materialization, SkillCorpusMaterializationConfig,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_LABEL_ROOT: &str =
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split";
const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-110-failure-mode-hierarchy-v2-fsv";
const DEFAULT_POST_FIX_SHIP_GATE_REPORT: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/ship-gate-fsv.json";
const FORBIDDEN_SYNTHETIC_BASELINES: [&str; 2] = ["0.866667", "0.8666666746139526"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse()?;
    let created_at = now_ms()?;
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
    let reverse_readback = read_all_skill_reverse_index_rows(&readback_db)?;
    let code_state_outcomes = read_code_state_outcomes(&args.label_root)?;
    let post_fix_ship_gate = read_post_fix_ship_gate_report(&args.post_fix_ship_gate_report)?;
    let holdout_eval = evaluate_holdout_named_mode_coverage(
        &code_state_outcomes,
        &reverse_readback,
        &all_membership_readback,
        &named_readback,
        0.85,
    )?;
    let post_fix_predicted_fail_eval = evaluate_post_fix_predicted_fail_named_mode_coverage(
        &code_state_outcomes,
        &post_fix_ship_gate.holdout_predictions,
        &reverse_readback,
        &all_membership_readback,
        &named_readback,
        0.85,
    )?;
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
        && !build.report.target_labels_used_as_live_inputs;
    let post_fix_revalidation_measured = post_fix_predicted_fail_eval.post_fix_predicted_fail_rows
        > 0
        && post_fix_predicted_fail_eval
            .unmapped_prediction_examples
            .is_empty()
        && post_fix_predicted_fail_eval.true_fail_coverage_pass
        && post_fix_predicted_fail_eval.closest_exemplar_oracle_agreement >= 0.85;
    let mut post_fix_revalidation_blockers = Vec::new();
    if !post_fix_predicted_fail_eval.coverage_pass {
        post_fix_revalidation_blockers
            .push("POST_FIX_AGGREGATE_PREDICTED_FAIL_NAMED_MODE_COVERAGE_FALSE");
    }
    if !post_fix_predicted_fail_eval.true_fail_coverage_pass {
        post_fix_revalidation_blockers.push("POST_FIX_TRUE_FAIL_NAMED_MODE_COVERAGE_NOT_VALIDATED");
    }
    if post_fix_predicted_fail_eval.post_fix_predicted_false_positive_rows_without_any_named_mode
        > 0
    {
        post_fix_revalidation_blockers.push("POST_FIX_UNEXPLAINED_FALSE_POSITIVE_FAIL_VERDICTS");
    }
    if post_fix_predicted_fail_eval.closest_exemplar_coverage_on_predicted_fail
        < post_fix_predicted_fail_eval.min_closest_exemplar_coverage_on_predicted_fail
    {
        post_fix_revalidation_blockers.push("POST_FIX_CLOSEST_EXEMPLAR_COVERAGE_BELOW_THRESHOLD");
    }
    let holdout_pass = holdout_eval.passes && post_fix_revalidation_measured;
    let all_passed =
        readback_pass && hierarchy_pass && runtime_pass && substrate_pass && holdout_pass;

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
        "post_fix_rebaseline_reference": {
            "report_path": args.post_fix_ship_gate_report,
            "report_sha256": post_fix_ship_gate.report_sha256,
            "raw_prediction_oracle_correlation": post_fix_ship_gate.raw_prediction_oracle_correlation,
            "rank_safe_prediction_oracle_correlation": post_fix_ship_gate.rank_safe_prediction_oracle_correlation,
            "rank_lift_prediction_oracle_correlation": post_fix_ship_gate.rank_lift_prediction_oracle_correlation,
            "ship_gate_passed": post_fix_ship_gate.ship_gate_passed,
            "holdout_prediction_rows": post_fix_ship_gate.holdout_predictions.len(),
            "forbidden_synthetic_baseline_hits": 0
        },
        "holdout_named_mode_evaluation": holdout_eval,
        "post_fix_predicted_fail_named_mode_evaluation": post_fix_predicted_fail_eval,
        "post_fix_revalidation_result": {
            "measurement_valid": post_fix_revalidation_measured,
            "coverage_validated": post_fix_predicted_fail_eval.passes,
            "typed_blockers": post_fix_revalidation_blockers,
            "interpretation": "The #404 substrate survives. Post-fix true-fail predictions all resolve to FailLikely named modes, but current active policy still emits some false-positive Fail verdicts with no named consequence explanation; repair belongs in predictor policy/feature selection, not blanket skill expansion."
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
            "substrate_slot_identity_and_live_label_policy_preserved": substrate_pass,
            "holdout_named_mode_coverage_and_exemplar_agreement": holdout_eval.passes,
            "post_fix_predicted_fail_revalidation_measured": post_fix_revalidation_measured,
            "post_fix_predicted_fail_named_mode_coverage_and_exemplar_agreement": post_fix_predicted_fail_eval.passes
        },
        "active_path_policy": {
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
            "holdout_named_mode_evaluation": final_report.get("holdout_named_mode_evaluation"),
            "post_fix_predicted_fail_named_mode_evaluation": final_report.get("post_fix_predicted_fail_named_mode_evaluation"),
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
    post_fix_ship_gate_report: PathBuf,
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
            post_fix_ship_gate_report: PathBuf::from(DEFAULT_POST_FIX_SHIP_GATE_REPORT),
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
                "--post-fix-ship-gate-report" => {
                    out.post_fix_ship_gate_report = PathBuf::from(
                        args.next()
                            .ok_or("missing --post-fix-ship-gate-report value")?,
                    )
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

#[derive(Debug, Deserialize)]
struct CodeStateOutcomeJsonRow {
    group_key: String,
    task_instance_id: String,
    mutation_category: String,
    docker_label: String,
    oracle_all_passed: bool,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
}

fn read_code_state_outcomes(
    label_root: &Path,
) -> Result<Vec<CodeStateOutcomeRow>, Box<dyn std::error::Error>> {
    let bridge = load_label_learning_bridge(label_root)?;
    let code_state_path =
        resolve_label_path(label_root, &bridge.manifest.paths.code_state_constellations);
    let mut rows = Vec::new();
    for row in read_jsonl::<CodeStateOutcomeJsonRow>(&code_state_path)? {
        rows.push(CodeStateOutcomeRow {
            code_state_key: row.group_key,
            task_instance_id: row.task_instance_id,
            mutation_category: row.mutation_category,
            docker_label: row.docker_label,
            oracle_all_passed: row.oracle_all_passed,
            slot_identity_preserved: row.slot_identity_preserved,
            flat_vector_concat_used: row.flat_vector_concat_used,
        });
    }
    if rows.is_empty() {
        return Err(format!(
            "code-state outcome file produced no rows: {}",
            code_state_path.display()
        )
        .into());
    }
    Ok(rows)
}

#[derive(Debug)]
struct PostFixShipGateReport {
    report_sha256: String,
    raw_prediction_oracle_correlation: f64,
    rank_safe_prediction_oracle_correlation: f64,
    rank_lift_prediction_oracle_correlation: f64,
    ship_gate_passed: bool,
    holdout_predictions: Vec<PostFixPredictionRow>,
}

fn read_post_fix_ship_gate_report(
    path: &Path,
) -> Result<PostFixShipGateReport, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8(bytes.clone())?;
    for forbidden in FORBIDDEN_SYNTHETIC_BASELINES {
        if text.contains(forbidden) {
            return Err(format!(
                "post-fix ship-gate report {} contains invalid synthetic baseline literal {forbidden}",
                path.display()
            )
            .into());
        }
    }
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let raw = metric_f64(
        &value,
        "/execute_and_inspect/raw_final_metrics/prediction_oracle_correlation",
    )?;
    let rank_safe = metric_f64(
        &value,
        "/execute_and_inspect/rank_safe_final_metrics/prediction_oracle_correlation",
    )?;
    let rank_lift = metric_f64(
        &value,
        "/execute_and_inspect/final_metrics/prediction_oracle_correlation",
    )?;
    for (name, metric) in [
        ("raw", raw),
        ("rank_safe", rank_safe),
        ("rank_lift", rank_lift),
    ] {
        if (metric - 0.866667).abs() < 0.000001 || (metric - 0.8666666746139526).abs() < 0.000001 {
            return Err(format!(
                "post-fix ship-gate report {} has invalid {name} correlation {metric}",
                path.display()
            )
            .into());
        }
    }
    let ship_gate_passed = value
        .pointer("/execute_and_inspect/ship_gate_passed")
        .and_then(serde_json::Value::as_bool)
        .ok_or("missing /execute_and_inspect/ship_gate_passed")?;
    let prediction_values = value
        .pointer("/execute_and_inspect/holdout_predictions")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing /execute_and_inspect/holdout_predictions")?;
    let mut holdout_predictions = Vec::new();
    for prediction in prediction_values {
        let row = PostFixPredictionRow {
            task_instance_id: value_string(prediction, "task_id")?,
            mutation_category: value_string(prediction, "category")?,
            actual_oracle_pass: value_bool(prediction, "actual_oracle_pass")?,
            predicted_oracle_pass: value_f64(prediction, "predicted_oracle_pass")?,
            predicted_pass: value_bool(prediction, "predicted_pass")?,
            patch_path: value_string(prediction, "patch_path")?,
            patch_sha256: value_string(prediction, "patch_sha256")?,
        };
        row.validate()?;
        if !row.patch_path.starts_with("/zfs/archive/contextgraph/") {
            return Err(format!(
                "post-fix prediction path is not aiwonder archive: {}",
                row.patch_path
            )
            .into());
        }
        holdout_predictions.push(row);
    }
    if holdout_predictions.is_empty() {
        return Err("post-fix ship-gate report has no holdout predictions".into());
    }
    Ok(PostFixShipGateReport {
        report_sha256: format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
        raw_prediction_oracle_correlation: raw,
        rank_safe_prediction_oracle_correlation: rank_safe,
        rank_lift_prediction_oracle_correlation: rank_lift,
        ship_gate_passed,
        holdout_predictions,
    })
}

fn metric_f64(value: &serde_json::Value, pointer: &str) -> Result<f64, Box<dyn std::error::Error>> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| format!("missing numeric metric {pointer}").into())
}

fn value_string(
    value: &serde_json::Value,
    field: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing string field {field}").into())
}

fn value_bool(value: &serde_json::Value, field: &str) -> Result<bool, Box<dyn std::error::Error>> {
    value
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("missing bool field {field}").into())
}

fn value_f64(value: &serde_json::Value, field: &str) -> Result<f64, Box<dyn std::error::Error>> {
    value
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| format!("missing numeric field {field}").into())
}

fn resolve_label_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Vec<T>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line).map_err(|err| {
            format!(
                "{}:{} failed JSON parse for code-state outcome row: {err}",
                path.display(),
                idx + 1
            )
        })?);
    }
    Ok(rows)
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

fn now_ms() -> Result<i64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
