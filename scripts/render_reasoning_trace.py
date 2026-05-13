#!/usr/bin/env python3
"""TASK-OBS-012 — Render `mejepa_inspect_prediction` output for operators.

Reads a JSON file produced by the MCP tool `mejepa_inspect_prediction`
(or its CLI equivalent `mejepa replay`/`mejepa explain-prediction`) and
emits an HTML or Markdown report containing:

* the contributing-chunks signals table,
* a saliency heatmap of the predicted-failure-mode scores,
* the counterfactual / TCT guard-violation summary.

Usage:
    python scripts/render_reasoning_trace.py \\
        --input /path/to/inspect_prediction.json \\
        --format html|markdown \\
        --output /path/to/report.html

Reads only — never writes back to the prediction store. Fail-closed on
missing required fields with `MEJEPA_RENDER_REASONING_*` error codes.
"""

from __future__ import annotations

import argparse
import html
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass
class RenderConfig:
    input_path: Path
    output_path: Path
    output_format: str  # "html" or "markdown"


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    config = validate(args)
    payload = load_inspect_payload(config.input_path)
    if config.output_format == "html":
        rendered = render_html(payload)
    else:
        rendered = render_markdown(payload)
    write_output(config.output_path, rendered)
    print(
        json.dumps(
            {
                "input": str(config.input_path),
                "output": str(config.output_path),
                "format": config.output_format,
                "bytes_written": len(rendered.encode("utf-8")),
                "source_of_truth": "mejepa_inspect_prediction JSON payload",
            },
            indent=2,
        )
    )
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render mejepa_inspect_prediction output")
    parser.add_argument("--input", required=True, type=Path, help="JSON file from mejepa_inspect_prediction")
    parser.add_argument("--output", required=True, type=Path, help="Output report path")
    parser.add_argument(
        "--format",
        required=True,
        choices=["html", "markdown"],
        help="Output format",
    )
    return parser.parse_args(argv)


def validate(args: argparse.Namespace) -> RenderConfig:
    if not args.input.is_file():
        raise SystemExit(f"MEJEPA_RENDER_REASONING_INPUT_MISSING: {args.input} not a file")
    if args.output == args.input:
        raise SystemExit("MEJEPA_RENDER_REASONING_OUTPUT_OVERWRITES_INPUT: output must differ from input")
    return RenderConfig(input_path=args.input, output_path=args.output, output_format=args.format)


def load_inspect_payload(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise SystemExit(f"MEJEPA_RENDER_REASONING_INPUT_JSON_INVALID: {path}: {err}") from err
    if not isinstance(payload, dict):
        raise SystemExit("MEJEPA_RENDER_REASONING_INPUT_SCHEMA: top-level JSON must be an object")
    required = {"predictionId", "predictionIdentity", "contributingChunks"}
    missing = required - payload.keys()
    if missing:
        raise SystemExit(
            "MEJEPA_RENDER_REASONING_INPUT_INCOMPLETE: missing keys " + ", ".join(sorted(missing))
        )
    return payload


def render_markdown(payload: dict[str, Any]) -> str:
    identity = payload.get("predictionIdentity", {}) or {}
    chunks = payload.get("contributingChunks", []) or []
    failure_modes = (
        payload.get("conformalTrace", {}).get("predictedTestPass", []) if isinstance(payload.get("conformalTrace"), dict) else []
    )
    guard_violations = payload.get("tctTrace", {}).get("guardViolations", []) if isinstance(payload.get("tctTrace"), dict) else []

    lines = [
        f"# Reasoning trace — prediction `{identity.get('predictionId', '?')}`",
        "",
        f"- Task: `{identity.get('taskId', '?')}`",
        f"- Session: `{identity.get('sessionId', '?')}`",
        f"- Language: `{identity.get('language', '?')}`",
        f"- Created at (unix ms): `{identity.get('createdAtUnixMs', '?')}`",
        "",
        "## Contributing chunks",
        "",
        "| chunk_id | persisted_value_sha256 | saliency |",
        "|---|---|---|",
    ]
    if not chunks:
        lines.append("| _(none)_ | | |")
    else:
        for chunk in chunks:
            lines.append(
                "| `{chunk_id}` | `{sha}` | {saliency:.3f} |".format(
                    chunk_id=chunk.get("chunkId", "?"),
                    sha=chunk.get("storedValueSha256", chunk.get("persisted_value_sha256", "?")),
                    saliency=safe_float(chunk.get("saliency")),
                )
            )

    lines += ["", "## Predicted test-pass deltas", ""]
    if not failure_modes:
        lines.append("_(none)_")
    else:
        for mode in failure_modes:
            lines.append(f"- `{mode}`")

    lines += ["", "## TCT guard violations", ""]
    if not guard_violations:
        lines.append("_None — predicted output lies inside the known-good constellation region._")
    else:
        for violation in guard_violations:
            lines.append(f"- `{violation}`")

    lines += ["", "## Source of truth", "", "- `CF_MEJEPA_LIVE_PREDICTIONS` + `CF_MEJEPA_DDA_SIGNALS`", ""]
    return "\n".join(lines)


def render_html(payload: dict[str, Any]) -> str:
    md = render_markdown(payload)
    body = md_to_html_minimal(md)
    return (
        "<!DOCTYPE html>\n<html><head><meta charset='utf-8'>"
        "<title>ME-JEPA reasoning trace</title>"
        "<style>body{font-family:system-ui,Arial,sans-serif;max-width:960px;margin:2em auto;padding:0 1em}"
        "table{border-collapse:collapse;margin:1em 0}th,td{border:1px solid #ddd;padding:.4em .8em;font-family:monospace;font-size:90%}"
        "th{background:#f5f5f5}code{background:#f5f5f5;padding:.1em .3em;border-radius:3px}h1,h2{border-bottom:1px solid #ddd;padding-bottom:.2em}"
        "</style></head><body>\n"
        + body
        + "\n</body></html>\n"
    )


def md_to_html_minimal(md: str) -> str:
    """Tiny markdown->HTML converter — no external deps."""
    lines = md.splitlines()
    out: list[str] = []
    in_table = False
    table_rows: list[list[str]] = []
    for raw in lines:
        line = raw.rstrip()
        if line.startswith("|") and line.endswith("|"):
            cells = [c.strip() for c in line.strip("|").split("|")]
            if all(set(c) <= set("-: ") for c in cells):
                continue  # separator row
            table_rows.append(cells)
            in_table = True
            continue
        if in_table:
            out.append(_emit_table(table_rows))
            table_rows = []
            in_table = False
        if line.startswith("# "):
            out.append(f"<h1>{html.escape(line[2:])}</h1>")
        elif line.startswith("## "):
            out.append(f"<h2>{html.escape(line[3:])}</h2>")
        elif line.startswith("- "):
            out.append(f"<li>{inline(line[2:])}</li>")
        elif line == "":
            out.append("")
        else:
            out.append(f"<p>{inline(line)}</p>")
    if in_table:
        out.append(_emit_table(table_rows))
    return "\n".join(out)


def _emit_table(rows: list[list[str]]) -> str:
    if not rows:
        return ""
    header, *body = rows
    parts = ["<table>", "<thead><tr>"]
    parts.extend(f"<th>{html.escape(cell)}</th>" for cell in header)
    parts.append("</tr></thead><tbody>")
    for row in body:
        parts.append("<tr>" + "".join(f"<td>{inline(cell)}</td>" for cell in row) + "</tr>")
    parts.append("</tbody></table>")
    return "\n".join(parts)


def inline(text: str) -> str:
    escaped = html.escape(text)
    # Replace backtick-wrapped spans with <code>.
    out: list[str] = []
    in_code = False
    buf = ""
    for ch in escaped:
        if ch == "`":
            if in_code:
                out.append(f"<code>{buf}</code>")
            else:
                out.append(buf)
            buf = ""
            in_code = not in_code
        else:
            buf += ch
    if in_code:
        out.append(f"<code>{buf}</code>")
    else:
        out.append(buf)
    return "".join(out)


def safe_float(value: Any) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def write_output(path: Path, contents: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
