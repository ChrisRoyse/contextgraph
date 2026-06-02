#!/usr/bin/env python3
"""Passive label/oracle agreement audit for BLINDSPOT-012 / #736."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable


SCHEMA_VERSION = 1
TASK_ID = "BLINDSPOT-012"
ARTIFACT_KIND = "label_oracle_agreement_audit"
FORMULA_VERSION = "label_oracle_agreement_v1"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_AUTO_LABEL_ROOT = Path("/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1")
DEFAULT_LABEL_REGISTRY = DEFAULT_AUTO_LABEL_ROOT / "accepted_label_registry.jsonl"
DEFAULT_OCCURRENCE_PATHS = (
    DEFAULT_AUTO_LABEL_ROOT / "chunk_constellation_labels.jsonl",
    DEFAULT_AUTO_LABEL_ROOT / "file_path_constellations.jsonl",
    DEFAULT_AUTO_LABEL_ROOT / "code_state_constellations.jsonl",
    DEFAULT_AUTO_LABEL_ROOT / "failure_evidence_sets.jsonl",
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/blindspot-012-label-oracle-agreement-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/label-oracle-agreement-audit/blindspot-012-v1")
TARGET_SIDE_LABEL_PREFIXES = (
    "oracle:",
    "docker:",
    "test_phase:",
    "test_result:",
    "code_state_outcome:",
    "partition:",
    "leakage_policy:",
    "ground_truth:",
    "truth:",
)


class LabelOracleAgreementAuditError(RuntimeError):
    pass


@dataclass(frozen=True)
class LabelProfile:
    label_id: str
    family: str
    status: str
    support: int
    fail_rate: float
    direction: str


@dataclass
class LabelAgreementStats:
    profile: LabelProfile
    oracle_pass_rows: int = 0
    oracle_fail_rows: int = 0
    missing_oracle_rows: int = 0
    agreement_rows: int = 0
    disagreement_rows: int = 0
    scope_counts: Counter[str] = field(default_factory=Counter)
    category_counts: Counter[str] = field(default_factory=Counter)
    disagreement_samples: list[dict[str, Any]] = field(default_factory=list)

    @property
    def audited_rows(self) -> int:
        return self.oracle_pass_rows + self.oracle_fail_rows

    @property
    def occurrence_rows(self) -> int:
        return self.audited_rows + self.missing_oracle_rows

    def observe(self, row: dict[str, Any], *, path: Path, line_no: int) -> None:
        scope = str(row.get("scope") or row.get("artifact_kind") or "unknown")
        category = str(row.get("mutation_category") or row.get("category") or "unknown")
        self.scope_counts[scope] += 1
        self.category_counts[category] += 1

        oracle = row.get("oracle_all_passed")
        if not isinstance(oracle, bool):
            self.missing_oracle_rows += 1
            return

        if oracle:
            self.oracle_pass_rows += 1
        else:
            self.oracle_fail_rows += 1

        expected_pass = self.profile.direction == "pass_likely"
        agrees = oracle is expected_pass
        if agrees:
            self.agreement_rows += 1
            return

        self.disagreement_rows += 1
        if len(self.disagreement_samples) < 10:
            self.disagreement_samples.append(
                {
                    "path": str(path),
                    "line_no": line_no,
                    "row_key": row.get("row_key") or row.get("group_key") or row.get("code_state_key"),
                    "task_instance_id": row.get("task_instance_id") or row.get("task_id"),
                    "mutation_category": category,
                    "scope": scope,
                    "oracle_all_passed": oracle,
                    "expected_oracle": "pass" if expected_pass else "fail",
                }
            )


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise LabelOracleAgreementAuditError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "LABEL_ORACLE_AGREEMENT_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


def reject_mounted_drive(label: str, path: Path) -> None:
    text = str(path)
    require(
        not (text.startswith("/mnt/c/") or text.startswith("/mnt/d/")),
        "LABEL_ORACLE_AGREEMENT_MOUNTED_DRIVE_FORBIDDEN",
        {label: text},
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


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


def read_jsonl(path: Path, code: str) -> list[tuple[int, dict[str, Any]]]:
    require(path.is_file(), code, {"path": str(path)})
    rows: list[tuple[int, dict[str, Any]]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise LabelOracleAgreementAuditError(
                    f"{code}: invalid JSONL line {line_no} at {path}: {exc}"
                ) from exc
            require(
                isinstance(value, dict),
                "LABEL_ORACLE_AGREEMENT_JSONL_ROW_NOT_OBJECT",
                {"path": str(path), "line_no": line_no},
            )
            rows.append((line_no, value))
    return rows


def finite_unit_interval(value: Any, *, field_name: str, label_id: str) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError) as exc:
        raise LabelOracleAgreementAuditError(
            f"LABEL_ORACLE_AGREEMENT_INVALID_RATE: {label_id} {field_name}={value!r}"
        ) from exc
    require(
        0.0 <= parsed <= 1.0,
        "LABEL_ORACLE_AGREEMENT_RATE_OUT_OF_RANGE",
        {"label_id": label_id, "field": field_name, "value": parsed},
    )
    return parsed


def classify_direction(fail_rate: float, *, fail_likely_min: float, pass_likely_max: float) -> str:
    if fail_rate >= fail_likely_min:
        return "fail_likely"
    if fail_rate <= pass_likely_max:
        return "pass_likely"
    return "mixed"


def target_side_label(label_id: str) -> bool:
    return label_id.startswith(TARGET_SIDE_LABEL_PREFIXES)


def load_label_profiles(
    path: Path,
    *,
    fail_likely_min: float,
    pass_likely_max: float,
    expected_label_count: int | None,
) -> dict[str, LabelProfile]:
    profiles: dict[str, LabelProfile] = {}
    for line_no, row in read_jsonl(path, "LABEL_ORACLE_AGREEMENT_REGISTRY_MISSING"):
        label_id = str(row.get("label_id") or "")
        require(label_id, "LABEL_ORACLE_AGREEMENT_REGISTRY_LABEL_ID_MISSING", {"path": str(path), "line_no": line_no})
        require(
            not target_side_label(label_id),
            "LABEL_ORACLE_AGREEMENT_TARGET_SIDE_REGISTRY_LABEL",
            {"path": str(path), "line_no": line_no, "label_id": label_id},
        )
        require(
            label_id not in profiles,
            "LABEL_ORACLE_AGREEMENT_DUPLICATE_REGISTRY_LABEL",
            {"path": str(path), "line_no": line_no, "label_id": label_id},
        )
        fail_rate = finite_unit_interval(row.get("fail_rate"), field_name="fail_rate", label_id=label_id)
        try:
            support = int(row.get("support") or 0)
        except (TypeError, ValueError) as exc:
            raise LabelOracleAgreementAuditError(
                f"LABEL_ORACLE_AGREEMENT_INVALID_SUPPORT: {label_id} support={row.get('support')!r}"
            ) from exc
        require(support >= 0, "LABEL_ORACLE_AGREEMENT_NEGATIVE_SUPPORT", {"label_id": label_id, "support": support})
        profiles[label_id] = LabelProfile(
            label_id=label_id,
            family=str(row.get("family") or label_id.split(":", 1)[0]),
            status=str(row.get("status") or "unknown"),
            support=support,
            fail_rate=fail_rate,
            direction=classify_direction(
                fail_rate,
                fail_likely_min=fail_likely_min,
                pass_likely_max=pass_likely_max,
            ),
        )
    if expected_label_count is not None:
        require(
            len(profiles) == expected_label_count,
            "LABEL_ORACLE_AGREEMENT_REGISTRY_COUNT_MISMATCH",
            {"expected": expected_label_count, "actual": len(profiles), "path": str(path)},
        )
    require(profiles, "LABEL_ORACLE_AGREEMENT_REGISTRY_EMPTY", {"path": str(path)})
    return profiles


def row_labels(row: dict[str, Any]) -> set[str]:
    out: set[str] = set()
    for field_name in ("labels", "live_predictor_labels", "target_supervision_labels"):
        value = row.get(field_name)
        if value is None:
            continue
        require(
            isinstance(value, list),
            "LABEL_ORACLE_AGREEMENT_LABEL_FIELD_NOT_LIST",
            {"field": field_name, "actual_type": type(value).__name__},
        )
        for item in value:
            text = str(item or "")
            if text:
                out.add(text)
    return out


def expected_oracle(direction: str) -> str | None:
    if direction == "fail_likely":
        return "fail"
    if direction == "pass_likely":
        return "pass"
    return None


def row_decision(
    stats: LabelAgreementStats,
    *,
    min_audited_occurrences: int,
    min_sound_agreement: float,
    structural_unsound_agreement: float,
) -> tuple[str, list[str]]:
    if stats.profile.direction == "mixed":
        return "not_directional_mixed_label", ["registry_fail_rate_not_directional"]
    if stats.audited_rows < min_audited_occurrences:
        return "insufficient_occurrence_join", [f"audited_occurrences<{min_audited_occurrences}"]
    rate = stats.agreement_rows / stats.audited_rows if stats.audited_rows else 0.0
    if rate < structural_unsound_agreement:
        return "targeted_label_fix_required", [f"agreement_rate<{structural_unsound_agreement}"]
    if rate < min_sound_agreement:
        return "review_required", [f"agreement_rate<{min_sound_agreement}"]
    if stats.missing_oracle_rows > 0:
        return "sound_with_missing_oracle_rows", ["some_occurrences_missing_oracle_all_passed"]
    return "sound", []


def matrix_row(
    stats: LabelAgreementStats,
    *,
    min_audited_occurrences: int,
    min_sound_agreement: float,
    structural_unsound_agreement: float,
) -> dict[str, Any]:
    decision, reasons = row_decision(
        stats,
        min_audited_occurrences=min_audited_occurrences,
        min_sound_agreement=min_sound_agreement,
        structural_unsound_agreement=structural_unsound_agreement,
    )
    agreement_rate = stats.agreement_rows / stats.audited_rows if stats.audited_rows else None
    return {
        "artifact_kind": f"{ARTIFACT_KIND}_matrix_row",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "label_id": stats.profile.label_id,
        "family": stats.profile.family,
        "registry_status": stats.profile.status,
        "registry_support": stats.profile.support,
        "registry_fail_rate": stats.profile.fail_rate,
        "expected_oracle": expected_oracle(stats.profile.direction),
        "direction": stats.profile.direction,
        "occurrence_rows": stats.occurrence_rows,
        "audited_rows": stats.audited_rows,
        "oracle_pass_rows": stats.oracle_pass_rows,
        "oracle_fail_rows": stats.oracle_fail_rows,
        "missing_oracle_rows": stats.missing_oracle_rows,
        "agreement_rows": stats.agreement_rows,
        "disagreement_rows": stats.disagreement_rows,
        "agreement_rate": agreement_rate,
        "decision": decision,
        "decision_reasons": reasons,
        "scope_counts": dict(sorted(stats.scope_counts.items())),
        "category_counts": dict(sorted(stats.category_counts.items())),
        "disagreement_samples": stats.disagreement_samples,
    }


def audit_occurrences(
    occurrence_paths: list[Path],
    profiles: dict[str, LabelProfile],
) -> tuple[dict[str, LabelAgreementStats], dict[str, Any]]:
    stats = {label_id: LabelAgreementStats(profile=profile) for label_id, profile in profiles.items()}
    counters: Counter[str] = Counter()
    unknown_labels: Counter[str] = Counter()
    occurrence_path_rows: dict[str, int] = {}
    for path in occurrence_paths:
        reject_mounted_drive("occurrence_path", path)
        rows = read_jsonl(path, "LABEL_ORACLE_AGREEMENT_OCCURRENCE_MISSING")
        occurrence_path_rows[str(path)] = len(rows)
        for line_no, row in rows:
            counters["occurrence_rows"] += 1
            labels = row_labels(row)
            if not labels:
                counters["rows_without_labels"] += 1
            for label_id in labels:
                profile = profiles.get(label_id)
                if profile is None:
                    unknown_labels[label_id] += 1
                    continue
                if profile.direction == "mixed":
                    stats[label_id].scope_counts[str(row.get("scope") or row.get("artifact_kind") or "unknown")] += 1
                    stats[label_id].category_counts[str(row.get("mutation_category") or row.get("category") or "unknown")] += 1
                    continue
                stats[label_id].observe(row, path=path, line_no=line_no)
    return stats, {
        "occurrence_path_rows": occurrence_path_rows,
        "counters": dict(sorted(counters.items())),
        "unknown_label_sample": [{"label_id": label, "occurrences": count} for label, count in unknown_labels.most_common(20)],
        "unknown_label_count": len(unknown_labels),
        "unknown_label_occurrences": sum(unknown_labels.values()),
    }


def build_manifest(
    *,
    args: argparse.Namespace,
    run_id: str,
    run_root: Path,
    state_run_root: Path,
    profiles: dict[str, LabelProfile],
    occurrence_summary: dict[str, Any],
    matrix_rows: list[dict[str, Any]],
    artifacts: dict[str, str],
) -> dict[str, Any]:
    decision_counts = Counter(row["decision"] for row in matrix_rows)
    directional_rows = [row for row in matrix_rows if row["direction"] != "mixed"]
    audited_directional_rows = [row for row in directional_rows if row["audited_rows"] > 0]
    promotion_blockers = []
    if decision_counts["targeted_label_fix_required"]:
        promotion_blockers.append("targeted_label_fix_required")
    if decision_counts["review_required"]:
        promotion_blockers.append("review_required")
    if decision_counts["insufficient_occurrence_join"]:
        promotion_blockers.append("insufficient_occurrence_join")
    if decision_counts["sound_with_missing_oracle_rows"]:
        promotion_blockers.append("missing_oracle_rows")
    if not directional_rows:
        decision = "no_directional_labels_to_audit"
    elif promotion_blockers:
        decision = "label_oracle_soundness_not_confirmed"
    elif len(audited_directional_rows) != len(directional_rows):
        decision = "incomplete_occurrence_join"
    else:
        decision = "labeler_sound_confirmed"
    return {
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "run_id": run_id,
        "created_at": utc_now(),
        "input_paths": {
            "label_registry": str(args.label_registry),
            "occurrence_paths": [str(path) for path in args.occurrences_jsonl],
        },
        "thresholds": {
            "fail_likely_min_fail_rate": args.fail_likely_min_fail_rate,
            "pass_likely_max_fail_rate": args.pass_likely_max_fail_rate,
            "min_sound_agreement": args.min_sound_agreement,
            "structural_unsound_agreement": args.structural_unsound_agreement,
            "min_audited_occurrences": args.min_audited_occurrences,
        },
        "label_counts": {
            "registry_labels": len(profiles),
            "directional_labels": len(directional_rows),
            "audited_directional_labels": len(audited_directional_rows),
            "mixed_labels": sum(1 for profile in profiles.values() if profile.direction == "mixed"),
        },
        "occurrence_summary": occurrence_summary,
        "decision_counts": dict(sorted(decision_counts.items())),
        "decision": {
            "label_oracle_agreement_measured": bool(audited_directional_rows),
            "passes": decision == "labeler_sound_confirmed",
            "decision": decision,
            "promotion_blockers": promotion_blockers,
        },
        "doctrine": {
            "measurement_only": True,
            "target_labels_used_as_live_inputs": False,
            "flat_vector_concat_used": False,
            "slot_identity_preserved": True,
            "inner_llm_used": False,
        },
        "artifacts": artifacts,
        "run_root": str(run_root),
        "state_run_root": str(state_run_root),
    }


def mirror_artifacts(run_root: Path, state_run_root: Path, filenames: Iterable[str]) -> None:
    state_run_root.mkdir(parents=True, exist_ok=True)
    for name in filenames:
        shutil.copy2(run_root / name, state_run_root / name)


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    run_id = args.run_id or f"run-{utc_stamp()}"
    require(args.fail_likely_min_fail_rate > args.pass_likely_max_fail_rate, "LABEL_ORACLE_AGREEMENT_DIRECTION_THRESHOLDS_OVERLAP")
    for path_label in ("label_registry", "output_root", "state_root"):
        reject_mounted_drive(path_label, Path(getattr(args, path_label)))
    require_archive_path(args.output_root, "output_root", args.allow_non_aiwonder_roots)
    require_archive_path(args.state_root, "state_root", args.allow_non_aiwonder_roots)

    run_root = args.output_root / run_id
    state_run_root = args.state_root / run_id
    require(not run_root.exists(), "LABEL_ORACLE_AGREEMENT_RUN_ROOT_EXISTS", {"run_root": str(run_root)})
    require(not state_run_root.exists(), "LABEL_ORACLE_AGREEMENT_STATE_RUN_ROOT_EXISTS", {"state_run_root": str(state_run_root)})
    run_root.mkdir(parents=True, exist_ok=False)

    profiles = load_label_profiles(
        args.label_registry,
        fail_likely_min=args.fail_likely_min_fail_rate,
        pass_likely_max=args.pass_likely_max_fail_rate,
        expected_label_count=args.expected_label_count,
    )
    stats, occurrence_summary = audit_occurrences(args.occurrences_jsonl, profiles)
    matrix_rows = [
        matrix_row(
            stats[label_id],
            min_audited_occurrences=args.min_audited_occurrences,
            min_sound_agreement=args.min_sound_agreement,
            structural_unsound_agreement=args.structural_unsound_agreement,
        )
        for label_id in sorted(stats)
    ]
    bottom_rows = sorted(
        (
            row
            for row in matrix_rows
            if row["direction"] != "mixed" and row["audited_rows"] > 0
        ),
        key=lambda row: (
            row["agreement_rate"] if row["agreement_rate"] is not None else -1.0,
            -row["audited_rows"],
            row["label_id"],
        ),
    )[:25]

    matrix_path = run_root / "label_oracle_agreement_matrix.jsonl"
    bottom_path = run_root / "bottom_disagreement_labels.jsonl"
    write_jsonl(matrix_path, matrix_rows)
    write_jsonl(bottom_path, bottom_rows)
    artifact_hashes = {
        "label_registry": sha256_file(args.label_registry),
        "matrix": sha256_file(matrix_path),
        "bottom_disagreement_labels": sha256_file(bottom_path),
    }
    for path in args.occurrences_jsonl:
        artifact_hashes[f"occurrences::{path.name}"] = sha256_file(path)
    artifact_hashes_path = run_root / "artifact_hashes.json"
    write_json(artifact_hashes_path, artifact_hashes)
    artifacts = {
        "matrix": str(matrix_path),
        "bottom_disagreement_labels": str(bottom_path),
        "artifact_hashes": str(artifact_hashes_path),
    }
    manifest = build_manifest(
        args=args,
        run_id=run_id,
        run_root=run_root,
        state_run_root=state_run_root,
        profiles=profiles,
        occurrence_summary=occurrence_summary,
        matrix_rows=matrix_rows,
        artifacts=artifacts,
    )
    manifest_path = run_root / "label_oracle_agreement_manifest.json"
    write_json(manifest_path, manifest)
    mirror_artifacts(
        run_root,
        state_run_root,
        (
            "label_oracle_agreement_matrix.jsonl",
            "bottom_disagreement_labels.jsonl",
            "artifact_hashes.json",
            "label_oracle_agreement_manifest.json",
        ),
    )
    return {
        "manifest": str(manifest_path),
        "matrix": str(matrix_path),
        "bottom_disagreement_labels": str(bottom_path),
        "artifact_hashes": str(artifact_hashes_path),
        "run_root": str(run_root),
        "state_run_root": str(state_run_root),
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label-registry", type=Path, default=DEFAULT_LABEL_REGISTRY)
    parser.add_argument("--occurrences-jsonl", type=Path, action="append", default=None)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--expected-label-count", type=int, default=None)
    parser.add_argument("--fail-likely-min-fail-rate", type=float, default=0.95)
    parser.add_argument("--pass-likely-max-fail-rate", type=float, default=0.05)
    parser.add_argument("--min-sound-agreement", type=float, default=0.97)
    parser.add_argument("--structural-unsound-agreement", type=float, default=0.90)
    parser.add_argument("--min-audited-occurrences", type=int, default=1)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    args = parser.parse_args(argv)
    args.occurrences_jsonl = args.occurrences_jsonl or list(DEFAULT_OCCURRENCE_PATHS)
    return args


def main(argv: list[str] | None = None) -> int:
    try:
        result = run_audit(parse_args(argv))
    except LabelOracleAgreementAuditError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps({"ok": True, **result}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
