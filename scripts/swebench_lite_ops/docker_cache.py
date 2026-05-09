from __future__ import annotations

import json
from collections import defaultdict
from typing import Any

from .core import official_image_ref, run


def docker_overview(timeout: int) -> dict[str, Any]:
    version = run(["docker", "version"], timeout=timeout)
    info = run(["docker", "info", "--format", "{{json .}}"], timeout=timeout, stdout_limit=500000)
    parsed = json.loads(info.stdout_tail)
    ps = run(["docker", "ps", "-a", "--format", "{{json .}}"], timeout=timeout, stdout_limit=200000)
    containers = [json.loads(line) for line in ps.stdout_tail.splitlines() if line.strip()]
    return {
        "version": version.to_json(),
        "info": {
            "server_version": parsed.get("ServerVersion"),
            "operating_system": parsed.get("OperatingSystem"),
            "architecture": parsed.get("Architecture"),
            "ncpu": parsed.get("NCPU"),
            "mem_total": parsed.get("MemTotal"),
            "docker_root_dir": parsed.get("DockerRootDir"),
        },
        "containers": containers,
    }


def local_images() -> list[dict[str, str]]:
    result = run(
        ["docker", "images", "--format", "{{json .}}"],
        timeout=120,
        stdout_limit=500000,
    )
    rows: list[dict[str, str]] = []
    for line in result.stdout_tail.splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def image_ref(row: dict[str, str]) -> str:
    return f"{row['Repository']}:{row['Tag']}"


def is_swebench_image(row: dict[str, str]) -> bool:
    repo = row.get("Repository", "")
    return repo.startswith("swebench/") or repo.startswith("sweb.")


def image_ref_to_instance_id(ref: str, namespace: str, arch: str) -> str | None:
    name = ref.rsplit(":", 1)[0]
    remote_prefix = f"{namespace}/sweb.eval.{arch}."
    local_prefix = f"sweb.eval.{arch}."
    if name.startswith(remote_prefix):
        return name[len(remote_prefix) :].replace("_1776_", "__")
    if name.startswith(local_prefix):
        return name[len(local_prefix) :]
    return None


def inspect_image(ref: str, timeout: int = 120) -> dict[str, Any]:
    result = run(["docker", "image", "inspect", ref], timeout=timeout, stdout_limit=200000)
    payload = json.loads(result.stdout_tail)
    image = payload[0] if payload else {}
    return {
        "id": image.get("Id"),
        "repo_tags": image.get("RepoTags", []),
        "size_bytes": image.get("Size"),
        "created": image.get("Created"),
    }


def cache_status(
    expected_instance_ids: list[str],
    namespace: str,
    arch: str,
    tag: str,
) -> dict[str, Any]:
    rows = local_images()
    all_refs = {image_ref(row): row for row in rows}
    swebench_rows = [row for row in rows if is_swebench_image(row)]
    swebench_refs = [image_ref(row) for row in swebench_rows]
    expected_refs = [official_image_ref(task_id, namespace, arch, tag) for task_id in expected_instance_ids]
    expected_ref_set = set(expected_refs)
    present_expected = [ref for ref in expected_refs if ref in all_refs]
    missing_expected = [ref for ref in expected_refs if ref not in all_refs]
    unexpected_refs = sorted(ref for ref in swebench_refs if ref not in expected_ref_set)

    refs_by_instance: dict[str, list[str]] = defaultdict(list)
    for ref in swebench_refs:
        instance_id = image_ref_to_instance_id(ref, namespace, arch)
        if instance_id:
            refs_by_instance[instance_id].append(ref)
    duplicate_instances = {
        instance_id: sorted(refs)
        for instance_id, refs in refs_by_instance.items()
        if len(refs) > 1
    }

    refs_by_id: dict[str, list[str]] = defaultdict(list)
    for row in swebench_rows:
        refs_by_id[row.get("ID", "")].append(image_ref(row))
    duplicate_image_ids = {
        image_id: sorted(refs)
        for image_id, refs in refs_by_id.items()
        if image_id and len(refs) > 1
    }

    return {
        "expected_local_instance_ids": expected_instance_ids,
        "expected_refs": expected_refs,
        "present_expected_refs": present_expected,
        "missing_expected_refs": missing_expected,
        "unexpected_swebench_refs": unexpected_refs,
        "duplicate_instances": duplicate_instances,
        "duplicate_image_ids": duplicate_image_ids,
        "swebench_image_count": len(swebench_refs),
        "swebench_images": {
            image_ref(row): {"id": row.get("ID"), "size": row.get("Size")}
            for row in swebench_rows
        },
        "passes_cache_hygiene": (
            not missing_expected
            and not unexpected_refs
            and not duplicate_instances
            and not duplicate_image_ids
        ),
    }
