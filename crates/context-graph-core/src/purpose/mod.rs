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
//! - [`PurposeVectorComputer`]: Trait for computing alignments
//! - [`DefaultPurposeComputer`]: Standard implementation
//! - [`SpladeAlignment`]: E13 keyword alignment details
//!
//! # Example
//!
//! ```ignore
//! use context_graph_core::purpose::{
//!     GoalNode, GoalHierarchy, PurposeVectorComputer,
//!     PurposeComputeConfig, DefaultPurposeComputer,
//! };
//! use context_graph_core::types::fingerprint::SemanticFingerprint;
//!
//! // Create goal hierarchy with North Star
//! let mut hierarchy = GoalHierarchy::new();
//! let north_star = GoalNode::north_star(
//!     "master_ml",
//!     "Master machine learning",
//!     vec![0.5; 1024],
//!     vec!["machine".into(), "learning".into()],
//! );
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

mod goals;
mod computer;
mod default_computer;
mod splade;

#[cfg(test)]
mod tests;

pub use goals::{GoalHierarchyError, GoalId, GoalLevel, GoalNode, GoalHierarchy};
pub use computer::{PurposeVectorComputer, PurposeComputeConfig, PurposeComputeError};
pub use default_computer::DefaultPurposeComputer;
pub use splade::SpladeAlignment;
