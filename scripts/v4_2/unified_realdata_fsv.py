#!/usr/bin/env python3
"""Real-data full-state verification for the unified CAMD + learner loop.

This runner intentionally uses persisted local datasets:

* Shakespeare CAMD DPO pairs from ~/.contextgraph/training/v4_2_camd_lmdb.jsonl
* GoEmotions sample from data/utl_calibration/go_emotions/go_emotions_sample.jsonl
* Extended embedder LMDB from ~/.contextgraph/extended_embeddings.lmdb

It creates a temporary RocksDB store, imports two real-content LearningEvent
rows, exports tensors, mines learner-conditioned DPO pairs, validates the
unified trainer input path, inspects LMDB embeddings, runs three edge cases,
and removes temporary files unless --keep-run-dir is set.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SHAKESPEARE_PAIRS = Path(
    "~/.contextgraph/training/v4_2_camd_lmdb.jsonl"
).expanduser()
DEFAULT_GO_EMOTIONS = ROOT / "data/utl_calibration/go_emotions/go_emotions_sample.jsonl"
DEFAULT_LMDB = Path("~/.contextgraph/extended_embeddings.lmdb").expanduser()
EVENT_GOOD = "11111111-2222-4333-8444-555555555555"
EVENT_BAD = "66666666-7777-4888-9999-aaaaaaaaaaaa"


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--shakespeare-pairs", type=Path, default=DEFAULT_SHAKESPEARE_PAIRS)
    p.add_argument("--go-emotions", type=Path, default=DEFAULT_GO_EMOTIONS)
    p.add_argument("--lmdb", type=Path, default=DEFAULT_LMDB)
    p.add_argument("--cli", type=Path, default=ROOT / "target/debug/context-graph-cli")
    p.add_argument("--run-dir", type=Path, default=None,
                   help="Optional run directory. Created if missing.")
    p.add_argument("--keep-run-dir", action="store_true",
                   help="Keep run directory for inspection instead of deleting it.")
    p.add_argument("--min-free-gb", type=float, default=1.0,
                   help="Fail before running if filesystem free space is below this.")
    return p.parse_args()


def require_file(path: Path, label: str) -> None:
    if not path.exists() or not path.is_file():
        raise FileNotFoundError(f"{label} missing: {path}")


def require_dir(path: Path, label: str) -> None:
    if not path.exists() or not path.is_dir():
        raise FileNotFoundError(f"{label} missing: {path}")


def run(cmd: list[str], *, allow_fail: bool = False) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        cmd,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if not allow_fail and proc.returncode != 0:
        raise RuntimeError(
            "command failed\n"
            f"cmd={cmd}\n"
            f"exit={proc.returncode}\n"
            f"stdout={proc.stdout}\n"
            f"stderr={proc.stderr}"
        )
    return proc


def json_from_command(cmd: list[str]) -> dict[str, Any]:
    proc = run(cmd)
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"command did not emit JSON: {cmd}\n{proc.stdout}") from exc


def first_jsonl(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            if line.strip():
                return json.loads(line)
    raise ValueError(f"{path} contains no JSONL rows")


def load_go_emotions(path: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    first = None
    second = None
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            if not line.strip():
                continue
            row = json.loads(line)
            if first is None:
                first = row
                continue
            if row.get("label_names") != first.get("label_names"):
                second = row
                break
    if first is None or second is None:
        raise ValueError(f"{path} must contain at least two rows with different labels")
    return first, second


def state(value: float, rank: int, domain: str) -> dict[str, Any]:
    return {
        "topicProfile": [value for _ in range(14)],
        "crossCorrelations": [value * 0.25 for _ in range(91)],
        "retrievalRank": rank,
        "embedderScores": [min(1.0, value + 0.01 * i) for i in range(14)],
        "contradictionPressure": 0.15,
        "integrationConfidence": 0.65,
        "recurrenceCount": 4,
        "stabilityScore": 0.7,
        "domain": domain,
        "successfulTransferCount": 1,
    }


def write_event_input(
    path: Path,
    camd_pair: dict[str, Any],
    emotion_a: dict[str, Any],
    emotion_b: dict[str, Any],
) -> None:
    query = (
        "Use the learner affect evidence to choose the response that preserves "
        "the productive learning window."
    )
    context = "\n".join([
        f"GoEmotions A id={emotion_a['id']} labels={emotion_a['label_names']} text={emotion_a['text']}",
        f"GoEmotions B id={emotion_b['id']} labels={emotion_b['label_names']} text={emotion_b['text']}",
        f"CAMD prompt={camd_pair['prompt']}",
    ])
    rows = [
        {
            "eventId": EVENT_GOOD,
            "memoryIds": ["aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"],
            "sessionId": "realdata-unified-fsv",
            "responseId": "realdata-good",
            "taskId": "realdata-unified-task",
            "query": query,
            "retrievedContext": context,
            "assistantResponse": camd_pair["chosen"],
            "before": state(0.2, 9, "affect_text"),
            "after": state(0.45, 2, "shakespeare_style"),
            "outcome": {
                "label": "useful",
                "utilityDelta": 0.8,
                "correctionRequired": False,
                "reuseObserved": True,
            },
        },
        {
            "eventId": EVENT_BAD,
            "memoryIds": ["bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"],
            "sessionId": "realdata-unified-fsv",
            "responseId": "realdata-bad",
            "taskId": "realdata-unified-task",
            "query": query,
            "retrievedContext": context,
            "assistantResponse": camd_pair["rejected"],
            "before": state(0.2, 9, "affect_text"),
            "after": state(0.1, 14, "shakespeare_style"),
            "outcome": {
                "label": "harmful",
                "utilityDelta": -0.2,
                "correctionRequired": True,
                "reuseObserved": False,
            },
        },
    ]
    with path.open("w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False, sort_keys=True))
            f.write("\n")


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    with path.open("r", encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def inspect_lmdb(path: Path, limit: int = 3) -> dict[str, Any]:
    sys.path.insert(0, str(ROOT / "scripts"))
    from v4_2.extended_store import ExtendedStore  # noqa: WPS433

    with ExtendedStore(path, readonly=True, create=False) as store:
        ids = list(store.iter_memory_ids())
        if not ids:
            raise RuntimeError(f"extended LMDB has zero memory ids: {path}")
        samples = []
        for mid in ids[:limit]:
            meta = store.get_meta(mid)
            dense = store.get_dense(mid, embedder="bge_m3_dense")
            if dense.shape != (1024,):
                raise RuntimeError(f"{mid} bge_m3_dense shape is {dense.shape}, expected (1024,)")
            samples.append({
                "memory_id": mid,
                "content_length": meta.content_length,
                "content_sha256_prefix": meta.content_sha256[:16],
                "dense_shape": list(dense.shape),
                "dense_dtype": str(dense.dtype),
            })
    return {
        "source_of_truth": str(path),
        "records": len(ids),
        "samples": samples,
    }


def count_rows(path: Path) -> int:
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def main() -> int:
    args = parse_args()
    require_file(args.shakespeare_pairs, "Shakespeare CAMD pairs")
    require_file(args.go_emotions, "GoEmotions sample")
    require_dir(args.lmdb, "Extended embeddings LMDB")
    require_file(args.cli, "context-graph-cli binary")

    free_gb = shutil.disk_usage(ROOT).free / (1024 ** 3)
    if free_gb < args.min_free_gb:
        raise RuntimeError(f"free disk {free_gb:.2f}GB < required {args.min_free_gb:.2f}GB")

    if args.run_dir is None:
        temp_ctx = tempfile.TemporaryDirectory(prefix="contextgraph-unified-fsv.")
        run_dir = Path(temp_ctx.name)
    else:
        temp_ctx = None
        run_dir = args.run_dir
        run_dir.mkdir(parents=True, exist_ok=True)

    try:
        storage = run_dir / "rocksdb"
        events_input = run_dir / "events_input.jsonl"
        exported_events = run_dir / "events_exported.jsonl"
        learner_pairs = run_dir / "learner_pairs.jsonl"
        camd_sample = run_dir / "camd_sample.jsonl"
        limit_export = run_dir / "limit_export.jsonl"
        empty_import = run_dir / "empty_import.jsonl"
        invalid_import = run_dir / "invalid_import.jsonl"

        camd_pair = first_jsonl(args.shakespeare_pairs)
        emotion_a, emotion_b = load_go_emotions(args.go_emotions)
        write_event_input(events_input, camd_pair, emotion_a, emotion_b)
        camd_sample.write_text(json.dumps({
            "prompt": camd_pair["prompt"],
            "chosen": camd_pair["chosen"],
            "rejected": camd_pair["rejected"],
            "meta": camd_pair.get("meta", {}),
        }, ensure_ascii=False) + "\n", encoding="utf-8")

        print(f"SOURCE OF TRUTH learning_events=RocksDB:{storage}")
        before_count = json_from_command([str(args.cli), "learning", "count", "--storage", str(storage)])
        import_result = json_from_command([
            str(args.cli), "learning", "record-events-jsonl",
            "--storage", str(storage),
            "--input", str(events_input),
            "--clear-existing",
        ])
        after_import_count = json_from_command([
            str(args.cli), "learning", "count", "--storage", str(storage)
        ])
        good_readback = json_from_command([
            str(args.cli), "learning", "get",
            "--storage", str(storage),
            "--event-id", EVENT_GOOD,
            "--include-text", "true",
        ])

        export_result = json_from_command([
            str(args.cli), "learning", "export-events-jsonl",
            "--storage", str(storage),
            "--out", str(exported_events),
        ])
        exported_rows = read_jsonl(exported_events)

        learner_proc = run([
            sys.executable,
            str(ROOT / "scripts/v4_2/build_learner_dpo_pairs.py"),
            "--events", str(exported_events),
            "--output", str(learner_pairs),
            "--overwrite",
            "--min-gap", "0.5",
        ])
        learner_summary = json.loads(learner_proc.stdout)
        learner_pair_rows = read_jsonl(learner_pairs)

        trainer_proc = run([
            sys.executable,
            str(ROOT / "scripts/v4_2/train_camd_dpo.py"),
            "--pairs", str(camd_sample),
            "--learner-pairs", str(learner_pairs),
            "--learner-repeat", "2",
            "--dry-run",
        ])

        lmdb_evidence = inspect_lmdb(args.lmdb)

        # Edge 1: empty import must fail and leave DB unchanged.
        empty_import.write_text("", encoding="utf-8")
        empty_before = json_from_command([
            str(args.cli), "learning", "count", "--storage", str(storage)
        ])
        empty_proc = run([
            str(args.cli), "learning", "record-events-jsonl",
            "--storage", str(storage),
            "--input", str(empty_import),
        ], allow_fail=True)
        empty_after = json_from_command([
            str(args.cli), "learning", "count", "--storage", str(storage)
        ])

        # Edge 2: export limit should write one row and not mutate DB.
        limit_before = json_from_command([
            str(args.cli), "learning", "count", "--storage", str(storage)
        ])
        limit_result = json_from_command([
            str(args.cli), "learning", "export-events-jsonl",
            "--storage", str(storage),
            "--out", str(limit_export),
            "--limit", "1",
        ])
        limit_after = json_from_command([
            str(args.cli), "learning", "count", "--storage", str(storage)
        ])

        # Edge 3: invalid cross-correlation length must fail before write.
        bad_event = read_jsonl(events_input)[0]
        bad_event["eventId"] = "99999999-9999-4999-8999-999999999999"
        bad_event["before"]["crossCorrelations"] = bad_event["before"]["crossCorrelations"][:-1]
        invalid_import.write_text(json.dumps(bad_event, ensure_ascii=False) + "\n", encoding="utf-8")
        invalid_before = json_from_command([
            str(args.cli), "learning", "count", "--storage", str(storage)
        ])
        invalid_proc = run([
            str(args.cli), "learning", "record-events-jsonl",
            "--storage", str(storage),
            "--input", str(invalid_import),
        ], allow_fail=True)
        invalid_after = json_from_command([
            str(args.cli), "learning", "count", "--storage", str(storage)
        ])

        evidence = {
            "status": "ok",
            "run_dir": str(run_dir),
            "cleanup_planned": not args.keep_run_dir,
            "datasets": {
                "shakespeare_pairs": {
                    "path": str(args.shakespeare_pairs),
                    "sample_prompt": camd_pair["prompt"][:100],
                    "chosen_len": len(camd_pair["chosen"]),
                    "rejected_len": len(camd_pair["rejected"]),
                },
                "go_emotions": {
                    "path": str(args.go_emotions),
                    "rows_sampled": [
                        {"id": emotion_a["id"], "labels": emotion_a["label_names"]},
                        {"id": emotion_b["id"], "labels": emotion_b["label_names"]},
                    ],
                },
            },
            "happy_path": {
                "before_count": before_count["count"],
                "imported": import_result["imported"],
                "after_import_count": after_import_count["count"],
                "good_readback": {
                    "found": good_readback["found"],
                    "event_id": good_readback["event_id"],
                    "query_prefix": good_readback["query"][:80],
                    "delta_e_scalar": good_readback["features"]["delta_e_scalar"],
                    "utility_delta": good_readback["outcome"]["utility_delta"],
                },
                "export_records_written": export_result["records_written"],
                "exported_event_ids": [row["event_id"] for row in exported_rows],
                "exported_tensor_lens": [row["feature_tensor_len"] for row in exported_rows],
                "learner_pair_summary": learner_summary,
                "learner_pair_count": len(learner_pair_rows),
                "learner_pair": {
                    "chosen_event_id": learner_pair_rows[0]["meta"]["chosen_event_id"],
                    "rejected_event_id": learner_pair_rows[0]["meta"]["rejected_event_id"],
                    "learner_reward_gap": learner_pair_rows[0]["meta"]["learner_reward_gap"],
                },
                "trainer_dry_run_tail": trainer_proc.stdout.strip().splitlines()[-5:],
            },
            "lmdb": lmdb_evidence,
            "edge_cases": {
                "empty_import": {
                    "before_count": empty_before["count"],
                    "exit_code": empty_proc.returncode,
                    "after_count": empty_after["count"],
                    "stderr_contains_zero_rows": "zero LearningEvent rows" in empty_proc.stderr,
                },
                "limit_export": {
                    "before_count": limit_before["count"],
                    "records_written": limit_result["records_written"],
                    "jsonl_rows": count_rows(limit_export),
                    "after_count": limit_after["count"],
                },
                "invalid_dimension": {
                    "before_count": invalid_before["count"],
                    "exit_code": invalid_proc.returncode,
                    "after_count": invalid_after["count"],
                    "stderr_contains_dimension": "Dimension mismatch" in invalid_proc.stderr
                    or "expected 91" in invalid_proc.stderr,
                },
            },
        }

        if evidence["happy_path"]["after_import_count"] != 2:
            raise RuntimeError("expected two persisted learning events")
        if evidence["happy_path"]["exported_tensor_lens"] != [149, 149]:
            raise RuntimeError("expected two 149-column event tensors")
        if evidence["happy_path"]["learner_pair_count"] != 1:
            raise RuntimeError("expected exactly one learner DPO pair")
        if evidence["edge_cases"]["empty_import"]["exit_code"] == 0:
            raise RuntimeError("empty import unexpectedly succeeded")
        if evidence["edge_cases"]["invalid_dimension"]["exit_code"] == 0:
            raise RuntimeError("invalid dimension import unexpectedly succeeded")

        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 0
    finally:
        if temp_ctx is not None and not args.keep_run_dir:
            temp_ctx.cleanup()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(json.dumps({"status": "error", "error": str(exc)}, sort_keys=True), file=sys.stderr)
        raise
