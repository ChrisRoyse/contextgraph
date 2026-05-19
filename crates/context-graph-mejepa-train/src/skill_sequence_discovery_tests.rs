use super::*;

fn step(index: u32, chunk: &str, file: &str, label: &str, group: &str) -> SkillStepEvidence {
    SkillStepEvidence {
        step_index: index,
        chunk_id: chunk.to_string(),
        file_path: file.to_string(),
        code_state_key: format!("python:state:{chunk}"),
        accepted_label_ids: vec![label.to_string()],
        group_ids: vec![group.to_string()],
    }
}

fn episode(id: &str, file_suffix: &str, verdict: Option<SkillOutcomeVerdict>) -> SkillEpisodeRow {
    SkillEpisodeRow {
        episode_id: id.to_string(),
        proposed_skill_name: Some("import_contract_drift".to_string()),
        ordered_steps: vec![
            step(
                0,
                &format!("chunk:{file_suffix}:caller"),
                &format!("pkg/{file_suffix}.py"),
                "ast_surface:import_from",
                "group:import_contract",
            ),
            step(
                1,
                &format!("chunk:{file_suffix}:callee"),
                &format!("pkg/{file_suffix}.py"),
                "pair_relation:caller_missing_symbol",
                "group:import_contract",
            ),
        ],
        outcome: verdict.map(|verdict| SkillOutcomeObservation {
            outcome_label_id: "oracle:fail".to_string(),
            verdict,
            target_side_supervision_only: true,
        }),
        failure_evidence_set_ids: vec![format!("evidence:{id}")],
        cell_baseline_fail_rate: 0.20,
    }
}

#[test]
fn discovery_promotes_ordered_skill_candidate() {
    let report = discover_skill_candidates(
        &[
            episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail)),
            episode("episode:b", "b", Some(SkillOutcomeVerdict::Fail)),
        ],
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )
    .unwrap();

    assert_eq!(report.rejections, Vec::new());
    assert_eq!(report.candidates.len(), 1);
    let row = &report.candidates[0];
    assert_eq!(row.skill_name, "import_contract_drift");
    assert_eq!(row.promotion_status, SkillPromotionStatus::PromotionReady);
    assert_eq!(row.ordered_steps.len(), 2);
    assert_eq!(row.transition_edges[0].from_step_index, 0);
    assert_eq!(row.transition_edges[0].to_step_index, 1);
    assert!(row.lift_over_cell_baseline >= 0.79);
}

#[test]
fn discovery_rejects_target_only_live_label() {
    let mut row = episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail));
    row.ordered_steps[0].accepted_label_ids = vec!["oracle:pass".to_string()];

    let err = discover_skill_candidates(&[row], SkillDiscoveryConfig::default(), 1_779_000_000_000)
        .unwrap_err();

    assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
}

#[test]
fn discovery_rejects_missing_order() {
    let mut row = episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail));
    row.ordered_steps[1].step_index = 3;

    let err = discover_skill_candidates(&[row], SkillDiscoveryConfig::default(), 1_779_000_000_000)
        .unwrap_err();

    assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
}

#[test]
fn discovery_rejects_low_support_overfit_candidate() {
    let report = discover_skill_candidates(
        &[episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail))],
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )
    .unwrap();

    assert_eq!(report.candidates, Vec::new());
    assert_eq!(report.rejections.len(), 1);
    assert_eq!(report.rejections[0].reason, "low_support_overfit");
}

#[test]
fn discovery_emits_pending_live_candidate_without_oracle_outcome() {
    let report = discover_skill_candidates(
        &[
            episode("episode:a", "a", None),
            episode("episode:b", "b", None),
        ],
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )
    .unwrap();

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(
        report.candidates[0].promotion_status,
        SkillPromotionStatus::ActiveLearning
    );
    assert_eq!(report.candidates[0].oracle_outcome_distribution.unknown, 2);
}
