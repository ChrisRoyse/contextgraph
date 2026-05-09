from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .commands import (
    command_gold_smoke,
    command_image_status,
    command_preflight,
    command_prune_cache,
    command_pull_images,
)
from .core import (
    DEFAULT_DATASET,
    DEFAULT_EVIDENCE_ROOT,
    DEFAULT_SMOKE_TASK,
    DEFAULT_SPLIT,
    DEFAULT_SWEBENCH_ROOT,
    DEFAULT_SWEBENCH_VENV,
    DEFAULT_TASKS_DIR,
    SwebenchOpsError,
    utc_now,
    write_json_checked,
)


def add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--swebench-root", type=Path, default=DEFAULT_SWEBENCH_ROOT)
    parser.add_argument("--venv", type=Path, default=DEFAULT_SWEBENCH_VENV)
    parser.add_argument("--tasks-dir", type=Path, default=DEFAULT_TASKS_DIR)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_EVIDENCE_ROOT)
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--arch", default="x86_64")
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--timeout", type=int, default=600)


def add_expected(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--expected-instance-ids",
        nargs="*",
        default=None,
        help="SWE-bench images allowed in local cache. Defaults to the command's sample task.",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Fail-closed operational checks for ContextGraph's SWE-bench Lite Phase 0 gate.",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    preflight = sub.add_parser("preflight", help="Verify official harness, Docker, dataset, manifests, and cache hygiene.")
    add_common(preflight)
    add_expected(preflight)
    preflight.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    preflight.add_argument(
        "--allow-dirty-cache",
        dest="require_clean_cache",
        action="store_false",
        default=True,
        help="Record dirty cache state instead of failing. Do not use for readiness gates.",
    )
    preflight.set_defaults(func=command_preflight, evidence_name="preflight-evidence.json")

    status = sub.add_parser("image-status", help="Inspect local SWE-bench Docker image cache.")
    add_common(status)
    add_expected(status)
    status.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    status.add_argument("--require-clean", action="store_true")
    status.set_defaults(func=command_image_status, evidence_name="image-status-evidence.json")

    pull = sub.add_parser("pull-images", help="Pull only the explicitly needed official SWE-bench Lite images.")
    add_common(pull)
    pull.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    pull.add_argument("--instance-ids", nargs="*")
    pull.add_argument(
        "--allow-dirty-cache",
        dest="require_clean",
        action="store_false",
        default=True,
        help="Do not fail if unrelated SWE-bench images are present after pull.",
    )
    pull.set_defaults(func=command_pull_images, evidence_name="pull-images-evidence.json")

    prune = sub.add_parser("prune-cache", help="Remove stale SWE-bench images not in the expected set.")
    add_common(prune)
    add_expected(prune)
    prune.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    prune.add_argument("--yes", action="store_true", help="Actually remove images. Without this, only writes a dry-run plan.")
    prune.set_defaults(func=command_prune_cache, evidence_name="prune-cache-evidence.json")

    smoke = sub.add_parser("gold-smoke", help="Run one official SWE-bench Lite gold evaluation through Docker.")
    add_common(smoke)
    smoke.add_argument("--instance-id", default=DEFAULT_SMOKE_TASK)
    smoke.add_argument("--run-id")
    smoke.add_argument("--instance-timeout", type=int, default=1800)
    smoke.set_defaults(func=command_gold_smoke, evidence_name="gold-smoke-evidence.json")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    evidence_path = args.evidence_root / args.evidence_name
    try:
        payload = args.func(args)
        payload["evidence_path"] = str(evidence_path)
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    except SwebenchOpsError as exc:
        payload = {
            "checked_at": utc_now(),
            "command": args.command,
            "evidence_path": str(evidence_path),
            **exc.to_json(),
        }
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 2
    except Exception as exc:
        payload = {
            "checked_at": utc_now(),
            "command": args.command,
            "evidence_path": str(evidence_path),
            "ok": False,
            "error_code": "SWEBENCH_UNEXPECTED_ERROR",
            "message": str(exc),
            "remediation": "Fix the script defect or unexpected environment issue before continuing.",
            "details": {"type": type(exc).__name__},
        }
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 3
