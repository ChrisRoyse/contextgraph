//! Batch request types for embedding processing.
//!
//! This module provides the `BatchRequest` type for submitting
//! individual embedding requests to the batch system.

use std::time::Instant;

use tokio::sync::oneshot;
use uuid::Uuid;

use crate::error::EmbeddingResult;
use crate::types::{ModelEmbedding, ModelId, ModelInput};

/// Individual embedding request submitted to the batch system.
///
/// Each request carries its input, target model, and a response channel
/// for asynchronous result delivery.
///
/// # Example
///
/// ```
/// # use context_graph_embeddings::batch::BatchRequest;
/// # use context_graph_embeddings::types::{ModelId, ModelInput};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let input = ModelInput::text("Hello, world!")?;
/// let (request, _receiver) = BatchRequest::new(input, ModelId::Semantic);
///
/// // Request has a unique ID
/// assert!(!request.id.is_nil());
/// // Request tracks model and priority
/// assert_eq!(request.model_id, ModelId::Semantic);
/// assert_eq!(request.priority, 0);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct BatchRequest {
    /// Unique request identifier for tracking and debugging.
    pub id: Uuid,

    /// Input to embed.
    pub input: ModelInput,

    /// Target model for embedding.
    pub model_id: ModelId,

    /// Channel for returning result.
    /// Consumed when the request is completed.
    pub response_tx: oneshot::Sender<EmbeddingResult<ModelEmbedding>>,

    /// Timestamp when request was submitted.
    /// Used for timeout calculations and metrics.
    pub submitted_at: Instant,

    /// Priority level (higher = more urgent).
    /// Default is 0. Higher values are processed first.
    pub priority: u8,
}

impl BatchRequest {
    /// Create a new batch request with default priority.
    ///
    /// Returns the request and a receiver for the result.
    ///
    /// # Arguments
    /// * `input` - The input to embed
    /// * `model_id` - The model to use for embedding
    ///
    /// # Returns
    /// A tuple of (request, receiver) where:
    /// - `request` should be submitted to a BatchQueue
    /// - `receiver` will receive the embedding result
    ///
    /// # Example
    ///
    /// ```
    /// # use context_graph_embeddings::batch::BatchRequest;
    /// # use context_graph_embeddings::types::{ModelId, ModelInput};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let input = ModelInput::text("Hello")?;
    /// let (request, _receiver) = BatchRequest::new(input, ModelId::Semantic);
    /// assert_eq!(request.model_id, ModelId::Semantic);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new(
        input: ModelInput,
        model_id: ModelId,
    ) -> (Self, oneshot::Receiver<EmbeddingResult<ModelEmbedding>>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            id: Uuid::new_v4(),
            input,
            model_id,
            response_tx: tx,
            submitted_at: Instant::now(),
            priority: 0,
        };
        (request, rx)
    }

    /// Create a new batch request with specified priority.
    ///
    /// # Arguments
    /// * `input` - The input to embed
    /// * `model_id` - The model to use for embedding
    /// * `priority` - Priority level (higher = more urgent, 0-255)
    #[must_use]
    pub fn with_priority(
        input: ModelInput,
        model_id: ModelId,
        priority: u8,
    ) -> (Self, oneshot::Receiver<EmbeddingResult<ModelEmbedding>>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            id: Uuid::new_v4(),
            input,
            model_id,
            response_tx: tx,
            submitted_at: Instant::now(),
            priority,
        };
        (request, rx)
    }

    /// Time elapsed since submission.
    ///
    /// Used for timeout checking and metrics.
    #[inline]
    #[must_use]
    pub fn elapsed(&self) -> std::time::Duration {
        self.submitted_at.elapsed()
    }

    /// Estimated token count for batching decisions.
    ///
    /// This is a rough estimate used for padding calculations:
    /// - Text: ~4 characters per token
    /// - Code: ~3 characters per token (more token-dense)
    /// - Image/Audio: fixed estimate of 100 tokens
    ///
    /// # Returns
    /// Estimated number of tokens for this input.
    #[must_use]
    pub fn estimated_tokens(&self) -> usize {
        match &self.input {
            ModelInput::Text {
                content,
                instruction,
            } => {
                // Include instruction in estimate if present
                let total_len = content.len() + instruction.as_ref().map_or(0, |s: &String| s.len());
                // Rough estimate: 4 chars per token, minimum 1
                (total_len / 4).max(1)
            }
            ModelInput::Code { content, .. } => {
                // Code is often more token-dense
                (content.len() / 3).max(1)
            }
            // Non-text inputs get fixed estimate
            ModelInput::Image { .. } | ModelInput::Audio { .. } => 100,
        }
    }
}
