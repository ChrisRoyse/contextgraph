//! Teleological Logic Layer Services.
//!
//! This module provides the computational services for the teleological fusion system.
//! These services implement TASKS-TELEO 007-015.
//!
//! # Services
//!
//! - **SynergyService** (007): Cross-embedding synergy computation
//! - **CorrelationExtractor** (008): 78 cross-correlation value extraction
//! - **MeaningPipeline** (009): Embedding → meaning extraction pipeline
//! - **TuckerDecomposer** (010): Tensor factorization for compression
//! - **GroupAggregator** (011): 13D → 6D group aggregation
//! - **FusionEngine** (012): Multi-embedding fusion orchestration
//! - **MultiSpaceRetriever** (013): Teleological-aware retrieval
//! - **FeedbackLearner** (014): GWT feedback learning loop
//! - **ProfileManager** (015): Task-specific profile management

pub mod correlation_extractor;
pub mod feedback_learner;
pub mod fusion_engine;
pub mod group_aggregator;
pub mod meaning_pipeline;
pub mod multi_space_retriever;
pub mod profile_manager;
pub mod synergy_service;
pub mod tucker_decomposer;

// Re-exports
pub use correlation_extractor::CorrelationExtractor;
pub use feedback_learner::FeedbackLearner;
pub use fusion_engine::FusionEngine;
pub use group_aggregator::GroupAggregator;
pub use meaning_pipeline::MeaningPipeline;
pub use multi_space_retriever::MultiSpaceRetriever;
pub use profile_manager::ProfileManager;
pub use synergy_service::SynergyService;
pub use tucker_decomposer::TuckerDecomposer;
