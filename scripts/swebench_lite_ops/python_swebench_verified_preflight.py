from __future__ import annotations

import argparse
import hashlib
import json
import time
from collections import Counter
from pathlib import Path
from typing import Any, Iterable


DEFAULT_DATASET_NAME = "princeton-nlp/SWE-bench_Verified"
DEFAULT_DATASET_SPLIT = "test"
DEFAULT_EXPECTED_ROWS = 500
REQUIRED_FIELDS = ("instance_id", "repo", "base_commit", "patch", "test_patch")


class SwebenchVerifiedPreflightError(RuntimeError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None) -> None:
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message
        self.details = details or {}


def sha256_text(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with path.open("w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n")
            count += 1
    return count


def reject_mounted_path(path: Path, field: str) -> None:
    text = str(path)
    if text == "/mnt/c" or text == "/mnt/d" or text.startswith("/mnt/c/") or text.startswith("/mnt/d/"):
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_MOUNTED_PATH_FORBIDDEN",
            f"{field} must not be on a mounted Windows drive",
            {"field": field, "path": text},
        )


def parse_test_list(value: Any, field: str, instance_id: str) -> list[str]:
    if value is None:
        return []
    parsed = json.loads(value) if isinstance(value, str) else value
    if not isinstance(parsed, list) or not all(isinstance(item, str) for item in parsed):
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_TEST_LIST_INVALID",
            f"{field} must decode to a list of strings",
            {"instance_id": instance_id, "field": field, "value_type": type(value).__name__},
        )
    return parsed


def canonical_dataset_row(row: dict[str, Any], dataset_name: str, dataset_split: str) -> dict[str, Any]:
    missing = [field for field in REQUIRED_FIELDS if not row.get(field)]
    instance_id = str(row.get("instance_id") or "")
    if missing:
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_REQUIRED_FIELD_MISSING",
            "dataset row is missing required fields",
            {"instance_id": instance_id, "missing": missing},
        )
    fail_to_pass = parse_test_list(row.get("FAIL_TO_PASS"), "FAIL_TO_PASS", instance_id)
    pass_to_pass = parse_test_list(row.get("PASS_TO_PASS"), "PASS_TO_PASS", instance_id)
    patch = str(row.get("patch") or "")
    test_patch = str(row.get("test_patch") or "")
    return {
        "dataset": dataset_name,
        "dataset_split": dataset_split,
        "instance_id": instance_id,
        "repo": str(row["repo"]),
        "base_commit": str(row["base_commit"]),
        "problem_statement_sha256": sha256_text(str(row.get("problem_statement") or "")),
        "oracle_patch_sha256": sha256_text(patch),
        "official_test_patch_sha256": sha256_text(test_patch),
        "fail_to_pass_count": len(fail_to_pass),
        "pass_to_pass_count": len(pass_to_pass),
        "created_at": row.get("created_at"),
        "version": row.get("version"),
        "difficulty": row.get("difficulty"),
        "environment_setup_commit": row.get("environment_setup_commit"),
        "source_of_truth": f"huggingface:{dataset_name}:{dataset_split}:{instance_id}",
    }


def task_manifest(row: dict[str, Any], dataset_name: str, dataset_split: str) -> dict[str, Any]:
    fail_to_pass = parse_test_list(row.get("FAIL_TO_PASS"), "FAIL_TO_PASS", str(row.get("instance_id") or ""))
    pass_to_pass = parse_test_list(row.get("PASS_TO_PASS"), "PASS_TO_PASS", str(row.get("instance_id") or ""))
    return {
        "dataset": dataset_name,
        "dataset_split": dataset_split,
        "instance_id": str(row["instance_id"]),
        "repo": str(row["repo"]),
        "base_commit": str(row["base_commit"]),
        "problem_statement": str(row.get("problem_statement") or ""),
        "fail_to_pass": fail_to_pass,
        "pass_to_pass_count": len(pass_to_pass),
        "official_test_patch_sha256": sha256_text(str(row.get("test_patch") or "")),
        "oracle_patch_sha256": sha256_text(str(row.get("patch") or "")),
        "source_of_truth": f"huggingface:{dataset_name}:{dataset_split}:{row['instance_id']}",
        "verified_metadata": {
            "created_at": row.get("created_at"),
            "version": row.get("version"),
            "difficulty": row.get("difficulty"),
            "environment_setup_commit": row.get("environment_setup_commit"),
        },
    }


def load_dataset_rows(dataset_name: str, dataset_split: str) -> list[dict[str, Any]]:
    from datasets import load_dataset

    ds = load_dataset(dataset_name, split=dataset_split)
    return [dict(row) for row in ds]


def lite_task_ids(lite_tasks_dir: Path | None) -> set[str]:
    if lite_tasks_dir is None or not lite_tasks_dir.exists():
        return set()
    return {path.stem for path in lite_tasks_dir.glob("*.json")}


def artifact_hashes(paths: list[Path]) -> dict[str, str]:
    out: dict[str, str] = {}
    for path in paths:
        out[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return out


def run_preflight(args: argparse.Namespace, rows: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    output_root = Path(args.output_root)
    tasks_output_dir = Path(args.tasks_output_dir) if args.tasks_output_dir else output_root / "tasks"
    reject_mounted_path(output_root, "output_root")
    reject_mounted_path(tasks_output_dir, "tasks_output_dir")
    if output_root.exists() and any(output_root.iterdir()) and not args.allow_existing_output:
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_OUTPUT_NOT_FRESH",
            "output_root exists and is not empty",
            {"output_root": str(output_root)},
        )
    if tasks_output_dir.exists() and any(tasks_output_dir.iterdir()) and not args.allow_existing_output:
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_TASKS_OUTPUT_NOT_FRESH",
            "tasks_output_dir exists and is not empty",
            {"tasks_output_dir": str(tasks_output_dir)},
        )
    output_root.mkdir(parents=True, exist_ok=True)
    tasks_output_dir.mkdir(parents=True, exist_ok=True)

    dataset_rows = rows if rows is not None else load_dataset_rows(args.dataset_name, args.dataset_split)
    if len(dataset_rows) != args.expected_rows:
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_ROW_COUNT_MISMATCH",
            "dataset row count does not match expected Verified size",
            {"expected_rows": args.expected_rows, "actual_rows": len(dataset_rows)},
        )

    canonical_rows = [canonical_dataset_row(row, args.dataset_name, args.dataset_split) for row in dataset_rows]
    ids = [row["instance_id"] for row in canonical_rows]
    duplicate_ids = sorted([instance_id for instance_id, count in Counter(ids).items() if count > 1])
    if duplicate_ids:
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_DUPLICATE_INSTANCE_ID",
            "dataset contains duplicate instance ids",
            {"duplicate_ids": duplicate_ids[:20], "duplicate_count": len(duplicate_ids)},
        )
    test_patch_shas = [row["official_test_patch_sha256"] for row in canonical_rows]
    duplicate_test_patch_shas = sorted([sha for sha, count in Counter(test_patch_shas).items() if count > 1])
    if duplicate_test_patch_shas:
        raise SwebenchVerifiedPreflightError(
            "SWEBENCH_VERIFIED_TEST_PATCH_SHA_COLLISION",
            "official test patch hashes must be unique for instance-atomic split leakage checks",
            {"duplicate_test_patch_shas": duplicate_test_patch_shas[:20]},
        )

    for row in dataset_rows:
        manifest = task_manifest(row, args.dataset_name, args.dataset_split)
        write_json(tasks_output_dir / f"{manifest['instance_id']}.json", manifest)

    canonical_rows_path = output_root / "dataset_rows.canonical.jsonl"
    canonical_count = write_jsonl(canonical_rows_path, canonical_rows)
    generated_task_paths = sorted(tasks_output_dir.glob("*.json"))
    lite_ids = lite_task_ids(Path(args.lite_tasks_dir) if args.lite_tasks_dir else None)
    verified_ids = set(ids)
    overlap = sorted(verified_ids & lite_ids)

    summary = {
        "artifact_kind": "swebench_verified_ingestion_preflight",
        "created_at_unix": int(time.time()),
        "dataset_name": args.dataset_name,
        "dataset_split": args.dataset_split,
        "expected_rows": args.expected_rows,
        "row_count": len(dataset_rows),
        "canonical_row_count": canonical_count,
        "tasks_output_dir": str(tasks_output_dir),
        "generated_task_manifest_count": len(generated_task_paths),
        "unique_instance_ids": len(verified_ids),
        "unique_official_test_patch_sha256": len(set(test_patch_shas)),
        "repo_counts": dict(sorted(Counter(row["repo"] for row in canonical_rows).items())),
        "lite_overlap_count": len(overlap),
        "lite_only_count": len(lite_ids - verified_ids),
        "verified_only_count": len(verified_ids - lite_ids),
        "lite_overlap_sample": overlap[:20],
        "next_corpus_command": (
            "target/release/mejepa-corpus generate "
            f"--output /zfs/archive/contextgraph/corpus/swebench-verified-v1-<TS> "
            f"--tasks-dir {tasks_output_dir} "
            f"--dataset-name {args.dataset_name} --dataset-split {args.dataset_split}"
        ),
        "doctrine": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "inner_llm_used": False,
            "aiwonder_durable_root_required": str(output_root).startswith("/zfs/archive/contextgraph/"),
        },
        "passes": True,
    }
    summary_path = output_root / "swebench_verified_preflight_manifest.json"
    write_json(summary_path, summary)
    hashes_path = output_root / "artifact_hashes.json"
    write_json(hashes_path, artifact_hashes([canonical_rows_path, summary_path]))
    summary["artifact_hashes_path"] = str(hashes_path)
    return summary


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="SWE-bench Verified ingestion preflight/materializer")
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET_NAME)
    parser.add_argument("--dataset-split", default=DEFAULT_DATASET_SPLIT)
    parser.add_argument("--expected-rows", type=int, default=DEFAULT_EXPECTED_ROWS)
    parser.add_argument("--output-root", required=True)
    parser.add_argument("--tasks-output-dir")
    parser.add_argument("--lite-tasks-dir", default="tasks/swebench-lite")
    parser.add_argument("--allow-existing-output", action="store_true")
    return parser


def main() -> int:
    parser = build_arg_parser()
    args = parser.parse_args()
    try:
        summary = run_preflight(args)
    except SwebenchVerifiedPreflightError as exc:
        payload = {"passes": False, "error_code": exc.code, "message": exc.message, "details": exc.details}
        print(json.dumps(payload, sort_keys=True))
        return 2
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
