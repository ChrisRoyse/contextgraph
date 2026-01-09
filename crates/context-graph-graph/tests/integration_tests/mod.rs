//! M04-T25: Modular Integration Tests for Knowledge Graph System.
//!
//! This module provides end-to-end integration testing for the context-graph-graph crate,
//! verifying all major subsystems work together correctly with REAL DATA.
//!
//! # Constitution References
//!
//! - REQ-KG-TEST: No mocks in production tests
//! - AP-001: Never unwrap() in prod - fail fast with proper errors
//! - AP-009: All weights must be in [0.0, 1.0]
//!
//! # NFR Targets
//!
//! - FAISS k=100 search: <2ms
//! - Poincare GPU 1kx1k: <1ms
//! - Cone GPU 1kx1k: <2ms
//! - BFS depth 6: <100ms
//! - Domain search: <10ms
//! - Entailment query: <1ms/cone
//!
//! # Test Modules
//!
//! 1. `storage_tests` - Storage lifecycle, batch operations, CRUD
//! 2. `hyperbolic_tests` - Poincare ball geometry and distance properties
//! 3. `cone_tests` - Entailment cone containment and storage
//! 4. `traversal_tests` - Graph traversal with NT modulation
//! 5. `search_tests` - Semantic search and contradiction detection
//! 6. `workflow_tests` - End-to-end workflow integration
//! 7. `edge_case_tests` - Boundary conditions and edge cases
//! 8. `nfr_tests` - NFR timing tests
//! 9. `canonical_formula_tests` - M04-T27 canonical formula consistency
//! 10. `batch_formula_tests` - M04-T27 batch formula comparison

pub mod batch_formula_tests;
pub mod canonical_formula_tests;
pub mod cone_tests;
pub mod edge_case_tests;
pub mod hyperbolic_tests;
pub mod nfr_tests;
pub mod search_tests;
pub mod storage_tests;
pub mod traversal_tests;
pub mod workflow_tests;
