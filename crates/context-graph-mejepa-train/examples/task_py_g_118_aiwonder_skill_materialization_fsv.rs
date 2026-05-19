use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, read_chunk_skill_membership_row, read_level2_skill_row,
    read_skill_lifecycle_audit_row, read_skill_reverse_index_row,
    write_skill_materialization_sync_readback,
};
use context_graph_mejepa_train::skill_corpus_materialization::{
    build_skill_corpus_materialization, SkillCorpusMaterializationConfig,
};
use context_graph_mejepa_train::skill_sequence_discovery::{
    discover_skill_candidates, SkillDiscoveryConfig, SkillEpisodeRow, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillStepEvidence,
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
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1";
const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-118-skill-sequence-discovery-fsv";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse()?;
    let created_at = now_ms()?;
    if args.fsv_root.to_string_lossy().contains("/mnt/d")
        || args.label_root.to_string_lossy().contains("/mnt/d")
    {
        return Err("active TASK-PY-G-118 FSV paths must not use /mnt/d".into());
    }
    fs::create_dir_all(&args.fsv_root)?;
    let run_root = args.fsv_root.join(format!("run-{created_at}"));
    fs::create_dir_all(&run_root)?;
    let before_tree = tree_state(&run_root)?;

    let mut config = SkillCorpusMaterializationConfig::default();
    config.max_chunk_rows = args.max_chunk_rows;
    config.max_materialized_skills = args.max_materialized_skills;
    config.discovery.min_support = args.min_support;
    config.discovery.min_lift_over_cell_baseline = args.min_lift;
    config.discovery.min_confidence = args.min_confidence;

    let build = build_skill_corpus_materialization(&args.label_root, config.clone(), created_at)?;
    let manual_review_path = run_root.join("manual_review_examples.json");
    fs::write(
        &manual_review_path,
        serde_json::to_vec_pretty(&build.report.sample_episodes)?,
    )?;

    let db_path = run_root.join("skill-corpus.rocksdb");
    let db = open_db(&db_path)?;
    let before_counts = cf_counts(&db)?;
    write_skill_materialization_sync_readback(&db, &build.materialization)?;
    let after_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_db(&db_path)?;
    let reopened_counts = cf_counts(&readback_db)?;
    let sample_skill = build
        .materialization
        .level2_skills
        .first()
        .ok_or("missing materialized skill")?;
    let sample_membership = build
        .materialization
        .chunk_memberships
        .first()
        .ok_or("missing materialized chunk membership")?;
    let sample_reverse = build
        .materialization
        .reverse_indexes
        .first()
        .ok_or("missing materialized reverse index")?;
    let sample_audit = build
        .materialization
        .lifecycle_audits
        .first()
        .ok_or("missing materialized lifecycle audit")?;
    let skill_readback = read_level2_skill_row(&readback_db, &sample_skill.skill_id)?
        .ok_or("missing level2 skill readback")?;
    let membership_readback =
        read_chunk_skill_membership_row(&readback_db, &sample_membership.membership_key)?
            .ok_or("missing membership readback")?;
    let reverse_readback =
        read_skill_reverse_index_row(&readback_db, &sample_reverse.reverse_index_key)?
            .ok_or("missing reverse index readback")?;
    let audit_readback =
        read_skill_lifecycle_audit_row(&readback_db, &sample_audit.skill_audit_id)?
            .ok_or("missing lifecycle audit readback")?;
    let readback_matches = skill_readback == *sample_skill
        && membership_readback == *sample_membership
        && reverse_readback == *sample_reverse
        && audit_readback == *sample_audit;

    let edge_target_label_rejected = target_label_edge_rejected();
    let edge_too_broad_rejected = too_broad_pattern_edge_rejected()?;
    let edge_mnt_d_rejected =
        build_skill_corpus_materialization("/mnt/d/contextgraph/not-active", config, created_at)
            .is_err();

    let expected_counts_match = reopened_counts == after_counts
        && after_counts["level2Skills"] == build.report.materialized_skill_count as u64
        && after_counts["chunkSkillMembership"]
            == build.report.materialized_membership_count as u64
        && after_counts["skillReverseIndex"]
            == build.report.materialized_reverse_index_count as u64
        && after_counts["skillLifecycleAudit"]
            == build.report.materialized_lifecycle_audit_count as u64;
    let all_passed = build.report.materialized_skill_count > 0
        && build.report.materialized_membership_count > 0
        && build.report.chunk_rows_streamed > 0
        && build.report.slot_identity_preserved
        && !build.report.flat_vector_concat_used
        && !build.report.target_labels_used_as_live_inputs
        && !build.report.mnt_d_active_reads_or_writes
        && expected_counts_match
        && readback_matches
        && edge_target_label_rejected
        && edge_too_broad_rejected
        && edge_mnt_d_rejected;

    let report_path = run_root.join("task_py_g_118_aiwonder_skill_materialization_fsv.json");
    let report = json!({
        "artifact_kind": "task_py_g_118_aiwonder_skill_materialization_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-118",
        "passes": all_passed,
        "run_root": run_root,
        "label_root": args.label_root,
        "db_path": db_path,
        "before": {
            "tree": before_tree,
            "cf_counts": before_counts
        },
        "after": {
            "tree": tree_state(&run_root)?,
            "cf_counts": after_counts,
            "reopened_cf_counts": reopened_counts
        },
        "production_materialization": build.report,
        "manual_review_artifact": {
            "path": manual_review_path,
            "sha256": format!("sha256:{}", hex::encode(Sha256::digest(fs::read(&manual_review_path)?)))
        },
        "sample_readback": {
            "level2Skill": skill_readback,
            "chunkSkillMembership": membership_readback,
            "skillReverseIndex": reverse_readback,
            "skillLifecycleAudit": audit_readback
        },
        "edge_cases": {
            "target_oracle_label_rejected_as_live_skill_input": edge_target_label_rejected,
            "too_broad_high_support_static_pattern_rejected": edge_too_broad_rejected,
            "mnt_d_active_path_rejected": edge_mnt_d_rejected
        },
        "active_path_policy": {
            "mnt_d_active_reads_or_writes": false,
            "aiwonder_label_root": true,
            "target_outcomes_supervise_only": true,
            "flat_vector_concat_used": false
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let final_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if final_report
        .get("passes")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
        || !all_passed
    {
        return Err(format!(
            "TASK-PY-G-118 aiwonder FSV failed at {}",
            report_path.display()
        )
        .into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "passes": true,
            "report_path": report_path,
            "run_root": final_report.get("run_root"),
            "cf_counts": final_report.get("after").and_then(|after| after.get("cf_counts")),
            "production_materialization": {
                "chunkRowsStreamed": final_report.pointer("/production_materialization/chunkRowsStreamed"),
                "generatedEpisodeCount": final_report.pointer("/production_materialization/generatedEpisodeCount"),
                "materializedSkillCount": final_report.pointer("/production_materialization/materializedSkillCount"),
                "materializedMembershipCount": final_report.pointer("/production_materialization/materializedMembershipCount"),
                "materializedReverseIndexCount": final_report.pointer("/production_materialization/materializedReverseIndexCount"),
                "materializedLifecycleAuditCount": final_report.pointer("/production_materialization/materializedLifecycleAuditCount"),
            },
            "edge_cases": final_report.get("edge_cases")
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

fn target_label_edge_rejected() -> bool {
    let episode = SkillEpisodeRow {
        episode_id: "episode:edge:target_label".to_string(),
        proposed_skill_name: Some("target_label_edge".to_string()),
        ordered_steps: vec![SkillStepEvidence {
            step_index: 0,
            chunk_id: "chunk:edge".to_string(),
            file_path: "edge.py".to_string(),
            code_state_key: "state:edge".to_string(),
            accepted_label_ids: vec!["oracle:pass".to_string()],
            group_ids: vec!["group:edge".to_string()],
        }],
        outcome: None,
        failure_evidence_set_ids: Vec::new(),
        cell_baseline_fail_rate: 0.5,
    };
    discover_skill_candidates(&[episode], SkillDiscoveryConfig::default(), 1).is_err()
}

fn too_broad_pattern_edge_rejected() -> Result<bool, Box<dyn std::error::Error>> {
    let mut episodes = Vec::new();
    for idx in 0..4097_u32 {
        episodes.push(SkillEpisodeRow {
            episode_id: format!("episode:edge:broad:{idx}"),
            proposed_skill_name: Some("too_broad_static_pattern".to_string()),
            ordered_steps: vec![SkillStepEvidence {
                step_index: 0,
                chunk_id: format!("chunk:edge:{idx}"),
                file_path: format!("pkg/edge_{idx}.py"),
                code_state_key: format!("state:edge:{idx}"),
                accepted_label_ids: vec!["ast_surface:call_api_surface".to_string()],
                group_ids: vec!["group:semantic_content_panel:present".to_string()],
            }],
            outcome: Some(SkillOutcomeObservation {
                outcome_label_id: "docker:docker_resolved_clean".to_string(),
                verdict: SkillOutcomeVerdict::Pass,
                target_side_supervision_only: true,
            }),
            failure_evidence_set_ids: Vec::new(),
            cell_baseline_fail_rate: 0.1,
        });
    }
    let report = discover_skill_candidates(
        &episodes,
        SkillDiscoveryConfig {
            min_support: 2,
            min_lift_over_cell_baseline: 0.0,
            min_confidence: 0.5,
            allow_pending_outcome_candidates: true,
        },
        1,
    )?;
    Ok(report.candidates.is_empty()
        && report
            .rejections
            .iter()
            .any(|row| row.reason == "too_broad_high_support_static_pattern"))
}

fn open_db(path: &Path) -> Result<DB, Box<dyn std::error::Error>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let descriptors = [
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
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
            "level2Skills".to_string(),
            count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)?,
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
