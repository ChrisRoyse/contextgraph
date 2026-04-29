#!/usr/bin/env python3
"""T-015..T-020 pairwise MI estimator orchestration."""

from __future__ import annotations

import argparse
import itertools
import json
from pathlib import Path
import sys

import numpy as np

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.audit_chain import AuditChain
from scripts.exp2.common import (
    EMBEDDER_KEYS,
    EMBEDDER_LABELS,
    Exp2Error,
    load_config,
    output_dir,
    read_json,
    repo_path,
    setup_logging,
    sha256_file,
    utc_now,
    write_json,
)
from scripts.exp2.estimators.bootstrap import bootstrap_ci
from scripts.exp2.estimators.infonce import InfoNCEEstimator
from scripts.exp2.estimators.ksg import ksg_mi


# Per docs/exp2/03_methodology.md §13: each per-pair JSON row must trace back
# to the exact weights, projections, corpus, and code that produced it.
# Embedders without a learned weight file (E2/E3/E4 sinusoidal/Fourier PE,
# E9 deterministic HDC) record None — there is no weight to hash.
EMBEDDER_WEIGHT_FILES: dict[str, str | None] = {
    "e1_semantic": "models/semantic/model.safetensors",
    "e2_temporal_recent": None,
    "e3_temporal_periodic": None,
    "e4_temporal_positional": None,
    "e5_causal_cause": "models/causal/model.safetensors",
    "e6_sparse_top_k": "models/sparse/model.safetensors",
    "e7_code": "models/code-1536/model.safetensors.index.json",
    "e8_graph_source": "models/graph/model.safetensors",
    "e9_hdc": None,
    "e10_paraphrase": "models/multimodal/model.safetensors",
    "e11_entity": "models/entity/model.safetensors",
    "e12_token_pooled": "models/late-interaction/model.safetensors",
    "e13_splade_top_k": "models/splade-v3/model.safetensors",
    "e14_bge_m3_dense": "models/bge-m3-dense/model.safetensors",
}


def pair_name(i: int, j: int) -> str:
    return f"E{i + 1}_E{j + 1}"


def load_provenance(cfg_path: str, out_dir: Path, pca_models_path: Path) -> dict:
    """Read provenance hashes from upstream artefacts. Missing inputs surface as None.

    Per spec §13, every per-pair row must reference these so an external reviewer can
    re-derive the row from corpus + weights + code + hyperparams. Hashes that aren't
    available at row-write time (e.g. corpus manifest skipped in a partial run) are
    recorded as None; the AuditChain will flag dangling references at finalize().
    """
    code_commit_sha: str | None = None
    try:
        code_commit_sha = AuditChain._git_commit()
    except Exp2Error:
        # Repo without git or detached state. Honest None rather than fake hash.
        code_commit_sha = None

    corpus_manifest_sha256: str | None = None
    manifest_path = out_dir / "corpus_manifest_v1.json"
    if manifest_path.exists():
        manifest = read_json(manifest_path)
        corpus_manifest_sha256 = manifest.get("manifest_sha256")

    weight_hashes: dict[str, str] = {}
    verif_log_path = out_dir / "verification_log.json"
    if verif_log_path.exists():
        verif = read_json(verif_log_path)
        weight_hashes = verif.get("weight_hashes", {}) or {}

    pca_hashes: dict[str, str] = {}
    if pca_models_path.exists():
        models = np.load(pca_models_path, allow_pickle=False)
        if "metadata_json" in models.files:
            meta = json.loads(str(models["metadata_json"][0]))
            pca_hashes = {key: entry.get("sha256") for key, entry in meta.items()}

    hyperparams_sha256: str | None = None
    cfg_full = repo_path(cfg_path)
    if cfg_full.exists():
        hyperparams_sha256 = sha256_file(cfg_full)

    return {
        "code_commit_sha": code_commit_sha,
        "corpus_manifest_sha256": corpus_manifest_sha256,
        "weight_hashes": weight_hashes,
        "pca_hashes": pca_hashes,
        "hyperparams_sha256": hyperparams_sha256,
    }


def embedder_weight_sha256(embedder_key: str, weight_hashes: dict[str, str]) -> str | None:
    """Look up the sha256 of an embedder's weight file from verification_log hashes.

    Returns None for embedders without a learned weight file (E2/E3/E4/E9), and
    None when verification has not run yet.
    """
    rel = EMBEDDER_WEIGHT_FILES.get(embedder_key)
    if rel is None:
        return None
    return weight_hashes.get(rel)


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


def run_pair(
    cfg: dict,
    arrays: dict[str, np.ndarray],
    i: int,
    j: int,
    out_dir: Path,
    provenance: dict,
) -> dict:
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
    weight_hashes = provenance["weight_hashes"]
    pca_hashes = provenance["pca_hashes"]
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
        # Provenance fields per docs/exp2/03_methodology.md §13.
        # `redundancy` is computed in effective_rank.py once entropies are known;
        # here we record None so the schema is complete and consumers don't crash.
        "code_commit_sha": provenance["code_commit_sha"],
        "corpus_manifest_sha256": provenance["corpus_manifest_sha256"],
        "embedder_j_weight_sha256": embedder_weight_sha256(EMBEDDER_KEYS[i], weight_hashes),
        "embedder_k_weight_sha256": embedder_weight_sha256(EMBEDDER_KEYS[j], weight_hashes),
        "pca_j_sha256": pca_hashes.get(EMBEDDER_KEYS[i]),
        "pca_k_sha256": pca_hashes.get(EMBEDDER_KEYS[j]),
        "hyperparams_sha256": provenance["hyperparams_sha256"],
        "redundancy": None,
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
    parser.add_argument("--pca-models", default="artefacts/exp2/pca_models.npz")
    parser.add_argument("--limit-pairs", type=int)
    parser.add_argument("--audit-chain", help="Path to phase audit chain JSON to append to.")
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    log = setup_logging(out)
    arrays = load_projected(args.projected)
    provenance = load_provenance(args.config, out, repo_path(args.pca_models))
    pairs = list(itertools.combinations(range(len(EMBEDDER_KEYS)), 2))
    if args.limit_pairs is not None:
        pairs = pairs[: args.limit_pairs]
    rows = []
    for i, j in pairs:
        log.info("Running pair %s", pair_name(i, j))
        rows.append(run_pair(cfg, arrays, i, j, out, provenance))
    if len(rows) == 91:
        cross_validation_report(cfg, rows, out)
    else:
        write_json(out / "pilot_results.json", {"pairs": rows, "total": len(rows)})

    if args.audit_chain:
        chain = AuditChain(cfg["run_id"], args.audit_chain, cfg["run"]["random_seed"])
        chain.add_event(
            "estimators_complete",
            {
                "n_pairs": len(rows),
                "limit_pairs": args.limit_pairs,
                "agreement_threshold_nats": cfg["cross_validation"]["agreement_threshold_nats"],
            },
        )
        for row in rows:
            chain.add_artefact(
                out / "pair_results" / f"{row['pair']}.json",
                f"pair_result:{row['pair']}",
                created_by="T-016",
            )
        chain.finalize(provenance["hyperparams_sha256"])


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 estimator run failed: {exc}") from exc
