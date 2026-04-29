#!/usr/bin/env python3
"""T-019..T-021 entropy and effective-rank computations."""

from __future__ import annotations

import argparse
import itertools
import math
from pathlib import Path
import sys

import numpy as np

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import EMBEDDER_KEYS, EMBEDDER_LABELS, Exp2Error, load_config, output_dir, read_json, repo_path, setup_logging, write_json
from scripts.exp2.estimators.bootstrap import bootstrap_ci
from scripts.exp2.estimators.infonce import InfoNCEEstimator
from scripts.exp2.estimators.ksg import ksg_entropy


def n_eff_redundancy_adjusted(mi_matrix: np.ndarray, entropy: np.ndarray) -> float:
    n = mi_matrix.shape[0]
    total = float(n)
    for i in range(n):
        for j in range(i + 1, n):
            denom = min(float(entropy[i]), float(entropy[j]))
            if denom <= 0:
                raise Exp2Error(f"Non-positive entropy denominator for pair {(i, j)}: {denom}")
            redundancy = min(1.0, max(0.0, float(mi_matrix[i, j]) / denom))
            total += 1.0 - redundancy
    return total


def backfill_pair_redundancy(rows: dict[str, dict], entropy: np.ndarray, out_dir: Path) -> None:
    """Per spec §13 each pair row carries redundancy = I / min(H_j, H_k).

    run_estimators.run_pair writes redundancy=None because entropies aren't yet known
    at row-write time. Once estimate_entropies() has produced per-embedder H, we fill
    the value in and rewrite the row in place. Idempotent.
    """
    for i, j in itertools.combinations(range(14), 2):
        key = f"E{i + 1}_E{j + 1}"
        row = rows[key]
        denom = min(float(entropy[i]), float(entropy[j]))
        if denom <= 0:
            raise Exp2Error(f"Non-positive entropy denominator for pair {key}: {denom}")
        mi = float(row["infonce_estimate_nats"])
        row["redundancy"] = min(1.0, max(0.0, mi / denom))
        write_json(out_dir / "pair_results" / f"{key}.json", row)


def n_eff_spectrum_entropy(mi_matrix: np.ndarray) -> tuple[float, np.ndarray]:
    norm = np.linalg.norm(mi_matrix, ord="fro")
    if norm <= 0:
        raise Exp2Error("MI matrix Frobenius norm is zero")
    eigenvalues = np.linalg.eigvalsh(mi_matrix / norm)
    weights = np.abs(eigenvalues)
    weights = weights / np.sum(weights)
    entropy = -float(np.sum(weights * np.log(weights + 1e-15)))
    return float(np.exp(entropy)), eigenvalues


def n_eff_schur_complement(mi_matrix: np.ndarray, threshold: float = 0.05) -> int:
    sv = np.linalg.svd(mi_matrix, compute_uv=False)
    return int(np.sum(sv > threshold * sv[0]))


def load_pair_rows(out_dir) -> dict[str, dict]:
    pair_dir = out_dir / "pair_results"
    rows = {}
    for path in sorted(pair_dir.glob("E*_E*.json")):
        rows[path.stem] = read_json(path)
    if len(rows) != 91:
        raise Exp2Error(f"Expected 91 pair result files, found {len(rows)} in {pair_dir}")
    return rows


def mi_matrix_from_rows(rows: dict[str, dict], method: str = "infonce") -> np.ndarray:
    m = np.zeros((14, 14), dtype=np.float64)
    for i, j in itertools.combinations(range(14), 2):
        row = rows[f"E{i + 1}_E{j + 1}"]
        value = float(row[f"{method}_estimate_nats"])
        if value < 0 or not math.isfinite(value):
            raise Exp2Error(f"{row['pair']} invalid {method} MI: {value}")
        m[i, j] = m[j, i] = value
    return m


def estimate_entropies(cfg: dict, arrays: dict[str, np.ndarray], out_dir) -> np.ndarray:
    bcfg = cfg["bootstrap"]
    k = int(cfg["ksg"]["k"])
    entries = []
    ent = np.zeros(14, dtype=np.float64)
    for i, key in enumerate(EMBEDDER_KEYS):
        x = arrays[key]
        point = ksg_entropy(x, k=k)
        if point <= 0:
            raise Exp2Error(f"{key}: KSG entropy is non-positive ({point}); inspect projection")
        low, high, _ = bootstrap_ci(
            lambda idx, arr=x: ksg_entropy(arr[idx], k=k, jitter_seed=int(bcfg["seed"])),
            n_items=x.shape[0],
            n_resamples=int(bcfg["n_resamples"]),
            seed=int(bcfg["seed"]),
            confidence=float(bcfg["confidence"]),
        )
        ent[i] = point
        entries.append({"embedder": EMBEDDER_LABELS[i], "entropy_nats": point, "ci_low": low, "ci_high": high})
    write_json(out_dir / "entropy_per_embedder.json", {"entries": entries})
    return ent


def bootstrap_n_eff(cfg: dict, arrays: dict[str, np.ndarray], rows: dict[str, dict], entropy: np.ndarray) -> tuple[float, float]:
    bcfg = cfg["bootstrap"]
    n_items = next(iter(arrays.values())).shape[0]
    critics: dict[tuple[int, int], InfoNCEEstimator] = {}
    for i, j in itertools.combinations(range(14), 2):
        row = rows[f"E{i + 1}_E{j + 1}"]
        estimator = InfoNCEEstimator(cfg["infonce"], seed=int(row["run_seed"]))
        estimator.load(row["infonce_critic_path"], arrays[EMBEDDER_KEYS[i]].shape[1])
        critics[(i, j)] = estimator

    def point(idx: np.ndarray) -> float:
        m = np.zeros((14, 14), dtype=np.float64)
        for a in range(14):
            m[a, a] = entropy[a]
        for i, j in itertools.combinations(range(14), 2):
            value = critics[(i, j)].estimate(arrays[EMBEDDER_KEYS[i]][idx], arrays[EMBEDDER_KEYS[j]][idx])
            m[i, j] = m[j, i] = max(0.0, value)
        return n_eff_redundancy_adjusted(m, entropy)

    low, high, _ = bootstrap_ci(point, n_items, int(bcfg["n_resamples"]), int(bcfg["seed"]), float(bcfg["confidence"]))
    return low, high


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--projected", default="artefacts/exp2/projected.npz")
    parser.add_argument("--audit-chain", help="Path to phase audit chain JSON to append to.")
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    log = setup_logging(out)
    projected = np.load(repo_path(args.projected), allow_pickle=False)
    arrays = {key: np.asarray(projected[key], dtype=np.float32) for key in EMBEDDER_KEYS}
    rows = load_pair_rows(out)
    entropy = estimate_entropies(cfg, arrays, out)
    backfill_pair_redundancy(rows, entropy, out)
    mi = mi_matrix_from_rows(rows, "infonce")
    for i in range(14):
        mi[i, i] = entropy[i]
    primary = n_eff_redundancy_adjusted(mi, entropy)
    spectrum, eigenvalues = n_eff_spectrum_entropy(mi)
    schur = n_eff_schur_complement(mi, float(cfg["effective_rank"]["schur_threshold"]))
    low, high = bootstrap_n_eff(cfg, arrays, rows, entropy)
    summary = {
        "headline": {
            "n_eff": primary,
            "ci_low": low,
            "ci_high": high,
            "method": "redundancy_adjusted",
            "panel_size": 14,
            "structural_upper_bound": 105,
        },
        "n_eff_redundancy_adjusted": {"point": primary, "ci_low": low, "ci_high": high},
        "n_eff_spectrum_entropy": {"point": spectrum},
        "n_eff_schur_complement_int": {"point": schur},
        "eigenvalues": eigenvalues.tolist(),
        "n_total_pairs": 91,
        "n_marginal_embedders": 14,
    }
    write_json(out / "n_eff_summary.json", summary)
    log.info("N_eff redundancy-adjusted %.6f [%.6f, %.6f]", primary, low, high)

    if args.audit_chain:
        from scripts.exp2.audit_chain import AuditChain
        from scripts.exp2.common import sha256_file as _sha256_file

        chain = AuditChain(cfg["run_id"], args.audit_chain, cfg["run"]["random_seed"])
        chain.add_event("n_eff_complete", {"primary": primary, "ci_low": low, "ci_high": high})
        chain.add_artefact(out / "n_eff_summary.json", "n_eff_summary", created_by="T-021")
        chain.add_artefact(out / "entropy_per_embedder.json", "entropy_per_embedder", created_by="T-019")
        cfg_full = repo_path(args.config)
        hp_sha = _sha256_file(cfg_full) if cfg_full.exists() else None
        chain.finalize(hp_sha)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 effective-rank step failed: {exc}") from exc
