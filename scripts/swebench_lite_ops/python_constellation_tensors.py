"""TASK-PY-G-088 cross-embedder constellation tensor materializer."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError
from .python_constellation_tensor_builders import (
    fail,
    finite_float,
    group_records,
    pair_support_state,
    pairwise_tensor_rows,
    parse_embedder_set,
    require,
    slot_distribution_rows,
    surface_summary,
)
from .python_constellation_training_objective import (
    DEFAULT_CORPUS_INDEX_JSON,
    ConstellationTrainingObjectiveError,
    load_oracle_index,
)
from .python_real_corpus_constellation_smoke import (
    DEFAULT_CHUNKS_JSONL,
    DEFAULT_FORWARD_ROOT,
    RealCorpusConstellationSmokeError,
    load_chunk_manifest_window,
    load_sample_records,
    select_completed_shards,
    write_jsonl,
)
from .smoke_intelligence_matrices import dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-088"
ARTIFACT_KIND = "python_constellation_cross_embedder_tensors"
FORMULA_VERSION = "slot_preserving_cross_embedder_tensor_v1"
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/constellation-tensors-v1")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-088-cross-embedder-correlation-tensors-fsv")
EXPECTED_ACTIVE_EMBEDDERS = ("e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14")


def parse_utc(value: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        fail("MEJEPA_CONSTELLATION_TENSORS_BAD_UTC_TIMESTAMP", f"bad UTC timestamp: {value}", {"value": value})
        raise AssertionError("unreachable") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def validate_freshness(args: argparse.Namespace, created_at_utc: str) -> dict[str, Any]:
    now = datetime.now(timezone.utc)
    created = parse_utc(created_at_utc)
    stale_after = parse_utc(args.stale_after_utc) if args.stale_after_utc else created + timedelta(days=args.tensor_ttl_days)
    require(
        created <= now + timedelta(seconds=args.max_future_skew_seconds),
        "MEJEPA_CONSTELLATION_TENSORS_FUTURE_CREATED_AT",
        f"created_at_utc={created.isoformat()} now={now.isoformat()}",
    )
    require(
        stale_after > now,
        "MEJEPA_CONSTELLATION_TENSORS_STALE_METADATA",
        f"stale_after_utc={stale_after.isoformat()} now={now.isoformat()}",
    )
    return {
        "generated_at_utc": created.isoformat(timespec="seconds"),
        "freshness_checked_at_utc": now.isoformat(timespec="seconds"),
        "stale_after_utc": stale_after.isoformat(timespec="seconds"),
        "tensor_ttl_days": args.tensor_ttl_days,
        "fresh_at_readback": True,
        "stale_tensor_policy": "fail_closed_before_prediction_consumption",
    }


def preflight_sources(args: argparse.Namespace) -> None:
    require(args.forward_root.exists(), "MEJEPA_CONSTELLATION_TENSORS_FORWARD_ROOT_MISSING", str(args.forward_root))
    require(args.chunks_jsonl.exists(), "MEJEPA_CONSTELLATION_TENSORS_CHUNKS_JSONL_MISSING", str(args.chunks_jsonl))
    require(args.corpus_index_json.exists(), "MEJEPA_CONSTELLATION_TENSORS_ORACLE_INDEX_MISSING", str(args.corpus_index_json))
    require(args.sample_shards > 0, "MEJEPA_CONSTELLATION_TENSORS_INVALID_SAMPLE_SHARDS", str(args.sample_shards))
    require(args.sample_rows_per_embedder_per_shard > 0, "MEJEPA_CONSTELLATION_TENSORS_INVALID_SAMPLE_ROWS", str(args.sample_rows_per_embedder_per_shard))
    require(args.min_overlap > 0, "MEJEPA_CONSTELLATION_TENSORS_INVALID_MIN_OVERLAP", str(args.min_overlap))
    require(args.mi_bins >= 3, "MEJEPA_CONSTELLATION_TENSORS_INVALID_MI_BINS", str(args.mi_bins))
    require(0.5 <= args.surface_quantile < 1.0, "MEJEPA_CONSTELLATION_TENSORS_INVALID_SURFACE_QUANTILE", str(args.surface_quantile))


def built_in_edge_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    edge_specs = [
        ("empty_embedder_set", lambda: parse_embedder_set(""), "MEJEPA_CONSTELLATION_TENSORS_EMPTY_EMBEDDER_SET"),
        ("duplicate_embedder_set", lambda: parse_embedder_set("e1,e1"), "MEJEPA_CONSTELLATION_TENSORS_DUPLICATE_EMBEDDER"),
        ("nonfinite_scalar_rejected", lambda: finite_float(float("nan")), "MEJEPA_CONSTELLATION_TENSORS_NONFINITE_SCALAR"),
    ]
    for name, fn, expected_code in edge_specs:
        try:
            fn()
            cases.append({"name": name, "passes": False, "expected_error_code": expected_code, "observed": "NO_ERROR"})
        except SwebenchOpsError as exc:
            cases.append({"name": name, "passes": exc.code == expected_code, "expected_error_code": expected_code, "observed_error_code": exc.code})
    stale_args = argparse.Namespace(tensor_ttl_days=1, stale_after_utc="2000-01-01T00:00:00+00:00", max_future_skew_seconds=1)
    try:
        validate_freshness(stale_args, "2000-01-01T00:00:00+00:00")
        cases.append({"name": "stale_freshness_metadata", "passes": False, "expected_error_code": "MEJEPA_CONSTELLATION_TENSORS_STALE_METADATA"})
    except SwebenchOpsError as exc:
        cases.append({"name": "stale_freshness_metadata", "passes": exc.code == "MEJEPA_CONSTELLATION_TENSORS_STALE_METADATA", "expected_error_code": "MEJEPA_CONSTELLATION_TENSORS_STALE_METADATA", "observed_error_code": exc.code})
    support = pair_support_state(left_count=2, right_count=2, overlap=1, min_overlap=3)
    cases.append({"name": "insufficient_pair_support_is_invalid", "passes": support["valid"] is False and support["invalid_reason"] == "TOO_FEW_OVERLAP_ROWS", "support": support})
    return cases


def build_report(args: argparse.Namespace, output_root: Path, context: dict[str, Any], pairwise_rows: list[dict[str, Any]], edge_cases: list[dict[str, Any]]) -> dict[str, Any]:
    load_summary = context["load_summary"]
    slot_summary = context["slot_summary"]
    pairwise_summary = context["pairwise_summary"]
    freshness = context["freshness"]
    passes = (
        load_summary["index_hash_mismatches"] == 0
        and load_summary["source_readback_unverified"] == 0
        and slot_summary["valid_slot_distribution_rows"] > 0
        and pairwise_summary["full_supported_cell_windows"] > 0
        and pairwise_summary["valid_pairwise_tensor_rows"] >= pairwise_summary["expected_valid_pairs_for_full_supported_windows"]
        and freshness.get("fresh_at_readback") is True
        and all(case.get("passes") is True for case in edge_cases)
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "formula_version": FORMULA_VERSION,
        "created_at_utc": context["created_at_utc"],
        "passes": passes,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "corpus_index_json": str(args.corpus_index_json),
            "output_root": str(output_root),
        },
        "constellation_policy": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "active_embedder_slots": list(args.required_embedders),
            "expected_active_embedder_count": args.expected_embedder_count,
            "declared_frozen_instrument_panel_slots": 15,
            "instrument_slot_policy": "emit matrices for the 12 active forward-cache slots present in this corpus; inactive/frozen-only slots are not synthesized",
            "expected_pairs_per_full_cell_window": pairwise_summary["expected_pairs_per_full_cell_window"],
            "flat_5120d_panel_policy": "storage_layout_only_not_prediction_input",
        },
        "freshness": freshness,
        "summary": {"sampled_record_count": load_summary["sampled_record_count"], **context["group_summary"], **slot_summary, **pairwise_summary},
        "source_summary": context["source_summary"],
        "oracle_summary": context["oracle_summary"],
        "chunk_summary": context["chunk_summary"],
        "load_summary": load_summary,
        "surfaces": surface_summary(pairwise_rows, args.top_k),
        "edge_cases": edge_cases,
    }


def load_source_records(args: argparse.Namespace, required_embedders: tuple[str, ...]) -> dict[str, Any]:
    try:
        selected_shards, source_summary = select_completed_shards(args.forward_root, args.sample_shards)
        shard_order = {str(shard.get("shard_id")): idx for idx, shard in enumerate(selected_shards)}
        oracle_index, oracle_summary = load_oracle_index(args.corpus_index_json)
        manifest_by_key, chunk_summary = load_chunk_manifest_window(args.chunks_jsonl, selected_shards)
        records, load_summary = load_sample_records(args.forward_root, selected_shards, manifest_by_key, required_embedders, args.sample_rows_per_embedder_per_shard)
    except (RealCorpusConstellationSmokeError, ConstellationTrainingObjectiveError) as exc:
        fail("MEJEPA_CONSTELLATION_TENSORS_SOURCE_READ_FAILED", str(exc), {"type": type(exc).__name__})
    return {
        "records": records,
        "oracle_index": oracle_index,
        "source_summary": source_summary,
        "oracle_summary": oracle_summary,
        "chunk_summary": chunk_summary,
        "load_summary": load_summary,
        "shard_order": shard_order,
    }


def command_python_constellation_tensors(args: argparse.Namespace) -> int:
    started = utc_now()
    created_at_utc = utc_now()
    required_embedders = parse_embedder_set(args.expected_embedders)
    require(args.expected_embedder_count == len(required_embedders), "MEJEPA_CONSTELLATION_TENSORS_EXPECTED_COUNT_MISMATCH", str(required_embedders))
    args.required_embedders = required_embedders
    args.schema_version = SCHEMA_VERSION
    args.formula_version = FORMULA_VERSION
    preflight_sources(args)
    freshness = validate_freshness(args, created_at_utc)

    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    loaded = load_source_records(args, required_embedders)
    records = loaded["records"]
    require(len(records) == args.expected_embedder_count, "MEJEPA_CONSTELLATION_TENSORS_ACTIVE_EMBEDDER_COUNT_MISMATCH", str(sorted(records)))
    args.shard_order = loaded["shard_order"]
    corpus_id = args.corpus_id or args.forward_root.name
    run_id = args.run_id or loaded["source_summary"].get("checkpoint_summary", {}).get("run_id") or args.forward_root.name
    grouped, group_summary = group_records(records, loaded["oracle_index"], args.shard_order)
    slot_rows, slot_summary = slot_distribution_rows(grouped, required_embedders, args, created_at_utc, corpus_id, str(run_id))
    pairwise_rows, pairwise_summary = pairwise_tensor_rows(grouped, required_embedders, args, created_at_utc, corpus_id, str(run_id))
    edge_cases = built_in_edge_cases()

    slot_path = args.output_root / "slot_distribution_tensors.jsonl"
    pairwise_path = args.output_root / "pairwise_correlation_tensors.jsonl"
    report_path = args.output_root / "constellation_tensor_report.json"
    manifest_path = args.output_root / "constellation_tensor_manifest.json"
    write_jsonl(slot_path, slot_rows)
    write_jsonl(pairwise_path, pairwise_rows)

    context = {**loaded, "created_at_utc": created_at_utc, "freshness": freshness, "group_summary": group_summary, "slot_summary": slot_summary, "pairwise_summary": pairwise_summary}
    report = build_report(args, args.output_root, context, pairwise_rows, edge_cases)
    dump_json(report_path, report)
    fsv = write_manifest_and_fsv(args, started, before, slot_rows, pairwise_rows, slot_path, pairwise_path, report_path, manifest_path, edge_cases)
    dump_json(args.fsv_root / f"{ARTIFACT_KIND}_fsv.json", fsv)
    final_readback = read_json(args.fsv_root / f"{ARTIFACT_KIND}_fsv.json")
    require(final_readback.get("passes") is True, "MEJEPA_CONSTELLATION_TENSORS_FSV_FAILED", str(args.fsv_root))
    print(json.dumps(final_readback, sort_keys=True, indent=2))
    return 0


def write_manifest_and_fsv(
    args: argparse.Namespace,
    started: str,
    before: dict[str, Any],
    slot_rows: list[dict[str, Any]],
    pairwise_rows: list[dict[str, Any]],
    slot_path: Path,
    pairwise_path: Path,
    report_path: Path,
    manifest_path: Path,
    edge_cases: list[dict[str, Any]],
) -> dict[str, Any]:
    readback_slots = sum(1 for _ in iter_jsonl(slot_path))
    readback_pairwise = sum(1 for _ in iter_jsonl(pairwise_path))
    stable_hashes = {"slot_distribution_tensors": sha256_file(slot_path), "pairwise_correlation_tensors": sha256_file(pairwise_path), "constellation_tensor_report": sha256_file(report_path)}
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "paths": {"slot_distribution_tensors": str(slot_path), "pairwise_correlation_tensors": str(pairwise_path), "constellation_tensor_report": str(report_path)},
        "line_counts": {"slot_distribution_tensors": readback_slots, "pairwise_correlation_tensors": readback_pairwise},
        "stable_hashes": stable_hashes,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
    }
    dump_json(manifest_path, manifest)
    stable_hashes["constellation_tensor_manifest"] = sha256_file(manifest_path)
    readback_report = read_json(report_path)
    readback_manifest = read_json(manifest_path)
    passes = readback_report.get("passes") is True and readback_manifest.get("flat_vector_concat_used") is False and readback_slots == len(slot_rows) and readback_pairwise == len(pairwise_rows) and readback_pairwise > 0
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "corpus_index_json": str(args.corpus_index_json),
            "output_root": str(args.output_root),
            "slot_distribution_tensors": str(slot_path),
            "pairwise_correlation_tensors": str(pairwise_path),
            "constellation_tensor_report": str(report_path),
            "constellation_tensor_manifest": str(manifest_path),
            "fsv": str(args.fsv_root / f"{ARTIFACT_KIND}_fsv.json"),
        },
        "before_state": before,
        "after_state": {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)},
        "readback": {
            "slot_distribution_rows": readback_slots,
            "pairwise_tensor_rows": readback_pairwise,
            "expected_slot_distribution_rows": len(slot_rows),
            "expected_pairwise_tensor_rows": len(pairwise_rows),
            "report_passes": readback_report.get("passes"),
            "manifest_flat_vector_concat_used": readback_manifest.get("flat_vector_concat_used"),
            "stable_hashes": stable_hashes,
        },
        "summary": readback_report.get("summary"),
        "surfaces": readback_report.get("surfaces"),
        "edge_cases": edge_cases,
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--corpus-index-json", type=Path, default=DEFAULT_CORPUS_INDEX_JSON)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--corpus-id", default=None)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_ACTIVE_EMBEDDERS))
    parser.add_argument("--expected-embedder-count", type=int, default=len(EXPECTED_ACTIVE_EMBEDDERS))
    parser.add_argument("--sample-shards", type=int, default=8)
    parser.add_argument("--sample-rows-per-embedder-per-shard", type=int, default=80)
    parser.add_argument("--min-slot-rows", type=int, default=2)
    parser.add_argument("--min-overlap", type=int, default=2)
    parser.add_argument("--mi-bins", type=int, default=8)
    parser.add_argument("--surface-quantile", type=float, default=0.85)
    parser.add_argument("--top-k", type=int, default=20)
    parser.add_argument("--tensor-ttl-days", type=int, default=14)
    parser.add_argument("--stale-after-utc", default=None)
    parser.add_argument("--max-future-skew-seconds", type=int, default=300)
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_constellation_tensors(args)
    except SwebenchOpsError as exc:
        print(f"PYTHON_CONSTELLATION_TENSORS_FAILED: {exc.code}: {exc.message}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
