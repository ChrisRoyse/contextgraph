use context_graph_mejepa_embedders::cache::{EmbedderCache, EmbedderCacheKey};
use context_graph_mejepa_embedders::{
    AlgorithmicEmbedderForward, EmbedderId, EmbedderInput, EmbedderOutput,
};
use serde::Serialize;
use serde_json::json;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-d-fsv";

#[derive(Debug, Serialize)]
struct CacheState {
    label: String,
    file_count: usize,
    byte_count: u64,
    root: String,
}

#[derive(Debug, Serialize)]
struct EdgeCaseReport {
    case: String,
    before: CacheState,
    error_code: String,
    error_message: String,
    after: CacheState,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    source_of_truth: serde_json::Value,
    before: CacheState,
    after_first_write: CacheState,
    after_cache_hit: CacheState,
    cache_file: String,
    cached_json: serde_json::Value,
    first_vector_prefix: Vec<f32>,
    second_vector_prefix: Vec<f32>,
    edge_cases: Vec<EdgeCaseReport>,
    evidence_path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(FSV_ROOT)?;
    let run_id = unix_ms()?;
    let cache_root = PathBuf::from(FSV_ROOT).join(format!("embedder-cache-{run_id}"));
    let evidence_path = PathBuf::from(FSV_ROOT).join("embedder-cache-fsv.json");
    let cache = EmbedderCache::new(&cache_root)?;
    let before = cache_state("before_happy_path", cache.root())?;

    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let input = EmbedderInput {
        embedder: EmbedderId::E9,
        text: "fn phase_d_add(a: i32, b: i32) -> i32 { a + b }".to_string(),
        source_id: "src/phase_d.rs#0".to_string(),
    };
    let first = cache.forward_cached(&forward, &input).await?;
    validate_hdc_output(&first)?;
    let key = EmbedderCacheKey::for_input(&forward, &input)?;
    let cache_file = cache.entry_path(&key);
    let after_first_write = cache_state("after_first_write", cache.root())?;
    let cached_json: serde_json::Value = serde_json::from_slice(&fs::read(&cache_file)?)?;

    let second = cache.forward_cached(&forward, &input).await?;
    if first.vector != second.vector {
        return Err("cache hit vector did not match first forward output".into());
    }
    let after_cache_hit = cache_state("after_cache_hit", cache.root())?;
    if after_first_write.file_count != after_cache_hit.file_count {
        return Err("cache hit unexpectedly changed cache file count".into());
    }

    let edge_cases = vec![
        invalid_key_edge(&cache_root)?,
        missing_temporal_timestamp_edge(&cache).await?,
        corrupt_cache_edge(&cache).await?,
    ];

    let report = FsvReport {
        source_of_truth: json!({
            "type": "content_addressed_json_files",
            "cache_root": cache_root,
            "key_fields": ["schema_version", "embedder", "model_version", "chunk_sha256"],
            "verification": "write via EmbedderCache::forward_cached, inspect physical JSON file, then perform a second independent cache read"
        }),
        before,
        after_first_write,
        after_cache_hit,
        cache_file: cache_file.display().to_string(),
        cached_json,
        first_vector_prefix: first.vector.iter().take(8).copied().collect(),
        second_vector_prefix: second.vector.iter().take(8).copied().collect(),
        edge_cases,
        evidence_path: evidence_path.display().to_string(),
    };
    fs::write(&evidence_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn validate_hdc_output(output: &EmbedderOutput) -> Result<(), Box<dyn Error>> {
    output.validate()?;
    if output.vector.iter().all(|value| *value == 0.0) {
        return Err("E9 algorithmic forward returned an all-zero vector".into());
    }
    if output.precision_class != "algorithmic_real_forward" {
        return Err("unexpected precision_class for E9 output".into());
    }
    Ok(())
}

fn invalid_key_edge(cache_root: &Path) -> Result<EdgeCaseReport, Box<dyn Error>> {
    let before = cache_state("invalid_key_before", cache_root)?;
    let key = EmbedderCacheKey::new(EmbedderId::E9, "phase-d", "not-a-sha".to_string());
    let err = key.validate().expect_err("invalid key unexpectedly passed");
    let after = cache_state("invalid_key_after", cache_root)?;
    if before.file_count != after.file_count {
        return Err("invalid key edge mutated cache files".into());
    }
    Ok(EdgeCaseReport {
        case: "invalid_chunk_sha_rejected".to_string(),
        before,
        error_code: err.code().to_string(),
        error_message: err.to_string(),
        after,
    })
}

async fn missing_temporal_timestamp_edge(
    cache: &EmbedderCache,
) -> Result<EdgeCaseReport, Box<dyn Error>> {
    let before = cache_state("missing_temporal_timestamp_before", cache.root())?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E2)?;
    let input = EmbedderInput {
        embedder: EmbedderId::E2,
        text: "temporal chunk without explicit time".to_string(),
        source_id: "src/lib.rs#0".to_string(),
    };
    let err = cache
        .forward_cached(&forward, &input)
        .await
        .expect_err("missing timestamp unexpectedly passed");
    let after = cache_state("missing_temporal_timestamp_after", cache.root())?;
    if before.file_count != after.file_count {
        return Err("missing temporal timestamp edge mutated cache files".into());
    }
    Ok(EdgeCaseReport {
        case: "missing_temporal_timestamp_rejected".to_string(),
        before,
        error_code: err.code().to_string(),
        error_message: err.to_string(),
        after,
    })
}

async fn corrupt_cache_edge(cache: &EmbedderCache) -> Result<EdgeCaseReport, Box<dyn Error>> {
    let before = cache_state("corrupt_cache_before", cache.root())?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let input = EmbedderInput {
        embedder: EmbedderId::E9,
        text: "fn corrupt_cache_probe() -> i32 { 7 }".to_string(),
        source_id: "src/corrupt.rs#0".to_string(),
    };
    let key = EmbedderCacheKey::for_input(&forward, &input)?;
    let path = cache.entry_path(&key);
    fs::create_dir_all(path.parent().ok_or("cache corrupt path missing parent")?)?;
    fs::write(&path, b"{not-valid-json")?;
    let err = cache
        .forward_cached(&forward, &input)
        .await
        .expect_err("corrupt cache unexpectedly passed as a miss");
    let after = cache_state("corrupt_cache_after", cache.root())?;
    if after.file_count != before.file_count + 1 {
        return Err(
            "corrupt cache edge did not leave the corrupt physical file for inspection".into(),
        );
    }
    Ok(EdgeCaseReport {
        case: "corrupt_cache_fails_closed".to_string(),
        before,
        error_code: err.code().to_string(),
        error_message: err.to_string(),
        after,
    })
}

fn cache_state(label: &str, root: &Path) -> Result<CacheState, Box<dyn Error>> {
    let mut file_count = 0usize;
    let mut byte_count = 0u64;
    if root.exists() {
        visit_files(root, &mut |path| {
            if path.is_file() {
                file_count += 1;
                byte_count += fs::metadata(path)?.len();
            }
            Ok(())
        })?;
    }
    Ok(CacheState {
        label: label.to_string(),
        file_count,
        byte_count,
        root: root.display().to_string(),
    })
}

fn visit_files<F>(path: &Path, f: &mut F) -> Result<(), Box<dyn Error>>
where
    F: FnMut(&Path) -> Result<(), Box<dyn Error>>,
{
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_files(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}

fn unix_ms() -> Result<u128, Box<dyn Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}
