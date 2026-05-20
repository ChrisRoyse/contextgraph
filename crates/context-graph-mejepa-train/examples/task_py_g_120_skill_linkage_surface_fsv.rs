use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, materialize_skill_memberships, write_skill_materialization_sync_readback,
};
use context_graph_mejepa_train::skill_linkage::{
    chunk_as_star, code_to_skill, constellation_membership, load_chunk_source_index_jsonl,
    open_skill_linkage_rocksdb, skill_browse, skill_conflict_graph, skill_coverage_audit,
    skill_graph_inspect, skill_impact, skill_set_query, skill_to_code, SkillLinkageOptions,
};
use context_graph_mejepa_train::skill_sequence_discovery::{
    Level2SkillRow, SkillEpisodeRow, SkillOutcomeDistribution, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillPromotionStatus, SkillStepEvidence, SkillStepTemplate,
    SkillTransitionEdge, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root();
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    let before_tree = tree_state(&fsv_root)?;

    let db_path = fsv_root.join("skill-linkage.rocksdb");
    let source_jsonl = fsv_root.join("ast_chunks_source.jsonl");
    write_source_jsonl(&source_jsonl)?;
    let _source_index = load_chunk_source_index_jsonl(&source_jsonl)?;
    let db = open_skill_linkage_rocksdb(&db_path, true)?;
    let before_counts = cf_counts(&db)?;

    let skill_a = skill(
        "skill:import_contract_drift:fsv",
        "import_contract_drift",
        vec!["episode:a:shared", "episode:a:other"],
        ["ast_surface:import_from", "pair_relation:provider_symbol"],
    );
    let episodes_a = vec![
        episode(
            "episode:a:shared",
            "chunk:shared",
            "pkg/shared.py",
            "state:shared",
            ["ast_surface:import_from", "pair_relation:provider_symbol"],
        ),
        episode(
            "episode:a:other",
            "chunk:a_only",
            "pkg/a.py",
            "state:other",
            ["ast_surface:import_from", "pair_relation:provider_symbol"],
        ),
    ];
    let materialized_a = materialize_skill_memberships(&skill_a, &episodes_a, 1_779_212_000_000)?;
    write_skill_materialization_sync_readback(&db, &materialized_a)?;

    let skill_b = skill(
        "skill:mock_only_change:fsv",
        "mock_only_change",
        vec!["episode:b:shared", "episode:b:other"],
        ["ast_surface:import_from", "pair_relation:mock_patch"],
    );
    let episodes_b = vec![
        episode(
            "episode:b:shared",
            "chunk:shared",
            "pkg/shared.py",
            "state:shared",
            ["ast_surface:import_from", "pair_relation:mock_patch"],
        ),
        episode(
            "episode:b:other",
            "chunk:b_only",
            "pkg/b.py",
            "state:other",
            ["ast_surface:import_from", "pair_relation:mock_patch"],
        ),
    ];
    let materialized_b = materialize_skill_memberships(&skill_b, &episodes_b, 1_779_212_000_100)?;
    write_skill_materialization_sync_readback(&db, &materialized_b)?;

    let skill_c = skill(
        "skill:isolated_state_transition:fsv",
        "isolated_state_transition",
        vec!["episode:c:other"],
        ["ast_surface:for_loop", "pair_relation:state_mutation"],
    );
    let episodes_c = vec![episode(
        "episode:c:other",
        "chunk:c_only",
        "pkg/c.py",
        "state:other",
        ["ast_surface:for_loop", "pair_relation:state_mutation"],
    )];
    let materialized_c = materialize_skill_memberships(&skill_c, &episodes_c, 1_779_212_000_200)?;
    write_skill_materialization_sync_readback(&db, &materialized_c)?;
    db.flush()?;
    let after_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_skill_linkage_rocksdb(&db_path, false)?;
    let source_index_readback = load_chunk_source_index_jsonl(&source_jsonl)?;
    let skill_to_code_report = skill_to_code(
        &readback_db,
        &skill_a.skill_id,
        Some(&source_index_readback),
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )?;
    let code_to_skill_report = code_to_skill(
        &readback_db,
        "chunk:shared",
        Some("state:shared"),
        Some(&source_index_readback),
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )?;
    let set_query_report = skill_set_query(
        &readback_db,
        &[skill_a.skill_id.clone(), skill_b.skill_id.clone()],
        &[],
        10,
    )?;
    let coverage_audit = skill_coverage_audit(
        &readback_db,
        &[
            "chunk:shared".to_string(),
            "chunk:shared:next".to_string(),
            "chunk:a_only".to_string(),
            "chunk:a_only:next".to_string(),
            "chunk:b_only".to_string(),
            "chunk:b_only:next".to_string(),
            "chunk:c_only".to_string(),
            "chunk:c_only:next".to_string(),
            "chunk:orphan".to_string(),
        ],
        10,
    )?;
    let constellation_membership_report =
        constellation_membership(&readback_db, "chunk:shared", Some("state:shared"), 10)?;
    let chunk_as_star_report = chunk_as_star(
        &readback_db,
        "chunk:shared",
        Some("state:shared"),
        Some(&source_index_readback),
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )?;
    let skill_impact_depth_one =
        skill_impact(&readback_db, "chunk:a_only", Some("state:other"), 1, 20)?;
    let skill_impact_depth_two =
        skill_impact(&readback_db, "chunk:a_only", Some("state:other"), 2, 20)?;
    let skill_graph_report = skill_graph_inspect(&readback_db, Some(&skill_a.skill_id), 20)?;
    let conflict_graph_report = skill_conflict_graph(&readback_db, 20)?;
    let skill_browse_report = skill_browse(&readback_db, None, 20)?;
    let skill_browse_filter_report = skill_browse(&readback_db, Some("mock"), 20)?;

    let edge_missing_source_fails_closed = skill_to_code(
        &readback_db,
        &skill_a.skill_id,
        None,
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )
    .is_err();
    let edge_overlap_set_query_rejected = skill_set_query(
        &readback_db,
        std::slice::from_ref(&skill_a.skill_id),
        std::slice::from_ref(&skill_a.skill_id),
        10,
    )
    .is_err();
    let edge_mntd_source_path_rejected =
        load_chunk_source_index_jsonl(Path::new("/zfs/archive/contextgraph/forbidden.jsonl"))
            .is_err();

    let skill_to_code_pass = skill_to_code_report.total_matching_memberships == 4
        && skill_to_code_report
            .chunks
            .iter()
            .any(|row| row.chunk_id == "chunk:shared" && row.source_text.is_some());
    let code_to_skill_pass = code_to_skill_report.total_matching_memberships == 2
        && code_to_skill_report
            .skills
            .iter()
            .any(|row| row.skill.skill_id == skill_a.skill_id)
        && code_to_skill_report
            .skills
            .iter()
            .any(|row| row.skill.skill_id == skill_b.skill_id);
    let set_query_pass =
        set_query_report.matching_chunk_ids == vec!["chunk:shared", "chunk:shared:next"];
    let coverage_pass = coverage_audit.total_chunk_universe
        == coverage_audit.chunks_with_membership + coverage_audit.chunks_without_membership
        && coverage_audit.zero_membership_chunk_ids == vec!["chunk:orphan"];
    let constellation_membership_pass = constellation_membership_report.memberships.len() == 2
        && constellation_membership_report
            .level2_skill_ids
            .contains(&skill_a.skill_id)
        && constellation_membership_report
            .level2_skill_ids
            .contains(&skill_b.skill_id)
        && constellation_membership_report
            .level1_group_ids
            .contains(&"group:sequence".to_string())
        && constellation_membership_report
            .live_level0_label_ids
            .contains(&"ast_surface:import_from".to_string());
    let chunk_as_star_pass = chunk_as_star_report
        .slot_parameter_cards
        .iter()
        .any(|card| card.slot_id == "e_ast")
        && chunk_as_star_report
            .active_skill_ids
            .contains(&skill_a.skill_id)
        && chunk_as_star_report
            .active_skill_ids
            .contains(&skill_b.skill_id)
        && chunk_as_star_report
            .source
            .as_ref()
            .and_then(|source| source.source_text.as_ref())
            .is_some()
        && !chunk_as_star_report.flat_vector_concat_used
        && !chunk_as_star_report.target_outcomes_used_as_live_inputs;
    let skill_impact_pass = skill_impact_depth_one
        .impacted_chunks
        .iter()
        .any(|row| row.chunk_id == "chunk:shared")
        && !skill_impact_depth_one
            .impacted_chunks
            .iter()
            .any(|row| row.chunk_id == "chunk:b_only")
        && skill_impact_depth_two
            .impacted_chunks
            .iter()
            .any(|row| row.chunk_id == "chunk:b_only")
        && skill_impact_depth_two.total_impacted_chunks
            >= skill_impact_depth_one.total_impacted_chunks;
    let skill_graph_pass = skill_graph_report.edges.iter().any(|edge| {
        edge.relation == "co_member_chunk"
            && edge.shared_chunk_ids.contains(&"chunk:shared".to_string())
            && ((edge.skill_a_id == skill_a.skill_id && edge.skill_b_id == skill_b.skill_id)
                || (edge.skill_a_id == skill_b.skill_id && edge.skill_b_id == skill_a.skill_id))
    });
    let conflict_graph_pass = conflict_graph_report.total_conflict_pairs >= 2
        && conflict_graph_report.conflict_pairs.iter().any(|pair| {
            (pair.skill_a_id == skill_a.skill_id && pair.skill_b_id == skill_c.skill_id)
                || (pair.skill_a_id == skill_c.skill_id && pair.skill_b_id == skill_a.skill_id)
        })
        && conflict_graph_report.conflict_pairs.iter().any(|pair| {
            (pair.skill_a_id == skill_b.skill_id && pair.skill_b_id == skill_c.skill_id)
                || (pair.skill_a_id == skill_c.skill_id && pair.skill_b_id == skill_b.skill_id)
        });
    let skill_browse_pass = skill_browse_report.total_catalog_skills == 3
        && skill_browse_report.total_matching_skills == 3
        && skill_browse_report.skills.iter().any(|row| {
            row.skill_id == skill_a.skill_id
                && row.membership_count == 4
                && row.distinct_chunk_count == 4
        })
        && skill_browse_filter_report.total_matching_skills == 1
        && skill_browse_filter_report
            .skills
            .iter()
            .any(|row| row.skill_id == skill_b.skill_id);
    let counts_pass = after_counts.get("level2Skills") == Some(&json!(3))
        && after_counts.get("chunkSkillMembership") == Some(&json!(10))
        && after_counts.get("skillReverseIndex") == Some(&json!(5))
        && after_counts.get("skillLifecycleAudit") == Some(&json!(3));
    let no_new_head_pass = skill_to_code_report.no_new_prediction_head_introduced
        && code_to_skill_report.no_new_prediction_head_introduced
        && set_query_report.no_new_prediction_head_introduced
        && coverage_audit.no_new_prediction_head_introduced
        && constellation_membership_report.no_new_prediction_head_introduced
        && chunk_as_star_report.no_new_prediction_head_introduced
        && skill_impact_depth_one.no_new_prediction_head_introduced
        && skill_impact_depth_two.no_new_prediction_head_introduced
        && skill_graph_report.no_new_prediction_head_introduced
        && conflict_graph_report.no_new_prediction_head_introduced
        && skill_browse_report.no_new_prediction_head_introduced
        && skill_browse_filter_report.no_new_prediction_head_introduced;
    let all_passed = skill_to_code_pass
        && code_to_skill_pass
        && set_query_pass
        && coverage_pass
        && constellation_membership_pass
        && chunk_as_star_pass
        && skill_impact_pass
        && skill_graph_pass
        && conflict_graph_pass
        && skill_browse_pass
        && counts_pass
        && no_new_head_pass
        && edge_missing_source_fails_closed
        && edge_overlap_set_query_rejected
        && edge_mntd_source_path_rejected;

    let report_path = fsv_root.join("task_py_g_120_skill_linkage_surface_fsv.json");
    let report = json!({
        "artifact_kind": "task_py_g_120_skill_linkage_surface_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-120",
        "passes": all_passed,
        "source_of_truth": {
            "db_path": db_path,
            "ast_chunk_source_jsonl": source_jsonl,
            "cfs": [
                CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
                CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
                CF_MEJEPA_SKILL_REVERSE_INDEX,
                CF_MEJEPA_SKILL_LIFECYCLE_AUDIT
            ]
        },
        "before": {
            "tree": before_tree,
            "cf_counts": before_counts
        },
        "after": {
            "tree": tree_state(&fsv_root)?,
            "cf_counts": after_counts,
            "report_sha256_placeholder": "computed-after-write"
        },
        "checks": {
            "skill_to_code_returns_source_bytes": skill_to_code_pass,
            "code_to_skill_preserves_many_to_many_membership": code_to_skill_pass,
            "skill_set_query_exact_intersection": set_query_pass,
            "coverage_audit_partitions_universe": coverage_pass,
            "constellation_membership_returns_named_levels": constellation_membership_pass,
            "chunk_as_star_preserves_slot_routes": chunk_as_star_pass,
            "skill_impact_blast_radius_monotonic": skill_impact_pass,
            "skill_graph_inspect_derives_comember_edges": skill_graph_pass,
            "skill_conflict_graph_derives_zero_cooccurrence_pairs": conflict_graph_pass,
            "skill_browse_returns_catalog_counts": skill_browse_pass,
            "cf_counts_match_expected": counts_pass,
            "no_new_prediction_head_introduced": no_new_head_pass
        },
        "reports": {
            "skill_to_code": skill_to_code_report,
            "code_to_skill": code_to_skill_report,
            "skill_set_query": set_query_report,
            "coverage_audit": coverage_audit,
            "constellation_membership": constellation_membership_report,
            "chunk_as_star": chunk_as_star_report,
            "skill_impact_depth_one": skill_impact_depth_one,
            "skill_impact_depth_two": skill_impact_depth_two,
            "skill_graph_inspect": skill_graph_report,
            "skill_conflict_graph": conflict_graph_report,
            "skill_browse": skill_browse_report,
            "skill_browse_filter": skill_browse_filter_report
        },
        "edge_cases": {
            "require_source_text_without_source_index_fails_closed": edge_missing_source_fails_closed,
            "overlapping_must_have_must_not_have_fails_closed": edge_overlap_set_query_rejected,
            "active_mntd_source_path_rejected": edge_mntd_source_path_rejected
        },
        "active_path_policy": {
            "mnt_d_active_reads_or_writes": false,
            "flat_vector_concat_used": false,
            "target_outcomes_used_as_live_inputs": false
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let report_sha = sha256_file(&report_path)?;
    let mut final_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    final_report["after"]["report_sha256_placeholder"] = json!(report_sha);
    fs::write(&report_path, serde_json::to_vec_pretty(&final_report)?)?;
    let final_readback: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if final_readback["passes"] != true || !all_passed {
        return Err(format!("TASK-PY-G-120 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&final_readback)?);
    Ok(())
}

fn parse_fsv_root() -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--fsv-root" {
            return PathBuf::from(args.next().expect("--fsv-root requires a value"));
        }
    }
    PathBuf::from("/tmp/contextgraph-task-py-g-120-skill-linkage-fsv")
}

fn step(
    index: u32,
    chunk_id: &str,
    file_path: &str,
    state: &str,
    label: &str,
) -> SkillStepEvidence {
    SkillStepEvidence {
        step_index: index,
        chunk_id: chunk_id.to_string(),
        file_path: file_path.to_string(),
        code_state_key: state.to_string(),
        accepted_label_ids: vec![label.to_string()],
        group_ids: vec!["group:sequence".to_string()],
    }
}

fn episode(
    episode_id: &str,
    first_chunk: &str,
    file_path: &str,
    state: &str,
    labels: [&str; 2],
) -> SkillEpisodeRow {
    SkillEpisodeRow {
        episode_id: episode_id.to_string(),
        proposed_skill_name: None,
        ordered_steps: vec![
            step(0, first_chunk, file_path, state, labels[0]),
            step(
                1,
                &format!("{first_chunk}:next"),
                file_path,
                state,
                labels[1],
            ),
        ],
        outcome: Some(SkillOutcomeObservation {
            outcome_label_id: "oracle:fail".to_string(),
            verdict: SkillOutcomeVerdict::Fail,
            target_side_supervision_only: true,
        }),
        failure_evidence_set_ids: vec![format!("evidence:{episode_id}")],
        cell_baseline_fail_rate: 0.25,
    }
}

fn skill(
    skill_id: &str,
    skill_name: &str,
    source_episode_ids: Vec<&str>,
    labels: [&str; 2],
) -> Level2SkillRow {
    Level2SkillRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_id: skill_id.to_string(),
        skill_name: skill_name.to_string(),
        parent_group_ids: vec!["group:sequence".to_string()],
        parent_skill_ids: Vec::new(),
        ordered_steps: vec![
            SkillStepTemplate {
                step_index: 0,
                accepted_label_ids: vec![labels[0].to_string()],
                group_ids: vec!["group:sequence".to_string()],
            },
            SkillStepTemplate {
                step_index: 1,
                accepted_label_ids: vec![labels[1].to_string()],
                group_ids: vec!["group:sequence".to_string()],
            },
        ],
        prerequisite_label_ids: vec![labels[0].to_string()],
        transition_edges: vec![SkillTransitionEdge {
            from_step_index: 0,
            to_step_index: 1,
            edge_label: "transition:next".to_string(),
        }],
        support: source_episode_ids.len() as u64,
        confidence: 0.9,
        lift_over_cell_baseline: 0.65,
        stability: 0.8,
        oracle_outcome_distribution: SkillOutcomeDistribution {
            pass: 0,
            fail: source_episode_ids.len() as u64,
            unknown: 0,
        },
        code_state_keys: vec!["state:other".to_string(), "state:shared".to_string()],
        source_episode_ids: source_episode_ids.into_iter().map(str::to_string).collect(),
        failure_evidence_set_ids: vec!["evidence:one".to_string()],
        live_input_allowed: true,
        promotion_status: SkillPromotionStatus::PromotionReady,
        operator_approved: false,
        created_at_unix_ms: 1_779_212_000_000,
    }
}

fn write_source_jsonl(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let chunks = [
        ("chunk:shared", "pkg/shared.py"),
        ("chunk:shared:next", "pkg/shared.py"),
        ("chunk:a_only", "pkg/a.py"),
        ("chunk:a_only:next", "pkg/a.py"),
        ("chunk:b_only", "pkg/b.py"),
        ("chunk:b_only:next", "pkg/b.py"),
        ("chunk:c_only", "pkg/c.py"),
        ("chunk:c_only:next", "pkg/c.py"),
    ];
    let mut file = fs::File::create(path)?;
    for (chunk_id, relative_path) in chunks {
        let source_text = format!("def {}():\n    return True\n", chunk_id.replace(':', "_"));
        let row = json!({
            "row_key": format!("row:{chunk_id}"),
            "chunk_id": chunk_id,
            "relative_path": relative_path,
            "byte_span": [0, source_text.len()],
            "source_text": source_text
        });
        writeln!(file, "{}", serde_json::to_string(&row)?)?;
    }
    Ok(())
}

fn cf_counts(
    db: &rocksdb::DB,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let mut map = serde_json::Map::new();
    map.insert(
        "level2Skills".to_string(),
        json!(count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)?),
    );
    map.insert(
        "chunkSkillMembership".to_string(),
        json!(count_cf_rows(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)?),
    );
    map.insert(
        "skillReverseIndex".to_string(),
        json!(count_cf_rows(db, CF_MEJEPA_SKILL_REVERSE_INDEX)?),
    );
    map.insert(
        "skillLifecycleAudit".to_string(),
        json!(count_cf_rows(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?),
    );
    Ok(map)
}

fn tree_state(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    if root.exists() {
        collect_files(root, root, &mut files)?;
    }
    files.sort_by(|left, right| {
        left["path"]
            .as_str()
            .unwrap_or("")
            .cmp(right["path"].as_str().unwrap_or(""))
    });
    Ok(json!({
        "root": root,
        "exists": root.exists(),
        "file_count": files.len(),
        "files": files
    }))
}

fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            files.push(json!({
                "path": path.strip_prefix(root)?.display().to_string(),
                "bytes": metadata.len()
            }));
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(fs::read(path)?))
    ))
}
