#!/usr/bin/env python3
"""Embedder pretraining-corpus leakage audit for BLINDSPOT-008c / #732."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
import tomllib
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable


SCHEMA_VERSION = 1
TASK_ID = "TASK-732"
ARTIFACT_KIND = "embedder_pretraining_leakage_audit"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_MODELS_CONFIG = Path("/zfs/hot/contextgraph/models/mejepa_models_config.toml")
DEFAULT_TASKS_DIR = Path("tasks/swebench-lite")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-732-embedder-pretraining-leakage-audit-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/embedder-pretraining-audit")
ACTIVE_STATIC_SLOTS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
RETIRED_STATIC_AST_TEMPORAL_SLOTS = ("e2", "e3", "e4")
DETERMINISTIC_ACTIVE_SLOTS = {"e9"}
SHA_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


class PretrainingLeakageAuditError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def sha256_text(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(canonical_json(row) + "\n")
            count += 1
    tmp.replace(path)
    return count


def read_json(path: Path, code: str) -> Any:
    require(path.is_file(), code, {"path": str(path)})
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise PretrainingLeakageAuditError(f"{code}: invalid JSON at {path}: {exc}") from exc


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise PretrainingLeakageAuditError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "PRETRAINING_AUDIT_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


def parse_date_or_none(value: Any, *, field: str, model_id: str) -> str | None:
    if value is None:
        return None
    require(isinstance(value, str) and value, "PRETRAINING_AUDIT_INVALID_DATE", {"model_id": model_id, "field": field})
    try:
        datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise PretrainingLeakageAuditError(
            f"PRETRAINING_AUDIT_INVALID_DATE: {model_id} {field}={value!r}"
        ) from exc
    return value


def load_models_config(path: Path) -> dict[str, Any]:
    require(path.is_file(), "PRETRAINING_AUDIT_MODELS_CONFIG_MISSING", {"path": str(path)})
    try:
        with path.open("rb") as handle:
            data = tomllib.load(handle)
    except tomllib.TOMLDecodeError as exc:
        raise PretrainingLeakageAuditError(f"PRETRAINING_AUDIT_MODELS_CONFIG_INVALID: {path}: {exc}") from exc
    embedders = data.get("embedders")
    require(isinstance(embedders, dict), "PRETRAINING_AUDIT_MODELS_CONFIG_MISSING_EMBEDDERS", {"path": str(path)})
    return data


def active_embedder_rows(models_config: dict[str, Any]) -> list[dict[str, Any]]:
    embedders = models_config["embedders"]
    rows: list[dict[str, Any]] = []
    for slot in ACTIVE_STATIC_SLOTS:
        raw = embedders.get(slot)
        require(isinstance(raw, dict), "PRETRAINING_AUDIT_ACTIVE_SLOT_MISSING", {"slot": slot})
        kind = str(raw.get("kind") or "")
        repo = str(raw.get("repo") or "")
        deterministic = slot in DETERMINISTIC_ACTIVE_SLOTS or kind == "content_deterministic"
        if deterministic:
            require(not repo, "PRETRAINING_AUDIT_DETERMINISTIC_SLOT_HAS_REPO", {"slot": slot, "repo": repo})
        else:
            require(kind == "content_pretrained", "PRETRAINING_AUDIT_ACTIVE_SLOT_KIND_UNSUPPORTED", {"slot": slot, "kind": kind})
            require(repo, "PRETRAINING_AUDIT_ACTIVE_PRETRAINED_REPO_MISSING", {"slot": slot})
        rows.append(
            {
                "slot": slot,
                "kind": kind,
                "name": str(raw.get("name") or ""),
                "repo": repo,
                "dimension": raw.get("dimension"),
                "manifest_sha256": str(raw.get("manifest_sha256") or ""),
                "path": str(raw.get("path") or ""),
                "pretrained": not deterministic,
                "deterministic": deterministic,
            }
        )
    return rows


def load_task_repos(tasks_dir: Path, expected_task_count: int | None) -> dict[str, Any]:
    require(tasks_dir.is_dir(), "PRETRAINING_AUDIT_TASKS_DIR_MISSING", {"tasks_dir": str(tasks_dir)})
    paths = sorted(tasks_dir.glob("*.json"))
    require(paths, "PRETRAINING_AUDIT_TASKS_EMPTY", {"tasks_dir": str(tasks_dir)})
    if expected_task_count is not None:
        require(
            len(paths) == expected_task_count,
            "PRETRAINING_AUDIT_TASK_COUNT_MISMATCH",
            {"expected": expected_task_count, "actual": len(paths), "tasks_dir": str(tasks_dir)},
        )
    counter: Counter[str] = Counter()
    samples: dict[str, list[str]] = {}
    for path in paths:
        row = read_json(path, "PRETRAINING_AUDIT_TASK_JSON_INVALID")
        repo = str(row.get("repo") or "")
        instance_id = str(row.get("instance_id") or path.stem)
        require(repo, "PRETRAINING_AUDIT_TASK_REPO_MISSING", {"path": str(path)})
        counter[repo] += 1
        samples.setdefault(repo, [])
        if len(samples[repo]) < 5:
            samples[repo].append(instance_id)
    repos = [
        {"repo": repo, "task_count": count, "sample_instance_ids": samples[repo]}
        for repo, count in sorted(counter.items())
    ]
    return {"task_count": len(paths), "repo_count": len(repos), "repos": repos}


def load_repo_metadata(path: Path, required_repos: Iterable[str]) -> dict[str, dict[str, Any]]:
    doc = read_json(path, "PRETRAINING_AUDIT_REPO_METADATA_MISSING")
    rows = doc.get("repos") if isinstance(doc, dict) else doc
    require(isinstance(rows, list), "PRETRAINING_AUDIT_REPO_METADATA_NOT_LIST", {"path": str(path)})
    by_repo: dict[str, dict[str, Any]] = {}
    for row in rows:
        require(isinstance(row, dict), "PRETRAINING_AUDIT_REPO_METADATA_ROW_NOT_OBJECT", row)
        name = str(row.get("full_name") or row.get("repo") or "")
        require(name, "PRETRAINING_AUDIT_REPO_METADATA_NAME_MISSING", row)
        created_at = parse_date_or_none(row.get("created_at"), field="created_at", model_id=name)
        require(created_at is not None, "PRETRAINING_AUDIT_REPO_CREATED_AT_MISSING", {"repo": name})
        by_repo[name] = {
            "repo": name,
            "created_at": created_at,
            "html_url": str(row.get("html_url") or f"https://github.com/{name}"),
        }
    missing = sorted(set(required_repos) - set(by_repo))
    require(not missing, "PRETRAINING_AUDIT_REPO_METADATA_INCOMPLETE", {"missing_repos": missing})
    return by_repo


def validate_evidence_source(source: dict[str, Any], source_snapshot_dir: Path | None) -> dict[str, Any]:
    source_url = str(source.get("source_url") or "")
    source_kind = str(source.get("source_kind") or "")
    source_sha256 = str(source.get("source_sha256") or "")
    require(source_url.startswith("https://"), "PRETRAINING_AUDIT_SOURCE_URL_INVALID", source)
    require(source_kind, "PRETRAINING_AUDIT_SOURCE_KIND_MISSING", source)
    require(SHA_RE.match(source_sha256) is not None, "PRETRAINING_AUDIT_SOURCE_SHA_INVALID", source)
    snapshot = str(source.get("source_snapshot") or "")
    actual_snapshot_sha = None
    if source_snapshot_dir is not None:
        require(snapshot and Path(snapshot).name == snapshot, "PRETRAINING_AUDIT_SOURCE_SNAPSHOT_INVALID", source)
        snapshot_path = source_snapshot_dir / snapshot
        require(snapshot_path.is_file(), "PRETRAINING_AUDIT_SOURCE_SNAPSHOT_MISSING", {"snapshot": str(snapshot_path)})
        actual_snapshot_sha = sha256_file(snapshot_path)
        require(
            actual_snapshot_sha == source_sha256,
            "PRETRAINING_AUDIT_SOURCE_SNAPSHOT_SHA_MISMATCH",
            {"snapshot": str(snapshot_path), "expected": source_sha256, "actual": actual_snapshot_sha},
        )
    return {
        "source_url": source_url,
        "source_kind": source_kind,
        "source_sha256": source_sha256,
        "source_snapshot": snapshot or None,
        "source_snapshot_sha_verified": actual_snapshot_sha == source_sha256 if actual_snapshot_sha else False,
        "evidence_summary": str(source.get("evidence_summary") or ""),
        "supports": list(source.get("supports") or []),
    }


def load_model_evidence(path: Path, source_snapshot_dir: Path | None) -> dict[str, dict[str, Any]]:
    doc = read_json(path, "PRETRAINING_AUDIT_MODEL_EVIDENCE_MISSING")
    rows = doc.get("models") if isinstance(doc, dict) else doc
    require(isinstance(rows, list), "PRETRAINING_AUDIT_MODEL_EVIDENCE_NOT_LIST", {"path": str(path)})
    by_model: dict[str, dict[str, Any]] = {}
    for row in rows:
        require(isinstance(row, dict), "PRETRAINING_AUDIT_MODEL_EVIDENCE_ROW_NOT_OBJECT", row)
        model_id = str(row.get("model_id") or "")
        require(model_id, "PRETRAINING_AUDIT_MODEL_ID_MISSING", row)
        require(model_id not in by_model, "PRETRAINING_AUDIT_MODEL_EVIDENCE_DUPLICATE", {"model_id": model_id})
        cutoff_disclosed = bool(row.get("cutoff_disclosed"))
        cutoff_date = parse_date_or_none(row.get("pretraining_cutoff_date"), field="pretraining_cutoff_date", model_id=model_id)
        if cutoff_disclosed:
            require(cutoff_date is not None, "PRETRAINING_AUDIT_CUTOFF_DISCLOSED_WITHOUT_DATE", {"model_id": model_id})
        else:
            require(cutoff_date is None, "PRETRAINING_AUDIT_CUTOFF_DATE_WITHOUT_DISCLOSURE", {"model_id": model_id})
        release_date = parse_date_or_none(row.get("release_date"), field="release_date", model_id=model_id)
        sources = row.get("evidence_sources") or []
        require(isinstance(sources, list) and sources, "PRETRAINING_AUDIT_MODEL_SOURCES_EMPTY", {"model_id": model_id})
        by_model[model_id] = {
            "model_id": model_id,
            "pretraining_corpus": str(row.get("pretraining_corpus") or "undisclosed"),
            "pretraining_cutoff_date": cutoff_date,
            "cutoff_disclosed": cutoff_disclosed,
            "release_date": release_date,
            "training_data_disclosed": bool(row.get("training_data_disclosed")),
            "evidence_sources": [validate_evidence_source(source, source_snapshot_dir) for source in sources],
            "leakage_safe_claim_allowed": bool(row.get("leakage_safe_claim_allowed")),
            "notes": str(row.get("notes") or ""),
        }
    return by_model


def compare_created_to_cutoff(created_at: str, cutoff_date: str) -> int:
    created = datetime.fromisoformat(created_at.replace("Z", "+00:00"))
    cutoff = datetime.fromisoformat(cutoff_date.replace("Z", "+00:00"))
    return -1 if created < cutoff else (1 if created > cutoff else 0)


def classify_overlap(slot: dict[str, Any], repo: dict[str, Any], evidence: dict[str, Any] | None) -> dict[str, Any]:
    if slot["deterministic"]:
        return {
            "classification": "not_pretrained_no_corpus_leakage_surface",
            "leakage_safe_claim": True,
            "reason": "active deterministic slot has no pretrained corpus",
            "cutoff_disclosed": None,
            "pretraining_cutoff_date": None,
        }
    require(evidence is not None, "PRETRAINING_AUDIT_MISSING_MODEL_EVIDENCE", {"slot": slot["slot"], "model_id": slot["repo"]})
    if not evidence["cutoff_disclosed"]:
        return {
            "classification": "unknown_possible_overlap",
            "leakage_safe_claim": False,
            "reason": "model card/source evidence does not disclose a pretraining cutoff",
            "cutoff_disclosed": False,
            "pretraining_cutoff_date": None,
        }
    relation = compare_created_to_cutoff(repo["created_at"], evidence["pretraining_cutoff_date"])
    if relation <= 0:
        return {
            "classification": "repo_predates_cutoff_possible_overlap",
            "leakage_safe_claim": False,
            "reason": "repository existed before or at the disclosed pretraining cutoff",
            "cutoff_disclosed": True,
            "pretraining_cutoff_date": evidence["pretraining_cutoff_date"],
        }
    return {
        "classification": "repo_created_after_cutoff_unlikely_pretraining_overlap",
        "leakage_safe_claim": True,
        "reason": "repository was created after the disclosed pretraining cutoff",
        "cutoff_disclosed": True,
        "pretraining_cutoff_date": evidence["pretraining_cutoff_date"],
    }


def build_audit(
    *,
    models_config: dict[str, Any],
    task_repos: dict[str, Any],
    repo_metadata: dict[str, dict[str, Any]],
    model_evidence: dict[str, dict[str, Any]],
    now: str,
) -> dict[str, Any]:
    active = active_embedder_rows(models_config)
    required_models = sorted({row["repo"] for row in active if row["pretrained"]})
    missing_evidence = [model for model in required_models if model not in model_evidence]
    require(not missing_evidence, "PRETRAINING_AUDIT_MODEL_EVIDENCE_INCOMPLETE", {"missing_models": missing_evidence})

    matrix: list[dict[str, Any]] = []
    summary_rows: list[dict[str, Any]] = []
    repos = task_repos["repos"]
    for slot in active:
        slot_counts: Counter[str] = Counter()
        model = model_evidence.get(slot["repo"])
        for repo_row in repos:
            repo = repo_metadata[repo_row["repo"]]
            overlap = classify_overlap(slot, repo, model)
            slot_counts[overlap["classification"]] += 1
            matrix.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "task_id": TASK_ID,
                    "time": now,
                    "slot": slot["slot"].upper(),
                    "slot_kind": slot["kind"],
                    "model_id": slot["repo"] or None,
                    "pretrained": slot["pretrained"],
                    "repo": repo["repo"],
                    "repo_created_at": repo["created_at"],
                    "repo_task_count": repo_row["task_count"],
                    "repo_html_url": repo["html_url"],
                    **overlap,
                }
            )
        summary_rows.append(
            {
                "slot": slot["slot"].upper(),
                "name": slot["name"],
                "kind": slot["kind"],
                "model_id": slot["repo"] or None,
                "pretrained": slot["pretrained"],
                "deterministic": slot["deterministic"],
                "classification_counts": dict(sorted(slot_counts.items())),
                "model_release_date": model.get("release_date") if model else None,
                "pretraining_corpus": model.get("pretraining_corpus") if model else None,
                "cutoff_disclosed": model.get("cutoff_disclosed") if model else None,
                "pretraining_cutoff_date": model.get("pretraining_cutoff_date") if model else None,
                "leakage_safe_claim_allowed": all(
                    row["leakage_safe_claim"] for row in matrix if row["slot"] == slot["slot"].upper()
                ),
            }
        )

    matrix_counts = Counter(row["classification"] for row in matrix)
    unsafe_claims = [row for row in matrix if row["classification"] != "not_pretrained_no_corpus_leakage_surface" and row["leakage_safe_claim"]]
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "task_id": TASK_ID,
        "time": now,
        "passes": True,
        "decision": {
            "leakage_free_ship_gate_claim_allowed": False,
            "held_out_by_cutoff_experiment_required": True,
            "reason": "active pretrained embedders lack disclosed leakage-free evidence for SWE-bench Lite repos",
        },
        "doctrine": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "durable_state_requires_aiwonder_archive": True,
            "retired_static_ast_temporal_slots": list(RETIRED_STATIC_AST_TEMPORAL_SLOTS),
        },
        "inputs": {
            "active_static_slots": list(ACTIVE_STATIC_SLOTS),
            "active_embedder_count": len(active),
            "active_pretrained_slot_count": sum(1 for row in active if row["pretrained"]),
            "active_deterministic_slot_count": sum(1 for row in active if row["deterministic"]),
            "unique_pretrained_model_count": len(required_models),
            "task_count": task_repos["task_count"],
            "repo_count": task_repos["repo_count"],
        },
        "classification_counts": dict(sorted(matrix_counts.items())),
        "unsafe_claim_row_count": len(unsafe_claims),
        "active_embedder_summary": summary_rows,
        "source_evidence_hash": sha256_text(canonical_json(model_evidence)),
    }
    return {"manifest": manifest, "matrix": matrix, "evidence": model_evidence, "repo_metadata": repo_metadata}


def flatten_evidence(model_evidence: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for model_id, evidence in sorted(model_evidence.items()):
        for source in evidence["evidence_sources"]:
            rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "model_id": model_id,
                    "pretraining_corpus": evidence["pretraining_corpus"],
                    "cutoff_disclosed": evidence["cutoff_disclosed"],
                    "pretraining_cutoff_date": evidence["pretraining_cutoff_date"],
                    "release_date": evidence["release_date"],
                    **source,
                }
            )
    return rows


def copy_source_snapshots(source_snapshot_dir: Path | None, model_evidence: dict[str, dict[str, Any]], output_dir: Path) -> list[Path]:
    if source_snapshot_dir is None:
        return []
    out_dir = output_dir / "source_snapshots"
    out_dir.mkdir(parents=True, exist_ok=True)
    copied: list[Path] = []
    seen: set[str] = set()
    for evidence in model_evidence.values():
        for source in evidence["evidence_sources"]:
            snapshot = source.get("source_snapshot")
            if not snapshot or snapshot in seen:
                continue
            seen.add(snapshot)
            src = source_snapshot_dir / snapshot
            dst = out_dir / snapshot
            shutil.copy2(src, dst)
            copied.append(dst)
    return copied


def write_artifacts(
    *,
    output_root: Path,
    state_root: Path,
    run_id: str,
    audit: dict[str, Any],
    model_evidence_path: Path,
    repo_metadata_path: Path,
    models_config_path: Path,
    source_snapshot_dir: Path | None,
) -> dict[str, Any]:
    fsv_root = output_root / run_id if output_root.name != run_id else output_root
    state_run_root = state_root / run_id if state_root.name != run_id else state_root
    fsv_root.mkdir(parents=True, exist_ok=True)
    state_run_root.mkdir(parents=True, exist_ok=True)

    manifest = dict(audit["manifest"])
    manifest["run_id"] = run_id
    manifest["source_of_truth"] = {
        "fsv_root": str(fsv_root),
        "state_run_root": str(state_run_root),
        "models_config": str(models_config_path),
        "model_evidence_json": str(model_evidence_path),
        "repo_metadata_json": str(repo_metadata_path),
        "source_snapshot_dir": str(source_snapshot_dir) if source_snapshot_dir else None,
    }

    active_summary = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "run_id": run_id,
        "active_embedder_summary": manifest["active_embedder_summary"],
    }
    repo_rows = list(audit["repo_metadata"].values())
    evidence_rows = flatten_evidence(audit["evidence"])

    paths = {
        "manifest": fsv_root / "embedder_pretraining_audit_manifest.json",
        "active_summary": fsv_root / "active_embedder_summary.json",
        "matrix": fsv_root / "repo_overlap_matrix.jsonl",
        "model_evidence": fsv_root / "model_card_evidence.jsonl",
        "repo_metadata": fsv_root / "repo_metadata_readback.jsonl",
    }
    write_json(paths["manifest"], manifest)
    write_json(paths["active_summary"], active_summary)
    write_jsonl(paths["matrix"], audit["matrix"])
    write_jsonl(paths["model_evidence"], evidence_rows)
    write_jsonl(paths["repo_metadata"], sorted(repo_rows, key=lambda row: row["repo"]))
    copied = copy_source_snapshots(source_snapshot_dir, audit["evidence"], fsv_root)

    state_manifest = state_run_root / "embedder_pretraining_audit_manifest.json"
    state_matrix = state_run_root / "repo_overlap_matrix.jsonl"
    state_evidence = state_run_root / "model_card_evidence.jsonl"
    shutil.copy2(paths["manifest"], state_manifest)
    shutil.copy2(paths["matrix"], state_matrix)
    shutil.copy2(paths["model_evidence"], state_evidence)
    state_copied = copy_source_snapshots(source_snapshot_dir, audit["evidence"], state_run_root)

    artifact_files = list(paths.values()) + copied + [state_manifest, state_matrix, state_evidence] + state_copied
    hashes = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "run_id": run_id,
        "artifacts": {str(path): {"sha256": sha256_file(path), "bytes": path.stat().st_size} for path in sorted(artifact_files)},
    }
    hashes_path = fsv_root / "artifact_hashes.json"
    write_json(hashes_path, hashes)
    manifest["artifact_hashes_path"] = str(hashes_path)
    write_json(paths["manifest"], manifest)
    shutil.copy2(paths["manifest"], state_manifest)
    hashes["artifacts"][str(paths["manifest"])] = {"sha256": sha256_file(paths["manifest"]), "bytes": paths["manifest"].stat().st_size}
    write_json(hashes_path, hashes)
    return {
        "fsv_root": str(fsv_root),
        "state_run_root": str(state_run_root),
        "manifest": str(paths["manifest"]),
        "matrix": str(paths["matrix"]),
        "artifact_hashes": str(hashes_path),
        "matrix_row_count": len(audit["matrix"]),
        "model_evidence_row_count": len(evidence_rows),
        "repo_metadata_row_count": len(repo_rows),
    }


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    run_id = args.run_id or f"run-{utc_stamp()}"
    require_archive_path(args.output_root, "output_root", args.allow_non_aiwonder_roots)
    require_archive_path(args.state_root, "state_root", args.allow_non_aiwonder_roots)
    if args.source_snapshot_dir is not None:
        require(args.source_snapshot_dir.is_dir(), "PRETRAINING_AUDIT_SOURCE_SNAPSHOT_DIR_MISSING", {"path": str(args.source_snapshot_dir)})

    models_config = load_models_config(args.models_config)
    task_repos = load_task_repos(args.tasks_dir, args.expected_task_count)
    repo_metadata = load_repo_metadata(args.repo_metadata_json, [row["repo"] for row in task_repos["repos"]])
    model_evidence = load_model_evidence(args.model_evidence_json, args.source_snapshot_dir)
    audit = build_audit(
        models_config=models_config,
        task_repos=task_repos,
        repo_metadata=repo_metadata,
        model_evidence=model_evidence,
        now=utc_now(),
    )
    return write_artifacts(
        output_root=args.output_root,
        state_root=args.state_root,
        run_id=run_id,
        audit=audit,
        model_evidence_path=args.model_evidence_json,
        repo_metadata_path=args.repo_metadata_json,
        models_config_path=args.models_config,
        source_snapshot_dir=args.source_snapshot_dir,
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--models-config", type=Path, default=DEFAULT_MODELS_CONFIG)
    parser.add_argument("--tasks-dir", type=Path, default=DEFAULT_TASKS_DIR)
    parser.add_argument("--model-evidence-json", type=Path, required=True)
    parser.add_argument("--repo-metadata-json", type=Path, required=True)
    parser.add_argument("--source-snapshot-dir", type=Path, default=None)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--expected-task-count", type=int, default=300)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        result = run_audit(args)
    except PretrainingLeakageAuditError as exc:
        print(
            json.dumps(
                {
                    "ok": False,
                    "error_code": str(exc).split(":", 1)[0],
                    "message": str(exc),
                    "task_id": TASK_ID,
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 2
    print(json.dumps({"ok": True, **result}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
