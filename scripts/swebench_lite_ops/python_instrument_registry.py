from __future__ import annotations

import copy
import json
import tomllib
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_file, utc_now, write_json_checked
from .oracle_events import (
    assert_hot_root_allowed,
    canonical_json,
    capacity_snapshot,
    file_state,
    load_jsonschema_validator,
    path_bytes,
    sha256_file,
    sha256_prefixed_text,
    write_jsonl_checked,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-014"
PRODUCER = "contextgraph.swebench_lite_ops.python_instrument_registry"
DATACONTENTTYPE = "application/json"

ACTIVE_CONTENT_EMBEDDERS = ("e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14")
DETERMINISTIC_EMBEDDERS = {"e2", "e3", "e4", "e9"}
PROOF_DOMAIN_TOKENS = ("gridworld", "counter_world", "counter-world")


class InstrumentRegistryError(SwebenchOpsError):
    pass


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    with path.open(encoding="utf-8") as handle:
        return sum(1 for line in handle if line.strip())


def read_json(path: Path, code: str) -> Any:
    ensure_file(path, code, "required JSON artifact")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise InstrumentRegistryError(
            code,
            f"invalid JSON at {path}",
            "Regenerate the upstream artifact before registering instruments.",
            {"path": str(path), "error": str(exc)},
        ) from exc


def read_models_config(path: Path) -> dict[str, Any]:
    ensure_file(path, "PY_INSTRUMENT_REGISTRY_MODELS_CONFIG_MISSING", "models_config.toml")
    try:
        with path.open("rb") as handle:
            data = tomllib.load(handle)
    except tomllib.TOMLDecodeError as exc:
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_MODELS_CONFIG_INVALID",
            f"models_config.toml is invalid TOML: {path}",
            "Fix the model config before registering the frozen embedder panel.",
            {"path": str(path), "error": str(exc)},
        ) from exc
    embedders = data.get("embedders")
    if not isinstance(embedders, dict):
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_MODELS_CONFIG_INVALID",
            "models_config.toml is missing [embedders] registrations",
            "Regenerate the Phase 1b models config with all active embedder pins.",
            {"path": str(path), "keys": sorted(data.keys())},
        )
    return data


def instrument_schema(schema_path: Path) -> dict[str, Any]:
    sha = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
    nullable_int = {"type": ["integer", "null"]}
    sot_artifact = {
        "type": "object",
        "properties": {
            "name": {"type": "string", "minLength": 1},
            "path": {"type": "string", "minLength": 1},
            "kind": {"type": "string", "minLength": 1},
            "exists": {"const": True},
            "bytes": {"type": "integer", "minimum": 0},
            "sha256": sha,
            "hash_origin": {"enum": ["file_sha256", "manifest_declared_sha256", "canonical_contract_sha256"]},
            "row_count": nullable_int,
        },
        "required": ["name", "path", "kind", "exists", "bytes", "sha256", "hash_origin", "row_count"],
        "additionalProperties": False,
    }
    code_artifact = {
        "type": "object",
        "properties": {
            "path": {"type": "string", "minLength": 1},
            "exists": {"const": True},
            "bytes": {"type": "integer", "minimum": 1},
            "sha256": sha,
        },
        "required": ["path", "exists", "bytes", "sha256"],
        "additionalProperties": False,
    }
    props = {
        "schema_version": {"const": SCHEMA_VERSION},
        "instrument_record_id": sha,
        "instrument_id": {"type": "string", "minLength": 1},
        "source": {"type": "string", "minLength": 1},
        "subject": {"type": "string", "minLength": 1},
        "dataschema": {"type": "string", "minLength": 1},
        "datacontenttype": {"const": DATACONTENTTYPE},
        "time": {"type": "string", "minLength": 1},
        "producer": {"const": PRODUCER},
        "task_id": {"const": TASK_ID},
        "panel_role": {"enum": ["target_side_frozen_instrument", "target_side_embedder", "declared_future_runtime_slot"]},
        "activation_status": {"enum": ["active", "declared_pending_forward_cache", "declared_not_python_scope"]},
        "domain_scope": {"enum": ["mejepa_code_python", "mejepa_runtime_cross_language", "non_python_auxiliary", "sibling_proof_domain"]},
        "instrument_kind": {
            "enum": [
                "ast_parser",
                "ast_chunker",
                "diff_chunk_mapper",
                "oracle_event_materializer",
                "static_reality_graph",
                "test_intent_extractor",
                "content_embedder_pretrained",
                "content_embedder_deterministic",
                "witness_chain_slot",
            ]
        },
        "version": {"type": "string", "minLength": 1},
        "model_or_algorithm": {"type": "string", "minLength": 1},
        "output_feature_family": {"type": "string", "minLength": 1},
        "output_dimension": nullable_int,
        "feature_count": nullable_int,
        "trainable_parameter_count": {"type": "integer", "minimum": 0},
        "target_side_frozen": {"type": "boolean"},
        "stable_hash_status": {"enum": ["stable", "pending_downstream"]},
        "stable_hashes": {
            "type": "object",
            "additionalProperties": sha,
            "minProperties": 1,
        },
        "preprocessing_contract": {"type": "object"},
        "preprocessing_hash": sha,
        "source_of_truth": {"type": "array", "items": sot_artifact, "minItems": 1},
        "code_paths": {"type": "array", "items": code_artifact, "minItems": 1},
        "used_by_q_heads": {
            "type": "array",
            "items": {"enum": ["Q1_EXIST", "Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT"]},
            "minItems": 1,
        },
        "failure_modes": {"type": "array", "items": {"type": "string", "minLength": 1}, "minItems": 1},
        "edge_guards": {"type": "array", "items": {"type": "string", "minLength": 1}, "minItems": 1},
        "source_readback_verified": {"const": True},
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python frozen instrument registry v1",
        "type": "object",
        "properties": props,
        "required": sorted(props),
        "additionalProperties": False,
    }


def code_artifact(path: Path) -> dict[str, Any]:
    ensure_file(path, "PY_INSTRUMENT_REGISTRY_CODE_PATH_MISSING", "instrument code path")
    return {"path": str(path), "exists": True, "bytes": path.stat().st_size, "sha256": sha256_file(path)}


def source_artifact(name: str, path: Path, kind: str, sha: str | None = None, hash_origin: str = "file_sha256", row_count: int | None = None) -> dict[str, Any]:
    ensure_file(path, "PY_INSTRUMENT_REGISTRY_SOURCE_PATH_MISSING", "instrument source of truth")
    stable_sha = sha if sha is not None else sha256_file(path)
    if not isinstance(stable_sha, str) or not stable_sha.startswith("sha256:"):
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_SOURCE_HASH_INVALID",
            f"source artifact has no stable sha256: {name}",
            "Regenerate the upstream manifest so every registered source has a stable hash.",
            {"name": name, "path": str(path), "sha256": stable_sha},
        )
    return {
        "name": name,
        "path": str(path),
        "kind": kind,
        "exists": True,
        "bytes": path.stat().st_size,
        "sha256": stable_sha,
        "hash_origin": hash_origin,
        "row_count": row_count,
    }


def hash_group(items: list[dict[str, Any]]) -> str:
    return sha256_prefixed_text(canonical_json(items))


def preprocessing_hash(contract: dict[str, Any]) -> str:
    return sha256_prefixed_text(canonical_json(contract))


def make_record(
    *,
    schema_path: Path,
    now: str,
    instrument_id: str,
    panel_role: str,
    activation_status: str,
    domain_scope: str,
    instrument_kind: str,
    version: str,
    model_or_algorithm: str,
    output_feature_family: str,
    output_dimension: int | None,
    feature_count: int | None,
    stable_hashes: dict[str, str],
    preprocessing_contract: dict[str, Any],
    source_of_truth: list[dict[str, Any]],
    code_paths: list[dict[str, Any]],
    used_by_q_heads: list[str],
    failure_modes: list[str],
    edge_guards: list[str],
) -> dict[str, Any]:
    contract_hash = preprocessing_hash(preprocessing_contract)
    record_seed = {
        "instrument_id": instrument_id,
        "version": version,
        "stable_hashes": stable_hashes,
        "preprocessing_hash": contract_hash,
        "source_hashes": [item["sha256"] for item in source_of_truth],
        "code_hashes": [item["sha256"] for item in code_paths],
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "instrument_record_id": sha256_prefixed_text(canonical_json(record_seed)),
        "instrument_id": instrument_id,
        "source": f"contextgraph://python-frozen-instrument-registry/{instrument_id}",
        "subject": f"{TASK_ID}#{instrument_id}",
        "dataschema": str(schema_path),
        "datacontenttype": DATACONTENTTYPE,
        "time": now,
        "producer": PRODUCER,
        "task_id": TASK_ID,
        "panel_role": panel_role,
        "activation_status": activation_status,
        "domain_scope": domain_scope,
        "instrument_kind": instrument_kind,
        "version": version,
        "model_or_algorithm": model_or_algorithm,
        "output_feature_family": output_feature_family,
        "output_dimension": output_dimension,
        "feature_count": feature_count,
        "trainable_parameter_count": 0,
        "target_side_frozen": True,
        "stable_hash_status": "stable" if activation_status == "active" else "pending_downstream",
        "stable_hashes": stable_hashes,
        "preprocessing_contract": preprocessing_contract,
        "preprocessing_hash": contract_hash,
        "source_of_truth": source_of_truth,
        "code_paths": code_paths,
        "used_by_q_heads": used_by_q_heads,
        "failure_modes": failure_modes,
        "edge_guards": edge_guards,
        "source_readback_verified": True,
    }


def manifest_output_sha(manifest: dict[str, Any], source_name: str) -> str | None:
    key_by_source = {
        "records_jsonl": "records_sha256",
        "chunks_jsonl": "chunks_sha256",
        "event_jsonl": "event_jsonl_sha256",
        "edge_records_jsonl": "edge_records_sha256",
        "edge_chunks_jsonl": "edge_chunks_sha256",
    }
    value = manifest.get(key_by_source.get(source_name, ""))
    return value if isinstance(value, str) and value.startswith("sha256:") else None


def source_artifacts_from_manifest(manifest_path: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    manifest = read_json(manifest_path, "PY_INSTRUMENT_REGISTRY_MANIFEST_INVALID")
    sot = manifest.get("source_of_truth")
    if not isinstance(sot, dict):
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_MANIFEST_INVALID",
            f"manifest has no source_of_truth object: {manifest_path}",
            "Regenerate the upstream artifact before registering instruments.",
            {"manifest": str(manifest_path), "keys": sorted(manifest.keys())},
        )
    artifacts = [source_artifact("manifest", manifest_path, "manifest")]
    for name, raw_path in sorted(sot.items()):
        path = Path(raw_path)
        if path == manifest_path:
            continue
        declared = manifest_output_sha(manifest, name)
        artifacts.append(
            source_artifact(
                name,
                path,
                "source_of_truth",
                declared,
                "manifest_declared_sha256" if declared else "file_sha256",
                row_count_for_source(manifest, name),
            )
        )
    return artifacts, manifest


def row_count_for_source(manifest: dict[str, Any], source_name: str) -> int | None:
    if source_name not in {"records_jsonl", "chunks_jsonl", "event_jsonl"}:
        return None
    summary = manifest.get("summary")
    if isinstance(summary, dict) and isinstance(summary.get("total"), int):
        return summary["total"]
    counts = manifest.get("counts")
    if isinstance(counts, dict):
        return sum(value for value in counts.values() if isinstance(value, int))
    return None


def data_instrument(
    *,
    schema_path: Path,
    now: str,
    instrument_id: str,
    instrument_kind: str,
    manifest_path: Path,
    code_files: list[Path],
    model_or_algorithm: str,
    output_feature_family: str,
    q_heads: list[str],
    failure_modes: list[str],
    edge_guards: list[str],
) -> dict[str, Any]:
    source_of_truth, manifest = source_artifacts_from_manifest(manifest_path)
    code_paths = [code_artifact(path) for path in code_files]
    summary = manifest.get("summary")
    feature_count = summary.get("total") if isinstance(summary, dict) and isinstance(summary.get("total"), int) else row_count_for_source(manifest, "event_jsonl")
    contract = {
        "language": "python",
        "schema_version": manifest.get("schema_version"),
        "task_id": manifest.get("task_id"),
        "producer_kind": instrument_kind,
        "determinism": "pure function of promoted upstream artifacts and checked source code",
        "gridworld_policy": "excluded from ME-JEPA-Code Python target panel",
    }
    stable_hashes = {
        "code_group": hash_group(code_paths),
        "preprocessing": preprocessing_hash(contract),
        "source_group": hash_group(source_of_truth),
    }
    return make_record(
        schema_path=schema_path,
        now=now,
        instrument_id=instrument_id,
        panel_role="target_side_frozen_instrument",
        activation_status="active",
        domain_scope="mejepa_code_python",
        instrument_kind=instrument_kind,
        version=f"{manifest.get('task_id')}:{manifest.get('schema_version')}",
        model_or_algorithm=model_or_algorithm,
        output_feature_family=output_feature_family,
        output_dimension=None,
        feature_count=feature_count,
        stable_hashes=stable_hashes,
        preprocessing_contract=contract,
        source_of_truth=source_of_truth,
        code_paths=code_paths,
        used_by_q_heads=q_heads,
        failure_modes=failure_modes,
        edge_guards=edge_guards,
    )


def forward_verified_ids(forward_evidence: dict[str, Any]) -> tuple[set[str], bool]:
    ids: set[str] = set()
    python_code_seen = False
    happy = forward_evidence.get("happy_path", {})
    for bucket in ("pretrained_forward_outputs", "algorithmic_forward_outputs", "e7_language_matrix_outputs"):
        for row in happy.get(bucket, []):
            embedder = row.get("after_state", {}).get("embedder")
            if isinstance(embedder, str):
                ids.add(embedder)
            if embedder == "e7" and row.get("language") == "python" and row.get("passes") is True:
                python_code_seen = True
    return ids, python_code_seen


def embedder_records(args: Any, schema_path: Path, now: str) -> list[dict[str, Any]]:
    config = read_models_config(args.models_config)
    embedders = config["embedders"]
    models_config_artifact = source_artifact("models_config", args.models_config, "models_config")
    embedders_readback_artifact = source_artifact("embedder_digest_readback", args.embedders_readback, "readback")
    forward_readback = read_json(args.forward_readback, "PY_INSTRUMENT_REGISTRY_FORWARD_READBACK_INVALID")
    forward_ids, e7_python_seen = forward_verified_ids(forward_readback)
    if not e7_python_seen:
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_CODE_EMBEDDER_PYTHON_READBACK_MISSING",
            "E7 code embedder has no verified Python forward output in forward READBACK",
            "Run the embedder forward READBACK before registering the Python code panel.",
            {"forward_readback": str(args.forward_readback), "forward_ids": sorted(forward_ids)},
        )
    records = []
    common_code = [
        Path("crates/context-graph-mejepa-embedders/src/embedder_id.rs"),
        Path("crates/context-graph-mejepa-embedders/src/config.rs"),
        Path("crates/context-graph-mejepa-embedders/src/forward.rs"),
        Path("crates/context-graph-mejepa-instruments/src/e_embedder_heads.rs"),
    ]
    for embedder_id in ACTIVE_CONTENT_EMBEDDERS:
        registration = embedders.get(embedder_id)
        if not isinstance(registration, dict):
            raise InstrumentRegistryError(
                "PY_INSTRUMENT_REGISTRY_ACTIVE_EMBEDDER_MISSING",
                f"active content embedder is missing from models_config: {embedder_id}",
                "Populate models_config with all active content embedder registrations.",
                {"embedder_id": embedder_id, "expected": list(ACTIVE_CONTENT_EMBEDDERS)},
            )
        if embedder_id not in forward_ids:
            raise InstrumentRegistryError(
                "PY_INSTRUMENT_REGISTRY_ACTIVE_EMBEDDER_FORWARD_READBACK_MISSING",
                f"active content embedder has no forward READBACK output: {embedder_id}",
                "Run context-graph-mejepa-forward READBACK before registering the panel.",
                {"embedder_id": embedder_id, "forward_ids": sorted(forward_ids)},
            )
        code_paths = [code_artifact(path) for path in common_code]
        is_deterministic = embedder_id in DETERMINISTIC_EMBEDDERS
        weight_sha = registration.get("manifest_sha256")
        if is_deterministic:
            weight_hash = sha256_prefixed_text(canonical_json({"embedder": embedder_id, "algorithm": registration.get("name"), "dimension": registration.get("dimension")}))
            instrument_kind = "content_embedder_deterministic"
        elif isinstance(weight_sha, str) and len(weight_sha) == 64:
            weight_hash = "sha256:" + weight_sha
            instrument_kind = "content_embedder_pretrained"
        else:
            raise InstrumentRegistryError(
                "PY_INSTRUMENT_REGISTRY_ACTIVE_EMBEDDER_WEIGHT_HASH_MISSING",
                f"active pretrained embedder has no manifest_sha256: {embedder_id}",
                "Pin the model manifest digest before using the embedder as target-side instrument.",
                {"embedder_id": embedder_id, "registration": registration},
            )
        contract = {
            "embedder_id": embedder_id,
            "name": registration.get("name"),
            "kind": registration.get("kind"),
            "dimension": registration.get("dimension"),
            "weight_files": registration.get("weight_files", []),
            "input_unit": "Python AST chunk text and code-context tuple",
            "normalization": "forward wrapper validates finite fixed-length vector; downstream DDA/TCT uses pinned weight_hash",
            "gridworld_policy": "excluded from ME-JEPA-Code Python target panel",
        }
        source_of_truth = [
            models_config_artifact,
            embedders_readback_artifact,
            source_artifact("forward_wrapper_readback", args.forward_readback, "readback"),
        ]
        stable_hashes = {
            "code_group": hash_group(code_paths),
            "preprocessing": preprocessing_hash(contract),
            "source_group": hash_group(source_of_truth),
            "weight_or_algorithm": weight_hash,
        }
        records.append(
            make_record(
                schema_path=schema_path,
                now=now,
                instrument_id=f"py_dda_{embedder_id}_{registration.get('name')}_v1",
                panel_role="target_side_embedder",
                activation_status="active",
                domain_scope="mejepa_code_python",
                instrument_kind=instrument_kind,
                version=str(registration.get("manifest_sha256") or f"contextgraph-deterministic-{embedder_id}-v1"),
                model_or_algorithm=str(registration.get("repo") or registration.get("name")),
                output_feature_family="dda_tct_embedder_vector",
                output_dimension=int(registration.get("dimension")),
                feature_count=None,
                stable_hashes=stable_hashes,
                preprocessing_contract=contract,
                source_of_truth=source_of_truth,
                code_paths=code_paths,
                used_by_q_heads=["Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT"],
                failure_modes=["missing_weight_hash", "digest_mismatch", "nonfinite_vector", "dimension_mismatch", "unsupported_language_route"],
                edge_guards=["models_config_digest_readback", "forward_output_readback", "no_cpu_fallback_for_required_gpu_outputs"],
            )
        )
    return records


def build_registry_records(args: Any, schema_path: Path, now: str) -> list[dict[str, Any]]:
    ops = Path("scripts/swebench_lite_ops")
    records = [
        data_instrument(
            schema_path=schema_path,
            now=now,
            instrument_id="py_oracle_events_v1",
            instrument_kind="oracle_event_materializer",
            manifest_path=args.oracle_event_manifest,
            code_files=[ops / "oracle_events.py", ops / "core.py"],
            model_or_algorithm="Docker SWE-bench oracle outcome to typed event/witness chain materializer",
            output_feature_family="oracle_reality_event",
            q_heads=["Q2_WORK", "Q3_WHY_FAIL", "Q5_IMPACT"],
            failure_modes=["missing_oracle_log", "artifact_hash_mismatch", "workspace_tree_hash_mismatch"],
            edge_guards=["malformed_outcome_rejected", "chain_head_sha256_required", "docker_oracle_readback_required"],
        ),
        data_instrument(
            schema_path=schema_path,
            now=now,
            instrument_id="py_ast_parse_v1",
            instrument_kind="ast_parser",
            manifest_path=args.ast_parse_manifest,
            code_files=[ops / "python_ast_parse.py", ops / "oracle_events.py", ops / "core.py"],
            model_or_algorithm="CPython ast.parse plus deterministic file decode/status classification",
            output_feature_family="python_file_parse_state",
            q_heads=["Q1_EXIST", "Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT"],
            failure_modes=["syntax_error", "decode_error", "missing_workspace_file"],
            edge_guards=["invalid_utf8_recorded", "syntax_error_recorded", "empty_file_recorded"],
        ),
        data_instrument(
            schema_path=schema_path,
            now=now,
            instrument_id="py_ast_chunks_v1",
            instrument_kind="ast_chunker",
            manifest_path=args.ast_chunk_manifest,
            code_files=[ops / "python_ast_chunk_command.py", ops / "python_ast_chunks.py", ops / "oracle_events.py", ops / "core.py"],
            model_or_algorithm="AST-aware deterministic Python code-unit chunker",
            output_feature_family="stable_python_ast_chunk",
            q_heads=["Q1_EXIST", "Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT"],
            failure_modes=["chunk_id_instability", "parse_record_mismatch", "line_span_invalid"],
            edge_guards=["module_empty_chunk", "assertion_chunk", "stable_chunk_hash_readback"],
        ),
        data_instrument(
            schema_path=schema_path,
            now=now,
            instrument_id="py_diff_chunk_map_v1",
            instrument_kind="diff_chunk_mapper",
            manifest_path=args.diff_chunk_manifest,
            code_files=[ops / "python_diff_chunk_map.py", ops / "python_diff_chunk_map_core.py", ops / "oracle_events.py", ops / "core.py"],
            model_or_algorithm="unified diff/test hunk to AST chunk mapper",
            output_feature_family="patch_to_reality_chunk_link",
            q_heads=["Q1_EXIST", "Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT"],
            failure_modes=["hunk_without_chunk", "workspace_state_missing", "patch_hash_mismatch"],
            edge_guards=["wrong_file_map_preserved", "compile_error_map_preserved", "test_patch_map_preserved"],
        ),
        data_instrument(
            schema_path=schema_path,
            now=now,
            instrument_id="py_static_reality_graph_v1",
            instrument_kind="static_reality_graph",
            manifest_path=args.static_graph_manifest,
            code_files=[ops / "python_static_graph.py", ops / "python_static_graph_core.py", ops / "python_ast_parse.py", ops / "oracle_events.py", ops / "core.py"],
            model_or_algorithm="Python AST import/call/reference/type/test dependency graph extractor",
            output_feature_family="static_code_reality_graph_edge",
            q_heads=["Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT"],
            failure_modes=["dynamic_import_uncertainty", "unresolved_reference", "circular_import"],
            edge_guards=["dynamic_import_flagged", "optional_dependency_flagged", "circular_import_flagged", "fixture_dependency_flagged"],
        ),
        data_instrument(
            schema_path=schema_path,
            now=now,
            instrument_id="py_test_intent_v1",
            instrument_kind="test_intent_extractor",
            manifest_path=args.test_intent_manifest,
            code_files=[ops / "python_test_intent.py", ops / "python_test_intent_core.py", ops / "oracle_events.py", ops / "core.py"],
            model_or_algorithm="Python test patch AST intent/assertion extractor linked to oracle labels",
            output_feature_family="test_intent_reward_signal",
            q_heads=["Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT"],
            failure_modes=["contradictory_test_label", "missing_test_patch_state", "unlinked_assertion"],
            edge_guards=["parametrized_test", "expected_exception", "skip_xfail", "fixture_record"],
        ),
    ]
    records.extend(embedder_records(args, schema_path, now))
    return sorted(records, key=lambda record: record["instrument_id"])


def validate_schema(records: list[dict[str, Any]], validator: Any) -> None:
    for record in records:
        errors = sorted(validator.iter_errors(record), key=lambda err: list(err.path))
        if errors:
            raise InstrumentRegistryError(
                "PY_INSTRUMENT_REGISTRY_SCHEMA_VALIDATION_FAILED",
                "generated instrument registry record failed strict JSON Schema validation",
                "Fix record construction before writing the registry.",
                {"instrument_id": record.get("instrument_id"), "errors": [err.message for err in errors[:10]]},
            )


def validate_panel(records: list[dict[str, Any]]) -> None:
    active_embedder_ids = set()
    seen_ids: set[str] = set()
    for record in records:
        rid = record.get("instrument_id")
        if rid in seen_ids:
            raise InstrumentRegistryError("PY_INSTRUMENT_REGISTRY_DUPLICATE_ID", f"duplicate instrument_id: {rid}", "Use one canonical record per instrument.", {"instrument_id": rid})
        seen_ids.add(rid)
        validate_record_invariants(record)
        if record["activation_status"] == "active" and record["instrument_kind"].startswith("content_embedder_"):
            active_embedder_ids.add(record["preprocessing_contract"]["embedder_id"])
    missing = sorted(set(ACTIVE_CONTENT_EMBEDDERS) - active_embedder_ids)
    if missing:
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_ACTIVE_CONTENT_EMBEDDER_SET_INCOMPLETE",
            "registry is missing active content embedders",
            "Register every active Python DDA content embedder before downstream reward-field work.",
            {"missing": missing, "expected": list(ACTIVE_CONTENT_EMBEDDERS), "actual": sorted(active_embedder_ids)},
        )


def validate_record_invariants(record: dict[str, Any]) -> None:
    active = record["activation_status"] == "active"
    active_surface = " ".join(
        [
            record["instrument_id"],
            record["model_or_algorithm"],
            record["output_feature_family"],
            " ".join(item["path"] for item in record["source_of_truth"]),
            " ".join(item["path"] for item in record["code_paths"]),
        ]
    ).lower()
    if active and any(token in active_surface for token in PROOF_DOMAIN_TOKENS):
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_NON_CODE_DOMAIN_ACTIVE",
            "proof-domain instrument appeared in active ME-JEPA-Code registry",
            "Remove gridworld/counter-world proof instruments from the Python target panel.",
            {"instrument_id": record["instrument_id"]},
        )
    if active and record["domain_scope"] != "mejepa_code_python":
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_ACTIVE_DOMAIN_INVALID",
            "active Python registry record is outside mejepa_code_python scope",
            "Mark non-code instruments pending or move them out of the Python panel.",
            {"instrument_id": record["instrument_id"], "domain_scope": record["domain_scope"]},
        )
    if active and record["target_side_frozen"] is not True:
        raise InstrumentRegistryError("PY_INSTRUMENT_REGISTRY_TARGET_NOT_FROZEN", "active target-side instrument is not frozen", "Freeze target-side instruments or remove them from the active panel.", {"instrument_id": record["instrument_id"]})
    if active and record["trainable_parameter_count"] != 0:
        raise InstrumentRegistryError("PY_INSTRUMENT_REGISTRY_TRAINABLE_TARGET", "active target-side instrument declares trainable parameters", "Target-side instruments must be deterministic/frozen with zero trainable params.", {"instrument_id": record["instrument_id"], "trainable_parameter_count": record["trainable_parameter_count"]})
    for name, value in record["stable_hashes"].items():
        if not isinstance(value, str) or not value.startswith("sha256:") or len(value) != 71:
            raise InstrumentRegistryError(
                "PY_INSTRUMENT_REGISTRY_STABLE_HASH_MISSING",
                "active instrument is missing a stable hash",
                "Populate model/config/source/code hashes before registering the panel.",
                {"instrument_id": record["instrument_id"], "hash_name": name, "hash_value": value},
            )
    expected_preprocessing = preprocessing_hash(record["preprocessing_contract"])
    if record["preprocessing_hash"] != expected_preprocessing:
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_PREPROCESSING_MISMATCH",
            "preprocessing_hash does not match preprocessing_contract",
            "Regenerate the registry from canonical preprocessing contracts.",
            {"instrument_id": record["instrument_id"], "expected": expected_preprocessing, "actual": record["preprocessing_hash"]},
        )


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    by_kind: dict[str, int] = {}
    by_role: dict[str, int] = {}
    by_status: dict[str, int] = {}
    q_head_coverage: dict[str, int] = {}
    for record in records:
        by_kind[record["instrument_kind"]] = by_kind.get(record["instrument_kind"], 0) + 1
        by_role[record["panel_role"]] = by_role.get(record["panel_role"], 0) + 1
        by_status[record["activation_status"]] = by_status.get(record["activation_status"], 0) + 1
        for head in record["used_by_q_heads"]:
            q_head_coverage[head] = q_head_coverage.get(head, 0) + 1
    return {
        "total": len(records),
        "active_total": sum(1 for record in records if record["activation_status"] == "active"),
        "by_kind": by_kind,
        "by_panel_role": by_role,
        "by_activation_status": by_status,
        "q_head_coverage": q_head_coverage,
        "active_content_embedders": sorted(record["preprocessing_contract"]["embedder_id"] for record in records if record["activation_status"] == "active" and record["instrument_kind"].startswith("content_embedder_")),
        "proof_domain_active_count": sum(1 for record in records if record["activation_status"] == "active" and active_record_mentions_proof_domain(record)),
    }


def active_record_mentions_proof_domain(record: dict[str, Any]) -> bool:
    surface = " ".join(
        [
            record["instrument_id"],
            record["model_or_algorithm"],
            record["output_feature_family"],
            " ".join(item["path"] for item in record["source_of_truth"]),
            " ".join(item["path"] for item in record["code_paths"]),
        ]
    ).lower()
    return any(token in surface for token in PROOF_DOMAIN_TOKENS)


def expect_edge_error(records: list[dict[str, Any]], case: str, mutate: Any, expected_code: str, record_count: int) -> dict[str, Any]:
    mutated = copy.deepcopy(records)
    mutate(mutated)
    before = {"record_count": record_count, "bad_record_count": len(mutated), "case": case}
    try:
        validate_panel(mutated)
    except InstrumentRegistryError as exc:
        return {
            "case": case,
            "before_state": before,
            "trigger": "validate_panel(mutated_registry)",
            "after_state": {"error_code": exc.code, "record_count": record_count},
            "passes": exc.code == expected_code,
        }
    return {
        "case": case,
        "before_state": before,
        "trigger": "validate_panel(mutated_registry)",
        "after_state": {"error_code": None, "record_count": record_count},
        "passes": False,
    }


def edge_cases(records: list[dict[str, Any]], record_count: int) -> list[dict[str, Any]]:
    active_index = next(i for i, record in enumerate(records) if record["activation_status"] == "active")
    return [
        expect_edge_error(records, "missing_weight_or_source_hash", lambda rows: rows[active_index]["stable_hashes"].__setitem__("source_group", ""), "PY_INSTRUMENT_REGISTRY_STABLE_HASH_MISSING", record_count),
        expect_edge_error(records, "trainable_parameter_detected", lambda rows: rows[active_index].__setitem__("trainable_parameter_count", 1), "PY_INSTRUMENT_REGISTRY_TRAINABLE_TARGET", record_count),
        expect_edge_error(records, "preprocessing_version_mismatch", lambda rows: rows[active_index]["preprocessing_contract"].__setitem__("tampered", True), "PY_INSTRUMENT_REGISTRY_PREPROCESSING_MISMATCH", record_count),
        expect_edge_error(records, "proof_domain_instrument_rejected", lambda rows: rows[active_index].update({"instrument_id": "gridworld_active_proof", "domain_scope": "sibling_proof_domain"}), "PY_INSTRUMENT_REGISTRY_NON_CODE_DOMAIN_ACTIVE", record_count),
    ]


def command_instrument_registry(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    assert_hot_root_allowed(args.hot_root)
    args.registry_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.registry_root / "python_frozen_instrument_schema.json"
    records_path = args.registry_root / "python_frozen_instrument_records.jsonl"
    manifest_path = args.registry_root / "python_frozen_instrument_manifest.json"
    readback_path = args.readback_root / "python_frozen_instrument_readback.json"
    before_state = {
        "checked_at": checked_at,
        "record_count": count_jsonl(records_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "manifest": file_state(manifest_path),
        "readback": file_state(readback_path),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "hot_budget_bytes": args.hot_budget_bytes,
        "registry_root_bytes": path_bytes(args.registry_root),
    }
    schema = instrument_schema(schema_path)
    validator = load_jsonschema_validator(schema)
    write_json_checked(schema_path, schema)
    now = utc_now()
    records = build_registry_records(args, schema_path, now)
    validate_schema(records, validator)
    validate_panel(records)
    record_store = write_jsonl_checked(records_path, records)
    if path_bytes(args.hot_root) > args.hot_budget_bytes:
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_HOT_BUDGET_EXCEEDED",
            "instrument registry hot staging exceeded its explicit byte budget",
            "Reduce hot staging volume or increase budget with capacity evidence.",
            {"hot_root": str(args.hot_root), "actual_bytes": path_bytes(args.hot_root), "budget_bytes": args.hot_budget_bytes},
        )
    summary = summarize(records)
    edge_audit = edge_cases(records, record_store["row_count"])
    if summary["proof_domain_active_count"] != 0 or not all(case["passes"] for case in edge_audit):
        raise InstrumentRegistryError(
            "PY_INSTRUMENT_REGISTRY_READBACK_FAILED",
            "instrument registry READBACK checks failed",
            "Fix the registry before downstream DDA/TCT training tasks consume it.",
            {"summary": summary, "edge_cases": edge_audit},
        )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 197,
        "source_of_truth": {"schema": str(schema_path), "records_jsonl": str(records_path), "manifest": str(manifest_path), "readback": str(readback_path)},
        "inputs": {
            "models_config": str(args.models_config),
            "embedders_readback": str(args.embedders_readback),
            "forward_readback": str(args.forward_readback),
            "oracle_event_manifest": str(args.oracle_event_manifest),
            "ast_parse_manifest": str(args.ast_parse_manifest),
            "ast_chunk_manifest": str(args.ast_chunk_manifest),
            "diff_chunk_manifest": str(args.diff_chunk_manifest),
            "static_graph_manifest": str(args.static_graph_manifest),
            "test_intent_manifest": str(args.test_intent_manifest),
        },
        "storage_policy_decision": 212,
        "summary": summary,
        "records_sha256": record_store["sha256"],
        "passes": True,
    }
    write_json_checked(manifest_path, manifest)
    after_state = {
        "checked_at": utc_now(),
        "record_count": count_jsonl(records_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "manifest": file_state(manifest_path),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "hot_budget_bytes": args.hot_budget_bytes,
        "registry_root_bytes": path_bytes(args.registry_root),
    }
    happy_path = {
        "all_active_content_embedders_registered": summary["active_content_embedders"] == sorted(ACTIVE_CONTENT_EMBEDDERS),
        "python_code_embedder_registered": "e7" in summary["active_content_embedders"],
        "core_code_instruments_registered": all(kind in summary["by_kind"] for kind in ("ast_parser", "ast_chunker", "diff_chunk_mapper", "oracle_event_materializer", "static_reality_graph", "test_intent_extractor")),
        "q1_to_q5_covered": all(summary["q_head_coverage"].get(head, 0) > 0 for head in ("Q1_EXIST", "Q2_WORK", "Q3_WHY_FAIL", "Q4_WHAT_ELSE", "Q5_IMPACT")),
        "no_active_gridworld_or_counter_world": summary["proof_domain_active_count"] == 0,
    }
    readback = {
        "schema_version": 1,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 197,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py instrument-registry", "schema_validation": "jsonschema Draft 2020-12, additionalProperties=false"},
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": edge_audit,
        "summary": summary,
        "passes": all(happy_path.values()) and all(case["passes"] for case in edge_audit),
    }
    write_json_checked(readback_path, readback)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": readback["trigger"],
        "after_state": after_state,
        "post_write_readback": {"readback": file_state(readback_path), "records": file_state(records_path), "manifest": file_state(manifest_path), "readback_root_bytes": path_bytes(args.readback_root)},
        "happy_path": happy_path,
        "edge_cases": edge_audit,
        "summary": summary,
        "record_store": record_store,
        "manifest_sha256": sha256_file(manifest_path),
        "readback_sha256": sha256_file(readback_path),
        "passes": readback["passes"],
    }
