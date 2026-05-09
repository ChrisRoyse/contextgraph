//! Reflexion-style attempt recall and deterministic synthesis over real attempts.jsonl.

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

impl Handlers {
    pub(crate) async fn call_attempts_query_reflexion(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match attempts_query_reflexion(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_attempts_critique_summary(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match attempts_critique_summary(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_attempts_success_strategies(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match attempts_success_strategies(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_attempts_synthesize(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match attempts_synthesize(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn attempts_query_reflexion(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let path = active_attempts_path().await?;
    let rows = read_attempt_rows(&path)?;
    let query = optional_str_strict(&args, "query")?.unwrap_or_default();
    let limit = optional_u64_strict(&args, "k", 10)?.min(100) as usize;
    let lambda = optional_f64_strict(&args, "lambda", 0.70)?;
    if !(0.0..=1.0).contains(&lambda) || !lambda.is_finite() {
        return Err(invalid_number("lambda", args.get("lambda")));
    }
    let filtered = filter_attempts(rows, &args)?;
    let mut scored = filtered
        .into_iter()
        .map(|row| {
            let text = attempt_text(&row);
            let similarity = if query.trim().is_empty() {
                1.0
            } else {
                token_similarity(&query, &text)
            };
            (row, similarity)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    let selected = mmr_select(scored, limit, lambda);
    Ok(json!({
        "status": "ok",
        "similarity_method": "deterministic_token_jaccard_mmr",
        "attempts": selected.into_iter().map(|(row, similarity)| {
            json!({
                "similarity": similarity,
                "attempt": row,
            })
        }).collect::<Vec<_>>(),
        "source_of_truth": file_sot(&path),
        "sha256": sha256_file(&path)?,
    }))
}

pub async fn attempts_critique_summary(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let path = active_attempts_path().await?;
    let rows = filter_attempts(read_attempt_rows(&path)?, &args)?;
    let mut tag_counts = BTreeMap::<String, u64>::new();
    let mut outcome_counts = BTreeMap::<String, u64>::new();
    let mut critiques = Vec::new();
    for row in &rows {
        for tag in row
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            *tag_counts.entry(tag.to_string()).or_default() += 1;
        }
        let outcome = row
            .get("outcome_kind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        *outcome_counts.entry(outcome.to_string()).or_default() += 1;
        if let Some(critique) = row.get("critique").and_then(Value::as_str) {
            critiques.push(json!({
                "instance_id": row.get("instance_id"),
                "attempt": row.get("attempt"),
                "outcome_kind": outcome,
                "critique": critique,
            }));
        }
    }
    critiques.truncate(optional_u64_strict(&args, "limit", 20)?.min(100) as usize);
    Ok(json!({
        "status": "ok",
        "attempt_count": rows.len(),
        "tag_counts": tag_counts,
        "outcome_counts": outcome_counts,
        "sample_critiques": critiques,
        "source_of_truth": file_sot(&path),
        "sha256": sha256_file(&path)?,
    }))
}

pub async fn attempts_success_strategies(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let path = active_attempts_path().await?;
    let mut merged = args.as_object().cloned().unwrap_or_default();
    merged.insert("only_successes".to_string(), json!(true));
    let rows = filter_attempts(read_attempt_rows(&path)?, &Value::Object(merged))?;
    let mut tag_counts = BTreeMap::<String, u64>::new();
    let mut patches = Vec::new();
    for row in &rows {
        for tag in row
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            *tag_counts.entry(tag.to_string()).or_default() += 1;
        }
        patches.push(json!({
            "instance_id": row.get("instance_id"),
            "attempt": row.get("attempt"),
            "patch_sha256": row.get("patch_sha256"),
            "critique": row.get("critique"),
            "reward": row.get("reward"),
        }));
    }
    patches.truncate(optional_u64_strict(&args, "limit", 20)?.min(100) as usize);
    Ok(json!({
        "status": "ok",
        "success_count": rows.len(),
        "common_success_tags": tag_counts,
        "successful_attempts": patches,
        "source_of_truth": file_sot(&path),
        "sha256": sha256_file(&path)?,
    }))
}

pub async fn attempts_synthesize(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let path = active_attempts_path().await?;
    let rows = filter_attempts(read_attempt_rows(&path)?, &args)?;
    let total = rows.len() as u64;
    let successes = rows
        .iter()
        .filter(|row| row.get("success").and_then(Value::as_bool) == Some(true))
        .count() as u64;
    let failures = total.saturating_sub(successes);
    let avg_reward = if total == 0 {
        None
    } else {
        Some(
            rows.iter()
                .filter_map(|row| row.get("reward").and_then(Value::as_f64))
                .sum::<f64>()
                / total as f64,
        )
    };
    let summary = format!(
        "attempts={} successes={} failures={} avg_reward={}",
        total,
        successes,
        failures,
        avg_reward
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    Ok(json!({
        "status": "ok",
        "summary": summary,
        "metrics": {
            "attempts": total,
            "successes": successes,
            "failures": failures,
            "avg_reward": avg_reward,
        },
        "source_of_truth": file_sot(&path),
        "sha256": sha256_file(&path)?,
    }))
}

pub fn apply_metadata_filter(rows: Vec<Value>, filter: &Value) -> Result<Vec<Value>> {
    if filter.is_null() {
        return Ok(rows);
    }
    let Some(obj) = filter.as_object() else {
        return Err(CCRealityError::new(
            "CCREALITY_METADATA_FILTER_INVALID",
            "metadata_filter must be an object",
            "arguments.metadata_filter",
            "use an object mapping field paths to scalar values or $operators",
            json!({"metadata_filter": filter}),
            None,
        ));
    };
    let mut out = Vec::new();
    'rows: for row in rows {
        for (path, expected) in obj {
            if !matches_filter(resolve_field(&row, path), expected)? {
                continue 'rows;
            }
        }
        out.push(row);
    }
    Ok(out)
}

async fn active_attempts_path() -> Result<PathBuf> {
    let runtime_root = require_active_runtime_root().await?;
    let run_id = latest_run_id(&runtime_root)?.ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_REFLEXION_NO_ACTIVE_RUN",
            "no active run under the runtime root",
            "active_run",
            "record ME-JEPA outer-loop evidence before querying attempts",
            json!({"runtime_root": runtime_root.display().to_string()}),
            Some(file_sot(&runtime_root)),
        )
    })?;
    Ok(runtime_root
        .join(run_id)
        .join("reality-optimizer")
        .join("attempts.jsonl"))
}

fn read_attempt_rows(path: &Path) -> Result<Vec<Value>> {
    if !path.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_REFLEXION_ATTEMPTS_MISSING",
            "attempts.jsonl is missing from the optimizer state directory",
            "attempts.path",
            "record ME-JEPA outer-loop evidence so attempts.jsonl exists",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ));
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| fs_error("CCREALITY_REFLEXION_ATTEMPTS_READ_FAILED", path, e))?;
    raw.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(idx, line)| {
            serde_json::from_str::<Value>(line).map_err(|err| {
                CCRealityError::new(
                    "CCREALITY_REFLEXION_ATTEMPTS_JSON_INVALID",
                    format!("invalid attempts.jsonl JSON at line {}: {err}", idx + 1),
                    "attempts.parse",
                    "repair or remove the corrupt attempts.jsonl line",
                    json!({"path": path.display().to_string(), "line": idx + 1}),
                    Some(file_sot(path)),
                )
            })
        })
        .collect()
}

fn filter_attempts(rows: Vec<Value>, args: &Value) -> Result<Vec<Value>> {
    let mut rows = rows;
    if let Some(model) = optional_str_strict(args, "model")? {
        rows.retain(|r| r.get("model").and_then(Value::as_str) == Some(model.as_str()));
    }
    if let Some(task) = optional_str_strict(args, "instance_id")? {
        rows.retain(|r| r.get("instance_id").and_then(Value::as_str) == Some(task.as_str()));
    }
    if optional_bool_strict(args, "only_failures", false)? {
        rows.retain(|r| r.get("success").and_then(Value::as_bool) != Some(true));
    }
    if optional_bool_strict(args, "only_successes", false)? {
        rows.retain(|r| r.get("success").and_then(Value::as_bool) == Some(true));
    }
    if let Some(tag) = optional_str_strict(args, "tag")? {
        rows.retain(|r| {
            r.get("tags")
                .and_then(Value::as_array)
                .map(|tags| tags.iter().any(|v| v.as_str() == Some(tag.as_str())))
                .unwrap_or(false)
        });
    }
    if let Some(outcome) = optional_str_strict(args, "outcome_kind")? {
        rows.retain(|r| r.get("outcome_kind").and_then(Value::as_str) == Some(outcome.as_str()));
    }
    if args.get("min_reward").is_some() {
        let min_reward = required_f64(args, "min_reward")?;
        rows.retain(|r| {
            r.get("reward")
                .and_then(Value::as_f64)
                .map(|reward| reward >= min_reward)
                .unwrap_or(false)
        });
    }
    if let Some(filter) = args.get("metadata_filter") {
        rows = apply_metadata_filter(rows, filter)?;
    }
    Ok(rows)
}

fn mmr_select(mut scored: Vec<(Value, f64)>, limit: usize, lambda: f64) -> Vec<(Value, f64)> {
    let mut selected: Vec<(Value, f64)> = Vec::new();
    while !scored.is_empty() && selected.len() < limit {
        let mut best_idx = 0usize;
        let mut best_score = f64::NEG_INFINITY;
        for (idx, (row, relevance)) in scored.iter().enumerate() {
            let diversity_penalty = selected
                .iter()
                .map(|(chosen, _)| token_similarity(&attempt_text(row), &attempt_text(chosen)))
                .fold(0.0, f64::max);
            let mmr = lambda * relevance - (1.0 - lambda) * diversity_penalty;
            if mmr > best_score {
                best_score = mmr;
                best_idx = idx;
            }
        }
        selected.push(scored.remove(best_idx));
    }
    selected
}

fn attempt_text(row: &Value) -> String {
    let mut parts = Vec::new();
    for key in [
        "task_summary",
        "input_excerpt",
        "output_excerpt",
        "critique",
        "outcome_kind",
        "error_code",
        "status",
    ] {
        if let Some(value) = row.get(key).and_then(Value::as_str) {
            parts.push(value.to_string());
        }
    }
    if let Some(tags) = row.get("tags").and_then(Value::as_array) {
        parts.extend(tags.iter().filter_map(Value::as_str).map(ToOwned::to_owned));
    }
    parts.join(" ")
}

fn token_similarity(a: &str, b: &str) -> f64 {
    let a = tokens(a);
    let b = tokens(b);
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(&b).count() as f64;
    let union = a.union(&b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .filter(|item| item.len() >= 2)
        .map(|item| item.to_ascii_lowercase())
        .collect()
}

fn resolve_field<'a>(row: &'a Value, path: &str) -> Option<&'a Value> {
    if path.starts_with('/') {
        return row.pointer(path);
    }
    let mut cur = row;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn matches_filter(actual: Option<&Value>, expected: &Value) -> Result<bool> {
    if let Some(obj) = expected.as_object() {
        if obj.is_empty() {
            return Err(filter_error("$empty", expected));
        }
        for (op, rhs) in obj {
            let matches = match op.as_str() {
                "$eq" => actual == Some(rhs),
                "$ne" => actual != Some(rhs),
                "$gt" => compare_numbers(actual, rhs, |a, b| a > b)?,
                "$gte" => compare_numbers(actual, rhs, |a, b| a >= b)?,
                "$lt" => compare_numbers(actual, rhs, |a, b| a < b)?,
                "$lte" => compare_numbers(actual, rhs, |a, b| a <= b)?,
                "$contains" => contains_value(actual, rhs),
                "$in" => {
                    let Some(items) = rhs.as_array() else {
                        return Err(filter_error("$in", rhs));
                    };
                    items.iter().any(|item| actual == Some(item))
                }
                _ => return Err(filter_error(op, rhs)),
            };
            if !matches {
                return Ok(false);
            }
        }
        Ok(true)
    } else {
        Ok(actual == Some(expected))
    }
}

fn compare_numbers<F: FnOnce(f64, f64) -> bool>(
    actual: Option<&Value>,
    rhs: &Value,
    cmp: F,
) -> Result<bool> {
    let Some(a) = actual.and_then(Value::as_f64) else {
        return Ok(false);
    };
    let Some(b) = rhs.as_f64() else {
        return Err(filter_error("numeric_operator", rhs));
    };
    Ok(cmp(a, b))
}

fn contains_value(actual: Option<&Value>, rhs: &Value) -> bool {
    match (actual, rhs) {
        (Some(Value::String(haystack)), Value::String(needle)) => haystack.contains(needle),
        (Some(Value::Array(items)), _) => items.iter().any(|item| item == rhs),
        _ => false,
    }
}

fn filter_error(op: &str, value: &Value) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_METADATA_FILTER_OPERATOR_INVALID",
        "metadata_filter contains an invalid operator or operand",
        "arguments.metadata_filter",
        "use $eq/$ne/$gt/$gte/$lt/$lte/$contains/$in with compatible operands",
        json!({"operator": op, "value": value}),
        None,
    )
}

fn required_f64(args: &Value, field: &str) -> Result<f64> {
    match args.get(field) {
        Some(Value::Number(n)) => n
            .as_f64()
            .ok_or_else(|| invalid_number(field, args.get(field))),
        Some(Value::String(s)) => s
            .trim()
            .parse::<f64>()
            .map_err(|_| invalid_number(field, args.get(field))),
        _ => Err(invalid_number(field, args.get(field))),
    }
}

fn optional_f64_strict(args: &Value, field: &str, default: f64) -> Result<f64> {
    if args.get(field).is_none() || args.get(field) == Some(&Value::Null) {
        Ok(default)
    } else {
        required_f64(args, field)
    }
}

fn invalid_number(field: &str, value: Option<&Value>) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_ARG_INVALID_F64",
        format!("argument '{field}' must be a finite number"),
        format!("arguments.{field}"),
        format!("omit arguments.{field} or provide a finite number"),
        json!({"value": value}),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metadata_filter_matches_real_json_rows() {
        let rows = vec![
            json!({"instance_id":"a","reward":0.2,"tags":["validation-failed"]}),
            json!({"instance_id":"b","reward":0.9,"tags":["official-resolved"]}),
        ];
        let filtered = apply_metadata_filter(
            rows,
            &json!({"reward":{"$gte":0.8},"tags":{"$contains":"official-resolved"}}),
        )
        .expect("filter");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["instance_id"], json!("b"));
    }

    #[test]
    fn metadata_filter_applies_all_operators() {
        let rows = vec![
            json!({"instance_id":"low","reward":0.2}),
            json!({"instance_id":"match","reward":0.9}),
            json!({"instance_id":"high","reward":1.2}),
        ];
        let filtered = apply_metadata_filter(rows, &json!({"reward":{"$gte":0.8,"$lte":1.0}}))
            .expect("filter");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["instance_id"], json!("match"));
    }

    #[test]
    fn token_similarity_is_deterministic() {
        let a = token_similarity("import error missing module", "missing import module");
        let b = token_similarity("import error missing module", "timeout docker failure");
        assert!(a > b);
    }

    #[test]
    fn invalid_metadata_operator_fails() {
        let err = apply_metadata_filter(vec![json!({"a":1})], &json!({"a":{"$regex":"."}}))
            .expect_err("invalid op");
        assert_eq!(err.error_code, "CCREALITY_METADATA_FILTER_OPERATOR_INVALID");
    }

    #[test]
    fn empty_metadata_operator_object_fails() {
        let err = apply_metadata_filter(vec![json!({"a":1})], &json!({"a":{}}))
            .expect_err("empty operator object");
        assert_eq!(err.error_code, "CCREALITY_METADATA_FILTER_OPERATOR_INVALID");
    }
}
