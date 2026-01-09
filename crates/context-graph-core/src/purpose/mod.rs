//! Purpose Vector computation module.
//!
//! Provides goal hierarchy management and purpose vector computation
//! for aligning memories to North Star goals across 13 embedding spaces.
//!
//! # Architecture
//!
//! This module implements the computation logic for Purpose Vectors as defined
//! in constitution.yaml. The `PurposeVector` struct (stored in fingerprint/purpose.rs)
//! is the output of this computation.
//!
//! ## Components
//!
//! - [`GoalHierarchy`]: Tree structure for organizing goals
//! - [`GoalNode`]: Individual goal with TeleologicalArray for apples-to-apples comparison
//! - [`GoalDiscoveryMetadata`]: How goals were autonomously discovered
//! - [`PurposeVectorComputer`]: Trait for computing alignments
//! - [`DefaultPurposeComputer`]: Standard implementation
//! - [`SpladeAlignment`]: E13 keyword alignment details
//!
//! # Architecture (constitution.yaml)
//!
//! - **ARCH-02**: Goals use TeleologicalArray for apples-to-apples comparison
//! - **ARCH-03**: Goals are discovered autonomously, not manually created
//! - **ARCH-05**: All 13 embedders must be present in teleological_array
//!
//! # Example
//!
//! ```ignore
//! use context_graph_core::purpose::{
//!     GoalNode, GoalHierarchy, GoalLevel, GoalDiscoveryMetadata, DiscoveryMethod,
//!     PurposeVectorComputer, PurposeComputeConfig, DefaultPurposeComputer,
//! };
//! use context_graph_core::types::fingerprint::SemanticFingerprint;
//!
//! // Create goal hierarchy with North Star (discovered autonomously)
//! let mut hierarchy = GoalHierarchy::new();
//! let discovery = GoalDiscoveryMetadata::bootstrap();
//! let north_star = GoalNode::autonomous_goal(
//!     "Emergent ML mastery goal".into(),
//!     GoalLevel::NorthStar,
//!     SemanticFingerprint::zeroed(),
//!     discovery,
//! ).unwrap();
//! hierarchy.add_goal(north_star).unwrap();
//!
//! // Compute purpose vector
//! let computer = DefaultPurposeComputer::new();
//! let fingerprint = SemanticFingerprint::zeroed();
//! let config = PurposeComputeConfig {
//!     hierarchy,
//!     ..Default::default()
//! };
//!
//! // In async context:
//! // let purpose = computer.compute_purpose(&fingerprint, &config).await?;
//! ```

mod computer;
mod default_computer;
mod goals;
mod splade;

#[cfg(test)]
mod tests;

// Re-export goal types (TASK-CORE-005)
// NOTE: GoalId is REMOVED - use uuid::Uuid directly
pub use computer::{PurposeComputeConfig, PurposeComputeError, PurposeVectorComputer};
pub use default_computer::DefaultPurposeComputer;
pub use goals::{
    DiscoveryMethod, GoalDiscoveryMetadata, GoalHierarchy, GoalHierarchyError, GoalLevel, GoalNode,
    GoalNodeError,
};
pub use splade::SpladeAlignment;
