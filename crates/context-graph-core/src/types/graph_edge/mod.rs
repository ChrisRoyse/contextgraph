//! Graph edge connecting two memory nodes with Marblestone architecture support.
//!
//! This module provides the GraphEdge struct which represents directed relationships
//! between MemoryNodes in the Context Graph. It implements the Marblestone architecture
//! features for neurotransmitter-based weight modulation, amortized shortcuts, and
//! steering rewards.
//!
//! # Constitution Reference
//! - edge_model: Full edge specification
//! - edge_model.nt_weights: Neurotransmitter modulation formula
//! - edge_model.amortized: Shortcut learning criteria
//! - edge_model.steering_reward: [-1,1] reward signal
//!
//! # Module Structure
//! - `edge`: Core GraphEdge struct and constructors
//! - `modulation`: Neurotransmitter and steering modulation methods
//! - `traversal`: Traversal tracking and shortcut detection methods

mod edge;
mod modulation;
mod traversal;

#[cfg(test)]
mod tests_constructor;
#[cfg(test)]
mod tests_modulation;
#[cfg(test)]
mod tests_struct;
#[cfg(test)]
mod tests_traversal;

// Re-export all public items for backwards compatibility
pub use self::edge::{EdgeId, GraphEdge};
