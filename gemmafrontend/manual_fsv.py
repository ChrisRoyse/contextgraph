#!/usr/bin/env python3
"""Manual FSV for the Gemma frontend.

This test uses a completed real BlackBoxFix/Gemma run root. It does not create
mock data and does not call the expensive live Gemma path.
"""
from __future__ import annotations

import argparse
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


APP_ROOT = Path(__file__).resolve().parent
REPO_ROOT = APP_ROOT.parent
MAX_PROMPT_CHARS = 4096


def log(lines: list[str], message: str) -> None:
    print(message)
    lines.append(message)


def request_json(url: str, body: dict[str, Any] | None = None, *, expect_status: int = 200) -> tuple[int, dict[str, Any]]:
    data = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json", "Accept": "application/json"},
        method="GET" if body is None else "POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=30.0) as response:
            status = response.status
            raw = response.read()
    except urllib.error.HTTPError as err:
        status = err.code
        raw = err.read()
    value = json.loads(raw.decode("utf-8"))
    if status != expect_status:
        raise AssertionError(f"expected HTTP {expect_status}, got {status}: {value}")
    return status, value


def direct_ledger_counts(ledger: Path) -> dict[str, int]:
    conn = sqlite3.connect(ledger)
    try:
        return {
            str(row[0]): int(row[1])
            for row in conn.execute(
                "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind"
            )
        }
    finally:
        conn.close()


def runtime_state(runtime_root: Path) -> dict[str, Any]:
    index = runtime_root / "frontend-run-index.jsonl"
    snapshots = list((runtime_root / "frontend-runs").glob("*/system-snapshot.json"))
    lines = index.read_text().splitlines() if index.exists() else []
    return {
        "runtime_root": str(runtime_root),
        "index_exists": index.exists(),
        "index_rows": len([line for line in lines if line.strip()]),
        "snapshot_files": len(snapshots),
        "source_of_truth": f"file:{index}",
    }


def wait_for_server(url: str) -> None:
    deadline = time.time() + 15
    last: Exception | None = None
    while time.time() < deadline:
        try:
            request_json(url + "/api/health")
            return
        except Exception as err:  # noqa: BLE001
            last = err
            time.sleep(0.25)
    raise RuntimeError(f"server did not become ready: {last}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-root", type=Path, default=Path("/tmp/blackboxfix-agentops-demo-manual-03"))
    parser.add_argument("--port", type=int, default=8798)
    args = parser.parse_args()

    run_root = args.run_root.resolve()
    if not (run_root / ".blackboxfix_agentops_demo_root").exists():
        raise SystemExit(f"real run root marker missing: {run_root}")

    runtime_root = Path(tempfile.mkdtemp(prefix="gemmafrontend-fsv-"))
    lines: list[str] = []
    server = subprocess.Popen(
        [
            sys.executable,
            str(APP_ROOT / "server.py"),
            "--host",
            "127.0.0.1",
            "--port",
            str(args.port),
            "--runtime-root",
            str(runtime_root),
        ],
        cwd=str(REPO_ROOT),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env={**os.environ, "PYTHONUNBUFFERED": "1"},
    )
    base = f"http://127.0.0.1:{args.port}"
    try:
        wait_for_server(base)
        log(lines, f"SOURCE_OF_TRUTH_FRONTEND=file:{runtime_root}/frontend-run-index.jsonl")
        log(lines, f"SOURCE_OF_TRUTH_LEDGER=sqlite:{run_root}/blackbox/blackbox.sqlite#ledger_records")

        before = runtime_state(runtime_root)
        log(lines, "HAPPY_BEFORE=" + json.dumps(before, sort_keys=True))
        _, snapshot = request_json(base + "/api/inspect", {"root": str(run_root), "compile_reward": False})
        after = runtime_state(runtime_root)
        log(lines, "HAPPY_AFTER=" + json.dumps(after, sort_keys=True))
        direct_counts = direct_ledger_counts(run_root / "blackbox/blackbox.sqlite")
        if snapshot["ledger"]["physical_counts"] != direct_counts:
            raise AssertionError("snapshot ledger counts did not match direct SQLite readback")
        if after["index_rows"] != before["index_rows"] + 1:
            raise AssertionError("frontend index row was not persisted")
        index_path = runtime_root / "frontend-run-index.jsonl"
        latest_index = json.loads(index_path.read_text().splitlines()[-1])
        persisted_snapshot = json.loads(Path(latest_index["snapshot_path"]).read_text())
        if persisted_snapshot["identity"]["blackbox_run_id"] != snapshot["identity"]["blackbox_run_id"]:
            raise AssertionError("persisted snapshot did not match HTTP snapshot")
        log(lines, "HAPPY_LEDGER_COUNTS=" + json.dumps(direct_counts, sort_keys=True))
        log(lines, "HAPPY_PERSISTED_SNAPSHOT=" + latest_index["snapshot_path"])

        edge_cases = [
            (
                "EDGE_EMPTY_PROMPT",
                "/api/chat",
                {"message": ""},
                "GEMMAFRONTEND_PROMPT_EMPTY",
            ),
            (
                "EDGE_MISSING_ROOT",
                "/api/inspect",
                {"root": f"/tmp/gemmafrontend-missing-{int(time.time())}"},
                "GEMMAFRONTEND_RUN_ROOT_MISSING",
            ),
            (
                "EDGE_TOO_LONG_PROMPT",
                "/api/chat",
                {"message": "x" * (MAX_PROMPT_CHARS + 1)},
                "GEMMAFRONTEND_PROMPT_TOO_LONG",
            ),
        ]
        for name, path, payload, expected_code in edge_cases:
            before_edge = runtime_state(runtime_root)
            log(lines, f"{name}_BEFORE=" + json.dumps(before_edge, sort_keys=True))
            _, value = request_json(base + path, payload, expect_status=400)
            after_edge = runtime_state(runtime_root)
            log(lines, f"{name}_AFTER=" + json.dumps(after_edge, sort_keys=True))
            if value.get("error_code") != expected_code:
                raise AssertionError(f"{name} expected {expected_code}, got {value}")
            if before_edge != after_edge:
                raise AssertionError(f"{name} mutated frontend source of truth")
            log(lines, f"{name}_ERROR={value['error_code']}")

        final = runtime_state(runtime_root)
        log(lines, "FINAL_STATE=" + json.dumps(final, sort_keys=True))
        log_path = runtime_root / "manual-fsv.log"
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"MANUAL_FSV_LOG={log_path}")
        return 0
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
