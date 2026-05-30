#!/usr/bin/env python3
"""TASK-CR-008 low-signal embedder audit preflight.

This runner is intentionally conservative. It creates a replacement-priority
preflight from already-landed source-of-truth artifacts, but it does not mark
active non-empty slots replaceable without the full #746 drop-ablation,
per-cell attribution, and live-temporal variance evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
import tempfile
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TASK_ID = "TASK-CR-008"
ARTIFACT_KIND = "low_signal_embedder_audit_preflight"
SCHEMA_VERSION = 1

DEFAULT_MI_MANIFEST = Path(
    "/zfs/archive/contextgraph/fsv/inv-032-panel-sufficiency-mi-fsv/"
    "run-20260529T234900Z/full-candidate-map/panel_oracle_mi_manifest.json"
)
DEFAULT_CAUSAL_AUC_MANIFEST = Path(
    "/zfs/archive/contextgraph/fsv/task-cr-006-causal-embedder-auc-fsv/"
    "run-20260530T0125Z-sample12271/causal_embedder_auc_manifest.json"
)
DEFAULT_ORACLE_MANIFEST = Path(
    "/zfs/archive/contextgraph/fsv/inv-035-oracle-noise-floor-fsv/"
    "run-20260529T233417Z/oracle_self_consistency_manifest.json"
)
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/task-cr-008-low-signal-embedder-audit-fsv/preflight")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/embedder-signal-audit/task-cr-008-signal-ranking-v1")

ACTIVE_STATIC_SLOTS = ("e1", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14")
TEMPORAL_SLOTS = ("e2", "e3", "e4")
CANDIDATE_SLOTS = ("e5", "e11", "e9", "e14", "e2", "e3", "e4")
SLOT_ORDER = {slot.upper(): index for index, slot in enumerate(CANDIDATE_SLOTS)}
EMPTY_SLOT_STATES = {
    "e5": "retired_empty",
    "e11": "disabled_empty",
}


class LowSignalAuditError(RuntimeError):
    pass


@dataclass(frozen=True)
class Evidence:
    mi_delta_bits: float | None = None
    mi_drop_one_bits: float | None = None
    test_bugfix_transition_auc: float | None = None
    test_asymmetry_auc: float | None = None
    validation_bugfix_transition_auc: float | None = None
    static_active: bool = False
    static_retired_temporal: bool = False


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(canonical_json(row) + "\n")
            count += 1
    tmp.replace(path)
    return count


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise LowSignalAuditError(f"{code}{suffix}")


def finite_float(value: Any) -> float | None:
    if isinstance(value, bool) or value is None:
        return None
    try:
        out = float(value)
    except (TypeError, ValueError):
        return None
    return out if math.isfinite(out) else None


def require_aiwonder_path(path: Path, name: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(str(path).startswith("/zfs/archive/contextgraph/"), "LOW_SIGNAL_AUDIT_NON_AIWONDER_PATH", {name: str(path)})


def load_manifest(path: Path, name: str, allow_non_aiwonder_roots: bool) -> tuple[dict[str, Any], dict[str, Any]]:
    require_aiwonder_path(path, name, allow_non_aiwonder_roots)
    require(path.is_file(), "LOW_SIGNAL_AUDIT_MANIFEST_MISSING", {name: str(path)})
    doc = read_json(path)
    require(isinstance(doc, dict), "LOW_SIGNAL_AUDIT_MANIFEST_NOT_OBJECT", {name: str(path)})
    return doc, {"path": str(path), "sha256": sha256_file(path)}


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False}
    files = [item for item in path.rglob("*") if item.is_file()]
    return {
        "path": str(path),
        "exists": True,
        "file_count": len(files),
        "byte_count": sum(item.stat().st_size for item in files),
        "sample": [str(item.relative_to(path)) for item in sorted(files)[:20]],
    }


def parse_mi_evidence(mi_manifest: dict[str, Any]) -> dict[str, Evidence]:
    require(mi_manifest.get("passes") is True, "LOW_SIGNAL_AUDIT_MI_NOT_PASSING")
    decision = mi_manifest.get("decision_rule") or {}
    require(decision.get("panel_sufficient") is False, "LOW_SIGNAL_AUDIT_MI_DOES_NOT_SHOW_PANEL_GAP", decision)
    drop_rows = mi_manifest.get("drop_one_mine_summary") or []
    require(isinstance(drop_rows, list), "LOW_SIGNAL_AUDIT_MI_DROP_ONE_NOT_LIST")
    by_slot: dict[str, Evidence] = {}
    for row in drop_rows:
        slot = str(row.get("embedder_id") or "").lower()
        if not slot:
            continue
        by_slot[slot] = Evidence(
            mi_delta_bits=finite_float(row.get("delta_from_global_bits")),
            mi_drop_one_bits=finite_float(row.get("drop_one_bounded_best_bits")),
            static_active=slot in ACTIVE_STATIC_SLOTS,
        )
    return by_slot


def parse_causal_auc_evidence(auc_manifest: dict[str, Any]) -> dict[str, Evidence]:
    require(auc_manifest.get("passes") is True, "LOW_SIGNAL_AUDIT_CAUSAL_AUC_NOT_PASSING")
    doctrine = auc_manifest.get("doctrine") or {}
    require(doctrine.get("slot_identity_preserved") is True, "LOW_SIGNAL_AUDIT_CAUSAL_AUC_SLOT_IDENTITY_NOT_PRESERVED")
    require(doctrine.get("flat_vector_concat_used") is False, "LOW_SIGNAL_AUDIT_CAUSAL_AUC_FLAT_VECTOR_USED")
    require(doctrine.get("target_labels_used_as_live_inputs") is False, "LOW_SIGNAL_AUDIT_CAUSAL_AUC_TARGET_LEAK")
    metrics = auc_manifest.get("per_embedder_metrics") or {}
    require(isinstance(metrics, dict) and metrics, "LOW_SIGNAL_AUDIT_CAUSAL_AUC_METRICS_MISSING")
    by_slot: dict[str, Evidence] = {}
    for slot, row in metrics.items():
        slot_id = str(slot).lower()
        by_slot[slot_id] = Evidence(
            test_bugfix_transition_auc=finite_float((row or {}).get("test_bugfix_transition_auc")),
            test_asymmetry_auc=finite_float((row or {}).get("test_asymmetry_auc")),
            validation_bugfix_transition_auc=finite_float((row or {}).get("validation_bugfix_transition_auc")),
            static_active=slot_id in ACTIVE_STATIC_SLOTS,
        )
    return by_slot


def parse_oracle_evidence(oracle_manifest: dict[str, Any]) -> dict[str, Any]:
    value = finite_float(oracle_manifest.get("oracle_self_consistency"))
    require(value is not None, "LOW_SIGNAL_AUDIT_ORACLE_SELF_CONSISTENCY_MISSING")
    return {"oracle_self_consistency": value, "noise_floor_non_binding": value >= 0.97}


def merge_evidence(mi: dict[str, Evidence], auc: dict[str, Evidence], slot: str) -> Evidence:
    left = mi.get(slot, Evidence())
    right = auc.get(slot, Evidence())
    return Evidence(
        mi_delta_bits=left.mi_delta_bits,
        mi_drop_one_bits=left.mi_drop_one_bits,
        test_bugfix_transition_auc=right.test_bugfix_transition_auc,
        test_asymmetry_auc=right.test_asymmetry_auc,
        validation_bugfix_transition_auc=right.validation_bugfix_transition_auc,
        static_active=slot in ACTIVE_STATIC_SLOTS or left.static_active or right.static_active,
        static_retired_temporal=slot in TEMPORAL_SLOTS,
    )


def classify_slot(slot: str, evidence: Evidence, *, live_variance_available: bool, full_ablation_available: bool) -> dict[str, Any]:
    slot = slot.lower()
    blockers: list[str] = []
    measurements: dict[str, Any] = {
        "holdout_corr_delta_when_dropped": None,
        "per_cell_median_attribution": None,
        "live_variance": None,
        "mi_delta_from_global_bits_proxy": evidence.mi_delta_bits,
        "mi_drop_one_bounded_best_bits_proxy": evidence.mi_drop_one_bits,
        "test_bugfix_transition_auc_proxy": evidence.test_bugfix_transition_auc,
        "test_asymmetry_auc_proxy": evidence.test_asymmetry_auc,
        "validation_bugfix_transition_auc_proxy": evidence.validation_bugfix_transition_auc,
    }

    if slot in EMPTY_SLOT_STATES:
        return {
            "slot": slot.upper(),
            "current_state": EMPTY_SLOT_STATES[slot],
            "replaceable": True,
            "authoritative_for_replacement": True,
            "preflight_only": False,
            "rank_bucket": 0,
            "decision": "promote_empty_slot_to_replacement_queue",
            "reason": "slot already empty; #746 audit body marks it trivially replaceable",
            "blockers": [],
            "measurements": measurements,
        }

    blockers.extend(["phase_1_drop_ablation_required", "phase_2_per_cell_attribution_required"])
    if slot in TEMPORAL_SLOTS:
        blockers.append("phase_3_live_temporal_variance_required")
        return {
            "slot": slot.upper(),
            "current_state": "active_temporal_static_ast_retired",
            "replaceable": False,
            "authoritative_for_replacement": False,
            "preflight_only": True,
            "rank_bucket": 30,
            "decision": "blocked_keep_temporal_until_live_variance_and_drop_ablation",
            "reason": "operator direction keeps temporal slots unless live-temporal variance is near zero and drop delta is <0.001",
            "blockers": blockers,
            "measurements": measurements,
        }

    low_proxy = (
        (evidence.test_bugfix_transition_auc is not None and evidence.test_bugfix_transition_auc < 0.6)
        or (evidence.mi_delta_bits is not None and evidence.mi_delta_bits <= 0.001)
    )
    if slot == "e9" and low_proxy:
        rank_bucket = 10
        decision = "highest_priority_active_slot_for_full_audit"
        reason = "proxy evidence is weak or negative, but #746 replacement requires real holdout drop-ablation and attribution"
    elif slot == "e14" and low_proxy:
        rank_bucket = 20 if (evidence.mi_delta_bits or 0.0) > 0.005 else 11
        decision = "audit_reserved_slot_before_replacement"
        reason = "reserved slot has weak causal-AUC proxy; positive MI-drop proxy prevents preflight replacement"
    else:
        rank_bucket = 25
        decision = "insufficient_proxy_evidence_for_active_slot"
        reason = "no full #746 replacement metric has been measured for this active slot"

    return {
        "slot": slot.upper(),
        "current_state": "active_static" if evidence.static_active else "active_or_reserved",
        "replaceable": False,
        "authoritative_for_replacement": False,
        "preflight_only": True,
        "rank_bucket": rank_bucket,
        "decision": decision,
        "reason": reason,
        "blockers": blockers if not (full_ablation_available and live_variance_available) else [],
        "measurements": measurements,
    }


def rank_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    def key(row: dict[str, Any]) -> tuple[int, float, str]:
        measurements = row["measurements"]
        mi_delta = measurements.get("mi_delta_from_global_bits_proxy")
        auc = measurements.get("test_bugfix_transition_auc_proxy")
        proxy_score = 0.0
        if isinstance(mi_delta, (int, float)):
            proxy_score += float(mi_delta)
        if isinstance(auc, (int, float)):
            proxy_score += max(0.0, float(auc) - 0.5)
        return (int(row["rank_bucket"]), proxy_score, f"{SLOT_ORDER.get(str(row['slot']), 999):03d}")

    ranked = sorted(rows, key=key)
    for index, row in enumerate(ranked, start=1):
        row["rank"] = index
        row.pop("rank_bucket", None)
    return ranked


def build_priority_rows(
    *,
    mi_manifest: dict[str, Any],
    auc_manifest: dict[str, Any],
    live_variance_available: bool = False,
    full_ablation_available: bool = False,
) -> list[dict[str, Any]]:
    mi = parse_mi_evidence(mi_manifest)
    auc = parse_causal_auc_evidence(auc_manifest)
    rows = [
        classify_slot(
            slot,
            merge_evidence(mi, auc, slot),
            live_variance_available=live_variance_available,
            full_ablation_available=full_ablation_available,
        )
        for slot in CANDIDATE_SLOTS
    ]
    return rank_rows(rows)


def validate_priority_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    cases = []
    empty = [row for row in rows if row["slot"] in {"E5", "E11"}]
    active_replaceable = [
        row
        for row in rows
        if row["slot"] not in {"E5", "E11"} and row.get("replaceable") is True
    ]
    temporal = [row for row in rows if row["slot"] in {"E2", "E3", "E4"}]
    cases.append(
        {
            "case": "empty_slots_are_authoritative_replaceable",
            "expected": "E5 and E11 replaceable=true with authoritative_for_replacement=true",
            "actual": {row["slot"]: row["replaceable"] and row["authoritative_for_replacement"] for row in empty},
            "pass": len(empty) == 2 and all(row["replaceable"] and row["authoritative_for_replacement"] for row in empty),
        }
    )
    cases.append(
        {
            "case": "active_slots_not_replaceable_from_proxy_evidence",
            "expected": "No non-empty active slot is replaceable before full #746 metrics",
            "actual": [row["slot"] for row in active_replaceable],
            "pass": not active_replaceable,
        }
    )
    cases.append(
        {
            "case": "temporal_slots_require_live_variance",
            "expected": "E2/E3/E4 carry phase_3_live_temporal_variance_required blocker",
            "actual": {row["slot"]: row.get("blockers", []) for row in temporal},
            "pass": len(temporal) == 3
            and all("phase_3_live_temporal_variance_required" in row.get("blockers", []) for row in temporal),
        }
    )
    cases.append(
        {
            "case": "ranking_artifact_has_all_candidate_slots",
            "expected": sorted(slot.upper() for slot in CANDIDATE_SLOTS),
            "actual": sorted(row["slot"] for row in rows),
            "pass": sorted(row["slot"] for row in rows) == sorted(slot.upper() for slot in CANDIDATE_SLOTS),
        }
    )
    return cases


def artifact_hashes(paths: dict[str, Path]) -> dict[str, str]:
    return {name: sha256_file(path) for name, path in sorted(paths.items()) if path.is_file()}


def run_pipeline(args: argparse.Namespace) -> dict[str, Any]:
    run_id = args.run_id or f"run-{utc_stamp()}"
    output_root = args.output_root or (args.output_parent / run_id)
    state_root = args.state_root
    require_aiwonder_path(output_root, "output_root", args.allow_non_aiwonder_roots)
    require_aiwonder_path(state_root, "state_root", args.allow_non_aiwonder_roots)

    before = {"output_root": tree_state(output_root), "state_root": tree_state(state_root)}
    if args.clean:
        if output_root.exists():
            shutil.rmtree(output_root)
        state_root.mkdir(parents=True, exist_ok=True)

    mi_manifest, mi_source = load_manifest(args.mi_manifest, "mi_manifest", args.allow_non_aiwonder_roots)
    auc_manifest, auc_source = load_manifest(args.causal_auc_manifest, "causal_auc_manifest", args.allow_non_aiwonder_roots)
    oracle_manifest, oracle_source = load_manifest(args.oracle_manifest, "oracle_manifest", args.allow_non_aiwonder_roots)
    oracle_summary = parse_oracle_evidence(oracle_manifest)

    rows = build_priority_rows(
        mi_manifest=mi_manifest,
        auc_manifest=auc_manifest,
        live_variance_available=False,
        full_ablation_available=False,
    )
    cases = validate_priority_rows(rows)
    all_passed = all(case["pass"] for case in cases)

    ranking_path = state_root / "embedder_replacement_priority.jsonl"
    ranking_count = write_jsonl(ranking_path, rows)
    summary_path = state_root / "embedder_replacement_priority.preflight-summary.json"
    state_summary = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "low_signal_embedder_replacement_priority_preflight_summary",
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "preflight_only": True,
        "full_audit_complete": False,
        "ranking_path": str(ranking_path),
        "ranking_row_count": ranking_count,
    }
    write_json(summary_path, state_summary)

    output_root.mkdir(parents=True, exist_ok=True)
    manifest_path = output_root / "low_signal_embedder_audit_preflight_manifest.json"
    readback_path = output_root / "manual_readback_summary.json"
    hashes_path = output_root / "artifact_hashes.json"
    source_of_truth = {
        "output_root": str(output_root),
        "state_root": str(state_root),
        "embedder_replacement_priority_jsonl": str(ranking_path),
        "preflight_summary_json": str(summary_path),
        "manifest_json": str(manifest_path),
        "manual_readback_summary_json": str(readback_path),
        "artifact_hashes_json": str(hashes_path),
    }
    doctrine = {
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "target_labels_used_as_live_inputs": False,
        "no_inner_llm_used": True,
        "aiwonder_durable_root_required": (
            str(output_root).startswith("/zfs/archive/contextgraph/")
            and str(state_root).startswith("/zfs/archive/contextgraph/")
        )
        if not args.allow_non_aiwonder_roots
        else True,
    }
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "preflight_only": True,
        "full_audit_complete": False,
        "allPassed": all_passed,
        "passes": all_passed,
        "sourceOfTruth": source_of_truth,
        "trigger": " ".join(sys.argv),
        "expected": "conservative #746 preflight ranking; only already-empty E5/E11 replaceable",
        "actual": {
            "ranking_rows": ranking_count,
            "replaceable_slots": [row["slot"] for row in rows if row["replaceable"]],
            "active_replaceable_slots": [
                row["slot"] for row in rows if row["slot"] not in {"E5", "E11"} and row["replaceable"]
            ],
        },
        "cases": cases,
        "source_artifacts": {
            "mi_manifest": mi_source,
            "causal_auc_manifest": auc_source,
            "oracle_manifest": oracle_source,
        },
        "oracle_summary": oracle_summary,
        "decision_limits": {
            "active_non_empty_slots_require_full_drop_ablation": True,
            "active_non_empty_slots_require_per_cell_attribution": True,
            "temporal_slots_require_live_temporal_variance": True,
            "proxy_metrics_are_not_promotion_metrics": True,
        },
        "doctrine": doctrine,
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
        "ranking_path": str(ranking_path),
        "ranking_sha256": sha256_file(ranking_path),
        "ranking_row_count": ranking_count,
        "replaceable_slots": [row["slot"] for row in rows if row["replaceable"]],
        "active_replaceable_slots": [
            row["slot"] for row in rows if row["slot"] not in {"E5", "E11"} and row["replaceable"]
        ],
        "cases": cases,
        "hashes_path": str(hashes_path),
    }
    write_json(readback_path, readback)
    hashes = artifact_hashes(
        {
            "low_signal_embedder_audit_preflight_manifest.json": manifest_path,
            "manual_readback_summary.json": readback_path,
            "embedder_replacement_priority.jsonl": ranking_path,
            "embedder_replacement_priority.preflight-summary.json": summary_path,
        }
    )
    write_json(hashes_path, hashes)
    if not all_passed:
        raise LowSignalAuditError(f"LOW_SIGNAL_AUDIT_PREFLIGHT_FAILED: {manifest_path}")
    print(json.dumps(readback, indent=2, sort_keys=True))
    return readback


def fixture_mi_manifest() -> dict[str, Any]:
    return {
        "passes": True,
        "decision_rule": {"panel_sufficient": False, "target_bits": 0.95},
        "drop_one_mine_summary": [
            {"embedder_id": "e9", "delta_from_global_bits": -0.02, "drop_one_bounded_best_bits": 0.12, "valid": True},
            {"embedder_id": "e14", "delta_from_global_bits": 0.02, "drop_one_bounded_best_bits": 0.07, "valid": True},
        ],
    }


def fixture_auc_manifest() -> dict[str, Any]:
    return {
        "passes": True,
        "doctrine": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
        },
        "per_embedder_metrics": {
            "e9": {
                "test_bugfix_transition_auc": 0.53,
                "test_asymmetry_auc": 0.51,
                "validation_bugfix_transition_auc": 0.50,
            },
            "e14": {
                "test_bugfix_transition_auc": 0.52,
                "test_asymmetry_auc": 0.47,
                "validation_bugfix_transition_auc": 0.44,
            },
        },
    }


def write_fixture_manifests(root: Path) -> tuple[Path, Path, Path]:
    mi = root / "mi.json"
    auc = root / "auc.json"
    oracle = root / "oracle.json"
    write_json(mi, fixture_mi_manifest())
    write_json(auc, fixture_auc_manifest())
    write_json(oracle, {"oracle_self_consistency": 1.0})
    return mi, auc, oracle


def run_self_test(args: argparse.Namespace) -> int:
    root = args.self_test_root or Path(tempfile.mkdtemp(prefix="low-signal-embedder-audit-"))
    if args.clean and root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)
    mi, auc, oracle = write_fixture_manifests(root)
    run_args = argparse.Namespace(
        mi_manifest=mi,
        causal_auc_manifest=auc,
        oracle_manifest=oracle,
        output_parent=root / "fsv",
        output_root=root / "fsv" / "run-self-test",
        state_root=root / "state",
        run_id="run-self-test",
        clean=True,
        allow_non_aiwonder_roots=True,
    )
    readback = run_pipeline(run_args)
    rows = [json.loads(line) for line in (root / "state" / "embedder_replacement_priority.jsonl").read_text().splitlines()]
    checks = validate_priority_rows(rows)
    report = {
        "passes": readback["passes"] is True and all(item["pass"] for item in checks),
        "self_test_root": str(root),
        "readback": readback,
        "checks": checks,
    }
    write_json(root / "self_test_report.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["passes"] else 1


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mi-manifest", type=Path, default=DEFAULT_MI_MANIFEST)
    parser.add_argument("--causal-auc-manifest", type=Path, default=DEFAULT_CAUSAL_AUC_MANIFEST)
    parser.add_argument("--oracle-manifest", type=Path, default=DEFAULT_ORACLE_MANIFEST)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--clean", action="store_true")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--self-test-root", type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        return run_self_test(args)
    run_pipeline(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
