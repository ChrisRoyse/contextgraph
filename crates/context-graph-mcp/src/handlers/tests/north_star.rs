//! North Star Integration Tests
//!
//! TASK-CORE-001: Updated tests after removing manual North Star methods per ARCH-03.
//!
//! ## Removed Methods (return METHOD_NOT_FOUND -32601):
//! - `purpose/north_star_alignment` - Use auto_bootstrap_north_star instead
//! - `purpose/north_star_update` - Use auto_bootstrap_north_star instead
//!
//! ## Still Available Methods:
//! - `goal/hierarchy_query` - Navigate goal hierarchy (North Star can be set programmatically)
//! - `purpose/drift_check` - Check alignment drift (requires North Star in hierarchy)
//! - `memory/store` - Store memories (works with or without North Star)
//!
//! ## Full State Verification (FSV)
//!
//! Each test directly inspects the GoalHierarchy to verify actual state changes.
//! Handler responses are cross-verified against the Source of Truth.

use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::purpose::{GoalHierarchy, GoalId, GoalLevel, GoalNode};
use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor};
use context_graph_core::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor,
};

use crate::handlers::Handlers;
use crate::protocol::JsonRpcId;

use super::make_request;

// =============================================================================
// Test Helper Functions
// =============================================================================

/// Create handlers with an EMPTY goal hierarchy (no North Star).
/// Returns handlers and shared hierarchy for FSV assertions.
fn create_handlers_no_north_star() -> (
    Handlers,
    Arc<InMemoryTeleologicalStore>,
    Arc<RwLock<GoalHierarchy>>,
) {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());

    // Empty hierarchy - no North Star
    let hierarchy = GoalHierarchy::new();
    let shared_hierarchy = Arc::new(RwLock::new(hierarchy));

    let store_for_handlers: Arc<dyn TeleologicalMemoryStore> = store.clone();
    let handlers = Handlers::with_shared_hierarchy(
        store_for_handlers,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        shared_hierarchy.clone(),
    );

    (handlers, store, shared_hierarchy)
}

/// Create handlers WITH an existing North Star.
/// Returns handlers and shared hierarchy for FSV assertions.
fn create_handlers_with_north_star() -> (
    Handlers,
    Arc<InMemoryTeleologicalStore>,
    Arc<RwLock<GoalHierarchy>>,
) {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());

    // Create hierarchy with North Star
    let mut hierarchy = GoalHierarchy::new();

    // Real 1024D embedding (using sin wave pattern for reproducibility)
    let ns_embedding: Vec<f32> = (0..1024)
        .map(|i| (i as f32 / 1024.0 * std::f32::consts::PI * 2.0).sin() * 0.5 + 0.5)
        .collect();

    hierarchy
        .add_goal(GoalNode::north_star(
            "initial_north_star",
            "Build the best AI assistant system",
            ns_embedding,
            vec!["ai".into(), "assistant".into(), "system".into()],
        ))
        .expect("Failed to add initial North Star");

    let shared_hierarchy = Arc::new(RwLock::new(hierarchy));

    let store_for_handlers: Arc<dyn TeleologicalMemoryStore> = store.clone();
    let handlers = Handlers::with_shared_hierarchy(
        store_for_handlers,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        shared_hierarchy.clone(),
    );

    (handlers, store, shared_hierarchy)
}

/// Create a full test hierarchy with multiple levels.
fn create_handlers_with_full_hierarchy() -> (
    Handlers,
    Arc<InMemoryTeleologicalStore>,
    Arc<RwLock<GoalHierarchy>>,
) {
    let store = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());

    let mut hierarchy = GoalHierarchy::new();

    // Real 1024D embedding
    let embedding: Vec<f32> = (0..1024)
        .map(|i| (i as f32 / 1024.0).sin() * 0.8)
        .collect();

    // North Star
    hierarchy
        .add_goal(GoalNode::north_star(
            "ns_knowledge",
            "Build comprehensive knowledge system",
            embedding.clone(),
            vec!["knowledge".into(), "learning".into()],
        ))
        .expect("Failed to add North Star");

    // Strategic goal
    hierarchy
        .add_goal(GoalNode::child(
            "s1_retrieval",
            "Improve knowledge retrieval",
            GoalLevel::Strategic,
            GoalId::new("ns_knowledge"),
            embedding.clone(),
            0.8,
            vec!["retrieval".into()],
        ))
        .expect("Failed to add strategic goal");

    // Tactical goal
    hierarchy
        .add_goal(GoalNode::child(
            "t1_semantic",
            "Implement semantic search",
            GoalLevel::Tactical,
            GoalId::new("s1_retrieval"),
            embedding.clone(),
            0.6,
            vec!["semantic".into()],
        ))
        .expect("Failed to add tactical goal");

    // Immediate goal
    hierarchy
        .add_goal(GoalNode::child(
            "i1_vectors",
            "Add vector embeddings",
            GoalLevel::Immediate,
            GoalId::new("t1_semantic"),
            embedding,
            0.5,
            vec!["vectors".into()],
        ))
        .expect("Failed to add immediate goal");

    let shared_hierarchy = Arc::new(RwLock::new(hierarchy));

    let store_for_handlers: Arc<dyn TeleologicalMemoryStore> = store.clone();
    let handlers = Handlers::with_shared_hierarchy(
        store_for_handlers,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        shared_hierarchy.clone(),
    );

    (handlers, store, shared_hierarchy)
}

// =============================================================================
// TASK-CORE-001: Deprecated Method Tests
// =============================================================================

/// TASK-CORE-001: Verify purpose/north_star_update returns METHOD_NOT_FOUND.
///
/// This method was removed per ARCH-03 (autonomous-first architecture).
/// Manual North Star creates single 1024D embeddings incompatible with 13-embedder arrays.
#[tokio::test]
async fn test_north_star_update_returns_method_not_found() {
    let (handlers, _store, _hierarchy) = create_handlers_no_north_star();

    let params = json!({
        "description": "Test North Star",
        "keywords": ["test"],
        "replace": false
    });
    let request = make_request(
        "purpose/north_star_update",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Returns METHOD_NOT_FOUND (-32601) per TASK-CORE-001
    assert!(
        response.error.is_some(),
        "purpose/north_star_update must return error (deprecated)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601), got {}",
        error.code
    );
}

/// TASK-CORE-001: Verify purpose/north_star_alignment returns METHOD_NOT_FOUND.
///
/// This method was removed per ARCH-03 (autonomous-first architecture).
/// Manual alignment uses single 1024D embeddings incompatible with 13-embedder arrays.
#[tokio::test]
async fn test_north_star_alignment_returns_method_not_found() {
    let (handlers, _store, _hierarchy) = create_handlers_with_north_star();

    let params = json!({
        "fingerprint_id": "00000000-0000-0000-0000-000000000001"
    });
    let request = make_request(
        "purpose/north_star_alignment",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Returns METHOD_NOT_FOUND (-32601) per TASK-CORE-001
    assert!(
        response.error.is_some(),
        "purpose/north_star_alignment must return error (deprecated)"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32601,
        "Must return METHOD_NOT_FOUND (-32601), got {}",
        error.code
    );
}

// =============================================================================
// Valid Tests: goal/hierarchy_query (Still Available)
// =============================================================================

/// Test that goal/hierarchy_query returns correct North Star data.
///
/// FSV: Cross-verify returned data matches Source of Truth.
#[tokio::test]
async fn test_get_north_star_returns_data() {
    // SETUP: Handlers with existing North Star
    let (handlers, _store, hierarchy) = create_handlers_with_north_star();

    // Get reference data from Source of Truth
    let expected_description: String;
    let expected_keywords: Vec<String>;
    {
        let h = hierarchy.read();
        let ns = h.north_star().expect("Must have NS");
        expected_description = ns.description.clone();
        expected_keywords = ns.keywords.clone();
    }

    // ACTION: Query goal hierarchy for get_all
    let params = json!({
        "operation": "get_all"
    });
    let request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Response contains North Star data
    assert!(response.error.is_none(), "Get must succeed");
    let result = response.result.expect("Must have result");

    let goals = result
        .get("goals")
        .and_then(|v| v.as_array())
        .expect("Must have goals array");
    assert_eq!(goals.len(), 1, "Must have exactly 1 goal");

    let ns_goal = &goals[0];

    // Cross-verify with Source of Truth
    let returned_description = ns_goal
        .get("description")
        .and_then(|v| v.as_str())
        .expect("Must have description");
    assert_eq!(
        returned_description, expected_description,
        "FSV: Returned description must match Source of Truth"
    );

    let returned_keywords = ns_goal
        .get("keywords")
        .and_then(|v| v.as_array())
        .expect("Must have keywords");
    for kw in &expected_keywords {
        let kw_exists = returned_keywords.iter().any(|v| v.as_str() == Some(kw));
        assert!(kw_exists, "FSV: Keyword '{}' must be in response", kw);
    }

    // Verify is_north_star flag
    let is_ns = ns_goal
        .get("is_north_star")
        .and_then(|v| v.as_bool())
        .expect("Must have is_north_star");
    assert!(is_ns, "FSV: is_north_star must be true");
}

/// Test that purpose/drift_check fails without North Star configured.
///
/// This tests the fail-fast behavior required by AP-007.
#[tokio::test]
async fn test_get_north_star_fails_without_north_star() {
    // SETUP: Empty hierarchy
    let (handlers, _store, hierarchy) = create_handlers_no_north_star();

    // FSV BEFORE: Verify no North Star
    {
        let h = hierarchy.read();
        assert!(!h.has_north_star(), "FSV BEFORE: Must NOT have North Star");
    }

    // ACTION: Try drift_check (requires North Star before fingerprint validation)
    let params = json!({
        "fingerprint_ids": ["00000000-0000-0000-0000-000000000001"]
    });
    let request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Response indicates failure with NORTH_STAR_NOT_CONFIGURED
    assert!(
        response.error.is_some(),
        "Must fail without North Star configured"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32021,
        "Must return NORTH_STAR_NOT_CONFIGURED (-32021)"
    );
    assert!(
        error.message.contains("North Star") || error.message.contains("north_star"),
        "Error message must mention North Star"
    );
}

// =============================================================================
// Valid Tests: memory/store (Works with or without North Star)
// =============================================================================

/// Test that memory/store succeeds autonomously without North Star configured.
///
/// AUTONOMOUS OPERATION: Per contextprd.md, the 13-embedding array IS the
/// teleological vector. Purpose alignment is SECONDARY metadata that defaults
/// to neutral [0.0; 13] when no North Star is configured.
#[tokio::test]
async fn test_store_memory_succeeds_without_north_star() {
    // SETUP: Empty hierarchy (no North Star configured)
    let (handlers, store, hierarchy) = create_handlers_no_north_star();

    // FSV BEFORE: Verify state
    {
        let h = hierarchy.read();
        assert!(!h.has_north_star(), "FSV BEFORE: Must NOT have North Star");
    }
    let before_count = store.count().await.expect("count works");
    assert_eq!(before_count, 0, "FSV BEFORE: Store must be empty");

    // ACTION: Store memory without North Star
    let params = json!({
        "content": "Machine learning enables autonomous improvement",
        "importance": 0.8
    });
    let request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Succeeds with default purpose vector (AUTONOMOUS OPERATION)
    assert!(
        response.error.is_none(),
        "Store MUST succeed without North Star (AUTONOMOUS OPERATION). \
         Error: {:?}",
        response.error
    );
    let result = response.result.expect("Must have result");

    // Verify response contains fingerprint ID
    assert!(
        result.get("fingerprintId").is_some(),
        "Result must contain fingerprintId"
    );

    // FSV AFTER: Memory stored successfully
    let after_count = store.count().await.expect("count works");
    assert_eq!(after_count, 1, "FSV AFTER: Store must have 1 entry");
}

/// Test that memory/store succeeds when North Star is configured.
///
/// FSV: Verify fingerprint stored with correct purpose alignment.
#[tokio::test]
async fn test_store_memory_succeeds_with_north_star() {
    // SETUP: Handlers with North Star
    let (handlers, store, hierarchy) = create_handlers_with_north_star();

    // FSV BEFORE: Verify North Star exists
    {
        let h = hierarchy.read();
        assert!(h.has_north_star(), "FSV BEFORE: Must have North Star");
    }
    let before_count = store.count().await.expect("count works");
    assert_eq!(before_count, 0, "FSV BEFORE: Store must be empty");

    // ACTION: Store memory
    let params = json!({
        "content": "Deep learning neural networks process information hierarchically",
        "importance": 0.9
    });
    let request = make_request(
        "memory/store",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Succeeds
    assert!(
        response.error.is_none(),
        "Store must succeed with North Star: {:?}",
        response.error
    );
    let result = response.result.expect("Must have result");

    // Extract fingerprint ID
    let fingerprint_id = result
        .get("fingerprintId")
        .and_then(|v| v.as_str())
        .expect("Must return fingerprintId");
    let uuid = uuid::Uuid::parse_str(fingerprint_id).expect("Must be valid UUID");

    // FSV AFTER: Verify stored in Source of Truth
    let after_count = store.count().await.expect("count works");
    assert_eq!(after_count, 1, "FSV AFTER: Store must have 1 fingerprint");

    // Retrieve and verify fingerprint
    let fp = store
        .retrieve(uuid)
        .await
        .expect("retrieve works")
        .expect("Fingerprint must exist");

    assert_eq!(fp.id, uuid, "FSV: Retrieved ID must match");
    assert_eq!(
        fp.purpose_vector.alignments.len(), 13,
        "FSV: Purpose vector must have 13 elements"
    );
}

// =============================================================================
// Valid Tests: goal/hierarchy_query tree operations
// =============================================================================

/// Test that goal/hierarchy_query returns structured tree with all levels.
///
/// FSV: Verify all 4 levels present and parent-child relationships correct.
#[tokio::test]
async fn test_get_goal_hierarchy_returns_tree() {
    // SETUP: Full hierarchy with all levels
    let (handlers, _store, hierarchy) = create_handlers_with_full_hierarchy();

    // FSV BEFORE: Verify hierarchy structure
    {
        let h = hierarchy.read();
        assert!(h.has_north_star(), "FSV BEFORE: Must have North Star");
        assert_eq!(h.len(), 4, "FSV BEFORE: Must have 4 goals");
        assert_eq!(h.at_level(GoalLevel::NorthStar).len(), 1, "Must have 1 NorthStar");
        assert_eq!(h.at_level(GoalLevel::Strategic).len(), 1, "Must have 1 Strategic");
        assert_eq!(h.at_level(GoalLevel::Tactical).len(), 1, "Must have 1 Tactical");
        assert_eq!(h.at_level(GoalLevel::Immediate).len(), 1, "Must have 1 Immediate");
    }

    // ACTION: Query get_all
    let params = json!({
        "operation": "get_all"
    });
    let request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Response contains all goals
    assert!(response.error.is_none(), "get_all must succeed");
    let result = response.result.expect("Must have result");

    let goals = result
        .get("goals")
        .and_then(|v| v.as_array())
        .expect("Must have goals array");
    assert_eq!(goals.len(), 4, "Must return all 4 goals");

    // Verify hierarchy_stats
    let stats = result
        .get("hierarchy_stats")
        .expect("Must have hierarchy_stats");
    assert_eq!(
        stats.get("total_goals").and_then(|v| v.as_u64()),
        Some(4),
        "total_goals must be 4"
    );
    assert_eq!(
        stats.get("has_north_star").and_then(|v| v.as_bool()),
        Some(true),
        "has_north_star must be true"
    );

    // Verify level_counts
    let level_counts = stats.get("level_counts").expect("Must have level_counts");
    assert_eq!(
        level_counts.get("north_star").and_then(|v| v.as_u64()),
        Some(1),
        "north_star count must be 1"
    );
    assert_eq!(
        level_counts.get("strategic").and_then(|v| v.as_u64()),
        Some(1),
        "strategic count must be 1"
    );
    assert_eq!(
        level_counts.get("tactical").and_then(|v| v.as_u64()),
        Some(1),
        "tactical count must be 1"
    );
    assert_eq!(
        level_counts.get("immediate").and_then(|v| v.as_u64()),
        Some(1),
        "immediate count must be 1"
    );

    // ACTION: Query get_children for North Star
    let children_params = json!({
        "operation": "get_children",
        "goal_id": "ns_knowledge"
    });
    let children_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(2)),
        Some(children_params),
    );
    let children_response = handlers.dispatch(children_request).await;

    assert!(children_response.error.is_none(), "get_children must succeed");
    let children_result = children_response.result.expect("Must have result");

    let children = children_result
        .get("children")
        .and_then(|v| v.as_array())
        .expect("Must have children array");
    assert_eq!(children.len(), 1, "North Star must have 1 child (Strategic)");

    // Verify the child is the Strategic goal
    let child = &children[0];
    assert_eq!(
        child.get("id").and_then(|v| v.as_str()),
        Some("s1_retrieval"),
        "Child must be s1_retrieval"
    );
    assert_eq!(
        child.get("level").and_then(|v| v.as_str()),
        Some("Strategic"),
        "Child level must be Strategic"
    );

    // ACTION: Query get_ancestors for Immediate goal
    let ancestors_params = json!({
        "operation": "get_ancestors",
        "goal_id": "i1_vectors"
    });
    let ancestors_request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(3)),
        Some(ancestors_params),
    );
    let ancestors_response = handlers.dispatch(ancestors_request).await;

    assert!(ancestors_response.error.is_none(), "get_ancestors must succeed");
    let ancestors_result = ancestors_response.result.expect("Must have result");

    let ancestors = ancestors_result
        .get("ancestors")
        .and_then(|v| v.as_array())
        .expect("Must have ancestors array");

    // Path: i1_vectors -> t1_semantic -> s1_retrieval -> ns_knowledge
    // Should have at least 3 ancestors (excluding self or including all)
    assert!(
        ancestors.len() >= 3,
        "Must have at least 3 ancestors in path to North Star"
    );
}

/// Test goal/hierarchy_query with non-existent goal_id.
#[tokio::test]
async fn test_hierarchy_query_nonexistent_goal() {
    // SETUP: Full hierarchy
    let (handlers, _store, _hierarchy) = create_handlers_with_full_hierarchy();

    // ACTION: Query non-existent goal
    let params = json!({
        "operation": "get_goal",
        "goal_id": "nonexistent_goal_xyz"
    });
    let request = make_request(
        "goal/hierarchy_query",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Fails with GOAL_NOT_FOUND
    assert!(
        response.error.is_some(),
        "Must fail with non-existent goal"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32020,
        "Must return GOAL_NOT_FOUND (-32020)"
    );
}

/// Test purpose/drift_check fails without North Star.
#[tokio::test]
async fn test_drift_check_fails_without_north_star() {
    // SETUP: Empty hierarchy
    let (handlers, _store, _hierarchy) = create_handlers_no_north_star();

    // ACTION: Try drift check
    let params = json!({
        "fingerprint_ids": ["00000000-0000-0000-0000-000000000001"]
    });
    let request = make_request(
        "purpose/drift_check",
        Some(JsonRpcId::Number(1)),
        Some(params),
    );
    let response = handlers.dispatch(request).await;

    // VERIFY: Fails with NORTH_STAR_NOT_CONFIGURED
    assert!(
        response.error.is_some(),
        "Drift check must fail without North Star"
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32021,
        "Must return NORTH_STAR_NOT_CONFIGURED (-32021)"
    );
}
