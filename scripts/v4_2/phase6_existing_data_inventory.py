#!/usr/bin/env python3
"""Inventory existing Phase 6 / Big Unlock evidence.

This script does not infer state from filenames alone. It reads physical
RocksDB column-family counts with `ldb`, inspects JSONL training/export files,
and summarizes model run metadata already present on disk.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_REPORT = Path("~/.contextgraph/reports/phase6_existing_data_inventory_latest.json").expanduser()
CONTEXT_ROOTS = [
    Path("~/.contextgraph").expanduser(),
    ROOT,
]
ROCKSDB_CFS = [
    "fingerprints",
    "content",
    "source_metadata",
    "embedding_registry",
    "training_records",
    "constellations",
    "constellation_by_selector",
    "contrastive_pairs",
    "contrastive_by_kind",
    "contrastive_by_anchor",
    "learning_events",
    "learner_training_datasets",
    "learner_profile",
    "fingerprints_learner",
    "learner_state_history",
    "learner_constellations",
    "learner_audit",
    "learner_delta_log",
    "learner_m_per_trace",
    "learner_retrieval_log",
    "learner_k_sleep",
]


class InventoryFailure(RuntimeError):
    """Raised for fail-closed inventory errors."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def path_state(path: Path, *, hash_limit_bytes: int = 512 * 1024 * 1024) -> dict[str, Any]:
    if not path.exists():
        state: dict[str, Any] = {"path": str(path), "exists": False}
        if path.is_symlink():
            state["kind"] = "broken_symlink"
            state["link_target"] = os.readlink(path)
        return state
    state: dict[str, Any] = {
        "path": str(path),
        "exists": True,
        "mtime": datetime.fromtimestamp(path.stat().st_mtime, timezone.utc).isoformat(timespec="seconds"),
    }
    if path.is_file():
        size_bytes = path.stat().st_size
        state.update({"kind": "file", "size_bytes": size_bytes})
        if size_bytes <= hash_limit_bytes:
            state["sha256"] = sha256_file(path)
        else:
            state["sha256_skipped"] = f"file larger than {hash_limit_bytes} bytes"
    elif path.is_dir():
        total = 0
        files = 0
        for root, _, names in os.walk(path):
            for name in names:
                files += 1
                total += (Path(root) / name).stat().st_size
        state.update({"kind": "directory", "file_count": files, "size_bytes": total})
    else:
        state["kind"] = "other"
    return state


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    tmp.replace(path)


def run(command: list[str], *, timeout: int) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def summarize_json_value(value: Any, *, depth: int = 0) -> Any:
    if depth >= 4:
        return f"<{type(value).__name__}>"
    if isinstance(value, dict):
        return {str(key): summarize_json_value(val, depth=depth + 1) for key, val in list(value.items())[:20]}
    if isinstance(value, list):
        return [summarize_json_value(item, depth=depth + 1) for item in value[:3]]
    if isinstance(value, str):
        return {"type": "str", "len": len(value), "preview": value[:240]}
    return value


def discover_rocksdb_dirs(roots: list[Path]) -> list[Path]:
    found: set[Path] = set()
    for root in roots:
        if not root.exists():
            continue
        for marker in ("CURRENT", "IDENTITY"):
            for path in root.rglob(marker):
                parent = path.parent
                if (parent / "CURRENT").exists() or (parent / "IDENTITY").exists():
                    found.add(parent)
    return sorted(found)


def ldb_count(db: Path, cf: str, timeout: int) -> dict[str, Any]:
    command = ["ldb", f"--db={db}", f"--column_family={cf}", "dump", "--count_only"]
    try:
        proc = run(command, timeout=timeout)
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": f"timeout after {timeout}s", "command": command}
    output = f"{proc.stdout}\n{proc.stderr}"
    match = re.search(r"Keys in range:\s*(\d+)", output)
    if proc.returncode == 0 and match:
        return {"ok": True, "count": int(match.group(1)), "command": command}
    compact = " ".join(output.strip().split())
    return {
        "ok": False,
        "returncode": proc.returncode,
        "error": compact[:500],
        "command": command,
    }


def rocksdb_inventory(dbs: list[Path], *, timeout: int) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for db in dbs:
        counts: dict[str, int] = {}
        errors: dict[str, str] = {}
        for cf in ROCKSDB_CFS:
            result = ldb_count(db, cf, timeout)
            if result["ok"]:
                count = int(result["count"])
                if count:
                    counts[cf] = count
            else:
                error = str(result.get("error") or "")
                if "Column family not found" not in error and "does not exist" not in error:
                    errors[cf] = error
        if counts or errors:
            rows.append({"db": str(db), "state": path_state(db), "counts": counts, "errors": errors})
    return rows


def jsonl_summary(path: Path, *, sample_lines: int = 1) -> dict[str, Any]:
    if not path.exists():
        state = path_state(path)
        state.update(
            {
                "line_count": None,
                "top_level_keys": [],
                "meta_keys": [],
                "outcome_labels": {},
                "session_counts": {},
                "first_records": [],
                "error": "path is missing or is a broken symlink",
            }
        )
        return state
    line_count = 0
    keys: set[str] = set()
    meta_keys: set[str] = set()
    outcome_labels: dict[str, int] = {}
    sessions: dict[str, int] = {}
    first_records: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            line_count += 1
            if line_count <= sample_lines:
                try:
                    first_records.append(summarize_json_value(json.loads(line)))
                except json.JSONDecodeError as exc:
                    first_records.append({"json_error": str(exc), "preview": line[:500]})
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            keys.update(str(key) for key in row.keys())
            meta = row.get("meta") or row.get("_meta")
            if isinstance(meta, dict):
                meta_keys.update(str(key) for key in meta.keys())
            outcome = row.get("outcome")
            if isinstance(outcome, dict) and outcome.get("label") is not None:
                label = str(outcome["label"])
                outcome_labels[label] = outcome_labels.get(label, 0) + 1
            if row.get("session_id") is not None:
                session = str(row["session_id"])
                sessions[session] = sessions.get(session, 0) + 1
    state = path_state(path)
    state.update(
        {
            "line_count": line_count,
            "top_level_keys": sorted(keys),
            "meta_keys": sorted(meta_keys),
            "outcome_labels": outcome_labels,
            "session_counts": dict(sorted(sessions.items(), key=lambda item: (-item[1], item[0]))[:20]),
            "first_records": first_records,
        }
    )
    return state


def discover_jsonl(paths: list[Path]) -> list[Path]:
    found: set[Path] = set()
    for path in paths:
        if path.is_file() and path.suffix == ".jsonl":
            found.add(path)
        elif path.is_dir():
            for child in path.rglob("*.jsonl"):
                found.add(child)
    return sorted(found)


def trainer_state_summary(path: Path) -> dict[str, Any]:
    state = path_state(path)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        state["json_error"] = str(exc)
        return state
    logs = payload.get("log_history")
    state["global_step"] = payload.get("global_step")
    if isinstance(logs, list):
        losses = [row.get("loss") for row in logs if isinstance(row, dict) and isinstance(row.get("loss"), (int, float))]
        eval_losses = [
            row.get("eval_loss")
            for row in logs
            if isinstance(row, dict) and isinstance(row.get("eval_loss"), (int, float))
        ]
        state["log_history_rows"] = len(logs)
        if losses:
            state["first_loss"] = losses[0]
            state["last_loss"] = losses[-1]
            state["min_loss"] = min(losses)
        if eval_losses:
            state["first_eval_loss"] = eval_losses[0]
            state["last_eval_loss"] = eval_losses[-1]
            state["min_eval_loss"] = min(eval_losses)
    return state


def training_fsv_summary(path: Path) -> dict[str, Any]:
    state = path_state(path)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        state["json_error"] = str(exc)
        return state
    state["completed_at"] = payload.get("completed_at")
    state["failed_at"] = payload.get("failed_at")
    state["error"] = payload.get("error")
    state["config"] = payload.get("config")
    post_state = payload.get("post_state")
    if isinstance(post_state, dict):
        smoke = post_state.get("generation_smoke")
        if isinstance(smoke, dict):
            state["generation_smoke"] = {
                "line_count": smoke.get("line_count"),
                "failures": smoke.get("failures"),
                "path": smoke.get("path"),
                "sha256": smoke.get("sha256"),
            }
        final_files = post_state.get("final_files")
        if isinstance(final_files, dict):
            adapter = final_files.get("adapter_model")
            if isinstance(adapter, dict):
                state["final_adapter"] = adapter
    return state


def model_inventory(model_roots: list[Path]) -> dict[str, Any]:
    adapters: list[dict[str, Any]] = []
    trainer_states: list[dict[str, Any]] = []
    training_fsvs: list[dict[str, Any]] = []
    generation_smokes: list[dict[str, Any]] = []
    for root in model_roots:
        if not root.exists():
            continue
        for path in root.rglob("adapter_model.safetensors"):
            adapters.append(path_state(path))
        for path in root.rglob("trainer_state.json"):
            trainer_states.append(trainer_state_summary(path))
        for path in root.rglob("training_fsv.json"):
            training_fsvs.append(training_fsv_summary(path))
        for path in root.rglob("generation_smoke.jsonl"):
            generation_smokes.append(jsonl_summary(path, sample_lines=1))
    return {
        "adapters": sorted(adapters, key=lambda row: row["path"]),
        "trainer_states": sorted(trainer_states, key=lambda row: row["path"]),
        "training_fsvs": sorted(training_fsvs, key=lambda row: row["path"]),
        "generation_smokes": sorted(generation_smokes, key=lambda row: row["path"]),
    }


def classify_inventory(rocksdb_rows: list[dict[str, Any]], jsonl_rows: list[dict[str, Any]]) -> dict[str, Any]:
    max_learning = max((row["counts"].get("learning_events", 0) for row in rocksdb_rows), default=0)
    max_learner_datasets = max((row["counts"].get("learner_training_datasets", 0) for row in rocksdb_rows), default=0)
    max_contrastive = max((row["counts"].get("contrastive_pairs", 0) for row in rocksdb_rows), default=0)
    largest_learning_dbs = [
        {"db": row["db"], "learning_events": row["counts"].get("learning_events", 0)}
        for row in rocksdb_rows
        if row["counts"].get("learning_events", 0)
    ]
    largest_learning_dbs.sort(key=lambda row: (-row["learning_events"], row["db"]))
    large_jsonl = [
        {"path": row["path"], "line_count": row["line_count"], "top_level_keys": row["top_level_keys"]}
        for row in jsonl_rows
        if isinstance(row.get("line_count"), int) and row["line_count"] >= 1000
    ]
    return {
        "max_physical_learning_events": max_learning,
        "max_physical_learner_training_datasets": max_learner_datasets,
        "max_physical_contrastive_pairs": max_contrastive,
        "learning_event_dbs": largest_learning_dbs,
        "large_jsonl_training_sets": sorted(large_jsonl, key=lambda row: (-row["line_count"], row["path"])),
        "interpretation": (
            "Existing long-run data is present as model-training JSONL/adapters. "
            "The largest persisted learner-outcome RocksDB store found is small; "
            "use it for policy benchmarking, not as a large learner DPO corpus."
        ),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--roots", nargs="*", type=Path, default=CONTEXT_ROOTS)
    parser.add_argument("--ldb-timeout-sec", type=int, default=180)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    roots = [path.expanduser().resolve() for path in args.roots]
    report: dict[str, Any] = {
        "status": "running",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "source_of_truth": {
            "rocksdb_counts": "physical RocksDB column families read by ldb",
            "jsonl_training_sets": "existing JSONL files read from disk",
            "model_artifacts": "adapter/trainer/FSV files read from disk",
        },
        "inputs": {
            "roots": [str(path) for path in roots],
            "ldb_timeout_sec": args.ldb_timeout_sec,
        },
    }
    try:
        if shutil.which("ldb") is None:
            raise InventoryFailure("ldb is not available on PATH")
        rocksdb_dirs = discover_rocksdb_dirs(roots)
        report["rocksdb"] = {
            "discovered": [str(path) for path in rocksdb_dirs],
            "stores": rocksdb_inventory(rocksdb_dirs, timeout=args.ldb_timeout_sec),
        }
        jsonl_paths = discover_jsonl(
            [
                Path("~/.contextgraph/training").expanduser(),
                Path("~/.contextgraph/experiments").expanduser(),
            ]
        )
        report["jsonl"] = [jsonl_summary(path, sample_lines=1) for path in jsonl_paths]
        report["lmdb"] = path_state(Path("~/.contextgraph/extended_embeddings.lmdb").expanduser())
        report["models"] = model_inventory(
            [
                Path("~/.contextgraph/models").expanduser(),
                ROOT / "models",
            ]
        )
        report["classification"] = classify_inventory(report["rocksdb"]["stores"], report["jsonl"])
        report["status"] = "ok"
    except Exception as exc:
        report["status"] = "failed"
        report["error"] = f"{type(exc).__name__}: {exc}"
        write_json_atomic(args.report, report)
        print(f"PHASE6 EXISTING DATA INVENTORY FAILED: {exc}", flush=True)
        print(f"REPORT_WRITTEN={args.report}", flush=True)
        return 1

    write_json_atomic(args.report, report)
    print(json.dumps({
        "status": report["status"],
        "report": str(args.report),
        "classification": report["classification"],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
