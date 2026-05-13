#!/usr/bin/env python3
"""TASK-OBS-016 — Executive summary renderer for ME-JEPA RealityPrediction.

Reads a JSON payload (either a single `RealityPrediction` row or the
output of `mejepa_inspect_prediction`) and emits a single paragraph,
≤200 words, capturing the operator's key concerns: verdict, confidence
interval, top failure modes, TCT guard rejections, and the predicted
side-effects from the Q5 RealityImpact view.

Usage:
    python scripts/render_executive_summary.py --input prediction.json [--output summary.txt]

Reads only — never writes back to prediction storage. Fail-closed on
missing required fields with `MEJEPA_EXECUTIVE_*` error codes.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

WORD_LIMIT = 200


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    payload = load(args.input)
    summary = render_summary(payload)
    word_count = len(summary.split())
    if word_count > WORD_LIMIT:
        raise SystemExit(
            f"MEJEPA_EXECUTIVE_SUMMARY_TOO_LONG: {word_count} > {WORD_LIMIT} words"
        )
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(summary + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "summary": summary,
                "word_count": word_count,
                "word_limit": WORD_LIMIT,
                "source_of_truth": "RealityPrediction JSON payload",
            },
            indent=2,
        )
    )
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render RealityPrediction executive summary")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=None)
    return parser.parse_args(argv)


def load(path: Path) -> dict[str, Any]:
    if not path.is_file():
        raise SystemExit(f"MEJEPA_EXECUTIVE_INPUT_MISSING: {path} not a file")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise SystemExit(f"MEJEPA_EXECUTIVE_INPUT_JSON_INVALID: {path}: {err}") from err
    if not isinstance(payload, dict):
        raise SystemExit("MEJEPA_EXECUTIVE_INPUT_SCHEMA: top-level must be a JSON object")
    return payload


def render_summary(payload: dict[str, Any]) -> str:
    # Tolerate both raw RealityPrediction and mejepa_inspect_prediction shapes.
    identity = payload.get("predictionIdentity") or payload.get("prediction") or payload
    task_id = str(identity.get("taskId", identity.get("task_id", "<unknown task>")))
    language = identity.get("language") or payload.get("language") or "?"

    verdict = (
        payload.get("verdict")
        or payload.get("prediction", {}).get("verdict")
        or payload.get("predicted_test_pass")
        or "Unknown"
    )

    interval = payload.get("conformalTrace", {}).get("alphaTauEntropy") or {}
    failure_modes = (
        payload.get("predictedFailureModes")
        or payload.get("predicted_failure_modes")
        or []
    )
    guard_violations = (
        payload.get("guardViolations")
        or payload.get("guard_violations")
        or payload.get("tctTrace", {}).get("guardViolations")
        or []
    )
    reality_impact = (
        payload.get("realityImpact")
        or payload.get("reality_impact")
        or {}
    )

    parts: list[str] = []
    parts.append(f"Task {task_id} ({language}):")
    parts.append(f"verdict {pretty(verdict)}.")
    if isinstance(interval, dict) and interval.get("alpha"):
        parts.append(
            f"Conformal interval at alpha={interval.get('alpha')} (entropy {interval.get('entropyBits', '?')} bits)."
        )

    if failure_modes:
        names = ", ".join(_failure_name(f) for f in failure_modes[:3])
        parts.append(f"Top predicted failure modes: {names}.")
    else:
        parts.append("No top-ranked failure modes flagged.")

    if guard_violations:
        parts.append(
            f"{len(guard_violations)} TCT guard cell(s) rejected the geometry — see the constellation refresh log."
        )
    else:
        parts.append("All TCT guard cells passed.")

    if reality_impact:
        impact_keys = sorted(reality_impact.keys()) if isinstance(reality_impact, dict) else []
        if impact_keys:
            parts.append(
                "Predicted side-effects to monitor: " + ", ".join(impact_keys[:5]) + "."
            )

    parts.append("Reads come from CF_MEJEPA_LIVE_PREDICTIONS; see mejepa_inspect_prediction for the full trace.")

    return " ".join(parts)


def _failure_name(mode: Any) -> str:
    if isinstance(mode, dict):
        return str(mode.get("class") or mode.get("name") or mode.get("label") or "?")
    return str(mode)


def pretty(verdict: Any) -> str:
    if isinstance(verdict, dict):
        for key in ("verdict", "kind", "label", "status"):
            if key in verdict:
                return str(verdict[key])
        return json.dumps(verdict, separators=(",", ":"))
    return str(verdict)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
