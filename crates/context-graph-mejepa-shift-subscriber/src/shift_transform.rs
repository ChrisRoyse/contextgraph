// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::{Path, PathBuf};

use context_graph_mejepa::{
    sha256_bytes, AstDiff, DiffHunk, Language, PatchBundle, TaskContext, TaskEnvironment, TaskId,
    TestId,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::models::{ShiftEntry, UtmlFactorBundle};
use crate::{Result, SubscriberError};

pub fn shift_to_inference(
    entry: &ShiftEntry,
    repo_root: PathBuf,
) -> Result<(PatchBundle, TaskContext, [u8; 32], String)> {
    let transition = load_harness_transition(entry)?;
    let attempt_id = attempt_id(entry, transition.as_ref())?;
    let path = replay_path(entry, transition.as_ref())?;
    let language = language(entry, &path)?;
    let before = before_text(entry, transition.as_ref())?;
    let after = after_text(entry, transition.as_ref(), &repo_root)?;
    let pre_sha = sha256_text(&before);
    let post_sha = sha256_text(&after);
    validate_declared_sha(
        declared_sha(entry, transition.as_ref(), "before", "before_sha256").as_deref(),
        "before.sha256",
        pre_sha,
    )?;
    validate_declared_sha(
        declared_sha(entry, transition.as_ref(), "after", "after_sha256").as_deref(),
        "after.sha256",
        post_sha,
    )?;
    let commit_message = entry
        .delta_summary
        .get("commit_message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            transition
                .as_ref()
                .and_then(|value| value.get("git_diff_stat").and_then(Value::as_str))
                .map(|value| format!("Phase 7 harness transition: {value}"))
        })
        .unwrap_or_else(|| format!("Phase 7 replay for shift {}", entry.shift_id.as_str()));
    let witness_chain_segment = witness_segment(entry)?;
    let patch_sha = sha256_bytes(after.as_bytes());
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from(&path),
                pre_sha,
                post_sha,
                before,
                after,
            }],
        },
        witness_chain_segment,
        commit_message,
        patch_sha,
    )?;
    let tests = optional_string_array(&entry.subject, "tests")?
        .unwrap_or_else(|| vec![format!("phase7_shift_{}", entry.shift_id.as_str())])
        .into_iter()
        .map(TestId)
        .collect::<Vec<_>>();
    let context = TaskContext {
        task_id: TaskId(attempt_id.clone()),
        session_id: entry.session_id,
        language,
        problem_statement: entry
            .subject
            .get("problem_statement")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("Replay durable reality shift {}", entry.shift_id.as_str())),
        tests,
        environment: TaskEnvironment {
            repo_root,
            python_version: entry
                .subject
                .get("python_version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            os: entry
                .subject
                .get("os")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| std::env::consts::OS.to_string()),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    context.validate()?;
    Ok((patch, context, post_sha, attempt_id))
}

pub(crate) fn is_inference_candidate(entry: &ShiftEntry) -> Result<bool> {
    let transition = load_harness_transition(entry)?;
    let Ok(path) = replay_path(entry, transition.as_ref()) else {
        return Ok(false);
    };
    Ok(language_from_extension(&path).is_some())
}

pub(crate) fn utml_bundle(value: &Value) -> Result<Option<UtmlFactorBundle>> {
    let Some(l_step) = value.get("l_step") else {
        return Ok(None);
    };
    let bundle = UtmlFactorBundle {
        l_step: value_to_f32(l_step, "verification.l_step")?,
        delta_p: value_to_f32(required_value(value, "delta_p")?, "verification.delta_p")?,
        delta_k: value_to_f32(required_value(value, "delta_k")?, "verification.delta_k")?,
        delta_omega: value_to_f32(
            required_value(value, "delta_omega")?,
            "verification.delta_omega",
        )?,
        delta_xi: value_to_f32(required_value(value, "delta_xi")?, "verification.delta_xi")?,
    };
    for (field, val) in [
        ("l_step", bundle.l_step),
        ("delta_p", bundle.delta_p),
        ("delta_k", bundle.delta_k),
        ("delta_omega", bundle.delta_omega),
        ("delta_xi", bundle.delta_xi),
    ] {
        if !val.is_finite() || !(0.0..=1.0).contains(&val) {
            return Err(SubscriberError::invalid(
                format!("verification.{field}"),
                "UTML factor must be finite and in [0, 1]",
            ));
        }
    }
    Ok(Some(bundle))
}

pub(crate) fn required_f32_array(value: &Value, field: &str) -> Result<Vec<f32>> {
    required_value(value, field)?
        .as_array()
        .ok_or_else(|| SubscriberError::invalid(field, "field must be an array"))?
        .iter()
        .enumerate()
        .map(|(idx, item)| value_to_f32(item, &format!("{field}[{idx}]")))
        .collect()
}

pub(crate) fn timestamp_ns_to_ms(timestamp_unix_ns: u128) -> Result<i64> {
    i64::try_from(timestamp_unix_ns / 1_000_000).map_err(|err| {
        SubscriberError::invalid(
            "timestamp_unix_ns",
            format!("timestamp milliseconds overflowed i64: {err}"),
        )
    })
}

fn load_harness_transition(entry: &ShiftEntry) -> Result<Option<Value>> {
    let candidates = [
        entry.harness_transition_path.as_deref(),
        entry
            .subject
            .get("harness_transition")
            .and_then(Value::as_str),
        entry
            .subject
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| path.starts_with("file:") || path.ends_with(".json")),
        entry
            .delta_summary
            .get("artifact")
            .and_then(Value::as_str)
            .filter(|path| path.ends_with(".json") || path.starts_with("file:")),
    ];
    for candidate in candidates.into_iter().flatten() {
        let path = path_from_sot(candidate);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|err| SubscriberError::io("read_harness_transition", path.clone(), err))?;
        let value: Value = serde_json::from_str(&text)?;
        return Ok(Some(value));
    }
    Ok(None)
}

fn attempt_id(entry: &ShiftEntry, transition: Option<&Value>) -> Result<String> {
    for value in [
        entry.subject.get("task_id").and_then(Value::as_str),
        entry.subject.get("taskId").and_then(Value::as_str),
        entry.subject.get("attempt_id").and_then(Value::as_str),
        entry.subject.get("attemptId").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    {
        if !value.trim().is_empty() {
            return sanitize_attempt_id(value);
        }
    }
    if let Some(transition) = transition {
        let run_id = transition.get("run_id").and_then(Value::as_str);
        let attempt = transition.get("attempt").and_then(Value::as_u64);
        let file_path = transition.get("file_path").and_then(Value::as_str);
        if let (Some(run_id), Some(attempt), Some(file_path)) = (run_id, attempt, file_path) {
            return sanitize_attempt_id(&format!("{run_id}_{attempt}_{file_path}"));
        }
    }
    sanitize_attempt_id(&format!("shift_{}", entry.shift_id.as_str()))
}

fn sanitize_attempt_id(value: &str) -> Result<String> {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '\0' => '_',
            other if other.is_control() => '_',
            other => other,
        })
        .collect::<String>();
    if sanitized.trim().is_empty() {
        return Err(SubscriberError::invalid(
            "attempt_id",
            "attempt identifier normalized to empty",
        ));
    }
    Ok(sanitized)
}

fn replay_path(entry: &ShiftEntry, transition: Option<&Value>) -> Result<PathBuf> {
    if let Some(path) = entry.subject.get("path").and_then(Value::as_str) {
        let candidate = PathBuf::from(path);
        if is_safe_relative_code_path(path, &candidate) {
            return Ok(candidate);
        }
    }
    if let Some(path) = transition.and_then(|value| value.get("file_path").and_then(Value::as_str))
    {
        let candidate = PathBuf::from(path);
        if is_safe_relative_code_path(path, &candidate) {
            return Ok(candidate);
        }
    }
    if let Some(path) = source_path_from_value(&entry.after)
        .or_else(|| transition.and_then(source_path_from_transition))
    {
        return path.file_name().map(PathBuf::from).ok_or_else(|| {
            SubscriberError::invalid("source.path", "source path has no file name")
        });
    }
    Err(SubscriberError::invalid(
        "subject.path",
        "shift must contain a safe relative code path or a readable after source path",
    ))
}

fn is_safe_relative_code_path(raw: &str, path: &Path) -> bool {
    !raw.starts_with("file:")
        && !path.is_absolute()
        && !raw.contains('\0')
        && !path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
        && language_from_extension(path).is_some()
}

fn language(entry: &ShiftEntry, path: &Path) -> Result<Language> {
    if let Some(raw) = entry.subject.get("language").and_then(Value::as_str) {
        return Ok(serde_json::from_value(Value::String(raw.to_string()))?);
    }
    language_from_extension(path).ok_or_else(|| {
        SubscriberError::invalid(
            "subject.language",
            format!(
                "could not derive supported language from path {}",
                path.display()
            ),
        )
    })
}

fn language_from_extension(path: &Path) -> Option<Language> {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "py" => Some(Language::Python),
        "rs" => Some(Language::Rust),
        "js" | "jsx" => Some(Language::Javascript),
        "ts" | "tsx" => Some(Language::Typescript),
        "go" => Some(Language::Go),
        "java" => Some(Language::Java),
        "c" | "h" => Some(Language::C),
        "cc" | "cpp" | "cxx" | "hpp" => Some(Language::Cpp),
        "cs" => Some(Language::CSharp),
        "rb" => Some(Language::Ruby),
        "php" => Some(Language::Php),
        _ => None,
    }
}

fn before_text(entry: &ShiftEntry, transition: Option<&Value>) -> Result<String> {
    if let Some(text) = entry.before.get("text").and_then(Value::as_str) {
        return Ok(text.to_string());
    }
    if let Some(path) = entry
        .before
        .get("text_source_of_truth")
        .and_then(Value::as_str)
        .map(path_from_sot)
    {
        return read_utf8_source(&path, "before.text_source_of_truth");
    }
    if let Some(path) = transition.and_then(|value| {
        value
            .get("before_text_source_of_truth")
            .and_then(Value::as_str)
            .map(path_from_sot)
    }) {
        return read_utf8_source(&path, "transition.before_text_source_of_truth");
    }
    if let Some(path) = transition
        .and_then(|value| value.get("preedit_state_path").and_then(Value::as_str))
        .map(path_from_sot)
    {
        let state_text = std::fs::read_to_string(&path)
            .map_err(|err| SubscriberError::io("read_preedit_state", path.clone(), err))?;
        let state: Value = serde_json::from_str(&state_text)?;
        if let Some(text_path) = state
            .get("before_text_source_of_truth")
            .and_then(Value::as_str)
            .map(path_from_sot)
        {
            return read_utf8_source(&text_path, "preedit_state.before_text_source_of_truth");
        }
    }
    Err(SubscriberError::invalid(
        "before.text",
        "shift is missing before.text or before.text_source_of_truth; cannot build a verified patch",
    ))
}

fn after_text(entry: &ShiftEntry, transition: Option<&Value>, repo_root: &Path) -> Result<String> {
    if let Some(text) = entry.after.get("text").and_then(Value::as_str) {
        return Ok(text.to_string());
    }
    if let Some(path) = source_path_from_value(&entry.after)
        .or_else(|| transition.and_then(source_path_from_transition))
    {
        return read_utf8_source(&path, "after.source_of_truth");
    }
    if let Some(path) = transition.and_then(|value| value.get("file_path").and_then(Value::as_str))
    {
        return read_utf8_source(&repo_root.join(path), "transition.file_path");
    }
    Err(SubscriberError::invalid(
        "after.text",
        "shift is missing after.text or after.source_of_truth; cannot build a verified patch",
    ))
}

fn source_path_from_transition(value: &Value) -> Option<PathBuf> {
    value
        .get("after_source_path")
        .and_then(Value::as_str)
        .map(path_from_sot)
}

fn source_path_from_value(value: &Value) -> Option<PathBuf> {
    value
        .get("source_of_truth")
        .or_else(|| value.get("sourceOfTruth"))
        .and_then(Value::as_str)
        .map(path_from_sot)
}

fn read_utf8_source(path: &Path, field: &str) -> Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|err| SubscriberError::io("read_source_text", path.to_path_buf(), err))?;
    String::from_utf8(bytes).map_err(|err| {
        SubscriberError::invalid(
            field,
            format!(
                "source text at {} is not valid UTF-8: {err}",
                path.display()
            ),
        )
    })
}

fn declared_sha(
    entry: &ShiftEntry,
    transition: Option<&Value>,
    side: &str,
    transition_field: &str,
) -> Option<String> {
    let side_value = match side {
        "before" => &entry.before,
        "after" => &entry.after,
        _ => return None,
    };
    side_value
        .get("sha256")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            transition
                .and_then(|value| value.get(transition_field).and_then(Value::as_str))
                .map(ToOwned::to_owned)
        })
}

fn witness_segment(entry: &ShiftEntry) -> Result<Vec<u8>> {
    if let Some(hex_text) = entry
        .verification
        .get("witness_chain_segment_hex")
        .and_then(Value::as_str)
    {
        return hex::decode(hex_text).map_err(|err| {
            SubscriberError::invalid(
                "verification.witness_chain_segment_hex",
                format!("hex decode failed: {err}"),
            )
        });
    }
    if let Some(segment) = entry.verification.get("witness_chain_segment") {
        let source = required_string(segment, "source_of_truth")?;
        let offset = required_value(segment, "offset")?.as_u64().ok_or_else(|| {
            SubscriberError::invalid("witness_chain_segment.offset", "offset must be a u64")
        })?;
        let entry_size = required_value(segment, "entry_size")?
            .as_u64()
            .ok_or_else(|| {
                SubscriberError::invalid(
                    "witness_chain_segment.entry_size",
                    "entry_size must be a u64",
                )
            })?;
        let path = path_from_sot(&source);
        let bytes = std::fs::read(&path)
            .map_err(|err| SubscriberError::io("read_witness_segment", path.clone(), err))?;
        let start = usize::try_from(offset)
            .ok()
            .and_then(|idx| idx.checked_mul(usize::try_from(entry_size).ok()?))
            .ok_or_else(|| {
                SubscriberError::invalid(
                    "witness_chain_segment.offset",
                    "offset * entry_size overflowed usize",
                )
            })?;
        let entry_size = usize::try_from(entry_size).map_err(|err| {
            SubscriberError::invalid(
                "witness_chain_segment.entry_size",
                format!("entry_size does not fit usize: {err}"),
            )
        })?;
        let end = start.checked_add(entry_size).ok_or_else(|| {
            SubscriberError::invalid(
                "witness_chain_segment.entry_size",
                "witness segment end offset overflowed usize",
            )
        })?;
        return bytes
            .get(start..end)
            .map(|slice| slice.to_vec())
            .ok_or_else(|| {
                SubscriberError::invalid(
                    "witness_chain_segment",
                    format!(
                        "range {start}..{end} is outside witness file {} ({} bytes)",
                        path.display(),
                        bytes.len()
                    ),
                )
            });
    }
    Err(SubscriberError::invalid(
        "verification.witness_chain_segment_hex",
        "shift is missing witness-chain segment evidence; refusing to synthesize one",
    ))
}

fn path_from_sot(value: &str) -> PathBuf {
    PathBuf::from(value.strip_prefix("file:").unwrap_or(value))
}

fn required_value<'a>(value: &'a Value, field: &str) -> Result<&'a Value> {
    value
        .get(field)
        .ok_or_else(|| SubscriberError::invalid(field, format!("missing required field {field:?}")))
}

fn required_string(value: &Value, field: &str) -> Result<String> {
    required_value(value, field)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| SubscriberError::invalid(field, "field must be a string"))
}

fn optional_string_array(value: &Value, field: &str) -> Result<Option<Vec<String>>> {
    let Some(raw) = value.get(field) else {
        return Ok(None);
    };
    raw.as_array()
        .ok_or_else(|| SubscriberError::invalid(field, "field must be an array"))?
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                SubscriberError::invalid(format!("{field}[{idx}]"), "array item must be a string")
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn value_to_f32(value: &Value, field: &str) -> Result<f32> {
    let number = value
        .as_f64()
        .ok_or_else(|| SubscriberError::invalid(field, "field must be numeric"))?;
    let out = number as f32;
    if !out.is_finite() {
        return Err(SubscriberError::invalid(field, "field must be finite"));
    }
    Ok(out)
}

fn validate_declared_sha(raw: Option<&str>, field: &str, expected: [u8; 32]) -> Result<()> {
    let Some(raw) = raw else {
        return Err(SubscriberError::invalid(
            field,
            "declared sha256 is required for Phase 7 source-state verification",
        ));
    };
    let raw = raw.strip_prefix("sha256:").unwrap_or(raw);
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(raw, &mut bytes).map_err(|err| {
        SubscriberError::invalid(field, format!("sha256 hex decode failed: {err}"))
    })?;
    if bytes != expected {
        return Err(SubscriberError::invalid(
            field,
            "declared sha256 does not match the text payload",
        ));
    }
    Ok(())
}

fn sha256_text(value: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ShiftEntry, ShiftId};
    use serde_json::json;

    #[test]
    fn shift_to_inference_reads_real_producer_text_sources() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let before_path = temp.path().join("before.txt");
        let after_path = repo.join("answer.py");
        let before = "def answer():\n    return 3\n";
        let after = "def answer():\n    return 4\n";
        std::fs::write(&before_path, before).unwrap();
        std::fs::write(&after_path, after).unwrap();
        let entry = ShiftEntry {
            shift_id: ShiftId::parse("01J0123456789ABCDEF0123").unwrap(),
            timestamp_unix_ns: 1_772_000_000_000_000_000,
            tool_name: "optimizer_record_harness_transition".to_string(),
            tool_use_id: Some("toolu-real-producer".to_string()),
            session_id: [1; 16],
            subject: json!({
                "type": "file_edit",
                "task_id": "real_producer_attempt",
                "path": "answer.py",
                "tests": ["test_answer"],
                "problem_statement": "answer returns four",
                "os": std::env::consts::OS
            }),
            before: json!({
                "sha256": format!("sha256:{}", hex::encode(sha256_text(before))),
                "text_source_of_truth": format!("file:{}", before_path.display())
            }),
            after: json!({
                "sha256": format!("sha256:{}", hex::encode(sha256_text(after))),
                "source_of_truth": format!("file:{}", after_path.display())
            }),
            delta_summary: json!({"artifact": "file:/tmp/transition.json"}),
            verification: json!({
                "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment())
            }),
            harness_transition_path: None,
            byte_offset: 0,
            next_byte_offset: 1,
            source_log_path: temp.path().join("session.jsonl"),
        };
        let (patch, context, post_sha, attempt_id) =
            shift_to_inference(&entry, repo.clone()).unwrap();
        assert_eq!(attempt_id, "real_producer_attempt");
        assert_eq!(context.environment.repo_root, repo);
        assert_eq!(patch.ast_diff.hunks[0].before, before);
        assert_eq!(patch.ast_diff.hunks[0].after, after);
        assert_eq!(post_sha, sha256_text(after));
    }

    #[test]
    fn shift_to_inference_fails_closed_without_before_text_source() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let after = "def answer():\n    return 4\n";
        let after_path = repo.join("answer.py");
        std::fs::write(&after_path, after).unwrap();
        let entry = ShiftEntry {
            shift_id: ShiftId::parse("01J0123456789ABCDEF0124").unwrap(),
            timestamp_unix_ns: 1_772_000_000_000_000_000,
            tool_name: "optimizer_record_harness_transition".to_string(),
            tool_use_id: None,
            session_id: [1; 16],
            subject: json!({"path": "answer.py"}),
            before: json!({"sha256": format!("sha256:{}", hex::encode([0u8; 32]))}),
            after: json!({
                "sha256": format!("sha256:{}", hex::encode(sha256_text(after))),
                "source_of_truth": format!("file:{}", after_path.display())
            }),
            delta_summary: json!({}),
            verification: json!({
                "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment())
            }),
            harness_transition_path: None,
            byte_offset: 0,
            next_byte_offset: 1,
            source_log_path: temp.path().join("session.jsonl"),
        };
        let err = shift_to_inference(&entry, repo).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT");
        assert!(err.to_string().contains("before.text"));
    }
}
