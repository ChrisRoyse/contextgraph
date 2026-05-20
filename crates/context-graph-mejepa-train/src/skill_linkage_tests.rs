use super::*;
use crate::chunk_skill_membership::{
    materialize_skill_memberships, write_skill_materialization_sync_readback,
};
use crate::skill_sequence_discovery::{
    Level2SkillRow, SkillOutcomeDistribution, SkillPromotionStatus, SkillStepEvidence,
    SkillStepTemplate, SkillTransitionEdge, SKILL_SEQUENCE_SCHEMA_VERSION,
};

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
    first_path: &str,
    state: &str,
    labels: [&str; 2],
) -> crate::skill_sequence_discovery::SkillEpisodeRow {
    crate::skill_sequence_discovery::SkillEpisodeRow {
        episode_id: episode_id.to_string(),
        proposed_skill_name: None,
        ordered_steps: vec![
            step(0, first_chunk, first_path, state, labels[0]),
            step(
                1,
                &format!("{first_chunk}:next"),
                first_path,
                state,
                labels[1],
            ),
        ],
        outcome: Some(crate::skill_sequence_discovery::SkillOutcomeObservation {
            outcome_label_id: "oracle:fail".to_string(),
            verdict: crate::skill_sequence_discovery::SkillOutcomeVerdict::Fail,
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
        created_at_unix_ms: 1_779_211_000_000,
    }
}

fn open_test_db(path: &std::path::Path) -> DB {
    open_skill_linkage_rocksdb(path, true).unwrap()
}

fn seed_db() -> (tempfile::TempDir, Vec<String>, String, String) {
    let temp = tempfile::tempdir().unwrap();
    let db = open_test_db(temp.path());
    let skill_a = skill(
        "skill:import_contract_drift:test",
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
    let materialized_a =
        materialize_skill_memberships(&skill_a, &episodes_a, 1_779_211_000_000).unwrap();
    write_skill_materialization_sync_readback(&db, &materialized_a).unwrap();

    let skill_b = skill(
        "skill:mock_only_change:test",
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
    let materialized_b =
        materialize_skill_memberships(&skill_b, &episodes_b, 1_779_211_000_100).unwrap();
    write_skill_materialization_sync_readback(&db, &materialized_b).unwrap();
    drop(db);

    let chunk_ids = vec![
        "chunk:shared".to_string(),
        "chunk:shared:next".to_string(),
        "chunk:a_only".to_string(),
        "chunk:a_only:next".to_string(),
        "chunk:b_only".to_string(),
        "chunk:b_only:next".to_string(),
    ];
    (temp, chunk_ids, skill_a.skill_id, skill_b.skill_id)
}

fn write_isolated_skill(db: &DB) -> String {
    let skill_c = skill(
        "skill:isolated_state_transition:test",
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
    let materialized_c =
        materialize_skill_memberships(&skill_c, &episodes_c, 1_779_211_000_200).unwrap();
    write_skill_materialization_sync_readback(db, &materialized_c).unwrap();
    skill_c.skill_id
}

fn source_index(chunk_ids: &[String]) -> ChunkSourceIndex {
    let mut index = ChunkSourceIndex::default();
    index
        .insert(ChunkSourceRow {
            chunk_id: "chunk:shared".to_string(),
            file_path: "pkg/other_copy.py".to_string(),
            byte_span: [0, 9],
            source_text: Some("duplicate".to_string()),
            source_text_sha256: None,
            source_row_key: Some("row:duplicate".to_string()),
        })
        .unwrap();
    for chunk_id in chunk_ids {
        let path = if chunk_id.starts_with("chunk:a_only") {
            "pkg/a.py"
        } else if chunk_id.starts_with("chunk:b_only") {
            "pkg/b.py"
        } else {
            "pkg/shared.py"
        };
        let source = format!("def {}(): pass", chunk_id.replace(':', "_"));
        index
            .insert(ChunkSourceRow {
                chunk_id: chunk_id.clone(),
                file_path: path.to_string(),
                byte_span: [0, source.len() as u64],
                source_text: Some(source),
                source_text_sha256: None,
                source_row_key: Some(format!("row:{chunk_id}")),
            })
            .unwrap();
    }
    index
}

#[test]
fn skill_impact_walks_transitive_comemberships_monotonically() {
    let (temp, _chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());

    let depth_one = skill_impact(&db, "chunk:a_only", Some("state:other"), 1, 20).unwrap();
    let depth_two = skill_impact(&db, "chunk:a_only", Some("state:other"), 2, 20).unwrap();
    let depth_one_chunks = depth_one
        .impacted_chunks
        .iter()
        .map(|row| row.chunk_id.as_str())
        .collect::<BTreeSet<_>>();
    let depth_two_chunks = depth_two
        .impacted_chunks
        .iter()
        .map(|row| row.chunk_id.as_str())
        .collect::<BTreeSet<_>>();

    assert!(depth_one.no_new_prediction_head_introduced);
    assert!(depth_two.no_new_prediction_head_introduced);
    assert!(depth_one_chunks.contains("chunk:a_only"));
    assert!(depth_one_chunks.contains("chunk:shared"));
    assert!(!depth_one_chunks.contains("chunk:b_only"));
    assert!(depth_two_chunks.contains("chunk:b_only"));
    assert!(depth_two.total_impacted_chunks >= depth_one.total_impacted_chunks);
    assert!(depth_two.touched_skill_ids.contains(&skill_a));
    assert!(depth_two.touched_skill_ids.contains(&skill_b));
}

#[test]
fn skill_graph_inspect_derives_comember_edges() {
    let (temp, _chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());

    let report = skill_graph_inspect(&db, Some(&skill_a), 20).unwrap();

    assert!(report.no_new_prediction_head_introduced);
    assert_eq!(report.total_edges, 1);
    let edge = report
        .edges
        .iter()
        .find(|edge| {
            (edge.skill_a_id == skill_a && edge.skill_b_id == skill_b)
                || (edge.skill_a_id == skill_b && edge.skill_b_id == skill_a)
        })
        .unwrap();
    assert_eq!(edge.relation, "co_member_chunk");
    assert_eq!(edge.support_count, 2);
    assert!(edge.shared_chunk_ids.contains(&"chunk:shared".to_string()));
    assert!(edge
        .shared_chunk_ids
        .contains(&"chunk:shared:next".to_string()));
}

#[test]
fn skill_conflict_graph_reports_zero_cooccurrence_candidates() {
    let (temp, _chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());
    let skill_c = write_isolated_skill(&db);

    let report = skill_conflict_graph(&db, 20).unwrap();
    let pairs = report
        .conflict_pairs
        .iter()
        .map(|pair| {
            (
                pair.skill_a_id.as_str().min(pair.skill_b_id.as_str()),
                pair.skill_a_id.as_str().max(pair.skill_b_id.as_str()),
                pair.relation.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();

    assert!(report.no_new_prediction_head_introduced);
    assert_eq!(report.skill_count, 3);
    assert_eq!(report.evaluated_pair_count, 3);
    assert_eq!(report.total_conflict_pairs, 2);
    assert!(pairs.contains(&(
        skill_a.as_str().min(skill_c.as_str()),
        skill_a.as_str().max(skill_c.as_str()),
        "mutually_exclusive_candidate"
    )));
    assert!(pairs.contains(&(
        skill_b.as_str().min(skill_c.as_str()),
        skill_b.as_str().max(skill_c.as_str()),
        "mutually_exclusive_candidate"
    )));
}

#[test]
fn skill_browse_returns_catalog_counts_and_filter() {
    let (temp, _chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());

    let report = skill_browse(&db, None, 20).unwrap();
    let filtered = skill_browse(&db, Some("mock"), 20).unwrap();

    assert!(report.no_new_prediction_head_introduced);
    assert_eq!(report.total_catalog_skills, 2);
    assert_eq!(report.total_matching_skills, 2);
    let row_a = report
        .skills
        .iter()
        .find(|row| row.skill_id == skill_a)
        .unwrap();
    assert_eq!(row_a.membership_count, 4);
    assert_eq!(row_a.distinct_chunk_count, 4);
    assert_eq!(row_a.source_episode_count, 2);
    assert_eq!(filtered.total_matching_skills, 1);
    assert_eq!(filtered.skills[0].skill_id, skill_b);
}

#[test]
fn skill_to_code_returns_source_bytes() {
    let (temp, chunk_ids, skill_a, _skill_b) = seed_db();
    let db = open_test_db(temp.path());
    let index = source_index(&chunk_ids);

    let report = skill_to_code(
        &db,
        &skill_a,
        Some(&index),
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )
    .unwrap();

    assert_eq!(report.total_matching_memberships, 4);
    assert!(report.no_new_prediction_head_introduced);
    assert!(report
        .chunks
        .iter()
        .any(|row| row.chunk_id == "chunk:shared" && row.source_text.is_some()));
}

#[test]
fn code_to_skill_preserves_many_to_many_membership() {
    let (temp, chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());
    let index = source_index(&chunk_ids);

    let report = code_to_skill(
        &db,
        "chunk:shared",
        Some("state:shared"),
        Some(&index),
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )
    .unwrap();
    let observed = report
        .skills
        .iter()
        .map(|row| row.skill.skill_id.clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(report.total_matching_memberships, 2);
    assert!(observed.contains(&skill_a));
    assert!(observed.contains(&skill_b));
}

#[test]
fn constellation_membership_returns_named_levels_for_chunk() {
    let (temp, _chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());

    let report = constellation_membership(&db, "chunk:shared", Some("state:shared"), 10).unwrap();

    assert_eq!(report.memberships.len(), 2);
    assert!(report.level2_skill_ids.contains(&skill_a));
    assert!(report.level2_skill_ids.contains(&skill_b));
    assert!(report
        .level1_group_ids
        .contains(&"group:sequence".to_string()));
    assert!(report
        .live_level0_label_ids
        .contains(&"ast_surface:import_from".to_string()));
    assert!(report.no_new_prediction_head_introduced);
}

#[test]
fn chunk_as_star_uses_label_routes_without_flat_vector_stats() {
    let (temp, chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());
    let index = source_index(&chunk_ids);

    let report = chunk_as_star(
        &db,
        "chunk:shared",
        Some("state:shared"),
        Some(&index),
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )
    .unwrap();

    assert!(report.active_skill_ids.contains(&skill_a));
    assert!(report.active_skill_ids.contains(&skill_b));
    assert!(report
        .source
        .as_ref()
        .is_some_and(|row| row.source_text.is_some()));
    assert!(report
        .slot_parameter_cards
        .iter()
        .any(|card| card.slot_id == "e_ast"));
    assert!(report.no_new_prediction_head_introduced);
    assert!(!report.flat_vector_concat_used);
    assert!(!report.target_outcomes_used_as_live_inputs);
    assert!(report
        .slot_parameter_cards
        .iter()
        .all(|card| card.vector_norm.is_none() && card.sparsity.is_none()));
}

#[test]
fn skill_set_query_returns_intersection_minus_exclusion() {
    let (temp, _chunk_ids, skill_a, skill_b) = seed_db();
    let db = open_test_db(temp.path());

    let intersection = skill_set_query(&db, &[skill_a.clone(), skill_b.clone()], &[], 10).unwrap();
    let excluded = skill_set_query(
        &db,
        std::slice::from_ref(&skill_a),
        std::slice::from_ref(&skill_b),
        10,
    )
    .unwrap();

    assert_eq!(
        intersection.matching_chunk_ids,
        vec!["chunk:shared", "chunk:shared:next"]
    );
    assert!(!excluded
        .matching_chunk_ids
        .contains(&"chunk:shared".to_string()));
    assert!(!excluded
        .matching_chunk_ids
        .contains(&"chunk:shared:next".to_string()));
    assert!(excluded
        .matching_chunk_ids
        .contains(&"chunk:a_only".to_string()));
    assert!(excluded
        .matching_chunk_ids
        .contains(&"chunk:a_only:next".to_string()));
}

#[test]
fn coverage_audit_partitions_chunk_universe() {
    let (temp, chunk_ids, _skill_a, _skill_b) = seed_db();
    let db = open_test_db(temp.path());
    let mut universe = chunk_ids;
    universe.push("chunk:orphan".to_string());

    let audit = skill_coverage_audit(&db, &universe, 5).unwrap();

    assert_eq!(audit.total_chunk_universe, 7);
    assert_eq!(audit.chunks_without_membership, 1);
    assert_eq!(audit.zero_membership_chunk_ids, vec!["chunk:orphan"]);
    assert_eq!(
        audit.total_chunk_universe,
        audit.chunks_with_membership + audit.chunks_without_membership
    );
}

#[test]
fn require_source_text_fails_closed_when_source_missing() {
    let (temp, _chunk_ids, skill_a, _skill_b) = seed_db();
    let db = open_test_db(temp.path());

    let err = skill_to_code(
        &db,
        &skill_a,
        None,
        SkillLinkageOptions {
            limit: 10,
            require_source_text: true,
        },
    )
    .unwrap_err();

    assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    assert!(err.to_string().contains("source index is required"));
}
