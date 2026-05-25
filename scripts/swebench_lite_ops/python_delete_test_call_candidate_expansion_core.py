"""Shared helpers for delete_test_call candidate expansion."""

from __future__ import annotations

import argparse
import ast
import json
import os
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file
from .python_delete_test_call_regeneration import has_mounted_drive_fragment


TASK_ID = "TASK-PY-G-135E"
ARTIFACT_KIND = "delete_test_call_candidate_expansion_plan"
DEFAULT_BATCH_KEY_PREFIX = "delete-test-call-135e-expansion-batch"
DEFAULT_SELECTION_POLICY = "fixed_split_sublabel_quota_no_oracle_label_selection_v1"
PASS_HYPOTHESIS_POLICY_VERSION = "delete_test_call_pre_oracle_pass_hypothesis_v1"

DEFAULT_TARGETS: dict[tuple[str, str], int] = {
    ("holdout", "other_assertion_like_deleted"): 8,
    ("holdout", "production_plain_assert_deleted"): 8,
    ("holdout", "pytest_raises_deleted"): 4,
    ("holdout", "test_self_assert_deleted"): 3,
    ("calibration", "other_assertion_like_deleted"): 2,
    ("calibration", "production_plain_assert_deleted"): 6,
    ("calibration", "pytest_raises_deleted"): 2,
    ("calibration", "test_self_assert_deleted"): 6,
    ("train", "other_assertion_like_deleted"): 2,
    ("train", "production_plain_assert_deleted"): 4,
    ("train", "pytest_raises_deleted"): 2,
    ("train", "test_self_assert_deleted"): 4,
}

TARGET_ORDER = (
    ("holdout", "other_assertion_like_deleted"),
    ("holdout", "production_plain_assert_deleted"),
    ("holdout", "pytest_raises_deleted"),
    ("holdout", "test_self_assert_deleted"),
    ("calibration", "production_plain_assert_deleted"),
    ("calibration", "test_self_assert_deleted"),
    ("calibration", "other_assertion_like_deleted"),
    ("calibration", "pytest_raises_deleted"),
    ("train", "production_plain_assert_deleted"),
    ("train", "test_self_assert_deleted"),
    ("train", "other_assertion_like_deleted"),
    ("train", "pytest_raises_deleted"),
)


class CandidateExpansionError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the candidate patches, existing oracle outcomes, and fixed expansion quotas before rerunning.",
            details or {},
        )


def validate_paths(args: argparse.Namespace) -> None:
    for name in (
        "candidate_patches",
        "existing_label_root",
        "existing_outcome_search_root",
        "next_action_report",
        "output_root",
        "readback_root",
        "evidence_root",
    ):
        path = getattr(args, name)
        if path is not None and has_mounted_drive_fragment(path):
            raise CandidateExpansionError(
                "DTC_EXPANSION_MOUNTED_PATH_REJECTED",
                f"{name} points at mounted-drive lineage: {path}",
                {"name": name, "path": str(path)},
            )
    for path in args.existing_outcomes_jsonl or []:
        if has_mounted_drive_fragment(path):
            raise CandidateExpansionError(
                "DTC_EXPANSION_MOUNTED_PATH_REJECTED",
                f"existing outcome path points at mounted-drive lineage: {path}",
                {"name": "existing_outcomes_jsonl", "path": str(path)},
            )
    if not args.candidate_patches.is_file():
        raise CandidateExpansionError(
            "DTC_EXPANSION_PATCHES_MISSING",
            f"candidate patches JSONL missing: {args.candidate_patches}",
            {"candidate_patches": str(args.candidate_patches)},
        )
    if not args.next_action_report.is_file():
        raise CandidateExpansionError(
            "DTC_EXPANSION_NEXT_ACTION_MISSING",
            f"next-action report missing: {args.next_action_report}",
            {"next_action_report": str(args.next_action_report)},
        )


def target_quotas(overrides: list[str] | None) -> dict[tuple[str, str], int]:
    targets = dict(DEFAULT_TARGETS)
    for raw in overrides or []:
        if "=" not in raw or ":" not in raw.split("=", 1)[0]:
            raise CandidateExpansionError(
                "DTC_EXPANSION_BAD_TARGET",
                f"target override must be split:sublabel=count: {raw}",
                {"target": raw},
            )
        lhs, value_text = raw.split("=", 1)
        split, sublabel = lhs.split(":", 1)
        try:
            value = int(value_text)
        except ValueError as exc:
            raise CandidateExpansionError(
                "DTC_EXPANSION_BAD_TARGET_COUNT",
                f"target count is not an integer: {raw}",
                {"target": raw},
            ) from exc
        if value < 0:
            raise CandidateExpansionError(
                "DTC_EXPANSION_NEGATIVE_TARGET_COUNT",
                f"target count must be non-negative: {raw}",
                {"target": raw},
            )
        targets[(split.strip(), sublabel.strip())] = value
    return targets


def read_next_action(path: Path) -> dict[str, Any]:
    payload = read_json(path, "DTC_EXPANSION_NEXT_ACTION_BAD_JSON")
    if payload.get("recommended_action_kind") != "generate_more_split_stable_candidate_oracle_rows":
        raise CandidateExpansionError(
            "DTC_EXPANSION_NEXT_ACTION_KIND_UNEXPECTED",
            "next-action report does not request more split-stable candidate oracle rows",
            {"path": str(path), "recommended_action_kind": payload.get("recommended_action_kind")},
        )
    if payload.get("holdout_tuning_used") is not False:
        raise CandidateExpansionError(
            "DTC_EXPANSION_NEXT_ACTION_HOLDOUT_TUNED",
            "next-action report is not holdout-clean",
            {"path": str(path), "holdout_tuning_used": payload.get("holdout_tuning_used")},
        )
    return payload


def load_candidate_patches(path: Path) -> list[dict[str, Any]]:
    rows = read_jsonl(path, "DTC_EXPANSION_PATCHES_BAD_JSONL")
    seen: set[str] = set()
    for line_number, row in enumerate(rows, start=1):
        candidate_id = require_string(row, "candidate_row_id", path, line_number)
        if candidate_id in seen:
            raise CandidateExpansionError(
                "DTC_EXPANSION_DUPLICATE_PATCH_ID",
                f"duplicate candidate_row_id in patch rows: {candidate_id}",
                {"path": str(path), "line_number": line_number, "candidate_row_id": candidate_id},
            )
        seen.add(candidate_id)
        for field in ("source_task_id", "stable_partition", "assertion_duty_sublabel", "source_path", "patch_path"):
            require_string(row, field, path, line_number)
        if row.get("oracle_status") != "ready":
            raise CandidateExpansionError(
                "DTC_EXPANSION_PATCH_NOT_READY",
                f"candidate patch row is not ready: {candidate_id}",
                {"path": str(path), "line_number": line_number, "oracle_status": row.get("oracle_status")},
            )
        for field in ("source_file", "patch_path"):
            value = Path(str(row.get(field)))
            if has_mounted_drive_fragment(value):
                raise CandidateExpansionError(
                    "DTC_EXPANSION_PATCH_MOUNTED_PATH",
                    f"{field} contains mounted-drive lineage",
                    {"candidate_row_id": candidate_id, "field": field, "value": str(value)},
                )
    return rows


def load_existing_candidate_ids(args: argparse.Namespace) -> tuple[set[str], dict[str, Any]]:
    ids: set[str] = set()
    sources: dict[str, Any] = {}
    label_path = args.existing_label_root / "candidate_code_state_constellations.jsonl"
    if label_path.is_file():
        label_rows = read_jsonl(label_path, "DTC_EXPANSION_EXISTING_LABELS_BAD_JSONL")
        label_ids = {str(row.get("candidate_row_id")) for row in label_rows if row.get("candidate_row_id")}
        ids.update(label_ids)
        sources["existing_label_rows"] = {"path": str(label_path), "count": len(label_ids), "sha256": sha256_file(label_path)}
    else:
        sources["existing_label_rows"] = {"path": str(label_path), "count": 0, "missing": True}

    outcome_paths = resolve_existing_outcome_paths(args)
    outcome_ids: set[str] = set()
    for path in outcome_paths:
        rows = read_jsonl(path, "DTC_EXPANSION_EXISTING_OUTCOMES_BAD_JSONL")
        outcome_ids.update(str(row.get("candidate_row_id")) for row in rows if row.get("candidate_row_id"))
    ids.update(outcome_ids)
    sources["existing_outcome_rows"] = {
        "paths": [str(path) for path in outcome_paths],
        "distinct_candidate_count": len(outcome_ids),
    }
    return ids, sources


def resolve_existing_outcome_paths(args: argparse.Namespace) -> list[Path]:
    paths = list(args.existing_outcomes_jsonl or [])
    if args.existing_outcome_search_root.is_dir():
        paths.extend(sorted(args.existing_outcome_search_root.rglob("delete_test_call_oracle_outcomes.jsonl")))
    unique: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key = str(path)
        if key in seen:
            continue
        if not path.is_file():
            raise CandidateExpansionError(
                "DTC_EXPANSION_EXISTING_OUTCOME_MISSING",
                f"existing outcome JSONL missing: {path}",
                {"path": str(path)},
            )
        unique.append(path)
        seen.add(key)
    return unique


def select_candidates(
    candidates: list[dict[str, Any]],
    existing_ids: set[str],
    targets: dict[tuple[str, str], int],
    *,
    prefer_pass_hypothesis: bool = False,
    require_pass_hypothesis: bool = False,
) -> tuple[list[dict[str, Any]], dict[str, dict[str, int]], Counter[tuple[str, str]]]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in candidates:
        if row["candidate_row_id"] in existing_ids:
            continue
        key = (str(row["stable_partition"]), str(row["assertion_duty_sublabel"]))
        if key in targets:
            if prefer_pass_hypothesis or require_pass_hypothesis:
                ensure_pre_oracle_pass_hypothesis(row)
                if require_pass_hypothesis and not has_pre_oracle_pass_hypothesis(row):
                    continue
            grouped[key].append(row)
    for key in grouped:
        grouped[key].sort(key=lambda row: expansion_selection_sort_key(row, prefer_pass_hypothesis))

    selected: list[dict[str, Any]] = []
    shortfalls: dict[str, dict[str, int]] = {}
    available_counts: Counter[tuple[str, str]] = Counter({key: len(value) for key, value in grouped.items()})
    ordered_keys = list(TARGET_ORDER) + sorted(set(targets) - set(TARGET_ORDER))
    for key in ordered_keys:
        target = targets.get(key, 0)
        if target <= 0:
            continue
        available = grouped.get(key, [])
        take = min(target, len(available))
        selected.extend(available[:take])
        if take < target:
            shortfalls["|".join(key)] = {"target": target, "selected": take, "available": len(available)}
    return selected, shortfalls, available_counts


def selection_sort_key(row: dict[str, Any]) -> tuple[Any, ...]:
    return (
        str(row.get("source_task_id") or ""),
        str(row.get("source_path") or ""),
        int(row.get("source_site_effective_offset") or row.get("source_site_offset") or 0),
        str(row.get("candidate_row_id") or ""),
    )


def expansion_selection_sort_key(row: dict[str, Any], prefer_pass_hypothesis: bool) -> tuple[Any, ...]:
    if not prefer_pass_hypothesis:
        return selection_sort_key(row)
    payload = ensure_pre_oracle_pass_hypothesis(row)
    return (
        0 if has_pre_oracle_pass_hypothesis(row) else 1,
        -float(payload.get("highest_confidence") or 0.0),
        str(payload.get("primary_kind") or ""),
        *selection_sort_key(row),
    )


def ensure_pre_oracle_pass_hypothesis(row: dict[str, Any]) -> dict[str, Any]:
    payload = row.get("pre_oracle_pass_hypothesis")
    if isinstance(payload, dict):
        return payload
    payload = detect_pre_oracle_pass_hypothesis(row)
    row["pre_oracle_pass_hypothesis"] = payload
    return payload


def has_pre_oracle_pass_hypothesis(row: dict[str, Any]) -> bool:
    payload = ensure_pre_oracle_pass_hypothesis(row)
    hypotheses = payload.get("hypotheses")
    return isinstance(hypotheses, list) and bool(hypotheses)


def detect_pre_oracle_pass_hypothesis(row: dict[str, Any]) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "policy_version": PASS_HYPOTHESIS_POLICY_VERSION,
        "analysis_status": "not_evaluated",
        "target_oracle_labels_used_as_features": False,
        "hypotheses": [],
    }
    source_file = Path(str(row.get("source_file") or ""))
    if not source_file.is_file() or has_mounted_drive_fragment(source_file):
        payload["analysis_status"] = "source_file_unavailable"
        payload["source_file"] = str(source_file) if str(source_file) else None
        return finalize_pass_hypothesis_payload(payload)

    try:
        source_text = source_file.read_text(encoding="utf-8")
        tree = ast.parse(source_text)
    except (OSError, UnicodeDecodeError, SyntaxError, ValueError) as exc:
        payload["analysis_status"] = "source_parse_unavailable"
        payload["source_file"] = str(source_file)
        payload["error"] = f"{type(exc).__name__}: {exc}"
        return finalize_pass_hypothesis_payload(payload)

    selected_line = selected_statement_start_line(row)
    if selected_line is None:
        payload["analysis_status"] = "selected_line_unavailable"
        payload["source_file"] = str(source_file)
        return finalize_pass_hypothesis_payload(payload)

    statement = find_statement_node(tree, selected_line)
    if statement is None:
        payload["analysis_status"] = "selected_statement_unavailable"
        payload["source_file"] = str(source_file)
        payload["selected_line"] = selected_line
        return finalize_pass_hypothesis_payload(payload)

    payload["analysis_status"] = "evaluated"
    payload["source_file"] = str(source_file)
    payload["selected_line"] = selected_line
    payload["selected_node_type"] = type(statement).__name__

    payload["hypotheses"].extend(tautological_assertion_hypotheses(statement))
    payload["hypotheses"].extend(trivial_pytest_raises_hypotheses(statement))
    redundant = redundant_assertion_hypothesis(tree, statement, selected_line)
    if redundant is not None:
        payload["hypotheses"].append(redundant)

    return finalize_pass_hypothesis_payload(payload)


def selected_statement_start_line(row: dict[str, Any]) -> int | None:
    statement = row.get("selected_statement")
    if isinstance(statement, dict) and isinstance(statement.get("start_line"), int):
        return int(statement["start_line"])
    return None


def finalize_pass_hypothesis_payload(payload: dict[str, Any]) -> dict[str, Any]:
    hypotheses = payload.get("hypotheses")
    if not isinstance(hypotheses, list):
        hypotheses = []
        payload["hypotheses"] = hypotheses
    hypotheses.sort(key=lambda item: (-float(item.get("confidence") or 0.0), str(item.get("kind") or "")))
    payload["hypothesis_count"] = len(hypotheses)
    payload["highest_confidence"] = float(hypotheses[0].get("confidence") or 0.0) if hypotheses else 0.0
    payload["primary_kind"] = str(hypotheses[0].get("kind") or "") if hypotheses else None
    return payload


def find_statement_node(tree: ast.AST, candidate_line: int) -> ast.stmt | None:
    candidates: list[ast.stmt] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.stmt):
            continue
        if not hasattr(node, "lineno") or not hasattr(node, "end_lineno"):
            continue
        end_line = int(getattr(node, "end_lineno", None) or node.lineno)
        if int(node.lineno) <= candidate_line <= end_line:
            candidates.append(node)
    if not candidates:
        return None
    candidates.sort(
        key=lambda node: (
            int(getattr(node, "end_lineno", None) or node.lineno) - int(node.lineno),
            int(getattr(node, "col_offset", 0)),
            -int(node.lineno),
        )
    )
    return candidates[0]


def tautological_assertion_hypotheses(statement: ast.stmt) -> list[dict[str, Any]]:
    hypotheses: list[dict[str, Any]] = []
    if isinstance(statement, ast.Assert):
        kind = tautology_kind_for_expr(statement.test)
        if kind is not None:
            hypotheses.append(
                {
                    "kind": "tautological_assertion",
                    "confidence": 0.9,
                    "evidence": {"tautology_kind": kind, "lineno": int(statement.lineno)},
                }
            )
    call = assertion_call(statement)
    if call is not None:
        method = assertion_call_method_name(call)
        args = list(call.args)
        tautology_kind: str | None = None
        if method in {"asserttrue", "assert_true"} and args:
            tautology_kind = tautology_kind_for_expr(args[0])
        elif method in {"assertfalse", "assert_false"} and args and falsehood_kind_for_expr(args[0]) is not None:
            tautology_kind = falsehood_kind_for_expr(args[0])
        elif method in {"assertequal", "assertequals", "assertis"} and len(args) >= 2 and expr_key(args[0]) == expr_key(args[1]):
            tautology_kind = "same_expression_arguments"
        if tautology_kind is not None:
            hypotheses.append(
                {
                    "kind": "tautological_assertion",
                    "confidence": 0.9,
                    "evidence": {"tautology_kind": tautology_kind, "method": method, "lineno": int(statement.lineno)},
                }
            )
    return hypotheses


def tautology_kind_for_expr(expr: ast.expr) -> str | None:
    if isinstance(expr, ast.Constant) and bool(expr.value) is True:
        return "truthy_constant"
    if isinstance(expr, ast.UnaryOp) and isinstance(expr.op, ast.Not):
        return falsehood_kind_for_expr(expr.operand)
    if isinstance(expr, ast.Compare) and len(expr.ops) == 1 and len(expr.comparators) == 1:
        left = expr.left
        right = expr.comparators[0]
        op = expr.ops[0]
        if isinstance(op, (ast.Eq, ast.Is)) and expr_key(left) == expr_key(right):
            return "same_expression_comparison"
        if isinstance(op, ast.GtE) and is_len_call(left) and is_zero_constant(right):
            return "len_nonnegative"
        if isinstance(op, ast.LtE) and is_zero_constant(left) and is_len_call(right):
            return "len_nonnegative"
    return None


def falsehood_kind_for_expr(expr: ast.expr) -> str | None:
    if isinstance(expr, ast.Constant) and bool(expr.value) is False:
        return "falsey_constant"
    if isinstance(expr, ast.Compare) and len(expr.ops) == 1 and len(expr.comparators) == 1:
        if isinstance(expr.ops[0], (ast.NotEq, ast.IsNot)) and expr_key(expr.left) == expr_key(expr.comparators[0]):
            return "same_expression_negative_comparison"
    return None


def trivial_pytest_raises_hypotheses(statement: ast.stmt) -> list[dict[str, Any]]:
    if not isinstance(statement, ast.With):
        return []
    raises_call = pytest_raises_call(statement)
    if raises_call is None or len(statement.body) != 1 or not isinstance(statement.body[0], ast.Raise):
        return []
    expected = expr_key(raises_call.args[0]) if raises_call.args else ""
    actual = raised_exception_key(statement.body[0])
    if expected and actual and expected == actual:
        return [
            {
                "kind": "trivial_pytest_raises_block",
                "confidence": 0.75,
                "evidence": {"exception_key": expected, "lineno": int(statement.lineno)},
            }
        ]
    return []


def pytest_raises_call(statement: ast.With) -> ast.Call | None:
    for item in statement.items:
        call = item.context_expr
        if isinstance(call, ast.Call) and call_name(call.func).endswith("pytest.raises"):
            return call
        if isinstance(call, ast.Call) and call_name(call.func) == "raises":
            return call
    return None


def raised_exception_key(node: ast.Raise) -> str:
    exc = node.exc
    if isinstance(exc, ast.Call):
        return expr_key(exc.func)
    if isinstance(exc, ast.expr):
        return expr_key(exc)
    return ""


def redundant_assertion_hypothesis(tree: ast.AST, statement: ast.stmt, selected_line: int) -> dict[str, Any] | None:
    keys = assertion_semantic_keys(statement)
    if not keys:
        return None
    scope = enclosing_scope(tree, selected_line)
    matches: list[int] = []
    for other in iter_scope_assertion_statements(scope):
        if assertion_semantic_keys(other) & keys:
            matches.append(int(other.lineno))
    if len(set(matches)) < 2:
        return None
    return {
        "kind": "redundant_assertion_same_scope",
        "confidence": 0.65,
        "evidence": {
            "scope_name": scope_name(scope),
            "scope_node_type": type(scope).__name__,
            "matching_assertion_count": len(set(matches)),
            "matching_lines": sorted(set(matches))[:20],
            "semantic_keys": sorted(keys)[:10],
        },
    }


def enclosing_scope(tree: ast.AST, line: int) -> ast.AST:
    scopes: list[ast.AST] = [tree]
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            continue
        end_line = int(getattr(node, "end_lineno", None) or node.lineno)
        if int(node.lineno) <= line <= end_line:
            scopes.append(node)
    scopes.sort(key=lambda node: scope_span(node))
    return scopes[0]


def scope_span(node: ast.AST) -> int:
    if isinstance(node, ast.Module):
        return 10**9
    return int(getattr(node, "end_lineno", 0) or 0) - int(getattr(node, "lineno", 0) or 0)


def scope_name(node: ast.AST) -> str:
    if isinstance(node, ast.Module):
        return "<module>"
    return str(getattr(node, "name", type(node).__name__))


def iter_scope_assertion_statements(scope: ast.AST) -> Iterable[ast.stmt]:
    def walk(node: ast.AST, *, root: bool = False) -> Iterable[ast.stmt]:
        if not root and isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            return
        if isinstance(node, (ast.Assert, ast.Expr, ast.With)) and assertion_semantic_keys(node):
            yield node
        for child in ast.iter_child_nodes(node):
            yield from walk(child)

    yield from walk(scope, root=True)


def assertion_semantic_keys(statement: ast.stmt) -> set[str]:
    if isinstance(statement, ast.Assert):
        return expression_assertion_keys(statement.test)
    call = assertion_call(statement)
    if call is None:
        if isinstance(statement, ast.With):
            raises = pytest_raises_call(statement)
            if raises is not None and raises.args:
                return {f"pytest_raises:{expr_key(raises.args[0])}"}
        return set()
    method = assertion_call_method_name(call)
    args = list(call.args)
    if method in {"assertequal", "assertequals"} and len(args) >= 2:
        return {commutative_key("equals", args[0], args[1])}
    if method == "assertis" and len(args) >= 2:
        return {commutative_key("is", args[0], args[1])}
    if method == "assertisinstance" and len(args) >= 2:
        return {f"isinstance:{expr_key(args[0])}:{expr_key(args[1])}"}
    if method in {"asserttrue", "assert_true"} and args:
        return {f"truthy:{expr_key(args[0])}"}
    if method in {"assertfalse", "assert_false"} and args:
        return {f"falsey:{expr_key(args[0])}"}
    return set()


def expression_assertion_keys(expr: ast.expr) -> set[str]:
    if isinstance(expr, ast.Compare) and len(expr.ops) == 1 and len(expr.comparators) == 1:
        op = expr.ops[0]
        right = expr.comparators[0]
        if isinstance(op, ast.Eq):
            return {commutative_key("equals", expr.left, right)}
        if isinstance(op, ast.Is):
            return {commutative_key("is", expr.left, right)}
    if isinstance(expr, ast.Call) and call_name(expr.func) == "isinstance" and len(expr.args) >= 2:
        return {f"isinstance:{expr_key(expr.args[0])}:{expr_key(expr.args[1])}"}
    if isinstance(expr, ast.UnaryOp) and isinstance(expr.op, ast.Not):
        return {f"falsey:{expr_key(expr.operand)}"}
    return {f"truthy:{expr_key(expr)}"}


def assertion_call(statement: ast.stmt) -> ast.Call | None:
    if not isinstance(statement, ast.Expr) or not isinstance(statement.value, ast.Call):
        return None
    name = call_name(statement.value.func)
    if ".assert" in name or name.startswith("assert"):
        return statement.value
    return None


def assertion_call_method_name(call: ast.Call) -> str:
    return call_name(call.func).rsplit(".", 1)[-1].lower()


def call_name(expr: ast.expr) -> str:
    if isinstance(expr, ast.Name):
        return expr.id
    if isinstance(expr, ast.Attribute):
        prefix = call_name(expr.value)
        return f"{prefix}.{expr.attr}" if prefix else expr.attr
    return ""


def commutative_key(prefix: str, left: ast.expr, right: ast.expr) -> str:
    first, second = sorted((expr_key(left), expr_key(right)))
    return f"{prefix}:{first}:{second}"


def expr_key(expr: ast.AST) -> str:
    return ast.dump(expr, annotate_fields=True, include_attributes=False)


def is_zero_constant(expr: ast.expr) -> bool:
    return isinstance(expr, ast.Constant) and expr.value == 0


def is_len_call(expr: ast.expr) -> bool:
    return isinstance(expr, ast.Call) and call_name(expr.func) == "len" and len(expr.args) == 1


def annotate_selected_rows(selected: list[dict[str, Any]], *, task_id: str = TASK_ID) -> list[dict[str, Any]]:
    ranks: Counter[tuple[str, str]] = Counter()
    out: list[dict[str, Any]] = []
    for ordinal, row in enumerate(selected):
        key = (str(row["stable_partition"]), str(row["assertion_duty_sublabel"]))
        ranks[key] += 1
        out.append(
            {
                "schema_version": 1,
                "task_id": task_id,
                "artifact_kind": "delete_test_call_candidate_expansion_selected_row",
                "selection_ordinal": ordinal,
                "selection_rank_in_group": ranks[key],
                "selection_group": "|".join(key),
                "candidate_row_id": row["candidate_row_id"],
                "source_task_id": row["source_task_id"],
                "repo": row.get("repo"),
                "stable_partition": row["stable_partition"],
                "mutation_category": row.get("mutation_category"),
                "assertion_duty_sublabel": row["assertion_duty_sublabel"],
                "assertion_kind": row.get("assertion_kind"),
                "file_kind": row.get("file_kind"),
                "source_path": row["source_path"],
                "source_file": row.get("source_file"),
                "source_file_sha256": row.get("source_file_sha256"),
                "source_site_effective_offset": row.get("source_site_effective_offset"),
                "source_site_offset": row.get("source_site_offset"),
                "patch_path": row["patch_path"],
                "patch_sha256": row.get("patch_sha256"),
                "selection_policy": DEFAULT_SELECTION_POLICY,
                "pre_oracle_pass_hypothesis": row.get("pre_oracle_pass_hypothesis"),
                "pre_oracle_pass_hypothesis_used_for_selection": has_pre_oracle_pass_hypothesis(row),
                "holdout_tuning_used": False,
                "target_oracle_label_used": False,
                "target_oracle_labels_used_as_features": False,
            }
        )
    return out


def build_batch_rows(
    rows: list[dict[str, Any]],
    *,
    batch_key_prefix: str = DEFAULT_BATCH_KEY_PREFIX,
    selection_policy: str = "TASK-PY-G-135E fixed split/sublabel expansion",
) -> list[dict[str, Any]]:
    batches: list[list[dict[str, Any]]] = []
    batch_tasks: list[set[str]] = []
    for row in rows:
        batch_index = next((index for index, tasks in enumerate(batch_tasks) if row["source_task_id"] not in tasks), None)
        if batch_index is None:
            batch_index = len(batches)
            batches.append([])
            batch_tasks.append(set())
        batches[batch_index].append(row)
        batch_tasks[batch_index].add(row["source_task_id"])
        row["expansion_batch_index"] = batch_index
        row["expansion_batch_key"] = f"{batch_key_prefix}-{batch_index:04d}"

    return [
        {
            "schema_version": 1,
            "artifact_kind": "delete_test_call_oracle_batch",
            "batch_index": index,
            "batch_key": f"{batch_key_prefix}-{index:04d}",
            "prediction_count": len(batch),
            "source_task_ids": [row["source_task_id"] for row in batch],
            "candidate_row_ids": [row["candidate_row_id"] for row in batch],
            "patch_paths": [row["patch_path"] for row in batch],
            "selection_policy": selection_policy,
        }
        for index, batch in enumerate(batches)
    ]


def validate_selection(
    selected_rows: list[dict[str, Any]],
    batch_rows: list[dict[str, Any]],
    shortfalls: dict[str, dict[str, int]],
    *,
    require_pass_hypothesis: bool = False,
) -> list[str]:
    failures: list[str] = []
    candidate_ids = [row["candidate_row_id"] for row in selected_rows]
    if not selected_rows:
        failures.append("DTC_EXPANSION_SELECTED_ROWS_EMPTY")
    if len(set(candidate_ids)) != len(candidate_ids):
        failures.append("DTC_EXPANSION_DUPLICATE_SELECTED_CANDIDATE")
    if shortfalls:
        failures.append("DTC_EXPANSION_TARGET_SHORTFALL")
    if duplicate_task_batch_count(batch_rows):
        failures.append("DTC_EXPANSION_BATCH_DUPLICATE_TASK")
    if require_pass_hypothesis and any(not has_pre_oracle_pass_hypothesis(row) for row in selected_rows):
        failures.append("DTC_EXPANSION_SELECTED_ROW_MISSING_PASS_HYPOTHESIS")
    for row in selected_rows:
        for field in ("source_file", "patch_path"):
            value = row.get(field)
            if value and has_mounted_drive_fragment(Path(str(value))):
                failures.append("DTC_EXPANSION_MOUNTED_SELECTED_PATH")
                break
    return sorted(set(failures))


def read_json(path: Path, code: str) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise CandidateExpansionError(code, f"invalid JSON: {path}", {"path": str(path), "error": str(exc)}) from exc
    if not isinstance(payload, dict):
        raise CandidateExpansionError(code, f"JSON root is not an object: {path}", {"path": str(path)})
    return payload


def read_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise CandidateExpansionError(
                    code,
                    f"invalid JSONL at {path}:{line_number}",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            if not isinstance(row, dict):
                raise CandidateExpansionError(
                    code,
                    f"JSONL row is not an object at {path}:{line_number}",
                    {"path": str(path), "line_number": line_number},
                )
            rows.append(row)
    return rows


def require_string(row: dict[str, Any], key: str, path: Path, line_number: int) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise CandidateExpansionError(
            "DTC_EXPANSION_REQUIRED_FIELD_MISSING",
            f"required string field missing: {key}",
            {"path": str(path), "line_number": line_number, "key": key},
        )
    return value


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise CandidateExpansionError(
            "DTC_EXPANSION_TEXT_READBACK_MISMATCH",
            f"text readback mismatch at {path}",
            {"path": str(path)},
        )


def write_jsonl_checked(path: Path, rows: Iterable[dict[str, Any]]) -> None:
    write_text_checked(
        path,
        "".join(json.dumps(row, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n" for row in rows),
    )


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"exists": False, "path": str(path)}
    if path.is_file():
        return {"exists": True, "path": str(path), "kind": "file", "size_bytes": path.stat().st_size}
    files = [child for child in path.rglob("*") if child.is_file()]
    return {
        "exists": True,
        "path": str(path),
        "kind": "dir",
        "file_count": len(files),
        "total_size_bytes": sum(child.stat().st_size for child in files),
    }


def count_lines(path: Path) -> int:
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for _ in handle)


def duplicate_task_batch_count(batch_rows: list[dict[str, Any]]) -> int:
    count = 0
    for row in batch_rows:
        task_ids = list(row.get("source_task_ids") or [])
        if len(task_ids) != len(set(task_ids)):
            count += 1
    return count


def mounted_drive_hit_count(paths: list[Path]) -> int:
    count = 0
    for path in paths:
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        count += text.count("/mnt/c") + text.count("/mnt/d")
    return count


def target_map(counter: Counter[tuple[str, str]] | dict[tuple[str, str], int]) -> dict[str, int]:
    return {"|".join(key): int(value) for key, value in sorted(counter.items())}


def pass_hypothesis_kind_counts(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts: Counter[str] = Counter()
    for row in rows:
        payload = row.get("pre_oracle_pass_hypothesis")
        if not isinstance(payload, dict):
            continue
        for hypothesis in payload.get("hypotheses") or []:
            if isinstance(hypothesis, dict) and hypothesis.get("kind"):
                counts[str(hypothesis["kind"])] += 1
    return dict(sorted(counts.items()))
