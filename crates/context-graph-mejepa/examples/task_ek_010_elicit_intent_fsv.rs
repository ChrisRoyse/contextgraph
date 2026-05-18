//! TASK-EK-010 north-star intent elicitation FSV.
//!
//! Proves the deterministic base surface for operator-specified teleological
//! vectors: four IntentSpecification modes, CF_MEJEPA_NORTH_STARS persistence,
//! fresh-process readback, alignment ordering, and fail-closed edge cases.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    align_to_intent, count_cf, elicit_intent, open_infer_rocksdb, read_north_star,
    write_north_star_sync_readback, AlignmentReport, ElicitationOptions, IntentCandidate,
    IntentSpecification, InteractiveRefinementRound, NorthStarVector, RawIntentInput,
    MEJEPA_INTENT_EMPTY_POSITIVES, MEJEPA_INTENT_NEGATIVES_OVERLAP_POSITIVES,
};
use context_graph_mejepa_cf::CF_MEJEPA_NORTH_STARS;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-010-elicit-intent-fsv";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModeEvidence {
    mode: String,
    intent_id: String,
    embedder_count: usize,
    positive_candidate_cosine: f32,
    negative_candidate_cosine: Option<f32>,
    passed_alignment: bool,
    source_of_truth_cf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskEk010Fsv {
    output_path: String,
    db_path: String,
    started_at_unix_ms: i64,
    north_star_cf: String,
    mode_evidence: Vec<ModeEvidence>,
    persisted_intent_id: String,
    cf_row_count_after_reopen: u64,
    readback_equal: bool,
    fresh_process_alignment_equal: bool,
    low_diversity_warning: bool,
    interactive_unconverged: bool,
    empty_positive_fails_closed: bool,
    overlap_fails_closed: bool,
    synthetic_customer_service_equivalent: bool,
    citation_block: Vec<String>,
    all_passed: bool,
    json_readback_equal: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    fs::create_dir_all(&run_root).context("create FSV run root")?;
    let db_path = run_root.join("rocksdb");
    let output_path = run_root.join("elicit_intent_fsv.json");

    let options = ElicitationOptions::default();
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;

    let free_text = elicit_intent(
        IntentSpecification::FreeText {
            description: "Always answer the user in fewer than three concise sentences."
                .to_string(),
        },
        options.clone(),
        started_at_unix_ms,
    )?;
    write_north_star_sync_readback(db.as_ref(), &free_text)?;
    let free_positive = align_to_intent(
        &free_text,
        IntentCandidate::RawInput {
            candidate_id: "free-positive".to_string(),
            text: "Always answer the user in fewer than three concise sentences.".to_string(),
        },
    )?;
    let free_negative = align_to_intent(
        &free_text,
        IntentCandidate::RawInput {
            candidate_id: "free-negative".to_string(),
            text: "Respond with a long essay and several unrelated tangents.".to_string(),
        },
    )?;

    let exemplars = elicit_intent(
        IntentSpecification::PositiveAndNegativeExemplars {
            positive: vec![
                RawIntentInput::try_new(
                    "customer-positive-1",
                    "I understand the issue, I own the follow-up, and the next step is clear.",
                )?,
                RawIntentInput::try_new(
                    "customer-positive-2",
                    "The customer leaves confident that their issue will be resolved.",
                )?,
            ],
            negative: vec![RawIntentInput::try_new(
                "customer-negative-1",
                "The agent deflects ownership and gives no next step.",
            )?],
        },
        options.clone(),
        started_at_unix_ms + 1,
    )?;
    let exemplar_positive = align_to_intent(
        &exemplars,
        IntentCandidate::VectorMap {
            candidate_id: "customer-positive-candidate".to_string(),
            vectors: exemplars.c_intent.clone(),
        },
    )?;
    let exemplar_negative = align_to_intent(
        &exemplars,
        IntentCandidate::RawInput {
            candidate_id: "customer-negative-candidate".to_string(),
            text: "The agent deflects ownership and gives no next step.".to_string(),
        },
    )?;

    let constellation = elicit_intent(
        IntentSpecification::ConstellationOfOutcome {
            subject: "desired_outcome".to_string(),
            references: (0..10)
                .map(|idx| {
                    RawIntentInput::try_new(
                        format!("reference-{idx}"),
                        format!("Reference {idx}: concise, correct, safe engineering outcome."),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        options.clone(),
        started_at_unix_ms + 2,
    )?;
    let constellation_positive = align_to_intent(
        &constellation,
        IntentCandidate::VectorMap {
            candidate_id: "constellation-positive".to_string(),
            vectors: constellation.c_intent.clone(),
        },
    )?;

    let interactive = elicit_intent(
        IntentSpecification::InteractiveRefinement {
            initial: Box::new(IntentSpecification::FreeText {
                description: "Prefer short answers.".to_string(),
            }),
            rounds: vec![InteractiveRefinementRound {
                round_id: "round-1".to_string(),
                critique: "Also require explicit ownership and a concrete next step.".to_string(),
                accepted: true,
            }],
            max_rounds: 1,
        },
        options.clone(),
        started_at_unix_ms + 3,
    )?;
    let interactive_positive = align_to_intent(
        &interactive,
        IntentCandidate::VectorMap {
            candidate_id: "interactive-positive".to_string(),
            vectors: interactive.c_intent.clone(),
        },
    )?;

    drop(db);
    let reopened = open_infer_rocksdb(&db_path).context("reopen inference RocksDB")?;
    let persisted_readback = read_north_star(reopened.as_ref(), &free_text.intent_id)?
        .context("persisted north-star missing")?;
    let fresh_alignment = align_to_intent(
        &persisted_readback,
        IntentCandidate::RawInput {
            candidate_id: "free-positive".to_string(),
            text: "Always answer the user in fewer than three concise sentences.".to_string(),
        },
    )?;
    let fresh_process_alignment_equal = alignment_close(&free_positive, &fresh_alignment);
    let cf_row_count_after_reopen = count_cf(reopened.as_ref(), CF_MEJEPA_NORTH_STARS)?;

    let low_diversity = elicit_intent(
        IntentSpecification::PositiveAndNegativeExemplars {
            positive: vec![
                RawIntentInput::try_new("dup-1", "identical north-star input")?,
                RawIntentInput::try_new("dup-2", "identical north-star input")?,
            ],
            negative: Vec::new(),
        },
        options,
        started_at_unix_ms + 4,
    )?;

    let empty_positive_fails_closed = elicit_intent(
        IntentSpecification::PositiveAndNegativeExemplars {
            positive: Vec::new(),
            negative: Vec::new(),
        },
        ElicitationOptions::default(),
        started_at_unix_ms + 5,
    )
    .err()
    .map(|err| err.to_string().contains(MEJEPA_INTENT_EMPTY_POSITIVES))
    .unwrap_or(false);

    let overlap_fails_closed = elicit_intent(
        IntentSpecification::PositiveAndNegativeExemplars {
            positive: vec![RawIntentInput::try_new("same-positive", "same target")?],
            negative: vec![RawIntentInput::try_new("same-negative", "same target")?],
        },
        ElicitationOptions::default(),
        started_at_unix_ms + 6,
    )
    .err()
    .map(|err| {
        err.to_string()
            .contains(MEJEPA_INTENT_NEGATIVES_OVERLAP_POSITIVES)
    })
    .unwrap_or(false);

    let mode_evidence = vec![
        mode_evidence(
            "free_text",
            &free_text,
            &free_positive,
            Some(&free_negative),
        ),
        mode_evidence(
            "positive_and_negative_exemplars",
            &exemplars,
            &exemplar_positive,
            Some(&exemplar_negative),
        ),
        mode_evidence(
            "constellation_of_outcome",
            &constellation,
            &constellation_positive,
            None,
        ),
        mode_evidence(
            "interactive_refinement",
            &interactive,
            &interactive_positive,
            None,
        ),
    ];

    let all_passed = mode_evidence.len() == 4
        && mode_evidence.iter().all(|row| row.passed_alignment)
        && free_positive.mean_cosine > free_negative.mean_cosine
        && exemplar_positive.mean_cosine > exemplar_negative.mean_cosine
        && persisted_readback == free_text
        && fresh_process_alignment_equal
        && cf_row_count_after_reopen == 1
        && low_diversity.low_diversity_warning
        && interactive.unconverged
        && empty_positive_fails_closed
        && overlap_fails_closed;

    let mut report = TaskEk010Fsv {
        output_path: output_path.display().to_string(),
        db_path: db_path.display().to_string(),
        started_at_unix_ms,
        north_star_cf: CF_MEJEPA_NORTH_STARS.to_string(),
        mode_evidence,
        persisted_intent_id: free_text.intent_id.clone(),
        cf_row_count_after_reopen,
        readback_equal: persisted_readback == free_text,
        fresh_process_alignment_equal,
        low_diversity_warning: low_diversity.low_diversity_warning,
        interactive_unconverged: interactive.unconverged,
        empty_positive_fails_closed,
        overlap_fails_closed,
        synthetic_customer_service_equivalent: true,
        citation_block: vec![
            "Royse 2026, The Symmetry of Knowing, partial repo source docs/the_symmetry_of_knowing_partial.md".to_string(),
            "Li and Klabjan 2024, computational reversibility measurement cited by Royse abstract".to_string(),
            "Operator teleological-vectors dissertation, represented in repo by TASK-EK-010 issue #332 and docs/explanatory_knowledge_and_mejepaupdated.md section 10.2(VI)".to_string(),
            "Synthetic equivalent used because the 1,000-transcript customer-service dataset is not recoverable in this repo snapshot".to_string(),
        ],
        all_passed,
        json_readback_equal: false,
    };
    write_json_readback(&output_path, &mut report)?;
    println!("{}", output_path.display());
    if !report.all_passed || !report.json_readback_equal {
        bail!("TASK-EK-010 elicit-intent FSV failed");
    }
    Ok(())
}

fn mode_evidence(
    mode: &str,
    north_star: &NorthStarVector,
    positive: &AlignmentReport,
    negative: Option<&AlignmentReport>,
) -> ModeEvidence {
    ModeEvidence {
        mode: mode.to_string(),
        intent_id: north_star.intent_id.clone(),
        embedder_count: north_star.c_intent.len(),
        positive_candidate_cosine: positive.mean_cosine,
        negative_candidate_cosine: negative.map(|report| report.mean_cosine),
        passed_alignment: positive.passed,
        source_of_truth_cf: north_star.source_of_truth_cf.clone(),
    }
}

fn alignment_close(lhs: &AlignmentReport, rhs: &AlignmentReport) -> bool {
    lhs.intent_id == rhs.intent_id
        && lhs.candidate_id == rhs.candidate_id
        && lhs.passed == rhs.passed
        && (lhs.mean_cosine - rhs.mean_cosine).abs() < 1e-6
        && lhs.per_embedder.len() == rhs.per_embedder.len()
}

fn write_json_readback(path: &Path, report: &mut TaskEk010Fsv) -> Result<()> {
    report.json_readback_equal = false;
    let first = serde_json::to_vec_pretty(report)?;
    fs::write(path, &first).with_context(|| format!("write {}", path.display()))?;
    let readback: Value = serde_json::from_slice(&fs::read(path)?)?;
    let expected: Value = serde_json::from_slice(&first)?;
    report.json_readback_equal = readback == expected;
    let final_bytes = serde_json::to_vec_pretty(report)?;
    fs::write(path, &final_bytes).with_context(|| format!("rewrite {}", path.display()))?;
    let final_readback: TaskEk010Fsv = serde_json::from_slice(&fs::read(path)?)?;
    if final_readback != *report {
        bail!("FSV JSON readback mismatch");
    }
    Ok(())
}

fn unix_ms() -> Result<i64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX epoch")?
        .as_millis();
    i64::try_from(millis).context("unix millis overflowed i64")
}
