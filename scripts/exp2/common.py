"""Shared strict utilities for the EXP-2 harness."""

from __future__ import annotations

import hashlib
import json
import logging
import os
import random
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np
import yaml


REPO_ROOT = Path(__file__).resolve().parents[2]
EMBEDDER_KEYS = [
    "e1_semantic",
    "e2_temporal_recent",
    "e3_temporal_periodic",
    "e4_temporal_positional",
    "e5_causal_cause",
    "e6_sparse_top_k",
    "e7_code",
    "e8_graph_source",
    "e9_hdc",
    "e10_paraphrase",
    "e11_entity",
    "e12_token_pooled",
    "e13_splade_top_k",
    "e14_bge_m3_dense",
]
EMBEDDER_LABELS = [
    "E1 (e5-large-v2 sem)",
    "E2 (sin-PE recent)",
    "E3 (Fourier periodic)",
    "E4 (positional)",
    "E5 (nomic-causal cause)",
    "E6 (SPLADE coCondenser)",
    "E7 (Qodo code)",
    "E8 (graph source)",
    "E9 (HDC)",
    "E10 (e5-base paraphrase)",
    "E11 (KEPLER entity)",
    "E12 (ColBERT pooled)",
    "E13 (SPLADE-PP)",
    "E14 (BGE-M3 dense)",
]
EXPECTED_DIMS = {
    "e1_semantic": 1024,
    "e2_temporal_recent": 512,
    "e3_temporal_periodic": 512,
    "e4_temporal_positional": 512,
    "e5_causal_cause": 768,
    "e6_sparse_top_k": 128,
    "e7_code": 1536,
    "e8_graph_source": 1024,
    "e9_hdc": 1024,
    "e10_paraphrase": 768,
    "e11_entity": 768,
    "e12_token_pooled": 128,
    "e13_splade_top_k": 128,
    "e14_bge_m3_dense": 1024,
}


class Exp2Error(RuntimeError):
    """Raised when EXP-2 cannot proceed without hiding a failure."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def repo_path(path: str | Path) -> Path:
    p = Path(path)
    return p if p.is_absolute() else REPO_ROOT / p


def load_config(path: str | Path) -> dict[str, Any]:
    cfg_path = repo_path(path)
    if not cfg_path.exists():
        raise Exp2Error(f"Config file missing: {cfg_path}")
    with cfg_path.open("r", encoding="utf-8") as fh:
        cfg = yaml.safe_load(fh)
    if not isinstance(cfg, dict):
        raise Exp2Error(f"Config must parse to an object: {cfg_path}")
    return cfg


def output_dir(cfg: dict[str, Any]) -> Path:
    out = repo_path(cfg["output_dir"])
    out.mkdir(parents=True, exist_ok=True)
    return out


def canonical_json(data: Any) -> bytes:
    return json.dumps(data, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: str | Path) -> str:
    h = hashlib.sha256()
    with repo_path(path).open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def write_json(path: str | Path, data: Any) -> None:
    dst = repo_path(path)
    dst.parent.mkdir(parents=True, exist_ok=True)
    tmp = dst.with_suffix(dst.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        json.dump(data, fh, indent=2, sort_keys=True, ensure_ascii=False)
        fh.write("\n")
    os.replace(tmp, dst)


def read_json(path: str | Path) -> Any:
    src = repo_path(path)
    if not src.exists():
        raise Exp2Error(f"JSON file missing: {src}")
    with src.open("r", encoding="utf-8") as fh:
        return json.load(fh)


def setup_logging(out: Path, name: str = "exp2") -> logging.Logger:
    out.mkdir(parents=True, exist_ok=True)
    logger = logging.getLogger(name)
    logger.handlers.clear()
    logger.setLevel(logging.INFO)
    stream = logging.StreamHandler(sys.stdout)
    stream.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(message)s"))
    file_handler = logging.FileHandler(out / "run.log.jsonl", encoding="utf-8")

    class JsonFormatter(logging.Formatter):
        def format(self, record: logging.LogRecord) -> str:
            payload = {
                "timestamp": utc_now(),
                "level": record.levelname,
                "logger": record.name,
                "message": record.getMessage(),
            }
            if record.exc_info:
                payload["exception"] = self.formatException(record.exc_info)
            return json.dumps(payload, sort_keys=True)

    file_handler.setFormatter(JsonFormatter())
    logger.addHandler(stream)
    logger.addHandler(file_handler)
    return logger


def set_global_seed(seed: int) -> None:
    random.seed(seed)
    np.random.seed(seed)
    try:
        import torch

        torch.manual_seed(seed)
        if torch.cuda.is_available():
            torch.cuda.manual_seed_all(seed)
        torch.backends.cudnn.deterministic = True
        torch.backends.cudnn.benchmark = False
        torch.use_deterministic_algorithms(True, warn_only=True)
    except Exception as exc:  # pragma: no cover - torch may be absent in env checks
        raise Exp2Error(f"Failed to set torch deterministic controls: {exc}") from exc


def l2_normalize(x: np.ndarray, label: str) -> np.ndarray:
    if not np.isfinite(x).all():
        raise Exp2Error(f"{label}: non-finite values before L2 normalization")
    norms = np.linalg.norm(x, axis=1, keepdims=True)
    if np.any(norms <= 0):
        bad = np.where(norms.squeeze() <= 0)[0][:10].tolist()
        raise Exp2Error(f"{label}: zero-norm rows at indices {bad}")
    return (x / norms).astype(np.float32)


def require_keys(obj: dict[str, Any], keys: list[str], label: str) -> None:
    missing = [k for k in keys if k not in obj]
    if missing:
        raise Exp2Error(f"{label} missing required keys: {missing}")


def token_count(text: str) -> int:
    return len(text.split())


def text_hash(text: str) -> str:
    return sha256_bytes(text.strip().encode("utf-8"))
