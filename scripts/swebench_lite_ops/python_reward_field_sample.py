"""Persist one physical end-to-end Python ME-JEPA reward-field sample."""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError
from .smoke_intelligence_matrices import (
    artifact_state,
    dump_json,
    iter_jsonl,
    read_json,
    sha256_file,
    tree_state,
    utc_now,
    write_jsonl,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-028"
ARTIFACT_KIND = "python_mejepa_reward_field_sample"
DEFAULT_OUTPUT_ROOT = Path("/mnt/d/contextgraph/corpus/python-reward-field-sample-v1")
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-028-reward-field-sample-fsv")


class RewardFieldSampleError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Read the reward-field sample FSV and fix the missing physical source-of-truth row before continuing.",
            details,
        )


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise RewardFieldSampleError(code, message, details)


def row_key(row: dict[str, Any]) -> str:
    return "|".join(
        [
            str(row["task_instance_id"]),
            str(row["workspace_state"]),
            str(row["chunk_id"]),
            str(row["source_sha256"]),
        ]
    )


def first_matching(rows: list[dict[str, Any]], case: str, predicate) -> dict[str, Any]:
    for row in rows:
        if predicate(row):
            return row
    raise RewardFieldSampleError("PY_REWARD_SAMPLE_SELECTION_EMPTY", f"no reward tensor row matched {case}", {"case": case})


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [row for _, row in iter_jsonl(path)]


def load_jsonl_map(path: Path, key_name: str) -> dict[str, dict[str, Any]]:
    mapped: dict[str, dict[str, Any]] = {}
    for _, row in iter_jsonl(path):
        key = str(row.get(key_name))
        require(bool(key) and key != "None", "PY_REWARD_SAMPLE_ROW_KEY_MISSING", f"{path} row is missing {key_name}", {"path": str(path)})
        mapped[key] = row
    return mapped


def load_jsonl_groups(path: Path, key_name: str) -> dict[str, list[dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for _, row in iter_jsonl(path):
        key = str(row.get(key_name))
        require(bool(key) and key != "None", "PY_REWARD_SAMPLE_ROW_KEY_MISSING", f"{path} row is missing {key_name}", {"path": str(path)})
        grouped.setdefault(key, []).append(row)
    return grouped


def select_reward_rows(reward_rows: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return {
        "representative_failure": first_matching(
            reward_rows,
            "representative_failure",
            lambda row: row.get("mutation_category") == "swap_variable"
            and row.get("missing_embedder_count") == 0
            and row.get("heads", {}).get("q2", {}).get("label") is False,
        ),
        "no_production_code_change": first_matching(
            reward_rows,
            "no_production_code_change",
            lambda row: row.get("mutation_category") == "known_good"
            and row.get("missing_embedder_count") == 0
            and row.get("heads", {}).get("q2", {}).get("label") is True,
        ),
        "syntax_error_mutation": first_matching(
            reward_rows,
            "syntax_error_mutation",
            lambda row: row.get("mutation_category") == "compile_error"
            and row.get("missing_embedder_count") == 0
            and row.get("heads", {}).get("q3", {}).get("primary_failure_reason") == "syntax_error",
        ),
    }


def load_ast_rows(chunks_jsonl: Path, selected_keys: set[str]) -> dict[str, dict[str, Any]]:
    found: dict[str, dict[str, Any]] = {}
    for _, row in iter_jsonl(chunks_jsonl):
        key = row_key(row)
        if key in selected_keys:
            found[key] = row
            if len(found) == len(selected_keys):
                break
    missing = sorted(selected_keys - set(found))
    require(not missing, "PY_REWARD_SAMPLE_AST_ROW_MISSING", "selected sample chunk is missing from the AST chunk store", {"missing": missing})
    return found


def load_timeout_event(oracle_events: Path) -> dict[str, Any]:
    final_timeout = None
    timeout_event = None
    for _, row in iter_jsonl(oracle_events):
        if row.get("event_type") == "oracle.timeout" and row.get("timeout") is True:
            timeout_event = row
        if row.get("event_type") == "oracle.final_verdict" and row.get("variant") == "timeout":
            final_timeout = row
    event = final_timeout or timeout_event
    require(event is not None, "PY_REWARD_SAMPLE_TIMEOUT_EVENT_MISSING", "oracle timeout event is missing from oracle_events.jsonl")
    return event


def load_forward_index_rows(forward_root: Path, needed_record_hashes: set[str]) -> dict[str, dict[str, Any]]:
    found: dict[str, dict[str, Any]] = {}
    if not needed_record_hashes:
        return found
    for index_path in sorted(forward_root.glob("shards/*/packed_records/*.index.jsonl")):
        for _, row in iter_jsonl(index_path):
            record_hash = str(row.get("record_sha256"))
            if record_hash in needed_record_hashes:
                indexed = dict(row)
                indexed["index_path"] = str(index_path)
                found[record_hash] = indexed
                if len(found) == len(needed_record_hashes):
                    return found
    missing = sorted(needed_record_hashes - set(found))
    raise RewardFieldSampleError(
        "PY_REWARD_SAMPLE_FORWARD_INDEX_MISSING",
        "selected DDA signal references missing forward-cache index rows",
        {"missing_record_sha256": missing[:20], "missing_count": len(missing)},
    )


def hash_existing_artifacts(paths: dict[str, Any], declared_hashes: dict[str, Any]) -> list[dict[str, Any]]:
    rows = []
    for name, raw_path in sorted(paths.items()):
        if raw_path is None:
            continue
        path = Path(str(raw_path))
        actual = sha256_file(path) if path.exists() else None
        expected = declared_hashes.get(name)
        rows.append(
            {
                "name": name,
                "path": str(path),
                "exists": path.exists(),
                "expected_sha256": expected,
                "actual_sha256": actual,
                "passes": path.exists() and expected == actual,
            }
        )
    return rows


def disk_state(paths: list[Path]) -> dict[str, dict[str, Any]]:
    state = {}
    for path in paths:
        usage = shutil.disk_usage(path)
        state[str(path)] = {"total": usage.total, "used": usage.used, "free": usage.free}
    return state


def compact_heads(tensor: dict[str, Any]) -> dict[str, Any]:
    heads = tensor.get("heads") or {}
    return {
        key: {
            "label": value.get("label"),
            "label_state": value.get("label_state"),
            "primary_failure_reason": value.get("primary_failure_reason"),
            "concern_targets": value.get("concern_targets"),
            "impact_target": value.get("impact_target"),
        }
        for key, value in heads.items()
    }


def build_sample(
    *,
    case: str,
    tensor: dict[str, Any],
    ast_rows: dict[str, dict[str, Any]],
    smoke_rows: dict[str, dict[str, Any]],
    patch_rows: dict[str, dict[str, Any]],
    dda_rows: dict[str, list[dict[str, Any]]],
    missing_dda_rows: dict[str, list[dict[str, Any]]],
    tct_memberships: dict[str, dict[str, Any]],
    tct_support: dict[str, dict[str, Any]],
    forward_index: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    key = str(tensor["row_key"])
    ast_row = ast_rows[key]
    dda_present = dda_rows.get(key, [])
    dda_missing = missing_dda_rows.get(key, [])
    membership = tct_memberships[key]
    cell = tct_support[str(tensor["cell_id"])]
    patch = patch_rows.get(str(tensor["patch_key"]))
    require(patch is not None, "PY_REWARD_SAMPLE_PATCH_TENSOR_MISSING", "selected sample is missing its patch tensor", {"patch_key": tensor.get("patch_key")})
    forward_rows = []
    for signal in dda_present:
        record_hash = str((signal.get("input_hashes") or {}).get("record_sha256"))
        if record_hash in forward_index:
            forward_rows.append(forward_index[record_hash])
    return {
        "case": case,
        "schema_version": SCHEMA_VERSION,
        "selection": {
            "row_key": key,
            "tensor_id": tensor["tensor_id"],
            "patch_key": tensor["patch_key"],
            "mutation_category": tensor["mutation_category"],
            "task_instance_id": tensor["task_instance_id"],
            "repo": tensor["repo"],
            "relative_path": tensor["relative_path"],
        },
        "q1_q5_head_summary": compact_heads(tensor),
        "source_code": {
            "absolute_path": ast_row.get("absolute_path"),
            "relative_path": ast_row.get("relative_path"),
            "line_start": ast_row.get("line_start"),
            "line_end": ast_row.get("line_end"),
            "byte_start": ast_row.get("byte_start"),
            "byte_end": ast_row.get("byte_end"),
            "chunk_kind": ast_row.get("chunk_kind"),
            "node_kind": ast_row.get("node_kind"),
            "source_sha256": ast_row.get("source_sha256"),
            "source_slice_verified": ast_row.get("source_slice_verified"),
            "source_text": ast_row.get("source_text"),
        },
        "source_rows": {
            "smoke_manifest": smoke_rows.get(key),
            "ast_chunk": ast_row,
            "reward_tensor": tensor,
            "patch_tensor": patch,
            "tct_membership": membership,
            "tct_cell_support": cell,
            "dda_present_rows": dda_present,
            "dda_missing_rows": dda_missing,
            "forward_cache_index_rows": forward_rows,
        },
        "physical_counts": {
            "dda_present": len(dda_present),
            "dda_missing": len(dda_missing),
            "forward_cache_index_rows": len(forward_rows),
            "patch_tensor_includes_sample": tensor["tensor_id"] in (patch.get("input_tensor_ids") or []),
            "tct_membership_present": bool(membership),
            "smoke_manifest_present": key in smoke_rows,
        },
    }


def command_python_reward_field_sample(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output:
        for root in (args.output_root, args.fsv_root):
            if root.exists():
                shutil.rmtree(root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {
        "output_root": tree_state(args.output_root),
        "fsv_root": tree_state(args.fsv_root),
        "disk": disk_state([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
    }

    readiness = read_json(args.readiness_decision)
    readiness_decision = readiness.get("decision") or {}
    require(
        readiness_decision.get("smoke_dry_run_may_proceed") is True,
        "PY_REWARD_SAMPLE_READINESS_GATE_CLOSED",
        "readiness gate does not permit smoke reward-field sampling",
        {"readiness_decision": str(args.readiness_decision), "decision": readiness_decision},
    )

    reward_rows = load_jsonl(args.reward_root / "chunk_reward_tensors.jsonl")
    selected = select_reward_rows(reward_rows)
    selected_keys = {str(row["row_key"]) for row in selected.values()}
    ast_rows = load_ast_rows(args.chunks_jsonl, selected_keys)
    smoke_rows = {row_key(row): row for row in load_jsonl(args.smoke_manifest)}
    patch_rows = load_jsonl_map(args.reward_root / "patch_reward_tensors.jsonl", "patch_key")
    dda_rows = load_jsonl_groups(args.dda_root / "per_embedder_signals.jsonl", "row_key")
    missing_dda_rows = load_jsonl_groups(args.dda_root / "missing_signal_states.jsonl", "row_key")
    tct_memberships = load_jsonl_map(args.tct_root / "tct_chunk_memberships.jsonl", "row_key")
    tct_support = load_jsonl_map(args.tct_root / "tct_cell_support.jsonl", "cell_id")
    needed_record_hashes = {
        str((signal.get("input_hashes") or {}).get("record_sha256"))
        for key in selected_keys
        for signal in dda_rows.get(key, [])
        if (signal.get("input_hashes") or {}).get("record_sha256")
    }
    forward_index = load_forward_index_rows(args.forward_root, needed_record_hashes)
    timeout_event = load_timeout_event(args.oracle_events)
    timeout_artifacts = hash_existing_artifacts(timeout_event.get("artifact_paths") or {}, timeout_event.get("artifact_hashes") or {})

    samples = [
        build_sample(
            case=case,
            tensor=tensor,
            ast_rows=ast_rows,
            smoke_rows=smoke_rows,
            patch_rows=patch_rows,
            dda_rows=dda_rows,
            missing_dda_rows=missing_dda_rows,
            tct_memberships=tct_memberships,
            tct_support=tct_support,
            forward_index=forward_index,
        )
        for case, tensor in selected.items()
    ]
    timeout_sample = {
        "case": "oracle_timeout",
        "schema_version": SCHEMA_VERSION,
        "selection": {
            "event_id": timeout_event.get("event_id"),
            "instance_id": timeout_event.get("instance_id"),
            "mutation_category": timeout_event.get("mutation_category"),
            "variant": timeout_event.get("variant"),
            "event_type": timeout_event.get("event_type"),
            "verdict_kind": timeout_event.get("verdict_kind"),
        },
        "oracle_event": timeout_event,
        "artifact_readback": timeout_artifacts,
        "physical_counts": {
            "artifact_count": len(timeout_artifacts),
            "artifact_hash_pass_count": sum(1 for row in timeout_artifacts if row["passes"]),
        },
    }

    edge_rows = [
        {
            "case": "no_production_code_change",
            "before": {
                "mutation_category": selected["no_production_code_change"]["mutation_category"],
                "workspace_state": str(selected["no_production_code_change"]["row_key"]).split("|")[1],
                "relative_path": selected["no_production_code_change"].get("heads", {}).get("q1", {}).get("evidence", {}).get("relative_path"),
            },
            "action": "read known-good/base reward tensor chain",
            "after": {
                "q2_label": selected["no_production_code_change"]["heads"]["q2"]["label"],
                "q3_state": selected["no_production_code_change"]["heads"]["q3"]["label_state"],
                "dda_present": next(s for s in samples if s["case"] == "no_production_code_change")["physical_counts"]["dda_present"],
            },
            "expected": "known_good/base has Q2 true, Q3 not_applicable_pass, and physical DDA rows",
            "pass": selected["no_production_code_change"]["heads"]["q2"]["label"] is True
            and selected["no_production_code_change"]["heads"]["q3"]["label_state"] == "not_applicable_pass"
            and next(s for s in samples if s["case"] == "no_production_code_change")["physical_counts"]["dda_present"] == args.expected_embedder_count,
        },
        {
            "case": "syntax_error_mutation",
            "before": {"mutation_category": selected["syntax_error_mutation"]["mutation_category"]},
            "action": "read compile_error reward tensor chain",
            "after": {
                "q2_label": selected["syntax_error_mutation"]["heads"]["q2"]["label"],
                "q3_primary_failure_reason": selected["syntax_error_mutation"]["heads"]["q3"]["primary_failure_reason"],
                "dda_present": next(s for s in samples if s["case"] == "syntax_error_mutation")["physical_counts"]["dda_present"],
            },
            "expected": "compile_error has Q2 false and Q3 syntax_error with physical DDA rows",
            "pass": selected["syntax_error_mutation"]["heads"]["q2"]["label"] is False
            and selected["syntax_error_mutation"]["heads"]["q3"]["primary_failure_reason"] == "syntax_error"
            and next(s for s in samples if s["case"] == "syntax_error_mutation")["physical_counts"]["dda_present"] == args.expected_embedder_count,
        },
        {
            "case": "oracle_timeout",
            "before": {"oracle_events": str(args.oracle_events), "variant": "timeout"},
            "action": "read real oracle timeout event and artifact hashes",
            "after": {
                "event_type": timeout_event.get("event_type"),
                "verdict_kind": timeout_event.get("verdict_kind"),
                "timeout": timeout_event.get("timeout"),
                "is_code_failure_label": timeout_event.get("is_code_failure_label"),
                "artifact_hash_pass_count": timeout_sample["physical_counts"]["artifact_hash_pass_count"],
            },
            "expected": "timeout is not a code-failure label and all persisted timeout artifacts hash-match",
            "pass": timeout_event.get("verdict_kind") == "timeout"
            and timeout_event.get("timeout") is True
            and timeout_event.get("is_code_failure_label") is False
            and timeout_sample["physical_counts"]["artifact_hash_pass_count"] == timeout_sample["physical_counts"]["artifact_count"],
        },
    ]

    sample_rows = samples + [timeout_sample]
    bundle = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "output_root": str(args.output_root),
            "fsv_root": str(args.fsv_root),
            "sample_bundle": str(args.output_root / "reward_field_sample_bundle.json"),
            "sample_rows": str(args.output_root / "sample_rows.jsonl"),
            "edge_cases": str(args.output_root / "edge_case_sample_states.jsonl"),
        },
        "dependency_hashes": {
            "readiness_decision": sha256_file(args.readiness_decision),
            "reward_tensors": sha256_file(args.reward_root / "chunk_reward_tensors.jsonl"),
            "patch_tensors": sha256_file(args.reward_root / "patch_reward_tensors.jsonl"),
            "dda_signals": sha256_file(args.dda_root / "per_embedder_signals.jsonl"),
            "tct_memberships": sha256_file(args.tct_root / "tct_chunk_memberships.jsonl"),
            "oracle_events": sha256_file(args.oracle_events),
        },
        "summary": {
            "sample_count": len(sample_rows),
            "selected_cases": [row["case"] for row in sample_rows],
            "representative_failure_tensor_id": selected["representative_failure"]["tensor_id"],
            "production_training_may_proceed": readiness_decision.get("production_training_may_proceed"),
            "smoke_dry_run_may_proceed": readiness_decision.get("smoke_dry_run_may_proceed"),
        },
        "samples": sample_rows,
    }
    write_jsonl(args.output_root / "sample_rows.jsonl", sample_rows)
    write_jsonl(args.output_root / "edge_case_sample_states.jsonl", edge_rows)
    dump_json(args.output_root / "reward_field_sample_bundle.json", bundle)

    after = {
        "output_root": tree_state(args.output_root),
        "fsv_root": tree_state(args.fsv_root),
        "disk": disk_state([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
    }
    readback_bundle = read_json(args.output_root / "reward_field_sample_bundle.json")
    readback_sample_count = sum(1 for _ in iter_jsonl(args.output_root / "sample_rows.jsonl"))
    readback_edge_rows = [row for _, row in iter_jsonl(args.output_root / "edge_case_sample_states.jsonl")]
    representative = next(row for row in sample_rows if row["case"] == "representative_failure")
    happy_path = {
        "case": "representative_failure_has_complete_q1_q5_chain",
        "before": {"row_key": representative["selection"]["row_key"]},
        "action": "read sample bundle, sample rows, and all joined physical stores",
        "after": {
            "q2_label": representative["q1_q5_head_summary"]["q2"]["label"],
            "q3_reason": representative["q1_q5_head_summary"]["q3"]["primary_failure_reason"],
            "dda_present": representative["physical_counts"]["dda_present"],
            "forward_cache_index_rows": representative["physical_counts"]["forward_cache_index_rows"],
            "patch_tensor_includes_sample": representative["physical_counts"]["patch_tensor_includes_sample"],
            "source_slice_verified": representative["source_code"]["source_slice_verified"],
        },
        "expected": "representative failing sample has Q1-Q5 tensor, AST code slice, DDA rows, TCT membership, forward cache index rows, and patch tensor linkage",
        "pass": representative["q1_q5_head_summary"]["q2"]["label"] is False
        and representative["q1_q5_head_summary"]["q3"]["primary_failure_reason"] not in (None, "none")
        and representative["physical_counts"]["dda_present"] == args.expected_embedder_count
        and representative["physical_counts"]["forward_cache_index_rows"] == args.expected_embedder_count
        and representative["physical_counts"]["patch_tensor_includes_sample"] is True
        and representative["source_code"]["source_slice_verified"] is True,
    }
    passes = (
        happy_path["pass"]
        and readback_bundle["summary"]["sample_count"] == 4
        and readback_sample_count == 4
        and len(readback_edge_rows) == 3
        and all(row["pass"] for row in readback_edge_rows)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "before_state": before,
        "after_state": after,
        "source_of_truth": {**bundle["source_of_truth"], "fsv": str(args.fsv_root / "python_reward_field_sample_fsv.json")},
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py python-reward-field-sample"},
        "happy_path": happy_path,
        "edge_cases": readback_edge_rows,
        "readback": {
            "bundle_sha256": sha256_file(args.output_root / "reward_field_sample_bundle.json"),
            "sample_rows_sha256": sha256_file(args.output_root / "sample_rows.jsonl"),
            "edge_cases_sha256": sha256_file(args.output_root / "edge_case_sample_states.jsonl"),
            "sample_count": readback_sample_count,
            "edge_case_count": len(readback_edge_rows),
            "artifact_state": artifact_state(args.output_root),
            "selected_cases": readback_bundle["summary"]["selected_cases"],
        },
    }
    dump_json(args.fsv_root / "python_reward_field_sample_fsv.json", fsv)
    final = read_json(args.fsv_root / "python_reward_field_sample_fsv.json")
    require(final.get("passes") is True, "PY_REWARD_SAMPLE_FSV_FAILED", "reward-field sample FSV failed", {"path": str(args.fsv_root / "python_reward_field_sample_fsv.json")})
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--smoke-manifest", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest/smoke_manifest.jsonl"))
    parser.add_argument("--chunks-jsonl", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    parser.add_argument("--forward-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/forward-cache"))
    parser.add_argument("--dda-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-dda-signals-v1/per-embedder"))
    parser.add_argument("--tct-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-tct-cells-v1"))
    parser.add_argument("--reward-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-reward-tensors-v1"))
    parser.add_argument("--oracle-events", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-oracle-events-v1/oracle_events.jsonl"))
    parser.add_argument("--readiness-decision", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-data-readiness-gate-v1/data_readiness_decision.json"))
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--expected-embedder-count", type=int, default=12)
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_reward_field_sample(args)
    except RewardFieldSampleError as exc:
        print(json.dumps({"ok": False, **exc.to_json()}, indent=2, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
