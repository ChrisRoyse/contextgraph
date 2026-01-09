//! Batch queue and request types for asynchronous embedding processing.
//!
//! This module provides the infrastructure for batching multiple embedding
//! requests together to improve GPU utilization and throughput.
//!
//! # Architecture
//!
//! ```text
//! Client             BatchQueue            BatchProcessor
//!   |                    |                      |
//!   |--BatchRequest-->  push()                  |
//!   |--BatchRequest-->  push()                  |
//!   |                    |                      |
//!   |              should_flush()               |
//!   |                    |                      |
//!   |               drain_batch()-->Batch------>|
//!   |                    |                      | (GPU inference)
//!   |<----Result---------+--------complete()----+
//! ```
//!
//! # Design Principles
//!
//! - **NO FALLBACKS**: Errors propagate immediately with full context
//! - **FAIL FAST**: Invalid state = immediate EmbeddingError
//! - **ASYNC NATIVE**: Uses tokio oneshot channels for response delivery
//! - **THREAD SAFE**: Statistics use atomics for concurrent access

mod batch;
mod queue;
mod request;
mod stats;

#[cfg(test)]
mod tests;

// Re-export all public types for backwards compatibility
pub use batch::Batch;
pub use queue::BatchQueue;
pub use request::BatchRequest;
pub use stats::{BatchQueueStats, BatchQueueSummary};
