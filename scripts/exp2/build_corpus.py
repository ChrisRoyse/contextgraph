#!/usr/bin/env python3
"""T-002..T-005 corpus construction and signing."""

from __future__ import annotations

import argparse
import json
import random
import re
import subprocess
from pathlib import Path
import sys
from typing import Any, Iterable

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import (
    EMBEDDER_LABELS,
    Exp2Error,
    canonical_json,
    load_config,
    output_dir,
    read_json,
    repo_path,
    setup_logging,
    sha256_bytes,
    text_hash,
    token_count,
    utc_now,
    write_json,
)


STRATA = [
    "S1_shakespeare",
    "S2_code",
    "S3_causal_claims",
    "S4_learning_events",
    "S5_beir_retrieval",
    "S6_wikipedia",
    "S7_temporal_sequence",
    "S8_multilingual",
]
MIN_CANDIDATES = 120
SAMPLE_PER_STRATUM = 100


def clean_text(text: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    text = re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f]", " ", text)
    return re.sub(r"\s+", " ", text).strip()


def chunks(text: str, min_words: int = 80, max_words: int = 600) -> Iterable[str]:
    words = clean_text(text).split()
    for i in range(0, len(words), max_words):
        part = " ".join(words[i : i + max_words])
        if min_words <= len(part.split()) <= max_words:
            yield part


def record(text: str, stratum: str, source: str, license_: str, metadata: dict[str, Any]) -> dict[str, Any]:
    cleaned = clean_text(text)
    n = token_count(cleaned)
    if n < 50 or n > 800:
        raise Exp2Error(f"{stratum}: candidate from {source} has {n} tokens, outside 50..800")
    return {
        "text": cleaned,
        "stratum": stratum,
        "metadata": {"source": source, "tokens": n, **metadata},
        "license": license_,
        "text_sha256": text_hash(cleaned),
    }


def collect_shakespeare() -> list[dict[str, Any]]:
    path = repo_path("shakespear/complete_works.txt")
    if not path.exists():
        raise Exp2Error(f"S1 source missing: {path}")
    return [
        record(text, "S1_shakespeare", str(path), "public_domain", {"chunk_index": i})
        for i, text in enumerate(chunks(path.read_text(encoding="utf-8", errors="strict")))
    ]


def collect_code() -> list[dict[str, Any]]:
    exts = {".py", ".rs", ".ts", ".js", ".sql", ".toml", ".yaml", ".yml"}
    roots = [repo_path("crates"), repo_path("scripts"), repo_path("config")]
    files = [p for root in roots for p in root.rglob("*") if p.is_file() and p.suffix in exts]
    out: list[dict[str, Any]] = []
    for path in sorted(files):
        if "__pycache__" in path.parts or "target" in path.parts:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="strict")
        except UnicodeDecodeError:
            continue
        license_ = "Apache-2.0"
        for i, chunk in enumerate(chunks(text, 80, 600)):
            out.append(
                record(
                    chunk,
                    "S2_code",
                    str(path.relative_to(repo_path("."))),
                    license_,
                    {"language": path.suffix.lstrip("."), "chunk_index": i},
                )
            )
    return out


def collect_causal_claims() -> list[dict[str, Any]]:
    markers = re.compile(r"\b(because|therefore|causes?|leads to|results in|trigger[s]?|due to)\b", re.I)
    roots = [repo_path("docs"), repo_path("tasks"), repo_path("crates")]
    out: list[dict[str, Any]] = []
    for root in roots:
        for path in sorted(root.rglob("*")):
            if not path.is_file() or path.suffix.lower() not in {".md", ".rs", ".py"}:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            sentences = re.split(r"(?<=[.!?])\s+", clean_text(text))
            window: list[str] = []
            for sent in sentences:
                if markers.search(sent):
                    window.append(sent)
                    joined = " ".join(window[-6:])
                    if 50 <= token_count(joined) <= 800:
                        out.append(
                            record(
                                joined,
                                "S3_causal_claims",
                                str(path.relative_to(repo_path("."))),
                                "Apache-2.0/internal_docs",
                                {"marker": markers.search(sent).group(0)},
                            )
                        )
    return out


def collect_learning_events(path: Path | None) -> list[dict[str, Any]]:
    if path is None:
        raise Exp2Error("S4 requires --learning-events-json exported from real CF_LEARNING_EVENTS")
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, list):
        raise Exp2Error("S4 learning events JSON must be a list")
    out = []
    for i, item in enumerate(data):
        text = item.get("text") or item.get("query") or item.get("content")
        label = item.get("outcome", {}).get("label") if isinstance(item.get("outcome"), dict) else item.get("label")
        if not isinstance(text, str) or not label:
            raise Exp2Error(f"S4 event {i} missing text/query/content or outcome.label")
        out.append(record(text, "S4_learning_events", str(path), "private_anonymized", {"event_index": i, "label": label}))
    return out


def collect_beir() -> list[dict[str, Any]]:
    try:
        from datasets import load_dataset
    except Exception as exc:
        raise Exp2Error(f"S5 requires datasets package: {exc}") from exc
    out = []
    ds = load_dataset("BeIR/scifact", "corpus", split="corpus", streaming=True)
    for i, row in enumerate(ds):
        text = row.get("text") or row.get("title", "")
        if text:
            cleaned = clean_text(text)
            if 50 <= token_count(cleaned) <= 800:
                out.append(record(cleaned, "S5_beir_retrieval", "BeIR/scifact/corpus", "CC-BY-SA/MIT metadata", {"row": i, "kind": "document"}))
        if len(out) >= MIN_CANDIDATES:
            break
    if len(out) < MIN_CANDIDATES:
        raise Exp2Error(f"S5 collected {len(out)} candidates, need {MIN_CANDIDATES}")
    return out


def collect_wikipedia() -> list[dict[str, Any]]:
    try:
        from datasets import load_dataset
    except Exception as exc:
        raise Exp2Error(f"S6 requires datasets package: {exc}") from exc
    out = []
    ds = load_dataset("wikimedia/wikipedia", "20231101.en", split="train", streaming=True)
    rng = random.Random(42)
    for i, row in enumerate(ds):
        if rng.random() > 0.015:
            continue
        for para in str(row.get("text", "")).split("\n"):
            cleaned = clean_text(para)
            if 100 <= token_count(cleaned) <= 400:
                out.append(record(cleaned, "S6_wikipedia", "wikimedia/wikipedia:20231101.en", "CC-BY-SA", {"row": i}))
                break
        if len(out) >= MIN_CANDIDATES:
            break
        if i > 25000 and len(out) < MIN_CANDIDATES:
            raise Exp2Error(f"S6 collected {len(out)} candidates after {i} rows, need {MIN_CANDIDATES}")
    return out


def collect_temporal() -> list[dict[str, Any]]:
    cmd = ["git", "log", "--date=iso-strict", "--pretty=format:%H%x09%ad%x09%s%n%b"]
    log = subprocess.check_output(cmd, cwd=repo_path("."), text=True, errors="replace")
    entries = [e for e in log.split("\n\n") if e.strip()]
    out = []
    window: list[str] = []
    for i, entry in enumerate(entries):
        window.append(f"Timeline event {i}: {entry}")
        candidate = " ".join(window[-8:])
        if 50 <= token_count(candidate) <= 800:
            out.append(record(candidate, "S7_temporal_sequence", "git_log", "repository_history", {"event_index": i, "window_size": len(window[-8:])}))
        if len(out) >= MIN_CANDIDATES:
            break
    if len(out) < MIN_CANDIDATES:
        raise Exp2Error(f"S7 collected {len(out)} git-log candidates, need {MIN_CANDIDATES}")
    return out


def collect_multilingual() -> list[dict[str, Any]]:
    try:
        from datasets import load_dataset
    except Exception as exc:
        raise Exp2Error(f"S8 requires datasets package: {exc}") from exc
    langs = [("eng_Latn", "English"), ("spa_Latn", "Spanish"), ("fra_Latn", "French"), ("deu_Latn", "German"), ("zho_Hans", "Chinese"), ("arb_Arab", "Arabic")]
    out = []
    for code, name in langs:
        ds = load_dataset("facebook/flores", code, split="dev", streaming=True)
        window: list[str] = []
        for i, row in enumerate(ds):
            text = clean_text(row.get("sentence", ""))
            if text:
                window.append(text)
                candidate = " ".join(window[-8:])
                if 50 <= token_count(candidate) <= 800:
                    out.append(record(candidate, "S8_multilingual", f"facebook/flores:{code}", "CC-BY-SA", {"language": name, "row": i, "window_size": len(window[-8:])}))
            if len([r for r in out if r["metadata"]["language"] == name]) >= 25:
                break
    if len(out) < MIN_CANDIDATES:
        raise Exp2Error(f"S8 collected {len(out)} candidates, need {MIN_CANDIDATES}")
    return out[:MIN_CANDIDATES]


def deduplicate(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    seen: set[str] = set()
    out = []
    for rec in records:
        h = rec["text_sha256"]
        if h in seen:
            continue
        seen.add(h)
        out.append(rec)
    return out


def build_sources(args: argparse.Namespace, out: Path) -> None:
    strata_dir = repo_path("scripts/exp2/strata")
    strata_dir.mkdir(parents=True, exist_ok=True)
    builders = {
        "S1_shakespeare": collect_shakespeare,
        "S2_code": collect_code,
        "S3_causal_claims": collect_causal_claims,
        "S4_learning_events": lambda: collect_learning_events(args.learning_events_json),
        "S5_beir_retrieval": collect_beir,
        "S6_wikipedia": collect_wikipedia,
        "S7_temporal_sequence": collect_temporal,
        "S8_multilingual": collect_multilingual,
    }
    global_hashes: set[str] = set()
    for name in STRATA:
        records = deduplicate(builders[name]())
        records = [r for r in records if r["text_sha256"] not in global_hashes]
        global_hashes.update(r["text_sha256"] for r in records)
        if len(records) < MIN_CANDIDATES:
            raise Exp2Error(f"{name}: collected {len(records)} candidates, need {MIN_CANDIDATES}")
        write_json(strata_dir / f"{name}.json", records)


def sample_sources(seed: int) -> None:
    strata_dir = repo_path("scripts/exp2/strata")
    for name in STRATA:
        records = read_json(strata_dir / f"{name}.json")
        rng = random.Random(seed)
        sampled = rng.sample(records, SAMPLE_PER_STRATUM)
        write_json(strata_dir / f"{name}_sampled.json", sampled)


def merge_manifest(cfg: dict[str, Any]) -> None:
    strata_dir = repo_path("scripts/exp2/strata")
    inputs = []
    seen: set[str] = set()
    for name in STRATA:
        records = read_json(strata_dir / f"{name}_sampled.json")
        if len(records) != SAMPLE_PER_STRATUM:
            raise Exp2Error(f"{name}_sampled has {len(records)} rows, expected 100")
        for rec in records:
            if rec["text_sha256"] in seen:
                raise Exp2Error(f"Duplicate text hash in sampled corpus: {rec['text_sha256']}")
            seen.add(rec["text_sha256"])
            inputs.append({"input_id": f"exp2_input_{len(inputs):05d}", **rec})
    manifest = {
        "corpus_id": "exp2_corpus_v1",
        "version": "1.0.0",
        "creation_timestamp": utc_now(),
        "creator_attribution": cfg["operator"],
        "total_inputs": len(inputs),
        "strata": {name: {"count": SAMPLE_PER_STRATUM, "selection_seed": 42} for name in STRATA},
        "inputs": inputs,
        "embedder_panel": [{"slot": f"E{i+1}", "name": label} for i, label in enumerate(EMBEDDER_LABELS)],
    }
    manifest["manifest_sha256"] = sha256_bytes(canonical_json({k: v for k, v in manifest.items() if k != "manifest_sha256"}))
    path = repo_path(cfg["corpus"]["manifest"])
    write_json(path, manifest)
    path.with_suffix(path.suffix + ".sha256").write_text(manifest["manifest_sha256"] + "\n", encoding="utf-8")


def build_pca_training(cfg: dict[str, Any], seed: int) -> None:
    strata_dir = repo_path("scripts/exp2/strata")
    audit_hashes = {r["text_sha256"] for name in STRATA for r in read_json(strata_dir / f"{name}_sampled.json")}
    pool = []
    for name in STRATA:
        candidates = [r for r in read_json(strata_dir / f"{name}.json") if r["text_sha256"] not in audit_hashes]
        if len(candidates) < 125:
            raise Exp2Error(f"{name}: only {len(candidates)} PCA candidates distinct from audit sample")
        pool.extend(candidates[:200])
    rng = random.Random(seed)
    selected = rng.sample(pool, cfg["pca"]["fit_corpus_count"])
    manifest = {
        "corpus_id": "exp2_pca_training_v1",
        "version": "1.0.0",
        "creation_timestamp": utc_now(),
        "selection_seed": seed,
        "total_inputs": len(selected),
        "inputs": [{"input_id": f"exp2_pca_{i:05d}", **rec} for i, rec in enumerate(selected)],
    }
    manifest["manifest_sha256"] = sha256_bytes(canonical_json({k: v for k, v in manifest.items() if k != "manifest_sha256"}))
    write_json(cfg["pca"]["fit_corpus"], manifest)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--phase", choices=["sources", "sample", "manifest", "pca", "all"], default="all")
    parser.add_argument("--learning-events-json", type=Path)
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    setup_logging(out)
    if args.phase in {"sources", "all"}:
        build_sources(args, out)
    if args.phase in {"sample", "all"}:
        sample_sources(seed=42)
    if args.phase in {"manifest", "all"}:
        merge_manifest(cfg)
    if args.phase in {"pca", "all"}:
        build_pca_training(cfg, seed=43)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 corpus build failed: {exc}") from exc
