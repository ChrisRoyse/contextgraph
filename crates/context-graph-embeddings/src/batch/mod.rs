//! Batch processing infrastructure for embedding requests.
//!
//! This module provides types and utilities for batching multiple embedding
//! requests together to improve GPU utilization and overall throughput.
//!
//! # Architecture
//!
//! The batch system consists of three main components:
//!
//! - **`BatchRequest`**: Individual embedding request with async response channel
//! - **`BatchQueue`**: Per-model queue that collects and organizes pending requests
//! - **`Batch`**: Assembled batch ready for GPU processing
//!
//! # Example Flow
//!
//! ```text
//! Client 1 ─┬─► BatchQueue ──► Batch ──► Model ──► Results
//! Client 2 ─┤    (collect)    (assemble)  (GPU)   (distribute)
//! Client 3 ─┘
//! ```
//!
//! # Usage
//!
//! ```
//! # use context_graph_embeddings::batch::{BatchQueue, BatchRequest};
//! # use context_graph_embeddings::config::BatchConfig;
//! # use context_graph_embeddings::types::{ModelId, ModelInput};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create queue for a model
//! let config = BatchConfig::default();
//! let mut queue = BatchQueue::new(ModelId::Semantic, config);
//!
//! // Submit requests
//! let input = ModelInput::text("Hello, world!")?;
//! let (request, _receiver) = BatchRequest::new(input, ModelId::Semantic);
//! queue.push(request);
//!
//! // Check queue state
//! assert_eq!(queue.len(), 1);
//! assert!(!queue.is_empty());
//!
//! // Extract batch for processing
//! if let Some(batch) = queue.drain_batch() {
//!     assert_eq!(batch.len(), 1);
//!     // In real code: run inference, then batch.complete(results)
//!     // For doc test: just fail the batch to clean up
//!     batch.fail("doc test cleanup");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Errors propagate immediately with full context
//! - **FAIL FAST**: Invalid state = immediate error
//! - **ASYNC NATIVE**: Uses tokio oneshot channels for response delivery
//! - **THREAD SAFE**: Statistics use atomics for concurrent access

mod processor;
mod types;

pub use processor::{BatchProcessor, BatchProcessorConfig, BatchProcessorStats};
pub use types::{Batch, BatchQueue, BatchQueueStats, BatchQueueSummary, BatchRequest};
