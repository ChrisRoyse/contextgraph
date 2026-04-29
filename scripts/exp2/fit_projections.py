#!/usr/bin/env python3
"""T-009..T-010 PCA fit and projection application."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

import numpy as np
from sklearn.decomposition import PCA

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import (
    EMBEDDER_KEYS,
    Exp2Error,
    l2_normalize,
    load_config,
    output_dir,
    repo_path,
    setup_logging,
    sha256_bytes,
    write_json,
)


def fit_pca_models(cfg: dict, training_npz: str, output_npz: str) -> None:
    out = output_dir(cfg)
    log = setup_logging(out)
    target_dim = int(cfg["pca"]["target_dim"])
    threshold = float(cfg["pca"]["variance_threshold"])
    data = np.load(repo_path(training_npz), allow_pickle=False)
    arrays: dict[str, np.ndarray] = {}
    metadata: dict[str, dict] = {}
    for key in EMBEDDER_KEYS:
        x = np.asarray(data[key], dtype=np.float32)
        if x.ndim != 2 or x.shape[0] <= target_dim:
            raise Exp2Error(f"{key}: PCA training shape invalid for d={target_dim}: {x.shape}")
        pca = PCA(n_components=target_dim, svd_solver="full", random_state=cfg["run"]["random_seed"])
        pca.fit(x)
        explained = float(np.sum(pca.explained_variance_ratio_))
        if explained < threshold:
            raise Exp2Error(
                f"{key}: PCA-{target_dim} retains {explained:.6f}, below {threshold}. "
                "Do not proceed. Pre-register a higher target_dim and rerun T-009."
            )
        arrays[f"{key}_components"] = pca.components_.astype(np.float32)
        arrays[f"{key}_mean"] = pca.mean_.astype(np.float32)
        arrays[f"{key}_explained_variance_ratio"] = pca.explained_variance_ratio_.astype(np.float32)
        model_hash = sha256_bytes(arrays[f"{key}_components"].tobytes() + arrays[f"{key}_mean"].tobytes())
        metadata[key] = {"target_dim": target_dim, "explained_variance_ratio_sum": explained, "sha256": model_hash}
        log.info("PCA fit %s explained=%.6f sha=%s", key, explained, model_hash[:12])
    arrays["metadata_json"] = np.asarray([json.dumps(metadata, sort_keys=True)])
    np.savez_compressed(repo_path(output_npz), **arrays)
    write_json(out / "pca_validation_report.json", metadata)


def apply_pca_models(cfg: dict, embeddings_npz: str, models_npz: str, output_npz: str) -> None:
    out = output_dir(cfg)
    log = setup_logging(out)
    embeddings = np.load(repo_path(embeddings_npz), allow_pickle=False)
    models = np.load(repo_path(models_npz), allow_pickle=False)
    projected = {
        "input_ids": embeddings["input_ids"],
        "memory_ids": embeddings["memory_ids"] if "memory_ids" in embeddings else np.asarray([]),
        "manifest_sha256": embeddings["manifest_sha256"] if "manifest_sha256" in embeddings else np.asarray([""]),
    }
    for key in EMBEDDER_KEYS:
        x = np.asarray(embeddings[key], dtype=np.float32)
        components = np.asarray(models[f"{key}_components"], dtype=np.float32)
        mean = np.asarray(models[f"{key}_mean"], dtype=np.float32)
        if x.shape[1] != mean.shape[0]:
            raise Exp2Error(f"{key}: embedding dim {x.shape[1]} != PCA mean dim {mean.shape[0]}")
        z = (x - mean) @ components.T
        projected[key] = l2_normalize(z, f"projected:{key}")
        log.info("Projected %s to %s", key, projected[key].shape)
    np.savez_compressed(repo_path(output_npz), **projected)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--fit", action="store_true")
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--training-npz", default="artefacts/exp2/pca_training_embeddings.npz")
    parser.add_argument("--embeddings-npz", default="artefacts/exp2/embeddings_per_input.npz")
    parser.add_argument("--models-npz", default="artefacts/exp2/pca_models.npz")
    parser.add_argument("--projected-npz", default="artefacts/exp2/projected.npz")
    args = parser.parse_args()
    cfg = load_config(args.config)
    if args.fit:
        fit_pca_models(cfg, args.training_npz, args.models_npz)
    if args.apply:
        apply_pca_models(cfg, args.embeddings_npz, args.models_npz, args.projected_npz)
    if not args.fit and not args.apply:
        raise Exp2Error("Specify --fit, --apply, or both")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 projection step failed: {exc}") from exc
