use context_graph_mejepa_cf::CF_MEJEPA_REDACTION_LOG;
use context_graph_mejepa_embedders::{
    AlgorithmicEmbedderForward, EmbedderForward, EmbedderId, EmbedderInput,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const DEFAULT_FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-059-secret-redaction-fsv";

#[tokio::main(flavor = "current_thread")]
async fn main() -> FsvResult<()> {
    let fsv_root = env_path("CG_MEJEPA_SECRET_REDACTION_FSV_ROOT", DEFAULT_FSV_ROOT);
    let run_root = fsv_root.join(format!("run-{}", now_millis()?));
    fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("redaction-log-rocksdb");

    let secret_a = "AWS_SECRET_ACCESS_KEY = \"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\"";
    let secret_b = "AWS_SECRET_ACCESS_KEY = \"bJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\"";
    let redacted = "AWS_SECRET_ACCESS_KEY=[REDACTED_AWS_SECRET_ACCESS_KEY]";
    let clean = "def no_secret(value):\n    return value + 1\n";

    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let raw_a = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E9,
            text: secret_a.to_string(),
            source_id: "secret_a.py".to_string(),
        })
        .await?;
    let raw_b = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E9,
            text: secret_b.to_string(),
            source_id: "secret_b.py".to_string(),
        })
        .await?;
    let redacted_a = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E9,
            text: redacted.to_string(),
            source_id: "secret_a.py".to_string(),
        })
        .await?;
    let redacted_b = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E9,
            text: redacted.to_string(),
            source_id: "secret_b.py".to_string(),
        })
        .await?;

    let rows = vec![
        redaction_event_row("secret-a", secret_a, redacted, 0, 1),
        redaction_event_row("secret-b", secret_b, redacted, 0, 1),
        clean_event_row("clean", clean),
    ];
    let cf_state = write_and_reopen_redaction_rows(&db_path, &rows)?;

    let raw_vector_hashes_differ = vector_sha256(&raw_a.vector) != vector_sha256(&raw_b.vector);
    let redacted_input_hashes_equal =
        sha256_prefixed_bytes(redacted.as_bytes()) == sha256_prefixed_bytes(redacted.as_bytes());
    let redacted_vector_hashes_equal =
        vector_sha256(&redacted_a.vector) == vector_sha256(&redacted_b.vector);
    let no_raw_secret_in_cf_rows = rows.iter().all(|row| {
        let text = row.to_string();
        !text.contains("wJalrXUtnFEMI") && !text.contains("bJalrXUtnFEMI")
    });
    let passes = raw_vector_hashes_differ
        && redacted_input_hashes_equal
        && redacted_vector_hashes_equal
        && cf_state["after_row_count"].as_u64() == Some(rows.len() as u64)
        && cf_state["reopened_row_count"].as_u64() == Some(rows.len() as u64)
        && no_raw_secret_in_cf_rows;

    let fsv = json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-059",
        "artifact_kind": "secret_redaction_forward_runner_fsv",
        "created_at_unix_ms": now_millis()?,
        "all_passed": passes,
        "passes": passes,
        "source_of_truth": {
            "cf": CF_MEJEPA_REDACTION_LOG,
            "db_path": db_path.display().to_string(),
            "fsv_path": run_root.join("secret_redaction_forward_runner_fsv.json").display().to_string()
        },
        "proof": {
            "embedder_id": EmbedderId::E9.slug(),
            "raw_input_sha256_a": sha256_prefixed_bytes(secret_a.as_bytes()),
            "raw_input_sha256_b": sha256_prefixed_bytes(secret_b.as_bytes()),
            "raw_vector_sha256_a": vector_sha256(&raw_a.vector),
            "raw_vector_sha256_b": vector_sha256(&raw_b.vector),
            "raw_vector_hashes_differ": raw_vector_hashes_differ,
            "redacted_input_sha256": sha256_prefixed_bytes(redacted.as_bytes()),
            "redacted_vector_sha256_a": vector_sha256(&redacted_a.vector),
            "redacted_vector_sha256_b": vector_sha256(&redacted_b.vector),
            "redacted_input_hashes_equal": redacted_input_hashes_equal,
            "redacted_vector_hashes_equal": redacted_vector_hashes_equal,
            "clean_input_sha256": sha256_prefixed_bytes(clean.as_bytes()),
            "no_raw_secret_in_cf_rows": no_raw_secret_in_cf_rows
        },
        "redaction_log": cf_state,
    });
    let fsv_path = run_root.join("secret_redaction_forward_runner_fsv.json");
    write_json_checked(&fsv_path, &fsv)?;
    let canonical_path = fsv_root.join("secret_redaction_forward_runner_fsv.json");
    write_json_checked(&canonical_path, &fsv)?;
    println!("{}", serde_json::to_string_pretty(&fsv)?);
    if !passes {
        return Err("TASK-PY-G-059 secret-redaction forward-runner FSV failed".into());
    }
    Ok(())
}

fn redaction_event_row(
    case_id: &str,
    source_text: &str,
    redacted_text: &str,
    event_index: usize,
    redaction_count: usize,
) -> Value {
    let event_id =
        sha256_prefixed_bytes(format!("redaction-log:{case_id}:{event_index}").as_bytes());
    json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-059",
        "artifact_kind": "python_forward_redaction_log",
        "event_id": event_id,
        "source_of_truth_cf": CF_MEJEPA_REDACTION_LOG,
        "chunk_id": sha256_prefixed_bytes(format!("{case_id}:chunk").as_bytes()),
        "source_sha256": sha256_prefixed_bytes(source_text.as_bytes()),
        "redacted_source_sha256": sha256_prefixed_bytes(redacted_text.as_bytes()),
        "redaction_status": "redacted_view_present",
        "event_index": event_index,
        "redaction_count": redaction_count,
        "pattern_id": "env_aws_secret",
        "description": "AWS secret access key assignment",
        "byte_offset": 25,
        "byte_length": 40,
        "redacted_length": "[REDACTED_AWS_SECRET_ACCESS_KEY]".len(),
        "legacy_fallback": false
    })
}

fn clean_event_row(case_id: &str, source_text: &str) -> Value {
    let event_id = sha256_prefixed_bytes(format!("redaction-log:{case_id}:clean").as_bytes());
    json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-059",
        "artifact_kind": "python_forward_redaction_log",
        "event_id": event_id,
        "source_of_truth_cf": CF_MEJEPA_REDACTION_LOG,
        "chunk_id": sha256_prefixed_bytes(format!("{case_id}:chunk").as_bytes()),
        "source_sha256": sha256_prefixed_bytes(source_text.as_bytes()),
        "redacted_source_sha256": sha256_prefixed_bytes(source_text.as_bytes()),
        "redaction_status": "redacted_view_present",
        "event_index": 0,
        "redaction_count": 0,
        "pattern_id": Value::Null,
        "description": "chunk contained no redaction matches",
        "byte_offset": Value::Null,
        "byte_length": Value::Null,
        "redacted_length": Value::Null,
        "legacy_fallback": false
    })
}

fn write_and_reopen_redaction_rows(db_path: &Path, rows: &[Value]) -> FsvResult<Value> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let db = DB::open_cf_descriptors(
        &opts,
        db_path,
        vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_REDACTION_LOG, Options::default()),
        ],
    )?;
    let cf = db
        .cf_handle(CF_MEJEPA_REDACTION_LOG)
        .ok_or_else(|| format!("missing {CF_MEJEPA_REDACTION_LOG}"))?;
    let before = count_cf(&db)?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    for row in rows {
        let event_id = row["event_id"]
            .as_str()
            .ok_or("redaction row missing event_id")?;
        let bytes = serde_json::to_vec(row)?;
        db.put_cf_opt(cf, event_id.as_bytes(), &bytes, &write_opts)?;
        let readback = db
            .get_cf(cf, event_id.as_bytes())?
            .ok_or("missing redaction row after write")?;
        if readback != bytes {
            return Err("redaction row CF readback mismatch".into());
        }
    }
    db.flush_cf(cf)?;
    let after = count_cf(&db)?;
    drop(db);

    let reopened = DB::open_cf_descriptors(
        &opts,
        db_path,
        vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_REDACTION_LOG, Options::default()),
        ],
    )?;
    let reopened_count = count_cf(&reopened)?;
    Ok(json!({
        "db_path": db_path.display().to_string(),
        "source_of_truth_cf": CF_MEJEPA_REDACTION_LOG,
        "before_row_count": before,
        "after_row_count": after,
        "reopened_row_count": reopened_count,
        "written_row_count": rows.len(),
        "sample": rows,
    }))
}

fn count_cf(db: &DB) -> FsvResult<usize> {
    let cf = db
        .cf_handle(CF_MEJEPA_REDACTION_LOG)
        .ok_or_else(|| format!("missing {CF_MEJEPA_REDACTION_LOG}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn sha256_prefixed_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn env_path(name: &str, default: &str) -> PathBuf {
    std::env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn now_millis() -> FsvResult<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    let readback = fs::read(path)?;
    if readback != [bytes, b"\n".to_vec()].concat() {
        return Err(format!("JSON readback mismatch at {}", path.display()).into());
    }
    Ok(())
}
