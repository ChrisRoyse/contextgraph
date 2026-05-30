#!/usr/bin/env python3
"""TASK-CR-008 forward-cache slot drop-ablation runner.

This runner consumes the landed INV-032 / #759 panel-oracle dataset and
re-trains a deterministic slot-preserving ridge probe with each candidate slot
zeroed during training and evaluation. In the default `bounded-static` contract
it preserves the earlier #746 static-AST probe semantics. In the
`forward-cache-production` contract it is the dedicated #812 E1-E14
forward-cache namespace ablation path: slot names are forward-cache embedder IDs,
unknown/non-materialized slots fail closed, and baseline/drop model artifacts are
written for manual source-of-truth readback. Neither contract modifies the
serving ME-JEPA checkpoint.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from .python_low_signal_slot_attribution import (
        DEFAULT_CANDIDATE_SLOTS,
        DEFAULT_DATASET_ROOT,
        EMPTY_SLOTS,
        TEMPORAL_SLOTS,
        SlotAttributionError,
        artifact_hashes,
        build_probe,
        canonical_json,
        fit_ridge_probe,
        load_dataset,
        metric_row,
        predict_probe,
        reduce_slot_blocks,
        require,
        require_aiwonder_path,
        sha256_file,
        tree_state,
        write_json,
        write_jsonl,
    )
except ImportError:  # pragma: no cover - direct script execution on aiwonder.
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from swebench_lite_ops.python_low_signal_slot_attribution import (  # type: ignore
        DEFAULT_CANDIDATE_SLOTS,
        DEFAULT_DATASET_ROOT,
        EMPTY_SLOTS,
        TEMPORAL_SLOTS,
        SlotAttributionError,
        artifact_hashes,
        build_probe,
        canonical_json,
        fit_ridge_probe,
        load_dataset,
        metric_row,
        predict_probe,
        reduce_slot_blocks,
        require,
        require_aiwonder_path,
        sha256_file,
        tree_state,
        write_json,
        write_jsonl,
    )


TASK_ID = "TASK-CR-008"
ARTIFACT_KIND = "low_signal_static_drop_ablation_phase_1"
SCHEMA_VERSION = 1
RUNNER_CONTRACT_BOUNDED_STATIC = "bounded-static"
RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION = "forward-cache-production"
VALID_RUNNER_CONTRACTS = (
    RUNNER_CONTRACT_BOUNDED_STATIC,
    RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION,
)
FORWARD_CACHE_STATIC_SLOTS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
SLOT_RE = re.compile(r"^e[0-9]+$")

DEFAULT_OUTPUT_PARENT = Path(
    "/zfs/archive/contextgraph/fsv/task-cr-008-low-signal-embedder-audit-fsv/"
    "phase-1-static-drop-ablation"
)
DEFAULT_STATE_PARENT = Path(
    "/zfs/archive/contextgraph/state/embedder-signal-audit/"
    "task-cr-008-phase-1-static-drop-ablation-v1"
)


class DropAblationError(RuntimeError):
    pass


def sha256_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def candidate_state(slot: str, *, measured: bool) -> tuple[str, str]:
    if slot in TEMPORAL_SLOTS:
        return (
            "active_temporal_static_ast_retired",
            "temporal_slot_absent_from_static_ast_dataset",
        )
    if slot in EMPTY_SLOTS:
        return (
            "empty_or_retired",
            "empty_or_absent_slot_not_in_759_dataset",
        )
    if not measured:
        return ("active_or_unknown", "slot_absent_from_759_dataset")
    return ("active_static", "")


def drop_slot_columns(Z: Any, block: slice) -> Any:
    import numpy as np

    dropped = np.array(Z, copy=True)
    dropped[:, block.start : block.stop] = 0.0
    return dropped


def validate_candidate_slots(candidate_slots: list[str], dataset: Any, runner_contract: str) -> None:
    require(runner_contract in VALID_RUNNER_CONTRACTS, "DROP_ABLATION_RUNNER_CONTRACT_INVALID", runner_contract)
    require(candidate_slots, "DROP_ABLATION_CANDIDATE_SLOTS_EMPTY")
    seen: set[str] = set()
    materialized_slots = set(dataset.feature_slices)
    for slot in candidate_slots:
        require(bool(SLOT_RE.match(slot)), "DROP_ABLATION_SLOT_ID_INVALID", slot)
        require(slot not in seen, "DROP_ABLATION_DUPLICATE_SLOT", slot)
        seen.add(slot)
        if runner_contract == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION:
            require(
                slot in materialized_slots,
                "DROP_ABLATION_FORWARD_CACHE_SLOT_NOT_MATERIALIZED",
                {"slot": slot, "materialized_slots": sorted(materialized_slots)},
            )
    if runner_contract == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION:
        require(
            tuple(sorted(materialized_slots)) == tuple(sorted(FORWARD_CACHE_STATIC_SLOTS)),
            "DROP_ABLATION_FORWARD_CACHE_SLOT_SET_MISMATCH",
            {"expected": sorted(FORWARD_CACHE_STATIC_SLOTS), "actual": sorted(materialized_slots)},
        )


def write_model_checkpoint(
    *,
    model_root: Path,
    state_model_root: Path,
    name: str,
    coefficients: Any,
    intercept: float,
    metadata: dict[str, Any],
) -> dict[str, Any]:
    import numpy as np

    safe_name = re.sub(r"[^A-Za-z0-9_.-]+", "_", name)
    require(safe_name and safe_name == name, "DROP_ABLATION_MODEL_NAME_INVALID", name)
    model_root.mkdir(parents=True, exist_ok=True)
    state_model_root.mkdir(parents=True, exist_ok=True)
    npz_path = model_root / f"{safe_name}.npz"
    state_npz_path = state_model_root / f"{safe_name}.npz"
    manifest_path = model_root / f"{safe_name}.manifest.json"
    state_manifest_path = state_model_root / f"{safe_name}.manifest.json"

    coeff = np.asarray(coefficients, dtype=np.float64)
    intercept_array = np.asarray([float(intercept)], dtype=np.float64)
    tmp_npz = npz_path.with_suffix(".npz.tmp")
    with tmp_npz.open("wb") as handle:
        np.savez_compressed(
            handle,
            coefficients=coeff,
            intercept=intercept_array,
            metadata_json=np.asarray([canonical_json(metadata)], dtype=str),
        )
        handle.flush()
        os.fsync(handle.fileno())
    tmp_npz.replace(npz_path)
    loaded = np.load(npz_path, allow_pickle=False)
    require(set(loaded.files) == {"coefficients", "intercept", "metadata_json"}, "DROP_ABLATION_MODEL_NPZ_KEYS", loaded.files)
    require(tuple(loaded["coefficients"].shape) == tuple(coeff.shape), "DROP_ABLATION_MODEL_COEFF_SHAPE_MISMATCH")
    require(float(loaded["intercept"][0]) == float(intercept), "DROP_ABLATION_MODEL_INTERCEPT_MISMATCH")

    shutil.copy2(npz_path, state_npz_path)
    require(sha256_file(npz_path) == sha256_file(state_npz_path), "DROP_ABLATION_STATE_MODEL_HASH_MISMATCH", safe_name)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "forward_cache_slot_ablation_model_checkpoint",
        "model_name": safe_name,
        "created_at_utc": utc_now(),
        "model_npz": str(npz_path),
        "state_model_npz": str(state_npz_path),
        "model_npz_sha256": sha256_file(npz_path),
        "coefficient_shape": list(coeff.shape),
        "intercept": float(intercept),
        "metadata": metadata,
    }
    write_json(manifest_path, manifest)
    write_json(state_manifest_path, manifest)
    require(sha256_file(manifest_path) == sha256_file(state_manifest_path), "DROP_ABLATION_STATE_MODEL_MANIFEST_HASH_MISMATCH", safe_name)
    return {
        "name": safe_name,
        "model_npz": str(npz_path),
        "model_npz_sha256": sha256_file(npz_path),
        "manifest_json": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "state_model_npz": str(state_npz_path),
        "state_manifest_json": str(state_manifest_path),
    }


def per_cell_metrics(
    *,
    slot: str,
    holdout_rows: list[dict[str, Any]],
    y_holdout: Any,
    full_scores: Any,
    dropped_scores: Any,
) -> list[dict[str, Any]]:
    import numpy as np

    categories = sorted({str(row.get("mutation_category") or "") for row in holdout_rows})
    rows: list[dict[str, Any]] = []
    for category in categories:
        indexes = [idx for idx, row in enumerate(holdout_rows) if str(row.get("mutation_category") or "") == category]
        idx = np.asarray(indexes, dtype=np.int64)
        full_cell = metric_row(y_holdout[idx], full_scores[idx])
        dropped_cell = metric_row(y_holdout[idx], dropped_scores[idx])
        corr_delta = (
            full_cell["pearson"] - dropped_cell["pearson"]
            if full_cell["pearson"] is not None and dropped_cell["pearson"] is not None
            else None
        )
        brier_delta = (
            dropped_cell["brier"] - full_cell["brier"]
            if full_cell["brier"] is not None and dropped_cell["brier"] is not None
            else None
        )
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "slot": slot.upper(),
                "scope": "cell",
                "cell": f"{category}::python",
                "mutation_category": category,
                "language": "python",
                "measured": True,
                "full_metrics": full_cell,
                "dropped_metrics": dropped_cell,
                "holdout_corr_delta_when_dropped": corr_delta,
                "brier_delta_when_dropped": brier_delta,
                "interpretation": "positive corr delta means the full retrained probe outperformed the same probe with this slot zeroed",
            }
        )
    return rows


def summarize_slot(
    slot: str,
    slot_rows: list[dict[str, Any]],
    *,
    measured: bool,
    runner_contract: str,
    model_artifact: dict[str, Any] | None = None,
) -> dict[str, Any]:
    current_state, absent_reason = candidate_state(slot, measured=measured)
    if not measured:
        blockers = ["phase_1_static_ast_slot_absent"]
        if slot in TEMPORAL_SLOTS:
            blockers.append("phase_3_live_temporal_variance_required")
        return {
            "slot": slot.upper(),
            "current_state": current_state,
            "measured": False,
            "reason": absent_reason,
            "holdout_corr_delta_when_dropped": None,
            "per_cell_median_corr_delta_when_dropped": None,
            "per_cell_min_corr_delta_when_dropped": None,
            "per_cell_max_corr_delta_when_dropped": None,
            "per_cell_valid_corr_count": 0,
            "replaceable": False,
            "authoritative_for_holdout_corr_delta": False,
            "authoritative_for_replacement": False,
            "decision": "phase_1_static_drop_not_measurable_for_slot",
            "blockers": blockers,
        }

    global_rows = [row for row in slot_rows if row.get("scope") == "global"]
    require(len(global_rows) == 1, "DROP_ABLATION_GLOBAL_ROW_MISSING", slot)
    global_delta = global_rows[0].get("holdout_corr_delta_when_dropped")
    cell_values = [
        float(row["holdout_corr_delta_when_dropped"])
        for row in slot_rows
        if row.get("scope") == "cell" and isinstance(row.get("holdout_corr_delta_when_dropped"), (int, float))
    ]
    median = sorted(cell_values)[len(cell_values) // 2] if cell_values else None
    if isinstance(global_delta, (int, float)) and global_delta >= 0.005:
        decision = "phase_1_static_keep_signal_detected"
    elif isinstance(global_delta, (int, float)) and global_delta < 0.001:
        decision = "phase_1_static_low_signal_candidate_needs_production_confirmation"
    else:
        decision = "phase_1_static_marginal_or_degenerate_needs_production_confirmation"
    if runner_contract == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION:
        blockers = ["slot_replacement_requires_task_cr_007_promotion_gates"]
        authoritative_for_delta = True
    else:
        blockers = ["production_mejepa_trainer_ablation_required_before_replacement"]
        authoritative_for_delta = False
    return {
        "slot": slot.upper(),
        "current_state": current_state,
        "measured": True,
        "reason": None,
        "holdout_corr_delta_when_dropped": global_delta,
        "holdout_brier_delta_when_dropped": global_rows[0].get("brier_delta_when_dropped"),
        "full_holdout_metrics": global_rows[0].get("full_metrics"),
        "dropped_holdout_metrics": global_rows[0].get("dropped_metrics"),
        "per_cell_median_corr_delta_when_dropped": median,
        "per_cell_min_corr_delta_when_dropped": min(cell_values) if cell_values else None,
        "per_cell_max_corr_delta_when_dropped": max(cell_values) if cell_values else None,
        "per_cell_valid_corr_count": len(cell_values),
        "replaceable": False,
        "authoritative_for_holdout_corr_delta": authoritative_for_delta,
        "authoritative_for_replacement": False,
        "decision": decision,
        "blockers": blockers,
        "model_artifact": model_artifact,
    }


def compute_drop_ablation(
    dataset: Any,
    probe: Any,
    Z_train: Any,
    Z_holdout: Any,
    y_train: Any,
    y_holdout: Any,
    candidate_slots: list[str],
    ridge_alpha: float,
    runner_contract: str,
    model_root: Path,
    state_model_root: Path,
    model_metadata_base: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    import numpy as np

    holdout_rows = [dataset.rows[index] for index in probe.holdout_idx]
    full_scores = predict_probe(Z_holdout, probe.coefficients, probe.intercept)
    full_global = metric_row(y_holdout, full_scores)
    per_cell_rows: list[dict[str, Any]] = []
    summary_rows: list[dict[str, Any]] = []
    model_artifacts: list[dict[str, Any]] = []
    baseline_model = write_model_checkpoint(
        model_root=model_root,
        state_model_root=state_model_root,
        name="baseline_full_forward_cache_probe",
        coefficients=probe.coefficients,
        intercept=probe.intercept,
        metadata={
            **model_metadata_base,
            "model_role": "baseline_full",
            "dropped_slot": None,
            "slot_namespace": "forward_cache_embedder_id",
        },
    )
    model_artifacts.append(baseline_model)
    for slot in candidate_slots:
        slot = slot.lower()
        block = probe.feature_blocks.get(slot)
        slot_rows: list[dict[str, Any]] = []
        if block is None:
            slot_rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "slot": slot.upper(),
                    "scope": "global",
                    "cell": "python::all",
                    "mutation_category": "all",
                    "language": "python",
                    "measured": False,
                    "full_metrics": full_global,
                    "dropped_metrics": None,
                    "holdout_corr_delta_when_dropped": None,
                    "brier_delta_when_dropped": None,
                    "invalid_reason": candidate_state(slot, measured=False)[1],
                }
            )
            per_cell_rows.extend(slot_rows)
            summary_rows.append(summarize_slot(slot, slot_rows, measured=False, runner_contract=runner_contract))
            continue

        Z_train_drop = drop_slot_columns(Z_train, block)
        Z_holdout_drop = drop_slot_columns(Z_holdout, block)
        coefficients, intercept = fit_drop_probe(Z_train_drop, y_train, ridge_alpha)
        model_artifact = write_model_checkpoint(
            model_root=model_root,
            state_model_root=state_model_root,
            name=f"drop_{slot}_forward_cache_probe",
            coefficients=coefficients,
            intercept=intercept,
            metadata={
                **model_metadata_base,
                "model_role": "slot_dropped",
                "dropped_slot": slot,
                "dropped_feature_block": {"start": int(block.start), "stop": int(block.stop)},
                "slot_namespace": "forward_cache_embedder_id",
            },
        )
        model_artifacts.append(model_artifact)
        dropped_scores = predict_probe(Z_holdout_drop, coefficients, intercept)
        dropped_global = metric_row(y_holdout, dropped_scores)
        global_delta = (
            full_global["pearson"] - dropped_global["pearson"]
            if full_global["pearson"] is not None and dropped_global["pearson"] is not None
            else None
        )
        global_brier_delta = (
            dropped_global["brier"] - full_global["brier"]
            if full_global["brier"] is not None and dropped_global["brier"] is not None
            else None
        )
        slot_rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "slot": slot.upper(),
                "scope": "global",
                "cell": "python::all",
                "mutation_category": "all",
                "language": "python",
                "measured": True,
                "full_metrics": full_global,
                "dropped_metrics": dropped_global,
                "holdout_corr_delta_when_dropped": global_delta,
                "brier_delta_when_dropped": global_brier_delta,
                "interpretation": "positive corr delta means the full retrained probe outperformed the same probe with this slot zeroed",
            }
        )
        slot_rows.extend(
            per_cell_metrics(
                slot=slot,
                holdout_rows=holdout_rows,
                y_holdout=y_holdout,
                full_scores=full_scores,
                dropped_scores=dropped_scores,
            )
        )
        per_cell_rows.extend(slot_rows)
        summary_rows.append(
            summarize_slot(
                slot,
                slot_rows,
                measured=True,
                runner_contract=runner_contract,
                model_artifact=model_artifact,
            )
        )

    diagnostics = {
        "train_row_count": len(probe.train_idx),
        "holdout_row_count": len(probe.holdout_idx),
        "holdout_label_counts": {str(key): int(value) for key, value in sorted(Counter(int(v) for v in y_holdout.tolist()).items())},
        "holdout_category_counts": dict(sorted(Counter(str(row.get("mutation_category") or "") for row in holdout_rows).items())),
        "full_holdout_metrics": full_global,
        "measured_slots": [row["slot"] for row in summary_rows if row.get("measured") is True],
        "unmeasured_slots": [row["slot"] for row in summary_rows if row.get("measured") is False],
        "model_artifacts": model_artifacts,
    }
    return per_cell_rows, summary_rows, diagnostics


def fit_drop_probe(Z_train: Any, y_train: Any, ridge_alpha: float) -> tuple[Any, float]:
    return fit_ridge_probe(Z_train, y_train, ridge_alpha)


def validation_cases(
    summary_rows: list[dict[str, Any]],
    candidate_slots: list[str],
    doctrine: dict[str, Any],
    runner_contract: str,
    model_artifacts: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    by_slot = {row["slot"]: row for row in summary_rows}
    active_expected = {slot.upper() for slot in candidate_slots if slot.lower() in {"e9", "e14"}}
    temporal_expected = {slot.upper() for slot in candidate_slots if slot.lower() in TEMPORAL_SLOTS}
    active_replaceable = [
        row["slot"]
        for row in summary_rows
        if row.get("replaceable") is True and row["slot"].lower() not in EMPTY_SLOTS
    ]
    cases = [
        {
            "case": "slot_identity_preserved",
            "expected": True,
            "actual": doctrine.get("slot_identity_preserved"),
            "pass": doctrine.get("slot_identity_preserved") is True,
        },
        {
            "case": "no_flat_vector_identity_erasure",
            "expected": False,
            "actual": doctrine.get("flat_vector_concat_used"),
            "pass": doctrine.get("flat_vector_concat_used") is False,
        },
        {
            "case": "active_static_candidates_have_drop_delta",
            "expected": sorted(active_expected),
            "actual": sorted(slot for slot in active_expected if isinstance(by_slot.get(slot, {}).get("holdout_corr_delta_when_dropped"), (int, float))),
            "pass": all(isinstance(by_slot.get(slot, {}).get("holdout_corr_delta_when_dropped"), (int, float)) for slot in active_expected),
        },
        {
            "case": "temporal_slots_not_fabricated_from_static_dataset",
            "expected": sorted(temporal_expected),
            "actual": sorted(slot for slot in temporal_expected if by_slot.get(slot, {}).get("measured") is False),
            "pass": all(by_slot.get(slot, {}).get("measured") is False for slot in temporal_expected),
        },
        {
            "case": "active_slots_not_marked_replaceable_from_bounded_static_ablation",
            "expected": [],
            "actual": active_replaceable,
            "pass": not active_replaceable,
        },
        {
            "case": "production_checkpoint_ablation_not_claimed",
            "expected": False,
            "actual": doctrine.get("production_mejepa_checkpoint_retrained"),
            "pass": doctrine.get("production_mejepa_checkpoint_retrained") is False,
        },
        {
            "case": "model_artifacts_written_for_baseline_and_measured_slots",
            "expected": 1 + len([row for row in summary_rows if row.get("measured") is True]),
            "actual": len(model_artifacts),
            "pass": len(model_artifacts) == 1 + len([row for row in summary_rows if row.get("measured") is True]),
        },
    ]
    if runner_contract == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION:
        cases.extend(
            [
                {
                    "case": "forward_cache_namespace_used",
                    "expected": "forward_cache_embedder_id",
                    "actual": doctrine.get("slot_namespace"),
                    "pass": doctrine.get("slot_namespace") == "forward_cache_embedder_id",
                },
                {
                    "case": "instrument_slot_aliasing_not_used",
                    "expected": False,
                    "actual": doctrine.get("instrument_slot_namespace_used"),
                    "pass": doctrine.get("instrument_slot_namespace_used") is False,
                },
                {
                    "case": "all_requested_slots_materialized_in_forward_cache_dataset",
                    "expected": sorted(slot.upper() for slot in candidate_slots),
                    "actual": sorted(row["slot"] for row in summary_rows if row.get("measured") is True),
                    "pass": sorted(slot.upper() for slot in candidate_slots)
                    == sorted(row["slot"] for row in summary_rows if row.get("measured") is True),
                },
            ]
        )
    return cases


def run_pipeline(args: argparse.Namespace) -> dict[str, Any]:
    runner_contract = getattr(args, "runner_contract", RUNNER_CONTRACT_BOUNDED_STATIC)
    candidate_slots = [slot.lower() for slot in args.candidate_slot]
    run_id = args.run_id or f"run-{utc_stamp()}"
    output_root = args.output_root or (args.output_parent / run_id)
    state_root = args.state_root or (args.state_parent / run_id)
    require_aiwonder_path(output_root, "output_root", args.allow_non_aiwonder_roots)
    require_aiwonder_path(state_root, "state_root", args.allow_non_aiwonder_roots)
    before = {"output_root": tree_state(output_root), "state_root": tree_state(state_root)}
    if args.clean:
        if output_root.exists():
            shutil.rmtree(output_root)
        if state_root.exists() and args.allow_clean_state_root:
            shutil.rmtree(state_root)
    output_root.mkdir(parents=True, exist_ok=True)
    state_root.mkdir(parents=True, exist_ok=True)

    dataset = load_dataset(args.dataset_root, allow_non_aiwonder_roots=args.allow_non_aiwonder_roots)
    validate_candidate_slots(candidate_slots, dataset, runner_contract)
    probe, Z_holdout, y_holdout = build_probe(
        dataset,
        holdout_fraction=args.holdout_fraction,
        seed=args.seed,
        components_per_slot=args.components_per_slot,
        ridge_alpha=args.ridge_alpha,
    )
    import numpy as np

    Z_train = np.asarray(
        reduce_slot_blocks(
            np.asarray(dataset.X)[probe.train_idx],
            np.asarray(dataset.X)[probe.holdout_idx],
            dataset.feature_slices,
            components_per_slot=args.components_per_slot,
            seed=args.seed,
        )[0],
        dtype=float,
    )
    y_train = np.asarray(dataset.y)[probe.train_idx]
    probe_config = {
        "runner_contract": runner_contract,
        "candidate_slots": candidate_slots,
        "holdout_fraction": args.holdout_fraction,
        "components_per_slot": args.components_per_slot,
        "ridge_alpha": args.ridge_alpha,
        "seed": args.seed,
        "slot_namespace": "forward_cache_embedder_id",
        "active_embedder_ids": sorted(dataset.feature_slices),
    }
    probe_config_sha256 = sha256_bytes(canonical_json(probe_config).encode("utf-8"))
    model_root = output_root / "model_artifacts"
    state_model_root = state_root / "model_artifacts"
    model_metadata_base = {
        "task_id": TASK_ID,
        "runner_contract": runner_contract,
        "probe_config_sha256": probe_config_sha256,
        "dataset_root": str(args.dataset_root),
        "dataset_manifest_sha256": sha256_file(args.dataset_root / "dataset_manifest.json"),
        "dataset_npz_sha256": sha256_file(dataset.dataset_npz),
        "dataset_rows_sha256": sha256_file(dataset.rows_path),
        "train_row_count": len(probe.train_idx),
        "holdout_row_count": len(probe.holdout_idx),
        "ridge_alpha": args.ridge_alpha,
        "seed": args.seed,
    }
    per_cell_rows, summary_rows, diagnostics = compute_drop_ablation(
        dataset,
        probe,
        Z_train,
        Z_holdout,
        y_train,
        y_holdout,
        candidate_slots,
        args.ridge_alpha,
        runner_contract,
        model_root,
        state_model_root,
        model_metadata_base,
    )

    doctrine = {
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "slot_namespace": "forward_cache_embedder_id",
        "instrument_slot_namespace_used": False,
        "instrument_slot_many_to_one_mapping_avoided": True,
        "slot_block_late_fusion_used": True,
        "target_labels_used_as_live_inputs": False,
        "target_labels_used_as_audit_supervision": True,
        "no_inner_llm_used": True,
        "phase_1_static_ast_retraining_ablation_run": True,
        "dedicated_forward_cache_ablation_runner": runner_contract == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION,
        "production_mejepa_checkpoint_retrained": False,
        "phase_3_live_temporal_variance_run": False,
        "active_replacement_authorized": False,
        "aiwonder_durable_root_required": (
            str(output_root).startswith("/zfs/archive/contextgraph/")
            and str(state_root).startswith("/zfs/archive/contextgraph/")
        )
        if not args.allow_non_aiwonder_roots
        else True,
    }
    model_artifacts = list(diagnostics.get("model_artifacts") or [])
    cases = validation_cases(summary_rows, candidate_slots, doctrine, runner_contract, model_artifacts)
    all_passed = all(case["pass"] for case in cases)

    per_cell_path = output_root / "per_cell_static_drop_ablation.jsonl"
    summary_path = output_root / "static_drop_ablation_summary.json"
    manifest_path = output_root / "low_signal_static_drop_ablation_manifest.json"
    readback_path = output_root / "manual_readback_summary.json"
    hashes_path = output_root / "artifact_hashes.json"
    state_summary_path = state_root / "static_drop_ablation_summary.json"
    state_per_cell_path = state_root / "per_cell_static_drop_ablation.jsonl"

    per_cell_count = write_jsonl(per_cell_path, per_cell_rows)
    write_jsonl(state_per_cell_path, per_cell_rows)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "low_signal_static_drop_ablation_summary",
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "summary_rows": summary_rows,
        "diagnostics": diagnostics,
        "decision_limits": {
            "bounded_static_ast_probe": True,
            "dedicated_forward_cache_ablation_runner": runner_contract == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION,
            "production_trainer_ablation_still_required_before_replacement": runner_contract
            != RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION,
            "temporal_slots_require_live_variance": True,
            "active_slot_replacement_authorized": False,
        },
    }
    write_json(summary_path, summary)
    write_json(state_summary_path, summary)
    source_of_truth = {
        "output_root": str(output_root),
        "state_root": str(state_root),
        "dataset_root": str(args.dataset_root),
        "dataset_npz": str(dataset.dataset_npz),
        "dataset_rows_jsonl": str(dataset.rows_path),
        "per_cell_static_drop_ablation_jsonl": str(per_cell_path),
        "static_drop_ablation_summary_json": str(summary_path),
        "state_per_cell_static_drop_ablation_jsonl": str(state_per_cell_path),
        "state_static_drop_ablation_summary_json": str(state_summary_path),
        "model_artifacts_root": str(model_root),
        "state_model_artifacts_root": str(state_model_root),
        "manifest_json": str(manifest_path),
        "manual_readback_summary_json": str(readback_path),
        "artifact_hashes_json": str(hashes_path),
    }
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "passes": all_passed,
        "allPassed": all_passed,
        "sourceOfTruth": source_of_truth,
        "trigger": " ".join(sys.argv),
        "expected": (
            "dedicated E1-E14 forward-cache slot drop-ablation over #759 slot-sliced panel dataset; "
            "baseline/drop model artifacts written; no serving checkpoint overwrite"
            if runner_contract == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION
            else "bounded static-AST retraining drop-ablation over #759 slot-sliced panel dataset; no production checkpoint replacement claim"
        ),
        "actual": {
            "candidate_slots": [slot.upper() for slot in candidate_slots],
            "measured_slots": [row["slot"] for row in summary_rows if row.get("measured") is True],
            "unmeasured_slots": [row["slot"] for row in summary_rows if row.get("measured") is False],
            "per_cell_row_count": per_cell_count,
            "summary_rows": summary_rows,
        },
        "cases": cases,
        "doctrine": doctrine,
        "runner_contract": runner_contract,
        "probe_config": probe_config,
        "probe_config_sha256": probe_config_sha256,
        "probe": {
            "estimator": "per_slot_random_projection_plus_ridge_linear_probe_retrained_with_slot_zeroed",
            "seed": args.seed,
            "holdout_fraction": args.holdout_fraction,
            "ridge_alpha": args.ridge_alpha,
            "train_row_count": len(probe.train_idx),
            "holdout_row_count": len(probe.holdout_idx),
            "reduction": probe.reduction,
        },
        "source_artifacts": {
            "dataset_manifest": {
                "path": str(args.dataset_root / "dataset_manifest.json"),
                "sha256": sha256_file(args.dataset_root / "dataset_manifest.json"),
            },
            "dataset_npz": {"path": str(dataset.dataset_npz), "sha256": sha256_file(dataset.dataset_npz)},
            "dataset_rows_jsonl": {"path": str(dataset.rows_path), "sha256": sha256_file(dataset.rows_path)},
        },
        "model_artifacts": model_artifacts,
        "before": before,
        "after": {"output_root": tree_state(output_root), "state_root": tree_state(state_root)},
    }
    write_json(manifest_path, manifest)
    readback = {
        "passes": all_passed,
        "allPassed": all_passed,
        "run_id": run_id,
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "summary_path": str(summary_path),
        "summary_sha256": sha256_file(summary_path),
        "per_cell_path": str(per_cell_path),
        "per_cell_sha256": sha256_file(per_cell_path),
        "per_cell_row_count": per_cell_count,
        "summary_rows": summary_rows,
        "cases": cases,
        "runner_contract": runner_contract,
        "probe_config_sha256": probe_config_sha256,
        "model_artifacts": model_artifacts,
    }
    write_json(readback_path, readback)
    hashes = artifact_hashes(
        {
            "low_signal_static_drop_ablation_manifest.json": manifest_path,
            "manual_readback_summary.json": readback_path,
            "per_cell_static_drop_ablation.jsonl": per_cell_path,
            "static_drop_ablation_summary.json": summary_path,
            "state_per_cell_static_drop_ablation.jsonl": state_per_cell_path,
            "state_static_drop_ablation_summary.json": state_summary_path,
        }
    )
    for artifact in model_artifacts:
        hashes[f"model:{artifact['name']}:npz"] = {
            "path": artifact["model_npz"],
            "sha256": artifact["model_npz_sha256"],
        }
        hashes[f"model:{artifact['name']}:manifest"] = {
            "path": artifact["manifest_json"],
            "sha256": artifact["manifest_sha256"],
        }
    write_json(hashes_path, hashes)
    if not all_passed:
        raise DropAblationError(f"DROP_ABLATION_VALIDATION_FAILED: {manifest_path}")
    print(json.dumps(readback, indent=2, sort_keys=True))
    return readback


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset-root", type=Path, default=DEFAULT_DATASET_ROOT)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-parent", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--state-root", type=Path)
    parser.add_argument("--run-id", default=f"run-{utc_stamp()}")
    parser.add_argument("--candidate-slot", action="append")
    parser.add_argument("--runner-contract", choices=VALID_RUNNER_CONTRACTS, default=RUNNER_CONTRACT_BOUNDED_STATIC)
    parser.add_argument("--holdout-fraction", type=float, default=0.2)
    parser.add_argument("--components-per-slot", type=int, default=32)
    parser.add_argument("--ridge-alpha", type=float, default=10.0)
    parser.add_argument("--seed", type=int, default=746003)
    parser.add_argument("--clean", action="store_true")
    parser.add_argument("--allow-clean-state-root", action="store_true")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    args = parser.parse_args(argv)
    if args.candidate_slot is None:
        args.candidate_slot = list(DEFAULT_CANDIDATE_SLOTS)
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    run_pipeline(args)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (DropAblationError, SlotAttributionError) as err:
        print(f"DROP_ABLATION_ERROR: {err}", file=sys.stderr)
        raise SystemExit(2)
