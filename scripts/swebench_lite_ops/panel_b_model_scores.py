from __future__ import annotations

import hashlib
import json
import math
import os
import platform
import socket
import sys
from contextlib import contextmanager
from dataclasses import dataclass
from importlib import metadata
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file, utc_now
from .panel_b_artifacts import DEFAULT_PANEL_B_MODELS_ROOT, PANEL_B_SOURCES, validate_models_root
from .panel_b_score_rows import prediction_id_hex


DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_LABEL_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split"
)
DEFAULT_PANEL_B_MODEL_SCORES_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-111d-panel-b-model-scores-fsv"
)
DEFAULT_PANEL_B_MODEL_SCORES_OUT = DEFAULT_PANEL_B_MODEL_SCORES_ROOT / "panel_b_model_scores.jsonl"
REQUIRED_CALIBRATION_ROWS = 240
REQUIRED_HOLDOUT_ROWS = 240
TARGET_SIDE_ID_MARKERS = (
    "oracle",
    "docker",
    "test_outcome",
    "target_side",
    "ground_truth",
)


@dataclass(frozen=True)
class GateRow:
    row_index: int
    split: str
    task_id: str
    repo: str
    category: str
    patch_path: Path
    patch_sha256: str
    actual_oracle_pass: bool
    score_a: float
    prediction_id_hex: str
    text: str


@dataclass(frozen=True)
class LabelBundle:
    accepted_label_ids: tuple[str, ...]
    failure_evidence_set_ids: tuple[str, ...]
    accepted_source: str
    failure_source: str


@dataclass(frozen=True)
class ModelArtifact:
    slot: str
    source_id: str
    revision: str
    scorer_kind: str
    root: Path
    files: tuple[dict[str, Any], ...]


def add_arguments(parser: Any) -> None:
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--models-root", type=Path, default=DEFAULT_PANEL_B_MODELS_ROOT)
    parser.add_argument("--label-root", type=Path, default=DEFAULT_LABEL_ROOT)
    parser.add_argument("--out-jsonl", type=Path, default=DEFAULT_PANEL_B_MODEL_SCORES_OUT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_PANEL_B_MODEL_SCORES_ROOT)
    parser.add_argument("--expected-calibration-count", type=int, default=REQUIRED_CALIBRATION_ROWS)
    parser.add_argument("--expected-holdout-count", type=int, default=REQUIRED_HOLDOUT_ROWS)
    parser.add_argument("--device", default="auto", choices=["auto", "cpu", "cuda"])
    parser.add_argument("--batch-size", type=int, default=8)
    parser.add_argument("--max-length", type=int, default=256)
    parser.add_argument("--knn-k", type=int, default=15)
    parser.add_argument("--temperature", type=float, default=0.15)
    parser.add_argument(
        "--fusion-policy",
        default="mean",
        choices=["mean", "calibration-category-signed-correlation"],
        help="Panel B late-fusion policy. Non-mean policies are fit on calibration rows only.",
    )
    parser.add_argument(
        "--fusion-minority-prior-threshold",
        type=int,
        default=5,
        help="Use calibration prior when a category has fewer than this many rows in either class.",
    )
    parser.add_argument("--github-issue", type=int, default=572)
    parser.add_argument(
        "--allow-network",
        action="store_true",
        help="Debug escape hatch only. By default, scoring forbids all network sockets.",
    )


def command_panel_b_model_scores(args: Any) -> dict[str, Any]:
    ship_gate_report = args.ship_gate_report.expanduser().resolve(strict=False)
    models_root = args.models_root.expanduser().resolve(strict=False)
    label_root = args.label_root.expanduser().resolve(strict=False)
    out_jsonl = args.out_jsonl.expanduser().resolve(strict=False)
    validate_not_mounted_drive("ship_gate_report", ship_gate_report)
    validate_not_mounted_drive("models_root", models_root)
    validate_not_mounted_drive("label_root", label_root)
    validate_not_mounted_drive("out_jsonl", out_jsonl)
    validate_models_root(models_root)
    validate_positive_int("expected_calibration_count", args.expected_calibration_count)
    validate_positive_int("expected_holdout_count", args.expected_holdout_count)
    validate_positive_int("batch_size", args.batch_size)
    validate_positive_int("max_length", args.max_length)
    validate_positive_int("knn_k", args.knn_k)
    validate_positive_int("fusion_minority_prior_threshold", args.fusion_minority_prior_threshold)
    if not math.isfinite(float(args.temperature)) or args.temperature <= 0.0:
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_TEMPERATURE_INVALID",
            "--temperature must be a positive finite number",
            "Pass a positive calibration temperature for the calibration-only KNN scorer.",
            {"temperature": args.temperature},
        )

    if not args.allow_network:
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["TRANSFORMERS_OFFLINE"] = "1"
        os.environ["HF_DATASETS_OFFLINE"] = "1"

    artifacts, artifact_set_sha256 = load_artifact_set(models_root)
    calibration_rows, holdout_rows = load_gate_rows(
        ship_gate_report,
        args.expected_calibration_count,
        args.expected_holdout_count,
    )
    label_index, label_report = load_label_index(label_root)

    package_versions: dict[str, str | None] = {}
    with network_guard(enabled=not args.allow_network):
        torch, transformers = import_model_runtime()
        package_versions = model_package_versions(torch, transformers)
        device = resolve_device(args.device, torch)
        calibration_by_model: dict[str, Any] = {}
        holdout_by_model: dict[str, Any] = {}
        model_reports: list[dict[str, Any]] = []
        for artifact in artifacts:
            tokenizer, model = load_local_model(transformers, artifact.root)
            model.to(device)
            model.eval()
            calibration_embeddings = encode_texts(
                torch,
                tokenizer,
                model,
                device,
                [row.text for row in calibration_rows],
                batch_size=args.batch_size,
                max_length=args.max_length,
            )
            holdout_embeddings = encode_texts(
                torch,
                tokenizer,
                model,
                device,
                [row.text for row in holdout_rows],
                batch_size=args.batch_size,
                max_length=args.max_length,
            )
            calibration_by_model[artifact.slot] = calibration_embeddings
            holdout_by_model[artifact.slot] = holdout_embeddings
            model_reports.append(
                {
                    "slot": artifact.slot,
                    "sourceId": artifact.source_id,
                    "root": str(artifact.root),
                    "device": device,
                    "embeddingDim": int(calibration_embeddings.shape[1]),
                    "calibrationRows": int(calibration_embeddings.shape[0]),
                    "holdoutRows": int(holdout_embeddings.shape[0]),
                    "scorerKind": artifact.scorer_kind,
                }
            )
            del model
            if device.startswith("cuda"):
                torch.cuda.empty_cache()

    calibration_labels = [1 if row.actual_oracle_pass else 0 for row in calibration_rows]
    calibration_categories = [row.category for row in calibration_rows]
    calibration_model_scores = {
        slot: [
            score_one_calibration_row(
                embeddings,
                row_index,
                calibration_labels,
                calibration_categories,
                row.category,
                k=args.knn_k,
                temperature=args.temperature,
            )
            for row_index, row in enumerate(calibration_rows)
        ]
        for slot, embeddings in calibration_by_model.items()
    }
    fusion_policy = build_fusion_policy(
        args.fusion_policy,
        sorted(calibration_by_model),
        calibration_rows,
        calibration_labels,
        calibration_model_scores,
        minority_prior_threshold=args.fusion_minority_prior_threshold,
    )
    score_rows: list[dict[str, Any]] = []
    label_join_counts = {"exact": 0, "fallback": 0, "failure_exact": 0, "failure_fallback": 0}
    for row_index, holdout in enumerate(holdout_rows):
        model_scores: dict[str, float] = {}
        for slot in calibration_by_model:
            score = score_one_row(
                calibration_by_model[slot],
                holdout_by_model[slot][row_index],
                calibration_labels,
                calibration_categories,
                holdout.category,
                k=args.knn_k,
                temperature=args.temperature,
            )
            model_scores[slot] = score
        score_b, transformed_scores = fuse_model_scores(
            model_scores,
            holdout.category,
            fusion_policy,
        )
        labels = label_index.get((holdout.task_id, holdout.category))
        if labels is None:
            labels = fallback_label_bundle(holdout)
        if labels.accepted_source == "fallback":
            label_join_counts["fallback"] += 1
        else:
            label_join_counts["exact"] += 1
        if labels.failure_source == "fallback":
            label_join_counts["failure_fallback"] += 1
        else:
            label_join_counts["failure_exact"] += 1
        score_rows.append(
            {
                "taskId": holdout.task_id,
                "category": holdout.category,
                "patchPath": str(holdout.patch_path),
                "patchSha256": holdout.patch_sha256,
                "predictionIdHex": holdout.prediction_id_hex,
                "scoreB": clamp01(score_b),
                "modelBackedPanelB": True,
                "acceptedLabelIds": list(labels.accepted_label_ids),
                "failureEvidenceSetIds": list(labels.failure_evidence_set_ids),
                "panelBArtifactSetSha256": artifact_set_sha256,
                "panelBModelScores": model_scores,
                "panelBModelTransformedScores": transformed_scores,
                "panelBFusionPolicy": fusion_policy["categories"].get(
                    holdout.category,
                    fusion_policy["default"],
                ),
                "scoringPolicy": "per_model_cosine_knn_late_fusion_v1",
            }
        )

    write_jsonl_checked(out_jsonl, score_rows)
    holdout_labels = [1 if row.actual_oracle_pass else 0 for row in holdout_rows]
    holdout_scores = [float(row["scoreB"]) for row in score_rows]
    return {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_111d_panel_b_model_scores",
        "githubIssue": args.github_issue,
        "ok": True,
        "shipGateReport": str(ship_gate_report),
        "shipGateReportSha256": sha256_file(ship_gate_report),
        "modelsRoot": str(models_root),
        "labelRoot": str(label_root),
        "outJsonl": str(out_jsonl),
        "outputSha256": sha256_file(out_jsonl),
        "panelBArtifactSetSha256": artifact_set_sha256,
        "calibrationRows": len(calibration_rows),
        "holdoutRows": len(holdout_rows),
        "scoreRows": len(score_rows),
        "localFilesOnly": True,
        "networkForbidden": not args.allow_network,
        "downloadAttempted": False,
        "hfOfflineEnv": {
            "HF_HUB_OFFLINE": os.environ.get("HF_HUB_OFFLINE"),
            "TRANSFORMERS_OFFLINE": os.environ.get("TRANSFORMERS_OFFLINE"),
            "HF_DATASETS_OFFLINE": os.environ.get("HF_DATASETS_OFFLINE"),
        },
        "pathPolicy": {
            "mntDActiveReadsOrWrites": False,
            "modelRootsAiwonderOnly": True,
            "flatVectorConcatUsed": False,
            "perModelSlotIdentityPreserved": True,
        },
        "scoringPolicy": {
            "name": "per_model_cosine_knn_late_fusion_v1",
            "calibrationOnlyTraining": True,
            "holdoutOracleLabelsUsedForScore": False,
            "perModelEmbeddingsKeptSeparate": True,
            "lateFusion": args.fusion_policy,
            "knnK": args.knn_k,
            "temperature": args.temperature,
            "maxLength": args.max_length,
            "batchSize": args.batch_size,
            "fusionPolicy": args.fusion_policy,
            "fusionPolicyFitSplit": "calibration",
            "fusionMinorityPriorThreshold": args.fusion_minority_prior_threshold,
        },
        "fusionPolicy": fusion_policy,
        "calibrationDiagnostics": calibration_diagnostics(
            calibration_rows,
            calibration_labels,
            calibration_model_scores,
            fusion_policy,
        ),
        "modelRuntime": {
            "python": sys.version,
            "platform": platform.platform(),
            "device": args.device,
            "resolvedDevice": model_reports[0].get("device") if model_reports and "device" in model_reports[0] else None,
            "packageVersions": package_versions,
        },
        "models": model_reports,
        "artifacts": [artifact_to_json(artifact) for artifact in artifacts],
        "labels": {**label_report, "joinCounts": label_join_counts},
        "metrics": {
            "holdoutPearson": pearson_or_none(holdout_labels, holdout_scores),
            "holdoutBrier": brier(holdout_labels, holdout_scores),
            "holdoutAccuracyAtHalf": accuracy_at_half(holdout_labels, holdout_scores),
            "holdoutCategoryMetrics": category_metrics(holdout_rows, holdout_scores),
        },
        "contract": {
            "feeds": "panel-b-score-rows --panel-b-scores-jsonl",
            "noHuggingFaceRuntimeFetch": not args.allow_network,
            "oneTimeMaterializationCommand": "panel-b-artifacts --download",
        },
    }


def import_model_runtime() -> tuple[Any, Any]:
    try:
        import torch  # type: ignore
        import transformers  # type: ignore
    except ImportError as exc:
        raise SwebenchOpsError(
            "PANEL_B_MODEL_RUNTIME_MISSING",
            "Panel B model scoring requires torch and transformers in the aiwonder venv",
            "Install once on aiwonder with uv pip install --python /zfs/hot/contextgraph/venv/bin/python3 transformers tokenizers safetensors torch.",
            {"missing": str(exc)},
        ) from exc
    return torch, transformers


def model_package_versions(torch: Any, transformers: Any) -> dict[str, str | None]:
    names = ("torch", "transformers", "tokenizers", "safetensors", "huggingface_hub")
    versions = {name: package_version_or_none(name) for name in names}
    versions["torch_runtime"] = getattr(torch, "__version__", None)
    versions["transformers_runtime"] = getattr(transformers, "__version__", None)
    return versions


def package_version_or_none(name: str) -> str | None:
    try:
        return metadata.version(name)
    except metadata.PackageNotFoundError:
        return None


def resolve_device(requested: str, torch: Any) -> str:
    if requested == "cpu":
        return "cpu"
    if requested == "cuda":
        if not torch.cuda.is_available():
            raise SwebenchOpsError(
                "PANEL_B_MODEL_SCORES_CUDA_UNAVAILABLE",
                "--device cuda was requested but torch.cuda.is_available() is false",
                "Use --device cpu or fix the aiwonder CUDA torch runtime.",
                {},
            )
        return "cuda"
    return "cuda" if torch.cuda.is_available() else "cpu"


def load_local_model(transformers: Any, root: Path) -> tuple[Any, Any]:
    if not root.is_dir():
        raise SwebenchOpsError(
            "PANEL_B_MODEL_ROOT_MISSING",
            f"Panel B local model root is missing: {root}",
            "Run panel-b-artifacts --download on aiwonder before scoring.",
            {"root": str(root)},
        )
    try:
        tokenizer = transformers.AutoTokenizer.from_pretrained(
            str(root),
            local_files_only=True,
            trust_remote_code=False,
            use_fast=True,
        )
        model = transformers.AutoModel.from_pretrained(
            str(root),
            local_files_only=True,
            trust_remote_code=False,
        )
    except Exception as exc:
        raise SwebenchOpsError(
            "PANEL_B_MODEL_LOCAL_LOAD_FAILED",
            f"Panel B model failed to load from local files only: {root}",
            "Ensure the local directory contains config, tokenizer files, and a transformers-compatible model weight file.",
            {"root": str(root), "error": str(exc), "type": type(exc).__name__},
        ) from exc
    return tokenizer, model


def encode_texts(
    torch: Any,
    tokenizer: Any,
    model: Any,
    device: str,
    texts: list[str],
    *,
    batch_size: int,
    max_length: int,
) -> Any:
    chunks: list[Any] = []
    for start in range(0, len(texts), batch_size):
        batch = texts[start : start + batch_size]
        inputs = tokenizer(
            batch,
            padding=True,
            truncation=True,
            max_length=max_length,
            return_tensors="pt",
        )
        inputs = {key: value.to(device) for key, value in inputs.items()}
        with torch.no_grad():
            outputs = model(**inputs)
            hidden = outputs.last_hidden_state
            mask = inputs["attention_mask"].unsqueeze(-1).to(hidden.dtype)
            summed = (hidden * mask).sum(dim=1)
            counts = mask.sum(dim=1).clamp(min=1.0)
            embeddings = summed / counts
            embeddings = torch.nn.functional.normalize(embeddings, p=2, dim=1)
        chunks.append(embeddings.detach().cpu())
    return torch.cat(chunks, dim=0)


def score_one_row(
    calibration_embeddings: Any,
    holdout_embedding: Any,
    calibration_labels: list[int],
    calibration_categories: list[str],
    category: str,
    *,
    k: int,
    temperature: float,
) -> float:
    indices = [idx for idx, value in enumerate(calibration_categories) if value == category]
    if not indices:
        indices = list(range(len(calibration_labels)))
    sims = calibration_embeddings[indices].matmul(holdout_embedding)
    take = min(k, len(indices))
    top_values, top_positions = sims.topk(take)
    weights = softmax([float(value) / temperature for value in top_values.tolist()])
    labels = [calibration_labels[indices[int(pos)]] for pos in top_positions.tolist()]
    return clamp01(sum(weight * float(label) for weight, label in zip(weights, labels)))


def score_one_calibration_row(
    calibration_embeddings: Any,
    row_index: int,
    calibration_labels: list[int],
    calibration_categories: list[str],
    category: str,
    *,
    k: int,
    temperature: float,
) -> float:
    indices = [
        idx
        for idx, value in enumerate(calibration_categories)
        if value == category and idx != row_index
    ]
    if not indices:
        indices = [idx for idx in range(len(calibration_labels)) if idx != row_index]
    if not indices:
        return clamp01(float(calibration_labels[row_index]))
    sims = calibration_embeddings[indices].matmul(calibration_embeddings[row_index])
    take = min(k, len(indices))
    top_values, top_positions = sims.topk(take)
    weights = softmax([float(value) / temperature for value in top_values.tolist()])
    labels = [calibration_labels[indices[int(pos)]] for pos in top_positions.tolist()]
    return clamp01(sum(weight * float(label) for weight, label in zip(weights, labels)))


def build_fusion_policy(
    policy_name: str,
    slots: list[str],
    calibration_rows: list[GateRow],
    calibration_labels: list[int],
    calibration_model_scores: dict[str, list[float]],
    *,
    minority_prior_threshold: int,
) -> dict[str, Any]:
    default = equal_weight_policy(slots, reason="default_equal_weight")
    categories: dict[str, dict[str, Any]] = {}
    by_category: dict[str, list[int]] = {}
    for index, row in enumerate(calibration_rows):
        by_category.setdefault(row.category, []).append(index)

    for category, indices in sorted(by_category.items()):
        labels = [calibration_labels[index] for index in indices]
        if policy_name == "mean":
            categories[category] = equal_weight_policy(slots, reason="mean_policy")
            continue
        if labels and all(label == labels[0] for label in labels):
            categories[category] = {
                "kind": policy_name,
                "reason": "calibration_category_constant_oracle_prior",
                "constantPrior": clamp01(float(labels[0])),
                "slotWeights": {slot: 0.0 for slot in slots},
                "slotSigns": {slot: 1.0 for slot in slots},
                "calibrationRows": len(indices),
                "calibrationOracleMean": clamp01(sum(labels) / len(labels)),
                "slotCalibrationPearson": {slot: None for slot in slots},
            }
            continue
        positive_count = sum(labels)
        negative_count = len(labels) - positive_count
        if min(positive_count, negative_count) < minority_prior_threshold:
            categories[category] = {
                "kind": policy_name,
                "reason": "calibration_category_minority_under_threshold_prior",
                "constantPrior": clamp01(sum(labels) / len(labels)),
                "slotWeights": {slot: 0.0 for slot in slots},
                "slotSigns": {slot: 1.0 for slot in slots},
                "calibrationRows": len(indices),
                "calibrationOracleMean": clamp01(sum(labels) / len(labels)),
                "slotCalibrationPearson": {
                    slot: pearson_or_none(
                        labels,
                        [calibration_model_scores[slot][index] for index in indices],
                    )
                    for slot in slots
                },
                "classCounts": {
                    "pass": positive_count,
                    "fail": negative_count,
                    "minorityPriorThreshold": minority_prior_threshold,
                },
            }
            continue
        slot_stats: dict[str, float | None] = {}
        strengths: dict[str, float] = {}
        signs: dict[str, float] = {}
        for slot in slots:
            values = [calibration_model_scores[slot][index] for index in indices]
            corr = pearson_or_none(labels, values)
            slot_stats[slot] = corr
            if corr is None:
                strengths[slot] = 0.0
                signs[slot] = 1.0
            else:
                strengths[slot] = abs(corr)
                signs[slot] = -1.0 if corr < 0.0 else 1.0
        total_strength = sum(strengths.values())
        if total_strength <= 1.0e-12:
            categories[category] = equal_weight_policy(
                slots,
                reason="zero_calibration_correlation_fallback",
                policy_name=policy_name,
                calibration_rows=len(indices),
                calibration_oracle_mean=clamp01(sum(labels) / len(labels)),
                slot_calibration_pearson=slot_stats,
            )
            continue
        categories[category] = {
            "kind": policy_name,
            "reason": "signed_abs_calibration_pearson",
            "constantPrior": None,
            "slotWeights": {
                slot: strengths[slot] / total_strength
                for slot in slots
            },
            "slotSigns": signs,
            "calibrationRows": len(indices),
            "calibrationOracleMean": clamp01(sum(labels) / len(labels)),
            "slotCalibrationPearson": slot_stats,
        }
    return {
        "name": policy_name,
        "fitSplit": "calibration",
        "holdoutOracleLabelsUsedForPolicy": False,
        "minorityPriorThreshold": minority_prior_threshold,
        "default": default,
        "categories": categories,
    }


def equal_weight_policy(
    slots: list[str],
    *,
    reason: str,
    policy_name: str = "mean",
    calibration_rows: int | None = None,
    calibration_oracle_mean: float | None = None,
    slot_calibration_pearson: dict[str, float | None] | None = None,
) -> dict[str, Any]:
    weight = 1.0 / len(slots)
    return {
        "kind": policy_name,
        "reason": reason,
        "constantPrior": None,
        "slotWeights": {slot: weight for slot in slots},
        "slotSigns": {slot: 1.0 for slot in slots},
        "calibrationRows": calibration_rows,
        "calibrationOracleMean": calibration_oracle_mean,
        "slotCalibrationPearson": slot_calibration_pearson or {slot: None for slot in slots},
    }


def fuse_model_scores(
    model_scores: dict[str, float],
    category: str,
    fusion_policy: dict[str, Any],
) -> tuple[float, dict[str, float]]:
    policy = fusion_policy["categories"].get(category, fusion_policy["default"])
    constant_prior = policy.get("constantPrior")
    if constant_prior is not None:
        return clamp01(float(constant_prior)), {
            slot: clamp01(float(constant_prior))
            for slot in sorted(model_scores)
        }
    weights = policy["slotWeights"]
    signs = policy["slotSigns"]
    transformed: dict[str, float] = {}
    fused = 0.0
    for slot in sorted(model_scores):
        raw_score = float(model_scores[slot])
        signed_score = raw_score if float(signs.get(slot, 1.0)) >= 0.0 else 1.0 - raw_score
        signed_score = clamp01(signed_score)
        transformed[slot] = signed_score
        fused += float(weights.get(slot, 0.0)) * signed_score
    return clamp01(fused), transformed


def calibration_diagnostics(
    calibration_rows: list[GateRow],
    calibration_labels: list[int],
    calibration_model_scores: dict[str, list[float]],
    fusion_policy: dict[str, Any],
) -> dict[str, Any]:
    fused = [
        fuse_model_scores(
            {slot: calibration_model_scores[slot][index] for slot in calibration_model_scores},
            row.category,
            fusion_policy,
        )[0]
        for index, row in enumerate(calibration_rows)
    ]
    by_category: dict[str, dict[str, list[float] | list[int]]] = {}
    for index, row in enumerate(calibration_rows):
        bucket = by_category.setdefault(row.category, {"labels": [], "scores": []})
        bucket["labels"].append(calibration_labels[index])  # type: ignore[union-attr]
        bucket["scores"].append(fused[index])  # type: ignore[union-attr]
    return {
        "rows": len(calibration_rows),
        "pearson": pearson_or_none(calibration_labels, fused),
        "brier": brier(calibration_labels, fused),
        "accuracyAtHalf": accuracy_at_half(calibration_labels, fused),
        "categoryMetrics": {
            category: {
                "count": len(values["labels"]),
                "pearson": pearson_or_none(values["labels"], values["scores"]),  # type: ignore[arg-type]
                "brier": brier(values["labels"], values["scores"]),  # type: ignore[arg-type]
                "accuracyAtHalf": accuracy_at_half(values["labels"], values["scores"]),  # type: ignore[arg-type]
            }
            for category, values in sorted(by_category.items())
        },
    }


def softmax(values: list[float]) -> list[float]:
    if not values:
        return []
    max_value = max(values)
    exp_values = [math.exp(value - max_value) for value in values]
    total = sum(exp_values)
    if total <= 0.0 or not math.isfinite(total):
        return [1.0 / len(values)] * len(values)
    return [value / total for value in exp_values]


def load_gate_rows(
    path: Path,
    expected_calibration_count: int,
    expected_holdout_count: int,
) -> tuple[list[GateRow], list[GateRow]]:
    payload = read_json_object(path, "PANEL_B_MODEL_SCORES_SHIP_GATE_REPORT")
    root = require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    calibration = parse_gate_split(
        require_array(root.get("calibration_predictions"), "calibration_predictions"),
        "calibration",
        expected_calibration_count,
    )
    holdout = parse_gate_split(
        require_array(root.get("holdout_predictions"), "holdout_predictions"),
        "holdout",
        expected_holdout_count,
    )
    return calibration, holdout


def parse_gate_split(rows: list[Any], split: str, expected_count: int) -> list[GateRow]:
    if len(rows) != expected_count:
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_GATE_ROW_COUNT_MISMATCH",
            f"{split} row count does not match expected count",
            "Use the current aiwonder ship-gate FSV report or pass the correct expected count.",
            {"split": split, "expected": expected_count, "actual": len(rows)},
        )
    out: list[GateRow] = []
    seen: set[str] = set()
    for row_index, raw in enumerate(rows):
        value = require_object(raw, f"{split}[{row_index}]")
        task_id = required_string(value, "task_id")
        category = required_string(value, "category")
        patch_path = Path(required_string(value, "patch_path")).expanduser().resolve(strict=False)
        validate_not_mounted_drive("patch_path", patch_path)
        patch_sha256 = normalize_sha256(required_string(value, "patch_sha256"))
        text = patch_path.read_text(encoding="utf-8", errors="replace")
        actual_sha = sha256_file(patch_path).removeprefix("sha256:")
        if actual_sha != patch_sha256:
            raise SwebenchOpsError(
                "PANEL_B_MODEL_SCORES_PATCH_SHA_MISMATCH",
                "patch file SHA does not match the ship-gate report",
                "Regenerate the ship-gate report or restore the aiwonder corpus patch file.",
                {
                    "split": split,
                    "row_index": row_index,
                    "patch_path": str(patch_path),
                    "expected": patch_sha256,
                    "actual": actual_sha,
                },
            )
        prediction_id = prediction_id_hex(task_id, category, str(patch_path), f"sha256:{patch_sha256}")
        key = f"{task_id}\0{category}\0{patch_path}\0{patch_sha256}"
        if key in seen:
            raise SwebenchOpsError(
                "PANEL_B_MODEL_SCORES_DUPLICATE_GATE_ROW",
                f"{split} rows contain duplicate patch identity",
                "Fix the ship-gate report before scoring Panel B.",
                {"split": split, "row_index": row_index, "row_key": key},
            )
        seen.add(key)
        out.append(
            GateRow(
                row_index=row_index,
                split=split,
                task_id=task_id,
                repo=required_string(value, "repo"),
                category=category,
                patch_path=patch_path,
                patch_sha256=f"sha256:{patch_sha256}",
                actual_oracle_pass=required_bool(value, "actual_oracle_pass"),
                score_a=required_score(value, "predicted_oracle_pass"),
                prediction_id_hex=prediction_id,
                text=text,
            )
        )
    return out


def load_label_index(label_root: Path) -> tuple[dict[tuple[str, str], LabelBundle], dict[str, Any]]:
    validate_not_mounted_drive("label_root", label_root)
    if not label_root.is_dir():
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_LABEL_ROOT_MISSING",
            f"label root is missing: {label_root}",
            "Point --label-root at the aiwonder TASK-PY-G-117 auto-label artifact root.",
            {"label_root": str(label_root)},
        )
    code_state_path = label_root / "code_state_constellations.jsonl"
    failure_path = label_root / "failure_evidence_sets.jsonl"
    code_labels, code_report = load_code_state_labels(code_state_path)
    failure_labels, failure_report = load_failure_evidence_labels(failure_path)
    out: dict[tuple[str, str], LabelBundle] = {}
    for key, labels in code_labels.items():
        failure = failure_labels.get(key)
        out[key] = LabelBundle(
            accepted_label_ids=labels,
            failure_evidence_set_ids=failure or ("failure_evidence:none",),
            accepted_source="exact",
            failure_source="exact" if failure is not None else "fallback",
        )
    for key, labels in failure_labels.items():
        if key not in out:
            task_id, category = key
            out[key] = LabelBundle(
                accepted_label_ids=(f"code_state_identity:mutation_category:{category}",),
                failure_evidence_set_ids=labels,
                accepted_source="fallback",
                failure_source="exact",
            )
    return out, {
        "labelRoot": str(label_root),
        "indexedCodeStateRows": code_report["rows"],
        "indexedFailureEvidenceRows": failure_report["rows"],
        "codeStateSha256": code_report["sha256"],
        "failureEvidenceSha256": failure_report["sha256"],
        "indexedKeys": len(out),
    }


def load_code_state_labels(path: Path) -> tuple[dict[tuple[str, str], tuple[str, ...]], dict[str, Any]]:
    rows = 0
    out: dict[tuple[str, str], tuple[str, ...]] = {}
    for value in iter_jsonl_objects(path, "PANEL_B_MODEL_SCORES_CODE_STATE_LABELS"):
        rows += 1
        task_id = optional_string(value, "task_instance_id", "task_id")
        category = optional_string(value, "mutation_category")
        if not task_id or not category:
            continue
        labels = sanitize_ids(value.get("live_predictor_labels") or value.get("labels") or [])
        labels = tuple(list(labels)[:9] or [f"code_state_identity:mutation_category:{category}"])
        key = (task_id, category)
        if key not in out:
            out[key] = labels
    return out, {"rows": rows, "sha256": sha256_file(path)}


def load_failure_evidence_labels(path: Path) -> tuple[dict[tuple[str, str], tuple[str, ...]], dict[str, Any]]:
    rows = 0
    out: dict[tuple[str, str], tuple[str, ...]] = {}
    for value in iter_jsonl_objects(path, "PANEL_B_MODEL_SCORES_FAILURE_EVIDENCE_LABELS"):
        rows += 1
        task_id = optional_string(value, "task_instance_id", "task_id")
        category = optional_string(value, "mutation_category")
        if not task_id or not category:
            continue
        labels = sanitize_ids(value.get("live_predictor_labels") or value.get("labels") or [])
        labels = tuple(list(labels)[:9] or ["failure_evidence:localized_candidate"])
        key = (task_id, category)
        if key not in out:
            out[key] = labels
    return out, {"rows": rows, "sha256": sha256_file(path)}


def fallback_label_bundle(row: GateRow) -> LabelBundle:
    return LabelBundle(
        accepted_label_ids=(f"code_state_identity:mutation_category:{row.category}",),
        failure_evidence_set_ids=("failure_evidence:none",),
        accepted_source="fallback",
        failure_source="fallback",
    )


def sanitize_ids(values: Any) -> tuple[str, ...]:
    if not isinstance(values, list):
        return ()
    out: list[str] = []
    seen: set[str] = set()
    for item in values:
        if not isinstance(item, str):
            continue
        item = item.strip()
        if not item or item in seen:
            continue
        lowered = item.lower()
        if any(marker in lowered for marker in TARGET_SIDE_ID_MARKERS):
            continue
        seen.add(item)
        out.append(item)
    return tuple(out)


def load_artifact_set(models_root: Path) -> tuple[list[ModelArtifact], str]:
    artifacts: list[ModelArtifact] = []
    digest_payload: list[dict[str, Any]] = []
    for source in PANEL_B_SOURCES:
        root = source.root(models_root).resolve(strict=False)
        validate_not_mounted_drive("model_root", root)
        files: list[dict[str, Any]] = []
        for filename in source.files:
            path = root / filename
            if not path.is_file() or path.stat().st_size <= 0:
                raise SwebenchOpsError(
                    "PANEL_B_MODEL_SCORES_ARTIFACT_MISSING",
                    f"required Panel B artifact is missing or empty: {path}",
                    "Run panel-b-artifacts --download --reuse-existing on aiwonder before scoring.",
                    {"slot": source.slot, "source_id": source.source_id, "path": str(path)},
                )
            files.append(
                {
                    "name": filename,
                    "path": str(path),
                    "bytes": path.stat().st_size,
                    "sha256": sha256_file(path),
                }
            )
        artifact = ModelArtifact(
            slot=source.slot,
            source_id=source.source_id,
            revision=source.revision,
            scorer_kind=source.scorer_kind,
            root=root,
            files=tuple(files),
        )
        artifacts.append(artifact)
        digest_payload.append(artifact_to_json(artifact))
    digest = hashlib.sha256(
        json.dumps(digest_payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    return artifacts, digest


def artifact_to_json(artifact: ModelArtifact) -> dict[str, Any]:
    return {
        "slot": artifact.slot,
        "sourceId": artifact.source_id,
        "revision": artifact.revision,
        "scorerKind": artifact.scorer_kind,
        "root": str(artifact.root),
        "files": list(artifact.files),
    }


@contextmanager
def network_guard(*, enabled: bool) -> Iterable[None]:
    if not enabled:
        yield
        return
    original_socket = socket.socket
    original_create_connection = socket.create_connection

    class OfflineSocket(original_socket):  # type: ignore[misc, valid-type]
        def connect(self, address: Any) -> None:  # type: ignore[override]
            raise RuntimeError(f"network access forbidden during Panel B local-only scoring: {address!r}")

        def connect_ex(self, address: Any) -> int:  # type: ignore[override]
            raise RuntimeError(f"network access forbidden during Panel B local-only scoring: {address!r}")

    def fail_create_connection(*args: Any, **kwargs: Any) -> Any:
        raise RuntimeError("network access forbidden during Panel B local-only scoring")

    socket.socket = OfflineSocket  # type: ignore[assignment]
    socket.create_connection = fail_create_connection  # type: ignore[assignment]
    try:
        yield
    finally:
        socket.socket = original_socket  # type: ignore[assignment]
        socket.create_connection = original_create_connection  # type: ignore[assignment]


def read_json_object(path: Path, code: str) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            f"{code}_MISSING",
            f"required JSON file missing: {path}",
            "Use the current aiwonder FSV artifact.",
            {"path": str(path)},
        )
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            f"{code}_BAD_JSON",
            f"required JSON file is not valid JSON: {path}",
            "Regenerate the artifact before consuming it.",
            {"path": str(path), "error": str(exc)},
        ) from exc
    return require_object(value, str(path))


def iter_jsonl_objects(path: Path, code: str) -> Iterable[dict[str, Any]]:
    if not path.is_file():
        raise SwebenchOpsError(
            f"{code}_MISSING",
            f"required JSONL file missing: {path}",
            "Use the current aiwonder label artifact root.",
            {"path": str(path)},
        )
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            stripped = line.strip()
            if not stripped:
                continue
            try:
                value = json.loads(stripped)
            except json.JSONDecodeError as exc:
                raise SwebenchOpsError(
                    f"{code}_BAD_JSONL",
                    f"JSONL line {line_number} is invalid in {path}",
                    "Regenerate the label artifact before consuming it.",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            yield require_object(value, f"{path}:{line_number}")


def require_object(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_OBJECT_REQUIRED",
            f"{name} must be a JSON object",
            "Regenerate the artifact before consuming it.",
            {"name": name, "type": type(value).__name__},
        )
    return value


def require_array(value: Any, name: str) -> list[Any]:
    if not isinstance(value, list):
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_ARRAY_REQUIRED",
            f"{name} must be a JSON array",
            "Regenerate the artifact before consuming it.",
            {"name": name, "type": type(value).__name__},
        )
    return value


def required_string(value: dict[str, Any], key: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_STRING_FIELD_INVALID",
            f"required string field missing or empty: {key}",
            "Use the current ship-gate report.",
            {"key": key, "value": raw},
        )
    return raw.strip()


def optional_string(value: dict[str, Any], *keys: str) -> str | None:
    for key in keys:
        raw = value.get(key)
        if isinstance(raw, str) and raw.strip():
            return raw.strip()
    return None


def required_bool(value: dict[str, Any], key: str) -> bool:
    raw = value.get(key)
    if not isinstance(raw, bool):
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_BOOL_FIELD_INVALID",
            f"required bool field missing or invalid: {key}",
            "Use the current ship-gate report.",
            {"key": key, "value": raw},
        )
    return raw


def required_score(value: dict[str, Any], key: str) -> float:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, (int, float)):
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_SCORE_FIELD_INVALID",
            f"required score field missing or invalid: {key}",
            "Use the current ship-gate report.",
            {"key": key, "value": raw},
        )
    score = float(raw)
    if not math.isfinite(score) or score < 0.0 or score > 1.0:
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_SCORE_OUT_OF_RANGE",
            f"score field must be finite and in [0, 1]: {key}",
            "Fix the score source before consuming it.",
            {"key": key, "value": raw},
        )
    return score


def normalize_sha256(value: str) -> str:
    raw = value.strip().removeprefix("sha256:").lower()
    if len(raw) != 64 or any(char not in "0123456789abcdef" for char in raw):
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_SHA256_INVALID",
            "expected a sha256 digest",
            "Use the current ship-gate report with patch SHA256 fields.",
            {"value": value},
        )
    return raw


def validate_not_mounted_drive(field: str, path: Path) -> None:
    if str(path).startswith("/mnt/d/"):
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_MNT_D_FORBIDDEN",
            f"{field} cannot point at mounted D drive: {path}",
            "Use aiwonder /zfs roots.",
            {"field": field, "path": str(path)},
        )


def validate_positive_int(name: str, value: int) -> None:
    if value <= 0:
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_POSITIVE_INT_REQUIRED",
            f"--{name.replace('_', '-')} must be positive",
            "Pass a positive integer.",
            {"name": name, "value": value},
        )


def write_jsonl_checked(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_name(f"{path.name}.part-{os.getpid()}")
    with temp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    temp.replace(path)
    readback = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if readback != rows:
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_JSONL_READBACK_MISMATCH",
            f"JSONL readback mismatch at {path}",
            "Check filesystem integrity and concurrent writers before using this score store.",
            {"path": str(path)},
        )


def clamp01(value: float) -> float:
    if not math.isfinite(value):
        raise SwebenchOpsError(
            "PANEL_B_MODEL_SCORES_NONFINITE_SCORE",
            "Panel B score is not finite",
            "Fix the model scoring computation before emitting score rows.",
            {"value": value},
        )
    return min(1.0, max(0.0, value))


def pearson_or_none(labels: list[int], probabilities: list[float]) -> float | None:
    if len(labels) < 3:
        return None
    x_mean = sum(float(value) for value in labels) / len(labels)
    y_mean = sum(float(value) for value in probabilities) / len(probabilities)
    x_var = sum((float(value) - x_mean) ** 2 for value in labels)
    y_var = sum((float(value) - y_mean) ** 2 for value in probabilities)
    if x_var <= 1.0e-12 or y_var <= 1.0e-12:
        return None
    cov = sum((float(x) - x_mean) * (float(y) - y_mean) for x, y in zip(labels, probabilities))
    return cov / math.sqrt(x_var * y_var)


def brier(labels: list[int], probabilities: list[float]) -> float:
    return sum((float(prob) - float(label)) ** 2 for label, prob in zip(labels, probabilities)) / len(labels)


def accuracy_at_half(labels: list[int], probabilities: list[float]) -> float:
    hits = sum(1 for label, prob in zip(labels, probabilities) if (1 if prob >= 0.5 else 0) == label)
    return hits / len(labels)


def category_metrics(rows: list[GateRow], scores: list[float]) -> dict[str, dict[str, Any]]:
    by_category: dict[str, tuple[list[int], list[float]]] = {}
    for row, score in zip(rows, scores):
        labels, probs = by_category.setdefault(row.category, ([], []))
        labels.append(1 if row.actual_oracle_pass else 0)
        probs.append(score)
    return {
        category: {
            "count": len(labels),
            "pearson": pearson_or_none(labels, probs),
            "brier": brier(labels, probs),
            "accuracyAtHalf": accuracy_at_half(labels, probs),
        }
        for category, (labels, probs) in sorted(by_category.items())
    }
