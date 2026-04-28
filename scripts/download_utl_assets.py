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
import subprocess
import sys
import zipfile
from pathlib import Path
from typing import Any
from urllib.request import urlopen

from datasets import load_dataset
from huggingface_hub import snapshot_download


MODEL_SOURCES: dict[str, dict[str, str]] = {
    "e16_affect_face": {
        "repo_url": "https://github.com/CMU-MultiComp-Lab/OpenFace-3.0.git",
        "weights_repo_id": "nutPace/openface_weights",
        "local_dir": "affect-face/OpenFace",
        "weights_dir": "affect-face/OpenFace/weights",
        "allow_patterns": "Alignment_RetinaFace.pth,Landmark_68.pkl,Landmark_98.pkl,MTL_backbone.pth,mobilenetV1X0.25_pretrain.tar",
    },
    "e15_affect_speech": {
        "repo_id": "audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim",
        "local_dir": "affect-speech",
        "allow_patterns": "config.json,model.safetensors,preprocessor_config.json",
    },
    "e17_affect_text": {
        "repo_id": "sentence-transformers/all-MiniLM-L6-v2",
        "local_dir": "affect-text",
        "allow_patterns": "config.json,model.safetensors,tokenizer.json,vocab.txt,tokenizer_config.json,special_tokens_map.json",
    },
    "e20_eeg": {
        "repo_id": "braindecode/labram-pretrained",
        "local_dir": "eeg",
        "allow_patterns": "config.json,model.safetensors",
    },
    "e21_eeg_artifact_robust": {
        "repo_id": "braindecode/eegpt-pretrained",
        "local_dir": "eeg-robust",
        "allow_patterns": "config.json,model.safetensors",
    },
}

MANUAL_MODEL_SOURCES: dict[str, str] = {
    "e19_eda": "WESAD-trained local stress-floor head; run scripts/train_utl_eda_stress_head.py",
}

MANUAL_MODEL_FILES: dict[str, dict[str, Any]] = {
    "e19_eda": {
        "local_dir": "eda",
        "required": ["config.json", "wesad_stress_head.pt"],
    },
}

ZENODO_SOURCES: dict[str, dict[str, Any]] = {
    "e18_ppg": {
        "url": "https://zenodo.org/records/13983110/files/papagei_s.pt?download=1",
        "local_dir": "ppg",
        "filename": "papagei_s.pt",
        "config": {
            "model": "PaPaGei-S",
            "source": "https://zenodo.org/records/13983110",
            "doi": "10.5281/zenodo.13983110",
            "embedding_dimension": 512,
            "architecture": "ResNet1DMoE",
            "in_channels": 1,
            "base_filters": 32,
            "kernel_size": 3,
            "stride": 2,
            "groups": 1,
            "n_block": 18,
            "n_classes": 512,
            "n_experts": 3,
            "target_sample_rate_hz": 125,
        },
    },
}

WESAD_OFFICIAL_URL = "https://uni-siegen.sciebo.de/s/HGdUkoNlW1Ub0Gx/download"

for _model_id in ZENODO_SOURCES:
    MANUAL_MODEL_SOURCES.pop(_model_id, None)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download_url(url: str, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    with urlopen(url) as response, target.open("wb") as handle:
        while True:
            chunk = response.read(1024 * 1024)
            if not chunk:
                break
            handle.write(chunk)


def ensure_git_cache(repo_url: str, target: Path) -> str:
    if target.exists():
        if not (target / ".git").is_dir():
            raise RuntimeError(f"{target} exists but is not a git checkout")
        subprocess.run(
            ["git", "-C", str(target), "fetch", "--depth", "1", "origin"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["git", "clone", "--depth", "1", repo_url, str(target)],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    rev = subprocess.run(
        ["git", "-C", str(target), "rev-parse", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return rev.stdout.strip()


def parse_csv(value: str) -> list[str]:
    if value == "none":
        return []
    if value == "all":
        return [
            *MODEL_SOURCES,
            *ZENODO_SOURCES,
            *MANUAL_MODEL_SOURCES,
        ]
    return [part.strip() for part in value.split(",") if part.strip()]


def download_models(models_root: Path, requested: list[str]) -> dict[str, Any]:
    token = os.environ.get("HF_TOKEN") or os.environ.get("HUGGING_FACE_HUB_TOKEN")
    summary: dict[str, Any] = {"requested": requested, "downloaded": [], "failed": []}
    models_root.mkdir(parents=True, exist_ok=True)

    for model_id in requested:
        if model_id in MANUAL_MODEL_SOURCES:
            spec = MANUAL_MODEL_FILES[model_id]
            target_dir = models_root / spec["local_dir"]
            missing = [
                filename
                for filename in spec["required"]
                if not (target_dir / filename).is_file()
            ]
            if not missing:
                summary["downloaded"].append(
                    {
                        "id": model_id,
                        "source": MANUAL_MODEL_SOURCES[model_id],
                        "path": str(target_dir),
                        "hashes": {
                            filename: sha256_file(target_dir / filename)
                            for filename in spec["required"]
                        },
                    }
                )
                continue
            summary["failed"].append(
                {
                    "id": model_id,
                    "reason": "manual_or_local_training_required",
                    "source": MANUAL_MODEL_SOURCES[model_id],
                    "missing": missing,
                }
            )
            continue
        if model_id in ZENODO_SOURCES:
            spec = ZENODO_SOURCES[model_id]
            target_dir = models_root / spec["local_dir"]
            target_file = target_dir / spec["filename"]
            config_file = target_dir / "config.json"
            print(f"[model] downloading {model_id} from {spec['url']} -> {target_file}")
            try:
                download_url(spec["url"], target_file)
                config_file.write_text(
                    json.dumps(spec["config"], indent=2, sort_keys=True) + "\n"
                )
            except Exception as exc:  # noqa: BLE001 - report exact upstream failure
                summary["failed"].append(
                    {"id": model_id, "url": spec["url"], "reason": repr(exc)}
                )
                continue
            summary["downloaded"].append(
                {
                    "id": model_id,
                    "url": spec["url"],
                    "path": str(target_dir),
                    "sha256": sha256_file(target_file),
                }
            )
            continue
        if model_id not in MODEL_SOURCES:
            summary["failed"].append({"id": model_id, "reason": "unknown_model_id"})
            continue

        spec = MODEL_SOURCES[model_id]
        if "weights_repo_id" in spec:
            code_target = models_root / spec["local_dir"]
            weights_target = models_root / spec["weights_dir"]
            print(f"[model] caching {model_id} code from {spec['repo_url']} -> {code_target}")
            try:
                git_rev = ensure_git_cache(spec["repo_url"], code_target)
                print(
                    f"[model] downloading {model_id} weights from "
                    f"{spec['weights_repo_id']} -> {weights_target}"
                )
                snapshot_download(
                    repo_id=spec["weights_repo_id"],
                    local_dir=str(weights_target),
                    token=token,
                    allow_patterns=parse_csv(spec.get("allow_patterns", "")),
                    local_dir_use_symlinks=False,
                    resume_download=True,
                )
            except Exception as exc:  # noqa: BLE001 - report exact upstream failure
                summary["failed"].append(
                    {
                        "id": model_id,
                        "repo_url": spec["repo_url"],
                        "weights_repo_id": spec["weights_repo_id"],
                        "reason": repr(exc),
                    }
                )
                continue
            summary["downloaded"].append(
                {
                    "id": model_id,
                    "repo_url": spec["repo_url"],
                    "git_rev": git_rev,
                    "weights_repo_id": spec["weights_repo_id"],
                    "path": str(code_target),
                    "weights_path": str(weights_target),
                    "weight_hashes": {
                        str(path.relative_to(weights_target)): sha256_file(path)
                        for path in sorted(weights_target.glob("*"))
                        if path.is_file()
                    },
                }
            )
            continue

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


def download_wesad_official(calibration_root: Path) -> dict[str, Any]:
    target = calibration_root / "wesad_official"
    target.mkdir(parents=True, exist_ok=True)
    archive_path = target / "WESAD.zip"
    manifest_path = target / "manifest.json"

    if archive_path.exists():
        print(f"[dataset] using cached official WESAD archive -> {archive_path}")
    else:
        print(f"[dataset] downloading official WESAD archive -> {archive_path}")
        download_url(WESAD_OFFICIAL_URL, archive_path)

    with zipfile.ZipFile(archive_path) as archive:
        members = archive.namelist()
    subject_pickles = sorted(
        member
        for member in members
        if member.startswith("WESAD/S") and member.endswith(".pkl")
    )
    if not subject_pickles:
        raise RuntimeError(f"no subject pickle files found in {archive_path}")

    manifest = {
        "dataset_id": "WESAD",
        "source": "https://ubi29.informatik.uni-siegen.de/usi/data_wesad.html",
        "download_url": WESAD_OFFICIAL_URL,
        "source_of_truth": str(archive_path),
        "archive_bytes": archive_path.stat().st_size,
        "archive_sha256": sha256_file(archive_path),
        "subject_pickle_count": len(subject_pickles),
        "subject_pickle_members": subject_pickles,
    }
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return {"path": str(archive_path), "manifest": str(manifest_path), **manifest}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--models-root", type=Path, default=Path("models"))
    parser.add_argument("--calibration-root", type=Path, default=Path("data/utl_calibration"))
    parser.add_argument(
        "--models",
        default="none",
        help="Comma-separated E15-E21 model ids, 'all', or 'none'.",
    )
    parser.add_argument(
        "--datasets",
        default="go_emotions",
        help="Comma-separated datasets: go_emotions, wesad_official, all, or none.",
    )
    parser.add_argument("--limit", type=int, default=256)
    parser.add_argument("--summary", type=Path, default=None)
    args = parser.parse_args()

    if args.limit <= 0:
        raise SystemExit("--limit must be > 0")

    model_summary = download_models(args.models_root, parse_csv(args.models))
    dataset_summary: dict[str, Any] = {}
    requested_datasets = parse_csv(args.datasets)
    if args.datasets == "all":
        requested_datasets = ["go_emotions", "wesad_official"]
    for dataset_id in requested_datasets:
        if dataset_id == "go_emotions":
            dataset_summary["go_emotions"] = download_go_emotions(
                args.calibration_root, args.limit
            )
        elif dataset_id == "wesad_official":
            dataset_summary["wesad_official"] = download_wesad_official(
                args.calibration_root
            )
        else:
            raise SystemExit(f"unknown dataset id: {dataset_id}")

    summary = {
        "models": model_summary,
        "datasets": dataset_summary,
    }
    text = json.dumps(summary, indent=2, sort_keys=True)
    print(text)
    if args.summary:
        args.summary.parent.mkdir(parents=True, exist_ok=True)
        args.summary.write_text(text + "\n")

    return 1 if model_summary["failed"] else 0


if __name__ == "__main__":
    sys.exit(main())
