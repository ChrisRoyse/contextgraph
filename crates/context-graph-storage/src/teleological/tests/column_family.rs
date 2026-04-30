//! Column family configuration tests.

use crate::teleological::*;

// =========================================================================
// Column Family Tests
// =========================================================================

#[test]
fn test_teleological_cf_names_count() {
    // 41 active teleological CFs (core teleological + training/export
    // + learning events + 11 learner-state UTL CFs
    //  + CF_CONSTELLATIONS + CF_CONSTELLATION_BY_SELECTOR
    //  + CF_CONTRASTIVE_PAIRS + CF_CONTRASTIVE_BY_KIND + CF_CONTRASTIVE_BY_ANCHOR
    //  + CF_TYPED_EDGE_RECORDS + CF_TYPED_EDGE_VALIDATIONS
    //  + CF_LEARNER_TRAINING_DATASETS)
    assert_eq!(
        TELEOLOGICAL_CFS.len(),
        TELEOLOGICAL_CF_COUNT,
        "Must have exactly {} teleological column families",
        TELEOLOGICAL_CF_COUNT
    );
    assert_eq!(TELEOLOGICAL_CF_COUNT, 41);
}

#[test]
fn test_teleological_cf_names_unique() {
    use std::collections::HashSet;
    let set: HashSet<_> = TELEOLOGICAL_CFS.iter().collect();
    assert_eq!(
        set.len(),
        TELEOLOGICAL_CF_COUNT,
        "All CF names must be unique"
    );
}

#[test]
fn test_teleological_cf_names_are_snake_case() {
    for name in TELEOLOGICAL_CFS {
        assert!(
            name.chars()
                .all(|c| c.is_lowercase() || c == '_' || c.is_ascii_digit()),
            "CF name '{}' should be snake_case",
            name
        );
    }
}

#[test]
fn test_teleological_cf_names_values() {
    // Original 4 CFs
    assert_eq!(CF_FINGERPRINTS, "fingerprints");
    assert_eq!(CF_TOPIC_PROFILES, "topic_profiles");
    assert_eq!(CF_E13_SPLADE_INVERTED, "e13_splade_inverted");
    assert_eq!(CF_E1_MATRYOSHKA_128, "e1_matryoshka_128");
}

#[test]
fn test_all_cfs_in_array() {
    assert!(TELEOLOGICAL_CFS.contains(&CF_FINGERPRINTS));
    assert!(TELEOLOGICAL_CFS.contains(&CF_TOPIC_PROFILES));
    assert!(TELEOLOGICAL_CFS.contains(&CF_E13_SPLADE_INVERTED));
    assert!(TELEOLOGICAL_CFS.contains(&CF_E1_MATRYOSHKA_128));
}

#[test]
fn test_fingerprint_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = fingerprint_cf_options(&cache);
    drop(opts); // Should not panic
}

#[test]
fn test_topic_profile_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = topic_profile_cf_options(&cache);
    drop(opts);
}

#[test]
fn test_e13_splade_inverted_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = e13_splade_inverted_cf_options(&cache);
    drop(opts);
}

#[test]
fn test_e1_matryoshka_128_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = e1_matryoshka_128_cf_options(&cache);
    drop(opts);
}

#[test]
fn test_get_teleological_cf_descriptors_returns_7() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let descriptors = get_teleological_cf_descriptors(&cache);
    assert_eq!(
        descriptors.len(),
        TELEOLOGICAL_CF_COUNT,
        "Must return exactly {} descriptors",
        TELEOLOGICAL_CF_COUNT
    );
}

// =========================================================================
// TASK-TELEO-006: New CF Option Builder Tests
// =========================================================================

#[test]
fn test_custom_weight_profiles_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = custom_weight_profiles_cf_options(&cache);
    drop(opts); // Should not panic
}

// =========================================================================
// TASK-TELEO-006: CF Descriptor Order Tests
// =========================================================================

#[test]
fn test_descriptors_in_correct_order() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let descriptors = get_teleological_cf_descriptors(&cache);

    // Verify order matches TELEOLOGICAL_CFS
    for (i, cf_name) in TELEOLOGICAL_CFS.iter().enumerate() {
        assert_eq!(
            descriptors[i].name(),
            *cf_name,
            "Descriptor {} should be '{}', got '{}'",
            i,
            cf_name,
            descriptors[i].name()
        );
    }
}

#[test]
fn test_get_all_teleological_cf_descriptors_returns_41() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let descriptors = get_all_teleological_cf_descriptors(&cache);

    // 41 teleological + 14 quantized embedder = 55 (post-E14 BGE-M3).
    // Quantized (14): emb_0 through emb_13.
    assert_eq!(
        descriptors.len(),
        TELEOLOGICAL_CF_COUNT + QUANTIZED_EMBEDDER_CF_COUNT,
        "Must return 41 teleological + 14 quantized = 55 CFs"
    );
}

// =========================================================================
// TASK-TELEO-006: Edge Case Tests (with before/after state printing)
// =========================================================================

#[test]
fn edge_case_multiple_cache_references_for_cfs() {
    println!("=== EDGE CASE: Multiple option builders sharing same cache ===");
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);

    println!("BEFORE: Creating options with shared cache reference");
    let opts1 = fingerprint_cf_options(&cache);
    let opts2 = custom_weight_profiles_cf_options(&cache);

    println!("AFTER: Option builders created successfully");
    drop(opts1);
    drop(opts2);
    println!("RESULT: PASS - Shared cache works across Options");
}
