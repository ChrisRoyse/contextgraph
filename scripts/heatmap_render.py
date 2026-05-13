#!/usr/bin/env python3
"""Render a pairwise MI CSV as markdown and a PNG heatmap using stdlib only."""

from __future__ import annotations

import argparse
import binascii
import csv
import pathlib
import struct
import zlib


CELL_SIZE = 32


def load_matrix(path: pathlib.Path) -> tuple[list[str], list[list[float]]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        rows = list(csv.reader(handle))
    if not rows or len(rows[0]) < 2 or rows[0][0] != "slot":
        raise SystemExit("MEJEPA_EVAL_INVALID_INPUT: CSV header must start with slot")
    slots = [value.strip() for value in rows[0][1:]]
    if any(not slot for slot in slots):
        raise SystemExit("MEJEPA_EVAL_INVALID_INPUT: slot names must be non-empty")
    values: list[list[float]] = []
    row_slots: list[str] = []
    for row in rows[1:]:
        if len(row) != len(slots) + 1:
            raise SystemExit("MEJEPA_EVAL_INVALID_INPUT: ragged CSV row")
        row_slots.append(row[0].strip())
        parsed = [float(value) for value in row[1:]]
        if any(value < 0.0 or value > 1.0 for value in parsed):
            raise SystemExit("MEJEPA_EVAL_INVALID_INPUT: MI values must be in [0,1]")
        values.append(parsed)
    if row_slots != slots:
        raise SystemExit("MEJEPA_EVAL_INVALID_INPUT: row labels must match header slots")
    return slots, values


def write_markdown(path: pathlib.Path, slots: list[str], values: list[list[float]]) -> None:
    lines = ["| slot | " + " | ".join(slots) + " |", "|---|" + "---:|" * len(slots)]
    for slot, row in zip(slots, values):
        lines.append("| " + slot + " | " + " | ".join(f"{value:.3f}" for value in row) + " |")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_png(path: pathlib.Path, values: list[list[float]]) -> None:
    width = len(values) * CELL_SIZE
    height = width
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        row = y // CELL_SIZE
        for x in range(width):
            col = x // CELL_SIZE
            raw.extend(heat_color(values[row][col]))
    png = bytearray(b"\x89PNG\r\n\x1a\n")
    png.extend(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)))
    png.extend(chunk(b"IDAT", zlib.compress(bytes(raw), level=1)))
    png.extend(chunk(b"IEND", b""))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes(png))


def heat_color(value: float) -> bytes:
    value = min(1.0, max(0.0, value))
    red = round(244.0 - 64.0 * value)
    green = round(247.0 - 150.0 * value)
    blue = round(251.0 - 180.0 * value)
    return bytes((red, green, blue))


def chunk(kind: bytes, data: bytes) -> bytes:
    payload = kind + data
    return struct.pack(">I", len(data)) + payload + struct.pack(">I", binascii.crc32(payload) & 0xFFFFFFFF)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--csv", required=True, type=pathlib.Path)
    parser.add_argument("--markdown", required=True, type=pathlib.Path)
    parser.add_argument("--png", required=True, type=pathlib.Path)
    args = parser.parse_args()
    slots, values = load_matrix(args.csv)
    write_markdown(args.markdown, slots, values)
    write_png(args.png, values)
    print(f"rendered {len(slots)}x{len(slots)} heatmap to {args.markdown} and {args.png}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
