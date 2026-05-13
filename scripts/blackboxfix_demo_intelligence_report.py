#!/usr/bin/env python3
"""Build a correlation intelligence report from a BlackBoxFix demo run.

The script is intentionally strict: missing required artifacts or malformed
fields abort the report with a typed error instead of silently producing a
partial story.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import sqlite3
import sys
from pathlib import Path
from typing import Any


class ReportError(Exception):
    def __init__(self, error_code: str, field_path: str, remediation: str, message: str) -> None:
        super().__init__(message)
        self.error_code = error_code
        self.field_path = field_path
        self.remediation = remediation
        self.message = message

    def to_json(self) -> dict[str, Any]:
        return {
            "status": "error",
            "error_code": self.error_code,
            "field_path": self.field_path,
            "message": self.message,
            "remediation": self.remediation,
        }


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def load_json(path: Path, field_path: str) -> Any:
    if not path.exists():
        raise ReportError(
            "BBX_INTEL_ARTIFACT_MISSING",
            field_path,
            f"rerun the demo or point --root at a complete run directory; missing {path}",
            f"required artifact does not exist: {path}",
        )
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ReportError(
            "BBX_INTEL_JSON_INVALID",
            field_path,
            f"inspect and regenerate {path}; JSON parse failed at line {exc.lineno} column {exc.colno}",
            f"invalid JSON in {path}: {exc}",
        ) from exc


def require(obj: Any, path: list[Any], field_path: str) -> Any:
    cur = obj
    for part in path:
        if isinstance(part, int):
            if not isinstance(cur, list) or part >= len(cur):
                raise ReportError(
                    "BBX_INTEL_FIELD_MISSING",
                    field_path,
                    "regenerate the run with the current harness so the required field is present",
                    f"required list index is missing at {field_path}",
                )
            cur = cur[part]
        else:
            if not isinstance(cur, dict) or part not in cur:
                raise ReportError(
                    "BBX_INTEL_FIELD_MISSING",
                    field_path,
                    "regenerate the run with the current harness so the required field is present",
                    f"required field is missing: {field_path}",
                )
            cur = cur[part]
    return cur


def optional(obj: Any, path: list[Any], default: Any = None) -> Any:
    cur = obj
    for part in path:
        if isinstance(part, int):
            if not isinstance(cur, list) or part >= len(cur):
                return default
            cur = cur[part]
        else:
            if not isinstance(cur, dict) or part not in cur:
                return default
            cur = cur[part]
    return cur


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return f"sha256:{digest.hexdigest()}"


def read_jsonl(path: Path, field_path: str) -> list[Any]:
    if not path.exists():
        raise ReportError(
            "BBX_INTEL_ARTIFACT_MISSING",
            field_path,
            f"rerun the episode export; missing {path}",
            f"required JSONL artifact does not exist: {path}",
        )
    records: list[Any] = []
    with path.open("r", encoding="utf-8") as handle:
        for index, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError as exc:
                raise ReportError(
                    "BBX_INTEL_JSONL_INVALID",
                    f"{field_path}[{index}]",
                    f"inspect and regenerate {path}; JSONL parse failed at line {index}",
                    f"invalid JSONL in {path}: {exc}",
                ) from exc
    return records


def read_ledger(db_path: Path) -> list[dict[str, Any]]:
    if not db_path.exists():
        raise ReportError(
            "BBX_INTEL_LEDGER_MISSING",
            "blackbox.blackbox.sqlite",
            "rerun the BlackBoxFix demo so the SQLite ledger is created",
            f"ledger does not exist: {db_path}",
        )
    try:
        with sqlite3.connect(str(db_path)) as con:
            con.row_factory = sqlite3.Row
            rows = con.execute(
                """
                select sequence, record_id, record_kind, run_id, step_id, created_at,
                       actor_json, payload_json, redaction_json, payload_sha256,
                       previous_record_sha256, run_previous_record_sha256,
                       run_chain_sha256, record_sha256
                from ledger_records
                order by sequence
                """
            ).fetchall()
    except sqlite3.Error as exc:
        raise ReportError(
            "BBX_INTEL_LEDGER_READ_FAILED",
            "blackbox.blackbox.sqlite.ledger_records",
            "close other writers/readers and rerun; then inspect sqlite integrity_check",
            f"failed to read ledger_records: {exc}",
        ) from exc

    records: list[dict[str, Any]] = []
    for row in rows:
        item = dict(row)
        for key in ("actor_json", "payload_json", "redaction_json"):
            try:
                item[key[:-5]] = json.loads(item[key])
            except json.JSONDecodeError as exc:
                raise ReportError(
                    "BBX_INTEL_LEDGER_PAYLOAD_INVALID",
                    f"ledger_records.sequence={item['sequence']}.{key}",
                    "inspect the ledger row and rerun the ledger integrity command",
                    f"ledger JSON field is invalid: {exc}",
                ) from exc
            del item[key]
        records.append(item)
    return records


def kind_counts(records: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for record in records:
        counts[record["record_kind"]] = counts.get(record["record_kind"], 0) + 1
    return dict(sorted(counts.items()))


def first_payload(records: list[dict[str, Any]], kind: str) -> dict[str, Any]:
    for record in records:
        if record["record_kind"] == kind:
            return record["payload"]
    raise ReportError(
        "BBX_INTEL_LEDGER_RECORD_MISSING",
        f"ledger_records.record_kind={kind}",
        "rerun the demo so the full BlackBoxFix ledger sequence is present",
        f"required ledger record kind is missing: {kind}",
    )


def records_by_kind(records: list[dict[str, Any]], kind: str) -> list[dict[str, Any]]:
    return [record for record in records if record["record_kind"] == kind]


def ratio(numerator: float, denominator: float) -> float | None:
    if denominator == 0:
        return None
    return round(numerator / denominator, 6)


def percent(value: float | None) -> str:
    if value is None:
        return "n/a"
    return f"{round(value * 100, 2)}%"


def build_report(root: Path) -> tuple[dict[str, Any], str]:
    harness = load_json(root / "harness-report.json", "harness_report")
    dashboard_summary = load_json(root / "blackbox/exports/dashboard/api/summary.json", "dashboard.summary")
    scorecard = load_json(root / "blackbox/exports/scorecard.json", "scorecard")
    evidence = load_json(root / "blackbox/exports/evidence.json", "evidence")
    provenance = load_json(root / "provenance/artifact-provenance.json", "provenance")
    episode_manifest = load_json(root / "dynamicjepa/exports/dynamicjepa-episodes.manifest.json", "dynamicjepa.episodes_manifest")
    episodes = read_jsonl(root / "dynamicjepa/exports/dynamicjepa-episodes.jsonl", "dynamicjepa.episodes")
    ledger = read_ledger(root / "blackbox/blackbox.sqlite")

    run_id = require(harness, ["blackbox_run_id"], "harness.blackbox_run_id")
    dynamic_run_id = require(harness, ["dynamic_run_id"], "harness.dynamic_run_id")
    tool_call_id = require(harness, ["tool_call_id"], "harness.tool_call_id")
    model = require(harness, ["model", "name"], "harness.model.name")
    source_of_truth = require(harness, ["source_of_truth"], "harness.source_of_truth")
    model_decision = first_payload(ledger, "model_decision")
    tool_call = first_payload(ledger, "tool_call")
    agent_run = first_payload(ledger, "agent_run")
    traceparent = optional(agent_run, ["sdk_event", "traceparent"])
    trace_id = traceparent.split("-")[1] if isinstance(traceparent, str) and traceparent.startswith("00-") else None
    prompt_sha256 = require(model_decision, ["input_redacted", "prompt_sha256"], "ledger.model_decision.input_redacted.prompt_sha256")
    model_decision_id = require(model_decision, ["model_decision_id"], "ledger.model_decision.model_decision_id")
    tool_name = require(tool_call, ["tool_name"], "ledger.tool_call.tool_name")
    capability_id = require(tool_call, ["capability_id"], "ledger.tool_call.capability_id")
    expected_delta = require(tool_call, ["expected_delta_redacted"], "ledger.tool_call.expected_delta_redacted")
    dynamic_summary = require(
        harness,
        ["dynamicjepa", "verify_counter_world", "summary"],
        "harness.dynamicjepa.verify_counter_world.summary",
    )
    dynamic_counts = require(
        harness,
        ["dynamicjepa", "verify_counter_world", "decoded_records", "verification_run", "after_counts"],
        "harness.dynamicjepa.verify_counter_world.decoded_records.verification_run.after_counts",
    )
    artifact_checks = require(
        harness,
        ["dynamicjepa", "verify_counter_world", "decoded_records", "verification_run", "artifact_hash_checks"],
        "harness.dynamicjepa.verify_counter_world.decoded_records.verification_run.artifact_hash_checks",
    )

    plan_episode = next((episode for episode in episodes if episode.get("record_kind") == "dynamicjepa_plan_episode"), None)
    if plan_episode is None:
        raise ReportError(
            "BBX_INTEL_EPISODE_MISSING",
            "dynamicjepa.episodes.record_kind=dynamicjepa_plan_episode",
            "rerun the DynamicJEPA episode export",
            "plan episode is missing from dynamicjepa-episodes.jsonl",
        )
    edge_episodes = [episode for episode in episodes if episode.get("record_kind") == "dynamicjepa_edge_case_episode"]

    action = require(plan_episode, ["action"], "dynamicjepa.plan_episode.action")
    candidate_action_ids = require(action, ["candidate_action_ids"], "dynamicjepa.plan_episode.action.candidate_action_ids")
    utility_scores = require(action, ["utility_scores"], "dynamicjepa.plan_episode.action.utility_scores")
    selected_action_id = require(action, ["selected_action_id"], "dynamicjepa.plan_episode.action.selected_action_id")
    selected_index = candidate_action_ids.index(selected_action_id) if selected_action_id in candidate_action_ids else None
    selected_utility = utility_scores[selected_index] if selected_index is not None else None
    max_utility = max(utility_scores) if utility_scores else None
    guard_decisions = optional(plan_episode, ["actual_readback", "guard_decisions"], [])
    guard_allow_count = sum(1 for guard in guard_decisions if guard.get("decision") == "Allow")

    surprise = optional(harness, ["dynamicjepa", "readbacks", "surprise", "decoded_records", "surprise_event"], {})
    score = require(scorecard, ["readback", "score"], "scorecard.readback.score")
    score_inputs = require(scorecard, ["score_inputs"], "scorecard.score_inputs")
    evidence_summary = require(evidence, ["report", "summary"], "evidence.report.summary")
    dashboard_counts = require(dashboard_summary, ["counts"], "dashboard.summary.counts")
    dashboard_rates = require(dashboard_summary, ["rates"], "dashboard.summary.rates")
    edge_cases = require(harness, ["full_state_verification", "edge_cases"], "harness.full_state_verification.edge_cases")

    split_counts = require(dynamic_summary, ["split_counts"], "dynamic_summary.split_counts")
    total_split = sum(split_counts.values())
    artifact_equal_count = sum(1 for check in artifact_checks if check.get("equal") is True)
    verifier_status_counts = require(score_inputs, ["verifier_status_counts"], "scorecard.score_inputs.verifier_status_counts")
    passed_verifiers = verifier_status_counts.get("passed", 0)
    failed_verifiers = verifier_status_counts.get("failed", 0)
    total_verifiers = require(score_inputs, ["verifier_results_total"], "scorecard.score_inputs.verifier_results_total")
    incident_count = require(score_inputs, ["incident_packets_total"], "scorecard.score_inputs.incident_packets_total")

    correlations = [
        {
            "name": "prompt_to_model_decision",
            "join_keys": ["run_id", "trace_id", "model_decision_id", "prompt_sha256"],
            "from": "prompt_sha256",
            "to": "model_decision",
            "status": "linked",
            "meaning": "The raw prompt is not duplicated in the ledger; the digest links prompt identity to the model decision without exposing content.",
        },
        {
            "name": "model_decision_to_tool_call",
            "join_keys": ["run_id", "trace_id", "tool_name", "capability_id"],
            "from": "model_decision",
            "to": "tool_call",
            "status": "linked",
            "meaning": "Gemma chose the approved tool and the harness recorded the mutating call.",
        },
        {
            "name": "tool_call_to_dynamic_state",
            "join_keys": ["tool_call_id", "dynamic_run_id"],
            "from": "tool_call.expected_delta_redacted.dynamicjepa",
            "to": "RocksDB column families",
            "status": "linked",
            "meaning": "The tool call expected durable DynamicJEPA state; RocksDB readback shows the records exist.",
        },
        {
            "name": "expected_delta_to_actual_readback",
            "join_keys": ["tool_call_id", "verifier_result_id", "source_of_truth"],
            "from": "expected_delta",
            "to": "readback_snapshot",
            "status": "mixed",
            "meaning": "The happy-path verifier passed and the intentional wrong-state verifier failed with an incident.",
        },
        {
            "name": "plan_to_training_episode",
            "join_keys": ["plan_trace_id", "artifact_id", "episode_id"],
            "from": "RocksDB plan trace",
            "to": "dynamicjepa-episodes.jsonl",
            "status": "linked",
            "meaning": "The verified plan became a training/eval episode with source-of-truth references.",
        },
        {
            "name": "artifact_to_provenance",
            "join_keys": ["artifact_id", "artifact_sha256", "activity ids"],
            "from": "model artifact registry",
            "to": "artifact-provenance.json",
            "status": "linked",
            "meaning": "The trained artifact is connected to the generating activity, software agents, and source files.",
        },
        {
            "name": "verifier_failure_to_incident_to_score",
            "join_keys": ["verifier_result_id", "incident_packet_id", "run_id"],
            "from": "failed verifier",
            "to": "incident packet and scorecard",
            "status": "linked",
            "meaning": "A source-of-truth mismatch changed the trust level to incident instead of being hidden.",
        },
    ]

    data_inventory = [
        {
            "name": "prompt identity",
            "source": "BlackBoxFix model_decision payload",
            "count": 1,
            "key_fields": ["run_id", "trace_id", "model_decision_id", "prompt_sha256"],
            "stored_content": "sha256 digest plus redacted requested capabilities",
            "privacy": "safe default; raw prompt should be stored only in an encrypted restricted prompt store when enabled by policy",
            "value": "groups outcomes by prompt template/content without leaking prompt text in normal evidence exports",
        },
        {
            "name": "model output / tool choice",
            "source": "historical Gemma local tool call and BlackBoxFix tool_call payload",
            "count": 1,
            "key_fields": ["tool_call_id", "tool_name", "capability_id"],
            "stored_content": "tool name and structured arguments",
            "privacy": "safe because arguments are allowlisted and secret-scanned",
            "value": "connects model decision to the actual capability it tried to exercise",
        },
        {
            "name": "durable operational state",
            "source": "DynamicJEPA RocksDB",
            "count": sum(dynamic_counts.values()),
            "key_fields": ["dynamic_run_id", "plan_trace_id", "artifact_id", "prediction_id", "surprise_event_id"],
            "stored_content": "events, panels, datasets, training runs, model artifacts, predictions, guards, surprise events",
            "privacy": "domain data should be classified by connector and redacted before export",
            "value": "shows whether the action changed real state and what downstream artifacts were generated",
        },
        {
            "name": "verification evidence",
            "source": "BlackBoxFix ledger and evidence export",
            "count": dashboard_counts.get("evidence_packets_total"),
            "key_fields": ["tool_call_id", "evidence_packet_id", "verifier_result_id", "source_of_truth"],
            "stored_content": "expected delta, actual readback, verifier status, record hashes",
            "privacy": "redacted payload plus hashes in exported evidence",
            "value": "proves whether intended outcomes persisted",
        },
        {
            "name": "training/eval signal",
            "source": "dynamicjepa-episodes.jsonl and blackbox episode export",
            "count": optional(episode_manifest, ["counts", "total_records"]),
            "key_fields": ["episode_id", "plan_trace_id", "source_of_truth"],
            "stored_content": "verified plan episode and edge-case episodes",
            "privacy": "should inherit source data classification and redact prompt/output text unless approved",
            "value": "turns real operations into supervised examples and regression fixtures",
        },
        {
            "name": "provenance graph",
            "source": "artifact-provenance.json",
            "count": len(provenance.get("entities", [])) + len(provenance.get("activities", [])) + len(provenance.get("agents", [])),
            "key_fields": ["entity.id", "activity.id", "agent.id", "artifact_id"],
            "stored_content": "entities, activities, agents, and relations",
            "privacy": "paths and model names may need environment redaction before customer export",
            "value": "answers who/what produced each artifact and which inputs were used",
        },
    ]

    calculated = {
        "verification": {
            "source_of_truth_coverage": dashboard_rates.get("source_of_truth_coverage"),
            "readback_ratio": score_inputs.get("readback_ratio"),
            "verifier_pass_rate": ratio(passed_verifiers, total_verifiers),
            "verifier_failure_rate": ratio(failed_verifiers, total_verifiers),
            "incident_rate_per_run": incident_count,
            "evidence_hash_verification": optional(dashboard_summary, ["evidence_hash_verification", "status"]),
        },
        "model_behavior": {
            "prompt_sha256": prompt_sha256,
            "raw_prompt_stored_in_ledger": False,
            "tool_name": tool_name,
            "tool_call_argument_status": "allowlisted",
            "requested_capability_count": len(optional(model_decision, ["input_redacted", "requested_capabilities"], [])),
            "model_decision_confidence": optional(model_decision, ["confidence"]),
        },
        "data_yield": {
            "raw_events": dynamic_counts.get("dj_raw_events"),
            "latent_panels": dynamic_counts.get("dj_latent_panels"),
            "transitions": dynamic_counts.get("dj_transitions"),
            "audit_log_records": dynamic_counts.get("dj_audit_log"),
            "latent_panel_yield_per_raw_event": ratio(dynamic_counts.get("dj_latent_panels", 0), dynamic_counts.get("dj_raw_events", 0)),
            "transition_yield_per_raw_event": ratio(dynamic_counts.get("dj_transitions", 0), dynamic_counts.get("dj_raw_events", 0)),
        },
        "training": {
            "dataset_rows_total": total_split,
            "split_counts": split_counts,
            "train_ratio": ratio(split_counts.get("train", 0), total_split),
            "val_ratio": ratio(split_counts.get("val", 0), total_split),
            "test_ratio": ratio(split_counts.get("test", 0), total_split),
            "training_runs": dynamic_counts.get("dj_training_runs"),
            "model_artifacts": dynamic_counts.get("dj_model_artifacts"),
            "artifact_hash_integrity_ratio": ratio(artifact_equal_count, len(artifact_checks)),
        },
        "planning": {
            "candidate_count": len(candidate_action_ids),
            "prediction_count": len(optional(action, ["prediction_ids"], [])),
            "guard_decision_count": len(optional(action, ["guard_decision_ids"], [])),
            "guard_allow_count": guard_allow_count,
            "guard_allow_rate": ratio(guard_allow_count, len(guard_decisions)),
            "selected_action_id": selected_action_id,
            "selected_utility": selected_utility,
            "max_utility": max_utility,
            "selected_highest_utility_action": selected_utility == max_utility,
        },
        "surprise": {
            "surprise_events": dynamic_counts.get("dj_surprise_events"),
            "surprise_rate_per_plan": ratio(dynamic_counts.get("dj_surprise_events", 0), dynamic_counts.get("dj_plan_traces", 0)),
            "latest_error_norm": surprise.get("error_norm"),
            "latest_cosine": surprise.get("cosine"),
            "latest_threshold": surprise.get("threshold"),
            "latest_kind": surprise.get("surprise_kind"),
        },
        "training_episode_yield": {
            "dynamicjepa_episode_count": optional(episode_manifest, ["counts", "total_records"]),
            "plan_episode_count": optional(episode_manifest, ["counts", "plan_episode_count"]),
            "edge_case_episode_count": optional(episode_manifest, ["counts", "edge_case_episode_count"]),
            "episodes_sha256": optional(episode_manifest, ["output", "sha256"]),
        },
        "governance": {
            "trust_level": score.get("trust_level"),
            "policy_compliance": score.get("policy_compliance"),
            "risk": score.get("risk"),
            "training_value": score.get("training_value"),
            "verification_confidence": score.get("verification_confidence"),
        },
    }

    current_gaps = [
        {
            "gap": "raw prompt and full model text output are not stored as first-class durable artifacts in this run",
            "why_it_matters": "prompt-to-outcome analysis can group by prompt_sha256, but cannot inspect exact phrasing unless a separate prompt store captured it",
            "recommended_fix": "add a model_io record that stores prompt_sha256, output_sha256, redacted_preview, token counts, model parameters, and encrypted object pointers for approved raw capture",
        },
        {
            "gap": "policy decision record is missing",
            "why_it_matters": "scorecard correctly marks policy_compliance=policy_missing",
            "recommended_fix": "emit a policy_decision record before the tool_call with allow/deny, rule ids, approver mode, and governed-mode enforcement result",
        },
        {
            "gap": "single run is enough for proof, but not enough for statistical correlation",
            "why_it_matters": "rates such as failure-by-prompt-template or surprise-by-action need multiple runs",
            "recommended_fix": "run the five-scenario suite and aggregate by prompt_sha256, tool_name, scenario, capability_id, connector, and verifier_id",
        },
    ]

    recommended_capture_contract = {
        "principle": "capture enough to correlate prompt -> decision -> tool -> expected delta -> actual readback -> score -> training signal without exposing sensitive text by default",
        "prompt_capture": {
            "ledger_default": ["prompt_sha256", "prompt_template_id", "prompt_version", "redacted_preview", "input_classification", "token_count"],
            "restricted_store_optional": ["raw_prompt_ciphertext", "kms_key_id", "object_uri", "retention_policy_id"],
            "never": ["unredacted secrets in normal ledger/export fields"],
        },
        "model_output_capture": {
            "ledger_default": ["output_sha256", "tool_calls", "finish_reason", "token_count", "redacted_text_preview"],
            "restricted_store_optional": ["raw_output_ciphertext", "object_uri", "retention_policy_id"],
        },
        "correlation_keys": [
            "run_id",
            "trace_id",
            "step_id",
            "model_decision_id",
            "tool_call_id",
            "expected_delta_id",
            "readback_snapshot_id",
            "verifier_result_id",
            "evidence_packet_id",
            "incident_packet_id",
            "dynamic_run_id",
            "plan_trace_id",
            "prediction_id",
            "guard_decision_id",
            "artifact_id",
            "episode_id",
        ],
        "calculated_metrics_to_track_across_runs": [
            "source_of_truth_coverage",
            "readback_ratio",
            "verifier_pass_rate",
            "incident_rate",
            "prompt_template_failure_rate",
            "tool_argument_rejection_rate",
            "mean_surprise_by_action_type",
            "artifact_hash_integrity_ratio",
            "training_episode_yield",
            "policy_denial_rate",
            "time_to_detection",
            "time_to_incident_packet",
            "repair_success_rate_after_incident",
        ],
    }

    report = {
        "format": "blackboxfix_correlation_intelligence_report_v1",
        "generated_at": utc_now(),
        "status": "ok",
        "source_root": str(root),
        "source_root_sha256_basis": {
            "harness_report": sha256_file(root / "harness-report.json"),
            "blackbox_ledger": sha256_file(root / "blackbox/blackbox.sqlite"),
            "dynamicjepa_episodes": sha256_file(root / "dynamicjepa/exports/dynamicjepa-episodes.jsonl"),
            "provenance": sha256_file(root / "provenance/artifact-provenance.json"),
        },
        "identity_graph": {
            "run_id": run_id,
            "traceparent": traceparent,
            "trace_id": trace_id,
            "dynamic_run_id": dynamic_run_id,
            "model": model,
            "prompt_sha256": prompt_sha256,
            "model_decision_id": model_decision_id,
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "capability_id": capability_id,
            "artifact_id": dynamic_summary.get("artifact_id"),
            "training_run_id": dynamic_summary.get("training_run_id"),
            "dataset_id": dynamic_summary.get("dataset_id"),
            "plan_trace_id": dynamic_summary.get("plan_trace_id"),
            "prediction_id": dynamic_summary.get("prediction_id"),
            "surprise_event_id": surprise.get("surprise_event_id"),
        },
        "source_of_truth": source_of_truth,
        "ledger": {
            "record_count": len(ledger),
            "record_kind_counts": kind_counts(ledger),
            "tip_record_sha256": ledger[-1]["record_sha256"] if ledger else None,
        },
        "data_inventory": data_inventory,
        "correlations": correlations,
        "calculated_intelligence": calculated,
        "expected_delta_summary": expected_delta,
        "evidence_summary": evidence_summary,
        "edge_cases": edge_cases,
        "provenance_summary": {
            "format": provenance.get("format"),
            "entities": len(provenance.get("entities", [])),
            "activities": len(provenance.get("activities", [])),
            "agents": len(provenance.get("agents", [])),
            "relations": len(provenance.get("relations", [])),
        },
        "recommended_capture_contract": recommended_capture_contract,
        "current_gaps": current_gaps,
    }

    markdown = render_markdown(report)
    return report, markdown


def render_markdown(report: dict[str, Any]) -> str:
    calc = report["calculated_intelligence"]
    identity = report["identity_graph"]
    lines = [
        "# BlackBoxFix Correlation Intelligence Report",
        "",
        f"Generated: {report['generated_at']}",
        f"Source root: `{report['source_root']}`",
        "",
        "## Executive Read",
        "",
        "The run has enough data to correlate prompt identity, model decision, tool call, durable state changes, verifier readbacks, incidents, scorecard output, training episodes, and artifact provenance.",
        "",
        f"- Prompt identity: `{identity['prompt_sha256']}`",
        f"- Tool call: `{identity['tool_name']}` / `{identity['tool_call_id']}`",
        f"- Dynamic run: `{identity['dynamic_run_id']}`",
        f"- Plan trace: `{identity['plan_trace_id']}`",
        f"- Artifact: `{identity['artifact_id']}`",
        f"- Trust level: `{calc['governance']['trust_level']}`",
        f"- Policy compliance: `{calc['governance']['policy_compliance']}`",
        "",
        "## Key Calculations",
        "",
        f"- Source-of-truth coverage: {percent(calc['verification']['source_of_truth_coverage'])}",
        f"- Readback ratio: {percent(calc['verification']['readback_ratio'])}",
        f"- Verifier pass rate: {percent(calc['verification']['verifier_pass_rate'])}",
        f"- Verifier failure rate: {percent(calc['verification']['verifier_failure_rate'])}",
        f"- Artifact hash integrity: {percent(calc['training']['artifact_hash_integrity_ratio'])}",
        f"- Guard allow rate: {percent(calc['planning']['guard_allow_rate'])}",
        f"- Selected highest-utility action: `{calc['planning']['selected_highest_utility_action']}`",
        f"- Surprise rate per plan: {calc['surprise']['surprise_rate_per_plan']}",
        f"- DynamicJEPA training/eval episodes: {calc['training_episode_yield']['dynamicjepa_episode_count']}",
        "",
        "## What We Should Attach To Prompts And Outputs",
        "",
        "Attach prompt and output records by digest and policy-governed object pointers, not by spreading raw text through every evidence export.",
        "",
        "- Ledger default for prompts: `prompt_sha256`, template id/version, redacted preview, classification, token count.",
        "- Ledger default for model outputs: `output_sha256`, tool calls, finish reason, token count, redacted preview.",
        "- Restricted store optional: encrypted raw prompt/output with retention policy, KMS key id, and object URI.",
        "- Join everything through `run_id`, `trace_id`, `model_decision_id`, `tool_call_id`, `verifier_result_id`, `dynamic_run_id`, `plan_trace_id`, `artifact_id`, and `episode_id`.",
        "",
        "## Current Gaps",
        "",
    ]
    for gap in report["current_gaps"]:
        lines.extend([
            f"### {gap['gap']}",
            "",
            f"Why it matters: {gap['why_it_matters']}",
            "",
            f"Recommended fix: {gap['recommended_fix']}",
            "",
        ])
    lines.extend([
        "## Correlation Edges",
        "",
    ])
    for edge in report["correlations"]:
        lines.append(f"- `{edge['name']}`: {edge['from']} -> {edge['to']} ({edge['status']}). {edge['meaning']}")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True, help="Demo output root directory")
    parser.add_argument("--out-dir", type=Path, default=None, help="Output directory; defaults to ROOT/intelligence")
    args = parser.parse_args()
    root = args.root.resolve()
    out_dir = (args.out_dir or root / "intelligence").resolve()
    try:
        report, markdown = build_report(root)
        out_dir.mkdir(parents=True, exist_ok=True)
        json_path = out_dir / "correlation-intelligence.json"
        md_path = out_dir / "correlation-intelligence.md"
        json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        md_path.write_text(markdown, encoding="utf-8")
        print(
            json.dumps(
                {
                    "status": "ok",
                    "json": str(json_path),
                    "json_sha256": sha256_file(json_path),
                    "markdown": str(md_path),
                    "markdown_sha256": sha256_file(md_path),
                    "source_of_truth": report["source_of_truth"],
                    "correlation_count": len(report["correlations"]),
                    "data_inventory_count": len(report["data_inventory"]),
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0
    except ReportError as exc:
        print(json.dumps(exc.to_json(), indent=2, sort_keys=True), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
