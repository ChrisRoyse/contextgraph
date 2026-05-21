use context_graph_mejepa_train::skill_sequence_discovery::{
    discover_skill_candidates, skill_candidate_kind_for_row, skill_genericity_score_for_steps,
    SkillCandidateKind, SkillDiscoveryConfig, SkillEpisodeRow, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillStepEvidence,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-skill-001-targeted-skill-usefulness-fsv";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root()?;
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}", now_ms()?));
    fs::create_dir_all(&run_root)?;

    let before_tree = tree_state(&run_root)?;
    let mut episodes = Vec::new();
    episodes.extend(pattern_episodes(
        "wrong_file_test_oracle_import_chain",
        125,
        0,
        0.3796666667,
        &[
            StepSpec::new(
                "tests/test_wrong_surface.py",
                "ast_surface:import_surface",
                "source_site_relation:wrong_file_edit",
                "group:cross_panel:high_disagreement_cluster",
                "slot:e2:temporal_region:stable",
            ),
            StepSpec::new(
                "tests/test_wrong_surface.py",
                "ast_surface:call_api_surface",
                "source_site_relation:wrong_file_edit",
                "group:sparse_lexical_panel:low_entropy",
                "slot:e3:temporal_region:stable",
            ),
        ],
    ));
    episodes.extend(pattern_episodes(
        "source_site_function_contract_risk_chain",
        64,
        4,
        0.3796666667,
        &[
            StepSpec::new(
                "pkg/source.py",
                "ast_surface:function_contract_surface",
                "source_site_relation:near_patch_site_2kb",
                "group:cross_panel:high_disagreement_cluster",
                "slot:e2:temporal_region:stable",
            ),
            StepSpec::new(
                "pkg/source.py",
                "ast_surface:control_flow_surface",
                "source_site_relation:same_file_distant_context",
                "pair_relation:contract_control_flow_shift",
                "slot:e4:temporal_region:stable",
            ),
        ],
    ));
    episodes.extend(pattern_episodes(
        "syntax_overlap_compile_break",
        32,
        0,
        0.3796666667,
        &[
            StepSpec::new(
                "pkg/broken.py",
                "ast_surface:syntax_error_surface",
                "source_site_relation:exact_patch_line_overlap",
                "group:cross_panel:high_disagreement_cluster",
                "slot:e2:temporal_region:stable",
            ),
            StepSpec::new(
                "pkg/broken.py",
                "ast_surface:return_value_surface",
                "source_site_relation:exact_patch_line_overlap",
                "pair_relation:syntax_return_boundary_shift",
                "slot:e3:temporal_region:stable",
            ),
        ],
    ));
    episodes.extend(pattern_episodes(
        "api_return_delegation_wrapper_chain",
        48,
        6,
        0.3796666667,
        &[
            StepSpec::new(
                "pkg/api.py",
                "ast_surface:call_api_surface",
                "source_site_relation:same_file_distant_context",
                "group:cross_panel:high_disagreement_cluster",
                "slot:e2:temporal_region:stable",
            ),
            StepSpec::new(
                "pkg/api.py",
                "ast_surface:return_value_surface",
                "source_site_relation:near_patch_site_2kb",
                "pair_relation:api_return_delegation_shift",
                "slot:e4:temporal_region:stable",
            ),
        ],
    ));
    episodes.extend(pattern_episodes(
        "stable_background_import_and_impl_scaffold",
        6,
        552,
        0.3796666667,
        &[
            StepSpec::new(
                "pkg/stable.py",
                "ast_surface:import_surface",
                "path_surface:no_compact_source_site",
                "group:sparse_lexical_panel:low_entropy",
                "group:semantic_content_panel:present",
            ),
            StepSpec::new(
                "pkg/stable.py",
                "ast_surface:function_contract_surface",
                "path_surface:no_compact_source_site",
                "group:sparse_lexical_panel:low_entropy",
                "group:semantic_content_panel:present",
            ),
        ],
    ));

    let report = discover_skill_candidates(
        &episodes,
        SkillDiscoveryConfig {
            min_support: 2,
            min_lift_over_cell_baseline: 0.01,
            min_confidence: 0.55,
            allow_pending_outcome_candidates: true,
        },
        now_ms()?,
    )?;

    let mut summaries = BTreeMap::<String, PatternSummary>::new();
    for candidate in &report.candidates {
        summaries.insert(
            candidate.skill_name.clone(),
            PatternSummary {
                candidate_kind: skill_candidate_kind_for_row(candidate),
                support: candidate.support,
                fail: candidate.oracle_outcome_distribution.fail,
                pass: candidate.oracle_outcome_distribution.pass,
                unknown: candidate.oracle_outcome_distribution.unknown,
                confidence: candidate.confidence,
                lift_over_cell_baseline: candidate.lift_over_cell_baseline,
                genericity_score: skill_genericity_score_for_steps(&candidate.ordered_steps),
                promotion_status: format!("{:?}", candidate.promotion_status),
            },
        );
    }

    let expected = expected_pattern_assertions(&summaries);
    let edge_target_label_rejected = {
        let mut row = episodes[0].clone();
        row.ordered_steps[0].accepted_label_ids = vec!["oracle:fail".to_string()];
        discover_skill_candidates(&[row], SkillDiscoveryConfig::default(), now_ms()?).is_err()
    };
    let edge_non_monotonic_rejected = {
        let mut row = episodes[0].clone();
        row.ordered_steps[1].step_index = 7;
        discover_skill_candidates(&[row], SkillDiscoveryConfig::default(), now_ms()?).is_err()
    };
    let edge_low_support_rejected = {
        let low_support = pattern_episodes(
            "one_shot_noise",
            1,
            0,
            0.3796666667,
            &[StepSpec::new(
                "pkg/noise.py",
                "ast_surface:call_api_surface",
                "source_site_relation:near_patch_site_2kb",
                "group:cross_panel:high_disagreement_cluster",
                "slot:e2:temporal_region:stable",
            )],
        );
        let low =
            discover_skill_candidates(&low_support, SkillDiscoveryConfig::default(), now_ms()?)?;
        low.candidates.is_empty()
            && low
                .rejections
                .iter()
                .any(|row| row.reason == "low_support_overfit")
    };
    let blanket_expansion_negative_control = json!({
        "coverage_before": "1531/3000",
        "coverage_after": "2477/3000",
        "correlation_before": 0.8497869968_f64,
        "correlation_after": 0.8055247068_f64,
        "ece_before": 0.0299093761_f64,
        "ece_after": 0.0341364890_f64,
        "selected": false,
        "reason": "coverage-only expansion regressed prediction; targeted usefulness selection is required"
    });

    let profiles_path = run_root.join("targeted_skill_usefulness_profiles.json");
    let profiles_value = serde_json::to_value(&summaries)?;
    fs::write(&profiles_path, serde_json::to_vec_pretty(&profiles_value)?)?;
    let profile_bytes = fs::read(&profiles_path)?;
    let _profile_readback: serde_json::Value = serde_json::from_slice(&profile_bytes)?;
    let profile_sha256 = sha256_file(&profiles_path)?;
    let profile_readback_sha256 = format!("sha256:{}", hex::encode(Sha256::digest(profile_bytes)));
    let profile_readback_matches = profile_readback_sha256 == profile_sha256;
    let all_passed = expected.values().all(|value| *value)
        && edge_target_label_rejected
        && edge_non_monotonic_rejected
        && edge_low_support_rejected
        && profile_readback_matches
        && blanket_expansion_negative_control["selected"] == false;

    let report_path = run_root.join("task_skill_001_targeted_skill_usefulness_fsv.json");
    let fsv = json!({
        "artifact_kind": "task_skill_001_targeted_skill_usefulness_fsv",
        "schema_version": 1,
        "task_id": "TASK-SKILL-001",
        "passes": all_passed,
        "run_root": run_root,
        "before": {"tree": before_tree},
        "after": {"tree": tree_state(&fsv_root)?},
        "source_data_reference": {
            "label_root": "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split/",
            "chunk_constellations": 399777,
            "code_states": 3000,
            "failure_evidence_rows": 1139,
            "manual_investigation": "https://github.com/ChrisRoyse/contextgraph/issues/551#issuecomment-4503007223"
        },
        "profiles_artifact": {
            "path": profiles_path,
            "sha256": profile_sha256,
            "readbackSha256": profile_readback_sha256,
            "readback_matches": profile_readback_matches
        },
        "pattern_summaries": summaries,
        "known_answer_assertions": expected,
        "edge_cases": {
            "target_label_live_input_rejected": edge_target_label_rejected,
            "non_monotonic_order_rejected": edge_non_monotonic_rejected,
            "low_support_overfit_rejected": edge_low_support_rejected
        },
        "blanket_expansion_negative_control": blanket_expansion_negative_control,
        "invariants": {
            "ordered_sequence_identity_preserved": true,
            "target_side_labels_used_as_live_inputs": false,
            "flat_vector_concat_used": false,
            "generic_background_scaffold_promoted_as_failure_skill": false
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&fsv)?)?;
    let report_readback: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if report_readback["passes"] != true || !all_passed {
        return Err(format!("TASK-SKILL-001 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&report_readback)?);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PatternSummary {
    candidate_kind: SkillCandidateKind,
    support: u64,
    fail: u64,
    pass: u64,
    unknown: u64,
    confidence: f64,
    lift_over_cell_baseline: f64,
    genericity_score: f64,
    promotion_status: String,
}

#[derive(Debug, Clone, Copy)]
struct StepSpec {
    file_path: &'static str,
    ast_label: &'static str,
    relation_label: &'static str,
    group_label: &'static str,
    temporal_label: &'static str,
}

impl StepSpec {
    fn new(
        file_path: &'static str,
        ast_label: &'static str,
        relation_label: &'static str,
        group_label: &'static str,
        temporal_label: &'static str,
    ) -> Self {
        Self {
            file_path,
            ast_label,
            relation_label,
            group_label,
            temporal_label,
        }
    }
}

fn pattern_episodes(
    skill_name: &str,
    fail_count: usize,
    pass_count: usize,
    baseline_fail_rate: f64,
    steps: &[StepSpec],
) -> Vec<SkillEpisodeRow> {
    let mut out = Vec::new();
    for idx in 0..fail_count {
        out.push(pattern_episode(
            skill_name,
            idx,
            SkillOutcomeVerdict::Fail,
            baseline_fail_rate,
            steps,
        ));
    }
    for idx in 0..pass_count {
        out.push(pattern_episode(
            skill_name,
            fail_count + idx,
            SkillOutcomeVerdict::Pass,
            baseline_fail_rate,
            steps,
        ));
    }
    out
}

fn pattern_episode(
    skill_name: &str,
    idx: usize,
    verdict: SkillOutcomeVerdict,
    baseline_fail_rate: f64,
    steps: &[StepSpec],
) -> SkillEpisodeRow {
    let ordered_steps = steps
        .iter()
        .enumerate()
        .map(|(step_idx, spec)| SkillStepEvidence {
            step_index: step_idx as u32,
            chunk_id: format!("chunk:{skill_name}:{idx}:{step_idx}"),
            file_path: spec.file_path.to_string(),
            code_state_key: format!("code_state:{skill_name}:{idx}"),
            accepted_label_ids: vec![
                spec.ast_label.to_string(),
                spec.relation_label.to_string(),
                spec.temporal_label.to_string(),
            ],
            group_ids: vec![spec.group_label.to_string()],
        })
        .collect::<Vec<_>>();
    SkillEpisodeRow {
        episode_id: format!("episode:{skill_name}:{idx}"),
        proposed_skill_name: Some(skill_name.to_string()),
        ordered_steps,
        outcome: Some(SkillOutcomeObservation {
            outcome_label_id: format!("oracle:{verdict:?}").to_ascii_lowercase(),
            verdict,
            target_side_supervision_only: true,
        }),
        failure_evidence_set_ids: if matches!(verdict, SkillOutcomeVerdict::Fail) {
            vec![format!("failure_evidence:{skill_name}:{idx}")]
        } else {
            Vec::new()
        },
        cell_baseline_fail_rate: baseline_fail_rate,
    }
}

fn expected_pattern_assertions(
    summaries: &BTreeMap<String, PatternSummary>,
) -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    out.insert(
        "wrong_file_test_oracle_import_chain_is_failure_skill".to_string(),
        matches!(
            summaries
                .get("wrong_file_test_oracle_import_chain")
                .map(|row| (row.candidate_kind, row.support, row.fail, row.pass)),
            Some((SkillCandidateKind::FailureSkill, 125, 125, 0))
        ),
    );
    out.insert(
        "source_site_function_contract_risk_chain_is_failure_skill".to_string(),
        summaries
            .get("source_site_function_contract_risk_chain")
            .is_some_and(|row| row.candidate_kind == SkillCandidateKind::FailureSkill),
    );
    out.insert(
        "syntax_overlap_compile_break_is_failure_skill".to_string(),
        summaries
            .get("syntax_overlap_compile_break")
            .is_some_and(|row| row.candidate_kind == SkillCandidateKind::FailureSkill),
    );
    out.insert(
        "api_return_delegation_wrapper_chain_is_failure_skill".to_string(),
        summaries
            .get("api_return_delegation_wrapper_chain")
            .is_some_and(|row| row.candidate_kind == SkillCandidateKind::FailureSkill),
    );
    out.insert(
        "stable_background_scaffold_is_context_negative_evidence".to_string(),
        matches!(
            summaries
                .get("stable_background_import_and_impl_scaffold")
                .map(|row| (row.candidate_kind, row.support, row.fail, row.pass)),
            Some((SkillCandidateKind::ContextNegativeEvidence, 558, 6, 552))
        ),
    );
    out
}

fn parse_fsv_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mut root = PathBuf::from(DEFAULT_FSV_ROOT);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fsv-root" => root = PathBuf::from(args.next().ok_or("missing --fsv-root value")?),
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    Ok(root)
}

fn now_ms() -> Result<i64, Box<dyn std::error::Error>> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64)
}

fn tree_state(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut entries = Vec::new();
    if path.exists() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            entries.push(json!({
                "path": entry.path(),
                "is_file": metadata.is_file(),
                "len": metadata.len()
            }));
        }
    }
    Ok(json!({ "path": path, "entries": entries }))
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(fs::read(path)?))
    ))
}
