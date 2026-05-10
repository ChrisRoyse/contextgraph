//! Utility and diversity reranking for persisted optimizer recommendations.

use super::diversity::{has_retrieval_tokens, mmr_select_indices, token_similarity};
use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::recommendation_certificate::recommendation_certificate;
use super::witness_chain::{
    append_witness_entry_for_run, verify_witness_chain_for_run, WitnessOpType,
};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::cmp::Ordering;
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
    recall_recommendations_for_run(&runtime_root, &run_id, &args)
}

pub(super) fn recall_recommendations_for_run(
    runtime_root: &Path,
    run_id: &str,
    args: &Value,
) -> Result<Value> {
    let failure_summary = required_str(args, "failure_summary")?;
    let k = bounded_k(args)?;
    let alpha = optional_f64_strict(args, "alpha", 0.7)?;
    let beta = optional_f64_strict(args, "beta", 0.2)?;
    let gamma = optional_f64_strict(args, "gamma", 0.1)?;
    let lambda = optional_f64_strict(args, "lambda", 0.7)?;
    for (name, value) in [("alpha", alpha), ("beta", beta), ("gamma", gamma)] {
        if !value.is_finite() || value < 0.0 {
            return Err(invalid_number(name, args.get(name)));
        }
    }
    if !(0.0..=1.0).contains(&lambda) || !lambda.is_finite() {
        return Err(CCRealityError::new(
            "CCREALITY_RECALL_LAMBDA_OUT_OF_RANGE",
            "lambda must be a finite number in [0,1]",
            "arguments.lambda",
            "provide lambda between 0 and 1; lower values increase novelty pressure",
            json!({"lambda": args.get("lambda")}),
            None,
        ));
    }
    let run_root = runtime_root.join(run_id);
    let witness_preflight = verify_witness_chain_for_run(runtime_root, run_id)?;
    let mut paths = Vec::new();
    collect_recommendation_paths(&run_root, &mut paths)?;
    paths.sort();
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
    let mut candidates = Vec::new();
    for path in paths {
        let body = read_json(&path)?;
        let text = recommendation_text(&body);
        if !has_retrieval_tokens(&text) {
            return Err(CCRealityError::new(
                "CCREALITY_RECALL_RECOMMENDATION_TEXT_EMPTY",
                "recommendation does not contain retrievable text fields",
                "recommendation.text",
                "record recommendations with reason or diagnosis_summary text before recall",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            ));
        }
        let similarity = token_similarity(&failure_summary, &text);
        let uplift = read_optional_number(&body, &["/uplift_against_baseline", "/utility/uplift"])
            .unwrap_or(0.0);
        let latency_cost = read_optional_number(
            &body,
            &["/latency_cost", "/cost_to_reapply", "/utility/cost"],
        )
        .unwrap_or(0.0);
        let utility = alpha * similarity + beta * uplift - gamma * latency_cost;
        candidates.push(RecallCandidate {
            path,
            body,
            text,
            similarity,
            uplift,
            latency_cost,
            utility,
            normalized_utility: 0.0,
        });
    }
    candidates.sort_by(|a, b| {
        b.utility
            .partial_cmp(&a.utility)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });
    normalize_utilities(&mut candidates);
    let relevances = candidates
        .iter()
        .map(|candidate| candidate.normalized_utility)
        .collect::<Vec<_>>();
    let texts = candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect::<Vec<_>>();
    let selections = mmr_select_indices(&relevances, &texts, k, lambda);
    let mut recommendations = Vec::new();
    for (rank, selection) in selections.iter().enumerate() {
        let candidate = &candidates[selection.index];
        recommendations.push(json!({
            "rank": rank + 1,
            "selection_method": "maximal_marginal_relevance",
            "mmr_score": selection.mmr_score,
            "diversity_penalty": selection.diversity_penalty,
            "normalized_utility": candidate.normalized_utility,
            "utility": candidate.utility,
            "similarity": candidate.similarity,
            "uplift": candidate.uplift,
            "latency_cost": candidate.latency_cost,
            "recommendation": candidate.body,
            "recommendation_path": file_sot(&candidate.path),
            "recommendation_relative_path": relative_path(&run_root, &candidate.path)?,
            "recommendation_sha256": sha256_file(&candidate.path)?,
            "certificate": recommendation_certificate(&candidate.path, &candidate.body)?,
        }));
    }
    let candidate_sources = candidates
        .iter()
        .map(|candidate| {
            Ok(json!({
                "recommendation_path": file_sot(&candidate.path),
                "recommendation_relative_path": relative_path(&run_root, &candidate.path)?,
                "recommendation_sha256": sha256_file(&candidate.path)?,
                "text_sha256": sha256_text(&candidate.text),
                "similarity": candidate.similarity,
                "uplift": candidate.uplift,
                "latency_cost": candidate.latency_cost,
                "utility": candidate.utility,
                "normalized_utility": candidate.normalized_utility,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let audit = canonical_json(json!({
        "schema_version": 1,
        "record_kind": "ccreality_recommendation_recall_audit",
        "run_id": run_id,
        "created_at_unix": unix_secs()?,
        "query": {
            "failure_summary": failure_summary,
            "k": k,
        },
        "formula": "MMR(lambda, normalized(alpha*similarity + beta*uplift - gamma*latency_cost), token_jaccard_redundancy)",
        "weights": {"alpha": alpha, "beta": beta, "gamma": gamma, "lambda": lambda},
        "candidate_count": candidates.len(),
        "selected_count": recommendations.len(),
        "candidate_sources": candidate_sources,
        "selected_recommendations": recommendations,
        "source_of_truth": {
            "run_root": file_sot(&run_root),
            "witness_preflight": witness_preflight,
        },
    }))?;
    let audit_path = next_recall_audit_path(&run_root)?;
    write_json_checked(&audit_path, &audit)?;
    let readback = read_json(&audit_path)?;
    if readback != audit {
        return Err(CCRealityError::new(
            "CCREALITY_RECALL_AUDIT_READBACK_MISMATCH",
            "recommendation recall audit readback did not match the written audit",
            "recommendation_recall.audit.readback",
            "inspect filesystem durability before trusting recall rankings",
            json!({
                "path": audit_path.display().to_string(),
                "expected_sha256": sha256_text(&serde_json::to_string(&audit).unwrap_or_default()),
                "actual_sha256": sha256_text(&serde_json::to_string(&readback).unwrap_or_default()),
            }),
            Some(file_sot(&audit_path)),
        ));
    }
    let audit_sha256 = sha256_file(&audit_path)?;
    let witness_append = append_witness_entry_for_run(
        runtime_root,
        run_id,
        WitnessOpType::RecommendationRecall,
        &audit_sha256,
    )?;
    Ok(json!({
        "status": "ok",
        "formula": audit["formula"],
        "weights": audit["weights"],
        "candidate_count": candidates.len(),
        "recommendations": audit["selected_recommendations"],
        "recall_audit_sha256": audit_sha256,
        "witness_append": witness_append,
        "source_of_truth": {
            "run_root": file_sot(&run_root),
            "recommendation_recall_audit": file_sot(&audit_path),
            "witness_chain": file_sot(&runtime_root.join(run_id).join("claude-code-optimizer").join("witness-chain.bin")),
        },
    }))
}

#[derive(Debug)]
struct RecallCandidate {
    path: PathBuf,
    body: Value,
    text: String,
    similarity: f64,
    uplift: f64,
    latency_cost: f64,
    utility: f64,
    normalized_utility: f64,
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

fn read_optional_number(body: &Value, paths: &[&str]) -> Option<f64> {
    for path in paths {
        if let Some(value) = body.pointer(path).and_then(Value::as_f64) {
            return Some(value);
        }
    }
    None
}

fn normalize_utilities(candidates: &mut [RecallCandidate]) {
    if candidates.is_empty() {
        return;
    }
    let min = candidates
        .iter()
        .map(|candidate| candidate.utility)
        .fold(f64::INFINITY, f64::min);
    let max = candidates
        .iter()
        .map(|candidate| candidate.utility)
        .fold(f64::NEG_INFINITY, f64::max);
    let span = max - min;
    for candidate in candidates {
        candidate.normalized_utility = if span.abs() < f64::EPSILON {
            1.0
        } else {
            (candidate.utility - min) / span
        };
    }
}

fn bounded_k(args: &Value) -> Result<usize> {
    let k = optional_u64_strict(args, "k", 5)?;
    if !(1..=50).contains(&k) {
        return Err(CCRealityError::new(
            "CCREALITY_RECALL_K_OUT_OF_RANGE",
            "k must be between 1 and 50",
            "arguments.k",
            "provide a recall limit in [1,50]",
            json!({"k": args.get("k")}),
            None,
        ));
    }
    Ok(k as usize)
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        CCRealityError::new(
            "CCREALITY_RECALL_PATH_OUTSIDE_RUN_ROOT",
            "recommendation path is outside the run root",
            "recommendations.path",
            "inspect recommendation discovery before trusting recall rankings",
            json!({"run_root": root.display().to_string(), "path": path.display().to_string()}),
            Some(file_sot(path)),
        )
    })?;
    Ok(relative.to_string_lossy().to_string())
}

fn next_recall_audit_path(run_root: &Path) -> Result<PathBuf> {
    let dir = run_root
        .join("reality-optimizer")
        .join("recommendation-recall");
    fs::create_dir_all(&dir)
        .map_err(|e| fs_error("CCREALITY_RECALL_AUDIT_DIR_CREATE_FAILED", &dir, e))?;
    let mut max_seq = 0u64;
    for entry in fs::read_dir(&dir)
        .map_err(|e| fs_error("CCREALITY_RECALL_AUDIT_DIR_READ_FAILED", &dir, e))?
    {
        let entry =
            entry.map_err(|e| fs_error("CCREALITY_RECALL_AUDIT_DIR_ENTRY_FAILED", &dir, e))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(num) = name
            .strip_prefix("recall-")
            .and_then(|s| s.strip_suffix(".json"))
            .and_then(|s| s.parse::<u64>().ok())
        {
            max_seq = max_seq.max(num);
        }
    }
    Ok(dir.join(format!("recall-{:04}.json", max_seq + 1)))
}

fn canonical_json(value: Value) -> Result<Value> {
    let text = serde_json::to_string(&value).map_err(|err| {
        CCRealityError::new(
            "CCREALITY_RECALL_AUDIT_CANONICALIZE_FAILED",
            format!("failed to serialize recommendation recall audit: {err}"),
            "recommendation_recall.audit",
            "inspect recall audit payload before writing it to disk",
            json!({}),
            None,
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        CCRealityError::new(
            "CCREALITY_RECALL_AUDIT_CANONICALIZE_FAILED",
            format!("failed to parse canonical recommendation recall audit: {err}"),
            "recommendation_recall.audit",
            "inspect recall audit payload before writing it to disk",
            json!({}),
            None,
        )
    })
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
}
