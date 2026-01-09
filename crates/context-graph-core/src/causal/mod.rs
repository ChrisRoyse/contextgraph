//! Causal Inference Module
//!
//! Implements structural causal models and omni-directional inference.
//!
//! ## Constitution Reference
//!
//! See `omni_infer` tool requirements (line 539).
//!
//! ## Features
//!
//! - Structural Causal Models (SCM) for representing causal relationships
//! - Omni-directional inference supporting 5 modes:
//!   - Forward: A -> B (effect of A on B)
//!   - Backward: B -> A (cause of B)
//!   - Bidirectional: A <-> B (mutual influence)
//!   - Bridge: Cross-domain causal bridging
//!   - Abduction: Best hypothesis for observation
//! - **E5 Asymmetric Similarity**: Constitution-specified causal similarity
//!   with direction modifiers (cause→effect=1.2, effect→cause=0.8)
//!
//! ## NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING

pub mod asymmetric;
pub mod inference;
pub mod scm;

pub use asymmetric::{
    adjust_batch_similarities, compute_asymmetric_similarity, compute_asymmetric_similarity_simple,
    CausalDirection, InterventionContext,
};
pub use inference::{InferenceDirection, InferenceResult, OmniInfer};
pub use scm::{CausalEdge, CausalGraph, CausalNode};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_causal_module_exports() {
        // Verify all types are exported correctly
        let _infer = OmniInfer::new();
        let _graph = CausalGraph::new();
        let _dir = InferenceDirection::Forward;
    }

    #[test]
    fn test_basic_inference() {
        let infer = OmniInfer::new();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let results = infer
            .infer(source, Some(target), InferenceDirection::Forward)
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].direction, InferenceDirection::Forward);
        assert_eq!(results[0].source, source);
        assert_eq!(results[0].target, target);
    }

    #[test]
    fn test_causal_graph_operations() {
        let mut graph = CausalGraph::new();

        let node1 = CausalNode {
            id: Uuid::new_v4(),
            name: "Event A".to_string(),
            domain: "physics".to_string(),
        };
        let node2 = CausalNode {
            id: Uuid::new_v4(),
            name: "Event B".to_string(),
            domain: "physics".to_string(),
        };

        let node1_id = node1.id;
        let node2_id = node2.id;

        graph.add_node(node1);
        graph.add_node(node2);

        graph.add_edge(CausalEdge {
            source: node1_id,
            target: node2_id,
            strength: 0.8,
            mechanism: "direct causation".to_string(),
        });

        let effects = graph.get_effects(node1_id);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].target, node2_id);

        let causes = graph.get_causes(node2_id);
        assert_eq!(causes.len(), 1);
        assert_eq!(causes[0].source, node1_id);
    }
}
