use super::*;

fn source(row: &str, prose: &str, outcome: Q4ReasoningOutcome) -> Q4ReasoningSource {
    Q4ReasoningSource {
        corpus_row_id: row.to_string(),
        chunk_id: "chunk-reasoning".to_string(),
        session_id: format!("session-{row}"),
        agent_prose: prose.to_string(),
        oracle_outcome: outcome,
        prediction_verdict: if outcome == Q4ReasoningOutcome::Pass {
            Q4ReasoningPredictionVerdict::Pass
        } else {
            Q4ReasoningPredictionVerdict::Fail
        },
        pairwise_cosine_reasoning_diff: 0.82,
        pairwise_cosine_reasoning_oracle: if outcome == Q4ReasoningOutcome::Pass {
            0.88
        } else {
            0.12
        },
        e17_affect_score: 0.55,
    }
}

#[test]
fn classifies_operational_reasoning_edges() {
    let rows = vec![
        source("row-empty", "", Q4ReasoningOutcome::Pass),
        source(
            "row-code",
            "```\ndef patched():\n    return True\n```",
            Q4ReasoningOutcome::Pass,
        ),
        source(
            "row-unsupported",
            "El codigo funciona porque la prueba paso.",
            Q4ReasoningOutcome::Pass,
        ),
        source(
            "row-hedge",
            "I think this should fix the test, but I am not sure.",
            Q4ReasoningOutcome::Pass,
        ),
        source(
            "row-overclaim",
            "This is definitely fixed and all tests pass.",
            Q4ReasoningOutcome::Fail,
        ),
    ];
    let extraction = extract_q4_reasoning_labels(&rows).unwrap();
    let classes = extraction
        .labels
        .iter()
        .map(|label| label.class)
        .collect::<Vec<_>>();
    assert!(classes.contains(&Q4ReasoningClass::None));
    assert!(classes.contains(&Q4ReasoningClass::CodeOnly));
    assert!(classes.contains(&Q4ReasoningClass::Unsupported));
    assert!(classes.contains(&Q4ReasoningClass::Hedging));
    assert!(classes.contains(&Q4ReasoningClass::Overclaiming));
}

#[test]
fn small_harvest_fails_closed_to_unknown() {
    let rows = vec![source(
        "row-small",
        "This is definitely fixed and all tests pass.",
        Q4ReasoningOutcome::Fail,
    )];
    let extraction = extract_q4_reasoning_labels(&rows).unwrap();
    let checkpoint = train_q4_reasoning_head(&extraction.labels).unwrap();
    assert_eq!(
        checkpoint.status,
        Q4ReasoningHeadStatus::UnknownInsufficientHarvest
    );
    assert_eq!(checkpoint.fallback_class, Q4ReasoningClass::Unknown);
}

#[test]
fn invalid_cosine_rejected_before_labeling() {
    let mut row = source(
        "row-bad-cosine",
        "Verified and fixed.",
        Q4ReasoningOutcome::Pass,
    );
    row.pairwise_cosine_reasoning_oracle = 1.5;
    assert!(extract_q4_reasoning_labels(&[row]).is_err());
}
