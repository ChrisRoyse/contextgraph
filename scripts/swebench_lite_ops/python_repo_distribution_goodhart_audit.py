#!/usr/bin/env python3
"""Repo-distribution Goodhart audit for BLINDSPOT-008a / #730."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import math
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

import numpy as np

from .honest_pearson_metrics import pearson_correlation


SCHEMA_VERSION = 1
TASK_ID = "TASK-730"
ARTIFACT_KIND = "repo_distribution_goodhart_audit"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_TASKS_DIR = Path("tasks/swebench-lite")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/blindspot-008a-repo-distribution-goodhart-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/repo-distribution-goodhart-audit")


class RepoDistributionAuditError(RuntimeError):
    pass


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


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise RepoDistributionAuditError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "REPO_DISTRIBUTION_AUDIT_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


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


def read_json(path: Path, code: str) -> Any:
    require(path.is_file(), code, {"path": str(path)})
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RepoDistributionAuditError(f"{code}: invalid JSON at {path}: {exc}") from exc


def finite_score(value: Any, *, field: str, index: int, split: str) -> float:
    require(
        isinstance(value, (int, float)) and not isinstance(value, bool),
        "REPO_DISTRIBUTION_AUDIT_ROW_BAD_SCORE",
        {"split": split, "index": index, "field": field, "value": value},
    )
    out = float(value)
    require(
        math.isfinite(out),
        "REPO_DISTRIBUTION_AUDIT_ROW_NONFINITE_SCORE",
        {"split": split, "index": index, "field": field, "value": value},
    )
    return out


def round_float(value: float | None) -> float | None:
    if value is None or not math.isfinite(value):
        return None
    return float(round(value, 10))


def safe_pearson(scores: list[float], labels: list[float]) -> float | None:
    if len(scores) < 2:
        return None
    try:
        value = pearson_correlation(scores, labels)
    except ValueError:
        return None
    if not math.isfinite(value):
        return None
    return value


def weighted_pearson(scores: list[float], labels: list[float], weights: list[float]) -> float | None:
    if len(scores) != len(labels) or len(scores) != len(weights) or len(scores) < 2:
        return None
    p = np.asarray(scores, dtype=np.float64)
    y = np.asarray(labels, dtype=np.float64)
    w = np.asarray(weights, dtype=np.float64)
    if not (np.isfinite(p).all() and np.isfinite(y).all() and np.isfinite(w).all()):
        return None
    if np.any(w < 0.0) or float(w.sum()) <= 0.0:
        return None
    w = w / w.sum()
    p_mean = float((w * p).sum())
    y_mean = float((w * y).sum())
    cov = float((w * (p - p_mean) * (y - y_mean)).sum())
    p_var = float((w * (p - p_mean) ** 2).sum())
    y_var = float((w * (y - y_mean) ** 2).sum())
    if p_var <= 0.0 or y_var <= 0.0:
        return None
    return cov / math.sqrt(p_var * y_var)


def _require_object(value: Any, label: str) -> dict[str, Any]:
    require(
        isinstance(value, dict),
        "REPO_DISTRIBUTION_AUDIT_BAD_INPUT",
        {"label": label, "actual_type": type(value).__name__},
    )
    return value


def _require_array(value: Any, label: str) -> list[Any]:
    require(
        isinstance(value, list),
        "REPO_DISTRIBUTION_AUDIT_BAD_INPUT",
        {"label": label, "actual_type": type(value).__name__},
    )
    return value


def raw_prediction_rows(execute: dict[str, Any], split_label: str) -> tuple[str, list[dict[str, Any]]]:
    for key in (
        f"{split_label}_raw_predictions",
        f"{split_label}_predictions",
        f"{split_label}_rank_lift_predictions",
    ):
        if key in execute and execute.get(key) is not None:
            rows = _require_array(execute.get(key), key)
            return key, [coerce_prediction_row(row, idx, split_label) for idx, row in enumerate(rows)]
    raise RepoDistributionAuditError(
        f"REPO_DISTRIBUTION_AUDIT_MISSING_ROWS: split={split_label}"
    )


def coerce_prediction_row(value: Any, index: int, split: str) -> dict[str, Any]:
    require(
        isinstance(value, dict),
        "REPO_DISTRIBUTION_AUDIT_ROW_NOT_OBJECT",
        {"split": split, "index": index, "actual_type": type(value).__name__},
    )
    task_id = str(value.get("task_id") or "")
    repo = str(value.get("repo") or "")
    category = str(value.get("category") or "")
    require(task_id, "REPO_DISTRIBUTION_AUDIT_ROW_TASK_ID_MISSING", {"split": split, "index": index})
    require(repo, "REPO_DISTRIBUTION_AUDIT_ROW_REPO_MISSING", {"split": split, "index": index})
    require(category, "REPO_DISTRIBUTION_AUDIT_ROW_CATEGORY_MISSING", {"split": split, "index": index})
    actual = value.get("actual_oracle_pass")
    require(
        isinstance(actual, (bool, int, float)),
        "REPO_DISTRIBUTION_AUDIT_ROW_LABEL_MISSING",
        {"split": split, "index": index, "task_id": task_id},
    )
    score = finite_score(value.get("predicted_oracle_pass"), field="predicted_oracle_pass", index=index, split=split)
    return {
        "row_index": index,
        "split": split,
        "task_id": task_id,
        "repo": repo,
        "category": category,
        "predicted_oracle_pass": score,
        "actual_oracle_pass": bool(actual),
        "label": 1.0 if bool(actual) else 0.0,
    }


def load_task_repo_counts(tasks_dir: Path, expected_task_count: int | None) -> dict[str, Any]:
    require(tasks_dir.is_dir(), "REPO_DISTRIBUTION_AUDIT_TASKS_DIR_MISSING", {"tasks_dir": str(tasks_dir)})
    paths = sorted(tasks_dir.glob("*.json"))
    require(paths, "REPO_DISTRIBUTION_AUDIT_TASKS_EMPTY", {"tasks_dir": str(tasks_dir)})
    if expected_task_count is not None:
        require(
            len(paths) == expected_task_count,
            "REPO_DISTRIBUTION_AUDIT_TASK_COUNT_MISMATCH",
            {"expected": expected_task_count, "actual": len(paths)},
        )
    counts: Counter[str] = Counter()
    samples: dict[str, list[str]] = {}
    for path in paths:
        row = read_json(path, "REPO_DISTRIBUTION_AUDIT_TASK_JSON_INVALID")
        repo = str(row.get("repo") or "")
        task_id = str(row.get("instance_id") or path.stem)
        require(repo, "REPO_DISTRIBUTION_AUDIT_TASK_REPO_MISSING", {"path": str(path)})
        counts[repo] += 1
        samples.setdefault(repo, [])
        if len(samples[repo]) < 5:
            samples[repo].append(task_id)
    rows = [
        {"repo": repo, "task_count": count, "sample_instance_ids": samples[repo]}
        for repo, count in sorted(counts.items())
    ]
    return {"task_count": len(paths), "repo_count": len(rows), "repos": rows}


def repo_metrics(rows: list[dict[str, Any]], baseline_pearson: float | None) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    by_repo: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        by_repo.setdefault(row["repo"], []).append(row)
    for repo, repo_rows in sorted(by_repo.items()):
        scores = [row["predicted_oracle_pass"] for row in repo_rows]
        labels = [row["label"] for row in repo_rows]
        value = safe_pearson(scores, labels)
        label_counts = Counter("pass" if row["actual_oracle_pass"] else "fail" for row in repo_rows)
        category_counts = Counter(row["category"] for row in repo_rows)
        out.append(
            {
                "repo": repo,
                "row_count": len(repo_rows),
                "task_count": len({row["task_id"] for row in repo_rows}),
                "label_counts": dict(sorted(label_counts.items())),
                "category_counts": dict(sorted(category_counts.items())),
                "mean_predicted_oracle_pass": round_float(float(np.mean(scores))) if scores else None,
                "raw_pearson": round_float(value),
                "drop_vs_natural_holdout_pearson": (
                    round_float(baseline_pearson - value)
                    if baseline_pearson is not None and value is not None
                    else None
                ),
                "pearson_defined": value is not None,
            }
        )
    return out


def uniform_repo_weights(rows: list[dict[str, Any]]) -> list[float]:
    counts = Counter(row["repo"] for row in rows)
    repo_count = len(counts)
    require(repo_count > 0, "REPO_DISTRIBUTION_AUDIT_NO_REPOS")
    return [1.0 / (repo_count * counts[row["repo"]]) for row in rows]


def subset_metrics(
    rows: list[dict[str, Any]],
    baseline_pearson: float | None,
    *,
    holdout_repo_count: int,
    max_rows: int = 25,
) -> list[dict[str, Any]]:
    repos = sorted({row["repo"] for row in rows})
    max_size = min(holdout_repo_count, len(repos))
    if max_size <= 0:
        return []
    out: list[dict[str, Any]] = []
    by_repo = {repo: [row for row in rows if row["repo"] == repo] for repo in repos}
    for size in range(1, max_size + 1):
        for subset in itertools.combinations(repos, size):
            subset_rows: list[dict[str, Any]] = []
            for repo in subset:
                subset_rows.extend(by_repo[repo])
            scores = [row["predicted_oracle_pass"] for row in subset_rows]
            labels = [row["label"] for row in subset_rows]
            value = safe_pearson(scores, labels)
            out.append(
                {
                    "repos": list(subset),
                    "repo_count": len(subset),
                    "row_count": len(subset_rows),
                    "task_count": len({row["task_id"] for row in subset_rows}),
                    "raw_pearson": round_float(value),
                    "drop_vs_natural_holdout_pearson": (
                        round_float(baseline_pearson - value)
                        if baseline_pearson is not None and value is not None
                        else None
                    ),
                    "pearson_defined": value is not None,
                }
            )
    out.sort(
        key=lambda row: (
            row["raw_pearson"] is None,
            row["raw_pearson"] if row["raw_pearson"] is not None else 999.0,
            -row["row_count"],
        )
    )
    return out[:max_rows]


def candidate_repo_atomic_split(
    task_repos: dict[str, Any] | None,
    *,
    holdout_repo_count: int,
    target_holdout_fraction: float,
) -> dict[str, Any] | None:
    if task_repos is None:
        return None
    repos = [(row["repo"], int(row["task_count"])) for row in task_repos["repos"]]
    if len(repos) < holdout_repo_count:
        return {
            "available": False,
            "reason": "repo_count_less_than_holdout_repo_count",
            "repo_count": len(repos),
            "holdout_repo_count": holdout_repo_count,
        }
    total = sum(count for _, count in repos)
    target = total * target_holdout_fraction
    best: tuple[float, int, tuple[tuple[str, int], ...]] | None = None
    for combo in itertools.combinations(repos, holdout_repo_count):
        count = sum(item[1] for item in combo)
        key = (abs(count - target), -count, combo)
        if best is None or key < best:
            best = key
    assert best is not None
    holdout = list(best[2])
    holdout_repos = [repo for repo, _ in holdout]
    train = [(repo, count) for repo, count in repos if repo not in set(holdout_repos)]
    return {
        "available": True,
        "repo_atomic_retraining_performed": False,
        "target_holdout_fraction": target_holdout_fraction,
        "target_holdout_task_count": round_float(target),
        "holdout_repo_count": holdout_repo_count,
        "holdout_task_count": sum(count for _, count in holdout),
        "train_task_count": sum(count for _, count in train),
        "holdout_repos": [{"repo": repo, "task_count": count} for repo, count in holdout],
        "train_repos": [{"repo": repo, "task_count": count} for repo, count in train],
    }


def build_manifest(
    *,
    ship_gate_report: Path,
    calibration_source: str,
    holdout_source: str,
    calibration_rows: list[dict[str, Any]],
    holdout_rows: list[dict[str, Any]],
    task_repos: dict[str, Any] | None,
    min_generalization_repos: int,
    holdout_repo_count: int,
    target_holdout_fraction: float,
    uniform_drop_threshold: float,
    severe_drop_threshold: float,
    per_repo_path: Path,
    subsets_path: Path,
    state_run_root: Path,
    run_id: str,
) -> dict[str, Any]:
    require(holdout_rows, "REPO_DISTRIBUTION_AUDIT_HOLDOUT_EMPTY")
    require(calibration_rows, "REPO_DISTRIBUTION_AUDIT_CALIBRATION_EMPTY")
    scores = [row["predicted_oracle_pass"] for row in holdout_rows]
    labels = [row["label"] for row in holdout_rows]
    natural = safe_pearson(scores, labels)
    weights = uniform_repo_weights(holdout_rows)
    uniform = weighted_pearson(scores, labels, weights)
    per_repo = repo_metrics(holdout_rows, natural)
    subsets = subset_metrics(holdout_rows, natural, holdout_repo_count=holdout_repo_count)
    repo_counts = Counter(row["repo"] for row in holdout_rows)
    calibration_repo_counts = Counter(row["repo"] for row in calibration_rows)
    combined_repo_counts = Counter(row["repo"] for row in calibration_rows + holdout_rows)
    full_repos = set(row["repo"] for row in task_repos["repos"]) if task_repos else set(combined_repo_counts)
    holdout_repos = set(repo_counts)
    missing_full_repos = sorted(full_repos - holdout_repos)
    uniform_drop = (
        natural - uniform
        if natural is not None and uniform is not None
        else None
    )
    worst_subset_drop = max(
        (
            float(row["drop_vs_natural_holdout_pearson"])
            for row in subsets
            if row["drop_vs_natural_holdout_pearson"] is not None
        ),
        default=None,
    )
    broad_enough = len(holdout_repos) >= min_generalization_repos and not missing_full_repos
    uniform_bad = uniform_drop is not None and uniform_drop > uniform_drop_threshold
    severe_uniform_bad = uniform_drop is not None and uniform_drop > severe_drop_threshold
    subset_bad = worst_subset_drop is not None and worst_subset_drop > uniform_drop_threshold
    full_required = (not broad_enough) or uniform_bad or subset_bad
    decision_reasons: list[str] = []
    if not broad_enough:
        decision_reasons.append("current_holdout_repo_coverage_insufficient")
    if uniform_bad:
        decision_reasons.append("uniform_repo_reweighting_drops_correlation")
    if subset_bad:
        decision_reasons.append("repo_subset_correlation_drop_detected")
    if not decision_reasons:
        decision_reasons.append("repo_distribution_goodhart_minor_on_current_fsv")
    candidate_split = candidate_repo_atomic_split(
        task_repos,
        holdout_repo_count=holdout_repo_count,
        target_holdout_fraction=target_holdout_fraction,
    )
    if candidate_split and candidate_split.get("available"):
        candidate_repos = {row["repo"] for row in candidate_split["holdout_repos"]}
        candidate_split["candidate_holdout_repos_present_in_current_holdout"] = sorted(
            candidate_repos & holdout_repos
        )
        candidate_split["candidate_holdout_repos_missing_from_current_holdout"] = sorted(
            candidate_repos - holdout_repos
        )
    write_jsonl(per_repo_path, per_repo)
    write_jsonl(subsets_path, subsets)
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "ship_gate_report_path": str(ship_gate_report),
        "ship_gate_report_sha256": sha256_file(ship_gate_report),
        "raw_prediction_sources": {
            "calibration": calibration_source,
            "holdout": holdout_source,
        },
        "row_counts": {
            "calibration_rows": len(calibration_rows),
            "holdout_rows": len(holdout_rows),
            "per_repo_metric_rows": len(per_repo),
            "repo_subset_metric_rows": len(subsets),
        },
        "repo_coverage": {
            "full_task_repo_count": len(full_repos),
            "calibration_repo_count": len(calibration_repo_counts),
            "holdout_repo_count": len(holdout_repos),
            "combined_prediction_repo_count": len(combined_repo_counts),
            "min_generalization_repos": min_generalization_repos,
            "holdout_repo_counts": dict(sorted(repo_counts.items())),
            "calibration_repo_counts": dict(sorted(calibration_repo_counts.items())),
            "combined_prediction_repo_counts": dict(sorted(combined_repo_counts.items())),
            "missing_full_task_repos_from_holdout": missing_full_repos,
            "current_holdout_generalization_claim_allowed": broad_enough,
        },
        "metrics": {
            "natural_holdout_raw_pearson": round_float(natural),
            "uniform_repo_weighted_holdout_raw_pearson": round_float(uniform),
            "uniform_repo_reweighting_drop": round_float(uniform_drop),
            "worst_observed_repo_subset_drop": round_float(worst_subset_drop),
            "uniform_drop_threshold": uniform_drop_threshold,
            "severe_drop_threshold": severe_drop_threshold,
            "severe_uniform_drop": severe_uniform_bad,
        },
        "repo_atomic_candidate_split": candidate_split,
        "decision": {
            "repo_atomic_retraining_performed": False,
            "repo_distribution_goodhart_risk_detected_on_current_fsv": bool(uniform_bad or subset_bad),
            "full_repo_atomic_disjoint_evaluation_required": full_required,
            "canonical_full_execution_issue": "#766" if full_required else None,
            "decision_reasons": decision_reasons,
            "ship_gate_repo_generalization_claim_allowed": broad_enough and not uniform_bad and not subset_bad,
        },
        "doctrine_policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "predictor_weights_changed": False,
            "repo_atomic_retraining_performed": False,
            "aiwonder_durable_root_required": True,
        },
        "artifacts": {
            "per_repo_holdout_metrics": str(per_repo_path),
            "repo_subset_holdout_metrics": str(subsets_path),
            "state_run_root": str(state_run_root),
        },
    }


def write_artifact_hashes(root: Path, paths: list[Path]) -> Path:
    rows = []
    for path in sorted(paths):
        if path.is_file():
            rows.append({"path": str(path), "sha256": sha256_file(path), "bytes": path.stat().st_size})
    out = root / "artifact_hashes.json"
    write_json(out, {"artifact_count": len(rows), "artifacts": rows})
    return out


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    require_archive_path(output_root, "output_root", bool(args.allow_non_aiwonder_roots))
    require_archive_path(state_root, "state_root", bool(args.allow_non_aiwonder_roots))
    run_id = str(args.run_id or f"run-{utc_stamp()}")
    fsv_run_root = output_root / run_id
    state_run_root = state_root / run_id
    fsv_run_root.mkdir(parents=True, exist_ok=False)
    state_run_root.mkdir(parents=True, exist_ok=False)

    payload = read_json(Path(args.ship_gate_report), "REPO_DISTRIBUTION_AUDIT_SHIP_GATE_REPORT_MISSING")
    execute = _require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    calibration_source, calibration_rows = raw_prediction_rows(execute, "calibration")
    holdout_source, holdout_rows = raw_prediction_rows(execute, "holdout")
    task_repos = None
    if args.tasks_dir is not None:
        task_repos = load_task_repo_counts(Path(args.tasks_dir), args.expected_task_count)

    per_repo_path = fsv_run_root / "per_repo_holdout_metrics.jsonl"
    subsets_path = fsv_run_root / "repo_subset_holdout_metrics.jsonl"
    manifest = build_manifest(
        ship_gate_report=Path(args.ship_gate_report),
        calibration_source=calibration_source,
        holdout_source=holdout_source,
        calibration_rows=calibration_rows,
        holdout_rows=holdout_rows,
        task_repos=task_repos,
        min_generalization_repos=int(args.min_generalization_repos),
        holdout_repo_count=int(args.holdout_repo_count),
        target_holdout_fraction=float(args.target_holdout_fraction),
        uniform_drop_threshold=float(args.uniform_drop_threshold),
        severe_drop_threshold=float(args.severe_drop_threshold),
        per_repo_path=per_repo_path,
        subsets_path=subsets_path,
        state_run_root=state_run_root,
        run_id=run_id,
    )
    manifest_path = fsv_run_root / "repo_distribution_goodhart_manifest.json"
    write_json(manifest_path, manifest)
    state_manifest_path = state_run_root / manifest_path.name
    state_per_repo_path = state_run_root / per_repo_path.name
    state_subsets_path = state_run_root / subsets_path.name
    write_json(state_manifest_path, manifest)
    write_jsonl(state_per_repo_path, [json.loads(line) for line in per_repo_path.read_text(encoding="utf-8").splitlines()])
    write_jsonl(state_subsets_path, [json.loads(line) for line in subsets_path.read_text(encoding="utf-8").splitlines()])
    artifacts = [manifest_path, per_repo_path, subsets_path, state_manifest_path, state_per_repo_path, state_subsets_path]
    hash_path = write_artifact_hashes(fsv_run_root, artifacts)
    state_hash_path = state_run_root / hash_path.name
    write_json(state_hash_path, read_json(hash_path, "REPO_DISTRIBUTION_AUDIT_HASH_JSON_INVALID"))
    return {
        "ok": True,
        "run_id": run_id,
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "manifest": str(manifest_path),
        "per_repo_holdout_metrics": str(per_repo_path),
        "repo_subset_holdout_metrics": str(subsets_path),
        "artifact_hashes": str(hash_path),
        "decision": manifest["decision"],
        "repo_coverage": manifest["repo_coverage"],
        "metrics": manifest["metrics"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit repo-distribution Goodhart risk from a ship-gate FSV report")
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--tasks-dir", type=Path, default=DEFAULT_TASKS_DIR)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--expected-task-count", type=int, default=300)
    parser.add_argument("--min-generalization-repos", type=int, default=8)
    parser.add_argument("--holdout-repo-count", type=int, default=4)
    parser.add_argument("--target-holdout-fraction", type=float, default=0.20)
    parser.add_argument("--uniform-drop-threshold", type=float, default=0.05)
    parser.add_argument("--severe-drop-threshold", type=float, default=0.10)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = run_audit(args)
    except RepoDistributionAuditError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, indent=2, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
