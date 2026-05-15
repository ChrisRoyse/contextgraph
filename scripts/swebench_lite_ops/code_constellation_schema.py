from __future__ import annotations

import hashlib
import json
from collections import Counter
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, utc_now, write_json_checked
from .oracle_events import capacity_snapshot, file_state, load_jsonschema_validator, path_bytes, sha256_file, write_jsonl_checked


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-019"
ISSUE_NUMBER = 202
PRODUCER = "contextgraph.swebench_lite_ops.code_constellation_schema"
DATACONTENTTYPE = "application/json"

MUTATION_CATEGORIES = (
    "compile_error",
    "delete_test_call",
    "known_good",
    "off_by_one",
    "over_engineer",
    "subtle_flip",
    "swap_variable",
    "wrong_file",
)

CHUNK_KINDS = (
    "assertion",
    "class",
    "fixture",
    "function",
    "import",
    "module_comment_only",
    "module_statement",
    "syntax_error_file",
    "test_class",
    "test_function",
)

ENTITY_FAMILY_BY_CHUNK_KIND = {
    "assertion": "assertion",
    "class": "class",
    "fixture": "test",
    "function": "function",
    "import": "import",
    "module_comment_only": "module",
    "module_statement": "module",
    "syntax_error_file": "syntax_error",
    "test_class": "test",
    "test_function": "test",
}

FAILURE_REASON_BY_CATEGORY = {
    "compile_error": "syntax_error",
    "delete_test_call": "test_coverage_removed",
    "known_good": "expected_pass_control",
    "off_by_one": "boundary_condition",
    "over_engineer": "irrelevant_complexity",
    "subtle_flip": "logic_flip",
    "swap_variable": "variable_misbind",
    "wrong_file": "wrong_blast_radius",
}

RISK_BY_CATEGORY = {
    "compile_error": "syntax_validity",
    "delete_test_call": "test_coverage_erosion",
    "known_good": "baseline_control",
    "off_by_one": "edge_boundary",
    "over_engineer": "tech_debt_complexity",
    "subtle_flip": "logic_correctness",
    "swap_variable": "dataflow_binding",
    "wrong_file": "impact_scope",
}

INVALID_REASONS = (
    "UNKNOWN_MUTATION_CATEGORY",
    "MISSING_ORACLE_OUTCOME",
    "AMBIGUOUS_CLAIM_TEXT",
)


class CodeConstellationError(SwebenchOpsError):
    pass


def require(condition: bool, code: str, message: str, remediation: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise CodeConstellationError(code, message, remediation, details or {})


def read_json(path: Path, code: str) -> Any:
    require(path.is_file(), code, f"required JSON artifact is missing: {path}", "Regenerate the upstream artifact before constellation schema mapping.", {"path": str(path)})
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise CodeConstellationError(code, f"invalid JSON at {path}", "Regenerate the upstream artifact before constellation schema mapping.", {"path": str(path), "error": str(exc)}) from exc


def iter_jsonl(path: Path, code: str):
    require(path.is_file(), code, f"required JSONL artifact is missing: {path}", "Regenerate the upstream artifact before constellation schema mapping.", {"path": str(path)})
    with path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                yield line_number, json.loads(line)
            except json.JSONDecodeError as exc:
                raise CodeConstellationError(code, f"invalid JSONL line {line_number} at {path}", "Regenerate the upstream artifact before constellation schema mapping.", {"path": str(path), "line_number": line_number, "error": str(exc)}) from exc


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def sha256_text(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def constellation_schema(schema_path: Path) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "issue": ISSUE_NUMBER,
        "artifact_kind": "python_code_teleological_constellation_schema",
        "producer": PRODUCER,
        "active_domain": "python_swebench_lite_300x8",
        "language": "python",
        "archived_domains": ["gridworld", "proof_domain"],
        "unknown_state_policy": "fail_closed_with_typed_invalid_reason",
        "cell_id_template": [
            "language",
            "mutation_category",
            "entity_family",
            "chunk_kind",
            "oracle_work_status",
            "test_intent",
            "instrument_panel_support",
        ],
        "axes": {
            "claim_existence": {
                "enum": ["corpus_gold_patch_present", "claim_absent_corpus_row", "claim_ambiguous", "claim_missing_target"],
                "operator_question": "Q1",
                "invalid_policy": "ambiguous or missing concrete target fails closed before reward assembly",
            },
            "oracle_work_status": {
                "enum": ["oracle_pass", "oracle_fail", "patch_apply_fail"],
                "operator_question": "Q2",
                "invalid_policy": "missing oracle outcome fails closed",
            },
            "failure_reason": {
                "enum": list(FAILURE_REASON_BY_CATEGORY.values()),
                "operator_question": "Q3",
                "invalid_policy": "unknown mutation category fails closed",
            },
            "edge_risk_concern": {
                "enum": list(RISK_BY_CATEGORY.values()),
                "operator_question": "Q4",
                "invalid_policy": "unknown risk axis fails closed",
            },
            "impact_reality": {
                "enum": ["oracle_bound_patch_and_tests", "oracle_bound_patch_only", "patch_apply_failed"],
                "operator_question": "Q5",
                "invalid_policy": "missing patch/oracle provenance fails closed",
            },
            "mutation_category": {"enum": list(MUTATION_CATEGORIES), "invalid_policy": "unknown category fails closed"},
            "chunk_kind": {"enum": list(CHUNK_KINDS), "invalid_policy": "unknown chunk kind fails closed"},
            "entity_family": {"enum": sorted(set(ENTITY_FAMILY_BY_CHUNK_KIND.values())), "invalid_policy": "unknown entity family fails closed"},
            "test_intent": {
                "enum": ["fail_to_pass_and_pass_to_pass", "fail_to_pass_only", "pass_to_pass_only", "no_expected_tests"],
                "invalid_policy": "missing expected_labels is invalid for corpus reward rows",
            },
            "instrument_panel_support": {
                "enum": ["ast_oracle_diff_test_static_embedder_panel"],
                "invalid_policy": "missing required deterministic instrument support fails closed",
            },
        },
        "minimum_support": {
            "cell_hard_min": 5,
            "bayesian_shrink_under": 50,
            "refresh_after_new_examples": 100,
            "unsupported_cell_policy": "persist invalid/low_support state; do not synthesize centroids",
        },
        "centroid_reference_construction": {
            "source_vectors": "completed Python AST chunk embedder forwards from the frozen 12-slot content panel",
            "cell_group_by": ["mutation_category", "entity_family", "language", "embedder_id"],
            "reference_statistic": "mean L2-normalized vector per cell with support/readback metadata",
            "target_side_trainable_parameters": 0,
        },
        "record_schema_path": str(schema_path),
        "research_basis": [
            {
                "name": "SWE-bench dataset fields",
                "url": "https://www.swebench.com/SWE-bench/guides/datasets/",
                "use": "instance_id/repo/base_commit/patch/test_patch/FAIL_TO_PASS/PASS_TO_PASS define durable Python corpus axes",
            },
            {
                "name": "TensorFlow Data Validation",
                "url": "https://www.tensorflow.org/tfx/data_validation/get_started",
                "use": "schema validation plus skew/drift thinking informs fail-closed corpus axis validation",
            },
            {
                "name": "Great Expectations schema validation",
                "url": "https://docs.greatexpectations.io/docs/reference/learn/data_quality_use_cases/schema",
                "use": "persisted validation results and schema versioning inform the FSV artifact shape",
            },
        ],
    }


def record_json_schema(schema_path: Path) -> dict[str, Any]:
    sha = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
    axes_properties = {
        "claim_existence": {"enum": ["corpus_gold_patch_present", "claim_absent_corpus_row", "claim_ambiguous", "claim_missing_target", "invalid"]},
        "oracle_work_status": {"enum": ["oracle_pass", "oracle_fail", "patch_apply_fail", "invalid"]},
        "failure_reason": {"enum": list(FAILURE_REASON_BY_CATEGORY.values()) + ["invalid"]},
        "edge_risk_concern": {"enum": list(RISK_BY_CATEGORY.values()) + ["invalid"]},
        "impact_reality": {"enum": ["oracle_bound_patch_and_tests", "oracle_bound_patch_only", "patch_apply_failed", "invalid"]},
        "mutation_category": {"enum": list(MUTATION_CATEGORIES) + ["invalid"]},
        "chunk_kind": {"enum": list(CHUNK_KINDS) + ["invalid"]},
        "entity_family": {"enum": sorted(set(ENTITY_FAMILY_BY_CHUNK_KIND.values())) + ["invalid"]},
        "test_intent": {"enum": ["fail_to_pass_and_pass_to_pass", "fail_to_pass_only", "pass_to_pass_only", "no_expected_tests", "invalid"]},
        "instrument_panel_support": {"enum": ["ast_oracle_diff_test_static_embedder_panel", "invalid"]},
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python Code Teleological Constellation Mapping v1",
        "type": "object",
        "additionalProperties": False,
        "required": [
            "schema_version",
            "record_id",
            "cell_id",
            "valid",
            "invalid_reason",
            "source",
            "datacontenttype",
            "time",
            "producer",
            "task_id",
            "issue",
            "language",
            "repo",
            "task_instance_id",
            "row_id",
            "raw_mutation_category",
            "chunk_id",
            "raw_chunk_kind",
            "workspace_state",
            "axes",
            "input_hashes",
            "support_context",
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "record_id": sha,
            "cell_id": {"type": "string", "pattern": "^(tct|invalid):v1:sha256:[0-9a-f]{64}$"},
            "valid": {"type": "boolean"},
            "invalid_reason": {"enum": [None, *INVALID_REASONS]},
            "source": {"type": "string", "minLength": 1},
            "datacontenttype": {"const": DATACONTENTTYPE},
            "time": {"type": "string", "minLength": 1},
            "producer": {"const": PRODUCER},
            "task_id": {"const": TASK_ID},
            "issue": {"const": ISSUE_NUMBER},
            "language": {"const": "python"},
            "repo": {"type": "string", "minLength": 1},
            "task_instance_id": {"type": "string", "minLength": 1},
            "row_id": {"type": "string", "minLength": 1},
            "raw_mutation_category": {"type": "string", "minLength": 1},
            "chunk_id": sha,
            "raw_chunk_kind": {"type": "string", "minLength": 1},
            "workspace_state": {"type": "string", "minLength": 1},
            "axes": {
                "type": "object",
                "additionalProperties": False,
                "required": list(axes_properties),
                "properties": axes_properties,
            },
            "input_hashes": {
                "type": "object",
                "additionalProperties": False,
                "required": ["cleaned_manifest_row_sha256", "chunk_source_sha256", "chunk_record_sha256", "schema_sha256"],
                "properties": {
                    "cleaned_manifest_row_sha256": sha,
                    "chunk_source_sha256": sha,
                    "chunk_record_sha256": sha,
                    "schema_sha256": sha,
                },
            },
            "support_context": {
                "type": "object",
                "additionalProperties": False,
                "required": ["category_support_rows", "category_chunk_rows", "cell_support_policy", "centroid_reference"],
                "properties": {
                    "category_support_rows": {"type": "integer", "minimum": 0},
                    "category_chunk_rows": {"type": "integer", "minimum": 0},
                    "cell_support_policy": {"enum": ["eligible", "low_support_shrink", "invalid"]},
                    "centroid_reference": {"type": "string", "minLength": 1},
                },
            },
        },
    }


def load_sample_rows(cleaned_manifest: Path) -> tuple[dict[str, dict[str, Any]], dict[str, int], str]:
    doc = read_json(cleaned_manifest, "PY_CONSTELLATION_CLEANED_MANIFEST_INVALID")
    rows = doc.get("rows")
    require(isinstance(rows, list) and rows, "PY_CONSTELLATION_CLEANED_ROWS_EMPTY", "cleaned manifest has no rows", "Regenerate TASK-PY-DATA-005 before schema mapping.", {"path": str(cleaned_manifest)})
    by_category: dict[str, dict[str, Any]] = {}
    counts: Counter[str] = Counter()
    for row in rows:
        category = str(row.get("mutation_category") or "")
        counts[category] += 1
        if category in MUTATION_CATEGORIES and category not in by_category:
            by_category[category] = row
    missing = sorted(set(MUTATION_CATEGORIES) - set(by_category))
    require(not missing, "PY_CONSTELLATION_CATEGORY_SAMPLE_MISSING", "cleaned manifest does not cover every Python mutation category", "Regenerate the 300x8 cleaned manifest before schema mapping.", {"missing": missing})
    return by_category, dict(sorted(counts.items())), sha256_file(cleaned_manifest)


def chunk_priority(row: dict[str, Any]) -> tuple[int, int]:
    priority = {
        "function": 0,
        "class": 1,
        "test_function": 2,
        "test_class": 3,
        "assertion": 4,
        "module_statement": 5,
        "import": 6,
        "syntax_error_file": 7,
    }
    return (priority.get(str(row.get("chunk_kind")), 99), int(row.get("line_start") or 0))


def load_sample_chunks(chunks_jsonl: Path, sample_rows: dict[str, dict[str, Any]]) -> tuple[dict[str, dict[str, Any]], dict[str, int], str]:
    targets = {(row["task_id"], category) for category, row in sample_rows.items()}
    selected: dict[str, dict[str, Any]] = {}
    chunk_counts: Counter[str] = Counter()
    for _, chunk in iter_jsonl(chunks_jsonl, "PY_CONSTELLATION_CHUNKS_JSONL_INVALID"):
        category = str(chunk.get("mutation_category") or "")
        chunk_counts[category] += 1
        key = (str(chunk.get("task_instance_id") or ""), category)
        if key not in targets:
            continue
        previous = selected.get(category)
        if previous is None or chunk_priority(chunk) < chunk_priority(previous):
            selected[category] = chunk
    missing = sorted(set(MUTATION_CATEGORIES) - set(selected))
    require(not missing, "PY_CONSTELLATION_CHUNK_SAMPLE_MISSING", "AST chunks do not cover every sampled mutation category", "Regenerate TASK-PY-DATA-058 before schema mapping.", {"missing": missing})
    return selected, dict(sorted(chunk_counts.items())), sha256_file(chunks_jsonl)


def test_intent_axis(row: dict[str, Any]) -> str:
    labels = row.get("expected_labels")
    if not isinstance(labels, dict):
        return "invalid"
    fail_to_pass = int(labels.get("fail_to_pass_count") or len(labels.get("fail_to_pass") or []))
    pass_to_pass = int(labels.get("pass_to_pass_count") or len(labels.get("pass_to_pass") or []))
    if fail_to_pass and pass_to_pass:
        return "fail_to_pass_and_pass_to_pass"
    if fail_to_pass:
        return "fail_to_pass_only"
    if pass_to_pass:
        return "pass_to_pass_only"
    return "no_expected_tests"


def oracle_axis(row: dict[str, Any]) -> tuple[str, str | None]:
    oracle = row.get("oracle")
    if not isinstance(oracle, dict):
        return "invalid", "MISSING_ORACLE_OUTCOME"
    if oracle.get("patch_successfully_applied") is False:
        return "patch_apply_fail", None
    if oracle.get("all_passed") is True and oracle.get("resolved") is True:
        return "oracle_pass", None
    return "oracle_fail", None


def impact_axis(row: dict[str, Any], oracle_work_status: str) -> str:
    if oracle_work_status == "patch_apply_fail":
        return "patch_apply_failed"
    if row.get("test_patch"):
        return "oracle_bound_patch_and_tests"
    return "oracle_bound_patch_only"


def ambiguous_claim(claim_text: str | None) -> bool:
    if claim_text is None:
        return False
    lowered = claim_text.strip().lower()
    ambiguous_terms = ("maybe", "probably", "stuff", "things", "various", "etc")
    return not lowered or any(term in lowered for term in ambiguous_terms)


def invalid_axes() -> dict[str, str]:
    return {
        "claim_existence": "invalid",
        "oracle_work_status": "invalid",
        "failure_reason": "invalid",
        "edge_risk_concern": "invalid",
        "impact_reality": "invalid",
        "mutation_category": "invalid",
        "chunk_kind": "invalid",
        "entity_family": "invalid",
        "test_intent": "invalid",
        "instrument_panel_support": "invalid",
    }


def build_record(
    *,
    row: dict[str, Any],
    chunk: dict[str, Any],
    schema_sha256: str,
    row_counts: dict[str, int],
    chunk_counts: dict[str, int],
    now: str,
    claim_text: str | None,
) -> dict[str, Any]:
    raw_category = str(row.get("mutation_category") or "")
    raw_chunk_kind = str(chunk.get("chunk_kind") or "")
    invalid_reason = None
    if raw_category not in MUTATION_CATEGORIES:
        invalid_reason = "UNKNOWN_MUTATION_CATEGORY"
    elif ambiguous_claim(claim_text):
        invalid_reason = "AMBIGUOUS_CLAIM_TEXT"
    oracle_work_status, oracle_invalid = oracle_axis(row)
    if invalid_reason is None and oracle_invalid is not None:
        invalid_reason = oracle_invalid
    if invalid_reason is None and raw_chunk_kind not in CHUNK_KINDS:
        invalid_reason = "UNKNOWN_MUTATION_CATEGORY"
    valid = invalid_reason is None
    if valid:
        axes = {
            "claim_existence": "corpus_gold_patch_present" if row.get("patch") or row.get("gold_patch") else "claim_absent_corpus_row",
            "oracle_work_status": oracle_work_status,
            "failure_reason": FAILURE_REASON_BY_CATEGORY[raw_category],
            "edge_risk_concern": RISK_BY_CATEGORY[raw_category],
            "impact_reality": impact_axis(row, oracle_work_status),
            "mutation_category": raw_category,
            "chunk_kind": raw_chunk_kind,
            "entity_family": ENTITY_FAMILY_BY_CHUNK_KIND[raw_chunk_kind],
            "test_intent": test_intent_axis(row),
            "instrument_panel_support": "ast_oracle_diff_test_static_embedder_panel",
        }
    else:
        axes = invalid_axes()
    cell_material = {
        "schema_version": SCHEMA_VERSION,
        "language": "python",
        "axes": axes,
        "raw_mutation_category": raw_category,
        "raw_chunk_kind": raw_chunk_kind,
        "invalid_reason": invalid_reason,
        "valid": valid,
    }
    prefix = "tct" if valid else "invalid"
    cell_id = f"{prefix}:v1:{sha256_text(canonical_json(cell_material))}"
    record_material = {
        "cell_id": cell_id,
        "row_id": row.get("row_id"),
        "chunk_id": chunk.get("chunk_id"),
        "invalid_reason": invalid_reason,
    }
    category_support = int(row_counts.get(raw_category, 0))
    category_chunks = int(chunk_counts.get(raw_category, 0))
    support_policy = "invalid" if not valid else "eligible" if category_support >= 50 and category_chunks >= 50 else "low_support_shrink"
    return {
        "schema_version": SCHEMA_VERSION,
        "record_id": sha256_text(canonical_json(record_material)),
        "cell_id": cell_id,
        "valid": valid,
        "invalid_reason": invalid_reason,
        "source": f"contextgraph://python-code-constellation-schema/{row.get('row_id')}/{chunk.get('chunk_id')}",
        "datacontenttype": DATACONTENTTYPE,
        "time": now,
        "producer": PRODUCER,
        "task_id": TASK_ID,
        "issue": ISSUE_NUMBER,
        "language": "python",
        "repo": str(row.get("repo") or chunk.get("repo") or "unknown"),
        "task_instance_id": str(row.get("task_id") or chunk.get("task_instance_id") or ""),
        "row_id": str(row.get("row_id") or ""),
        "raw_mutation_category": raw_category,
        "chunk_id": str(chunk.get("chunk_id") or ""),
        "raw_chunk_kind": raw_chunk_kind,
        "workspace_state": str(chunk.get("workspace_state") or ""),
        "axes": axes,
        "input_hashes": {
            "cleaned_manifest_row_sha256": str(row.get("row_sha256") or sha256_text(canonical_json(row))),
            "chunk_source_sha256": str(chunk.get("source_sha256") or sha256_text(str(chunk.get("source_text") or ""))),
            "chunk_record_sha256": sha256_text(canonical_json(chunk)),
            "schema_sha256": schema_sha256,
        },
        "support_context": {
            "category_support_rows": category_support,
            "category_chunk_rows": category_chunks,
            "cell_support_policy": support_policy,
            "centroid_reference": "mean_l2_normalized_vector_by_mutation_entity_language_embedder",
        },
    }


def validate_records(records: list[dict[str, Any]], validator: Any) -> None:
    for record in records:
        errors = sorted(validator.iter_errors(record), key=lambda err: list(err.path))
        if errors:
            raise CodeConstellationError(
                "PY_CONSTELLATION_SCHEMA_VALIDATION_FAILED",
                "generated constellation mapping failed strict JSON Schema validation",
                "Fix mapping construction before using these cells for reward assembly.",
                {"record_id": record.get("record_id"), "errors": [err.message for err in errors[:10]]},
            )


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "total": len(records),
        "valid": sum(1 for record in records if record["valid"]),
        "invalid": sum(1 for record in records if not record["valid"]),
        "by_mutation_category": dict(sorted(Counter(record["raw_mutation_category"] for record in records).items())),
        "by_entity_family": dict(sorted(Counter(record["axes"]["entity_family"] for record in records).items())),
        "by_oracle_work_status": dict(sorted(Counter(record["axes"]["oracle_work_status"] for record in records).items())),
    }


def command_code_constellation_schema(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    args.schema_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.schema_root / "code_constellation_schema.json"
    record_schema_path = args.schema_root / "code_constellation_mapping_record_schema.json"
    records_path = args.schema_root / "code_constellation_mapping_records.jsonl"
    edge_records_path = args.schema_root / "edge_case_constellation_mapping_records.jsonl"
    manifest_path = args.schema_root / "code_constellation_schema_manifest.json"
    fsv_path = args.fsv_root / "code_constellation_schema_fsv.json"
    before_state = {
        "checked_at": checked_at,
        "schema_root": file_state(args.schema_root),
        "schema": file_state(schema_path),
        "record_schema": file_state(record_schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_records_path),
        "manifest": file_state(manifest_path),
        "fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
        "schema_root_bytes": path_bytes(args.schema_root),
    }
    record_schema = record_json_schema(record_schema_path)
    write_json_checked(record_schema_path, record_schema)
    schema = constellation_schema(record_schema_path)
    write_json_checked(schema_path, schema)
    schema_sha256 = sha256_file(schema_path)
    validator = load_jsonschema_validator(record_schema)
    rows_by_category, row_counts, cleaned_sha = load_sample_rows(args.cleaned_manifest)
    chunks_by_category, chunk_counts, chunks_sha = load_sample_chunks(args.chunks_jsonl, rows_by_category)
    now = utc_now()
    records = [
        build_record(
            row=rows_by_category[category],
            chunk=chunks_by_category[category],
            schema_sha256=schema_sha256,
            row_counts=row_counts,
            chunk_counts=chunk_counts,
            now=now,
            claim_text=f"Apply {category} Python corpus mutation for {rows_by_category[category]['task_id']}",
        )
        for category in MUTATION_CATEGORIES
    ]
    validate_records(records, validator)
    record_store = write_jsonl_checked(records_path, records)
    edge_base_row = dict(rows_by_category["known_good"])
    edge_base_chunk = dict(chunks_by_category["known_good"])
    unknown_category_row = dict(edge_base_row)
    unknown_category_row["mutation_category"] = "new_unknown_category"
    missing_oracle_row = dict(edge_base_row)
    missing_oracle_row.pop("oracle", None)
    ambiguous_claim_row = dict(edge_base_row)
    edge_records = [
        build_record(row=unknown_category_row, chunk=edge_base_chunk, schema_sha256=schema_sha256, row_counts=row_counts, chunk_counts=chunk_counts, now=now, claim_text="Apply a concrete unknown mutation"),
        build_record(row=missing_oracle_row, chunk=edge_base_chunk, schema_sha256=schema_sha256, row_counts=row_counts, chunk_counts=chunk_counts, now=now, claim_text="Apply known_good Python corpus mutation"),
        build_record(row=ambiguous_claim_row, chunk=edge_base_chunk, schema_sha256=schema_sha256, row_counts=row_counts, chunk_counts=chunk_counts, now=now, claim_text="maybe fixed various stuff etc"),
    ]
    validate_records(edge_records, validator)
    edge_store = write_jsonl_checked(edge_records_path, edge_records)
    edge_cases = [
        {
            "case": "unknown_mutation_category",
            "before": {"raw_mutation_category": "new_unknown_category"},
            "action": "map row through strict constellation axes",
            "after": {"valid": edge_records[0]["valid"], "invalid_reason": edge_records[0]["invalid_reason"]},
            "expected": "valid=false with UNKNOWN_MUTATION_CATEGORY",
            "pass": edge_records[0]["valid"] is False and edge_records[0]["invalid_reason"] == "UNKNOWN_MUTATION_CATEGORY",
        },
        {
            "case": "missing_oracle_outcome",
            "before": {"oracle_present": False},
            "action": "map row through oracle_work_status axis",
            "after": {"valid": edge_records[1]["valid"], "invalid_reason": edge_records[1]["invalid_reason"]},
            "expected": "valid=false with MISSING_ORACLE_OUTCOME",
            "pass": edge_records[1]["valid"] is False and edge_records[1]["invalid_reason"] == "MISSING_ORACLE_OUTCOME",
        },
        {
            "case": "ambiguous_claim_text",
            "before": {"claim_text": "maybe fixed various stuff etc"},
            "action": "map row through claim_existence axis",
            "after": {"valid": edge_records[2]["valid"], "invalid_reason": edge_records[2]["invalid_reason"]},
            "expected": "valid=false with AMBIGUOUS_CLAIM_TEXT",
            "pass": edge_records[2]["valid"] is False and edge_records[2]["invalid_reason"] == "AMBIGUOUS_CLAIM_TEXT",
        },
    ]
    summary = summarize(records)
    happy_path = {
        "all_mutation_categories_mapped": sorted(summary["by_mutation_category"]) == list(MUTATION_CATEGORIES),
        "all_records_valid": summary["valid"] == len(MUTATION_CATEGORIES) and summary["invalid"] == 0,
        "strict_schema_has_no_additional_properties": record_schema.get("additionalProperties") is False,
        "gridworld_archived_not_active": "gridworld" in schema["archived_domains"] and schema["active_domain"] == "python_swebench_lite_300x8",
        "cell_ids_are_deterministic": len({record["cell_id"] for record in records}) == len(records),
    }
    require(all(happy_path.values()), "PY_CONSTELLATION_HAPPY_PATH_FAILED", "constellation schema happy path failed", "Fix schema axes or real corpus mapping before downstream reward-field assembly.", {"happy_path": happy_path})
    require(all(case["pass"] for case in edge_cases), "PY_CONSTELLATION_EDGE_CASE_FAILED", "constellation schema edge-case audit failed", "Fix fail-closed validation before downstream reward-field assembly.", {"edge_cases": edge_cases})
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": ISSUE_NUMBER,
        "passes": True,
        "source_of_truth": {
            "schema": str(schema_path),
            "record_schema": str(record_schema_path),
            "records_jsonl": str(records_path),
            "edge_records_jsonl": str(edge_records_path),
            "manifest": str(manifest_path),
            "fsv": str(fsv_path),
        },
        "inputs": {
            "cleaned_manifest": str(args.cleaned_manifest),
            "chunks_jsonl": str(args.chunks_jsonl),
        },
        "input_hashes": {
            "cleaned_manifest": cleaned_sha,
            "chunks_jsonl": chunks_sha,
            "schema": schema_sha256,
        },
        "summary": summary,
        "record_store": record_store,
        "edge_store": edge_store,
    }
    write_json_checked(manifest_path, manifest)
    after_state = {
        "checked_at": utc_now(),
        "schema": file_state(schema_path),
        "record_schema": file_state(record_schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_records_path),
        "manifest": file_state(manifest_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
        "schema_root_bytes": path_bytes(args.schema_root),
        "record_count": count_jsonl(records_path),
        "edge_record_count": count_jsonl(edge_records_path),
    }
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": ISSUE_NUMBER,
        "passes": True,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py code-constellation-schema", "validation": "jsonschema Draft 2020-12, additionalProperties=false"},
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": edge_cases,
        "summary": summary,
        "research_basis": schema["research_basis"],
    }
    write_json_checked(fsv_path, fsv)
    final_readback = read_json(fsv_path, "PY_CONSTELLATION_FSV_READBACK_INVALID")
    require(final_readback.get("passes") is True, "PY_CONSTELLATION_FSV_READBACK_FAILED", "FSV readback did not pass", "Inspect the FSV artifact and rerun after fixing schema generation.", {"fsv": str(fsv_path)})
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": fsv["trigger"],
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": edge_cases,
        "summary": summary,
        "manifest_sha256": sha256_file(manifest_path),
        "fsv_sha256": sha256_file(fsv_path),
        "passes": True,
    }
