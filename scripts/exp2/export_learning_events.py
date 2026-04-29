#!/usr/bin/env python3
"""Export real CF_LEARNING_EVENTS rows for EXP-2 S4 with readback proof."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path
import sys
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import Exp2Error, load_config, output_dir, repo_path, setup_logging, text_hash, write_json


def run_json(command: list[str], *, timeout: int = 300) -> dict[str, Any]:
    proc = subprocess.run(
        command,
        cwd=repo_path("."),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=False,
    )
    if proc.returncode != 0:
        raise Exp2Error(
            f"command failed: {' '.join(command)}\n"
            f"stdout_tail={proc.stdout[-2000:]}\nstderr_tail={proc.stderr[-2000:]}"
        )
    try:
        value = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise Exp2Error(f"command did not emit JSON: {' '.join(command)}") from exc
    if not isinstance(value, dict):
        raise Exp2Error(f"command JSON root is not an object: {' '.join(command)}")
    return value


def event_text(row: dict[str, Any]) -> str:
    outcome = row.get("outcome") if isinstance(row.get("outcome"), dict) else {}
    before = row.get("before") if isinstance(row.get("before"), dict) else {}
    after = row.get("after") if isinstance(row.get("after"), dict) else {}
    summary = row.get("feature_summary") if isinstance(row.get("feature_summary"), dict) else {}
    signals = row.get("signals") if isinstance(row.get("signals"), list) else []

    def floats(values: Any, limit: int = 14) -> str:
        if not isinstance(values, list):
            return ""
        return " ".join(f"{float(value):.4f}" for value in values[:limit])

    signal_summary = " ".join(
        f"{signal.get('signal_id')}={float(signal.get('scalar', 0.0)):.4f}:{signal.get('label')}"
        for signal in signals
        if isinstance(signal, dict)
    )
    parts = [
        f"Query: {row.get('query', '')}",
        f"Retrieved context: {row.get('retrieved_context', '')}",
        f"Assistant response: {row.get('assistant_response', '')}",
        (
            "Outcome "
            f"label {outcome.get('label')} "
            f"utility {float(outcome.get('utility_delta', 0.0)):.4f} "
            f"correction {outcome.get('correction_required')} "
            f"reuse {outcome.get('reuse_observed')}"
        ),
        (
            "Before "
            f"domain {before.get('domain')} "
            f"rank {before.get('retrieval_rank')} "
            f"topic {floats(before.get('topic_profile'))}"
        ),
        (
            "After "
            f"domain {after.get('domain')} "
            f"rank {after.get('retrieval_rank')} "
            f"topic {floats(after.get('topic_profile'))}"
        ),
        (
            "Feature summary "
            f"delta_e {float(summary.get('delta_e_scalar', 0.0)):.4f} "
            f"surprise {float(summary.get('surprise_score', 0.0)):.4f} "
            f"coherence {float(summary.get('coherence_delta', 0.0)):.4f} "
            f"consolidation {float(summary.get('consolidation_readiness', 0.0)):.4f} "
            f"transfer {float(summary.get('transfer_score', 0.0)):.4f} "
            f"rank_shift {float(summary.get('retrieval_rank_shift', 0.0)):.4f}"
        ),
        f"Signals {signal_summary}",
    ]
    return " ".join("\n".join(parts).split())


def word_count(text: str) -> int:
    return len(text.split())


def validate_exported_row(row: dict[str, Any], *, source: Path, line_no: int) -> None:
    required = [
        "record_kind",
        "event_id",
        "query",
        "retrieved_context",
        "assistant_response",
        "outcome",
        "before",
        "after",
        "feature_tensor",
        "feature_schema",
        "feature_tensor_len",
    ]
    missing = [key for key in required if key not in row]
    if missing:
        raise Exp2Error(f"{source}:{line_no}: missing fields {missing}")
    if row["record_kind"] != "learning_event_tensor":
        raise Exp2Error(f"{source}:{line_no}: expected record_kind=learning_event_tensor")
    outcome = row["outcome"]
    if not isinstance(outcome, dict) or not outcome.get("label"):
        raise Exp2Error(f"{source}:{line_no}: missing outcome.label")
    tensor = row["feature_tensor"]
    schema = row["feature_schema"]
    if not isinstance(tensor, list) or not isinstance(schema, list) or len(tensor) != len(schema):
        raise Exp2Error(f"{source}:{line_no}: feature tensor/schema mismatch")
    if int(row["feature_tensor_len"]) != len(schema):
        raise Exp2Error(f"{source}:{line_no}: feature_tensor_len does not match schema")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--cli", default="target/debug/context-graph-cli")
    parser.add_argument("--storage", action="append", type=Path, required=True)
    parser.add_argument("--out", default="artefacts/exp2/learning_events_export.json")
    parser.add_argument("--report", default="artefacts/exp2/learning_events_export_report.json")
    parser.add_argument("--min-candidates", type=int, default=225)
    args = parser.parse_args()

    cfg = load_config(args.config)
    out_dir = output_dir(cfg)
    log = setup_logging(out_dir)
    cli = repo_path(args.cli)
    if not cli.exists():
        raise Exp2Error(f"context-graph-cli missing: {cli}")

    rows: list[dict[str, Any]] = []
    source_reports = []
    with tempfile.TemporaryDirectory(prefix="exp2-learning-events-") as tmp:
        tmp_dir = Path(tmp)
        for index, storage in enumerate(args.storage):
            storage = storage.expanduser()
            if not (storage / "CURRENT").exists():
                raise Exp2Error(f"RocksDB source missing CURRENT: {storage}")
            before = run_json([str(cli), "learning", "count", "--storage", str(storage)])
            export_path = tmp_dir / f"source_{index:03d}.jsonl"
            exported = run_json(
                [
                    str(cli),
                    "learning",
                    "export-events-jsonl",
                    "--storage",
                    str(storage),
                    "--out",
                    str(export_path),
                    "--include-text",
                    "true",
                    "--overwrite",
                ],
                timeout=900,
            )
            after = run_json([str(cli), "learning", "count", "--storage", str(storage)])
            if before.get("count") != after.get("count"):
                raise Exp2Error(f"export mutated CF_LEARNING_EVENTS for {storage}: before={before} after={after}")
            exported_rows = []
            for line_no, line in enumerate(export_path.read_text(encoding="utf-8").splitlines(), start=1):
                if not line.strip():
                    continue
                row = json.loads(line)
                validate_exported_row(row, source=export_path, line_no=line_no)
                exported_rows.append(row)
            if len(exported_rows) != int(exported.get("records_written", -1)):
                raise Exp2Error(
                    f"{storage}: JSONL rows {len(exported_rows)} != records_written {exported.get('records_written')}"
                )
            rows.extend({"_source_storage": str(storage), "_source_line": i + 1, **row} for i, row in enumerate(exported_rows))
            source_reports.append(
                {
                    "storage": str(storage),
                    "count_before": before["count"],
                    "count_after": after["count"],
                    "records_written": exported["records_written"],
                    "bytes_written": exported["bytes_written"],
                }
            )

    selected = []
    seen_text_hashes: set[str] = set()
    seen_event_ids: set[str] = set()
    skipped = {"duplicate_event_id": 0, "duplicate_text": 0, "token_bounds": 0}
    for row in rows:
        event_id = str(row["event_id"])
        if event_id in seen_event_ids:
            skipped["duplicate_event_id"] += 1
            continue
        seen_event_ids.add(event_id)
        text = event_text(row)
        n_words = word_count(text)
        if n_words < 50 or n_words > 800:
            skipped["token_bounds"] += 1
            continue
        h = text_hash(text)
        if h in seen_text_hashes:
            skipped["duplicate_text"] += 1
            continue
        seen_text_hashes.add(h)
        outcome = row["outcome"]
        selected.append(
            {
                "text": text,
                "query": row.get("query"),
                "outcome": outcome,
                "metadata": {
                    "source_storage": row["_source_storage"],
                    "source_line": row["_source_line"],
                    "event_id": event_id,
                    "created_at": row.get("created_at"),
                    "session_id": row.get("session_id"),
                    "task_id": row.get("task_id"),
                    "response_id": row.get("response_id"),
                    "feature_tensor_len": row.get("feature_tensor_len"),
                    "feature_summary": row.get("feature_summary"),
                    "text_words": n_words,
                    "text_sha256": h,
                },
            }
        )

    if len(selected) < args.min_candidates:
        raise Exp2Error(f"S4 export has {len(selected)} valid candidates, need {args.min_candidates}")

    out_path = repo_path(args.out)
    report_path = repo_path(args.report)
    write_json(out_path, selected)
    reread = json.loads(out_path.read_text(encoding="utf-8"))
    if len(reread) != len(selected):
        raise Exp2Error(f"S4 output readback count mismatch: wrote {len(selected)} read {len(reread)}")
    labels: dict[str, int] = {}
    for row in selected:
        label = str(row["outcome"]["label"])
        labels[label] = labels.get(label, 0) + 1
    report = {
        "status": "ok",
        "source_of_truth": {
            "input": "RocksDB CF_LEARNING_EVENTS for each storage source",
            "output": str(out_path),
        },
        "sources": source_reports,
        "raw_rows": len(rows),
        "selected_candidates": len(selected),
        "label_counts": labels,
        "skipped": skipped,
        "output_readback_count": len(reread),
    }
    write_json(report_path, report)
    log.info("Exported %s S4 learning-event candidates to %s", len(selected), out_path)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 learning-event export failed: {exc}") from exc
