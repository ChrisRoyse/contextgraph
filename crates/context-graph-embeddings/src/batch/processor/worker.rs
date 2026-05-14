//! BatchProcessor worker loop and batch processing.
//!
//! Contains the async worker loop that manages queue polling
//! and batch processing through models.
//!
//! # Design Principle: No Detached Tasks
//!
//! This module does NOT use `tokio::spawn` for batch processing.
//! All work is done inline within the worker loop, ensuring:
//! - No orphaned tasks
//! - No resource leaks
//! - Clean shutdown without tracking
//!
//! Concurrency is achieved through the semaphore limiting concurrent
//! batches, not through spawning detached tasks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Notify, RwLock, Semaphore};
use tokio::time::interval;

use crate::error::EmbeddingError;
use crate::models::ModelRegistry;
use crate::types::ModelId;

use crate::batch::{Batch, BatchQueue, BatchRequest};

use super::stats::BatchProcessorStatsInternal;

// ============================================================================
// WORKER LOOP
// ============================================================================

/// Main worker loop that processes requests and batches.
///
/// # No Detached Tasks
///
/// All batch processing happens inline. The semaphore limits concurrency
/// but work is never spawned to detached tasks.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn worker_loop(
    queues: Arc<RwLock<HashMap<ModelId, BatchQueue>>>,
    registry: Arc<ModelRegistry>,
    mut request_rx: mpsc::Receiver<BatchRequest>,
    shutdown_notify: Arc<Notify>,
    is_running: Arc<AtomicBool>,
    stats: Arc<BatchProcessorStatsInternal>,
    batch_semaphore: Arc<Semaphore>,
    poll_interval: Duration,
) {
    let mut poll_timer = interval(poll_interval);

    loop {
        tokio::select! {
            // Check for shutdown
            _ = shutdown_notify.notified() => {
                tracing::info!("Worker received shutdown signal, flushing queues...");
                flush_all_queues(&queues, &registry, &stats, &batch_semaphore).await;
                tracing::info!("Worker shutdown complete");
                break;
            }

            // Receive new requests
            Some(request) = request_rx.recv() => {
                let model_id = request.model_id;

                // Add to appropriate queue
                {
                    let mut queues_guard = queues.write().await;
                    if let Some(queue) = queues_guard.get_mut(&model_id) {
                        queue.push(request);
                    }
                }

                // Process queue inline - NO SPAWNING
                process_queue_if_ready(
                    &queues,
                    &registry,
                    model_id,
                    &stats,
                    &batch_semaphore,
                ).await;
            }

            // Poll for timeouts
            _ = poll_timer.tick() => {
                if !is_running.load(Ordering::Relaxed) {
                    tracing::debug!("Worker detected is_running=false, exiting");
                    break;
                }

                // Check all queues for timeout-triggered flushes
                for model_id in ModelId::all() {
                    process_queue_if_ready(
                        &queues,
                        &registry,
                        *model_id,
                        &stats,
                        &batch_semaphore,
                    ).await;
                }
            }
        }
    }
}

// ============================================================================
// QUEUE PROCESSING - INLINE, NO SPAWNING
// ============================================================================

/// Process a queue if it's ready to flush.
///
/// All processing is done INLINE - no detached tasks are created.
async fn process_queue_if_ready(
    queues: &Arc<RwLock<HashMap<ModelId, BatchQueue>>>,
    registry: &Arc<ModelRegistry>,
    model_id: ModelId,
    stats: &Arc<BatchProcessorStatsInternal>,
    batch_semaphore: &Arc<Semaphore>,
) {
    // Check if should flush (read lock)
    let should_flush = {
        let queues_guard = queues.read().await;
        queues_guard
            .get(&model_id)
            .map(|q| q.should_flush())
            .unwrap_or(false)
    };

    if !should_flush {
        return;
    }

    // Try to acquire semaphore permit
    let permit = match batch_semaphore.try_acquire() {
        Ok(permit) => permit,
        Err(_) => return, // Max concurrent batches reached, try next poll
    };

    // Extract batch (write lock)
    let batch = {
        let mut queues_guard = queues.write().await;
        queues_guard
            .get_mut(&model_id)
            .and_then(|q| q.drain_batch())
    };

    if let Some(batch) = batch {
        // Process INLINE - no spawning
        process_batch(batch, registry, stats).await;
    }

    drop(permit);
}

// ============================================================================
// BATCH PROCESSING
// ============================================================================

/// Process a single batch through the model.
///
/// Called inline from the worker loop - never spawned.
async fn process_batch(
    batch: Batch,
    registry: &Arc<ModelRegistry>,
    stats: &Arc<BatchProcessorStatsInternal>,
) {
    let batch_size = batch.len();
    let model_id = batch.model_id;

    // Get model from registry
    let model = match registry.get_model(model_id).await {
        Ok(model) => model,
        Err(e) => {
            // Fail entire batch - NO FALLBACKS
            tracing::error!(
                model_id = ?model_id,
                error = %e,
                "Failed to get model for batch - failing entire batch"
            );
            batch.fail(format!("Failed to get model {:?}: {}", model_id, e));
            stats.add_requests_failed(batch_size as u64);
            return;
        }
    };

    // Process the full input set through the explicit true-batch contract.
    // Queue batching is not sufficient here; unsupported models must fail
    // closed instead of silently looping over single-input inference.
    let embeddings = match model.embed_true_batch(&batch.inputs).await {
        Ok(embeddings) => embeddings,
        Err(e) => {
            tracing::error!(
                model_id = ?model_id,
                batch_size,
                error = %e,
                "True-batch embedding failed - failing entire batch"
            );
            let results = (0..batch_size)
                .map(|_| Err(clone_batch_error(&e)))
                .collect();
            batch.complete(results);
            stats.add_requests_failed(batch_size as u64);
            stats.inc_batches_processed();
            return;
        }
    };

    if embeddings.len() != batch_size {
        tracing::error!(
            model_id = ?model_id,
            batch_size,
            actual_outputs = embeddings.len(),
            "True-batch embedding returned wrong output count - failing entire batch"
        );
        let expected = batch_size;
        let actual = embeddings.len();
        let results = (0..batch_size)
            .map(|_| {
                Err(EmbeddingError::TrueBatchOutputCountMismatch {
                    model_id,
                    expected,
                    actual,
                    recovery_hint:
                        "fix the concrete true-batch implementation so output rows exactly match input rows before durable promotion"
                            .to_string(),
                })
            })
            .collect();
        batch.complete(results);
        stats.add_requests_failed(batch_size as u64);
        stats.inc_batches_processed();
        return;
    }

    batch.complete(embeddings.into_iter().map(Ok).collect());

    stats.add_requests_completed(batch_size as u64);
    stats.inc_batches_processed();
}

fn clone_batch_error(error: &EmbeddingError) -> EmbeddingError {
    match error {
        EmbeddingError::TrueBatchEmpty {
            model_id,
            recovery_hint,
        } => EmbeddingError::TrueBatchEmpty {
            model_id: *model_id,
            recovery_hint: recovery_hint.clone(),
        },
        EmbeddingError::TrueBatchUnsupported {
            model_id,
            batch_size,
            recovery_hint,
        } => EmbeddingError::TrueBatchUnsupported {
            model_id: *model_id,
            batch_size: *batch_size,
            recovery_hint: recovery_hint.clone(),
        },
        EmbeddingError::TrueBatchOutputCountMismatch {
            model_id,
            expected,
            actual,
            recovery_hint,
        } => EmbeddingError::TrueBatchOutputCountMismatch {
            model_id: *model_id,
            expected: *expected,
            actual: *actual,
            recovery_hint: recovery_hint.clone(),
        },
        other => EmbeddingError::BatchError {
            message: other.to_string(),
        },
    }
}

// ============================================================================
// FLUSH OPERATIONS
// ============================================================================

/// Flush all queues during shutdown.
async fn flush_all_queues(
    queues: &Arc<RwLock<HashMap<ModelId, BatchQueue>>>,
    registry: &Arc<ModelRegistry>,
    stats: &Arc<BatchProcessorStatsInternal>,
    batch_semaphore: &Arc<Semaphore>,
) {
    for model_id in ModelId::all() {
        loop {
            let has_items = {
                let queues_guard = queues.read().await;
                queues_guard
                    .get(model_id)
                    .map(|q| !q.is_empty())
                    .unwrap_or(false)
            };

            if !has_items {
                break;
            }

            let permit = match batch_semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => break,
            };

            let batch = {
                let mut queues_guard = queues.write().await;
                queues_guard.get_mut(model_id).and_then(|q| q.drain_batch())
            };

            if let Some(batch) = batch {
                process_batch(batch, registry, stats).await;
            }

            drop(permit);
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EmbeddingResult;
    use crate::models::ModelRegistryConfig;
    use crate::traits::{EmbeddingModel, ModelFactory, SingleModelConfig};
    use crate::types::{InputType, ModelEmbedding, ModelInput};
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn test_queues_created_for_all_14_models() {
        let all_models = ModelId::all();
        assert_eq!(
            all_models.len(),
            15,
            "Expected 15 models (14 + legacy Entity)"
        );
    }

    struct CountingModel {
        embed_calls: Arc<AtomicU64>,
    }

    #[async_trait::async_trait]
    impl EmbeddingModel for CountingModel {
        fn model_id(&self) -> ModelId {
            ModelId::Semantic
        }

        fn supported_input_types(&self) -> &[InputType] {
            &[InputType::Text]
        }

        async fn embed(&self, _input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
            self.embed_calls.fetch_add(1, Ordering::SeqCst);
            Ok(ModelEmbedding::new(ModelId::Semantic, vec![0.0; 1024], 1))
        }

        fn is_initialized(&self) -> bool {
            true
        }
    }

    struct CountingFactory {
        embed_calls: Arc<AtomicU64>,
    }

    impl CountingFactory {
        fn new(embed_calls: Arc<AtomicU64>) -> Self {
            Self { embed_calls }
        }
    }

    #[async_trait::async_trait]
    impl ModelFactory for CountingFactory {
        fn create_model(
            &self,
            model_id: ModelId,
            config: &SingleModelConfig,
        ) -> EmbeddingResult<Box<dyn EmbeddingModel>> {
            config.validate()?;
            if model_id != ModelId::Semantic {
                return Err(EmbeddingError::ModelNotFound { model_id });
            }
            Ok(Box::new(CountingModel {
                embed_calls: Arc::clone(&self.embed_calls),
            }))
        }

        fn supported_models(&self) -> &[ModelId] {
            &[ModelId::Semantic]
        }

        fn estimate_memory(&self, model_id: ModelId) -> usize {
            if model_id == ModelId::Semantic {
                1
            } else {
                0
            }
        }
    }

    #[tokio::test]
    async fn process_batch_rejects_unsupported_true_batch_without_single_embed_loop() {
        let embed_calls = Arc::new(AtomicU64::new(0));
        let factory = Arc::new(CountingFactory::new(Arc::clone(&embed_calls)));
        let registry = Arc::new(
            ModelRegistry::new(ModelRegistryConfig::testing(1024), factory)
                .await
                .unwrap(),
        );
        let stats = Arc::new(BatchProcessorStatsInternal::default());
        let mut batch = Batch::new(ModelId::Semantic);
        let (request_one, rx_one) = BatchRequest::new(
            ModelInput::text("first real input").unwrap(),
            ModelId::Semantic,
        );
        let (request_two, rx_two) = BatchRequest::new(
            ModelInput::text("second real input").unwrap(),
            ModelId::Semantic,
        );
        batch.add(request_one);
        batch.add(request_two);

        println!("BEFORE: embed_calls={}", embed_calls.load(Ordering::SeqCst));
        println!("BEFORE: stats={:?}", stats.snapshot());

        process_batch(batch, &registry, &stats).await;

        let err_one = rx_one.await.unwrap().unwrap_err();
        let err_two = rx_two.await.unwrap().unwrap_err();
        println!("AFTER: err_one={err_one}");
        println!("AFTER: err_two={err_two}");
        println!("AFTER: embed_calls={}", embed_calls.load(Ordering::SeqCst));
        println!("AFTER: stats={:?}", stats.snapshot());

        assert_eq!(embed_calls.load(Ordering::SeqCst), 0);
        for err in [err_one, err_two] {
            match err {
                EmbeddingError::TrueBatchUnsupported {
                    model_id,
                    batch_size,
                    recovery_hint,
                } => {
                    assert_eq!(model_id, ModelId::Semantic);
                    assert_eq!(batch_size, 2);
                    assert!(recovery_hint.contains("native"));
                }
                _ => panic!("expected TRUE_BATCH_UNSUPPORTED, got {err}"),
            }
        }
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.requests_completed, 0);
        assert_eq!(snapshot.requests_failed, 2);
        assert_eq!(snapshot.batches_processed, 1);
    }
}
