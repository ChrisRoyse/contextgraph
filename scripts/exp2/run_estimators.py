#!/usr/bin/env python3
"""T-015..T-020 pairwise MI estimator orchestration."""

from __future__ import annotations

import argparse
import itertools
from pathlib import Path
import sys

import numpy as np

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import (
    EMBEDDER_KEYS,
    EMBEDDER_LABELS,
    Exp2Error,
    load_config,
    output_dir,
    read_json,
    repo_path,
    setup_logging,
    utc_now,
    write_json,
)
from scripts.exp2.estimators.bootstrap import bootstrap_ci
from scripts.exp2.estimators.infonce import InfoNCEEstimator
from scripts.exp2.estimators.ksg import ksg_mi


def pair_name(i: int, j: int) -> str:
    return f"E{i + 1}_E{j + 1}"


def load_projected(path: str) -> dict[str, np.ndarray]:
    data = np.load(repo_path(path), allow_pickle=False)
    arrays = {key: np.asarray(data[key], dtype=np.float32) for key in EMBEDDER_KEYS}
    n = next(iter(arrays.values())).shape[0]
    for key, arr in arrays.items():
        if arr.shape != (n, arr.shape[1]) or arr.shape[1] not in {32, 64}:
            raise Exp2Error(f"Projected array {key} has invalid shape {arr.shape}")
        if not np.isfinite(arr).all():
            raise Exp2Error(f"Projected array {key} contains non-finite values")
    return arrays


def run_pair(cfg: dict, arrays: dict[str, np.ndarray], i: int, j: int, out_dir) -> dict:
    name = pair_name(i, j)
    u = arrays[EMBEDDER_KEYS[i]]
    v = arrays[EMBEDDER_KEYS[j]]
    seed = int(cfg["run"]["random_seed"]) + i * 100 + j
    estimator = InfoNCEEstimator(cfg["infonce"], seed=seed)
    info = estimator.fit(u, v)
    critic_path = out_dir / "critics" / f"{name}.pt"
    estimator.save(critic_path)
    k = int(cfg["ksg"]["k"])
    ksg_value = ksg_mi(u, v, k=k)
    bcfg = cfg["bootstrap"]
    n_resamples = int(bcfg["n_resamples"])
    inf_low, inf_high, _ = bootstrap_ci(
        lambda idx: estimator.estimate(u[idx], v[idx]),
        n_items=u.shape[0],
        n_resamples=n_resamples,
        seed=int(bcfg["seed"]),
        confidence=float(bcfg["confidence"]),
    )
    ksg_low, ksg_high, _ = bootstrap_ci(
        lambda idx: ksg_mi(u[idx], v[idx], k=k, jitter_seed=int(bcfg["seed"])),
        n_items=u.shape[0],
        n_resamples=n_resamples,
        seed=int(bcfg["seed"]),
        confidence=float(bcfg["confidence"]),
    )
    delta = abs(info.estimate_nats - ksg_value)
    row = {
        "pair": name,
        "embedder_j": EMBEDDER_LABELS[i],
        "embedder_k": EMBEDDER_LABELS[j],
        "infonce_estimate_nats": info.estimate_nats,
        "infonce_ci_low": inf_low,
        "infonce_ci_high": inf_high,
        "infonce_train_loss_final": -info.train_median_last,
        "infonce_val_loss_final": -info.val_median_last,
        "infonce_train_val_gap": info.train_val_gap,
        "infonce_stopped_step": info.stopped_step,
        "infonce_critic_path": str(critic_path.relative_to(repo_path("."))),
        "ksg_estimate_nats": ksg_value,
        "ksg_ci_low": ksg_low,
        "ksg_ci_high": ksg_high,
        "ksg_bootstrap_duplicate_policy": "deterministic_1e-12_tie_break_jitter",
        "agreement_nats": delta,
        "agreement_pass": delta <= float(cfg["cross_validation"]["agreement_threshold_nats"]),
        "run_seed": seed,
        "run_timestamp": utc_now(),
    }
    write_json(out_dir / "pair_results" / f"{name}.json", row)
    return row


def cross_validation_report(cfg: dict, rows: list[dict], out_dir) -> dict:
    failures = [r for r in rows if not r["agreement_pass"]]
    rate = (len(rows) - len(failures)) / len(rows)
    report = {
        "agreement_threshold_nats": cfg["cross_validation"]["agreement_threshold_nats"],
        "agreement_rate": rate,
        "passed_pairs": len(rows) - len(failures),
        "total_pairs": len(rows),
        "failures": [
            {
                "pair": r["pair"],
                "infonce": r["infonce_estimate_nats"],
                "ksg": r["ksg_estimate_nats"],
                "delta": r["agreement_nats"],
                "hypothesis": "estimator disagreement; inspect PCA variance, InfoNCE loss curve, and KSG k-sweep",
            }
            for r in failures
        ],
    }
    write_json(out_dir / "cross_validation_report.json", report)
    if rate < float(cfg["cross_validation"]["min_agreement_rate"]):
        raise Exp2Error(f"Cross-validation agreement {rate:.3f} below required threshold")
    return report


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--projected", default="artefacts/exp2/projected.npz")
    parser.add_argument("--limit-pairs", type=int)
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    log = setup_logging(out)
    arrays = load_projected(args.projected)
    pairs = list(itertools.combinations(range(len(EMBEDDER_KEYS)), 2))
    if args.limit_pairs is not None:
        pairs = pairs[: args.limit_pairs]
    rows = []
    for i, j in pairs:
        log.info("Running pair %s", pair_name(i, j))
        rows.append(run_pair(cfg, arrays, i, j, out))
    if len(rows) == 91:
        cross_validation_report(cfg, rows, out)
    else:
        write_json(out / "pilot_results.json", {"pairs": rows, "total": len(rows)})


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 estimator run failed: {exc}") from exc
