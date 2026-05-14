//! BatchProcessor submission API.
//!
//! Contains methods for submitting embedding requests to the processor.

use std::sync::atomic::Ordering;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::{ModelEmbedding, ModelId, ModelInput};

use crate::batch::BatchRequest;

use super::core::BatchProcessor;

// ============================================================================
// SUBMISSION API
// ============================================================================

impl BatchProcessor {
    /// Submit a single embedding request.
    ///
    /// The request is queued and processed when the batch is ready
    /// (either max_batch_size reached or timeout expired).
    ///
    /// # Arguments
    /// * `model_id` - Target model
    /// * `input` - Input to embed
    ///
    /// # Returns
    /// The embedding result when processing completes.
    ///
    /// # Errors
    /// * `EmbeddingError::BatchError` if processor is shutting down
    /// * `EmbeddingError::BatchError` if channel is closed
    /// * Other errors from model inference
    pub async fn submit(
        &self,
        model_id: ModelId,
        input: ModelInput,
    ) -> EmbeddingResult<ModelEmbedding> {
        if !self.is_running_internal() {
            return Err(EmbeddingError::BatchError {
                message: "BatchProcessor is shutting down".to_string(),
            });
        }

        let (request, rx) = BatchRequest::new(input, model_id);
        self.inc_requests_submitted();

        // Send to worker
        self.send_request(request).await?;

        // Wait for result
        rx.await.map_err(|_| EmbeddingError::BatchError {
            message: "Request was dropped before completion".to_string(),
        })?
    }

    /// Submit multiple inputs for batch processing.
    ///
    /// Inputs are queued together and processed efficiently.
    /// Results are returned in the same order as inputs.
    ///
    /// # Arguments
    /// * `model_id` - Target model (same for all inputs)
    /// * `inputs` - Inputs to embed
    ///
    /// # Returns
    /// Embeddings in same order as inputs.
    ///
    /// # Errors
    /// * Returns first error encountered
    /// * All inputs fail if any critical error occurs
    pub async fn submit_batch(
        &self,
        model_id: ModelId,
        inputs: Vec<ModelInput>,
    ) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if inputs.is_empty() {
            return Err(EmbeddingError::TrueBatchEmpty {
                model_id,
                recovery_hint:
                    "submit at least one input; empty batches are caller bugs, not no-op successes"
                        .to_string(),
            });
        }

        if !self.is_running_internal() {
            return Err(EmbeddingError::BatchError {
                message: "BatchProcessor is shutting down".to_string(),
            });
        }

        // Create requests and collect receivers
        let mut receivers = Vec::with_capacity(inputs.len());

        for input in inputs {
            let (request, rx) = BatchRequest::new(input, model_id);
            self.inc_requests_submitted();
            self.send_request(request).await?;
            receivers.push(rx);
        }

        // Collect all results
        let mut results = Vec::with_capacity(receivers.len());
        for rx in receivers {
            let result = rx.await.map_err(|_| EmbeddingError::BatchError {
                message: "Request was dropped before completion".to_string(),
            })??;
            results.push(result);
        }

        Ok(results)
    }

    // ========================================================================
    // INTERNAL HELPERS
    // ========================================================================

    /// Check if processor is running (internal version).
    #[inline]
    pub(crate) fn is_running_internal(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Increment requests submitted counter.
    #[inline]
    pub(crate) fn inc_requests_submitted(&self) {
        self.stats.inc_requests_submitted();
    }

    /// Send a request to the worker.
    pub(crate) async fn send_request(&self, request: BatchRequest) -> EmbeddingResult<()> {
        self.request_tx
            .send(request)
            .await
            .map_err(|_| EmbeddingError::BatchError {
                message: "Failed to submit request: channel closed".to_string(),
            })
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::batch::{BatchProcessor, BatchProcessorConfig};
    use crate::error::EmbeddingError;
    use crate::models::{ModelRegistry, ModelRegistryConfig};
    use crate::traits::{EmbeddingModel, ModelFactory, SingleModelConfig};
    use crate::types::ModelId;
    use crate::types::ModelInput;
    use std::sync::Arc;

    struct EmptyBatchFactory;

    #[async_trait::async_trait]
    impl ModelFactory for EmptyBatchFactory {
        fn create_model(
            &self,
            model_id: ModelId,
            _config: &SingleModelConfig,
        ) -> crate::error::EmbeddingResult<Box<dyn EmbeddingModel>> {
            panic!("empty submit_batch must reject before loading {model_id:?}")
        }

        fn supported_models(&self) -> &[ModelId] {
            ModelId::all()
        }

        fn estimate_memory(&self, _model_id: ModelId) -> usize {
            1
        }
    }

    #[tokio::test]
    async fn test_edge_case_1_empty_batch() {
        // BEFORE: Call submit_batch with empty vec
        // OPERATION: submit_batch(ModelId::Semantic, vec![])
        // AFTER: Returns TRUE_BATCH_EMPTY immediately - no queue interaction
        // VERIFY: No silent no-op success

        println!("\n========================================");
        println!("EDGE CASE 1: Empty Batch");
        println!("========================================");

        let registry = Arc::new(
            ModelRegistry::new(
                ModelRegistryConfig::testing(1024),
                Arc::new(EmptyBatchFactory),
            )
            .await
            .unwrap(),
        );
        let mut processor = BatchProcessor::new(registry, BatchProcessorConfig::default())
            .await
            .unwrap();
        let inputs: Vec<ModelInput> = vec![];
        let model_id = ModelId::Semantic;
        let before = processor.stats().await;

        // The submit_batch method must reject empty inputs before queueing.
        assert!(inputs.is_empty());
        println!("BEFORE: inputs = {:?}", inputs);
        println!("BEFORE: stats = {:?}", before);
        println!("OPERATION: submit_batch with empty vec");
        let err = processor.submit_batch(model_id, inputs).await.unwrap_err();
        let after = processor.stats().await;
        processor.shutdown().await;
        match err {
            EmbeddingError::TrueBatchEmpty {
                model_id: observed,
                recovery_hint,
            } => {
                assert_eq!(observed, ModelId::Semantic);
                assert!(recovery_hint.contains("at least one input"));
            }
            _ => panic!("Expected TrueBatchEmpty error"),
        }
        println!("AFTER: TRUE_BATCH_EMPTY for {:?}", model_id);
        println!("AFTER: stats = {:?}", after);
        assert_eq!(before.requests_submitted, after.requests_submitted);
        assert_eq!(before.current_queue_depth, after.current_queue_depth);
        println!("VERIFY: No panic, no empty success vector");
        println!("========================================\n");
    }
}
