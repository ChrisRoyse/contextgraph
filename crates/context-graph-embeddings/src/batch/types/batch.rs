//! Assembled batch for GPU processing.
//!
//! This module provides the `Batch` type which represents a collection
//! of embedding requests ready for processing together.

use std::time::Instant;

use tokio::sync::oneshot;
use uuid::Uuid;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::{ModelEmbedding, ModelId, ModelInput};

use super::request::BatchRequest;

/// Assembled batch ready for processing.
///
/// Contains all the inputs to be processed together and the channels
/// to send results back to individual requesters.
///
/// # Lifecycle
///
/// 1. Created empty with `Batch::new()`
/// 2. Requests added with `add()`
/// 3. Processed by embedding model
/// 4. Results distributed with `complete()` or `fail()`
///
/// # Example
///
/// ```
/// # use context_graph_embeddings::batch::{Batch, BatchRequest};
/// # use context_graph_embeddings::types::{ModelId, ModelInput};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut batch = Batch::new(ModelId::Semantic);
/// assert!(batch.is_empty());
///
/// // Add requests to batch
/// let input1 = ModelInput::text("Hello")?;
/// let (request1, _rx1) = BatchRequest::new(input1, ModelId::Semantic);
/// batch.add(request1);
///
/// let input2 = ModelInput::text("World")?;
/// let (request2, _rx2) = BatchRequest::new(input2, ModelId::Semantic);
/// batch.add(request2);
///
/// assert_eq!(batch.len(), 2);
/// // Batch would be processed then completed with batch.complete(results)
/// // For doc test cleanup:
/// batch.fail("doc test");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Batch {
    /// Batch identifier for tracking.
    pub id: Uuid,

    /// Model to use.
    pub model_id: ModelId,

    /// Inputs in this batch.
    pub inputs: Vec<ModelInput>,

    /// Response channels (same order as inputs).
    pub response_txs: Vec<oneshot::Sender<EmbeddingResult<ModelEmbedding>>>,

    /// Original request IDs for tracking.
    pub request_ids: Vec<Uuid>,

    /// When batch was assembled.
    pub assembled_at: Instant,

    /// Total estimated tokens in batch (for padding estimation).
    pub total_tokens: usize,
}

impl Batch {
    /// Create a new empty batch for a model.
    #[must_use]
    pub fn new(model_id: ModelId) -> Self {
        Self {
            id: Uuid::new_v4(),
            model_id,
            inputs: Vec::new(),
            response_txs: Vec::new(),
            request_ids: Vec::new(),
            assembled_at: Instant::now(),
            total_tokens: 0,
        }
    }

    /// Add a request to the batch.
    ///
    /// Consumes the request and stores its components.
    pub fn add(&mut self, request: BatchRequest) {
        self.total_tokens += request.estimated_tokens();
        self.request_ids.push(request.id);
        self.inputs.push(request.input);
        self.response_txs.push(request.response_tx);
    }

    /// Number of items in batch.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inputs.len()
    }

    /// Check if batch is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    /// Time since batch was assembled.
    #[inline]
    #[must_use]
    pub fn elapsed(&self) -> std::time::Duration {
        self.assembled_at.elapsed()
    }

    /// Send results back to requesters.
    ///
    /// Consumes the batch and sends each result to its corresponding
    /// response channel. Results must be in the same order as the inputs.
    ///
    /// # Arguments
    /// * `results` - Embedding results, one per input
    ///
    /// # Panics
    /// Panics if `results.len() != self.len()` in debug builds.
    pub fn complete(self, results: Vec<EmbeddingResult<ModelEmbedding>>) {
        debug_assert_eq!(
            self.response_txs.len(),
            results.len(),
            "Results count ({}) must match batch size ({})",
            results.len(),
            self.response_txs.len()
        );

        for (tx, result) in self.response_txs.into_iter().zip(results.into_iter()) {
            // Ignore send errors (receiver may have dropped)
            let _ = tx.send(result);
        }
    }

    /// Send error to all requesters.
    ///
    /// Consumes the batch and sends a BatchError to all response channels.
    ///
    /// # Arguments
    /// * `message` - The error message to send to all requesters
    pub fn fail(self, message: impl Into<String>) {
        let msg = message.into();
        for tx in self.response_txs {
            let _ = tx.send(Err(EmbeddingError::BatchError {
                message: msg.clone(),
            }));
        }
    }

    /// Get the maximum estimated tokens across all inputs.
    ///
    /// Useful for determining padding requirements.
    #[must_use]
    pub fn max_tokens(&self) -> usize {
        // We need to estimate from inputs since we don't store per-request tokens
        self.inputs
            .iter()
            .map(|input| match input {
                ModelInput::Text {
                    content,
                    instruction,
                } => {
                    let total_len =
                        content.len() + instruction.as_ref().map_or(0, |s: &String| s.len());
                    (total_len / 4).max(1)
                }
                ModelInput::Code { content, .. } => (content.len() / 3).max(1),
                ModelInput::Image { .. } | ModelInput::Audio { .. } => 100,
            })
            .max()
            .unwrap_or(0)
    }
}
