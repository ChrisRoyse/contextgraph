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
    canonical_json,
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


def pair_name(i: int, j: int) -> str:
    return f"E{i + 1}_E{j + 1}"


def load_provenance(cfg_path: str, out_dir: Path, pca_models_path: Path) -> dict:
    """Read required provenance hashes from upstream artefacts.

    EXP-2 rows are not valid unless every required upstream artefact exists and
    every hash is populated. Missing provenance is a hard error because it makes
    the result unreplayable.
    """
    code_commit_sha = AuditChain._git_commit()

    manifest_path = out_dir / "corpus_manifest_v1.json"
    if not manifest_path.exists():
        raise Exp2Error(f"Corpus manifest missing before estimator run: {manifest_path}")
    manifest = read_json(manifest_path)
    corpus_manifest_sha256 = manifest.get("manifest_sha256")
    if not isinstance(corpus_manifest_sha256, str) or len(corpus_manifest_sha256) != 64:
        raise Exp2Error(f"Corpus manifest has invalid manifest_sha256: {corpus_manifest_sha256}")
    sha_file = manifest_path.with_suffix(manifest_path.suffix + ".sha256")
    if not sha_file.exists():
        raise Exp2Error(f"Corpus manifest sidecar hash missing: {sha_file}")
    if sha_file.read_text(encoding="utf-8").strip() != corpus_manifest_sha256:
        raise Exp2Error(f"Corpus manifest sidecar hash mismatch: {sha_file}")
    canonical_manifest = dict(manifest)
    canonical_manifest.pop("manifest_sha256", None)
    from scripts.exp2.common import sha256_bytes

    if sha256_bytes(canonical_json(canonical_manifest)) != corpus_manifest_sha256:
        raise Exp2Error("Corpus manifest canonical hash does not match manifest_sha256")

    verif_log_path = out_dir / "verification_log.json"
    if not verif_log_path.exists():
        raise Exp2Error(f"Environment verification log missing before estimator run: {verif_log_path}")
    verif = read_json(verif_log_path)
    embedder_hashes = verif.get("embedder_hashes")
    if not isinstance(embedder_hashes, dict):
        raise Exp2Error("verification_log.json missing embedder_hashes; rerun T-001")
    missing_embedder_hashes = [key for key in EMBEDDER_KEYS if not embedder_hashes.get(key)]
    if missing_embedder_hashes:
        raise Exp2Error(f"verification_log.json missing embedder hashes: {missing_embedder_hashes}")

    if not pca_models_path.exists():
        raise Exp2Error(f"PCA model archive missing before estimator run: {pca_models_path}")
    models = np.load(pca_models_path, allow_pickle=False)
    if "metadata_json" not in models.files:
        raise Exp2Error(f"PCA model archive missing metadata_json: {pca_models_path}")
    meta = json.loads(str(models["metadata_json"][0]))
    pca_hashes = {key: entry.get("sha256") for key, entry in meta.items()}
    missing_pca_hashes = [key for key in EMBEDDER_KEYS if not pca_hashes.get(key)]
    if missing_pca_hashes:
        raise Exp2Error(f"PCA metadata missing hashes for embedders: {missing_pca_hashes}")

    cfg_full = repo_path(cfg_path)
    if not cfg_full.exists():
        raise Exp2Error(f"Config missing before estimator run: {cfg_full}")
    hyperparams_sha256 = sha256_file(cfg_full)

    return {
        "code_commit_sha": code_commit_sha,
        "corpus_manifest_sha256": corpus_manifest_sha256,
        "embedder_hashes": embedder_hashes,
        "pca_hashes": pca_hashes,
        "hyperparams_sha256": hyperparams_sha256,
    }


def embedder_weight_sha256(embedder_key: str, embedder_hashes: dict[str, str]) -> str:
    value = embedder_hashes.get(embedder_key)
    if not isinstance(value, str) or len(value) != 64:
        raise Exp2Error(f"Missing sha256 for embedder {embedder_key}")
    return value


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
    if info.estimate_nats < 0 or not np.isfinite(info.estimate_nats):
        raise Exp2Error(f"{name}: InfoNCE produced invalid MI estimate {info.estimate_nats}")
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
    embedder_hashes = provenance["embedder_hashes"]
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
        "embedder_j_weight_sha256": embedder_weight_sha256(EMBEDDER_KEYS[i], embedder_hashes),
        "embedder_k_weight_sha256": embedder_weight_sha256(EMBEDDER_KEYS[j], embedder_hashes),
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


def ksg_sensitivity_report(cfg: dict, arrays: dict[str, np.ndarray], pairs: list[tuple[int, int]], out_dir: Path) -> None:
    sweep_ks = [int(k) for k in cfg["ksg"]["k_sweep_pilot"]]
    threshold = 0.1
    report_rows = []
    for i, j in pairs[:10]:
        u = arrays[EMBEDDER_KEYS[i]]
        v = arrays[EMBEDDER_KEYS[j]]
        estimates = {str(k): ksg_mi(u, v, k=k) for k in sweep_ks}
        spread = max(estimates.values()) - min(estimates.values())
        row = {
            "pair": pair_name(i, j),
            "estimates": estimates,
            "spread_nats": spread,
            "threshold_nats": threshold,
            "pass": spread <= threshold,
        }
        report_rows.append(row)
    write_json(out_dir / "ksg_sensitivity_report.json", {"rows": report_rows})
    failures = [row for row in report_rows if not row["pass"]]
    if failures:
        raise Exp2Error(f"KSG k-sweep instability exceeded {threshold} nats: {failures[:3]}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--projected", default="artefacts/exp2/projected.npz")
    parser.add_argument("--pca-models", default="artefacts/exp2/pca_models.npz")
    parser.add_argument("--limit-pairs", type=int, help=argparse.SUPPRESS)
    parser.add_argument("--audit-chain", help="Path to phase audit chain JSON to append to.")
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    log = setup_logging(out)
    if args.limit_pairs is not None:
        raise Exp2Error("--limit-pairs is not allowed for EXP-2 production runs; run all 91 pairs")
    arrays = load_projected(args.projected)
    provenance = load_provenance(args.config, out, repo_path(args.pca_models))
    pairs = list(itertools.combinations(range(len(EMBEDDER_KEYS)), 2))
    ksg_sensitivity_report(cfg, arrays, pairs, out)
    rows = []
    for i, j in pairs:
        log.info("Running pair %s", pair_name(i, j))
        rows.append(run_pair(cfg, arrays, i, j, out, provenance))
    if len(rows) != 91:
        raise Exp2Error(f"Expected 91 pair results, got {len(rows)}")
    cross_validation_report(cfg, rows, out)

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
