use super::*;
use crate::skill_sequence_discovery::{
    discover_skill_candidates, SkillDiscoveryConfig, SkillOutcomeObservation, SkillOutcomeVerdict,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use rocksdb::{ColumnFamilyDescriptor, Options};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct LegacySkillLifecycleAuditRow {
    schema_version: u32,
    skill_audit_id: String,
    prediction_id: Option<String>,
    mistake_id: Option<String>,
    previous_skill_id: Option<String>,
    decision: SkillLifecycleDecision,
    candidate_skill_id: Option<String>,
    evidence_label_ids: Vec<String>,
    evidence_chunk_ids: Vec<String>,
    reason: String,
    created_at_unix_ms: i64,
}

fn step(index: u32, chunk: &str, label: &str) -> SkillStepEvidence {
    SkillStepEvidence {
        step_index: index,
        chunk_id: chunk.to_string(),
        file_path: format!("pkg/{chunk}.py"),
        code_state_key: format!("python:state:{chunk}"),
        accepted_label_ids: vec![label.to_string()],
        group_ids: vec!["group:boundary".to_string()],
    }
}

fn episode(id: &str, chunk_prefix: &str) -> SkillEpisodeRow {
    SkillEpisodeRow {
        episode_id: id.to_string(),
        proposed_skill_name: Some("boundary_check_removed".to_string()),
        ordered_steps: vec![
            step(0, &format!("{chunk_prefix}:guard"), "ast_surface:if_guard"),
            step(
                1,
                &format!("{chunk_prefix}:return"),
                "pair_relation:guard_to_return",
            ),
        ],
        outcome: Some(SkillOutcomeObservation {
            outcome_label_id: "oracle:fail".to_string(),
            verdict: SkillOutcomeVerdict::Fail,
            target_side_supervision_only: true,
        }),
        failure_evidence_set_ids: vec![format!("evidence:{id}")],
        cell_baseline_fail_rate: 0.25,
    }
}

fn open_db(path: &std::path::Path) -> DB {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    DB::open_cf_descriptors(
        &opts,
        path,
        vec![
            ColumnFamilyDescriptor::new(CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_SKILL_REVERSE_INDEX, Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, Options::default()),
        ],
    )
    .unwrap()
}

#[test]
fn materialization_writes_and_reads_many_to_many_rows() {
    let episodes = vec![episode("episode:a", "a"), episode("episode:b", "b")];
    let report = discover_skill_candidates(
        &episodes,
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )
    .unwrap();
    let skill = report.candidates.first().unwrap();
    let materialized = materialize_skill_memberships(skill, &episodes, 1_779_000_000_000).unwrap();
    assert_eq!(materialized.level2_skills.len(), 1);
    assert_eq!(materialized.chunk_memberships.len(), 4);
    assert_eq!(materialized.lifecycle_audits.len(), 1);

    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());
    write_skill_materialization_sync_readback(&db, &materialized).unwrap();

    assert_eq!(
        count_cf_rows(&db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP).unwrap(),
        4
    );
    assert_eq!(
        count_cf_rows(&db, CF_MEJEPA_SKILL_REVERSE_INDEX).unwrap(),
        4
    );
    assert_eq!(
        count_cf_rows(&db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS).unwrap(),
        1
    );
}

#[test]
fn materialization_rejects_leaky_step_label() {
    let mut episodes = vec![episode("episode:a", "a"), episode("episode:b", "b")];
    let report = discover_skill_candidates(
        &episodes,
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )
    .unwrap();
    let skill = report.candidates.first().unwrap().clone();
    episodes[0].ordered_steps[0].accepted_label_ids = vec!["oracle:pass".to_string()];

    let err = materialize_skill_memberships(&skill, &episodes, 1_779_000_000_000).unwrap_err();

    assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
}

#[test]
fn materialization_rejects_duplicate_source_episode_ids() {
    let episodes = vec![episode("episode:a", "a"), episode("episode:b", "b")];
    let report = discover_skill_candidates(
        &episodes,
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )
    .unwrap();
    let skill = report.candidates.first().unwrap().clone();
    let duplicated = vec![
        episode("episode:a", "a"),
        episode("episode:a", "a-duplicate"),
        episode("episode:b", "b"),
    ];

    let err = materialize_skill_memberships(&skill, &duplicated, 1_779_000_000_000).unwrap_err();

    assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    assert!(
        err.to_string().contains("duplicate source episode id"),
        "{err}"
    );
}

#[test]
fn membership_hash_changes_when_row_content_changes() {
    let episodes = vec![episode("episode:a", "a"), episode("episode:b", "b")];
    let report = discover_skill_candidates(
        &episodes,
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )
    .unwrap();
    let skill = report.candidates.first().unwrap();
    let materialized = materialize_skill_memberships(skill, &episodes, 1_779_000_000_000).unwrap();
    let baseline = membership_hash(&materialized.chunk_memberships).unwrap();
    let mut changed = materialized.chunk_memberships.clone();
    changed[0].membership_score = 0.123;

    let changed_hash = membership_hash(&changed).unwrap();

    assert_ne!(baseline, changed_hash);
}

#[test]
fn lifecycle_audit_deserializes_legacy_rows_with_empty_ability_context() {
    let legacy = LegacySkillLifecycleAuditRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_audit_id: "skill_audit:legacy".to_string(),
        prediction_id: Some("prediction:legacy".to_string()),
        mistake_id: Some("mistake:legacy".to_string()),
        previous_skill_id: Some("skill:legacy".to_string()),
        decision: SkillLifecycleDecision::UpdateExistingSkill,
        candidate_skill_id: None,
        evidence_label_ids: vec!["ast_surface:function".to_string()],
        evidence_chunk_ids: vec!["chunk:legacy".to_string()],
        reason: "legacy_row".to_string(),
        created_at_unix_ms: 1_778_000_000_000,
    };
    let bytes = bincode::serialize(&legacy).unwrap();
    let decoded = decode_skill_lifecycle_audit_row(&bytes).unwrap();

    decoded.validate().unwrap();
    assert!(decoded.evidence_skill_ids.is_empty());
    assert!(decoded.evidence_higher_ability_ids.is_empty());
    assert!(decoded.source_membership_keys.is_empty());
}
