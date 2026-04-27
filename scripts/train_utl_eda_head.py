#!/usr/bin/env python3
"""Train the E18 WESAD stress head from the official raw WESAD archive.

This script uses the real WESAD subject pickle files inside WESAD.zip. It does
not create fallback data. Missing archives, missing signal keys, empty windows,
or non-finite inputs abort with a concrete error.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pickle
import random
import sys
import zipfile
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import torch
from torch import nn
from torch.utils.data import DataLoader, TensorDataset


LABELS = {
    1: "baseline",
    2: "stress",
    3: "amusement",
    4: "meditation",
}
LABEL_TO_INDEX = {label: idx for idx, label in enumerate(sorted(LABELS))}


class WesadEdaCnn(nn.Module):
    def __init__(self, channels: int, classes: int) -> None:
        super().__init__()
        self.net = nn.Sequential(
            nn.Conv1d(channels, 24, kernel_size=7, padding=3),
            nn.BatchNorm1d(24),
            nn.ReLU(),
            nn.MaxPool1d(2),
            nn.Conv1d(24, 48, kernel_size=5, padding=2),
            nn.BatchNorm1d(48),
            nn.ReLU(),
            nn.MaxPool1d(2),
            nn.Conv1d(48, 64, kernel_size=3, padding=1),
            nn.BatchNorm1d(64),
            nn.ReLU(),
            nn.AdaptiveAvgPool1d(1),
        )
        self.head = nn.Linear(64, classes)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.head(self.net(x).squeeze(-1))


@dataclass(frozen=True)
class WindowSet:
    x: np.ndarray
    y: np.ndarray
    subjects: list[str]
    counts: Counter[int]


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


def load_subject(archive: zipfile.ZipFile, member: str) -> dict[str, Any]:
    with archive.open(member) as handle:
        return pickle.load(handle, encoding="latin1")


def majority_label(labels: np.ndarray) -> int | None:
    target = labels[np.isin(labels, list(LABELS))]
    if target.size == 0:
        return None
    counts = np.bincount(target.astype(np.int64), minlength=max(LABELS) + 1)
    return int(counts.argmax())


def build_windows(
    zip_path: Path,
    window_seconds: int,
    stride_seconds: int,
    max_windows_per_subject: int | None,
) -> WindowSet:
    if not zip_path.exists():
        raise FileNotFoundError(f"missing WESAD archive: {zip_path}")
    if window_seconds <= 0 or stride_seconds <= 0:
        raise ValueError("window_seconds and stride_seconds must be positive")

    window = window_seconds * 4
    stride = stride_seconds * 4
    if window < 16:
        raise ValueError("window is too small for 1D CNN pooling")

    xs: list[np.ndarray] = []
    ys: list[int] = []
    subjects: list[str] = []
    counts: Counter[int] = Counter()

    with zipfile.ZipFile(zip_path) as archive:
        for member in subject_members(zip_path):
            subject = Path(member).stem
            data = load_subject(archive, member)
            wrist = data["signal"]["wrist"]
            labels = np.asarray(data["label"]).reshape(-1)
            eda = np.asarray(wrist["EDA"], dtype=np.float32).reshape(-1)
            temp = np.asarray(wrist["TEMP"], dtype=np.float32).reshape(-1)
            bvp = np.asarray(wrist["BVP"], dtype=np.float32).reshape(-1)

            if eda.size == 0 or temp.size == 0 or bvp.size == 0 or labels.size == 0:
                raise RuntimeError(f"{subject} has an empty required signal")
            if not np.isfinite(eda).all() or not np.isfinite(temp).all() or not np.isfinite(bvp).all():
                raise RuntimeError(f"{subject} contains non-finite wrist signal values")

            bvp_trimmed = bvp[: (bvp.size // 16) * 16].reshape(-1, 16).mean(axis=1)
            signal_len = min(eda.size, temp.size, bvp_trimmed.size)
            if signal_len < window:
                raise RuntimeError(f"{subject} has only {signal_len} aligned 4 Hz samples")

            subject_added = 0
            for start in range(0, signal_len - window + 1, stride):
                end = start + window
                label_start = int(round(start * 175.0))
                label_end = int(round(end * 175.0))
                label = majority_label(labels[label_start:label_end])
                if label is None:
                    continue
                if max_windows_per_subject is not None and subject_added >= max_windows_per_subject:
                    break

                stacked = np.stack(
                    [eda[start:end], temp[start:end], bvp_trimmed[start:end]], axis=0
                )
                xs.append(stacked)
                ys.append(LABEL_TO_INDEX[label])
                subjects.append(subject)
                counts[label] += 1
                subject_added += 1

    if not xs:
        raise RuntimeError("no labeled WESAD windows were produced")
    x = np.stack(xs).astype(np.float32)
    y = np.asarray(ys, dtype=np.int64)
    if not np.isfinite(x).all():
        raise RuntimeError("produced WESAD windows contain non-finite values")
    return WindowSet(x=x, y=y, subjects=subjects, counts=counts)


def split_by_subject(windows: WindowSet, holdout_subjects: set[str]) -> tuple[np.ndarray, np.ndarray]:
    train = np.asarray([subject not in holdout_subjects for subject in windows.subjects])
    val = ~train
    if train.sum() == 0 or val.sum() == 0:
        raise RuntimeError("subject split produced empty train or validation set")
    return train, val


def normalize(train_x: np.ndarray, all_x: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    mean = train_x.mean(axis=(0, 2), keepdims=True)
    std = train_x.std(axis=(0, 2), keepdims=True)
    if np.any(std < 1e-6):
        raise RuntimeError(f"near-zero WESAD signal std: {std.reshape(-1).tolist()}")
    return ((all_x - mean) / std).astype(np.float32), mean.reshape(-1), std.reshape(-1)


def accuracy_and_confusion(model: nn.Module, loader: DataLoader, device: torch.device) -> tuple[float, list[list[int]]]:
    model.eval()
    correct = 0
    total = 0
    confusion = np.zeros((len(LABELS), len(LABELS)), dtype=np.int64)
    with torch.no_grad():
        for xb, yb in loader:
            xb = xb.to(device)
            yb = yb.to(device)
            pred = model(xb).argmax(dim=1)
            correct += int((pred == yb).sum().item())
            total += int(yb.numel())
            for truth, guess in zip(yb.cpu().numpy(), pred.cpu().numpy(), strict=True):
                confusion[int(truth), int(guess)] += 1
    if total == 0:
        raise RuntimeError("empty validation loader")
    return correct / total, confusion.tolist()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--wesad-zip",
        type=Path,
        default=Path("data/utl_calibration/wesad_official/WESAD.zip"),
    )
    parser.add_argument("--models-root", type=Path, default=Path("models"))
    parser.add_argument("--window-seconds", type=int, default=60)
    parser.add_argument("--stride-seconds", type=int, default=30)
    parser.add_argument("--max-windows-per-subject", type=int, default=220)
    parser.add_argument("--epochs", type=int, default=10)
    parser.add_argument("--batch-size", type=int, default=128)
    parser.add_argument("--seed", type=int, default=17)
    args = parser.parse_args()

    random.seed(args.seed)
    np.random.seed(args.seed)
    torch.manual_seed(args.seed)

    out_dir = args.models_root / "eda"
    checkpoint_path = out_dir / "wesad_cnn.pt"
    config_path = out_dir / "config.json"
    metrics_path = out_dir / "training_metrics.json"
    print(
        json.dumps(
            {
                "source_of_truth": "models/eda/wesad_cnn.pt + models/eda/config.json",
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

    windows = build_windows(
        args.wesad_zip,
        args.window_seconds,
        args.stride_seconds,
        args.max_windows_per_subject,
    )
    unique_subjects = sorted(set(windows.subjects), key=lambda value: int(value[1:]))
    holdout_subjects = set(unique_subjects[-2:])
    train_mask, val_mask = split_by_subject(windows, holdout_subjects)
    x_norm, mean, std = normalize(windows.x[train_mask], windows.x)
    y = windows.y

    train_ds = TensorDataset(torch.from_numpy(x_norm[train_mask]), torch.from_numpy(y[train_mask]))
    val_ds = TensorDataset(torch.from_numpy(x_norm[val_mask]), torch.from_numpy(y[val_mask]))
    train_loader = DataLoader(train_ds, batch_size=args.batch_size, shuffle=True)
    val_loader = DataLoader(val_ds, batch_size=args.batch_size)

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    model = WesadEdaCnn(channels=x_norm.shape[1], classes=len(LABELS)).to(device)

    class_counts = np.bincount(y[train_mask], minlength=len(LABELS)).astype(np.float32)
    if np.any(class_counts == 0):
        raise RuntimeError(f"missing class in training split: {class_counts.tolist()}")
    weights = torch.tensor(class_counts.sum() / class_counts, dtype=torch.float32, device=device)
    criterion = nn.CrossEntropyLoss(weight=weights)
    optimizer = torch.optim.AdamW(model.parameters(), lr=2e-3, weight_decay=1e-3)

    history: list[dict[str, float]] = []
    for epoch in range(1, args.epochs + 1):
        model.train()
        total_loss = 0.0
        total = 0
        for xb, yb in train_loader:
            xb = xb.to(device)
            yb = yb.to(device)
            optimizer.zero_grad(set_to_none=True)
            loss = criterion(model(xb), yb)
            loss.backward()
            optimizer.step()
            total_loss += float(loss.item()) * int(yb.numel())
            total += int(yb.numel())
        val_acc, _ = accuracy_and_confusion(model, val_loader, device)
        history.append(
            {
                "epoch": float(epoch),
                "train_loss": total_loss / total,
                "validation_accuracy": val_acc,
            }
        )
        print(
            json.dumps(
                {
                    "epoch": epoch,
                    "train_loss": total_loss / total,
                    "validation_accuracy": val_acc,
                },
                sort_keys=True,
            )
        )

    val_acc, confusion = accuracy_and_confusion(model, val_loader, device)
    out_dir.mkdir(parents=True, exist_ok=True)
    checkpoint = {
        "architecture": "WesadEdaCnn",
        "state_dict": model.cpu().state_dict(),
        "input_channels": ["wrist_eda_4hz", "wrist_temp_4hz", "wrist_bvp_mean_4hz"],
        "window_seconds": args.window_seconds,
        "sample_rate_hz": 4,
        "labels": {str(LABEL_TO_INDEX[key]): value for key, value in LABELS.items()},
        "normalization_mean": mean.tolist(),
        "normalization_std": std.tolist(),
    }
    torch.save(checkpoint, checkpoint_path)

    metrics = {
        "history": history,
        "validation_accuracy": val_acc,
        "validation_confusion": confusion,
        "train_count": int(train_mask.sum()),
        "validation_count": int(val_mask.sum()),
        "train_subjects": [subject for subject in unique_subjects if subject not in holdout_subjects],
        "validation_subjects": sorted(holdout_subjects, key=lambda value: int(value[1:])),
        "class_window_counts": {LABELS[label]: int(count) for label, count in sorted(windows.counts.items())},
    }
    metrics_path.write_text(json.dumps(metrics, indent=2, sort_keys=True) + "\n")

    config = {
        "model": "WESAD EDA/physiology 1D-CNN stress head",
        "source": "https://ubi29.informatik.uni-siegen.de/usi/data_wesad.html",
        "source_archive": str(args.wesad_zip),
        "source_archive_sha256": sha256_file(args.wesad_zip),
        "checkpoint": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "metrics": str(metrics_path),
        "metrics_sha256": sha256_file(metrics_path),
        "window_seconds": args.window_seconds,
        "stride_seconds": args.stride_seconds,
        "sample_rate_hz": 4,
        "input_channels": checkpoint["input_channels"],
        "labels": checkpoint["labels"],
        "normalization_mean": checkpoint["normalization_mean"],
        "normalization_std": checkpoint["normalization_std"],
        "validation_accuracy": val_acc,
        "validation_confusion": confusion,
        "train_count": metrics["train_count"],
        "validation_count": metrics["validation_count"],
        "train_subjects": metrics["train_subjects"],
        "validation_subjects": metrics["validation_subjects"],
        "class_window_counts": metrics["class_window_counts"],
    }
    config_path.write_text(json.dumps(config, indent=2, sort_keys=True) + "\n")

    print(
        json.dumps(
            {
                "source_of_truth": "models/eda/wesad_cnn.pt + models/eda/config.json",
                "after": {
                    "checkpoint": file_state(checkpoint_path),
                    "config": file_state(config_path),
                    "metrics": file_state(metrics_path),
                },
                "validation_accuracy": val_acc,
                "validation_confusion": confusion,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # noqa: BLE001 - fail loudly with an exact cause
        print(f"E18 WESAD training failed: {exc!r}", file=sys.stderr)
        raise
