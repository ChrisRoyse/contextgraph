from __future__ import annotations

import hashlib
import json
import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now


DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/readback/swebench-300x8-ship-gate-readback/"
    "run-20260521T132308Z/ship-gate-readback.json"
)
DEFAULT_PANEL_B_SCORE_ROWS_ROOT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-111d-panel-b-score-rows-readback"
)
DEFAULT_PANEL_B_SCORE_ROWS_OUT = DEFAULT_PANEL_B_SCORE_ROWS_ROOT / "panel_b_score_rows.jsonl"
REQUIRED_ROW_COUNT = 240
HEX_32 = re.compile(r"^[0-9a-fA-F]{32}$")
HEX_64 = re.compile(r"^(?:sha256:)?[0-9a-fA-F]{64}$")
TARGET_SIDE_ID_MARKERS = (
    "oracle",
    "docker",
    "test_outcome",
    "target_side",
    "ground_truth",
)


@dataclass(frozen=True)
class ShipGateRow:
    row_index: int
    task_id: str
    repo: str
    category: str
    patch_path: str
    patch_sha256: str
    actual_oracle_pass: bool
    score_a: float
    prediction_id_hex: str


@dataclass(frozen=True)
class PanelBScoreInput:
    row_index: int
    task_id: str
    category: str
    patch_path: str
    patch_sha256: str
    prediction_id_hex: str | None
    score_b: float
    accepted_label_ids: tuple[str, ...]
    failure_evidence_set_ids: tuple[str, ...]
    panel_b_artifact_set_sha256: str


def add_arguments(parser: Any) -> None:
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--panel-b-scores-jsonl", type=Path, required=True)
    parser.add_argument("--out-jsonl", type=Path, default=DEFAULT_PANEL_B_SCORE_ROWS_OUT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_PANEL_B_SCORE_ROWS_ROOT)
    parser.add_argument("--expected-row-count", type=int, default=REQUIRED_ROW_COUNT)


def command_panel_b_score_rows(args: Any) -> dict[str, Any]:
    ship_gate_report = args.ship_gate_report.expanduser().resolve(strict=False)
    panel_b_scores_jsonl = args.panel_b_scores_jsonl.expanduser().resolve(strict=False)
    out_jsonl = args.out_jsonl.expanduser().resolve(strict=False)
    validate_not_mounted_drive("ship_gate_report", ship_gate_report)
    validate_not_mounted_drive("panel_b_scores_jsonl", panel_b_scores_jsonl)
    validate_not_mounted_drive("out_jsonl", out_jsonl)
    if args.expected_row_count <= 1:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_EXPECTED_COUNT_INVALID",
            "--expected-row-count must be greater than 1",
            "Use the current gate row count, normally 240 holdout rows.",
            {"expected_row_count": args.expected_row_count},
        )

    ship_rows = load_ship_gate_rows(ship_gate_report, args.expected_row_count)
    score_inputs = load_panel_b_score_inputs(panel_b_scores_jsonl, args.expected_row_count)
    score_by_key = index_panel_b_score_inputs(score_inputs)

    output_rows: list[dict[str, Any]] = []
    missing_rows: list[dict[str, str]] = []
    artifact_set_hashes: set[str] = set()
    for row in ship_rows:
        key = row_key(row.task_id, row.category, row.patch_path, row.patch_sha256)
        score = score_by_key.get(key)
        if score is None:
            missing_rows.append(
                {
                    "taskId": row.task_id,
                    "category": row.category,
                    "patchPath": row.patch_path,
                    "patchSha256": row.patch_sha256,
                }
            )
            continue
        if score.prediction_id_hex is not None and score.prediction_id_hex.lower() != row.prediction_id_hex:
            raise SwebenchOpsError(
                "PANEL_B_SCORE_ROWS_PREDICTION_ID_MISMATCH",
                "Panel B score row predictionIdHex does not match the ship-gate row identity",
                "Regenerate Panel B scores from the same ship-gate report and row identity function.",
                {
                    "task_id": row.task_id,
                    "category": row.category,
                    "expected": row.prediction_id_hex,
                    "actual": score.prediction_id_hex,
                },
            )
        artifact_set_hashes.add(score.panel_b_artifact_set_sha256)
        output_rows.append(
            {
                "predictionIdHex": row.prediction_id_hex,
                "scoreA": row.score_a,
                "scoreB": score.score_b,
                "oraclePass": row.actual_oracle_pass,
                "acceptedLabelIds": list(score.accepted_label_ids),
                "failureEvidenceSetIds": list(score.failure_evidence_set_ids),
            }
        )
    if missing_rows:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_MISSING_ROWS",
            "Panel B scores are missing ship-gate rows",
            "Score every current holdout row before emitting CG_PANEL_B_SCORE_ROWS_JSONL.",
            {"missing_count": len(missing_rows), "missing_rows": missing_rows[:20]},
        )
    extra_keys = sorted(set(score_by_key).difference(
        row_key(row.task_id, row.category, row.patch_path, row.patch_sha256) for row in ship_rows
    ))
    if extra_keys:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_EXTRA_ROWS",
            "Panel B scores contain rows that are not present in the ship-gate surface",
            "Regenerate scores from the current ship-gate report only.",
            {"extra_count": len(extra_keys), "extra_row_keys": extra_keys[:20]},
        )
    if len(artifact_set_hashes) != 1:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_ARTIFACT_SET_MISMATCH",
            "Panel B score rows do not share one artifact-set hash",
            "Use one pinned materialized Panel B artifact set for the full scorer run.",
            {"artifact_set_hashes": sorted(artifact_set_hashes)},
        )

    write_jsonl_checked(out_jsonl, output_rows)
    return {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_111d_panel_b_score_rows",
        "githubIssue": 572,
        "ok": True,
        "shipGateReport": str(ship_gate_report),
        "panelBScoresJsonl": str(panel_b_scores_jsonl),
        "outJsonl": str(out_jsonl),
        "rowCount": len(output_rows),
        "expectedRowCount": args.expected_row_count,
        "rowAlignmentCount": len(output_rows),
        "panelBArtifactSetSha256": next(iter(artifact_set_hashes)),
        "outputSha256": sha256_file(out_jsonl),
        "shipGateReportSha256": sha256_file(ship_gate_report),
        "panelBScoresSha256": sha256_file(panel_b_scores_jsonl),
        "contract": {
            "outputEnv": "CG_PANEL_B_SCORE_ROWS_JSONL",
            "scoreA": "ship-gate holdout predicted_oracle_pass",
            "scoreB": "model-backed Panel B per-row score",
            "predictionIdHex": "sha256 first 16 bytes over task/category/patch identity",
            "modelBackedPanelBRequired": True,
            "mountedDriveReadsOrWrites": False,
        },
    }


def load_ship_gate_rows(path: Path, expected_row_count: int) -> list[ShipGateRow]:
    payload = read_json_object(path, "PANEL_B_SCORE_ROWS_SHIP_GATE_REPORT")
    rows = pointer_array(payload, ["execute_and_inspect", "holdout_predictions"], "ship-gate holdout_predictions")
    if len(rows) != expected_row_count:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_SHIP_GATE_COUNT_MISMATCH",
            "ship-gate holdout row count does not match expected count",
            "Use the current 240-row holdout ship-gate READBACK report or pass the correct expected count.",
            {"expected": expected_row_count, "actual": len(rows), "path": str(path)},
        )
    out: list[ShipGateRow] = []
    seen: set[str] = set()
    for index, value in enumerate(rows):
        row = ShipGateRow(
            row_index=index,
            task_id=required_string(value, "task_id", "taskId"),
            repo=required_string(value, "repo"),
            category=required_string(value, "category"),
            patch_path=required_string(value, "patch_path", "patchPath"),
            patch_sha256=required_string(value, "patch_sha256", "patchSha256"),
            actual_oracle_pass=required_bool(value, "actual_oracle_pass", "actualOraclePass"),
            score_a=required_score(value, "predicted_oracle_pass", "predictedOraclePass"),
            prediction_id_hex="",
        )
        prediction_id = prediction_id_hex(row.task_id, row.category, row.patch_path, row.patch_sha256)
        row = ShipGateRow(**{**row.__dict__, "prediction_id_hex": prediction_id})
        key = row_key(row.task_id, row.category, row.patch_path, row.patch_sha256)
        if key in seen:
            raise SwebenchOpsError(
                "PANEL_B_SCORE_ROWS_DUPLICATE_SHIP_GATE_ROW",
                "ship-gate report contains duplicate row identity",
                "Fix the ship-gate report before building Panel B rows.",
                {"row_key": key, "row_index": index},
            )
        seen.add(key)
        out.append(row)
    return out


def load_panel_b_score_inputs(path: Path, expected_row_count: int) -> list[PanelBScoreInput]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_INPUT_MISSING",
            f"Panel B scores JSONL is missing: {path}",
            "Run the real model-backed Panel B scorer and pass its JSONL output.",
            {"path": str(path)},
        )
    out: list[PanelBScoreInput] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            stripped = line.strip()
            if not stripped:
                continue
            try:
                value = json.loads(stripped)
            except json.JSONDecodeError as exc:
                raise SwebenchOpsError(
                    "PANEL_B_SCORE_ROWS_BAD_JSONL",
                    f"Panel B scores JSONL line {line_number} is not valid JSON",
                    "Write one JSON object per row from the real Panel B scorer.",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            if not isinstance(value, dict):
                raise SwebenchOpsError(
                    "PANEL_B_SCORE_ROWS_NON_OBJECT_ROW",
                    f"Panel B scores JSONL line {line_number} is not an object",
                    "Write one JSON object per Panel B score row.",
                    {"path": str(path), "line_number": line_number},
                )
            out.append(panel_b_score_input_from_json(value, line_number))
    if len(out) != expected_row_count:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_INPUT_COUNT_MISMATCH",
            "Panel B score row count does not match expected count",
            "Score exactly the current ship-gate holdout rows.",
            {"expected": expected_row_count, "actual": len(out), "path": str(path)},
        )
    return out


def panel_b_score_input_from_json(value: dict[str, Any], line_number: int) -> PanelBScoreInput:
    if value.get("modelBackedPanelB") is not True and value.get("model_backed_panel_b") is not True:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_NOT_MODEL_BACKED",
            "Panel B score row is not marked model-backed",
            "Only rows produced from the local pinned Panel B artifacts may enter CG_PANEL_B_SCORE_ROWS_JSONL.",
            {"line_number": line_number},
        )
    prediction_id = optional_string(value, "predictionIdHex", "prediction_id_hex")
    if prediction_id is not None:
        validate_prediction_id(prediction_id, line_number)
        prediction_id = prediction_id.lower()
    artifact_set_hash = normalize_artifact_hash(
        required_string(value, "panelBArtifactSetSha256", "panel_b_artifact_set_sha256"),
        line_number,
    )
    return PanelBScoreInput(
        row_index=line_number - 1,
        task_id=required_string(value, "taskId", "task_id"),
        category=required_string(value, "category"),
        patch_path=required_string(value, "patchPath", "patch_path"),
        patch_sha256=required_string(value, "patchSha256", "patch_sha256"),
        prediction_id_hex=prediction_id,
        score_b=required_score(value, "scoreB", "score_b"),
        accepted_label_ids=required_id_set(value, line_number, "acceptedLabelIds", "accepted_label_ids"),
        failure_evidence_set_ids=required_id_set(
            value,
            line_number,
            "failureEvidenceSetIds",
            "failure_evidence_set_ids",
        ),
        panel_b_artifact_set_sha256=artifact_set_hash,
    )


def index_panel_b_score_inputs(rows: list[PanelBScoreInput]) -> dict[str, PanelBScoreInput]:
    out: dict[str, PanelBScoreInput] = {}
    for row in rows:
        key = row_key(row.task_id, row.category, row.patch_path, row.patch_sha256)
        if key in out:
            raise SwebenchOpsError(
                "PANEL_B_SCORE_ROWS_DUPLICATE_SCORE_ROW",
                "Panel B score rows contain duplicate row identity",
                "Regenerate scores so each ship-gate row appears exactly once.",
                {"row_key": key, "first_row_index": out[key].row_index, "duplicate_row_index": row.row_index},
            )
        out[key] = row
    return out


def row_key(task_id: str, category: str, patch_path: str, patch_sha256: str) -> str:
    return f"{task_id}\0{category}\0{patch_path}\0{patch_sha256}"


def prediction_id_hex(task_id: str, category: str, patch_path: str, patch_sha256: str) -> str:
    digest = hashlib.sha256()
    digest.update(b"panel-b-score-row-v1\0")
    digest.update(task_id.encode("utf-8"))
    digest.update(b"\0")
    digest.update(category.encode("utf-8"))
    digest.update(b"\0")
    digest.update(patch_path.encode("utf-8"))
    digest.update(b"\0")
    digest.update(patch_sha256.encode("utf-8"))
    return digest.hexdigest()[:32]


def read_json_object(path: Path, code: str) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            f"{code}_MISSING",
            f"required JSON file missing: {path}",
            "Use the current aiwonder READBACK artifact or a local smoke fixture.",
            {"path": str(path)},
        )
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            f"{code}_BAD_JSON",
            f"required JSON file is not valid JSON: {path}",
            "Regenerate the artifact before consuming it.",
            {"path": str(path), "error": str(exc)},
        ) from exc
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            f"{code}_NOT_OBJECT",
            f"required JSON file must contain an object: {path}",
            "Regenerate the artifact before consuming it.",
            {"path": str(path)},
        )
    return value


def pointer_array(value: dict[str, Any], keys: list[str], name: str) -> list[Any]:
    current: Any = value
    for key in keys:
        if not isinstance(current, dict) or key not in current:
            raise SwebenchOpsError(
                "PANEL_B_SCORE_ROWS_REPORT_POINTER_MISSING",
                f"missing {name}",
                "Use a current ship-gate READBACK report with holdout_predictions.",
                {"path": "/" + "/".join(keys)},
            )
        current = current[key]
    if not isinstance(current, list):
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_REPORT_POINTER_NOT_ARRAY",
            f"{name} is not an array",
            "Use a current ship-gate READBACK report with holdout_predictions.",
            {"path": "/" + "/".join(keys), "type": type(current).__name__},
        )
    return current


def required_string(value: dict[str, Any], *keys: str) -> str:
    raw = first_present(value, *keys)
    if not isinstance(raw, str) or not raw.strip():
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_STRING_FIELD_INVALID",
            f"required string field missing or empty: {keys[0]}",
            "Emit the required identity fields for every row.",
            {"keys": list(keys), "value": raw},
        )
    return raw.strip()


def optional_string(value: dict[str, Any], *keys: str) -> str | None:
    raw = first_present(value, *keys, required=False)
    if raw is None:
        return None
    if not isinstance(raw, str) or not raw.strip():
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_OPTIONAL_STRING_FIELD_INVALID",
            f"optional string field is present but invalid: {keys[0]}",
            "Omit the field or emit a non-empty string.",
            {"keys": list(keys), "value": raw},
        )
    return raw.strip()


def required_bool(value: dict[str, Any], *keys: str) -> bool:
    raw = first_present(value, *keys)
    if not isinstance(raw, bool):
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_BOOL_FIELD_INVALID",
            f"required boolean field missing or invalid: {keys[0]}",
            "Use a current ship-gate report with boolean oracle labels.",
            {"keys": list(keys), "value": raw},
        )
    return raw


def required_score(value: dict[str, Any], *keys: str) -> float:
    raw = first_present(value, *keys)
    if isinstance(raw, bool) or not isinstance(raw, (int, float)):
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_SCORE_FIELD_INVALID",
            f"score field missing or non-numeric: {keys[0]}",
            "Emit finite numeric scores in [0, 1].",
            {"keys": list(keys), "value": raw},
        )
    score = float(raw)
    if not (score == score and 0.0 <= score <= 1.0 and score not in (float("inf"), float("-inf"))):
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_SCORE_OUT_OF_RANGE",
            f"score field must be finite and in [0, 1]: {keys[0]}",
            "Fix the scorer calibration before emitting cross-panel rows.",
            {"keys": list(keys), "value": raw},
        )
    return score


def required_id_set(value: dict[str, Any], line_number: int, *keys: str) -> tuple[str, ...]:
    raw = first_present(value, *keys)
    if not isinstance(raw, list) or not raw:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_ID_SET_INVALID",
            f"required non-empty id list missing or invalid: {keys[0]}",
            "Emit live-safe accepted labels and failure evidence IDs for every row.",
            {"line_number": line_number, "keys": list(keys), "value": raw},
        )
    out: list[str] = []
    seen: set[str] = set()
    for item in raw:
        if not isinstance(item, str) or not item.strip():
            raise SwebenchOpsError(
                "PANEL_B_SCORE_ROWS_ID_INVALID",
                f"id list {keys[0]} contains a non-string or empty id",
                "Emit non-empty string IDs only.",
                {"line_number": line_number, "value": item},
            )
        item = item.strip()
        lowered = item.lower()
        if any(marker in lowered for marker in TARGET_SIDE_ID_MARKERS):
            raise SwebenchOpsError(
                "PANEL_B_SCORE_ROWS_TARGET_SIDE_ID_FORBIDDEN",
                f"id list {keys[0]} contains target-side label evidence",
                "Use live-safe accepted labels and failure evidence IDs; oracle truth remains only oraclePass.",
                {"line_number": line_number, "id": item, "markers": list(TARGET_SIDE_ID_MARKERS)},
            )
        if item in seen:
            raise SwebenchOpsError(
                "PANEL_B_SCORE_ROWS_DUPLICATE_ID",
                f"id list {keys[0]} contains a duplicate id",
                "Deduplicate row label/evidence IDs before emitting scorer output.",
                {"line_number": line_number, "id": item},
            )
        seen.add(item)
        out.append(item)
    return tuple(out)


def first_present(value: dict[str, Any], *keys: str, required: bool = True) -> Any:
    for key in keys:
        if key in value:
            return value[key]
    if required:
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_FIELD_MISSING",
            f"required field missing: {keys[0]}",
            "Emit the required field for every row.",
            {"keys": list(keys)},
        )
    return None


def validate_prediction_id(value: str, line_number: int) -> None:
    if not HEX_32.fullmatch(value):
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_PREDICTION_ID_INVALID",
            "predictionIdHex must be exactly 32 hexadecimal characters",
            "Use the command's stable row identity function or omit predictionIdHex.",
            {"line_number": line_number, "predictionIdHex": value},
        )


def normalize_artifact_hash(value: str, line_number: int) -> str:
    if not HEX_64.fullmatch(value):
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_ARTIFACT_HASH_INVALID",
            "panelBArtifactSetSha256 must be a SHA256 hex digest",
            "Hash the materialized Panel B artifact manifest and include it on every row.",
            {"line_number": line_number, "panelBArtifactSetSha256": value},
        )
    return value.lower().removeprefix("sha256:")


def validate_not_mounted_drive(field: str, path: Path) -> None:
    if str(path).startswith("/mnt/d/"):
        raise SwebenchOpsError(
            "PANEL_B_SCORE_ROWS_MNT_D_FORBIDDEN",
            f"{field} cannot point at mounted D drive: {path}",
            "Use aiwonder /zfs roots for production or /tmp for local smoke fixtures.",
            {"field": field, "path": str(path)},
        )


def write_jsonl_checked(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(f"{path.name}.part-{os.getpid()}")
    with temp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    temp.replace(path)
