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

use crate::error::EmbeddingResult;
use crate::models::ModelRegistry;
use crate::types::{ModelEmbedding, ModelId};

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

    // Process each input in the batch
    let mut results: Vec<EmbeddingResult<ModelEmbedding>> = Vec::with_capacity(batch_size);
    let mut success_count: u64 = 0;
    let mut fail_count: u64 = 0;

    for input in &batch.inputs {
        match model.embed(input).await {
            Ok(embedding) => {
                results.push(Ok(embedding));
                success_count += 1;
            }
            Err(e) => {
                tracing::warn!(
                    model_id = ?model_id,
                    error = %e,
                    "Embedding failed for input"
                );
                results.push(Err(e));
                fail_count += 1;
            }
        }
    }

    // Complete batch with individual results
    batch.complete(results);

    // Update stats
    stats.add_requests_completed(success_count);
    stats.add_requests_failed(fail_count);
    stats.inc_batches_processed();
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

    #[test]
    fn test_queues_created_for_all_14_models() {
        let all_models = ModelId::all();
        assert_eq!(all_models.len(), 14, "Expected 14 models");
    }
}
