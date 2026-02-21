//! Basic tests for MemoryTracker.
//!
//! Tests covering:
//! - Construction
//! - Allocation checks (can_allocate)
//! - Memory allocation

use crate::error::EmbeddingError;
use crate::models::memory_tracker::MemoryTracker;
use crate::types::ModelId;

// =========================================================================
// CONSTRUCTION TESTS
// =========================================================================

#[test]
fn test_new_creates_empty_tracker() {
    let tracker = MemoryTracker::new(1_000_000_000);
    assert_eq!(tracker.current_usage(), 0);
    assert_eq!(tracker.budget(), 1_000_000_000);
    assert_eq!(tracker.remaining(), 1_000_000_000);
    assert_eq!(tracker.allocation_count(), 0);
}

#[test]
fn test_new_with_zero_budget() {
    let tracker = MemoryTracker::new(0);
    assert_eq!(tracker.budget(), 0);
    assert!(!tracker.can_allocate(1));
}

#[test]
fn test_new_with_large_budget() {
    let budget = 128_000_000_000; // 128GB
    let tracker = MemoryTracker::new(budget);
    assert_eq!(tracker.budget(), budget);
    assert!(tracker.can_allocate(64_000_000_000));
}

// =========================================================================
// CAN_ALLOCATE TESTS
// =========================================================================

#[test]
fn test_can_allocate_within_budget() {
    let tracker = MemoryTracker::new(1_000_000_000);
    assert!(tracker.can_allocate(500_000_000));
    assert!(tracker.can_allocate(1_000_000_000));
}

#[test]
fn test_can_allocate_exceeds_budget() {
    let tracker = MemoryTracker::new(1_000_000_000);
    assert!(!tracker.can_allocate(1_000_000_001));
    assert!(!tracker.can_allocate(2_000_000_000));
}

#[test]
fn test_can_allocate_after_allocation() {
    let mut tracker = MemoryTracker::new(1_000_000_000);
    tracker.allocate(ModelId::Semantic, 600_000_000).unwrap();

    assert!(tracker.can_allocate(400_000_000));
    assert!(!tracker.can_allocate(400_000_001));
}

#[test]
fn test_can_allocate_zero_always_succeeds() {
    let tracker = MemoryTracker::new(100);
    assert!(tracker.can_allocate(0));

    let full_tracker = MemoryTracker::new(0);
    assert!(full_tracker.can_allocate(0));
}

// =========================================================================
// ALLOCATE TESTS
// =========================================================================

#[test]
fn test_allocate_success() {
    let mut tracker = MemoryTracker::new(2_000_000_000);
    let result = tracker.allocate(ModelId::Semantic, 1_400_000_000);

    assert!(result.is_ok());
    assert_eq!(tracker.current_usage(), 1_400_000_000);
    assert_eq!(tracker.allocation_for(ModelId::Semantic), 1_400_000_000);
    assert!(tracker.is_allocated(ModelId::Semantic));
}

#[test]
fn test_allocate_multiple_models() {
    let mut tracker = MemoryTracker::new(3_000_000_000);

    tracker.allocate(ModelId::Semantic, 1_400_000_000).unwrap();
    tracker.allocate(ModelId::Code, 550_000_000).unwrap();
    tracker.allocate(ModelId::Graph, 120_000_000).unwrap();

    assert_eq!(tracker.allocation_count(), 3);
    assert_eq!(tracker.current_usage(), 2_070_000_000);
    assert_eq!(tracker.remaining(), 930_000_000);
}

#[test]
fn test_allocate_fails_when_budget_exceeded() {
    let mut tracker = MemoryTracker::new(1_000_000_000);
    let result = tracker.allocate(ModelId::Contextual, 1_600_000_000);

    assert!(result.is_err());
    match result {
        Err(EmbeddingError::MemoryBudgetExceeded {
            requested_bytes,
            available_bytes,
            budget_bytes,
        }) => {
            assert_eq!(requested_bytes, 1_600_000_000);
            assert_eq!(available_bytes, 1_000_000_000);
            assert_eq!(budget_bytes, 1_000_000_000);
        }
        _ => panic!("Expected MemoryBudgetExceeded error"),
    }

    // Tracker should be unchanged
    assert_eq!(tracker.current_usage(), 0);
    assert!(!tracker.is_allocated(ModelId::Contextual));
}

#[test]
fn test_allocate_fails_when_model_already_loaded() {
    let mut tracker = MemoryTracker::new(3_000_000_000);
    tracker.allocate(ModelId::Semantic, 1_400_000_000).unwrap();

    let result = tracker.allocate(ModelId::Semantic, 1_400_000_000);
    assert!(result.is_err());
    match result {
        Err(EmbeddingError::ModelAlreadyLoaded { model_id }) => {
            assert_eq!(model_id, ModelId::Semantic);
        }
        _ => panic!("Expected ModelAlreadyLoaded error"),
    }

    // Original allocation unchanged
    assert_eq!(tracker.current_usage(), 1_400_000_000);
}

#[test]
fn test_allocate_all_14_models() {
    // Use real memory estimates from MEMORY_ESTIMATES
    use crate::traits::MEMORY_ESTIMATES;

    let total: usize = MEMORY_ESTIMATES.iter().map(|(_, m)| *m).sum();
    let mut tracker = MemoryTracker::new(total + 1_000_000); // Slight buffer

    for (model_id, bytes) in MEMORY_ESTIMATES {
        let result = tracker.allocate(model_id, bytes);
        assert!(result.is_ok(), "Failed to allocate {:?}", model_id);
    }

    assert_eq!(tracker.allocation_count(), 14);
    assert_eq!(tracker.current_usage(), total);
}
