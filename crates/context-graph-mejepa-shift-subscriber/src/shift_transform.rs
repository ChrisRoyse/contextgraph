// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::PathBuf;

use context_graph_mejepa::{
    sha256_bytes, AstDiff, DiffHunk, Language, PatchBundle, TaskContext, TaskEnvironment, TaskId,
    TestId,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::models::{ShiftEntry, UtmlFactorBundle};
use crate::{Result, SubscriberError};

pub(crate) fn shift_to_inference(
    entry: &ShiftEntry,
    repo_root: PathBuf,
) -> Result<(PatchBundle, TaskContext, [u8; 32], String)> {
    let attempt_id = required_string(&entry.subject, "task_id")?;
    let path = required_string(&entry.subject, "path")?;
    let language: Language =
        serde_json::from_value(Value::String(required_string(&entry.subject, "language")?))?;
    let before = required_string(&entry.before, "text")?;
    let after = required_string(&entry.after, "text")?;
    let pre_sha = sha256_text(&before);
    let post_sha = sha256_text(&after);
    validate_optional_sha(&entry.before, "sha256", pre_sha)?;
    validate_optional_sha(&entry.after, "sha256", post_sha)?;
    let commit_message = required_string(&entry.delta_summary, "commit_message")?;
    let witness_hex = required_string(&entry.verification, "witness_chain_segment_hex")?;
    let witness_chain_segment = hex::decode(&witness_hex).map_err(|err| {
        SubscriberError::invalid(
            "verification.witness_chain_segment_hex",
            format!("hex decode failed: {err}"),
        )
    })?;
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
    let tests = required_string_array(&entry.subject, "tests")?
        .into_iter()
        .map(TestId)
        .collect::<Vec<_>>();
    let context = TaskContext {
        task_id: TaskId(attempt_id.clone()),
        session_id: entry.session_id,
        language,
        problem_statement: required_string(&entry.subject, "problem_statement")?,
        tests,
        environment: TaskEnvironment {
            repo_root,
            python_version: entry
                .subject
                .get("python_version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            os: required_string(&entry.subject, "os")?,
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    context.validate()?;
    Ok((patch, context, post_sha, attempt_id))
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

fn required_string_array(value: &Value, field: &str) -> Result<Vec<String>> {
    required_value(value, field)?
        .as_array()
        .ok_or_else(|| SubscriberError::invalid(field, "field must be an array"))?
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                SubscriberError::invalid(format!("{field}[{idx}]"), "array item must be a string")
            })
        })
        .collect()
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

fn validate_optional_sha(value: &Value, field: &str, expected: [u8; 32]) -> Result<()> {
    let Some(raw) = value.get(field).and_then(Value::as_str) else {
        return Ok(());
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
