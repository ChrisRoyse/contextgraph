#!/usr/bin/env python3
"""EXP-2 full-state verification for persisted artefacts."""

from __future__ import annotations

import argparse
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import EMBEDDER_KEYS, Exp2Error, load_config, repo_path, sha256_file, write_json


REQUIRED_ARTEFACTS = [
    "corpus_manifest_v1.json",
    "embeddings_per_input.npz",
    "pca_models.npz",
    "projected.npz",
    "entropy_per_embedder.json",
    "n_eff_summary.json",
    "per_pair_table.csv",
    "redundancy_heatmap.png",
    "redundancy_heatmap.pdf",
]


def validate_reference(root: Path) -> dict[str, Any]:
    root = repo_path(root)
    if not root.exists():
        raise Exp2Error(f"Reference artefact dir missing: {root}")
    report: dict[str, Any] = {"artefacts": {}, "checks": []}

    def check(name: str, ok: bool, detail: Any) -> None:
        report["checks"].append({"name": name, "ok": ok, "detail": detail})
        if not ok:
            raise Exp2Error(f"FSV check failed: {name}: {detail}")

    for rel in REQUIRED_ARTEFACTS:
        path = root / rel
        check(f"exists:{rel}", path.exists(), str(path))
        report["artefacts"][rel] = sha256_file(path)
    projected = np.load(root / "projected.npz", allow_pickle=False)
    n = len(projected["input_ids"])
    check("projected_count", n == 800, n)
    for key in EMBEDDER_KEYS:
        arr = np.asarray(projected[key])
        check(f"projected_shape:{key}", arr.shape[0] == n and arr.shape[1] in {32, 64}, arr.shape)
        norms = np.linalg.norm(arr, axis=1)
        check(f"projected_l2:{key}", bool(np.allclose(norms, 1.0, atol=1e-4)), {"min": float(norms.min()), "max": float(norms.max())})
    pair_files = sorted((root / "pair_results").glob("E*_E*.json"))
    check("pair_result_count", len(pair_files) == 91, len(pair_files))
    summary = _read_json(root / "n_eff_summary.json")
    headline = summary["headline"]["n_eff"]
    check("headline_finite", math.isfinite(float(headline)), headline)
    return report


def compare_replay(reference: Path, replay: Path, tolerance: float) -> dict[str, Any]:
    reference = repo_path(reference)
    replay = repo_path(replay)
    report = {"tolerance": tolerance, "comparisons": []}
    for rel in ["n_eff_summary.json"]:
        ref = _read_json(reference / rel)
        rep = _read_json(replay / rel)
        _compare_scalar(report, rel, "headline.n_eff", ref["headline"]["n_eff"], rep["headline"]["n_eff"], tolerance)
    for ref_path in sorted((reference / "pair_results").glob("E*_E*.json")):
        rep_path = replay / "pair_results" / ref_path.name
        ref = _read_json(ref_path)
        rep = _read_json(rep_path)
        for key in ["infonce_estimate_nats", "ksg_estimate_nats", "agreement_nats"]:
            _compare_scalar(report, ref_path.name, key, ref[key], rep[key], tolerance)
    failures = [c for c in report["comparisons"] if not c["ok"]]
    if failures:
        raise Exp2Error(f"Replay comparison failed with {len(failures)} mismatches")
    return report


def _compare_scalar(report: dict, artefact: str, key: str, ref: float, rep: float, tolerance: float) -> None:
    diff = abs(float(ref) - float(rep))
    report["comparisons"].append({"artefact": artefact, "key": key, "reference": ref, "replay": rep, "diff": diff, "ok": diff <= tolerance})


def _read_json(path: Path) -> Any:
    import json

    with repo_path(path).open("r", encoding="utf-8") as fh:
        return json.load(fh)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--reference", default="artefacts/exp2")
    parser.add_argument("--replay")
    args = parser.parse_args()
    cfg = load_config(args.config)
    reference = repo_path(args.reference)
    report = {"reference": validate_reference(reference)}
    if args.replay:
        report["replay"] = compare_replay(reference, repo_path(args.replay), float(cfg["run"]["replay_tolerance"]))
    write_json(reference / "fsv_replay_log.json", report)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 FSV failed: {exc}") from exc
