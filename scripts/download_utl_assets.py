#!/usr/bin/env python3
"""Download UTL learner assets without writing credentials to disk.

This script intentionally has no fallback assets. Unsupported/manual assets
return a non-zero exit code with the missing source named explicitly.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from pathlib import Path
from typing import Any

from datasets import load_dataset
from huggingface_hub import snapshot_download


MODEL_SOURCES: dict[str, dict[str, str]] = {
    "e14_affect_speech": {
        "repo_id": "audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim",
        "local_dir": "affect-speech",
        "allow_patterns": "config.json,model.safetensors,preprocessor_config.json",
    },
    "e16_affect_text": {
        "repo_id": "sentence-transformers/all-MiniLM-L6-v2",
        "local_dir": "affect-text",
        "allow_patterns": "config.json,model.safetensors,tokenizer.json,vocab.txt,tokenizer_config.json,special_tokens_map.json",
    },
    "e19_eeg": {
        "repo_id": "braindecode/labram-pretrained",
        "local_dir": "eeg",
        "allow_patterns": "config.json,model.safetensors",
    },
    "e20_eeg_artifact_robust": {
        "repo_id": "braindecode/eegpt-pretrained",
        "local_dir": "eeg-robust",
        "allow_patterns": "config.json,model.safetensors",
    },
}

MANUAL_MODEL_SOURCES: dict[str, str] = {
    "e15_affect_face": "CMU-MultiComp-Lab/OpenFace-3.0 plus a local emotion-head checkpoint",
    "e17_ppg": "PaPaGei PPG foundation model research release",
    "e18_eda": "WESAD-trained local 1D-CNN checkpoint",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_csv(value: str) -> list[str]:
    if value == "none":
        return []
    if value == "all":
        return list(MODEL_SOURCES)
    return [part.strip() for part in value.split(",") if part.strip()]


def download_models(models_root: Path, requested: list[str]) -> dict[str, Any]:
    token = os.environ.get("HF_TOKEN") or os.environ.get("HUGGING_FACE_HUB_TOKEN")
    summary: dict[str, Any] = {"requested": requested, "downloaded": [], "failed": []}
    models_root.mkdir(parents=True, exist_ok=True)

    for model_id in requested:
        if model_id in MANUAL_MODEL_SOURCES:
            summary["failed"].append(
                {
                    "id": model_id,
                    "reason": "manual_or_local_training_required",
                    "source": MANUAL_MODEL_SOURCES[model_id],
                }
            )
            continue
        if model_id not in MODEL_SOURCES:
            summary["failed"].append({"id": model_id, "reason": "unknown_model_id"})
            continue

        spec = MODEL_SOURCES[model_id]
        target = models_root / spec["local_dir"]
        print(f"[model] downloading {model_id} from {spec['repo_id']} -> {target}")
        try:
            snapshot_download(
                repo_id=spec["repo_id"],
                local_dir=str(target),
                token=token,
                allow_patterns=parse_csv(spec.get("allow_patterns", "")),
                local_dir_use_symlinks=False,
                resume_download=True,
            )
        except Exception as exc:  # noqa: BLE001 - report exact upstream failure
            summary["failed"].append(
                {"id": model_id, "repo_id": spec["repo_id"], "reason": repr(exc)}
            )
            continue
        summary["downloaded"].append(
            {"id": model_id, "repo_id": spec["repo_id"], "path": str(target)}
        )
    return summary


def download_go_emotions(calibration_root: Path, limit: int) -> dict[str, Any]:
    target = calibration_root / "go_emotions"
    target.mkdir(parents=True, exist_ok=True)
    sample_path = target / "go_emotions_sample.jsonl"
    manifest_path = target / "manifest.json"

    print(f"[dataset] downloading google-research-datasets/go_emotions simplified train[:{limit}]")
    dataset = load_dataset(
        "google-research-datasets/go_emotions",
        "simplified",
        split=f"train[:{limit}]",
    )
    label_names = list(dataset.features["labels"].feature.names)

    row_count = 0
    with sample_path.open("w", encoding="utf-8") as handle:
        for row in dataset:
            label_ids = list(row["labels"])
            record = {
                "id": row["id"],
                "text": row["text"],
                "label_ids": label_ids,
                "label_names": [label_names[idx] for idx in label_ids],
            }
            handle.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")
            row_count += 1

    manifest = {
        "dataset_id": "google-research-datasets/go_emotions",
        "config": "simplified",
        "split": f"train[:{limit}]",
        "row_count": row_count,
        "source_of_truth": str(sample_path),
        "sha256": sha256_file(sample_path),
        "label_count": len(label_names),
    }
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return {"path": str(sample_path), "manifest": str(manifest_path), **manifest}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--models-root", type=Path, default=Path("models"))
    parser.add_argument("--calibration-root", type=Path, default=Path("data/utl_calibration"))
    parser.add_argument(
        "--models",
        default="none",
        help="Comma-separated E14-E20 model ids, 'all', or 'none'.",
    )
    parser.add_argument(
        "--datasets",
        default="go_emotions",
        choices=["go_emotions", "none"],
        help="Real calibration dataset to download.",
    )
    parser.add_argument("--limit", type=int, default=256)
    parser.add_argument("--summary", type=Path, default=None)
    args = parser.parse_args()

    if args.limit <= 0:
        raise SystemExit("--limit must be > 0")

    model_summary = download_models(args.models_root, parse_csv(args.models))
    dataset_summary = None
    if args.datasets == "go_emotions":
        dataset_summary = download_go_emotions(args.calibration_root, args.limit)

    summary = {
        "models": model_summary,
        "datasets": {"go_emotions": dataset_summary} if dataset_summary else {},
    }
    text = json.dumps(summary, indent=2, sort_keys=True)
    print(text)
    if args.summary:
        args.summary.parent.mkdir(parents=True, exist_ok=True)
        args.summary.write_text(text + "\n")

    return 1 if model_summary["failed"] else 0


if __name__ == "__main__":
    sys.exit(main())
