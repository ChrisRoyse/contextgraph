use crate::cache::{EmbedderCache, EmbedderCacheKey};
use crate::forward::AlgorithmicEmbedderForward;
use crate::{EmbedderCacheConfig, EmbedderId, EmbedderInput};

#[tokio::test]
async fn cache_writes_and_reads_algorithmic_output_from_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = EmbedderCache::new(tmp.path()).unwrap();
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9).unwrap();
    let input = EmbedderInput {
        embedder: EmbedderId::E9,
        text: "fn cache_add(a: i32, b: i32) -> i32 { a + b }".to_string(),
        source_id: "src/cache.rs#0".to_string(),
    };

    let first = cache.forward_cached(&forward, &input).await.unwrap();
    let key = EmbedderCacheKey::for_input(&forward, &input).unwrap();
    assert!(cache.entry_path(&key).is_file());

    let second = cache.forward_cached(&forward, &input).await.unwrap();
    assert_eq!(first.vector, second.vector);
    assert_eq!(second.source_id, input.source_id);
    let telemetry = cache.telemetry().unwrap();
    assert_eq!(telemetry.hits, 1);
    assert_eq!(telemetry.misses, 1);
    assert_eq!(telemetry.writes, 1);
}

#[test]
fn cache_key_rejects_invalid_chunk_sha() {
    let key = EmbedderCacheKey::new(EmbedderId::E1, "v1", "not-a-sha".to_string());
    let err = key.validate().unwrap_err();
    assert_eq!(err.code(), "MEJEPA_EMBED_INVALID_INPUT");
}

#[tokio::test]
async fn cache_prunes_lru_entries_and_records_telemetry() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = EmbedderCache::new_with_config(
        tmp.path(),
        EmbedderCacheConfig {
            max_entries: 2,
            max_bytes: 16 * 1024 * 1024,
        },
    )
    .unwrap();
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9).unwrap();
    for idx in 0..3 {
        let input = EmbedderInput {
            embedder: EmbedderId::E9,
            text: format!("fn cache_lru_{idx}() -> i32 {{ {idx} }}"),
            source_id: format!("src/cache_lru_{idx}.rs#0"),
        };
        cache.forward_cached(&forward, &input).await.unwrap();
    }
    let telemetry = cache.telemetry().unwrap();
    assert_eq!(telemetry.entry_count, 2);
    assert_eq!(telemetry.evictions, 1);
    assert_eq!(telemetry.misses, 3);
    assert_eq!(telemetry.writes, 3);
    let report = cache.enforce_limits(EmbedderId::E9).unwrap();
    assert_eq!(report.evicted_entries, 0);
}

#[tokio::test]
async fn cache_rejects_entry_larger_than_byte_bound() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = EmbedderCache::new_with_config(
        tmp.path(),
        EmbedderCacheConfig {
            max_entries: 8,
            max_bytes: 16,
        },
    )
    .unwrap();
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9).unwrap();
    let input = EmbedderInput {
        embedder: EmbedderId::E9,
        text: "fn too_large_for_cache() -> i32 { 1 }".to_string(),
        source_id: "src/cache_large.rs#0".to_string(),
    };
    let err = cache
        .forward_cached(&forward, &input)
        .await
        .expect_err("oversized cache write unexpectedly passed");
    assert_eq!(err.code(), "MEJEPA_EMBED_FORWARD_FAILED");
    let telemetry = cache.telemetry().unwrap();
    assert_eq!(telemetry.misses, 1);
    assert_eq!(telemetry.writes, 0);
    assert_eq!(telemetry.write_rejections, 1);
    assert_eq!(telemetry.entry_count, 0);
}
