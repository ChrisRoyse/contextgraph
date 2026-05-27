#!/usr/bin/env python3
"""Combine all NN_*.md files in this directory into COMBINED_MARKETING_PLAN.md.

Files are concatenated in numeric prefix order (00_*, 01_*, ..., 22_*).
The combined file itself, this script, and any non-NN-prefixed files are skipped.
Each section is preceded by a horizontal rule and a header pointing to the source file.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT_NAME = "COMBINED_MARKETING_PLAN.md"
PATTERN = re.compile(r"^(\d{2})_.*\.md$")


def main() -> int:
    entries: list[tuple[int, Path]] = []
    for p in HERE.iterdir():
        if not p.is_file():
            continue
        if p.name == OUT_NAME:
            continue
        m = PATTERN.match(p.name)
        if not m:
            continue
        entries.append((int(m.group(1)), p))

    if not entries:
        print(f"No NN_*.md files found in {HERE}", file=sys.stderr)
        return 1

    entries.sort(key=lambda e: (e[0], e[1].name))

    parts: list[str] = []
    parts.append("# Combined Marketing Plan\n")
    parts.append(
        f"_Auto-generated from {len(entries)} source files in `{HERE.name}/` "
        f"by `combine.py`. Do not hand-edit — edit the source files and re-run._\n"
    )
    parts.append("## Table of Contents\n")
    for idx, path in entries:
        anchor = path.stem.lower().replace("_", "-")
        parts.append(f"- [{path.stem}](#{anchor})")
    parts.append("")

    for idx, path in entries:
        parts.append("\n---\n")
        parts.append(f"<a id=\"{path.stem.lower().replace('_', '-')}\"></a>")
        parts.append(f"<!-- source: {path.name} -->\n")
        text = path.read_text(encoding="utf-8").rstrip()
        parts.append(text)
        parts.append("")

    out_path = HERE / OUT_NAME
    out_path.write_text("\n".join(parts) + "\n", encoding="utf-8")

    total_bytes = out_path.stat().st_size
    print(
        f"Wrote {out_path} ({len(entries)} sections, {total_bytes:,} bytes)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
