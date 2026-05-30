from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_embedder_pretraining_leakage_audit import (
    PretrainingLeakageAuditError,
    classify_overlap,
    load_model_evidence,
    run_audit,
    sha256_file,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_models_config(path: Path, *, omit: str | None = None) -> None:
    embedders = {
        "e1": {"embedder": "e1", "name": "semantic", "kind": "content_pretrained", "repo": "model/e5", "dimension": 1024, "path": "/zfs/hot/contextgraph/models/e1", "manifest_sha256": "sha256:" + "1" * 64},
        "e6": {"embedder": "e6", "name": "sparse", "kind": "content_pretrained", "repo": "model/splade", "dimension": 30522, "path": "/zfs/hot/contextgraph/models/e6", "manifest_sha256": "sha256:" + "6" * 64},
        "e7": {"embedder": "e7", "name": "code", "kind": "content_pretrained", "repo": "model/qodo", "dimension": 1536, "path": "/zfs/hot/contextgraph/models/e7", "manifest_sha256": "sha256:" + "7" * 64},
        "e8": {"embedder": "e8", "name": "graph", "kind": "content_pretrained", "repo": "model/e5", "dimension": 1024, "path": "/zfs/hot/contextgraph/models/e8", "manifest_sha256": "sha256:" + "8" * 64},
        "e9": {"embedder": "e9", "name": "hdc", "kind": "content_deterministic", "repo": "", "dimension": 1024, "path": "", "manifest_sha256": ""},
        "e10": {"embedder": "e10", "name": "contextual", "kind": "content_pretrained", "repo": "model/e5-base", "dimension": 768, "path": "/zfs/hot/contextgraph/models/e10", "manifest_sha256": "sha256:" + "a" * 64},
        "e12": {"embedder": "e12", "name": "late", "kind": "content_pretrained", "repo": "model/colbert", "dimension": 128, "path": "/zfs/hot/contextgraph/models/e12", "manifest_sha256": "sha256:" + "b" * 64},
        "e13": {"embedder": "e13", "name": "splade_v3", "kind": "content_pretrained", "repo": "model/splade-pp", "dimension": 30522, "path": "/zfs/hot/contextgraph/models/e13", "manifest_sha256": "sha256:" + "c" * 64},
        "e14": {"embedder": "e14", "name": "bge", "kind": "content_pretrained", "repo": "model/bge", "dimension": 1024, "path": "/zfs/hot/contextgraph/models/e14", "manifest_sha256": "sha256:" + "d" * 64},
    }
    if omit:
        embedders.pop(omit)
    lines = ["schema_version = 1", ""]
    for slot, row in embedders.items():
        lines.append(f"[embedders.{slot}]")
        for key, value in row.items():
            if isinstance(value, str):
                lines.append(f'{key} = "{value}"')
            else:
                lines.append(f"{key} = {value}")
        lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def write_tasks(root: Path) -> None:
    write_json(root / "repo-a.json", {"instance_id": "repo-a", "repo": "example/repo-a"})
    write_json(root / "repo-b.json", {"instance_id": "repo-b", "repo": "example/repo-b"})


def write_repo_metadata(path: Path) -> None:
    write_json(
        path,
        {
            "repos": [
                {"full_name": "example/repo-a", "created_at": "2017-01-01T00:00:00Z", "html_url": "https://github.com/example/repo-a"},
                {"full_name": "example/repo-b", "created_at": "2024-01-01T00:00:00Z", "html_url": "https://github.com/example/repo-b"},
            ]
        },
    )


def write_evidence(root: Path, *, omit_model: str | None = None) -> tuple[Path, Path]:
    snapshots = root / "snapshots"
    snapshots.mkdir()
    model_ids = ["model/e5", "model/e5-base", "model/splade", "model/qodo", "model/colbert", "model/splade-pp", "model/bge"]
    models = []
    for model_id in model_ids:
        if model_id == omit_model:
            continue
        name = model_id.replace("/", "__") + ".README.md"
        snapshot = snapshots / name
        snapshot.write_text(f"# {model_id}\ntraining data not fully disclosed\n", encoding="utf-8")
        models.append(
            {
                "model_id": model_id,
                "pretraining_corpus": "undisclosed model-card corpus",
                "pretraining_cutoff_date": None,
                "cutoff_disclosed": False,
                "release_date": "2023-01-01T00:00:00Z",
                "training_data_disclosed": False,
                "leakage_safe_claim_allowed": False,
                "evidence_sources": [
                    {
                        "source_url": f"https://huggingface.co/{model_id}/raw/main/README.md",
                        "source_kind": "huggingface_model_card",
                        "source_snapshot": name,
                        "source_sha256": sha256_file(snapshot),
                        "evidence_summary": "model card exists but does not disclose a cutoff",
                        "supports": ["model_identity", "cutoff_absence"],
                    }
                ],
            }
        )
    evidence = root / "model_evidence.json"
    write_json(evidence, {"models": models})
    return evidence, snapshots


def run_fixture(tmp_path: Path, *, omit_model: str | None = None):
    models_config = tmp_path / "models.toml"
    tasks_dir = tmp_path / "tasks"
    repo_metadata = tmp_path / "repo_metadata.json"
    write_models_config(models_config)
    write_tasks(tasks_dir)
    write_repo_metadata(repo_metadata)
    evidence, snapshots = write_evidence(tmp_path, omit_model=omit_model)
    return run_audit(
        Namespace(
            models_config=models_config,
            tasks_dir=tasks_dir,
            model_evidence_json=evidence,
            repo_metadata_json=repo_metadata,
            source_snapshot_dir=snapshots,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_task_count=2,
            allow_non_aiwonder_roots=True,
        )
    )


def test_unknown_cutoff_produces_possible_overlap_not_safe_claim(tmp_path: Path):
    result = run_fixture(tmp_path)
    matrix_path = Path(result["matrix"])
    rows = [json.loads(line) for line in matrix_path.read_text(encoding="utf-8").splitlines()]
    pretrained_rows = [row for row in rows if row["pretrained"]]
    deterministic_rows = [row for row in rows if row["slot"] == "E9"]

    assert pretrained_rows
    assert all(row["classification"] == "unknown_possible_overlap" for row in pretrained_rows)
    assert all(row["leakage_safe_claim"] is False for row in pretrained_rows)
    assert {row["classification"] for row in deterministic_rows} == {"not_pretrained_no_corpus_leakage_surface"}

    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    assert manifest["classification_counts"]["unknown_possible_overlap"] == 16
    assert manifest["decision"]["leakage_free_ship_gate_claim_allowed"] is False
    assert Path(manifest["artifact_hashes_path"]).is_file()
    assert (Path(result["state_run_root"]) / "source_snapshots" / "model__e5.README.md").is_file()


def test_missing_active_model_evidence_fails_closed(tmp_path: Path):
    with pytest.raises(PretrainingLeakageAuditError, match="PRETRAINING_AUDIT_MODEL_EVIDENCE_INCOMPLETE"):
        run_fixture(tmp_path, omit_model="model/qodo")


def test_source_snapshot_hash_mismatch_fails_closed(tmp_path: Path):
    evidence, snapshots = write_evidence(tmp_path)
    doc = json.loads(evidence.read_text(encoding="utf-8"))
    doc["models"][0]["evidence_sources"][0]["source_sha256"] = "sha256:" + "0" * 64
    write_json(evidence, doc)

    with pytest.raises(PretrainingLeakageAuditError, match="PRETRAINING_AUDIT_SOURCE_SNAPSHOT_SHA_MISMATCH"):
        load_model_evidence(evidence, snapshots)


def test_disclosed_cutoff_distinguishes_pre_and_post_cutoff_repos():
    slot = {"slot": "e1", "deterministic": False, "repo": "model/e5"}
    evidence = {"cutoff_disclosed": True, "pretraining_cutoff_date": "2020-01-01T00:00:00Z"}

    before = classify_overlap(slot, {"created_at": "2019-01-01T00:00:00Z"}, evidence)
    after = classify_overlap(slot, {"created_at": "2021-01-01T00:00:00Z"}, evidence)

    assert before["classification"] == "repo_predates_cutoff_possible_overlap"
    assert before["leakage_safe_claim"] is False
    assert after["classification"] == "repo_created_after_cutoff_unlikely_pretraining_overlap"
    assert after["leakage_safe_claim"] is True


def test_durable_roots_require_aiwonder_without_test_escape(tmp_path: Path):
    models_config = tmp_path / "models.toml"
    tasks_dir = tmp_path / "tasks"
    repo_metadata = tmp_path / "repo_metadata.json"
    write_models_config(models_config)
    write_tasks(tasks_dir)
    write_repo_metadata(repo_metadata)
    evidence, snapshots = write_evidence(tmp_path)

    with pytest.raises(PretrainingLeakageAuditError, match="PRETRAINING_AUDIT_NON_AIWONDER_DURABLE_PATH"):
        run_audit(
            Namespace(
                models_config=models_config,
                tasks_dir=tasks_dir,
                model_evidence_json=evidence,
                repo_metadata_json=repo_metadata,
                source_snapshot_dir=snapshots,
                output_root=tmp_path / "fsv",
                state_root=Path("/zfs/archive/contextgraph/state/embedder-pretraining-audit"),
                run_id="run-test",
                expected_task_count=2,
                allow_non_aiwonder_roots=False,
            )
        )
