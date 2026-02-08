//! Storage Roundtrip Tests - Comprehensive Verification of Store/Retrieve Integrity
//!
//! This test file verifies:
//! 1. StoredQuantizedFingerprint creation with all 13 embeddings
//! 2. Serialization/deserialization roundtrip preserves all data exactly
//! 3. IndexEntry creation and cosine similarity calculations
//! 4. EmbedderQueryResult and MultiSpaceQueryResult creation
//! 5. RRF formula calculations match Constitution k=60
//!
//! # CRITICAL INVARIANTS
//! - All 13 embeddings MUST be present (panic otherwise)
//! - RRF formula: 1/(60 + rank) for each space
//! - Cosine similarity in range [-1.0, 1.0]
//! - Score-based filters at 0.55 threshold
//! - Storage size should be reasonable (<25KB per fingerprint)
//!
//! # Constitution Reference
//! From constitution.yaml `embeddings.storage_per_memory`:
//! - Quantized StoredQuantizedFingerprint: ~17KB
//! - RRF(d) = sum_i 1/(k + rank_i(d)) where k=60

mod helpers;

mod comprehensive_validation;
mod edge_cases;
mod fingerprint_creation;
mod index_entry;
mod query_result;
mod rrf_formula;
mod serialization_roundtrip;
