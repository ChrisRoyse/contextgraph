#!/usr/bin/env python3
"""Strict Phase 0 verification for 5090jepa fixtures.

This verifies the actual files on disk. It does not fabricate fixture records
and does not claim runtime DynamicJEPA support exists.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from collections import Counter, defaultdict
from dataclasses import dataclass
from itertools import zip_longest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
CONFIG_ROOT = ROOT / "configs" / "dynamicjepa"
DOMAIN_ROOT = CONFIG_ROOT / "domain_packs"
VERIFY_ROOT = CONFIG_ROOT / "verification"
PROTOCOL_PATH = ROOT / "docs" / "5090jepa" / "14_full_state_verification_protocol.md"

ID_RE = re.compile(r"^[a-z][a-z0-9_.-]{2,127}$")
ENV_REF_RE = re.compile(r"\$\{[^}]+\}")

DJ_CF_NAMES = [
    "dj_domain_packs",
    "dj_domain_pack_by_nv",
    "dj_instrument_registry",
    "dj_adapter_registry",
    "dj_raw_events",
    "dj_normalized_states",
    "dj_actions",
    "dj_outcomes",
    "dj_transitions",
    "dj_adapter_runs",
    "dj_instrument_readings",
    "dj_latent_panels",
    "dj_bindings",
    "dj_bindings_by_entity",
    "dj_trajectories",
    "dj_dataset_shards",
    "dj_training_runs",
    "dj_model_artifacts",
    "dj_predictions",
    "dj_skill_policies",
    "dj_plan_traces",
    "dj_guard_decisions",
    "dj_surprise_events",
    "dj_verification_runs",
    "dj_audit_log",
]

REQUIRED_PHASE0_FILES = {
    "configs/dynamicjepa/domain_packs/counter_world.v1.toml": "a29762572b70fc787a1b098c2b4e4def60ddfaa8c081a5be65731bfefa2d5062",
    "configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml": "33b3603c6be4a56ff6c22eaf8446c48321abd40d6a3a3a9adf36aa500d3ee8b1",
    "configs/dynamicjepa/verification/counter_world_happy_path.jsonl": "dcfad3dec2405a48020e1ac5bb48299ba0e9855a763bee04926507e100762d3c",
    "configs/dynamicjepa/verification/counter_world_edge_empty_input.jsonl": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "configs/dynamicjepa/verification/counter_world_edge_invalid_action.jsonl": "33124ae82ccfc18b60a2302dd951abff737a049545bfcd833f10cd889674c8a3",
    "configs/dynamicjepa/verification/counter_world_edge_max_panel.jsonl": "95952008e5596931394183b615c0b9e5860b960b78ca1cc935fc99d2e8bfd486",
    "configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl": "3c216c56e160dc3bea0803bf53bbd2bca07d79b0aaa2a24cb1fe30fe8718537b",
    "configs/dynamicjepa/verification/gridworld_5x5_edge_empty.jsonl": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "configs/dynamicjepa/verification/gridworld_5x5_edge_invalid_action.jsonl": "1b15f6836c0847a51534a91a5ec559dba8d9939c3fea6bca8d1fba3f66618b17",
    "configs/dynamicjepa/verification/gridworld_5x5_edge_oob_state.jsonl": "26739226399d99eb0804579d5026ccf7fa4bf308f0ac9187dbdc607b30d85bc2",
    "configs/dynamicjepa/verification/tiny_train_config.json": "4b5bcdd881a72b9b330e605a618bf6d692f7ee3470f8d3c797a6bfdaed1eeff3",
    "configs/dynamicjepa/verification/gridworld_5x5_train_config.json": "262e01f5316af8fbf2b9601d3b87e53893694968e04c56b32b5af7e71ebae374",
}

COUNTER_HAPPY_ROWS = [
    {"state": {"counter": 0}, "action": {"delta": 1}, "outcome": {"next_counter": 1}, "ts": 1700000000000},
    {"state": {"counter": 1}, "action": {"delta": 2}, "outcome": {"next_counter": 3}, "ts": 1700000001000},
    {"state": {"counter": 3}, "action": {"delta": 1}, "outcome": {"next_counter": 4}, "ts": 1700000002000},
]

GRID_ACTIONS = ["up", "down", "left", "right", "noop"]
GRID = 5
GRID_TRAJECTORIES = 1000
GRID_TRAJ_LEN = 8

PROTOCOL_SNIPPETS = [
    "| `dj_raw_events` | 3 |",
    "| `dj_adapter_runs` | 3 (all `Completed`) |",
    "| `dj_dataset_shards` | 2 (train + val) |",
    "| `dj_raw_events` | 8000 |",
    "| `dj_trajectories` | 1000 |",
    "| `dj_dataset_shards` | 3 (train/val/test) |",
    "| `dj_raw_events` | +1 |",
    "`dj_raw_events` | 0",
]


@dataclass
class Phase0VerificationError(Exception):
    operation: str
    target: str
    expected: Any
    actual: Any
    remediation: str
    code: str = "PHASE0_VALIDATION_FAILED"

    def to_json(self) -> dict[str, Any]:
        return {
            "operation": self.operation,
            "target": self.target,
            "status": "error",
            "error_code": self.code,
            "expected": self.expected,
            "actual": self.actual,
            "remediation": self.remediation,
        }


def fail(operation: str, target: str, expected: Any, actual: Any, remediation: str) -> None:
    raise Phase0VerificationError(operation, target, expected, actual, remediation)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_json(value: Any) -> str:
    return json.dumps(value, separators=(",", ":"), sort_keys=False)


def assert_keys(
    obj: dict[str, Any],
    allowed: set[str],
    required: set[str],
    path: str,
) -> None:
    actual = set(obj)
    extra = sorted(actual - allowed)
    missing = sorted(required - actual)
    if extra:
        fail("strict_schema", path, f"no unknown keys; allowed={sorted(allowed)}", extra, "remove unknown keys or extend the Phase 0 validator intentionally")
    if missing:
        fail("strict_schema", path, f"required keys {sorted(required)}", missing, "add the missing required keys to the fixture contract")


def assert_equal(operation: str, target: str, expected: Any, actual: Any, remediation: str) -> None:
    if actual != expected:
        fail(operation, target, expected, actual, remediation)


def ensure_no_env_refs(value: Any, path: str) -> None:
    if isinstance(value, str):
        if ENV_REF_RE.search(value):
            fail("no_env_substitution", path, "no ${...} references", value, "replace environment-variable substitution with explicit fixture values")
        return
    if isinstance(value, list):
        for idx, item in enumerate(value):
            ensure_no_env_refs(item, f"{path}[{idx}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            ensure_no_env_refs(item, f"{path}.{key}")


def load_toml(path: Path) -> dict[str, Any]:
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        fail("parse_toml", str(path), "valid TOML", repr(exc), "fix TOML syntax before registration can be implemented")
    ensure_no_env_refs(data, str(path))
    return data


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        fail("parse_json", str(path), "valid JSON", repr(exc), "fix JSON syntax in the training config")


def load_jsonl(path: Path) -> list[tuple[int, bytes, Any]]:
    rows = []
    offset = 0
    raw = path.read_bytes()
    if raw and not raw.endswith(b"\n"):
        fail("parse_jsonl", str(path), "file ends with newline", "missing trailing newline", "write JSONL fixtures with one complete newline-terminated JSON object per row")
    for raw_line in raw.splitlines(keepends=True):
        line = raw_line.rstrip(b"\n")
        try:
            rows.append((offset, line, json.loads(line.decode("utf-8"))))
        except Exception as exc:
            fail("parse_jsonl", f"{path}:{offset}", "valid JSON object", repr(exc), "fix the JSONL row; comments and blank rows are not allowed")
        offset += len(raw_line)
    return rows


def validate_id(value: str, path: str) -> None:
    if not isinstance(value, str) or not ID_RE.match(value):
        fail("validate_id", path, "^[a-z][a-z0-9_.-]{2,127}$", value, "use lowercase ASCII ids only")


def validate_kind(kind: Any, path: str) -> None:
    if isinstance(kind, str):
        if kind not in {"i64", "f64", "bool", "string", "unix_ms"}:
            fail("validate_kind", path, "known scalar kind", kind, "use a declared Phase 0 FieldKind")
        return
    if not isinstance(kind, dict):
        fail("validate_kind", path, "string or table", type(kind).__name__, "encode FieldKind as TOML string or inline table")
    assert_keys(kind, {"type", "variants", "dim", "encoding"}, {"type"}, path)
    kind_type = kind["type"]
    if kind_type == "categorical":
        variants = kind.get("variants")
        if not isinstance(variants, list) or not variants or any(not isinstance(v, str) for v in variants):
            fail("validate_kind", path, "categorical variants list", variants, "declare non-empty string variants")
    elif kind_type == "onehot":
        dim = kind.get("dim")
        if not isinstance(dim, int) or dim <= 0:
            fail("validate_kind", path, "positive onehot dim", dim, "declare a positive integer dim")
    elif kind_type not in {"i64", "f64", "bool", "string", "vector", "unix_ms", "scalar", "time"}:
        fail("validate_kind", path, "known kind type", kind_type, "use a declared Phase 0 kind type")


def validate_field(field: dict[str, Any], path: str) -> str:
    assert_keys(field, {"name", "kind", "required", "min", "max"}, {"name", "kind", "required"}, path)
    name = field["name"]
    if not isinstance(name, str) or not name:
        fail("validate_field", f"{path}.name", "non-empty string", name, "declare a field name")
    if not isinstance(field["required"], bool):
        fail("validate_field", f"{path}.required", "bool", field["required"], "set required to true or false")
    validate_kind(field["kind"], f"{path}.kind")
    for bound in ("min", "max"):
        if bound in field and not isinstance(field[bound], (int, float)):
            fail("validate_field", f"{path}.{bound}", "number", field[bound], "use numeric bounds")
    return name


def validate_schema_fields(section: dict[str, Any], section_name: str) -> list[str]:
    assert_keys(section, {"fields"}, {"fields"}, section_name)
    fields = section["fields"]
    if not isinstance(fields, list):
        fail("validate_schema", f"{section_name}.fields", "list", type(fields).__name__, "use TOML array-of-tables for schema fields")
    names = [validate_field(field, f"{section_name}.fields[{idx}]") for idx, field in enumerate(fields)]
    duplicates = [name for name, count in Counter(names).items() if count > 1]
    if duplicates:
        fail("validate_schema", f"{section_name}.fields", "unique field names", duplicates, "remove duplicate field declarations")
    return names


def validate_instrument(spec: dict[str, Any], path: str) -> dict[str, Any]:
    allowed = {
        "id",
        "kind",
        "input_fields",
        "output_shape",
        "normalization",
        "required",
        "model_ref",
        "version",
        "validation",
    }
    assert_keys(spec, allowed, {"id", "kind", "input_fields", "output_shape", "normalization", "required", "version", "validation"}, path)
    validate_id(spec["id"], f"{path}.id")
    validate_kind(spec["kind"], f"{path}.kind")
    if not isinstance(spec["input_fields"], list) or not spec["input_fields"] or any(not isinstance(item, str) for item in spec["input_fields"]):
        fail("validate_instrument", f"{path}.input_fields", "non-empty list of dotted paths", spec["input_fields"], "declare instrument inputs")
    shape = spec["output_shape"]
    if not isinstance(shape, list) or not shape or any(not isinstance(dim, int) or dim <= 0 for dim in shape):
        fail("validate_instrument", f"{path}.output_shape", "positive integer dimensions", shape, "declare a strict output shape")
    if not isinstance(spec["required"], bool):
        fail("validate_instrument", f"{path}.required", "bool", spec["required"], "set required to true or false")
    if spec["version"] != 1:
        fail("validate_instrument", f"{path}.version", 1, spec["version"], "Phase 0 instruments use version 1")
    assert_keys(spec["normalization"], {"type", "mean", "std"}, {"type"}, f"{path}.normalization")
    assert_keys(spec["validation"], {"require_finite", "min", "max", "reject_nan", "reject_inf"}, {"require_finite", "reject_nan", "reject_inf"}, f"{path}.validation")
    return {"id": spec["id"], "required": spec["required"], "output_shape": shape}


def validate_domain_pack(path: Path, expected_id: str) -> dict[str, Any]:
    data = load_toml(path)
    assert_keys(
        data,
        {
            "domain",
            "time",
            "state",
            "action",
            "outcome",
            "entity",
            "instruments",
            "adapters",
            "objectives",
            "invariants",
            "dataset_policy",
            "planner_policy",
            "verification_policy",
        },
        {
            "domain",
            "time",
            "state",
            "action",
            "outcome",
            "entity",
            "instruments",
            "adapters",
            "objectives",
            "invariants",
            "dataset_policy",
            "planner_policy",
            "verification_policy",
        },
        str(path),
    )

    domain = data["domain"]
    assert_keys(domain, {"id", "version", "title", "schema_version"}, {"id", "version", "title", "schema_version"}, "domain")
    assert_equal("validate_domain", "domain.id", expected_id, domain["id"], "the TOML filename and domain.id must agree")
    validate_id(domain["id"], "domain.id")
    assert_equal("validate_domain", "domain.version", "1.0.0", domain["version"], "Phase 0 locks version 1.0.0")
    assert_equal("validate_domain", "domain.schema_version", 1, domain["schema_version"], "Phase 0 uses schema version 1")

    assert_keys(data["time"], {"field"}, {"field"}, "time")
    validate_field(data["time"]["field"], "time.field")
    field_names = {
        "state": validate_schema_fields(data["state"], "state"),
        "action": validate_schema_fields(data["action"], "action"),
        "outcome": validate_schema_fields(data["outcome"], "outcome"),
        "entity": validate_schema_fields(data["entity"], "entity"),
    }

    instruments = [validate_instrument(spec, f"instruments[{idx}]") for idx, spec in enumerate(data["instruments"])]
    instrument_ids = [item["id"] for item in instruments]
    duplicates = [name for name, count in Counter(instrument_ids).items() if count > 1]
    if duplicates:
        fail("validate_domain", "instruments.id", "unique instrument ids", duplicates, "remove duplicate instrument ids")

    adapters = data["adapters"]
    if not isinstance(adapters, list) or not adapters:
        fail("validate_domain", "adapters", "at least one adapter", adapters, "declare json_event adapter")
    adapter_ids = []
    for idx, adapter in enumerate(adapters):
        assert_keys(adapter, {"id", "kind", "version", "mapping"}, {"id", "kind", "version", "mapping"}, f"adapters[{idx}]")
        validate_id(adapter["id"], f"adapters[{idx}].id")
        assert_equal("validate_domain", f"adapters[{idx}].kind", "json_event", adapter["kind"], "Phase 0 supports json_event only")
        assert_equal("validate_domain", f"adapters[{idx}].version", 1, adapter["version"], "Phase 0 adapter version is 1")
        if not isinstance(adapter["mapping"], dict) or not adapter["mapping"]:
            fail("validate_domain", f"adapters[{idx}].mapping", "non-empty mapping", adapter["mapping"], "declare JSON path mappings")
        for output_path, source_path in adapter["mapping"].items():
            if not isinstance(output_path, str) or not isinstance(source_path, str) or not source_path.startswith("$."):
                fail("validate_domain", f"adapters[{idx}].mapping", "dotted output paths and $. input paths", adapter["mapping"], "use the json_event path subset")
        adapter_ids.append(adapter["id"])
    duplicates = [name for name, count in Counter(adapter_ids).items() if count > 1]
    if duplicates:
        fail("validate_domain", "adapters.id", "unique adapter ids", duplicates, "remove duplicate adapter ids")

    objectives = data["objectives"]
    if not isinstance(objectives, list) or not objectives:
        fail("validate_domain", "objectives", "at least one objective", objectives, "declare latent_delta objective")
    for idx, objective in enumerate(objectives):
        assert_keys(objective, {"id", "kind", "input_panel", "target", "loss_weight", "required_dataset_fields"}, {"id", "kind", "input_panel", "target", "loss_weight", "required_dataset_fields"}, f"objectives[{idx}]")
        validate_id(objective["id"], f"objectives[{idx}].id")
        assert_equal("validate_domain", f"objectives[{idx}].kind", "latent_delta", objective["kind"], "Phase 0 supports latent_delta objective")

    for idx, invariant in enumerate(data["invariants"]):
        assert_keys(invariant, {"id", "expression", "severity"}, {"id", "expression", "severity"}, f"invariants[{idx}]")
        validate_id(invariant["id"], f"invariants[{idx}].id")
        assert_equal("validate_domain", f"invariants[{idx}].severity", "fatal", invariant["severity"], "Phase 0 invariants are fatal")

    dataset_allowed = {"default_split", "split_key", "split_buckets", "default_objective", "negative_sampling", "min_negatives_per_row"}
    assert_keys(data["dataset_policy"], dataset_allowed, {"default_split", "default_objective", "negative_sampling", "min_negatives_per_row"}, "dataset_policy")
    assert_equal("validate_domain", "dataset_policy.negative_sampling", "same_domain_different_outcome", data["dataset_policy"]["negative_sampling"], "Phase 0 uses real same-domain negatives")

    assert_keys(data["planner_policy"], {"candidate_actions", "guards", "surprise_threshold"}, {"candidate_actions", "guards", "surprise_threshold"}, "planner_policy")
    candidate_actions = data["planner_policy"]["candidate_actions"]
    assert_keys(candidate_actions, {"kind", "deltas", "moves"}, {"kind"}, "planner_policy.candidate_actions")
    assert_equal("validate_domain", "planner_policy.candidate_actions.kind", "enumerated", candidate_actions["kind"], "Phase 0 uses enumerated actions")
    assert_equal("validate_domain", "planner_policy.guards", ["bounds_check"], data["planner_policy"]["guards"], "Phase 0 guard list is bounds_check only")

    verification = data["verification_policy"]
    if domain["id"] == "counter_world":
        required_verification = {"required_cfs", "expected_event_count", "expected_panel_count", "expected_dataset_row_count"}
    else:
        required_verification = {
            "required_cfs",
            "expected_trajectory_count",
            "expected_event_count",
            "expected_panel_count",
            "expected_train_rows_min",
            "expected_train_rows_max",
            "expected_val_rows_min",
            "expected_val_rows_max",
            "expected_test_rows_min",
            "expected_test_rows_max",
        }
    assert_keys(verification, required_verification, required_verification, "verification_policy")

    required_cfs = verification["required_cfs"]
    if not isinstance(required_cfs, list) or not required_cfs:
        fail("validate_domain", "verification_policy.required_cfs", "non-empty list", required_cfs, "list every source-of-truth CF required by FSV")
    unknown_cfs = sorted(set(required_cfs) - set(DJ_CF_NAMES))
    if unknown_cfs:
        fail("validate_domain", "verification_policy.required_cfs", "known DynamicJEPA CFs", unknown_cfs, "fix CF names to match 05_storage_schema_v1.md")

    return {
        "path": str(path.relative_to(ROOT)),
        "id": domain["id"],
        "version": domain["version"],
        "state_fields": field_names["state"],
        "action_fields": field_names["action"],
        "outcome_fields": field_names["outcome"],
        "entity_fields": field_names["entity"],
        "instrument_count": len(instruments),
        "adapter_count": len(adapters),
        "objective_count": len(objectives),
        "required_instrument_count": sum(1 for item in instruments if item["required"]),
        "required_cfs": required_cfs,
        "verification_policy": verification,
    }


def grid_step(x: int, y: int, action: str) -> tuple[int, int]:
    if action == "up":
        return x, max(y - 1, 0)
    if action == "down":
        return x, min(y + 1, GRID - 1)
    if action == "left":
        return max(x - 1, 0), y
    if action == "right":
        return min(x + 1, GRID - 1), y
    if action == "noop":
        return x, y
    fail("grid_step", "action", GRID_ACTIONS, action, "use a declared gridworld action")


def expected_grid_rows():
    import random

    rng = random.Random(0x5090)
    for trajectory_index in range(GRID_TRAJECTORIES):
        trajectory_id = hashlib.blake2b(f"gridworld5x5:{trajectory_index}".encode("utf-8"), digest_size=8).hexdigest()
        x = rng.randrange(GRID)
        y = rng.randrange(GRID)
        for step_index in range(GRID_TRAJ_LEN):
            action = rng.choice(GRID_ACTIONS)
            next_x, next_y = grid_step(x, y, action)
            yield {
                "trajectory_id": trajectory_id,
                "step": step_index,
                "state": {"x": x, "y": y},
                "action": {"move": action},
                "outcome": {"next_x": next_x, "next_y": next_y},
            }
            x, y = next_x, next_y


def validate_counter_fixtures() -> dict[str, Any]:
    path = VERIFY_ROOT / "counter_world_happy_path.jsonl"
    rows = load_jsonl(path)
    assert_equal("validate_counter_fixture", str(path), len(COUNTER_HAPPY_ROWS), len(rows), "counter happy path must contain exactly three events")
    previous_ts = None
    for idx, ((offset, raw_line, row), expected) in enumerate(zip(rows, COUNTER_HAPPY_ROWS)):
        assert_equal("validate_counter_fixture", f"{path}:{offset}", expected, row, "restore the locked counter_world happy-path row")
        canonical = canonical_json(expected).encode("utf-8")
        assert_equal("validate_counter_fixture", f"{path}:{offset}:bytes", canonical, raw_line, "write canonical JSONL without whitespace drift")
        actual_next = row["state"]["counter"] + row["action"]["delta"]
        assert_equal("validate_counter_arithmetic", f"{path}:row{idx}", row["outcome"]["next_counter"], actual_next, "counter_world outcome must equal state.counter + action.delta")
        if previous_ts is not None and row["ts"] <= previous_ts:
            fail("validate_counter_fixture", f"{path}:row{idx}.ts", f"> {previous_ts}", row["ts"], "timestamps must be strictly increasing")
        previous_ts = row["ts"]

    empty_path = VERIFY_ROOT / "counter_world_edge_empty_input.jsonl"
    assert_equal("validate_counter_edge", str(empty_path), 0, empty_path.stat().st_size, "empty input edge fixture must remain zero bytes")

    invalid_action = load_jsonl(VERIFY_ROOT / "counter_world_edge_invalid_action.jsonl")
    assert_equal("validate_counter_edge", "counter invalid action row count", 1, len(invalid_action), "invalid action edge fixture is a single real event")
    assert_equal("validate_counter_edge", "counter invalid action value", "two", invalid_action[0][2]["action"]["delta"], "edge case must preserve the invalid string action")

    max_panel = load_jsonl(VERIFY_ROOT / "counter_world_edge_max_panel.jsonl")
    assert_equal("validate_counter_edge", "counter max panel row count", 1, len(max_panel), "max panel edge fixture is a single real event")
    assert_equal("validate_counter_edge", "counter max panel next_counter", 1032, max_panel[0][2]["outcome"]["next_counter"], "edge case must exceed the declared max=1024")

    return {
        "happy_path_events": len(rows),
        "happy_path_offsets": [offset for offset, _, _ in rows],
        "edge_empty_bytes": empty_path.stat().st_size,
        "edge_invalid_action_expected_failure": {
            "field_path": "action.delta",
            "expected_kind": "i64",
            "actual_kind": "string",
            "expected_deltas": {"dj_raw_events": 1, "dj_adapter_runs": 1, "dj_normalized_states": 0, "dj_actions": 0, "dj_outcomes": 0, "dj_transitions": 0},
        },
        "edge_max_panel_expected_failure": {
            "field_path": "outcome.next_counter",
            "value": 1032,
            "max": 1024,
            "expected_deltas": {"dj_raw_events": 1, "dj_adapter_runs": 1, "dj_normalized_states": 0, "dj_actions": 0, "dj_outcomes": 0, "dj_transitions": 0},
        },
    }


def validate_gridworld_fixtures() -> dict[str, Any]:
    path = VERIFY_ROOT / "gridworld_5x5_corpus.jsonl"
    rows = load_jsonl(path)
    expected_count = GRID_TRAJECTORIES * GRID_TRAJ_LEN
    assert_equal("validate_gridworld_fixture", str(path), expected_count, len(rows), "gridworld corpus must contain 1000 trajectories x 8 events")

    per_trajectory_steps: dict[str, list[int]] = defaultdict(list)
    action_counts: Counter[str] = Counter()
    first_row = None
    last_row = None

    expected_iter = expected_grid_rows()
    for idx, actual_tuple in enumerate(rows):
        offset, raw_line, actual = actual_tuple
        try:
            expected = next(expected_iter)
        except StopIteration:
            fail("validate_gridworld_fixture", str(path), GRID_TRAJECTORIES * GRID_TRAJ_LEN, idx, "gridworld fixture has more rows than the deterministic generator")
        assert_equal("validate_gridworld_fixture", f"{path}:line{idx + 1}", expected, actual, "regenerate gridworld corpus from the locked generator")
        canonical = canonical_json(expected).encode("utf-8")
        assert_equal("validate_gridworld_fixture", f"{path}:line{idx + 1}:bytes", canonical, raw_line, "gridworld corpus must use canonical JSON bytes")

        trajectory_id = actual["trajectory_id"]
        per_trajectory_steps[trajectory_id].append(actual["step"])
        action = actual["action"]["move"]
        action_counts[action] += 1
        expected_next = grid_step(actual["state"]["x"], actual["state"]["y"], action)
        assert_equal("validate_gridworld_transition", f"{path}:line{idx + 1}.outcome", {"next_x": expected_next[0], "next_y": expected_next[1]}, actual["outcome"], "gridworld outcome must match wall-clipped transition")
        if first_row is None:
            first_row = actual
        last_row = actual

    try:
        extra_expected = next(expected_iter)
    except StopIteration:
        extra_expected = None
    if extra_expected is not None:
        fail("validate_gridworld_fixture", str(path), expected_count, len(rows), "gridworld fixture ended before expected generator sequence")

    assert_equal("validate_gridworld_fixture", "unique trajectories", GRID_TRAJECTORIES, len(per_trajectory_steps), "gridworld corpus must contain 1000 unique trajectories")
    bad_steps = {
        trajectory_id: steps
        for trajectory_id, steps in per_trajectory_steps.items()
        if steps != list(range(GRID_TRAJ_LEN))
    }
    if bad_steps:
        sample_key = sorted(bad_steps)[0]
        fail("validate_gridworld_fixture", f"trajectory {sample_key}", list(range(GRID_TRAJ_LEN)), bad_steps[sample_key], "each gridworld trajectory must have steps 0..7 exactly once in order")
    missing_actions = sorted(set(GRID_ACTIONS) - set(action_counts))
    if missing_actions:
        fail("validate_gridworld_fixture", "action coverage", GRID_ACTIONS, dict(action_counts), "all declared actions must appear in the corpus")

    empty_path = VERIFY_ROOT / "gridworld_5x5_edge_empty.jsonl"
    assert_equal("validate_gridworld_edge", str(empty_path), 0, empty_path.stat().st_size, "empty gridworld edge fixture must remain zero bytes")

    invalid_action = load_jsonl(VERIFY_ROOT / "gridworld_5x5_edge_invalid_action.jsonl")
    assert_equal("validate_gridworld_edge", "grid invalid action row count", 1, len(invalid_action), "invalid action edge fixture is a single real event")
    assert_equal("validate_gridworld_edge", "grid invalid action value", "diagonal", invalid_action[0][2]["action"]["move"], "edge case must preserve undeclared move")

    oob_state = load_jsonl(VERIFY_ROOT / "gridworld_5x5_edge_oob_state.jsonl")
    assert_equal("validate_gridworld_edge", "grid oob row count", 1, len(oob_state), "oob state edge fixture is a single real event")
    assert_equal("validate_gridworld_edge", "grid oob state.x", 7, oob_state[0][2]["state"]["x"], "edge case must exceed max x=4")

    return {
        "corpus_events": len(rows),
        "trajectory_count": len(per_trajectory_steps),
        "trajectory_length": GRID_TRAJ_LEN,
        "action_counts": dict(sorted(action_counts.items())),
        "first_row": first_row,
        "last_row": last_row,
        "edge_empty_bytes": empty_path.stat().st_size,
        "edge_invalid_action_expected_failure": {
            "field_path": "action.move",
            "expected_kind": "categorical{up,down,left,right,noop}",
            "actual": "diagonal",
            "expected_deltas": {"dj_raw_events": 1, "dj_adapter_runs": 1, "dj_normalized_states": 0, "dj_actions": 0, "dj_outcomes": 0, "dj_transitions": 0},
        },
        "edge_oob_state_expected_failure": {
            "field_path": "state.x",
            "value": 7,
            "max": 4,
            "expected_deltas": {"dj_raw_events": 1, "dj_adapter_runs": 1, "dj_normalized_states": 0, "dj_actions": 0, "dj_outcomes": 0, "dj_transitions": 0},
        },
    }


def validate_train_configs() -> dict[str, Any]:
    counter = load_json(VERIFY_ROOT / "tiny_train_config.json")
    grid = load_json(VERIFY_ROOT / "gridworld_5x5_train_config.json")
    expected_counter = {
        "model": {"encoder": {"kind": "mlp", "hidden": [16], "out_dim": 8}, "predictor": {"kind": "mlp", "hidden": [16], "in_action_dim": 1, "out_dim": 8}, "ema_momentum": 0.99},
        "loss": {"latent_mse_weight": 1.0, "vicreg_variance_weight": 1.0, "vicreg_covariance_weight": 0.04, "vicreg_target_std": 1.0},
        "optim": {"kind": "adamw", "lr": 0.001, "weight_decay": 0.0001, "warmup_steps": 0},
        "schedule": {"epochs": 200, "batch_size": 2, "device": "cuda"},
        "stopping": {"metric": "val_latent_mse", "target": 0.005, "max_seconds": 30},
        "seed": 42,
    }
    expected_grid = {
        "model": {"encoder": {"kind": "mlp", "hidden": [64, 64], "out_dim": 32}, "predictor": {"kind": "mlp", "hidden": [64, 64], "in_action_dim": 5, "out_dim": 32}, "ema_momentum": 0.995},
        "loss": {"latent_mse_weight": 1.0, "vicreg_variance_weight": 1.0, "vicreg_covariance_weight": 0.04, "vicreg_target_std": 1.0},
        "optim": {"kind": "adamw", "lr": 0.0005, "weight_decay": 0.0001, "warmup_steps": 200},
        "schedule": {"epochs": 30, "batch_size": 256, "device": "cuda"},
        "stopping": {"metric": "val_latent_mse", "target": 0.04, "max_seconds": 90},
        "seed": 42,
    }
    assert_equal("validate_train_config", "tiny_train_config.json", expected_counter, counter, "restore locked counter_world train config")
    assert_equal("validate_train_config", "gridworld_5x5_train_config.json", expected_grid, grid, "restore locked gridworld train config")
    return {
        "counter_world": {"device": counter["schedule"]["device"], "epochs": counter["schedule"]["epochs"], "batch_size": counter["schedule"]["batch_size"]},
        "gridworld_5x5": {"device": grid["schedule"]["device"], "epochs": grid["schedule"]["epochs"], "batch_size": grid["schedule"]["batch_size"]},
    }


def validate_expected_counts(counter_domain: dict[str, Any], grid_domain: dict[str, Any]) -> dict[str, Any]:
    counter_policy = counter_domain["verification_policy"]
    grid_policy = grid_domain["verification_policy"]

    assert_equal("validate_expected_counts", "counter expected_event_count", 3, counter_policy["expected_event_count"], "counter policy must match happy fixture event count")
    assert_equal("validate_expected_counts", "counter expected_panel_count", 3, counter_policy["expected_panel_count"], "counter policy must match expected materialized panels")
    assert_equal("validate_expected_counts", "counter expected_dataset_row_count", 2, counter_policy["expected_dataset_row_count"], "counter policy must match t0->t1 and t1->t2 rows")

    assert_equal("validate_expected_counts", "grid expected_trajectory_count", GRID_TRAJECTORIES, grid_policy["expected_trajectory_count"], "grid policy must match generator trajectory count")
    assert_equal("validate_expected_counts", "grid expected_event_count", GRID_TRAJECTORIES * GRID_TRAJ_LEN, grid_policy["expected_event_count"], "grid policy must match generator event count")
    assert_equal("validate_expected_counts", "grid expected_panel_count", GRID_TRAJECTORIES * GRID_TRAJ_LEN, grid_policy["expected_panel_count"], "grid policy must match one panel per event")

    total_grid_rows = GRID_TRAJECTORIES * (GRID_TRAJ_LEN - 1)
    expected_nominal = {"train": int(total_grid_rows * 0.70), "val": int(total_grid_rows * 0.20), "test": int(total_grid_rows * 0.10)}
    ranges = {
        "train": [grid_policy["expected_train_rows_min"], grid_policy["expected_train_rows_max"]],
        "val": [grid_policy["expected_val_rows_min"], grid_policy["expected_val_rows_max"]],
        "test": [grid_policy["expected_test_rows_min"], grid_policy["expected_test_rows_max"]],
    }
    for split, nominal in expected_nominal.items():
        lower, upper = ranges[split]
        if not (lower <= nominal <= upper):
            fail("validate_expected_counts", f"grid {split} row range", f"{lower} <= {nominal} <= {upper}", nominal, "update verification_policy row ranges to include deterministic nominal rows")

    counter_final_counts = {
        "dj_domain_packs": 1,
        "dj_domain_pack_by_nv": 1,
        "dj_instrument_registry": counter_domain["instrument_count"],
        "dj_adapter_registry": 1,
        "dj_raw_events": 3,
        "dj_adapter_runs": 3,
        "dj_normalized_states": 3,
        "dj_actions": 8,
        "dj_outcomes": 3,
        "dj_transitions": 3,
        "dj_instrument_readings": 3 * counter_domain["instrument_count"],
        "dj_latent_panels": 3,
        "dj_bindings": 0,
        "dj_bindings_by_entity": 0,
        "dj_trajectories": 1,
        "dj_dataset_shards": 2,
        "dj_training_runs": 1,
        "dj_model_artifacts": 1,
        "dj_predictions": 6,
        "dj_skill_policies": 1,
        "dj_plan_traces": 1,
        "dj_guard_decisions": 5,
        "dj_surprise_events": {"allowed_min": 0, "allowed_max": 1, "happy_path_expected": 0},
        "dj_verification_runs": 1,
        "dj_audit_log": {"minimum": 30},
    }
    grid_final_counts = {
        "dj_domain_packs": 1,
        "dj_domain_pack_by_nv": 1,
        "dj_instrument_registry": grid_domain["instrument_count"],
        "dj_adapter_registry": 1,
        "dj_raw_events": 8000,
        "dj_adapter_runs": 8000,
        "dj_normalized_states": 8000,
        "dj_actions": 8005,
        "dj_outcomes": 8000,
        "dj_transitions": 8000,
        "dj_instrument_readings": 8000 * grid_domain["instrument_count"],
        "dj_latent_panels": 8000,
        "dj_bindings": 0,
        "dj_bindings_by_entity": 0,
        "dj_trajectories": 1000,
        "dj_dataset_shards": 3,
        "dj_training_runs": 1,
        "dj_model_artifacts": 1,
        "dj_predictions": 6,
        "dj_skill_policies": 1,
        "dj_plan_traces": 1,
        "dj_guard_decisions": 5,
        "dj_surprise_events": 0,
        "dj_verification_runs": 1,
        "dj_audit_log": {"minimum": 8000},
    }

    assert_equal("validate_expected_counts", "counter CF coverage", sorted(DJ_CF_NAMES), sorted(counter_final_counts), "counter final count table must cover every DynamicJEPA CF")
    assert_equal("validate_expected_counts", "grid CF coverage", sorted(DJ_CF_NAMES), sorted(grid_final_counts), "grid final count table must cover every DynamicJEPA CF")

    protocol_text = PROTOCOL_PATH.read_text(encoding="utf-8")
    missing = [snippet for snippet in PROTOCOL_SNIPPETS if snippet not in protocol_text]
    if missing:
        fail("validate_protocol", str(PROTOCOL_PATH), "authoritative count snippets present", missing, "align Phase 0 fixtures with 14_full_state_verification_protocol.md")

    return {
        "source_of_truth_protocol": str(PROTOCOL_PATH.relative_to(ROOT)),
        "counter_world_final_counts_from_protocol": counter_final_counts,
        "gridworld_5x5_final_counts_from_protocol": grid_final_counts,
        "gridworld_dataset_row_expectations": {"total": total_grid_rows, "nominal": expected_nominal, "accepted_ranges": ranges},
    }


def snapshot_files() -> dict[str, Any]:
    snapshot = {}
    for rel, expected_hash in REQUIRED_PHASE0_FILES.items():
        path = ROOT / rel
        if not path.exists():
            fail("snapshot_phase0_files", rel, "file exists", "missing", "create the required Phase 0 fixture file")
        actual_hash = sha256_file(path)
        assert_equal("snapshot_phase0_files", rel, expected_hash, actual_hash, "restore the locked Phase 0 fixture bytes or intentionally update the verifier hash")
        snapshot[rel] = {
            "size_bytes": path.stat().st_size,
            "sha256": actual_hash,
            "line_count": len(path.read_bytes().splitlines()) if path.suffix in {".jsonl", ".toml", ".json"} else None,
        }
    return snapshot


def verify_phase0(evidence_path: Path | None = None) -> dict[str, Any]:
    before_snapshot = snapshot_files()

    counter_domain = validate_domain_pack(DOMAIN_ROOT / "counter_world.v1.toml", "counter_world")
    grid_domain = validate_domain_pack(DOMAIN_ROOT / "gridworld_5x5.v1.toml", "gridworld_5x5")
    counter_fixtures = validate_counter_fixtures()
    grid_fixtures = validate_gridworld_fixtures()
    train_configs = validate_train_configs()
    count_expectations = validate_expected_counts(counter_domain, grid_domain)

    after_snapshot = snapshot_files()
    assert_equal("verify_phase0", "file state before/after", before_snapshot, after_snapshot, "Phase 0 verification must be a read-only inspection")

    evidence = {
        "operation": "5090jepa_phase0_verify",
        "status": "ok",
        "trigger_event": "operator runs python3 scripts/5090jepa/verify_phase0.py",
        "source_of_truth": {
            "primary": "configs/dynamicjepa files on disk",
            "secondary": str(PROTOCOL_PATH.relative_to(ROOT)),
        },
        "before_state": before_snapshot,
        "after_state": after_snapshot,
        "decoded_records": {
            "domain_packs": {"counter_world": counter_domain, "gridworld_5x5": grid_domain},
            "fixtures": {"counter_world": counter_fixtures, "gridworld_5x5": grid_fixtures},
            "train_configs": train_configs,
            "expected_count_contracts": count_expectations,
        },
        "edge_case_audit": {
            "counter_world": {
                "empty_input": {"before_state": before_snapshot["configs/dynamicjepa/verification/counter_world_edge_empty_input.jsonl"], "expected_storage_delta": "all DynamicJEPA CFs unchanged except FSV audit row once Phase 10 exists", "after_state": after_snapshot["configs/dynamicjepa/verification/counter_world_edge_empty_input.jsonl"]},
                "invalid_action": {
                    "before_state": before_snapshot["configs/dynamicjepa/verification/counter_world_edge_invalid_action.jsonl"],
                    "decoded_edge_contract": counter_fixtures["edge_invalid_action_expected_failure"],
                    "after_state": after_snapshot["configs/dynamicjepa/verification/counter_world_edge_invalid_action.jsonl"],
                },
                "max_panel": {
                    "before_state": before_snapshot["configs/dynamicjepa/verification/counter_world_edge_max_panel.jsonl"],
                    "decoded_edge_contract": counter_fixtures["edge_max_panel_expected_failure"],
                    "after_state": after_snapshot["configs/dynamicjepa/verification/counter_world_edge_max_panel.jsonl"],
                },
            },
            "gridworld_5x5": {
                "empty_input": {"before_state": before_snapshot["configs/dynamicjepa/verification/gridworld_5x5_edge_empty.jsonl"], "expected_storage_delta": "all DynamicJEPA CFs unchanged except FSV audit row once Phase 10 exists", "after_state": after_snapshot["configs/dynamicjepa/verification/gridworld_5x5_edge_empty.jsonl"]},
                "invalid_action": {
                    "before_state": before_snapshot["configs/dynamicjepa/verification/gridworld_5x5_edge_invalid_action.jsonl"],
                    "decoded_edge_contract": grid_fixtures["edge_invalid_action_expected_failure"],
                    "after_state": after_snapshot["configs/dynamicjepa/verification/gridworld_5x5_edge_invalid_action.jsonl"],
                },
                "oob_state": {
                    "before_state": before_snapshot["configs/dynamicjepa/verification/gridworld_5x5_edge_oob_state.jsonl"],
                    "decoded_edge_contract": grid_fixtures["edge_oob_state_expected_failure"],
                    "after_state": after_snapshot["configs/dynamicjepa/verification/gridworld_5x5_edge_oob_state.jsonl"],
                },
            },
        },
        "dynamicjepa_runtime_claim": "none; Phase 0 only locks contracts and fixtures",
    }

    if evidence_path is not None:
        evidence_path.parent.mkdir(parents=True, exist_ok=True)
        evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return evidence


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Verify 5090jepa Phase 0 fixtures and count contracts")
    parser.add_argument("--evidence", type=Path, help="optional path for JSON evidence output")
    args = parser.parse_args(argv)

    try:
        evidence = verify_phase0(args.evidence)
    except Phase0VerificationError as exc:
        print(json.dumps(exc.to_json(), indent=2, sort_keys=True), file=sys.stderr)
        return 1
    except Exception as exc:
        print(
            json.dumps(
                {
                    "operation": "5090jepa_phase0_verify",
                    "status": "error",
                    "error_code": "UNHANDLED_PHASE0_VERIFICATION_ERROR",
                    "error_message": repr(exc),
                    "remediation": "fix the root exception; do not bypass Phase 0 verification",
                },
                indent=2,
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 1

    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
