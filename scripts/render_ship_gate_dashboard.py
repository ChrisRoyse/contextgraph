#!/usr/bin/env python3
"""Render the active Python ship-gate cell dashboard from SoT JSON.

The preferred input is `context-graph-cli mejepa cell-status` JSON, which is
read from `CF_MEJEPA_EVAL_REPORTS`. The renderer also accepts the
`mejepa_ship_gate_status` MCP payload shape. Both inputs must describe exactly
the 8 Python mutation-category cells; stale 80-cell language grids fail closed.
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


MUTATION_CATEGORIES = [
    "known_good",
    "subtle_flip",
    "off_by_one",
    "swap_variable",
    "delete_test_call",
    "wrong_file",
    "over_engineer",
    "compile_error",
]

ACTIVE_GATE_LANGUAGES = ["python"]

ACTIVE_GATE_NAME = "python_swebench_lite_300x8"
ACTIVE_REQUIRED_GRID = "8 mutation categories x python"
REQUIRED_CELL_COUNT = len(MUTATION_CATEGORIES) * len(ACTIVE_GATE_LANGUAGES)
REQUIRED_CELLS = {
    f"{category}::{language}"
    for category in MUTATION_CATEGORIES
    for language in ACTIVE_GATE_LANGUAGES
}
DEFAULT_THRESHOLD = 0.95


@dataclass(frozen=True)
class CellRow:
    cell: str
    mutation_category: str
    language: str
    correlation: float | None
    holdout_count: int | None
    distance_to_threshold: float | None
    last_update_unix_ms: int | None
    status: str
    exemption_reason: str | None
    prediction_links: tuple[str, ...]


@dataclass(frozen=True)
class DashboardState:
    latest_report_date: str
    generated_at_unix_ms: int | None
    threshold: float
    source_of_truth: str
    ship_gate_passed: bool | None
    required_cell_count: int
    observed_cell_count: int
    passing_cell_count: int
    below_threshold_cell_count: int
    missing_cell_count: int
    failing_cell_count: int
    rows: tuple[CellRow, ...]


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    payload = load_json(args.input)
    state = build_dashboard_state(payload, args.failing_predictions_dir, args.output)
    if args.format == "markdown":
        rendered = render_markdown(state, args.title)
    else:
        rendered = render_html(state, args.title)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")
    summary = {
        "output": str(args.output),
        "format": args.format,
        "sourceOfTruth": state.source_of_truth,
        "requiredCellCount": state.required_cell_count,
        "renderedCellCount": len(state.rows),
        "failingCellCount": state.failing_cell_count,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render Phase G ship-gate dashboard")
    parser.add_argument(
        "--input",
        "--status-json",
        "--cell-status-json",
        dest="input",
        required=True,
        type=Path,
        help="cell-status JSON or mejepa_ship_gate_status JSON",
    )
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--format", choices=("markdown", "html"), default="markdown")
    parser.add_argument(
        "--failing-predictions-dir",
        type=Path,
        default=None,
        help="directory containing <cell>.md/html/txt or <category>__<language>.md/html/txt",
    )
    parser.add_argument("--title", default="ME-JEPA Ship Gate Dashboard")
    return parser.parse_args(argv)


def load_json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_INPUT_MISSING: {path} is not a file")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_JSON_INVALID: {path}: {err}") from err
    if not isinstance(value, dict):
        raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: top-level JSON must be an object")
    return value


def build_dashboard_state(
    payload: dict[str, Any],
    failing_predictions_dir: Path | None,
    output_path: Path,
) -> DashboardState:
    threshold = parse_threshold(payload)
    validate_active_gate_metadata(payload)
    source_of_truth = source_of_truth_label(payload)
    exemptions = parse_exemptions(payload.get("cellExemptions", {}))
    provided_rows = parse_cell_rows(payload, threshold)
    per_cell_correlations = payload.get("perCellCorrelation", {})
    if per_cell_correlations is None:
        per_cell_correlations = {}
    if not isinstance(per_cell_correlations, dict):
        raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: perCellCorrelation must be an object")
    validate_active_gate_cells(provided_rows, per_cell_correlations)
    prediction_map = parse_prediction_links(payload.get("failingPredictions", {}))

    rows: list[CellRow] = []
    observed = 0
    passing = 0
    below = 0
    missing = 0
    for category in MUTATION_CATEGORIES:
        for language in ACTIVE_GATE_LANGUAGES:
            cell = f"{category}::{language}"
            raw = provided_rows.get(cell)
            if raw is None:
                correlation = parse_optional_float(per_cell_correlations.get(cell))
                holdout_count = parse_optional_int(payload.get("holdoutCount"))
                last_update = parse_optional_int(payload.get("generatedAtUnixMs"))
                distance = None if correlation is None else max(threshold - correlation, 0.0)
                status = status_for_correlation(correlation, threshold)
            else:
                correlation = raw["correlation"]
                holdout_count = raw["holdout_count"]
                last_update = raw["last_update_unix_ms"]
                distance = raw["distance_to_threshold"]
                status = raw["status"]
            if status == "pass":
                observed += 1
                passing += 1
            elif status == "below_threshold":
                observed += 1
                below += 1
            elif status == "missing":
                missing += 1
            else:
                if correlation is not None:
                    observed += 1
            links = tuple(
                list(prediction_map.get(cell, ()))
                + prediction_files_for_cell(failing_predictions_dir, output_path, cell)
            )
            rows.append(
                CellRow(
                    cell=cell,
                    mutation_category=category,
                    language=language,
                    correlation=correlation,
                    holdout_count=holdout_count,
                    distance_to_threshold=distance,
                    last_update_unix_ms=last_update,
                    status=status,
                    exemption_reason=exemptions.get(cell),
                    prediction_links=links,
                )
            )
    failing = below + missing
    latest_report_date = str(
        payload.get("latestReportDate")
        or payload.get("reportDate")
        or payload.get("report", {}).get("reportDate")
        or "unknown"
    )
    generated_at = parse_optional_int(
        payload.get("generatedAtUnixMs") or payload.get("report", {}).get("generatedAtUnixMs")
    )
    return DashboardState(
        latest_report_date=latest_report_date,
        generated_at_unix_ms=generated_at,
        threshold=threshold,
        source_of_truth=source_of_truth,
        ship_gate_passed=parse_optional_bool(
            payload.get("passed", payload.get("shipGatePassed"))
        ),
        required_cell_count=REQUIRED_CELL_COUNT,
        observed_cell_count=observed,
        passing_cell_count=passing,
        below_threshold_cell_count=below,
        missing_cell_count=missing,
        failing_cell_count=failing,
        rows=tuple(rows),
    )


def parse_threshold(payload: dict[str, Any]) -> float:
    value = payload.get("threshold", payload.get("shipGateCorrelationThreshold", DEFAULT_THRESHOLD))
    parsed = parse_optional_float(value)
    if parsed is None or not -1.0 <= parsed <= 1.0:
        raise SystemExit(
            f"MEJEPA_SHIP_GATE_DASHBOARD_THRESHOLD_INVALID: threshold must be in [-1,1], got {value!r}"
        )
    return parsed


def parse_cell_rows(payload: dict[str, Any], threshold: float) -> dict[str, dict[str, Any]]:
    raw_rows = payload.get("cells")
    if raw_rows is None:
        return {}
    if not isinstance(raw_rows, list):
        raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: cells must be an array")
    rows: dict[str, dict[str, Any]] = {}
    for index, raw in enumerate(raw_rows):
        if not isinstance(raw, dict):
            raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: cells[{index}] must be an object")
        cell = str(raw.get("cell", ""))
        category, language = split_cell(cell)
        raw_category = raw.get("mutationCategory", category)
        raw_language = raw.get("language", language)
        if raw_category != category or raw_language != language:
            raise SystemExit(
                f"MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: row {cell} category/language mismatch"
            )
        if cell in rows:
            raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: duplicate cell row {cell}")
        correlation = parse_optional_float(raw.get("correlation"))
        status = str(raw.get("status") or status_for_correlation(correlation, threshold))
        rows[cell] = {
            "correlation": correlation,
            "holdout_count": parse_optional_int(raw.get("holdoutCount")),
            "distance_to_threshold": parse_optional_float(raw.get("distanceToThreshold")),
            "last_update_unix_ms": parse_optional_int(raw.get("lastUpdateUnixMs")),
            "status": status,
        }
    return rows


def split_cell(cell: str) -> tuple[str, str]:
    if "::" not in cell:
        raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_CELL_INVALID: {cell!r}")
    category, language = cell.split("::", 1)
    if category not in MUTATION_CATEGORIES:
        raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_CATEGORY_INVALID: {category!r}")
    if language not in ACTIVE_GATE_LANGUAGES:
        raise SystemExit(
            f"MEJEPA_SHIP_GATE_DASHBOARD_STALE_GATE_LANGUAGE: active gate {ACTIVE_GATE_NAME} accepts only python cells; got {language!r}"
        )
    return category, language


def validate_active_gate_metadata(payload: dict[str, Any]) -> None:
    count = payload.get("requiredCellCount")
    if count is not None:
        parsed = parse_optional_int(count)
        if parsed != REQUIRED_CELL_COUNT:
            raise SystemExit(
                f"MEJEPA_SHIP_GATE_DASHBOARD_STALE_REQUIRED_CELL_COUNT: expected {REQUIRED_CELL_COUNT} for {ACTIVE_REQUIRED_GRID}; got {count!r}"
            )
    gate = payload.get("activeGate")
    if gate is not None and str(gate) != ACTIVE_GATE_NAME:
        raise SystemExit(
            f"MEJEPA_SHIP_GATE_DASHBOARD_ACTIVE_GATE_INVALID: expected {ACTIVE_GATE_NAME}; got {gate!r}"
        )
    source = payload.get("sourceOfTruth")
    if isinstance(source, dict):
        required_grid = source.get("required_grid") or source.get("requiredGrid")
        if required_grid is not None and str(required_grid) != ACTIVE_REQUIRED_GRID:
            raise SystemExit(
                f"MEJEPA_SHIP_GATE_DASHBOARD_STALE_REQUIRED_GRID: expected {ACTIVE_REQUIRED_GRID}; got {required_grid!r}"
            )


def validate_active_gate_cells(
    provided_rows: dict[str, dict[str, Any]],
    per_cell_correlations: dict[str, Any],
) -> None:
    actual = set(provided_rows) if provided_rows else set(per_cell_correlations)
    if actual != REQUIRED_CELLS:
        missing = sorted(REQUIRED_CELLS - actual)
        extra = sorted(actual - REQUIRED_CELLS)
        if extra:
            for cell in extra:
                split_cell(cell)
        raise SystemExit(
            "MEJEPA_SHIP_GATE_DASHBOARD_CELL_SET_MISMATCH: "
            f"expected {REQUIRED_CELL_COUNT} cells for {ACTIVE_REQUIRED_GRID}; "
            f"observed={len(actual)} missing={format_list(missing)} extra={format_list(extra)}"
        )


def format_list(values: list[str]) -> str:
    if not values:
        return "[]"
    preview = values[:8]
    if len(values) > 8:
        preview.append(f"...{len(values) - 8} more")
    return "[" + ", ".join(preview) + "]"


def parse_exemptions(raw: Any) -> dict[str, str]:
    if raw in (None, {}):
        return {}
    if not isinstance(raw, dict):
        raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: cellExemptions must be an object")
    result: dict[str, str] = {}
    for cell, payload in raw.items():
        if not isinstance(cell, str):
            raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: exemption key must be a string")
        if "::" in cell:
            category, language = cell.split("::", 1)
            split_cell(cell)
            if isinstance(payload, dict):
                result[cell] = str(payload.get("reason") or "active exemption")
            else:
                result[cell] = "active exemption"
    return result


def parse_prediction_links(raw: Any) -> dict[str, list[str]]:
    if raw in (None, {}):
        return {}
    if not isinstance(raw, dict):
        raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: failingPredictions must be an object")
    result: dict[str, list[str]] = {}
    for cell, entries in raw.items():
        if not isinstance(cell, str):
            raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: failingPredictions key must be a string")
        split_cell(cell)
        result[cell] = normalize_prediction_entries(entries)
    return result


def normalize_prediction_entries(entries: Any) -> list[str]:
    if isinstance(entries, str):
        return [entries]
    if isinstance(entries, dict):
        href = entries.get("path") or entries.get("href") or entries.get("url")
        return [str(href)] if href else []
    if isinstance(entries, list):
        links: list[str] = []
        for item in entries:
            if isinstance(item, str):
                links.append(item)
            elif isinstance(item, dict):
                href = item.get("path") or item.get("href") or item.get("url")
                if href:
                    links.append(str(href))
        return links
    raise SystemExit("MEJEPA_SHIP_GATE_DASHBOARD_SCHEMA: failingPredictions entries invalid")


def prediction_files_for_cell(
    directory: Path | None,
    output_path: Path,
    cell: str,
) -> list[str]:
    if directory is None:
        return []
    if not directory.is_dir():
        raise SystemExit(
            f"MEJEPA_SHIP_GATE_DASHBOARD_PREDICTIONS_DIR_MISSING: {directory} is not a directory"
        )
    safe = safe_cell_name(cell)
    candidates = []
    for stem in (cell, safe):
        for suffix in (".md", ".html", ".txt", ".json"):
            candidates.append(directory / f"{stem}{suffix}")
    links = []
    for path in candidates:
        if path.is_file():
            links.append(relative_link(output_path, path))
    return links


def safe_cell_name(cell: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]+", "__", cell)


def relative_link(output_path: Path, target: Path) -> str:
    try:
        return os.path.relpath(target, start=output_path.parent)
    except ValueError:
        return str(target)


def status_for_correlation(correlation: float | None, threshold: float) -> str:
    if correlation is None:
        return "missing"
    if correlation >= threshold:
        return "pass"
    return "below_threshold"


def source_of_truth_label(payload: dict[str, Any]) -> str:
    source = payload.get("sourceOfTruth")
    if isinstance(source, str):
        return source
    if isinstance(source, dict):
        cf = source.get("column_family") or source.get("evalReport")
        selector = source.get("selector")
        if cf and selector:
            return f"{cf} ({selector})"
        if cf:
            return str(cf)
    return "CF_MEJEPA_EVAL_REPORTS"


def render_markdown(state: DashboardState, title: str) -> str:
    lines: list[str] = [
        f"# {title}",
        "",
        "| Field | Value |",
        "|---|---|",
        f"| Source of truth | `{state.source_of_truth}` |",
        f"| Latest report | `{state.latest_report_date}` |",
        f"| Generated at | `{format_unix_ms(state.generated_at_unix_ms)}` |",
        f"| Threshold | `{state.threshold:.3f}` |",
        f"| Ship gate passed | `{format_bool(state.ship_gate_passed)}` |",
        f"| Required cells | `{state.required_cell_count}` |",
        f"| Observed cells | `{state.observed_cell_count}` |",
        f"| Passing cells | `{state.passing_cell_count}` |",
        f"| Below-threshold cells | `{state.below_threshold_cell_count}` |",
        f"| Missing cells | `{state.missing_cell_count}` |",
        "",
        "## Correlation Heatmap",
        "",
        heatmap_markdown(state),
        "",
        "## Per-Cell Drilldown",
        "",
        "| Cell | Mutation | Language | Correlation | Holdout | Last update | Status | Exemption | Representative failing predictions |",
        "|---|---|---|---:|---:|---|---|---|---|",
    ]
    for row in state.rows:
        lines.append(
            "| "
            + " | ".join(
                [
                    f"`{row.cell}`",
                    f"`{row.mutation_category}`",
                    f"`{row.language}`",
                    format_correlation(row.correlation),
                    format_optional_int(row.holdout_count),
                    f"`{format_unix_ms(row.last_update_unix_ms)}`",
                    f"`{row.status}`",
                    markdown_text(row.exemption_reason or "none"),
                    format_markdown_links(row.prediction_links),
                ]
            )
            + " |"
        )
    return "\n".join(lines) + "\n"


def heatmap_markdown(state: DashboardState) -> str:
    row_by_cell = {row.cell: row for row in state.rows}
    lines = [
        "| Mutation category | " + " | ".join(f"`{language}`" for language in ACTIVE_GATE_LANGUAGES) + " |",
        "|---|" + "---:|" * len(ACTIVE_GATE_LANGUAGES),
    ]
    for category in MUTATION_CATEGORIES:
        values = []
        for language in ACTIVE_GATE_LANGUAGES:
            row = row_by_cell[f"{category}::{language}"]
            values.append(heatmap_value(row))
        lines.append(f"| `{category}` | " + " | ".join(values) + " |")
    return "\n".join(lines)


def heatmap_value(row: CellRow) -> str:
    prefix = format_correlation(row.correlation)
    if row.exemption_reason:
        return f"{prefix} exempt"
    if row.status == "missing":
        return "missing"
    if row.status != "pass":
        return f"{prefix} {row.status}"
    return prefix


def render_html(state: DashboardState, title: str) -> str:
    rows = "\n".join(
        "<tr>"
        + "".join(
            f"<td>{html.escape(value)}</td>"
            for value in [
                row.cell,
                row.mutation_category,
                row.language,
                format_correlation(row.correlation),
                format_optional_int(row.holdout_count),
                format_unix_ms(row.last_update_unix_ms),
                row.status,
                row.exemption_reason or "none",
            ]
        )
        + f"<td>{format_html_links(row.prediction_links)}</td></tr>"
        for row in state.rows
    )
    heatmap = html_heatmap(state)
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{html.escape(title)}</title>
<style>
body {{ font-family: system-ui, sans-serif; margin: 24px; color: #16212f; }}
table {{ border-collapse: collapse; width: 100%; margin: 16px 0 28px; font-size: 14px; }}
th, td {{ border: 1px solid #c8d1dc; padding: 6px 8px; text-align: left; }}
th {{ background: #eef2f6; }}
.metric td:first-child {{ font-weight: 600; width: 240px; }}
</style>
</head>
<body>
<h1>{html.escape(title)}</h1>
<table class="metric">
<tr><td>Source of truth</td><td><code>{html.escape(state.source_of_truth)}</code></td></tr>
<tr><td>Latest report</td><td><code>{html.escape(state.latest_report_date)}</code></td></tr>
<tr><td>Generated at</td><td><code>{html.escape(format_unix_ms(state.generated_at_unix_ms))}</code></td></tr>
<tr><td>Threshold</td><td><code>{state.threshold:.3f}</code></td></tr>
<tr><td>Ship gate passed</td><td><code>{format_bool(state.ship_gate_passed)}</code></td></tr>
<tr><td>Required cells</td><td><code>{state.required_cell_count}</code></td></tr>
<tr><td>Observed cells</td><td><code>{state.observed_cell_count}</code></td></tr>
<tr><td>Passing cells</td><td><code>{state.passing_cell_count}</code></td></tr>
<tr><td>Below-threshold cells</td><td><code>{state.below_threshold_cell_count}</code></td></tr>
<tr><td>Missing cells</td><td><code>{state.missing_cell_count}</code></td></tr>
</table>
<h2>Correlation Heatmap</h2>
{heatmap}
<h2>Per-Cell Drilldown</h2>
<table>
<tr><th>Cell</th><th>Mutation</th><th>Language</th><th>Correlation</th><th>Holdout</th><th>Last update</th><th>Status</th><th>Exemption</th><th>Representative failing predictions</th></tr>
{rows}
</table>
</body>
</html>
"""


def html_heatmap(state: DashboardState) -> str:
    row_by_cell = {row.cell: row for row in state.rows}
    header = "".join(f"<th>{html.escape(language)}</th>" for language in ACTIVE_GATE_LANGUAGES)
    body_rows = []
    for category in MUTATION_CATEGORIES:
        cells = []
        for language in ACTIVE_GATE_LANGUAGES:
            row = row_by_cell[f"{category}::{language}"]
            cells.append(f"<td>{html.escape(heatmap_value(row))}</td>")
        body_rows.append(f"<tr><th>{html.escape(category)}</th>{''.join(cells)}</tr>")
    return f"<table><tr><th>Mutation category</th>{header}</tr>{''.join(body_rows)}</table>"


def format_correlation(value: float | None) -> str:
    if value is None:
        return "missing"
    return f"{value:.3f}"


def format_optional_int(value: int | None) -> str:
    return "missing" if value is None else str(value)


def format_unix_ms(value: int | None) -> str:
    if value is None:
        return "unknown"
    if value <= 0:
        return str(value)
    dt = datetime.fromtimestamp(value / 1000, tz=timezone.utc)
    return f"{value} ({dt.isoformat()})"


def format_bool(value: bool | None) -> str:
    if value is None:
        return "unknown"
    return "true" if value else "false"


def markdown_text(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def format_markdown_links(links: tuple[str, ...]) -> str:
    if not links:
        return "none"
    return ", ".join(f"[open {index + 1}]({escape_markdown_link(link)})" for index, link in enumerate(links))


def escape_markdown_link(link: str) -> str:
    return link.replace(")", "%29").replace(" ", "%20")


def format_html_links(links: tuple[str, ...]) -> str:
    if not links:
        return "none"
    return ", ".join(
        f'<a href="{html.escape(link, quote=True)}">open {index + 1}</a>'
        for index, link in enumerate(links)
    )


def parse_optional_float(value: Any) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_NUMBER_INVALID: {value!r}")
    if isinstance(value, (int, float)):
        parsed = float(value)
    elif isinstance(value, str) and value.strip().lower() not in ("", "none", "null", "missing"):
        parsed = float(value)
    else:
        return None
    if parsed != parsed or parsed in (float("inf"), float("-inf")):
        raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_NUMBER_INVALID: {value!r}")
    return parsed


def parse_optional_int(value: Any) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise SystemExit(f"MEJEPA_SHIP_GATE_DASHBOARD_INT_INVALID: {value!r}")
    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    if isinstance(value, str) and value.strip().lower() not in ("", "none", "null", "missing"):
        return int(value)
    return None


def parse_optional_bool(value: Any) -> bool | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        lowered = value.strip().lower()
        if lowered == "true":
            return True
        if lowered == "false":
            return False
    return None


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
