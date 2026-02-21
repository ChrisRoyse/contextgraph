//! Comprehensive integration tests for the warm model loading system.
//!
//! # Test Groups
//!
//! 1. **Config Tests**: WarmConfig defaults, environment loading, validation
//! 2. **Error Tests**: Exit codes 101-110, error categories, fatal vs non-fatal
//! 3. **State Machine Tests**: WarmModelState transitions and predicates
//! 4. **Registry Tests**: WarmModelRegistry with all 14 models
//! 5. **Memory Pool Tests**: WarmMemoryPools dual-pool architecture
//! 6. **Validation Tests**: WarmValidator dimension/weight/inference validation
//! 7. **Handle Tests**: ModelHandle VRAM pointer tracking
//! 8. **Loader Tests**: WarmLoader orchestration logic
//! 9. **Health Check Tests**: WarmHealthChecker status monitoring
//! 10. **Diagnostics Tests**: WarmDiagnostics JSON reporting
//!
//! # Design Principles
//!
//! - **NO MOCKS**: All tests use real component instances
//! - **COMPREHENSIVE**: Cover all major code paths and edge cases
//! - **FAIL-FAST**: Verify error handling works correctly

mod config_tests;
mod diagnostics_tests;
mod error_tests;
mod handle_tests;
mod health_tests;
mod helpers;
mod integration_tests;
mod loader_tests;
mod memory_pool_tests;
mod registry_tests;
mod state_tests;
mod validation_tests;
