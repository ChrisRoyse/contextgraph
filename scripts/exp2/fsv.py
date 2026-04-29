#!/usr/bin/env python3
"""EXP-2 full-state verification for persisted artefacts."""

from __future__ import annotations

import argparse
import math
import shutil
import subprocess
import uuid
from pathlib import Path
import sys
from typing import Any

import numpy as np
import yaml

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import (
    EMBEDDER_KEYS,
    EXPECTED_DIMS,
    Exp2Error,
    canonical_json,
    load_config,
    output_dir,
    repo_path,
    sha256_bytes,
    sha256_file,
    utc_now,
    write_json,
)


REQUIRED_ARTEFACTS = [
    "corpus_manifest_v1.json",
    "corpus_manifest_v1.json.sha256",
    "pca_training_corpus_v1.json",
    "embeddings_per_input.npz",
    "pca_models.npz",
    "projected.npz",
    "entropy_per_embedder.json",
    "n_eff_summary.json",
    "per_pair_table.csv",
    "redundancy_heatmap.png",
    "redundancy_heatmap.pdf",
    "spectrum_plot.png",
    "spectrum_plot.pdf",
    "entropy_bars.png",
    "entropy_bars.pdf",
    "cross_validation_report.json",
    "ksg_sensitivity_report.json",
    "embedder_swap_recommendation.md",
    "publishable_summary.md",
    "audit_chain.json",
]


PAIR_REQUIRED_KEYS = [
    "pair",
    "embedder_j",
    "embedder_k",
    "infonce_estimate_nats",
    "infonce_ci_low",
    "infonce_ci_high",
    "ksg_estimate_nats",
    "ksg_ci_low",
    "ksg_ci_high",
    "agreement_nats",
    "agreement_pass",
    "redundancy",
    "code_commit_sha",
    "corpus_manifest_sha256",
    "embedder_j_weight_sha256",
    "embedder_k_weight_sha256",
    "pca_j_sha256",
    "pca_k_sha256",
    "hyperparams_sha256",
    "run_seed",
]


def validate_reference(root: Path, cfg: dict[str, Any]) -> dict[str, Any]:
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
    manifest = _read_json(root / "corpus_manifest_v1.json")
    expected_count = int(cfg["corpus"]["expected_count"])
    expected_strata = int(cfg["corpus"]["expected_strata"])
    check("manifest_total_inputs", manifest.get("total_inputs") == expected_count, manifest.get("total_inputs"))
    strata = manifest.get("strata", {})
    check("manifest_strata_count", isinstance(strata, dict) and len(strata) == expected_strata, strata)
    for name, entry in sorted(strata.items()):
        check(f"manifest_stratum_count:{name}", entry.get("count") == 100, entry)
    check("manifest_no_deviations", "deviations" not in manifest, manifest.get("deviations"))
    sidecar = root / "corpus_manifest_v1.json.sha256"
    check("manifest_sha_sidecar_exists", sidecar.exists(), str(sidecar))
    canonical_manifest = dict(manifest)
    canonical_manifest.pop("manifest_sha256", None)
    computed_manifest_sha = sha256_bytes(canonical_json(canonical_manifest))
    check("manifest_canonical_sha", computed_manifest_sha == manifest.get("manifest_sha256"), computed_manifest_sha)
    check("manifest_sidecar_sha", sidecar.read_text(encoding="utf-8").strip() == manifest.get("manifest_sha256"), sidecar.read_text(encoding="utf-8").strip())

    embeddings = np.load(root / "embeddings_per_input.npz", allow_pickle=False)
    check("embeddings_count", len(embeddings["input_ids"]) == expected_count, len(embeddings["input_ids"]))
    for key in EMBEDDER_KEYS:
        arr = np.asarray(embeddings[key])
        check(f"embeddings_shape:{key}", arr.shape == (expected_count, EXPECTED_DIMS[key]), arr.shape)
        check(f"embeddings_finite:{key}", bool(np.isfinite(arr).all()), arr.shape)
        norms = np.linalg.norm(arr, axis=1)
        check(f"embeddings_nonzero:{key}", bool(np.all(norms > 0)), {"min": float(norms.min()), "max": float(norms.max())})

    projected = np.load(root / "projected.npz", allow_pickle=False)
    n = len(projected["input_ids"])
    check("projected_count", n == expected_count, n)
    for key in EMBEDDER_KEYS:
        arr = np.asarray(projected[key])
        check(f"projected_shape:{key}", arr.shape[0] == n and arr.shape[1] in {32, 64}, arr.shape)
        norms = np.linalg.norm(arr, axis=1)
        check(f"projected_l2:{key}", bool(np.allclose(norms, 1.0, atol=1e-4)), {"min": float(norms.min()), "max": float(norms.max())})
    pair_files = sorted((root / "pair_results").glob("E*_E*.json"))
    check("pair_result_count", len(pair_files) == 91, len(pair_files))
    for path in pair_files:
        row = _read_json(path)
        missing = [key for key in PAIR_REQUIRED_KEYS if key not in row]
        check(f"pair_schema:{path.name}", not missing, {"missing": missing})
        nulls = [key for key in PAIR_REQUIRED_KEYS if row.get(key) is None]
        check(f"pair_no_null_required:{path.name}", not nulls, {"nulls": nulls})
        for key in ["infonce_estimate_nats", "ksg_estimate_nats", "agreement_nats", "redundancy"]:
            check(f"pair_scalar_finite:{path.name}:{key}", math.isfinite(float(row[key])), row[key])
        check(f"pair_redundancy_bounds:{path.name}", 0.0 <= float(row["redundancy"]) <= 1.0, row["redundancy"])
    summary = _read_json(root / "n_eff_summary.json")
    headline = summary["headline"]["n_eff"]
    check("headline_finite", math.isfinite(float(headline)), headline)
    check("headline_structural_upper_bound", summary["headline"]["structural_upper_bound"] == 105, summary["headline"])
    cross = _read_json(root / "cross_validation_report.json")
    check("cross_validation_total_pairs", cross.get("total_pairs") == 91, cross)
    check(
        "cross_validation_agreement_rate",
        float(cross.get("agreement_rate", -1.0)) >= float(cfg["cross_validation"]["min_agreement_rate"]),
        cross,
    )
    return report


def compare_replay(reference: Path, replay: Path, tolerance: float) -> dict[str, Any]:
    reference = repo_path(reference)
    replay = repo_path(replay)
    report = {"tolerance": tolerance, "comparisons": []}
    for rel in ["n_eff_summary.json"]:
        ref = _read_json(reference / rel)
        rep = _read_json(replay / rel)
        _compare_scalar(report, rel, "headline.n_eff", ref["headline"]["n_eff"], rep["headline"]["n_eff"], tolerance)
        _compare_scalar(report, rel, "headline.ci_low", ref["headline"]["ci_low"], rep["headline"]["ci_low"], tolerance)
        _compare_scalar(report, rel, "headline.ci_high", ref["headline"]["ci_high"], rep["headline"]["ci_high"], tolerance)
        _compare_scalar(report, rel, "n_eff_spectrum_entropy.point", ref["n_eff_spectrum_entropy"]["point"], rep["n_eff_spectrum_entropy"]["point"], tolerance)
        _compare_scalar(report, rel, "n_eff_schur_complement_int.point", ref["n_eff_schur_complement_int"]["point"], rep["n_eff_schur_complement_int"]["point"], tolerance)
    for ref_path in sorted((reference / "pair_results").glob("E*_E*.json")):
        rep_path = replay / "pair_results" / ref_path.name
        ref = _read_json(ref_path)
        rep = _read_json(rep_path)
        for key in [
            "infonce_estimate_nats",
            "infonce_ci_low",
            "infonce_ci_high",
            "ksg_estimate_nats",
            "ksg_ci_low",
            "ksg_ci_high",
            "agreement_nats",
            "redundancy",
        ]:
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


def run_replay_pipeline(config_path: Path, reference: Path, replay: Path) -> None:
    replay = repo_path(replay)
    reference = repo_path(reference)
    if replay.exists():
        raise Exp2Error(f"Replay directory already exists; refusing to compare stale state: {replay}")
    replay.mkdir(parents=True)
    cfg = load_config(config_path)
    replay_cfg = dict(cfg)
    replay_cfg["run_id"] = f"{cfg['run_id']}_replay"
    replay_cfg["output_dir"] = str(replay)
    replay_cfg["corpus"] = dict(cfg["corpus"])
    replay_cfg["corpus"]["manifest"] = str(replay / "corpus_manifest_v1.json")
    replay_cfg["pca"] = dict(cfg["pca"])
    replay_cfg["pca"]["fit_corpus"] = str(replay / "pca_training_corpus_v1.json")
    shutil.copy2(reference / "corpus_manifest_v1.json", replay / "corpus_manifest_v1.json")
    shutil.copy2(reference / "corpus_manifest_v1.json.sha256", replay / "corpus_manifest_v1.json.sha256")
    shutil.copy2(reference / "pca_training_corpus_v1.json", replay / "pca_training_corpus_v1.json")
    shutil.copy2(reference / "verification_log.json", replay / "verification_log.json")
    replay_cfg_path = replay / "fsv_replay_config.yml"
    replay_cfg_path.write_text(yaml.safe_dump(replay_cfg, sort_keys=False), encoding="utf-8")
    commands = [
        [sys.executable, "scripts/exp2/extract_embeddings.py", "--config", str(replay_cfg_path)],
        [
            sys.executable,
            "scripts/exp2/extract_embeddings.py",
            "--config",
            str(replay_cfg_path),
            "--manifest",
            str(replay / "pca_training_corpus_v1.json"),
            "--output",
            str(replay / "pca_training_embeddings.npz"),
            "--storage-name",
            "pca",
        ],
        [
            sys.executable,
            "scripts/exp2/fit_projections.py",
            "--config",
            str(replay_cfg_path),
            "--fit",
            "--apply",
            "--training-npz",
            str(replay / "pca_training_embeddings.npz"),
            "--embeddings-npz",
            str(replay / "embeddings_per_input.npz"),
            "--models-npz",
            str(replay / "pca_models.npz"),
            "--projected-npz",
            str(replay / "projected.npz"),
        ],
        [
            sys.executable,
            "scripts/exp2/run_estimators.py",
            "--config",
            str(replay_cfg_path),
            "--projected",
            str(replay / "projected.npz"),
            "--pca-models",
            str(replay / "pca_models.npz"),
        ],
        [sys.executable, "scripts/exp2/effective_rank.py", "--config", str(replay_cfg_path), "--projected", str(replay / "projected.npz")],
        [sys.executable, "scripts/exp2/visualize.py", "--config", str(replay_cfg_path)],
    ]
    for cmd in commands:
        res = subprocess.run(cmd, cwd=repo_path("."), text=True)
        if res.returncode != 0:
            raise Exp2Error(f"Replay command failed ({res.returncode}): {' '.join(cmd)}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--reference", default="artefacts/exp2")
    parser.add_argument("--replay")
    args = parser.parse_args()
    cfg = load_config(args.config)
    reference = repo_path(args.reference)
    report = {"started_at": utc_now(), "reference": validate_reference(reference, cfg)}
    replay_dir = repo_path(args.replay) if args.replay else reference.parent / f"{reference.name}_replay_{uuid.uuid4().hex[:8]}"
    if not replay_dir.exists():
        run_replay_pipeline(repo_path(args.config), reference, replay_dir)
    report["replay"] = compare_replay(reference, replay_dir, float(cfg["run"]["replay_tolerance"]))
    report["replay"]["path"] = str(replay_dir)
    report["finished_at"] = utc_now()
    write_json(reference / "fsv_replay_log.json", report)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 FSV failed: {exc}") from exc
