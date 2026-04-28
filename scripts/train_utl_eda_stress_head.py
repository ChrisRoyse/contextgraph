#!/usr/bin/env python3
"""Train the improved E19 WESAD stress-floor head from raw WESAD physiology.

The output is a local, loadable PyTorch checkpoint containing an exported
logistic stress classifier, scaler statistics, and the exact feature schema.
It uses real official WESAD subject pickle files from WESAD.zip and fails
loudly on missing, empty, malformed, or non-finite data.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pickle
import sys
import zipfile
from collections import Counter
from pathlib import Path
from typing import Any

import numpy as np
import torch
from scipy.signal import find_peaks
from sklearn.linear_model import LogisticRegression
from sklearn.metrics import (
    accuracy_score,
    balanced_accuracy_score,
    confusion_matrix,
    precision_recall_fscore_support,
)
from sklearn.pipeline import make_pipeline
from sklearn.preprocessing import StandardScaler


LABELS = {
    1: "baseline",
    2: "stress",
    3: "amusement",
    4: "meditation",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False, "bytes": None, "sha256": None}
    return {
        "path": str(path),
        "exists": True,
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def subject_members(zip_path: Path) -> list[str]:
    with zipfile.ZipFile(zip_path) as archive:
        members = [
            name
            for name in archive.namelist()
            if name.startswith("WESAD/S") and name.endswith(".pkl")
        ]
    if not members:
        raise RuntimeError(f"no WESAD subject pickle files found in {zip_path}")
    return sorted(members, key=lambda name: int(Path(name).stem[1:]))


def majority_label(labels: np.ndarray) -> int | None:
    target = labels[np.isin(labels, list(LABELS))]
    if target.size == 0:
        return None
    counts = np.bincount(target.astype(np.int64), minlength=max(LABELS) + 1)
    return int(counts.argmax())


def signal_features(values: np.ndarray, prefix: str) -> tuple[list[str], list[float]]:
    x = np.asarray(values, dtype=np.float32).reshape(-1)
    if x.size < 2:
        raise ValueError(f"{prefix} signal window is too short")
    if not np.isfinite(x).all():
        raise ValueError(f"{prefix} signal window contains non-finite values")
    diff = np.diff(x)
    index = np.arange(x.size, dtype=np.float32)
    slope = float(np.polyfit(index, x, 1)[0])
    names = [
        "mean",
        "std",
        "median",
        "min",
        "max",
        "p05",
        "p25",
        "p75",
        "p95",
        "range",
        "slope",
        "mean_abs_diff",
        "diff_std",
        "rms",
    ]
    values_out = [
        float(x.mean()),
        float(x.std()),
        float(np.median(x)),
        float(x.min()),
        float(x.max()),
        float(np.percentile(x, 5)),
        float(np.percentile(x, 25)),
        float(np.percentile(x, 75)),
        float(np.percentile(x, 95)),
        float(x.max() - x.min()),
        slope,
        float(np.abs(diff).mean()),
        float(diff.std()),
        float(np.sqrt(np.mean(x * x))),
    ]
    return [f"{prefix}_{name}" for name in names], values_out


def ecg_features(ecg: np.ndarray, sample_rate_hz: int) -> tuple[list[str], list[float]]:
    x = np.asarray(ecg, dtype=np.float32).reshape(-1)
    if x.size < sample_rate_hz * 5:
        raise ValueError("ECG window is too short for peak features")
    z = (x - x.mean()) / (x.std() + 1e-6)
    peaks, _ = find_peaks(z, distance=int(0.35 * sample_rate_hz), prominence=0.6)
    rr = np.diff(peaks) / sample_rate_hz if peaks.size >= 2 else np.asarray([], dtype=np.float32)
    if rr.size:
        hr = 60.0 / rr
        peak_values = [
            float(len(peaks) * 60.0 / (len(x) / sample_rate_hz)),
            float(rr.mean()),
            float(rr.std()),
            float(np.sqrt(np.mean(np.diff(rr) ** 2))) if rr.size > 1 else 0.0,
            float(np.percentile(hr, 10)),
            float(np.percentile(hr, 90)),
        ]
    else:
        peak_values = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    peak_names = [
        "ecg_peak_rate_bpm",
        "ecg_rr_mean",
        "ecg_rr_std",
        "ecg_rmssd",
        "ecg_hr_p10",
        "ecg_hr_p90",
    ]
    signal_names, signal_values = signal_features(x, "ecg")
    return peak_names + signal_names[:8], peak_values + signal_values[:8]


def respiration_features(resp: np.ndarray, sample_rate_hz: int) -> tuple[list[str], list[float]]:
    x = np.asarray(resp, dtype=np.float32).reshape(-1)
    z = (x - x.mean()) / (x.std() + 1e-6)
    peaks, _ = find_peaks(z, distance=int(1.0 * sample_rate_hz), prominence=0.4)
    rate = float(len(peaks) * 60.0 / (len(x) / sample_rate_hz))
    names, values = signal_features(x, "resp")
    return ["resp_peak_rate_bpm"] + names, [rate] + values


def extract_chest_feature_matrix(
    zip_path: Path,
    window_seconds: int,
    stride_seconds: int,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, list[str], dict[str, int]]:
    if not zip_path.exists():
        raise FileNotFoundError(f"missing WESAD archive: {zip_path}")
    if window_seconds <= 0 or stride_seconds <= 0:
        raise ValueError("window_seconds and stride_seconds must be positive")

    sample_rate_hz = 700
    window = window_seconds * sample_rate_hz
    stride = stride_seconds * sample_rate_hz
    features: list[list[float]] = []
    labels: list[int] = []
    subjects: list[str] = []
    label_counts: Counter[str] = Counter()
    feature_names: list[str] | None = None

    with zipfile.ZipFile(zip_path) as archive:
        for member in subject_members(zip_path):
            subject = Path(member).stem
            with archive.open(member) as handle:
                data = pickle.load(handle, encoding="latin1")
            chest = data["signal"]["chest"]
            raw_labels = np.asarray(data["label"]).reshape(-1)
            required = ["ECG", "EDA", "Resp", "Temp"]
            for key in required:
                if key not in chest:
                    raise KeyError(f"{subject} is missing chest signal {key}")
                if np.asarray(chest[key]).size == 0:
                    raise RuntimeError(f"{subject} chest signal {key} is empty")

            for start in range(0, raw_labels.size - window + 1, stride):
                end = start + window
                label = majority_label(raw_labels[start:end])
                if label is None:
                    continue
                eda = np.asarray(chest["EDA"][start:end, 0], dtype=np.float32)
                temp = np.asarray(chest["Temp"][start:end, 0], dtype=np.float32)
                resp = np.asarray(chest["Resp"][start:end, 0], dtype=np.float32)
                ecg = np.asarray(chest["ECG"][start:end, 0], dtype=np.float32)

                names: list[str] = []
                row: list[float] = []
                for signal_name, values in (("eda", eda), ("temp", temp)):
                    signal_names, signal_values = signal_features(values, signal_name)
                    names.extend(signal_names)
                    row.extend(signal_values)
                resp_names, resp_values = respiration_features(resp, sample_rate_hz)
                ecg_names, ecg_values = ecg_features(ecg, sample_rate_hz)
                names.extend(resp_names)
                names.extend(ecg_names)
                row.extend(resp_values)
                row.extend(ecg_values)

                if feature_names is None:
                    feature_names = names
                elif feature_names != names:
                    raise RuntimeError("feature schema changed while extracting WESAD")

                features.append(row)
                labels.append(1 if label == 2 else 0)
                subjects.append(subject)
                label_counts[LABELS[label]] += 1

    if not features:
        raise RuntimeError("no labeled WESAD feature windows were produced")
    x = np.nan_to_num(np.asarray(features, dtype=np.float32), nan=0.0, posinf=0.0, neginf=0.0)
    y = np.asarray(labels, dtype=np.int64)
    subject_array = np.asarray(subjects, dtype=str)
    if feature_names is None:
        raise RuntimeError("feature schema was not created")
    if not np.isfinite(x).all():
        raise RuntimeError("WESAD feature matrix contains non-finite values")
    return x, y, subject_array, feature_names, dict(label_counts)


def evaluate(y_true: np.ndarray, y_pred: np.ndarray, proba: np.ndarray) -> dict[str, Any]:
    precision, recall, f1, _ = precision_recall_fscore_support(
        y_true, y_pred, labels=[0, 1], zero_division=0
    )
    return {
        "accuracy": float(accuracy_score(y_true, y_pred)),
        "balanced_accuracy": float(balanced_accuracy_score(y_true, y_pred)),
        "confusion": confusion_matrix(y_true, y_pred, labels=[0, 1]).tolist(),
        "precision": precision.tolist(),
        "recall": recall.tolist(),
        "f1": f1.tolist(),
        "proba_minmax": [float(proba.min()), float(proba.max())],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--wesad-zip",
        type=Path,
        default=Path("data/utl_calibration/wesad_official/WESAD.zip"),
    )
    parser.add_argument("--models-root", type=Path, default=Path("models"))
    parser.add_argument("--window-seconds", type=int, default=60)
    parser.add_argument("--stride-seconds", type=int, default=10)
    parser.add_argument("--test-subjects", default="S16,S17")
    parser.add_argument("--seed", type=int, default=17)
    args = parser.parse_args()

    np.random.seed(args.seed)
    out_dir = args.models_root / "eda"
    checkpoint_path = out_dir / "wesad_stress_head.pt"
    config_path = out_dir / "config.json"
    metrics_path = out_dir / "training_metrics.json"

    print(
        json.dumps(
            {
                "source_of_truth": "models/eda/wesad_stress_head.pt + models/eda/config.json",
                "before": {
                    "checkpoint": file_state(checkpoint_path),
                    "config": file_state(config_path),
                    "metrics": file_state(metrics_path),
                },
            },
            indent=2,
            sort_keys=True,
        )
    )

    subject_list = [
        Path(member).stem for member in subject_members(args.wesad_zip)
    ]
    test_subjects = {
        value.strip() for value in args.test_subjects.split(",") if value.strip()
    }
    unknown = sorted(test_subjects - set(subject_list))
    if unknown:
        raise ValueError(f"unknown WESAD test subjects: {unknown}")
    if not test_subjects:
        raise ValueError("at least one WESAD test subject is required")
    if test_subjects == set(subject_list):
        raise ValueError("test subjects cannot include every WESAD subject")

    x, y, subjects, feature_names, label_counts = extract_chest_feature_matrix(
        args.wesad_zip,
        args.window_seconds,
        args.stride_seconds,
    )
    extracted_subjects = sorted(set(subjects.tolist()), key=lambda value: int(value[1:]))
    if extracted_subjects != subject_list:
        raise RuntimeError(
            f"extracted subject list mismatch: expected {subject_list}, got {extracted_subjects}"
        )
    train_mask = ~np.isin(subjects, sorted(test_subjects))
    test_mask = np.isin(subjects, sorted(test_subjects))
    if int(train_mask.sum()) == 0 or int(test_mask.sum()) == 0:
        raise RuntimeError("empty train or test split")

    pipeline = make_pipeline(
        StandardScaler(),
        LogisticRegression(class_weight="balanced", max_iter=2000, random_state=args.seed),
    )
    pipeline.fit(x[train_mask], y[train_mask])
    proba = pipeline.predict_proba(x[test_mask])[:, 1]
    pred = (proba >= 0.5).astype(np.int64)
    metrics = {
        "model": "WESAD chest physiology logistic stress-floor head",
        "source_archive": str(args.wesad_zip),
        "source_archive_sha256": sha256_file(args.wesad_zip),
        "window_seconds": args.window_seconds,
        "stride_seconds": args.stride_seconds,
        "sample_rate_hz": 700,
        "feature_count": len(feature_names),
        "feature_names": feature_names,
        "target": "stress_vs_non_stress",
        "decision_threshold": 0.5,
        "label_mapping": {"0": "non_stress", "1": "stress"},
        "train_subjects": [
            subject for subject in subject_list if subject not in test_subjects
        ],
        "test_subjects": sorted(test_subjects, key=lambda value: int(value[1:])),
        "train_count": int(train_mask.sum()),
        "test_count": int(test_mask.sum()),
        "train_class_counts": np.bincount(y[train_mask], minlength=2).astype(int).tolist(),
        "test_class_counts": np.bincount(y[test_mask], minlength=2).astype(int).tolist(),
        "raw_wesad_label_counts": label_counts,
        "test_metrics": evaluate(y[test_mask], pred, proba),
    }

    scaler: StandardScaler = pipeline.named_steps["standardscaler"]
    classifier: LogisticRegression = pipeline.named_steps["logisticregression"]
    checkpoint = {
        "architecture": "WesadChestFeatureLogisticStressHead",
        "target": "stress_vs_non_stress",
        "decision_threshold": 0.5,
        "label_mapping": metrics["label_mapping"],
        "feature_names": feature_names,
        "scaler_mean": scaler.mean_.astype(np.float32).tolist(),
        "scaler_scale": scaler.scale_.astype(np.float32).tolist(),
        "coefficient": classifier.coef_.astype(np.float32).tolist(),
        "intercept": classifier.intercept_.astype(np.float32).tolist(),
        "window_seconds": args.window_seconds,
        "stride_seconds": args.stride_seconds,
        "sample_rate_hz": 700,
        "test_metrics": metrics["test_metrics"],
    }

    out_dir.mkdir(parents=True, exist_ok=True)
    torch.save(checkpoint, checkpoint_path)
    metrics_path.write_text(json.dumps(metrics, indent=2, sort_keys=True) + "\n")

    config = {
        "model": metrics["model"],
        "source": "https://ubi29.informatik.uni-siegen.de/usi/data_wesad.html",
        "source_archive": str(args.wesad_zip),
        "source_archive_sha256": metrics["source_archive_sha256"],
        "checkpoint": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "metrics": str(metrics_path),
        "metrics_sha256": sha256_file(metrics_path),
        "window_seconds": args.window_seconds,
        "stride_seconds": args.stride_seconds,
        "sample_rate_hz": 700,
        "feature_count": len(feature_names),
        "target": "stress_vs_non_stress",
        "decision_threshold": 0.5,
        "label_mapping": metrics["label_mapping"],
        "train_count": metrics["train_count"],
        "test_count": metrics["test_count"],
        "train_subjects": metrics["train_subjects"],
        "test_subjects": metrics["test_subjects"],
        "train_class_counts": metrics["train_class_counts"],
        "test_class_counts": metrics["test_class_counts"],
        "test_metrics": metrics["test_metrics"],
    }
    config_path.write_text(json.dumps(config, indent=2, sort_keys=True) + "\n")

    print(
        json.dumps(
            {
                "source_of_truth": "models/eda/wesad_stress_head.pt + models/eda/config.json",
                "after": {
                    "checkpoint": file_state(checkpoint_path),
                    "config": file_state(config_path),
                    "metrics": file_state(metrics_path),
                },
                "test_metrics": metrics["test_metrics"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # noqa: BLE001
        print(f"E19 WESAD stress-head training failed: {exc!r}", file=sys.stderr)
        raise
