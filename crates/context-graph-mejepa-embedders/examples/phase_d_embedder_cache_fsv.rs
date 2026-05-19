use context_graph_mejepa_embedders::cache::{EmbedderCache, EmbedderCacheKey};
use context_graph_mejepa_embedders::{
    AlgorithmicEmbedderForward, EmbedderCacheConfig, EmbedderId, EmbedderInput, EmbedderOutput,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-d-fsv";

#[derive(Debug, Serialize)]
struct EntryEvidence {
    path: String,
    byte_count: u64,
    parse_error: Option<String>,
    key: Option<Value>,
    precision_class: Option<String>,
    vector_len: Option<usize>,
    vector_prefix: Vec<f32>,
    access_count: Option<u64>,
    written_at_unix_ms: Option<u128>,
    last_accessed_unix_ms: Option<u128>,
}

#[derive(Debug, Serialize)]
struct CacheState {
    label: String,
    root: String,
    file_count: usize,
    entry_file_count: usize,
    byte_count: u64,
    telemetry_path: String,
    telemetry_json: Option<Value>,
    entries: Vec<EntryEvidence>,
}

#[derive(Debug, Serialize)]
struct EdgeCaseReport {
    case: String,
    trigger: String,
    expected: Value,
    before: CacheState,
    error_code: Option<String>,
    error_message: Option<String>,
    after: CacheState,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    source_of_truth: Value,
    happy_path_before: CacheState,
    happy_path_after_first_write: CacheState,
    happy_path_after_cache_hit: CacheState,
    cache_file: String,
    first_vector_prefix: Vec<f32>,
    second_vector_prefix: Vec<f32>,
    edge_cases: Vec<EdgeCaseReport>,
    evidence_path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(FSV_ROOT)?;
    let run_id = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("embedder-cache-fsv-{run_id}"));
    let happy_root = run_root.join("happy-path");
    let evidence_path = PathBuf::from(FSV_ROOT).join("embedder-cache-fsv.json");

    let cache = bounded_cache(&happy_root, 16, 16 * 1024 * 1024)?;
    let before = cache_state("happy_path_before", &happy_root, &cache.telemetry_path())?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let input = e9_input("fn phase_d_add(a: i32, b: i32) -> i32 { a + b }", "happy");

    let first = cache.forward_cached(&forward, &input).await?;
    validate_hdc_output(&first)?;
    let key = EmbedderCacheKey::for_input(&forward, &input)?;
    let cache_file = cache.entry_path(&key);
    let after_first = cache_state(
        "happy_path_after_first_write",
        &happy_root,
        &cache.telemetry_path(),
    )?;
    assert_entry_state(&after_first, 1, "happy path first write")?;
    assert_telemetry_u64(&after_first, "misses", 1)?;
    assert_telemetry_u64(&after_first, "writes", 1)?;

    let second = cache.forward_cached(&forward, &input).await?;
    if first.vector != second.vector {
        return Err("cache hit vector did not match first forward output".into());
    }
    let after_hit = cache_state(
        "happy_path_after_cache_hit",
        &happy_root,
        &cache.telemetry_path(),
    )?;
    assert_entry_state(&after_hit, 1, "happy path cache hit")?;
    assert_telemetry_u64(&after_hit, "hits", 1)?;
    assert_first_entry_access_count(&after_hit, 1)?;

    let report = FsvReport {
        source_of_truth: json!({
            "type": "physical_cache_directory_plus_persisted_telemetry",
            "cache_root": happy_root,
            "telemetry_file": cache.telemetry_path(),
            "entry_files": "cache_root/<embedder>/<sha256(model_version)>/<sha256(chunk_text)>.json",
            "verification": [
                "execute EmbedderCache::forward_cached",
                "read the physical cache JSON entry from disk",
                "read .embedder-cache-telemetry.json from disk",
                "assert exact counters, entry counts, and LRU eviction file presence"
            ]
        }),
        happy_path_before: before,
        happy_path_after_first_write: after_first,
        happy_path_after_cache_hit: after_hit,
        cache_file: cache_file.display().to_string(),
        first_vector_prefix: first.vector.iter().take(8).copied().collect(),
        second_vector_prefix: second.vector.iter().take(8).copied().collect(),
        edge_cases: vec![
            empty_cache_miss_edge(&run_root).await?,
            lru_max_plus_one_edge(&run_root).await?,
            corrupt_cache_edge(&run_root).await?,
            byte_limit_edge(&run_root).await?,
        ],
        evidence_path: evidence_path.display().to_string(),
    };
    fs::write(&evidence_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

async fn empty_cache_miss_edge(run_root: &Path) -> Result<EdgeCaseReport, Box<dyn Error>> {
    let root = run_root.join("edge-empty-cache");
    let cache = bounded_cache(&root, 2, 16 * 1024 * 1024)?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let input = e9_input("fn edge_empty() -> i32 { 0 }", "empty");
    let key = EmbedderCacheKey::for_input(&forward, &input)?;
    let before = cache_state("empty_cache_before_get", &root, &cache.telemetry_path())?;
    let output = cache.get(&key, input.source_id.clone())?;
    if output.is_some() {
        return Err("empty cache lookup unexpectedly returned an output".into());
    }
    let after = cache_state("empty_cache_after_get", &root, &cache.telemetry_path())?;
    assert_entry_state(&after, 0, "empty cache miss")?;
    assert_telemetry_u64(&after, "misses", 1)?;
    Ok(EdgeCaseReport {
        case: "empty_cache_miss_records_telemetry".to_string(),
        trigger: "lookup a valid E9 key in a brand-new cache root".to_string(),
        expected: json!({"entries": 0, "misses": 1}),
        before,
        error_code: None,
        error_message: None,
        after,
    })
}

async fn lru_max_plus_one_edge(run_root: &Path) -> Result<EdgeCaseReport, Box<dyn Error>> {
    let root = run_root.join("edge-lru-max-plus-one");
    let cache = bounded_cache(&root, 2, 16 * 1024 * 1024)?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let before = cache_state("lru_before_writes", &root, &cache.telemetry_path())?;
    let mut expected_evicted_path = String::new();
    let mut expected_remaining_paths = Vec::new();
    for idx in 0..3 {
        let input = e9_input(&format!("fn edge_lru_{idx}() -> i32 {{ {idx} }}"), "lru");
        let key = EmbedderCacheKey::for_input(&forward, &input)?;
        let path = cache.entry_path(&key).display().to_string();
        if idx == 0 {
            expected_evicted_path = path;
        } else {
            expected_remaining_paths.push(path);
        }
        cache.forward_cached(&forward, &input).await?;
        sleep(Duration::from_millis(2)).await;
    }
    let after = cache_state("lru_after_three_writes", &root, &cache.telemetry_path())?;
    assert_entry_state(&after, 2, "LRU max+1")?;
    assert_telemetry_u64(&after, "evictions", 1)?;
    assert_telemetry_u64(&after, "misses", 3)?;
    assert_path_absent(&after, &expected_evicted_path)?;
    for path in &expected_remaining_paths {
        assert_path_present(&after, path)?;
    }
    Ok(EdgeCaseReport {
        case: "max_plus_one_prunes_oldest_lru_entry".to_string(),
        trigger: "write three distinct E9 entries into a cache configured for max_entries=2"
            .to_string(),
        expected: json!({
            "entry_count": 2,
            "evictions": 1,
            "evicted_path": expected_evicted_path,
            "remaining_paths": expected_remaining_paths
        }),
        before,
        error_code: None,
        error_message: None,
        after,
    })
}

async fn corrupt_cache_edge(run_root: &Path) -> Result<EdgeCaseReport, Box<dyn Error>> {
    let root = run_root.join("edge-corrupt-cache");
    let cache = bounded_cache(&root, 2, 16 * 1024 * 1024)?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let input = e9_input("fn edge_corrupt() -> i32 { 7 }", "corrupt");
    let key = EmbedderCacheKey::for_input(&forward, &input)?;
    let path = cache.entry_path(&key);
    fs::create_dir_all(path.parent().ok_or("cache corrupt path missing parent")?)?;
    fs::write(&path, b"{not-valid-json")?;
    let before = cache_state("corrupt_before_forward", &root, &cache.telemetry_path())?;
    let err = cache
        .forward_cached(&forward, &input)
        .await
        .expect_err("corrupt cache unexpectedly passed as a miss");
    let after = cache_state("corrupt_after_forward", &root, &cache.telemetry_path())?;
    assert_entry_state(&after, 1, "corrupt cache")?;
    assert_telemetry_u64(&after, "corrupt_entry_errors", 1)?;
    Ok(EdgeCaseReport {
        case: "corrupt_cache_fails_closed_and_records_telemetry".to_string(),
        trigger: "place invalid JSON at the exact content-addressed cache path, then request the same key".to_string(),
        expected: json!({"corrupt_entry_errors": 1, "corrupt_file_preserved": path}),
        before,
        error_code: Some(err.code().to_string()),
        error_message: Some(err.to_string()),
        after,
    })
}

async fn byte_limit_edge(run_root: &Path) -> Result<EdgeCaseReport, Box<dyn Error>> {
    let root = run_root.join("edge-byte-limit");
    let cache = bounded_cache(&root, 8, 16)?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let input = e9_input("fn edge_too_large_for_cache() -> i32 { 1 }", "byte-limit");
    let before = cache_state("byte_limit_before_forward", &root, &cache.telemetry_path())?;
    let err = cache
        .forward_cached(&forward, &input)
        .await
        .expect_err("oversized cache write unexpectedly passed");
    let after = cache_state("byte_limit_after_forward", &root, &cache.telemetry_path())?;
    assert_entry_state(&after, 0, "byte limit rejection")?;
    assert_telemetry_u64(&after, "misses", 1)?;
    assert_telemetry_u64(&after, "write_rejections", 1)?;
    assert_telemetry_u64(&after, "writes", 0)?;
    Ok(EdgeCaseReport {
        case: "byte_limit_rejects_oversized_entry_without_partial_state".to_string(),
        trigger: "write a real E9 output into a cache configured with max_bytes=16".to_string(),
        expected: json!({
            "entries": 0,
            "misses": 1,
            "write_rejections": 1,
            "error_code": "MEJEPA_EMBED_FORWARD_FAILED"
        }),
        before,
        error_code: Some(err.code().to_string()),
        error_message: Some(err.to_string()),
        after,
    })
}

fn bounded_cache(
    root: &Path,
    max_entries: usize,
    max_bytes: u64,
) -> Result<EmbedderCache, Box<dyn Error>> {
    Ok(EmbedderCache::new_with_config(
        root,
        EmbedderCacheConfig {
            max_entries,
            max_bytes,
        },
    )?)
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

fn e9_input(text: &str, slug: &str) -> EmbedderInput {
    EmbedderInput {
        embedder: EmbedderId::E9,
        text: text.to_string(),
        source_id: format!("src/{slug}.rs#0"),
    }
}

fn cache_state(
    label: &str,
    root: &Path,
    telemetry_path: &Path,
) -> Result<CacheState, Box<dyn Error>> {
    let mut file_count = 0usize;
    let mut entry_file_count = 0usize;
    let mut byte_count = 0u64;
    let mut telemetry_json = None;
    let mut entries = Vec::new();
    if root.exists() {
        visit_files(root, &mut |path| {
            let bytes = fs::read(path)?;
            file_count += 1;
            byte_count += bytes.len() as u64;
            if path == telemetry_path {
                telemetry_json = Some(serde_json::from_slice(&bytes)?);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                entry_file_count += 1;
                entries.push(entry_evidence(path, &bytes)?);
            }
            Ok(())
        })?;
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(CacheState {
        label: label.to_string(),
        root: root.display().to_string(),
        file_count,
        entry_file_count,
        byte_count,
        telemetry_path: telemetry_path.display().to_string(),
        telemetry_json,
        entries,
    })
}

fn entry_evidence(path: &Path, bytes: &[u8]) -> Result<EntryEvidence, Box<dyn Error>> {
    let parsed: Result<Value, _> = serde_json::from_slice(bytes);
    let json = match parsed {
        Ok(value) => value,
        Err(err) => {
            return Ok(EntryEvidence {
                path: path.display().to_string(),
                byte_count: bytes.len() as u64,
                parse_error: Some(err.to_string()),
                key: None,
                precision_class: None,
                vector_len: None,
                vector_prefix: Vec::new(),
                access_count: None,
                written_at_unix_ms: None,
                last_accessed_unix_ms: None,
            });
        }
    };
    let vector = json["vector"].as_array().cloned().unwrap_or_default();
    Ok(EntryEvidence {
        path: path.display().to_string(),
        byte_count: bytes.len() as u64,
        parse_error: None,
        key: json.get("key").cloned(),
        precision_class: json["precision_class"].as_str().map(str::to_string),
        vector_len: Some(vector.len()),
        vector_prefix: vector
            .iter()
            .take(8)
            .filter_map(|value| value.as_f64().map(|number| number as f32))
            .collect(),
        access_count: json["access_count"].as_u64(),
        written_at_unix_ms: json["written_at_unix_ms"].as_u64().map(u128::from),
        last_accessed_unix_ms: json["last_accessed_unix_ms"].as_u64().map(u128::from),
    })
}

fn assert_entry_state(
    state: &CacheState,
    expected: usize,
    context: &str,
) -> Result<(), Box<dyn Error>> {
    if state.entry_file_count != expected {
        return Err(format!(
            "{context} expected {expected} cache entries, found {}",
            state.entry_file_count
        )
        .into());
    }
    Ok(())
}

fn assert_telemetry_u64(
    state: &CacheState,
    field: &str,
    expected: u64,
) -> Result<(), Box<dyn Error>> {
    let actual = state
        .telemetry_json
        .as_ref()
        .and_then(|json| json[field].as_u64())
        .ok_or_else(|| format!("missing telemetry field {field} in {}", state.label))?;
    if actual != expected {
        return Err(format!(
            "{} telemetry {field} expected {expected}, got {actual}",
            state.label
        )
        .into());
    }
    Ok(())
}

fn assert_first_entry_access_count(
    state: &CacheState,
    expected: u64,
) -> Result<(), Box<dyn Error>> {
    let actual = state
        .entries
        .first()
        .and_then(|entry| entry.access_count)
        .ok_or_else(|| format!("missing first entry access_count in {}", state.label))?;
    if actual != expected {
        return Err(format!("access_count expected {expected}, got {actual}").into());
    }
    Ok(())
}

fn assert_path_absent(state: &CacheState, path: &str) -> Result<(), Box<dyn Error>> {
    if state.entries.iter().any(|entry| entry.path == path) {
        return Err(format!("expected evicted path to be absent: {path}").into());
    }
    Ok(())
}

fn assert_path_present(state: &CacheState, path: &str) -> Result<(), Box<dyn Error>> {
    if !state.entries.iter().any(|entry| entry.path == path) {
        return Err(format!("expected remaining path to exist: {path}").into());
    }
    Ok(())
}

fn visit_files<F>(path: &Path, f: &mut F) -> Result<(), Box<dyn Error>>
where
    F: FnMut(&Path) -> Result<(), Box<dyn Error>>,
{
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
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
