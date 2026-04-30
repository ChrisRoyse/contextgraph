use context_graph_core::learning::{
    LearningEvent, LearningOutcome, LearningOutcomeLabel, LearningStateSnapshot,
    MAX_LEARNING_EVENT_LABEL_CHARS, MAX_LEARNING_EVENT_MEMORY_IDS,
};
use context_graph_core::training::NUM_CROSS_CORRELATIONS;
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tempfile::TempDir;
use uuid::Uuid;

fn approx_eq(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-5,
        "expected {expected}, got {actual}"
    );
}

struct StateInput<'a> {
    profile_base: f32,
    rank: Option<u32>,
    contradiction: f32,
    integration: f32,
    recurrence: u32,
    stability: f32,
    domain: &'a str,
    transfer_count: u32,
}

fn state(input: StateInput<'_>) -> LearningStateSnapshot {
    let mut topic_profile = [0.0f32; NUM_EMBEDDERS];
    let mut embedder_scores = [0.0f32; NUM_EMBEDDERS];
    for idx in 0..NUM_EMBEDDERS {
        topic_profile[idx] = input.profile_base;
        embedder_scores[idx] = 0.2 + (idx as f32 / (NUM_EMBEDDERS - 1) as f32) * 0.7;
    }
    LearningStateSnapshot {
        topic_profile,
        cross_correlations: vec![0.1; NUM_CROSS_CORRELATIONS],
        retrieval_rank: input.rank,
        embedder_scores,
        contradiction_pressure: input.contradiction,
        integration_confidence: input.integration,
        recurrence_count: input.recurrence,
        stability_score: input.stability,
        domain: Some(input.domain.into()),
        successful_transfer_count: input.transfer_count,
    }
}

fn outcome(label: LearningOutcomeLabel, utility_delta: f32) -> LearningOutcome {
    LearningOutcome {
        label,
        utility_delta,
        correction_required: false,
        reuse_observed: utility_delta > 0.0,
    }
}

fn happy_event(event_id: Uuid) -> LearningEvent {
    let before = state(StateInput {
        profile_base: 0.2,
        rank: Some(10),
        contradiction: 0.2,
        integration: 0.4,
        recurrence: 1,
        stability: 0.4,
        domain: "docs",
        transfer_count: 0,
    });
    let after = state(StateInput {
        profile_base: 0.3,
        rank: Some(2),
        contradiction: 0.1,
        integration: 0.8,
        recurrence: 8,
        stability: 0.9,
        domain: "code",
        transfer_count: 3,
    });
    LearningEvent::new(
        event_id,
        vec![Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap()],
        Some("learning-fsv".into()),
        Some("response-1".into()),
        Some("task-1".into()),
        "synthetic known input".into(),
        "synthetic context".into(),
        "synthetic response".into(),
        before,
        after,
        outcome(LearningOutcomeLabel::Useful, 0.4),
    )
    .unwrap()
}

#[tokio::test]
async fn happy_path_persists_reopens_reads_and_deletes_learning_event() {
    let tmp = TempDir::new().unwrap();
    let event_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let event = happy_event(event_id);

    println!("SOURCE OF TRUTH: RocksDB CF learning_events");
    println!(
        "BEFORE store: event_id={}, count=0 expected, topic_len={}, cross_len={}",
        event_id,
        event.before.topic_profile.len(),
        event.before.cross_correlations.len()
    );

    {
        let store = RocksDbTeleologicalStore::open_with_config(
            tmp.path(),
            TeleologicalStoreConfig::default(),
        )
        .unwrap();
        let count_before = store.count_learning_events().await.unwrap();
        println!("BEFORE count_learning_events={}", count_before);
        assert_eq!(count_before, 0);
        store.store_learning_event(&event).await.unwrap();
        let immediate_count = store.count_learning_events().await.unwrap();
        println!(
            "AFTER write immediate count_learning_events={}",
            immediate_count
        );
        assert_eq!(immediate_count, 1);
    }

    let store =
        RocksDbTeleologicalStore::open_with_config(tmp.path(), TeleologicalStoreConfig::default())
            .unwrap();
    let ids = store.list_learning_event_ids().await.unwrap();
    let readback = store
        .get_learning_event(event_id)
        .await
        .unwrap()
        .expect("event must exist after reopen");

    println!(
        "AFTER reopen: ids={:?}, readback.event_id={}, delta_e_scalar={}, rank_shift={}, surprise={}, coherence={}, consolidation={}, transfer={}",
        ids,
        readback.event_id,
        readback.features.delta_e_scalar,
        readback.features.retrieval_rank_shift,
        readback.features.surprise_score,
        readback.features.coherence_delta,
        readback.features.consolidation_readiness,
        readback.features.transfer_score
    );

    assert_eq!(ids, vec![event_id]);
    assert_eq!(readback.before.topic_profile.len(), NUM_EMBEDDERS);
    assert_eq!(
        readback.before.cross_correlations.len(),
        NUM_CROSS_CORRELATIONS
    );
    assert_eq!(readback.signals.len(), 5);
    approx_eq(readback.features.delta_e_scalar, 0.1);
    approx_eq(readback.features.retrieval_rank_shift, 0.8);
    approx_eq(readback.features.embedder_disagreement, 0.7);
    approx_eq(readback.features.surprise_score, 0.75);
    approx_eq(readback.features.coherence_delta, 0.4);
    approx_eq(readback.features.consolidation_readiness, 0.85);
    approx_eq(readback.features.transfer_score, 0.35);

    let deleted = store.delete_learning_event(event_id).await.unwrap();
    let after_delete = store.get_learning_event(event_id).await.unwrap();
    let final_count = store.count_learning_events().await.unwrap();
    println!(
        "AFTER delete: deleted={}, readback_exists={}, count={}",
        deleted,
        after_delete.is_some(),
        final_count
    );
    assert!(deleted);
    assert!(after_delete.is_none());
    assert_eq!(final_count, 0);
}

#[tokio::test]
async fn edge_empty_event_stream_and_no_state_change_stores_zero_delta() {
    let tmp = TempDir::new().unwrap();
    let store =
        RocksDbTeleologicalStore::open_with_config(tmp.path(), TeleologicalStoreConfig::default())
            .unwrap();
    let event_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let before = state(StateInput {
        profile_base: 0.2,
        rank: None,
        contradiction: 0.0,
        integration: 0.0,
        recurrence: 0,
        stability: 0.0,
        domain: "docs",
        transfer_count: 0,
    });
    let after = before.clone();

    println!(
        "EDGE empty/no-change BEFORE count={}",
        store.count_learning_events().await.unwrap()
    );
    let event = LearningEvent::new(
        event_id,
        Vec::new(),
        None,
        None,
        None,
        String::new(),
        String::new(),
        String::new(),
        before,
        after,
        outcome(LearningOutcomeLabel::NoLearning, 0.0),
    )
    .unwrap();
    store.store_learning_event(&event).await.unwrap();
    let readback = store.get_learning_event(event_id).await.unwrap().unwrap();
    println!(
        "EDGE empty/no-change AFTER count={}, memory_ids={}, query_len={}, delta_e_scalar={}, surprise={}",
        store.count_learning_events().await.unwrap(),
        readback.memory_ids.len(),
        readback.query.len(),
        readback.features.delta_e_scalar,
        readback.features.surprise_score
    );
    assert!(readback.memory_ids.is_empty());
    assert!(readback.query.is_empty());
    approx_eq(readback.features.delta_e_scalar, 0.0);
    approx_eq(readback.features.surprise_score, 0.35);
}

#[test]
fn edge_invalid_cross_correlation_dimension_fails_before_storage() {
    let mut before = state(StateInput {
        profile_base: 0.2,
        rank: Some(10),
        contradiction: 0.0,
        integration: 0.0,
        recurrence: 0,
        stability: 0.0,
        domain: "docs",
        transfer_count: 0,
    });
    before.cross_correlations.pop();
    let after = state(StateInput {
        profile_base: 0.3,
        rank: Some(2),
        contradiction: 0.0,
        integration: 0.0,
        recurrence: 0,
        stability: 0.0,
        domain: "docs",
        transfer_count: 0,
    });
    println!(
        "EDGE invalid dimension BEFORE before_cross_len={}, expected={}",
        before.cross_correlations.len(),
        NUM_CROSS_CORRELATIONS
    );
    let result = LearningEvent::new(
        Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        Vec::new(),
        None,
        None,
        None,
        "q".into(),
        String::new(),
        String::new(),
        before,
        after,
        outcome(LearningOutcomeLabel::Neutral, 0.0),
    );
    println!(
        "EDGE invalid dimension AFTER result={:?}",
        result.as_ref().err()
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn edge_maximum_memory_ids_and_label_lengths_persist() {
    let tmp = TempDir::new().unwrap();
    let store =
        RocksDbTeleologicalStore::open_with_config(tmp.path(), TeleologicalStoreConfig::default())
            .unwrap();
    let event_id = Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap();
    let memory_ids: Vec<Uuid> = (0..MAX_LEARNING_EVENT_MEMORY_IDS)
        .map(|_| Uuid::new_v4())
        .collect();
    let label = "s".repeat(MAX_LEARNING_EVENT_LABEL_CHARS);
    println!(
        "EDGE maximum BEFORE count={}, memory_ids={}, label_len={}",
        store.count_learning_events().await.unwrap(),
        memory_ids.len(),
        label.len()
    );
    let event = LearningEvent::new(
        event_id,
        memory_ids,
        Some(label),
        None,
        None,
        "q".into(),
        String::new(),
        String::new(),
        state(StateInput {
            profile_base: 0.2,
            rank: Some(5),
            contradiction: 0.0,
            integration: 0.5,
            recurrence: 10,
            stability: 1.0,
            domain: "docs",
            transfer_count: 0,
        }),
        state(StateInput {
            profile_base: 0.3,
            rank: Some(1),
            contradiction: 0.0,
            integration: 1.0,
            recurrence: 10,
            stability: 1.0,
            domain: "docs",
            transfer_count: 0,
        }),
        outcome(LearningOutcomeLabel::Useful, 1.0),
    )
    .unwrap();
    store.store_learning_event(&event).await.unwrap();
    let readback = store.get_learning_event(event_id).await.unwrap().unwrap();
    println!(
        "EDGE maximum AFTER count={}, memory_ids={}, session_len={}",
        store.count_learning_events().await.unwrap(),
        readback.memory_ids.len(),
        readback.session_id.as_ref().unwrap().len()
    );
    assert_eq!(readback.memory_ids.len(), MAX_LEARNING_EVENT_MEMORY_IDS);
    assert_eq!(
        readback.session_id.as_ref().unwrap().len(),
        MAX_LEARNING_EVENT_LABEL_CHARS
    );
}
