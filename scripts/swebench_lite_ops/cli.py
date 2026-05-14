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
from .oracle_events import command_oracle_events
from .python_ast_chunk_command import command_ast_chunks
from .python_ast_parse import command_ast_parse
from .python_diff_chunk_map import command_diff_chunk_map
from .python_static_graph import command_static_graph


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

    events = sub.add_parser("oracle-events", help="Materialize strict typed oracle events from Docker oracle outcomes.")
    events.add_argument("--outcomes-jsonl", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-oracle-baselines-v1/oracle_outcomes.jsonl"))
    events.add_argument("--cleaned-manifest", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-cleaned-manifest-v1/cleaned_manifest.json"))
    events.add_argument("--workspace-manifest", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    events.add_argument("--event-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-oracle-events-v1"))
    events.add_argument("--fsv-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-008-oracle-events-fsv"))
    events.add_argument("--evidence-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-008-oracle-events-fsv"))
    events.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-008-oracle-events"))
    events.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    events.set_defaults(func=command_oracle_events, evidence_name="oracle-events-command-evidence.json")

    ast_parse = sub.add_parser("ast-parse", help="Materialize fail-closed Python AST parse records.")
    ast_parse.add_argument("--workspace-manifest", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    ast_parse.add_argument("--parse-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-ast-parse-v1"))
    ast_parse.add_argument("--fsv-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-009-ast-parse-fsv"))
    ast_parse.add_argument("--evidence-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-009-ast-parse-fsv"))
    ast_parse.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-009-ast-parse"))
    ast_parse.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    ast_parse.set_defaults(func=command_ast_parse, evidence_name="ast-parse-command-evidence.json")

    ast_chunks = sub.add_parser("ast-chunks", help="Materialize stable AST-aware Python code chunks.")
    ast_chunks.add_argument("--parse-records", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-ast-parse-v1/python_ast_parse_records.jsonl"))
    ast_chunks.add_argument("--chunk-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-ast-chunks-v1"))
    ast_chunks.add_argument("--fsv-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-010-ast-chunks-fsv"))
    ast_chunks.add_argument("--evidence-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-010-ast-chunks-fsv"))
    ast_chunks.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-010-ast-chunks"))
    ast_chunks.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    ast_chunks.set_defaults(func=command_ast_chunks, evidence_name="ast-chunks-command-evidence.json")

    diff_map = sub.add_parser("diff-chunk-map", help="Map Python patch/test hunks to stable AST chunks.")
    diff_map.add_argument("--workspace-manifest", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    diff_map.add_argument("--chunks-jsonl", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    diff_map.add_argument("--map-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-diff-chunk-map-v1"))
    diff_map.add_argument("--fsv-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-011-diff-chunk-map-fsv"))
    diff_map.add_argument("--evidence-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-011-diff-chunk-map-fsv"))
    diff_map.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-011-diff-chunk-map"))
    diff_map.set_defaults(func=command_diff_chunk_map, evidence_name="diff-chunk-map-command-evidence.json")

    static_graph = sub.add_parser("static-graph", help="Extract deterministic Python static reality graph features.")
    static_graph.add_argument("--parse-records", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-ast-parse-v1/python_ast_parse_records.jsonl"))
    static_graph.add_argument("--chunks-jsonl", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    static_graph.add_argument("--graph-root", type=Path, default=Path("/mnt/d/contextgraph/corpus/python-static-reality-graph-v1"))
    static_graph.add_argument("--fsv-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-012-static-graph-fsv"))
    static_graph.add_argument("--evidence-root", type=Path, default=Path("/mnt/d/contextgraph/fsv/task-py-data-012-static-graph-fsv"))
    static_graph.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-012-static-graph"))
    static_graph.set_defaults(func=command_static_graph, evidence_name="static-graph-command-evidence.json")
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
