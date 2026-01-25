//! TASK-TELEO-014: FeedbackLearner Implementation
//!
//! Implements feedback learning loop for the teleological system. The service
//! collects feedback events from embedder interactions and uses gradient-based
//! learning to adjust embedder weights for improved teleological alignment.
//!
//! # Core Responsibilities
//!
//! 1. Record positive/negative/neutral feedback events
//! 2. Compute gradients from accumulated feedback
//! 3. Apply momentum-based gradient updates
//! 4. Track per-embedder adjustment values
//! 5. Integrate with coherence feedback
//!
//! Feedback provides the 'reward signal' - when a retrieval leads to successful
//! task completion (high coherence), we reinforce the embedder weights that
//! contributed most to that retrieval.

mod config;
mod learner;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public items for backwards compatibility
pub use config::FeedbackLearnerConfig;
pub use learner::FeedbackLearner;
pub use types::{FeedbackEvent, FeedbackType, LearningResult};
