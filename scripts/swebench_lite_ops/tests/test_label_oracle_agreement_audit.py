from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_label_oracle_agreement_audit import (
    LabelOracleAgreementAuditError,
    run_audit,
)


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in rows),
        encoding="utf-8",
    )


def registry_row(label_id: str, *, fail_rate: float, support: int = 20) -> dict:
    return {
        "artifact_kind": "python_auto_label_discovery_accepted_label_registry_row",
        "schema_version": 1,
        "label_id": label_id,
        "label_hash": "sha256:" + "a" * 64,
        "family": label_id.split(":", 1)[0],
        "status": "accepted_predictive_candidate",
        "support": support,
        "fail_rate": fail_rate,
        "lift_over_weighted_category_baseline": 0.5,
        "live_prediction_input_allowed": True,
    }


def occurrence_row(
    row_key: str,
    labels: list[str],
    *,
    oracle_all_passed: bool | None,
    category: str = "off_by_one",
    scope: str = "chunk",
) -> dict:
    row = {
        "artifact_kind": "python_auto_label_discovery_chunk_constellation_labels",
        "schema_version": 1,
        "row_key": row_key,
        "task_instance_id": f"task-{row_key}",
        "mutation_category": category,
        "scope": scope,
        "labels": labels,
        "live_predictor_labels": labels,
        "target_supervision_labels": [],
    }
    if oracle_all_passed is not None:
        row["oracle_all_passed"] = oracle_all_passed
    return row


def run_fixture(tmp_path: Path, registry_rows: list[dict], occurrence_rows: list[dict]) -> dict:
    registry = tmp_path / "accepted_label_registry.jsonl"
    occurrences = tmp_path / "chunk_constellation_labels.jsonl"
    write_jsonl(registry, registry_rows)
    write_jsonl(occurrences, occurrence_rows)
    return run_audit(
        Namespace(
            label_registry=registry,
            occurrences_jsonl=[occurrences],
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_label_count=len(registry_rows),
            fail_likely_min_fail_rate=0.95,
            pass_likely_max_fail_rate=0.05,
            min_sound_agreement=0.97,
            structural_unsound_agreement=0.90,
            min_audited_occurrences=1,
            allow_non_aiwonder_roots=True,
        )
    )


def read_manifest(result: dict) -> dict:
    return json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))


def read_matrix(result: dict) -> dict[str, dict]:
    return {
        row["label_id"]: row
        for row in (
            json.loads(line)
            for line in Path(result["matrix"]).read_text(encoding="utf-8").splitlines()
            if line.strip()
        )
    }


def test_sound_directional_labels_confirm_labeler(tmp_path: Path):
    result = run_fixture(
        tmp_path,
        [
            registry_row("surface:assertion_deleted", fail_rate=1.0),
            registry_row("surface:known_good_shape", fail_rate=0.0),
            registry_row("surface:mixed_descriptor", fail_rate=0.5),
        ],
        [
            occurrence_row("fail-1", ["surface:assertion_deleted"], oracle_all_passed=False),
            occurrence_row("fail-2", ["surface:assertion_deleted"], oracle_all_passed=False),
            occurrence_row("pass-1", ["surface:known_good_shape"], oracle_all_passed=True),
            occurrence_row("pass-2", ["surface:known_good_shape", "surface:mixed_descriptor"], oracle_all_passed=True),
        ],
    )

    manifest = read_manifest(result)
    matrix = read_matrix(result)

    assert manifest["decision"]["decision"] == "labeler_sound_confirmed"
    assert manifest["decision"]["passes"] is True
    assert matrix["surface:assertion_deleted"]["agreement_rate"] == 1.0
    assert matrix["surface:known_good_shape"]["agreement_rate"] == 1.0
    assert matrix["surface:mixed_descriptor"]["decision"] == "not_directional_mixed_label"
    assert (Path(result["state_run_root"]) / "label_oracle_agreement_manifest.json").read_text(
        encoding="utf-8"
    ) == Path(result["manifest"]).read_text(encoding="utf-8")


def test_disagreement_below_structural_threshold_requires_targeted_fix(tmp_path: Path):
    result = run_fixture(
        tmp_path,
        [registry_row("surface:fail_likely", fail_rate=1.0)],
        [
            occurrence_row("fail-1", ["surface:fail_likely"], oracle_all_passed=False),
            occurrence_row("pass-1", ["surface:fail_likely"], oracle_all_passed=True),
        ],
    )

    manifest = read_manifest(result)
    row = read_matrix(result)["surface:fail_likely"]
    bottom_rows = [
        json.loads(line)
        for line in Path(result["bottom_disagreement_labels"]).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]

    assert manifest["decision"]["decision"] == "label_oracle_soundness_not_confirmed"
    assert "targeted_label_fix_required" in manifest["decision"]["promotion_blockers"]
    assert row["decision"] == "targeted_label_fix_required"
    assert row["agreement_rate"] == 0.5
    assert row["disagreement_samples"][0]["expected_oracle"] == "fail"
    assert bottom_rows[0]["label_id"] == "surface:fail_likely"


def test_missing_oracle_rows_are_not_silent_success(tmp_path: Path):
    result = run_fixture(
        tmp_path,
        [registry_row("surface:fail_likely", fail_rate=1.0)],
        [
            occurrence_row("fail-1", ["surface:fail_likely"], oracle_all_passed=False),
            occurrence_row("missing-1", ["surface:fail_likely"], oracle_all_passed=None),
        ],
    )

    manifest = read_manifest(result)
    row = read_matrix(result)["surface:fail_likely"]

    assert manifest["decision"]["decision"] == "label_oracle_soundness_not_confirmed"
    assert "missing_oracle_rows" in manifest["decision"]["promotion_blockers"]
    assert row["decision"] == "sound_with_missing_oracle_rows"
    assert row["missing_oracle_rows"] == 1


def test_target_side_registry_label_fails_closed(tmp_path: Path):
    registry = tmp_path / "accepted_label_registry.jsonl"
    occurrences = tmp_path / "chunk_constellation_labels.jsonl"
    write_jsonl(registry, [registry_row("oracle:pass", fail_rate=0.0)])
    write_jsonl(occurrences, [occurrence_row("pass-1", ["oracle:pass"], oracle_all_passed=True)])

    with pytest.raises(LabelOracleAgreementAuditError, match="LABEL_ORACLE_AGREEMENT_TARGET_SIDE_REGISTRY_LABEL"):
        run_audit(
            Namespace(
                label_registry=registry,
                occurrences_jsonl=[occurrences],
                output_root=tmp_path / "fsv",
                state_root=tmp_path / "state",
                run_id="run-test",
                expected_label_count=1,
                fail_likely_min_fail_rate=0.95,
                pass_likely_max_fail_rate=0.05,
                min_sound_agreement=0.97,
                structural_unsound_agreement=0.90,
                min_audited_occurrences=1,
                allow_non_aiwonder_roots=True,
            )
        )


def test_durable_roots_require_aiwonder_without_escape(tmp_path: Path):
    registry = tmp_path / "accepted_label_registry.jsonl"
    occurrences = tmp_path / "chunk_constellation_labels.jsonl"
    write_jsonl(registry, [registry_row("surface:known_good_shape", fail_rate=0.0)])
    write_jsonl(occurrences, [occurrence_row("pass-1", ["surface:known_good_shape"], oracle_all_passed=True)])

    with pytest.raises(LabelOracleAgreementAuditError, match="LABEL_ORACLE_AGREEMENT_NON_AIWONDER_DURABLE_PATH"):
        run_audit(
            Namespace(
                label_registry=registry,
                occurrences_jsonl=[occurrences],
                output_root=tmp_path / "fsv",
                state_root=Path("/zfs/archive/contextgraph/state/label-oracle-agreement-audit/blindspot-012-v1"),
                run_id="run-test",
                expected_label_count=1,
                fail_likely_min_fail_rate=0.95,
                pass_likely_max_fail_rate=0.05,
                min_sound_agreement=0.97,
                structural_unsound_agreement=0.90,
                min_audited_occurrences=1,
                allow_non_aiwonder_roots=False,
            )
        )
