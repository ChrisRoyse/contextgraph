//! Utility reranking for persisted optimizer recommendations.

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

impl Handlers {
    pub(crate) async fn call_optimizer_recall_recommendations(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_recall_recommendations(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn optimizer_recall_recommendations(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let failure_summary = required_str(&args, "failure_summary")?;
    let k = optional_u64_strict(&args, "k", 5)?.min(50) as usize;
    let alpha = optional_f64_strict(&args, "alpha", 0.7)?;
    let beta = optional_f64_strict(&args, "beta", 0.2)?;
    let gamma = optional_f64_strict(&args, "gamma", 0.1)?;
    for (name, value) in [("alpha", alpha), ("beta", beta), ("gamma", gamma)] {
        if !value.is_finite() || value < 0.0 {
            return Err(invalid_number(name, args.get(name)));
        }
    }
    let runtime_root = require_active_runtime_root().await?;
    let run_id = latest_run_id(&runtime_root)?.ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_RECALL_NO_ACTIVE_RUN",
            "no active run under the runtime root",
            "active_run",
            "run reality-loop attempt before recalling recommendations",
            json!({"runtime_root": runtime_root.display().to_string()}),
            Some(file_sot(&runtime_root)),
        )
    })?;
    let run_root = runtime_root.join(&run_id);
    let mut paths = Vec::new();
    collect_recommendation_paths(&run_root, &mut paths)?;
    if paths.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_RECALL_RECOMMENDATIONS_MISSING",
            "no recommendation-turn-*.json files were found under the active run",
            "recommendations.path",
            "record optimizer recommendations before using utility recall",
            json!({"run_root": run_root.display().to_string()}),
            Some(file_sot(&run_root)),
        ));
    }
    let mut scored = Vec::new();
    for path in paths {
        let body = read_json(&path)?;
        let text = recommendation_text(&body);
        let similarity = token_similarity(&failure_summary, &text);
        let uplift = read_optional_number(&body, &["/uplift_against_baseline", "/utility/uplift"])
            .unwrap_or(0.0);
        let latency_cost = read_optional_number(
            &body,
            &["/latency_cost", "/cost_to_reapply", "/utility/cost"],
        )
        .unwrap_or(0.0);
        let utility = alpha * similarity + beta * uplift - gamma * latency_cost;
        scored.push((path, body, similarity, uplift, latency_cost, utility));
    }
    scored.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap_or(Ordering::Equal));
    let mut recommendations = Vec::new();
    for (path, body, similarity, uplift, latency_cost, utility) in scored.into_iter().take(k) {
        recommendations.push(json!({
            "utility": utility,
            "similarity": similarity,
            "uplift": uplift,
            "latency_cost": latency_cost,
            "recommendation": body,
            "recommendation_path": file_sot(&path),
            "recommendation_sha256": sha256_file(&path)?,
            "certificate": recommendation_certificate(&path, &body)?,
        }));
    }
    Ok(json!({
        "status": "ok",
        "formula": "alpha*similarity + beta*uplift - gamma*latency_cost",
        "weights": {"alpha": alpha, "beta": beta, "gamma": gamma},
        "recommendations": recommendations,
        "source_of_truth": file_sot(&run_root),
    }))
}

fn collect_recommendation_paths(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in
        fs::read_dir(root).map_err(|e| fs_error("CCREALITY_RECALL_WALK_READ_FAILED", root, e))?
    {
        let entry = entry.map_err(|e| fs_error("CCREALITY_RECALL_WALK_ENTRY_FAILED", root, e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_recommendation_paths(&path, out)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("recommendation-turn-") && name.ends_with(".json"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
    Ok(())
}

fn recommendation_text(body: &Value) -> String {
    let mut parts = Vec::new();
    for path in [
        "/reason",
        "/next_inner_prompt_delta",
        "/expected_reward_signal_delta",
        "/diagnosis_summary/root_cause_hypothesis",
        "/diagnosis_summary/failure_class",
        "/diagnosis_summary/intervention_surface",
    ] {
        if let Some(text) = body.pointer(path).and_then(Value::as_str) {
            parts.push(text.to_string());
        }
    }
    parts.join(" ")
}

fn recommendation_certificate(path: &Path, body: &Value) -> Result<Value> {
    let mut leaves = Vec::new();
    leaves.push(source_leaf(
        "recommendation_file",
        &file_sot(path),
        &fs::read(path).map_err(|e| fs_error("CCREALITY_RECALL_CERT_READ_FAILED", path, e))?,
    ));
    if let Some(readbacks) = body
        .get("source_of_truth_readbacks")
        .and_then(Value::as_array)
    {
        for item in readbacks.iter().filter_map(Value::as_str) {
            if let Some(file) = item.strip_prefix("file:") {
                let source_path = PathBuf::from(file);
                let bytes = fs::read(&source_path).map_err(|e| {
                    fs_error("CCREALITY_RECALL_CERT_SOURCE_READ_FAILED", &source_path, e)
                })?;
                leaves.push(source_leaf("file", item, &bytes));
            } else if let Some(sqlite) = item.strip_prefix("sqlite:") {
                let db_path = sqlite.split('#').next().unwrap_or(sqlite);
                let source_path = PathBuf::from(db_path);
                let bytes = fs::read(&source_path).map_err(|e| {
                    fs_error("CCREALITY_RECALL_CERT_SQLITE_READ_FAILED", &source_path, e)
                })?;
                leaves.push(source_leaf("sqlite_file", item, &bytes));
            } else {
                leaves.push(source_leaf("opaque_reference", item, item.as_bytes()));
            }
        }
    }
    let leaf_hashes = leaves
        .iter()
        .map(|leaf| {
            let bytes = serde_json::to_vec(leaf).map_err(|err| {
                CCRealityError::new(
                    "CCREALITY_RECALL_CERT_LEAF_SERIALIZE_FAILED",
                    format!("failed to serialize certificate leaf: {err}"),
                    "certificate.leaf",
                    "inspect source references before issuing certificate",
                    json!({"leaf": leaf}),
                    Some(file_sot(path)),
                )
            })?;
            Ok(merkle_leaf_hash(&bytes))
        })
        .collect::<Result<Vec<_>>>()?;
    let merkle_root = merkle_root(&leaf_hashes);
    Ok(json!({
        "schema_version": 1,
        "record_kind": "ccreality_recommendation_recall_certificate",
        "hash_strategy": "rfc9162_domain_separated_sha256",
        "leaf_count": leaves.len(),
        "source_hashes": leaves,
        "merkle_root": format!("sha256:{}", hex::encode(merkle_root)),
    }))
}

fn source_leaf(kind: &str, source: &str, bytes: &[u8]) -> Value {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    json!({
        "kind": kind,
        "source": source,
        "content_sha256": format!("sha256:{:x}", hasher.finalize()),
    })
}

fn merkle_leaf_hash(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn merkle_node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x01]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return Sha256::digest([]).into();
    }
    if leaves.len() == 1 {
        return leaves[0];
    }
    let split = leaves.len().next_power_of_two() / 2;
    merkle_node_hash(
        &merkle_root(&leaves[..split]),
        &merkle_root(&leaves[split..]),
    )
}

fn read_optional_number(body: &Value, paths: &[&str]) -> Option<f64> {
    for path in paths {
        if let Some(value) = body.pointer(path).and_then(Value::as_f64) {
            return Some(value);
        }
    }
    None
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

fn optional_f64_strict(args: &Value, field: &str, default: f64) -> Result<f64> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(default),
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

    #[test]
    fn utility_similarity_prefers_matching_failure_text() {
        let good = token_similarity("import error missing module", "missing import module fix");
        let bad = token_similarity("import error missing module", "docker timeout");
        assert!(good > bad);
    }

    #[test]
    fn merkle_root_changes_when_leaf_changes() {
        let a = merkle_root(&[merkle_leaf_hash(b"a"), merkle_leaf_hash(b"b")]);
        let b = merkle_root(&[merkle_leaf_hash(b"a"), merkle_leaf_hash(b"c")]);
        assert_ne!(a, b);
    }
}
