"""CLI bridge for #786 Mondrian per-cell conformal evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .sc_cp_metrics import build_mondrian_conformal_report


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            "MONDRIAN_CONFORMAL_BAD_INPUT",
            f"{label} must be a JSON object",
            "Pass a ship-gate report whose execute_and_inspect is an object.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def _require_array(value: Any, label: str) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        raise SwebenchOpsError(
            "MONDRIAN_CONFORMAL_BAD_INPUT",
            f"{label} must be a JSON array",
            "Pass a ship-gate report with per-row prediction arrays.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def _raw_prediction_rows(
    execute: dict[str, Any], split_label: str
) -> tuple[str, list[dict[str, Any]]]:
    for key in (
        f"{split_label}_raw_predictions",
        f"{split_label}_predictions",
        f"{split_label}_rank_lift_predictions",
    ):
        if key in execute and execute.get(key) is not None:
            return key, _require_array(execute.get(key), key)
    raise SwebenchOpsError(
        "MONDRIAN_CONFORMAL_BAD_INPUT",
        f"{split_label} raw prediction rows are missing",
        "Pass a ship-gate report with raw per-row prediction arrays.",
        {"split": split_label},
    )


def build_report(
    report_path: Path,
    *,
    alpha: float = 0.10,
    min_cell_rows: int = 20,
    in_band_cell_fraction: float = 0.80,
) -> dict[str, Any]:
    payload = json.loads(report_path.read_text(encoding="utf-8"))
    execute = _require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    calibration_source, calibration_rows = _raw_prediction_rows(execute, "calibration")
    holdout_source, holdout_rows = _raw_prediction_rows(execute, "holdout")
    mondrian = build_mondrian_conformal_report(
        calibration_rows,
        holdout_rows,
        alpha=alpha,
        min_cell_rows=min_cell_rows,
        in_band_cell_fraction=in_band_cell_fraction,
        source_label=str(report_path),
    )
    return {
        "schema_version": 1,
        "artifact_kind": "inv_019_mondrian_conformal_ship_gate_report",
        "created_at_utc": utc_now(),
        "ship_gate_report_path": str(report_path),
        "ship_gate_report_sha256": sha256_file(report_path),
        "raw_prediction_sources": {
            "calibration": calibration_source,
            "holdout": holdout_source,
        },
        "method_sources": {
            "mondrian_conformal_prediction": "https://github.com/henrikbostrom/crepes",
            "conformal_prediction_library_reference": "https://github.com/deel-ai/puncc",
        },
        "doctrine_policy": {
            "no_holdout_tuning": True,
            "no_target_labels_as_live_inputs": True,
            "no_inner_llm": True,
            "no_panel_or_slot_mutation": True,
            "aiwonder_only_for_production_evidence": str(report_path).startswith(
                "/zfs/archive/contextgraph/"
            ),
        },
        "mondrian_conformal": mondrian,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compute #786 Mondrian per-cell conformal evidence from a ship-gate FSV report"
    )
    parser.add_argument("--ship-gate-report", type=Path, required=True)
    parser.add_argument("--output-path", type=Path, required=True)
    parser.add_argument("--alpha", type=float, default=0.10)
    parser.add_argument("--min-cell-rows", type=int, default=20)
    parser.add_argument("--in-band-cell-fraction", type=float, default=0.80)
    args = parser.parse_args(argv)
    report = build_report(
        args.ship_gate_report,
        alpha=float(args.alpha),
        min_cell_rows=int(args.min_cell_rows),
        in_band_cell_fraction=float(args.in_band_cell_fraction),
    )
    write_json_checked(args.output_path, report)
    print(json.dumps({"ok": True, "wrote": str(args.output_path)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
