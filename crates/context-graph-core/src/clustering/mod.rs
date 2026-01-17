//! Multi-space clustering types for topic synthesis.
//!
//! This module provides foundational types for clustering memories across
//! the 13 embedding spaces. Used by HDBSCAN (batch) and BIRCH (incremental).
//!
//! # Architecture
//!
//! Per constitution:
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - ARCH-04: Temporal embedders (E2-E4) NEVER count toward topic detection
//!
//! # Key Types
//!
//! - [`ClusterMembership`]: Tracks which cluster a memory belongs to per space
//! - [`Cluster`]: Represents a cluster with centroid and quality metrics
//! - [`ClusterError`]: Error types for clustering operations

pub mod cluster;
pub mod error;
pub mod membership;

pub use cluster::Cluster;
pub use error::ClusterError;
pub use membership::ClusterMembership;
