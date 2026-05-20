use super::*;
use crate::chunk_skill_membership::{
    materialize_skill_memberships, membership_key, read_all_chunk_skill_membership_rows,
    write_skill_materialization_sync_readback, SkillLifecycleDecision,
};
use crate::label_bridge::{
    ability_signature_hash, accepted_label_signature_hash, membership_signature_hash,
    skill_signature_hash,
};
use crate::mistake_log::MistakeTruthSource;
use crate::skill_sequence_discovery::{
    Level2SkillRow, SkillEpisodeRow, SkillOutcomeDistribution, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillPromotionStatus, SkillStepEvidence, SkillStepTemplate,
    SkillTransitionEdge,
};
use context_graph_mejepa::{PredictionId, PredictionLabelContext, Verdict};
use context_graph_mejepa_cf::CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP;
use rocksdb::DB;

const NOW: i64 = 1_779_300_000_000;

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
        group_ids: vec!["group:runtime".to_string()],
    }
}

fn episode(
    episode_id: &str,
    first_chunk: &str,
    first_path: &str,
    state: &str,
    labels: [&str; 2],
) -> SkillEpisodeRow {
    SkillEpisodeRow {
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
        parent_group_ids: vec!["group:runtime".to_string()],
        parent_skill_ids: Vec::new(),
        ordered_steps: vec![
            SkillStepTemplate {
                step_index: 0,
                accepted_label_ids: vec![labels[0].to_string()],
                group_ids: vec!["group:runtime".to_string()],
            },
            SkillStepTemplate {
                step_index: 1,
                accepted_label_ids: vec![labels[1].to_string()],
                group_ids: vec!["group:runtime".to_string()],
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
        code_state_keys: vec!["state:shared".to_string(), "state:other".to_string()],
        source_episode_ids: source_episode_ids.into_iter().map(str::to_string).collect(),
        failure_evidence_set_ids: vec!["evidence:unit".to_string()],
        live_input_allowed: true,
        promotion_status: SkillPromotionStatus::PromotionReady,
        operator_approved: false,
        created_at_unix_ms: NOW,
    }
}

fn seed_db() -> (tempfile::TempDir, DB, String, String) {
    let temp = tempfile::tempdir().unwrap();
    let db = open_ability_resolver_rocksdb(temp.path(), true).unwrap();
    let skill_a = skill(
        "skill:import_contract_drift:unit",
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
    write_skill_materialization_sync_readback(
        &db,
        &materialize_skill_memberships(&skill_a, &episodes_a, NOW).unwrap(),
    )
    .unwrap();
    let skill_b = skill(
        "skill:mock_only_change:unit",
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
    write_skill_materialization_sync_readback(
        &db,
        &materialize_skill_memberships(&skill_b, &episodes_b, NOW + 1).unwrap(),
    )
    .unwrap();
    (temp, db, skill_a.skill_id, skill_b.skill_id)
}

fn write_higher_ability_membership(db: &DB, source_skill_id: &str) -> String {
    let mut row = read_all_chunk_skill_membership_rows(db)
        .unwrap()
        .into_iter()
        .find(|row| row.skill_id == source_skill_id && row.chunk_id == "chunk:shared")
        .unwrap();
    row.skill_id = "ability:import_mock_runtime_sequence:unit".to_string();
    row.hierarchy_level = 3;
    row.membership_key = membership_key(&row.chunk_id, &row.code_state_key, &row.skill_id).unwrap();
    row.provenance_hashes = vec!["sha256:higherabilityunit".to_string()];
    row.validate().unwrap();
    let cf = db.cf_handle(CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP).unwrap();
    db.put_cf(
        cf,
        row.membership_key.as_bytes(),
        bincode::serialize(&row).unwrap(),
    )
    .unwrap();
    db.flush_cf(cf).unwrap();
    row.skill_id
}

fn request(chunks: &[&str]) -> AbilityResolverRequest {
    request_with_state("state:shared", chunks)
}

fn request_with_state(state: &str, chunks: &[&str]) -> AbilityResolverRequest {
    AbilityResolverRequest {
        code_state_key: state.to_string(),
        chunks: chunks
            .iter()
            .map(|chunk_id| LiveChunkInput {
                chunk_id: (*chunk_id).to_string(),
                file_path: Some("pkg/shared.py".to_string()),
                accepted_label_ids: vec!["ast_surface:function".to_string()],
            })
            .collect(),
    }
}

fn label_context(context: &AbilityContext) -> PredictionLabelContext {
    PredictionLabelContext {
        accepted_label_ids: context.accepted_label_ids.clone(),
        code_state_key: Some(context.code_state_key.clone()),
        active_skill_ids: context.active_skill_ids.clone(),
        active_higher_ability_ids: context.active_higher_ability_ids.clone(),
        source_membership_keys: context.source_membership_keys.clone(),
        label_signature_hash: Some(
            accepted_label_signature_hash(&context.accepted_label_ids).unwrap(),
        ),
        skill_signature_hash: if context.active_skill_ids.is_empty() {
            None
        } else {
            Some(skill_signature_hash(&context.active_skill_ids).unwrap())
        },
        ability_signature_hash: if context.active_higher_ability_ids.is_empty() {
            None
        } else {
            Some(ability_signature_hash(&context.active_higher_ability_ids).unwrap())
        },
        membership_signature_hash: if context.source_membership_keys.is_empty() {
            None
        } else {
            Some(membership_signature_hash(&context.source_membership_keys).unwrap())
        },
        ..PredictionLabelContext::default()
    }
}

fn input(byte: u8) -> AbilityRefutationInput {
    AbilityRefutationInput {
        prediction_id: PredictionId([byte; 16]),
        panel_signature_hash: format!("panel:test:{byte:02x}"),
        predicted_verdict: Verdict::Pass,
        ground_truth_verdict: Verdict::Fail,
        truth_source: MistakeTruthSource::ShiftLogReplay,
        language: "python".to_string(),
        mutation_or_live_cell: "live_project".to_string(),
        named_failure_mode: "failure:runtime_ability".to_string(),
        surprise_z: 2.5,
        coverage_gap_score: 0.7,
        created_at_unix_ms: NOW + 10,
    }
}

#[test]
fn resolver_preserves_many_to_many_memberships() {
    let (_temp, db, skill_a, skill_b) = seed_db();

    let context = resolve_ability_context(&db, request(&["chunk:shared"])).unwrap();

    assert_eq!(context.active_skill_ids, vec![skill_a, skill_b]);
    assert_eq!(context.source_membership_keys.len(), 2);
    assert!(context
        .accepted_label_ids
        .contains(&"ast_surface:function".to_string()));
    assert!(context.no_new_prediction_head_introduced);
    assert!(!context.flat_vector_concat_used);
}

#[test]
fn ability_context_rejects_unbacked_aggregate_ids() {
    let (_temp, db, _skill_a, _skill_b) = seed_db();
    let mut context = resolve_ability_context(&db, request(&["chunk:shared"])).unwrap();
    context
        .active_higher_ability_ids
        .push("ability:not_backed_by_membership".to_string());
    context.ability_signature_hash =
        Some(ability_signature_hash(&context.active_higher_ability_ids).unwrap());

    let err = context.validate().unwrap_err();

    assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
}

#[test]
fn ordered_skill_signature_tracks_chunk_order() {
    let (_temp, db, skill_a, skill_b) = seed_db();

    let left = resolve_ability_context(
        &db,
        request_with_state("state:other", &["chunk:a_only", "chunk:b_only"]),
    )
    .unwrap();
    let right = resolve_ability_context(
        &db,
        request_with_state("state:other", &["chunk:b_only", "chunk:a_only"]),
    )
    .unwrap();

    assert_eq!(
        left.active_skill_ids,
        vec![skill_a.clone(), skill_b.clone()]
    );
    assert_eq!(right.active_skill_ids, vec![skill_b, skill_a]);
    assert_ne!(left.skill_signature_hash, right.skill_signature_hash);
}

#[test]
fn repeated_same_skill_steps_are_distinguished_by_membership_signature() {
    let (_temp, db, skill_a, _skill_b) = seed_db();

    let first_step =
        resolve_ability_context(&db, request_with_state("state:other", &["chunk:a_only"])).unwrap();
    let two_steps = resolve_ability_context(
        &db,
        request_with_state("state:other", &["chunk:a_only", "chunk:a_only:next"]),
    )
    .unwrap();

    assert_eq!(first_step.active_skill_ids, vec![skill_a.clone()]);
    assert_eq!(two_steps.active_skill_ids, vec![skill_a]);
    assert_eq!(
        first_step.skill_signature_hash,
        two_steps.skill_signature_hash
    );
    assert_eq!(first_step.source_membership_keys.len(), 1);
    assert_eq!(two_steps.source_membership_keys.len(), 2);
    assert_ne!(
        first_step.membership_signature_hash,
        two_steps.membership_signature_hash
    );
}

#[test]
fn refutation_writes_replay_mistake_and_lifecycle_rows() {
    let (_temp, db, skill_a, _skill_b) = seed_db();
    let ability_id = write_higher_ability_membership(&db, &skill_a);
    let context = resolve_ability_context(&db, request(&["chunk:shared"])).unwrap();
    let label_context = label_context(&context);

    let report =
        record_ability_refutation_sync_readback(&db, &context, &label_context, input(0x42))
            .unwrap();

    assert!(report.label_skill_ability_membership_ids_agree);
    assert_eq!(report.lifecycle_audits.len(), 3);
    assert_eq!(context.active_higher_ability_ids, vec![ability_id]);
    assert_eq!(
        report.mistake_row.active_higher_ability_ids,
        context.active_higher_ability_ids
    );
    assert_eq!(
        report.replay_row.source_membership_keys,
        context.source_membership_keys
    );
    assert!(
        report
            .online_head_update_report
            .same_panel_signature_corrected
    );
    assert!(report.online_head_update_report.repeat_metric_byte_readable);
}

#[test]
fn refutation_rejects_stale_label_context_signatures() {
    let (_temp, db, skill_a, _skill_b) = seed_db();
    write_higher_ability_membership(&db, &skill_a);
    let context = resolve_ability_context(&db, request(&["chunk:shared"])).unwrap();
    let mut label_context = label_context(&context);
    label_context.membership_signature_hash = Some("memberships:stale".to_string());

    let err = record_ability_refutation_sync_readback(&db, &context, &label_context, input(0x43))
        .unwrap_err();

    assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
}

#[test]
fn refutation_without_existing_skill_creates_candidate() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_ability_resolver_rocksdb(temp.path(), true).unwrap();
    let context = resolve_ability_context(&db, request(&["chunk:unknown"])).unwrap();
    let label_context = label_context(&context);

    let report =
        record_ability_refutation_sync_readback(&db, &context, &label_context, input(0x55))
            .unwrap();

    assert!(report.candidate_created_when_no_existing_ability);
    assert_eq!(report.lifecycle_audits.len(), 1);
    assert_eq!(
        report.lifecycle_audits[0].decision,
        SkillLifecycleDecision::CreateNewCandidateSkill
    );
    assert_eq!(
        report
            .online_head_update_report
            .corrected_verdict_after_update,
        Verdict::Fail
    );
}

#[test]
fn resolver_rejects_target_side_live_labels_and_leaky_memberships() {
    let (_temp, db, skill_a, _skill_b) = seed_db();
    let bad_request = AbilityResolverRequest {
        code_state_key: "state:shared".to_string(),
        chunks: vec![LiveChunkInput {
            chunk_id: "chunk:bad".to_string(),
            file_path: Some("pkg/bad.py".to_string()),
            accepted_label_ids: vec!["oracle:fail".to_string()],
        }],
    };
    assert!(resolve_ability_context(&db, bad_request).is_err());

    let mut row = read_all_chunk_skill_membership_rows(&db)
        .unwrap()
        .into_iter()
        .find(|row| row.skill_id == skill_a && row.chunk_id == "chunk:shared")
        .unwrap();
    row.live_input_allowed = false;
    row.membership_key = membership_key(&row.chunk_id, &row.code_state_key, &row.skill_id).unwrap();
    let cf = db.cf_handle(CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP).unwrap();
    db.put_cf(
        cf,
        row.membership_key.as_bytes(),
        bincode::serialize(&row).unwrap(),
    )
    .unwrap();
    db.flush_cf(cf).unwrap();

    assert!(resolve_ability_context(&db, request(&["chunk:shared"])).is_err());
}
