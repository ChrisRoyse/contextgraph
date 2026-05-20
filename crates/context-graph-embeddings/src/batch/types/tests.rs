//! Comprehensive tests for batch types module.
//!
//! This module contains all unit and integration tests for the batch
//! processing infrastructure.

#![allow(clippy::field_reassign_with_default)]

use super::*;
use crate::config::BatchConfig;
use crate::error::EmbeddingResult;
use crate::types::{ImageFormat, ModelEmbedding, ModelId, ModelInput};

// ============================================================
// BATCH REQUEST TESTS
// ============================================================

#[test]
fn test_batch_request_new_creates_valid_request() {
    let input = ModelInput::text("Hello, world!").unwrap();
    let (request, _rx) = BatchRequest::new(input.clone(), ModelId::Semantic);

    assert_eq!(request.model_id, ModelId::Semantic);
    assert_eq!(request.priority, 0);
    assert!(!request.id.is_nil());
}

#[test]
fn test_batch_request_with_priority() {
    let input = ModelInput::text("Urgent request").unwrap();
    let (request, _rx) = BatchRequest::with_priority(input, ModelId::Semantic, 100);

    assert_eq!(request.priority, 100);
}

#[test]
fn test_batch_request_elapsed_increases() {
    let input = ModelInput::text("Test").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);

    let elapsed1 = request.elapsed();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let elapsed2 = request.elapsed();

    assert!(elapsed2 > elapsed1);
}

#[test]
fn test_batch_request_estimated_tokens_text() {
    // 12 characters / 4 = 3 tokens
    let input = ModelInput::text("Hello world!").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);

    assert_eq!(request.estimated_tokens(), 3);
}

#[test]
fn test_batch_request_estimated_tokens_text_with_instruction() {
    // "Hello" (5) + "query:" (6) = 11 / 4 = 2
    let input = ModelInput::text_with_instruction("Hello", "query:").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);

    assert_eq!(request.estimated_tokens(), 2);
}

#[test]
fn test_batch_request_estimated_tokens_code() {
    // 12 characters / 3 = 4 tokens
    let input = ModelInput::code("fn main() {}", "rust").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Code);

    assert_eq!(request.estimated_tokens(), 4);
}

#[test]
fn test_batch_request_estimated_tokens_image() {
    let input = ModelInput::image(vec![1, 2, 3, 4], ImageFormat::Png).unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Contextual);

    assert_eq!(request.estimated_tokens(), 100);
}

#[test]
fn test_batch_request_estimated_tokens_minimum_one() {
    // Very short text still returns at least 1
    let input = ModelInput::text("Hi").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);

    assert!(request.estimated_tokens() >= 1);
}

// ============================================================
// BATCH QUEUE STATS TESTS
// ============================================================

#[test]
fn test_batch_queue_stats_default() {
    let stats = BatchQueueStats::default();
    let summary = stats.summary();

    assert_eq!(summary.requests_received, 0);
    assert_eq!(summary.batches_processed, 0);
    assert_eq!(summary.requests_completed, 0);
    assert_eq!(summary.requests_failed, 0);
    assert_eq!(summary.avg_batch_size, 0.0);
}

#[test]
fn test_batch_queue_stats_record_request() {
    let stats = BatchQueueStats::default();
    stats.record_request();
    stats.record_request();

    assert_eq!(stats.summary().requests_received, 2);
}

#[test]
fn test_batch_queue_stats_record_batch() {
    let stats = BatchQueueStats::default();
    stats.record_batch(10, 5000);
    stats.record_batch(20, 3000);

    let summary = stats.summary();
    assert_eq!(summary.batches_processed, 2);
    assert!((summary.avg_batch_size - 15.0).abs() < 0.001);
    assert_eq!(summary.avg_wait_time_us, 4000); // (5000 + 3000) / 2
}

#[test]
fn test_batch_queue_stats_record_completion() {
    let stats = BatchQueueStats::default();
    stats.record_completion(true);
    stats.record_completion(true);
    stats.record_completion(false);

    let summary = stats.summary();
    assert_eq!(summary.requests_completed, 2);
    assert_eq!(summary.requests_failed, 1);
}

#[test]
fn test_batch_queue_stats_reset() {
    let stats = BatchQueueStats::default();
    stats.record_request();
    stats.record_batch(10, 5000);
    stats.record_completion(true);

    stats.reset();

    let summary = stats.summary();
    assert_eq!(summary.requests_received, 0);
    assert_eq!(summary.batches_processed, 0);
}

#[test]
fn test_batch_queue_stats_clone() {
    let stats = BatchQueueStats::default();
    stats.record_request();
    stats.record_batch(10, 5000);

    let cloned = stats.clone();
    let summary = cloned.summary();
    assert_eq!(summary.requests_received, 1);
    assert_eq!(summary.batches_processed, 1);
}

// ============================================================
// BATCH QUEUE TESTS
// ============================================================

#[test]
fn test_batch_queue_new() {
    let config = BatchConfig::default();
    let queue = BatchQueue::new(ModelId::Semantic, config);

    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    assert_eq!(queue.model_id(), ModelId::Semantic);
}

#[test]
fn test_batch_queue_push() {
    let config = BatchConfig::default();
    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    let input = ModelInput::text("Test").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);
    queue.push(request);

    assert_eq!(queue.len(), 1);
    assert!(!queue.is_empty());
}

#[test]
fn test_batch_queue_should_flush_empty() {
    let config = BatchConfig::default();
    let queue = BatchQueue::new(ModelId::Semantic, config);

    assert!(!queue.should_flush());
}

#[test]
fn test_batch_queue_should_flush_max_size() {
    let mut config = BatchConfig::default();
    config.max_batch_size = 2;

    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    // Add first request - should not flush
    let input1 = ModelInput::text("Test 1").unwrap();
    let (request1, _rx1) = BatchRequest::new(input1, ModelId::Semantic);
    queue.push(request1);
    assert!(!queue.should_flush());

    // Add second request - should flush (max_batch_size = 2)
    let input2 = ModelInput::text("Test 2").unwrap();
    let (request2, _rx2) = BatchRequest::new(input2, ModelId::Semantic);
    queue.push(request2);
    assert!(queue.should_flush());
}

#[test]
fn test_batch_queue_should_flush_timeout() {
    let mut config = BatchConfig::default();
    config.max_wait_ms = 2; // 2ms timeout (shortened for test perf)

    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    let input = ModelInput::text("Test").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);
    queue.push(request);

    // Should not flush immediately
    assert!(!queue.should_flush());

    // Wait for timeout
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(queue.should_flush());
}

#[test]
fn test_batch_queue_drain_batch_empty() {
    let config = BatchConfig::default();
    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    assert!(queue.drain_batch().is_none());
}

#[test]
fn test_batch_queue_drain_batch_returns_batch() {
    let config = BatchConfig::default();
    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    let input = ModelInput::text("Test").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);
    queue.push(request);

    let batch = queue.drain_batch();
    assert!(batch.is_some());

    let batch = batch.unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch.model_id, ModelId::Semantic);
}

#[test]
fn test_batch_queue_drain_batch_respects_max_size() {
    let mut config = BatchConfig::default();
    config.max_batch_size = 2;

    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    // Add 3 requests
    for i in 0..3 {
        let input = ModelInput::text(format!("Test {}", i)).unwrap();
        let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);
        queue.push(request);
    }

    // First drain should get 2
    let batch1 = queue.drain_batch().unwrap();
    assert_eq!(batch1.len(), 2);

    // Second drain should get 1
    let batch2 = queue.drain_batch().unwrap();
    assert_eq!(batch2.len(), 1);

    // Third drain should get none
    assert!(queue.drain_batch().is_none());
}

#[test]
fn test_batch_queue_drain_batch_sorts_by_length() {
    let mut config = BatchConfig::default();
    config.sort_by_length = true;

    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    // Add requests with different lengths
    let long_input = ModelInput::text("This is a very long sentence that has many words").unwrap();
    let short_input = ModelInput::text("Short").unwrap();
    let medium_input = ModelInput::text("Medium length text").unwrap();

    let (req1, _) = BatchRequest::new(long_input, ModelId::Semantic);
    let (req2, _) = BatchRequest::new(short_input, ModelId::Semantic);
    let (req3, _) = BatchRequest::new(medium_input, ModelId::Semantic);

    queue.push(req1);
    queue.push(req2);
    queue.push(req3);

    let batch = queue.drain_batch().unwrap();

    // V-012 fix (#507): previous version only checked batch.inputs[0]
    // contained "Short". A broken sort that placed Short first then left
    // the rest in arbitrary order would have passed. Now we extract all
    // three positions and assert the full expected ordering by length.
    assert_eq!(batch.inputs.len(), 3);

    let texts: Vec<&str> = batch
        .inputs
        .iter()
        .map(|inp| match inp {
            ModelInput::Text { content, .. } => content.as_str(),
            other => panic!("expected ModelInput::Text, got {:?}", other),
        })
        .collect();
    assert_eq!(
        texts,
        vec![
            "Short",
            "Medium length text",
            "This is a very long sentence that has many words",
        ],
        "drain_batch must sort by length ascending"
    );
}

#[test]
fn test_batch_queue_oldest_wait_time() {
    let config = BatchConfig::default();
    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    // Empty queue
    assert!(queue.oldest_wait_time().is_none());

    // Add request
    let input = ModelInput::text("Test").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);
    queue.push(request);

    std::thread::sleep(std::time::Duration::from_millis(2));

    let wait = queue.oldest_wait_time();
    assert!(wait.is_some());
    assert!(wait.unwrap() >= std::time::Duration::from_millis(2));
}

#[tokio::test]
async fn test_batch_queue_cancel_all() {
    let config = BatchConfig::default();
    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    let input = ModelInput::text("Test").unwrap();
    let (request, rx) = BatchRequest::new(input, ModelId::Semantic);
    queue.push(request);

    queue.cancel_all("Shutdown");

    assert!(queue.is_empty());

    // Receiver should get error
    let result = rx.await.unwrap();
    assert!(result.is_err());
}

// ============================================================
// BATCH TESTS
// ============================================================

#[test]
fn test_batch_new() {
    let batch = Batch::new(ModelId::Semantic);

    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
    assert_eq!(batch.model_id, ModelId::Semantic);
    assert!(!batch.id.is_nil());
    assert_eq!(batch.total_tokens, 0);
}

#[test]
fn test_batch_add() {
    let mut batch = Batch::new(ModelId::Semantic);

    let input = ModelInput::text("Hello world!").unwrap();
    let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);

    let expected_tokens = request.estimated_tokens();
    let request_id = request.id;

    batch.add(request);

    assert_eq!(batch.len(), 1);
    assert!(!batch.is_empty());
    assert_eq!(batch.total_tokens, expected_tokens);
    assert_eq!(batch.request_ids[0], request_id);
}

#[test]
fn test_batch_max_tokens() {
    let mut batch = Batch::new(ModelId::Semantic);

    // Short text: 5/4 = 1 token
    let input1 = ModelInput::text("Short").unwrap();
    let (req1, _) = BatchRequest::new(input1, ModelId::Semantic);
    batch.add(req1);

    // Long text: 40/4 = 10 tokens
    let input2 = ModelInput::text("This is a much longer piece of text here").unwrap();
    let (req2, _) = BatchRequest::new(input2, ModelId::Semantic);
    batch.add(req2);

    assert_eq!(batch.max_tokens(), 10);
}

#[tokio::test]
async fn test_batch_complete() {
    let mut batch = Batch::new(ModelId::Semantic);

    let input = ModelInput::text("Test").unwrap();
    let (request, rx) = BatchRequest::new(input, ModelId::Semantic);
    batch.add(request);

    // Create a result
    let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);

    batch.complete(vec![Ok(embedding.clone())]);

    // Receiver should get the result
    let result = rx.await.unwrap();
    assert!(result.is_ok());
    assert_eq!(result.unwrap().vector, embedding.vector);
}

#[tokio::test]
async fn test_batch_complete_multiple() {
    let mut batch = Batch::new(ModelId::Semantic);

    let input1 = ModelInput::text("Test 1").unwrap();
    let (req1, rx1) = BatchRequest::new(input1, ModelId::Semantic);
    batch.add(req1);

    let input2 = ModelInput::text("Test 2").unwrap();
    let (req2, rx2) = BatchRequest::new(input2, ModelId::Semantic);
    batch.add(req2);

    // Create results
    let emb1 = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);
    let emb2 = ModelEmbedding::new(ModelId::Semantic, vec![0.2; 1024], 200);

    batch.complete(vec![Ok(emb1.clone()), Ok(emb2.clone())]);

    // Both receivers should get results
    let result1 = rx1.await.unwrap().unwrap();
    let result2 = rx2.await.unwrap().unwrap();

    assert_eq!(result1.vector[0], 0.1);
    assert_eq!(result2.vector[0], 0.2);
}

#[tokio::test]
async fn test_batch_fail() {
    let mut batch = Batch::new(ModelId::Semantic);

    let input = ModelInput::text("Test").unwrap();
    let (request, rx) = BatchRequest::new(input, ModelId::Semantic);
    batch.add(request);

    batch.fail("Test error");

    // Receiver should get error
    let result = rx.await.unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_fail_multiple() {
    let mut batch = Batch::new(ModelId::Semantic);

    let input1 = ModelInput::text("Test 1").unwrap();
    let (req1, rx1) = BatchRequest::new(input1, ModelId::Semantic);
    batch.add(req1);

    let input2 = ModelInput::text("Test 2").unwrap();
    let (req2, rx2) = BatchRequest::new(input2, ModelId::Semantic);
    batch.add(req2);

    batch.fail("Batch failed");

    // Both should get the same error
    assert!(rx1.await.unwrap().is_err());
    assert!(rx2.await.unwrap().is_err());
}

#[test]
fn test_batch_elapsed() {
    let batch = Batch::new(ModelId::Semantic);
    std::thread::sleep(std::time::Duration::from_millis(2));

    assert!(batch.elapsed() >= std::time::Duration::from_millis(2));
}

// ============================================================
// INTEGRATION TESTS
// ============================================================

#[tokio::test]
async fn test_full_batch_workflow() {
    let mut config = BatchConfig::default();
    config.max_batch_size = 3;

    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    // Add 3 requests
    let mut receivers = Vec::new();
    for i in 0..3 {
        let input = ModelInput::text(format!("Request {}", i)).unwrap();
        let (request, rx) = BatchRequest::new(input, ModelId::Semantic);
        queue.push(request);
        receivers.push(rx);
    }

    assert!(queue.should_flush());

    // Drain batch
    let batch = queue.drain_batch().unwrap();
    assert_eq!(batch.len(), 3);

    // Complete with results
    let results: Vec<EmbeddingResult<ModelEmbedding>> = (0..3)
        .map(|i| {
            Ok(ModelEmbedding::new(
                ModelId::Semantic,
                vec![i as f32; 1024],
                100 + i as u64,
            ))
        })
        .collect();

    batch.complete(results);

    // All receivers should get results
    for (i, rx) in receivers.into_iter().enumerate() {
        let result = rx.await.unwrap().unwrap();
        assert_eq!(result.vector[0], i as f32);
    }
}

#[test]
fn test_stats_updated_through_workflow() {
    let mut config = BatchConfig::default();
    config.max_batch_size = 2;

    let mut queue = BatchQueue::new(ModelId::Semantic, config);

    // Add requests
    for _ in 0..2 {
        let input = ModelInput::text("Test").unwrap();
        let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);
        queue.push(request);
    }

    // Check requests received
    assert_eq!(queue.stats_summary().requests_received, 2);

    // Drain batch
    let _batch = queue.drain_batch().unwrap();

    // Check batches processed
    assert_eq!(queue.stats_summary().batches_processed, 1);
    assert!((queue.stats_summary().avg_batch_size - 2.0).abs() < 0.001);
}
