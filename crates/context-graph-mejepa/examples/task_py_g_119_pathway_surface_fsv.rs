use context_graph_mejepa::{
    pathway_leaf_credit_assignment, persist_pathway_surface, read_operator_pathway_choices,
    read_pathway_tree, read_surfaced_pathways_for_prediction, reject_ambiguous_leaf,
    surface_pathways, write_operator_pathway_choice, OperatorPathwayChoiceRecord, PathwayLeaf,
    PathwayLeafCalibrationReference, PathwayLeafEvidence, PathwayLeafKind, PathwayLeafOutcome,
    PathwaySurfaceInput, Q5PathwayEventInput, PATHWAY_AMBIGUOUS_LEAF_REJECTED,
    PATHWAY_SCHEMA_VERSION, UNKNOWN_PATHWAY_SIGNATURE,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_OPERATOR_PATHWAY_CHOICES, CF_MEJEPA_PATHWAY_TREES, CF_MEJEPA_SURFACED_PATHWAYS,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const NOW: i64 = 1_779_360_000_000;
const PREDICTION_ID: &str = "abcdef1234567890abcdef1234567890";
const PATCH_SHA: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";

fn main() -> FsvResult<()> {
    let fsv_root = arg_path("--fsv-root").unwrap_or_else(|| {
        PathBuf::from("/zfs/archive/contextgraph/fsv/task-py-g-119-pathway-surface-fsv")
    });
    reject_mntd_root(&fsv_root)?;
    if fsv_root.exists() {
        std::fs::remove_dir_all(&fsv_root)?;
    }
    std::fs::create_dir_all(&fsv_root)?;
    let db_path = fsv_root.join("pathway.rocksdb");
    let db = open_db(&db_path, true)?;

    let report = surface_pathways(fixture_input())?;
    persist_pathway_surface(&db, &report)?;
    let choice = OperatorPathwayChoiceRecord {
        schema_version: PATHWAY_SCHEMA_VERSION,
        choice_id: "choice:operator:001".to_string(),
        prediction_id_hex: PREDICTION_ID.to_string(),
        pathway_id: report.surfaced_pathways[0].pathway_id.clone(),
        operator_id: "operator:codex-fsv".to_string(),
        rationale_text: Some("surface the highest-probability binary consequence path".to_string()),
        chosen_at_unix_ms: NOW + 1,
    };
    let choice_first_inserted = write_operator_pathway_choice(&db, &choice)?;
    let choice_second_inserted = write_operator_pathway_choice(&db, &choice)?;
    flush_cfs(&db)?;
    drop(db);

    let readback_db = open_db(&db_path, false)?;
    let readback_tree = read_pathway_tree(&readback_db, &report.tree.tree_id)?
        .ok_or("missing pathway tree after reopen")?;
    let readback_pathways = read_surfaced_pathways_for_prediction(&readback_db, PREDICTION_ID, 20)?;
    let readback_choices = read_operator_pathway_choices(&readback_db, 20)?;
    let cf_counts = cf_counts(&readback_db)?;
    drop(readback_db);

    let ambiguous_edge = ambiguous_leaf_error();
    let missing_cf_edge = missing_cf_error(&report.tree)?;
    let normalized_sum = readback_pathways
        .iter()
        .map(|pathway| pathway.normalized_probability)
        .sum::<f32>();
    let no_ambiguous_leaves = readback_tree.ambiguous_leaves_in_pathway == 0
        && readback_pathways
            .iter()
            .flat_map(|pathway| pathway.leaf_chain.iter())
            .all(|leaf| leaf.leaf_kind.is_binary());
    let q5_skill_attachment_preserved = readback_pathways
        .iter()
        .flat_map(|pathway| pathway.leaf_chain.iter())
        .any(|leaf| {
            leaf.leaf_kind == PathwayLeafKind::Q5ShiftEvent
                && leaf
                    .evidence
                    .active_skill_ids
                    .contains(&"skill:api".to_string())
                && leaf
                    .evidence
                    .higher_ability_ids
                    .contains(&"ability:service_contract".to_string())
                && leaf
                    .evidence
                    .source_membership_keys
                    .contains(&"member:chunk-1".to_string())
        });
    let unknown_signature_present = readback_pathways
        .iter()
        .any(|pathway| pathway.unknown_pathway_signature)
        && readback_pathways.iter().any(|pathway| {
            pathway
                .leaf_chain
                .iter()
                .any(|leaf| leaf.evidence.unknown_signature)
        });
    let q1_live_evidence_preserved = readback_pathways
        .iter()
        .flat_map(|pathway| pathway.leaf_chain.iter())
        .filter(|leaf| leaf.leaf_kind == PathwayLeafKind::Q1ClaimExists)
        .all(|leaf| {
            leaf.evidence
                .accepted_label_ids
                .contains(&"label:api".to_string())
                && leaf
                    .evidence
                    .active_skill_ids
                    .contains(&"skill:api".to_string())
        });
    let q2_pass_live_evidence_preserved = readback_pathways
        .iter()
        .flat_map(|pathway| pathway.leaf_chain.iter())
        .filter(|leaf| {
            leaf.leaf_kind == PathwayLeafKind::Q2OraclePass
                && leaf.predicted_outcome == PathwayLeafOutcome::Pass
        })
        .all(|leaf| {
            leaf.evidence
                .source_membership_keys
                .contains(&"member:chunk-1".to_string())
                && leaf.evidence.skill_signature_hash.as_deref() == Some("skills:api-contract")
        });
    let calibrated_non_cold_leaves_have_rows =
        report.leaf_calibration_references.iter().any(|reference| {
            reference.leaf_id == "q2:oracle:pass"
                && reference.calibration_source_cf == "CF_MEJEPA_HEAD_CALIBRATIONS"
        }) && readback_pathways
            .iter()
            .flat_map(|pathway| pathway.leaf_chain.iter())
            .filter(|leaf| !leaf.cold_cell_warning)
            .all(|leaf| {
                report
                    .leaf_calibration_references
                    .iter()
                    .any(|reference| reference.leaf_id == leaf.leaf_id)
            });
    let calibration_missing_fails_closed = calibration_missing_fails_closed();
    let credit_assignment = readback_pathways
        .iter()
        .find(|pathway| {
            pathway
                .leaf_chain
                .iter()
                .any(|leaf| leaf.leaf_id == "q2:oracle:fail")
        })
        .map(|pathway| {
            pathway_leaf_credit_assignment(pathway, "q2:oracle:fail", PathwayLeafOutcome::Pass)
        })
        .transpose()?
        .ok_or("missing q2 fail pathway for credit assignment")?;
    let pathway_credit_assignment_preserves_context = credit_assignment
        .active_skill_ids
        .contains(&"skill:api".to_string())
        && credit_assignment
            .higher_ability_ids
            .contains(&"ability:service_contract".to_string())
        && credit_assignment
            .source_membership_keys
            .contains(&"member:chunk-1".to_string())
        && credit_assignment.skill_signature_hash.as_deref() == Some("skills:api-contract")
        && credit_assignment.predicted_outcome == PathwayLeafOutcome::Fail
        && credit_assignment.observed_outcome == PathwayLeafOutcome::Pass;
    let top_k_normalized = (normalized_sum - 1.0).abs() <= 1.0e-4;
    let byte_replay_pass = readback_tree == report.tree
        && readback_pathways == report.surfaced_pathways
        && readback_choices == vec![choice.clone()];
    let choice_idempotent = choice_first_inserted && !choice_second_inserted;
    let all_passed = no_ambiguous_leaves
        && q5_skill_attachment_preserved
        && q1_live_evidence_preserved
        && q2_pass_live_evidence_preserved
        && calibrated_non_cold_leaves_have_rows
        && calibration_missing_fails_closed
        && pathway_credit_assignment_preserves_context
        && unknown_signature_present
        && top_k_normalized
        && byte_replay_pass
        && choice_idempotent
        && ambiguous_edge
        && missing_cf_edge;

    let evidence = json!({
        "artifactKind": "task_py_g_119_pathway_surface_fsv",
        "schemaVersion": PATHWAY_SCHEMA_VERSION,
        "coveredGithubIssues": [416],
        "allPassed": all_passed,
        "sourceOfTruth": {
            "dbPath": db_path,
            "pathwayTreesCf": CF_MEJEPA_PATHWAY_TREES,
            "surfacedPathwaysCf": CF_MEJEPA_SURFACED_PATHWAYS,
            "operatorPathwayChoicesCf": CF_MEJEPA_OPERATOR_PATHWAY_CHOICES
        },
        "checks": {
            "ambiguousLeavesInPathway": readback_tree.ambiguous_leaves_in_pathway,
            "noAmbiguousLeaves": no_ambiguous_leaves,
            "ambiguousLeafRejected": ambiguous_edge,
            "ambiguousLeafRejectCode": PATHWAY_AMBIGUOUS_LEAF_REJECTED,
            "topKProbabilitySum": normalized_sum,
            "topKNormalized": top_k_normalized,
            "byteReplayPass": byte_replay_pass,
            "surfacedPathwayCount": readback_pathways.len(),
            "generatedBranchCount": readback_tree.generated_branch_count,
            "q5SkillAttachmentPreserved": q5_skill_attachment_preserved,
            "q1LiveEvidencePreserved": q1_live_evidence_preserved,
            "q2PassLiveEvidencePreserved": q2_pass_live_evidence_preserved,
            "calibratedNonColdLeavesHaveRows": calibrated_non_cold_leaves_have_rows,
            "calibrationMissingFailsClosed": calibration_missing_fails_closed,
            "pathwayCreditAssignmentPreservesContext": pathway_credit_assignment_preserves_context,
            "unknownSignaturePresent": unknown_signature_present,
            "unknownPathwaySignature": UNKNOWN_PATHWAY_SIGNATURE,
            "choiceFirstInserted": choice_first_inserted,
            "choiceSecondInserted": choice_second_inserted,
            "choiceIdempotent": choice_idempotent,
            "missingCfFailsClosed": missing_cf_edge
        },
        "cfCounts": cf_counts,
        "tree": readback_tree,
        "surfacedPathways": readback_pathways,
        "leafCalibrationReferences": report.leaf_calibration_references,
        "sampleCreditAssignment": credit_assignment,
        "operatorChoices": readback_choices,
        "activePathPolicy": {
            "mntDActiveReadsOrWrites": false
        }
    });
    let report_path = fsv_root.join("pathway_surface_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&report_path, &evidence)?;
    let report_bytes = std::fs::read(&report_path)?;
    let readback: serde_json::Value = serde_json::from_slice(&report_bytes)?;
    let readback_path = fsv_root.join("pathway_surface_readback.json");
    context_graph_mejepa::eval::report::write_json_0600(
        &readback_path,
        &json!({
            "reportPath": report_path,
            "reportSha256": sha256_hex(&report_bytes),
            "readbackAllPassed": readback["allPassed"] == true,
            "readbackNoAmbiguousLeaves": readback["checks"]["noAmbiguousLeaves"] == true,
            "readbackTopKNormalized": readback["checks"]["topKNormalized"] == true,
            "readbackByteReplayPass": readback["checks"]["byteReplayPass"] == true
        }),
    )?;
    if readback["allPassed"] != true {
        return Err(format!("TASK-PY-G-119 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn fixture_input() -> PathwaySurfaceInput {
    let evidence = PathwayLeafEvidence {
        accepted_label_ids: vec!["label:api".to_string()],
        active_skill_ids: vec!["skill:api".to_string()],
        higher_ability_ids: vec!["ability:service_contract".to_string()],
        source_membership_keys: vec!["member:chunk-1".to_string()],
        skill_signature_hash: Some("skills:api-contract".to_string()),
        closest_historical_pathway_id: Some("historical:pathway:api".to_string()),
        unknown_signature: false,
    };
    PathwaySurfaceInput {
        prediction_id_hex: PREDICTION_ID.to_string(),
        candidate_patch_sha256: PATCH_SHA.to_string(),
        q1_claim_exists_probability: 0.9,
        q1_conformal_interval: [0.82, 0.95],
        q1_claim_evidence: evidence.clone(),
        q2_oracle_pass_probability: 0.7,
        q2_conformal_interval: [0.61, 0.77],
        q2_pass_evidence: evidence.clone(),
        q2_fail_evidence: evidence.clone(),
        q5_events: vec![
            Q5PathwayEventInput {
                event_id: "event:test_isolation".to_string(),
                event_label: "test_isolation_holds".to_string(),
                occurred_probability: 0.8,
                conformal_interval: [0.72, 0.86],
                cold_cell_warning: false,
                evidence: evidence.clone(),
            },
            Q5PathwayEventInput {
                event_id: "event:admin_save".to_string(),
                event_label: "admin_save_consumer_ok".to_string(),
                occurred_probability: 0.6,
                conformal_interval: [0.50, 0.68],
                cold_cell_warning: true,
                evidence: PathwayLeafEvidence::unknown(),
            },
        ],
        leaf_calibration_references: fixture_calibration_references(),
        require_non_cold_calibration: true,
        top_k: 5,
        prune_epsilon: 0.01,
        created_at_unix_ms: NOW,
    }
}

fn fixture_calibration_references() -> Vec<PathwayLeafCalibrationReference> {
    vec![
        PathwayLeafCalibrationReference {
            leaf_id: "q1:claim_exists:yes".to_string(),
            calibration_source_cf: "CF_MEJEPA_HEAD_CALIBRATIONS".to_string(),
            calibration_row_key: "head:q1:claim_exists".to_string(),
        },
        PathwayLeafCalibrationReference {
            leaf_id: "q1:claim_exists:no".to_string(),
            calibration_source_cf: "CF_MEJEPA_HEAD_CALIBRATIONS".to_string(),
            calibration_row_key: "head:q1:claim_exists".to_string(),
        },
        PathwayLeafCalibrationReference {
            leaf_id: "q2:oracle:pass".to_string(),
            calibration_source_cf: "CF_MEJEPA_HEAD_CALIBRATIONS".to_string(),
            calibration_row_key: "head:q2:oracle_pass".to_string(),
        },
        PathwayLeafCalibrationReference {
            leaf_id: "q2:oracle:fail".to_string(),
            calibration_source_cf: "CF_MEJEPA_HEAD_CALIBRATIONS".to_string(),
            calibration_row_key: "head:q2:oracle_pass".to_string(),
        },
        PathwayLeafCalibrationReference {
            leaf_id: "q5:event:test_isolation:yes".to_string(),
            calibration_source_cf: "CF_MEJEPA_Q5_CALIBRATIONS".to_string(),
            calibration_row_key: "q5:event:test_isolation".to_string(),
        },
        PathwayLeafCalibrationReference {
            leaf_id: "q5:event:test_isolation:no".to_string(),
            calibration_source_cf: "CF_MEJEPA_Q5_CALIBRATIONS".to_string(),
            calibration_row_key: "q5:event:test_isolation".to_string(),
        },
    ]
}

fn calibration_missing_fails_closed() -> bool {
    let mut input = fixture_input();
    input.leaf_calibration_references.retain(|reference| {
        reference.leaf_id != "q2:oracle:pass" && reference.leaf_id != "q2:oracle:fail"
    });
    surface_pathways(input)
        .unwrap_err()
        .to_string()
        .contains("missing calibration row reference")
}

fn ambiguous_leaf_error() -> bool {
    let leaf = PathwayLeaf {
        leaf_id: "q4:ambiguous".to_string(),
        leaf_kind: PathwayLeafKind::Ambiguous,
        predicted_outcome: PathwayLeafOutcome::Yes,
        predicted_probability: 0.5,
        conformal_interval: [0.4, 0.6],
        event_id: None,
        event_label: Some("ambiguous".to_string()),
        cold_cell_warning: false,
        evidence: PathwayLeafEvidence::unknown(),
    };
    reject_ambiguous_leaf(&leaf)
        .unwrap_err()
        .to_string()
        .contains(PATHWAY_AMBIGUOUS_LEAF_REJECTED)
}

fn missing_cf_error(tree: &context_graph_mejepa::PathwayTreeRecord) -> FsvResult<bool> {
    let dir = env::temp_dir().join(format!(
        "contextgraph-pathway-missing-cf-{}",
        std::process::id()
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    let mut opts = Options::default();
    opts.create_if_missing(true);
    let db = DB::open(&opts, &dir)?;
    let result = context_graph_mejepa::write_pathway_tree(&db, tree);
    drop(db);
    std::fs::remove_dir_all(&dir)?;
    Ok(result
        .unwrap_err()
        .to_string()
        .contains("missing column family"))
}

fn open_db(path: &Path, create_if_missing: bool) -> FsvResult<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(create_if_missing);
    opts.create_missing_column_families(create_if_missing);
    let descriptors = vec![
        ColumnFamilyDescriptor::new("default", Options::default()),
        ColumnFamilyDescriptor::new(CF_MEJEPA_PATHWAY_TREES, Options::default()),
        ColumnFamilyDescriptor::new(CF_MEJEPA_SURFACED_PATHWAYS, Options::default()),
        ColumnFamilyDescriptor::new(CF_MEJEPA_OPERATOR_PATHWAY_CHOICES, Options::default()),
    ];
    Ok(DB::open_cf_descriptors(&opts, path, descriptors)?)
}

fn flush_cfs(db: &DB) -> FsvResult<()> {
    for cf_name in [
        CF_MEJEPA_PATHWAY_TREES,
        CF_MEJEPA_SURFACED_PATHWAYS,
        CF_MEJEPA_OPERATOR_PATHWAY_CHOICES,
    ] {
        let cf = db
            .cf_handle(cf_name)
            .ok_or_else(|| format!("missing CF {cf_name}"))?;
        db.flush_cf(cf)?;
    }
    Ok(())
}

fn cf_counts(db: &DB) -> FsvResult<serde_json::Value> {
    let mut counts = serde_json::Map::new();
    for cf_name in [
        CF_MEJEPA_PATHWAY_TREES,
        CF_MEJEPA_SURFACED_PATHWAYS,
        CF_MEJEPA_OPERATOR_PATHWAY_CHOICES,
    ] {
        let cf = db
            .cf_handle(cf_name)
            .ok_or_else(|| format!("missing CF {cf_name}"))?;
        counts.insert(
            cf_name.to_string(),
            json!(db.iterator_cf(cf, IteratorMode::Start).count()),
        );
    }
    Ok(serde_json::Value::Object(counts))
}

fn arg_path(flag: &str) -> Option<PathBuf> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn reject_mntd_root(path: &Path) -> FsvResult<()> {
    let value = path.display().to_string();
    if value == "/mnt/d" || value.starts_with("/mnt/d/") {
        return Err("TASK-PY-G-119 FSV root must not use legacy /mnt/d".into());
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
