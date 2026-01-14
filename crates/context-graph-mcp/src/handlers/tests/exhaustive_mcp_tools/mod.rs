//! Exhaustive MCP Tool Tests with Synthetic Data
//!
//! TASK: Comprehensive testing of ALL 45 MCP tools with known inputs/outputs.
//! This module provides:
//! - Synthetic test data with predictable outcomes
//! - Edge case testing (boundary values, invalid inputs)
//! - GWT consciousness requirement verification
//!
//! Reference: contextprd.md and constitution.yaml
//!
//! # Tool Categories (45 total)
//!
//! 1. Core (6): inject_context, store_memory, get_memetic_status, get_graph_manifest, search_graph, utl_status
//! 2. GWT/Consciousness (8): get_consciousness_state, get_kuramoto_sync, get_workspace_status, get_ego_state, trigger_workspace_broadcast, adjust_coupling, get_coherence_state, get_identity_continuity
//! 3. UTL (1): gwt/compute_delta_sc
//! 4. ATC (3): get_threshold_status, get_calibration_metrics, trigger_recalibration
//! 5. Dream (5): trigger_dream, get_dream_status, abort_dream, get_amortized_shortcuts, get_gpu_status
//! 6. Neuromod (2): get_neuromodulation_state, adjust_neuromodulator
//! 7. Steering (1): get_steering_feedback
//! 8. Causal (1): omni_infer
//! 9. Teleological (5): search_teleological, compute_teleological_vector, fuse_embeddings, update_synergy_matrix, manage_teleological_profile
//! 10. Autonomous (7): auto_bootstrap_north_star, get_alignment_drift, trigger_drift_correction, get_pruning_candidates, trigger_consolidation, discover_sub_goals, get_autonomous_status
//! 11. Meta-UTL (3): get_meta_learning_status, trigger_lambda_recalibration, get_meta_learning_log
//! 12. Epistemic (1): epistemic_action
//! 13. Merge (1): merge_concepts
//! 14. Johari (1): get_johari_classification

mod synthetic_data;
mod helpers;
mod core_tools;
mod gwt_consciousness_tools;
mod atc_tools;
mod dream_tools;
mod neuromod_tools;
mod steering_causal_tools;
mod teleological_tools;
mod autonomous_tools;
mod gwt_verification;
mod edge_cases;
mod gpu_status;
mod identity_continuity;
