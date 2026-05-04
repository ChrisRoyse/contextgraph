#!/usr/bin/env python3
"""Manual FSV for the ContextGraph model frontend.

This test uses a completed real BlackBoxFix/Gemma run root. It does not create
mock data and does not call the expensive live Gemma path.
"""
from __future__ import annotations

import argparse
import hashlib
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


def category_state(runtime_root: Path) -> dict[str, Any]:
    category_root = runtime_root / "product-category"
    packs = list(category_root.glob("*/product-category-pack.json")) if category_root.exists() else []
    return {
        "category_root": str(category_root),
        "pack_files": len(packs),
        "source_of_truth": f"file:{category_root}",
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


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
        log(lines, "SOURCE_OF_TRUTH_EVENTS=process:gemmafrontend.server.EVENT_LOG")

        other_client = "fsv-other-client"
        other_request = "fsv-other-request"
        _, stale_error = request_json(
            base + "/api/inspect",
            {
                "root": f"/tmp/gemmafrontend-missing-stale-{int(time.time())}",
                "client_id": other_client,
                "request_id": other_request,
            },
            expect_status=400,
        )
        if stale_error.get("error_code") != "GEMMAFRONTEND_RUN_ROOT_MISSING":
            raise AssertionError(f"expected stale setup error, got {stale_error}")
        _, other_events = request_json(
            base + f"/api/events/recent?client_id={other_client}&request_id={other_request}"
        )
        if not any(event.get("level") == "error" for event in other_events["events"]):
            raise AssertionError("stale setup error was not recorded in the other client's event log")
        log(lines, "STALE_OTHER_CLIENT_ERROR=" + stale_error["error_code"])

        before = runtime_state(runtime_root)
        happy_client = "fsv-happy-client"
        happy_request = "fsv-happy-request"
        log(lines, "HAPPY_BEFORE=" + json.dumps(before, sort_keys=True))
        _, snapshot = request_json(
            base + "/api/inspect",
            {
                "root": str(run_root),
                "compile_reward": False,
                "client_id": happy_client,
                "request_id": happy_request,
            },
        )
        after = runtime_state(runtime_root)
        log(lines, "HAPPY_AFTER=" + json.dumps(after, sort_keys=True))
        direct_counts = direct_ledger_counts(run_root / "blackbox/blackbox.sqlite")
        if snapshot["ledger"]["physical_counts"] != direct_counts:
            raise AssertionError("snapshot ledger counts did not match direct SQLite readback")
        interaction = snapshot.get("model_interaction") or {}
        tool_contract = interaction.get("tool_contract") or {}
        if tool_contract.get("mode") not in {"single_allowlisted_mutating_tool", "multi_tool_contextgraph_gateway"}:
            raise AssertionError(f"unexpected tool contract mode: {tool_contract}")
        if not tool_contract.get("available_tool_names"):
            raise AssertionError(f"missing tool names in tool contract: {tool_contract}")
        if not interaction.get("truth_statement"):
            raise AssertionError("model interaction truth statement missing")
        if after["index_rows"] != before["index_rows"] + 1:
            raise AssertionError("frontend index row was not persisted")
        if after["snapshot_files"] != before["snapshot_files"] + 1:
            raise AssertionError("frontend snapshot file was not physically created")
        index_path = runtime_root / "frontend-run-index.jsonl"
        latest_index = json.loads(index_path.read_text().splitlines()[-1])
        persisted_snapshot = json.loads(Path(latest_index["snapshot_path"]).read_text())
        if persisted_snapshot["identity"]["blackbox_run_id"] != snapshot["identity"]["blackbox_run_id"]:
            raise AssertionError("persisted snapshot did not match HTTP snapshot")
        log(lines, "HAPPY_LEDGER_COUNTS=" + json.dumps(direct_counts, sort_keys=True))
        log(lines, "HAPPY_PERSISTED_SNAPSHOT=" + latest_index["snapshot_path"])
        log(lines, "HAPPY_TOOL_CONTRACT=" + json.dumps(tool_contract, sort_keys=True))
        _, happy_events = request_json(
            base + f"/api/events/recent?client_id={happy_client}&request_id={happy_request}"
        )
        if any(event.get("level") == "error" for event in happy_events["events"]):
            raise AssertionError("current happy request event log contains an unrelated error")
        if any(event.get("client_id") == other_client for event in happy_events["events"]):
            raise AssertionError("other client's stale error leaked into current request events")
        log(lines, "HAPPY_EVENT_COUNT=" + str(len(happy_events["events"])))

        agent_client = "fsv-agent-client"
        agent_request = "fsv-agent-request"
        agent_before = runtime_state(runtime_root)
        log(lines, "AGENT_INSPECT_BEFORE=" + json.dumps(agent_before, sort_keys=True))
        _, agent_snapshot = request_json(
            base + "/api/chat",
            {
                "message": "Use the ContextGraph tool inspect_latest_run to inspect the latest real run and state what the physical evidence proves.",
                "client_id": agent_client,
                "request_id": agent_request,
            },
        )
        agent_after = runtime_state(runtime_root)
        log(lines, "AGENT_INSPECT_AFTER=" + json.dumps(agent_after, sort_keys=True))
        chat_agent = agent_snapshot.get("chat_agent") or {}
        selected_tool = (chat_agent.get("selected_tool") or {}).get("name")
        if selected_tool != "inspect_latest_run":
            raise AssertionError(f"Gemma selected unexpected tool: {chat_agent}")
        agent_ledger = Path(agent_snapshot["ledger"]["path"])
        agent_counts = direct_ledger_counts(agent_ledger)
        if agent_snapshot["ledger"]["physical_counts"] != agent_counts:
            raise AssertionError("agent chat snapshot ledger counts did not match direct SQLite readback")
        agent_index = json.loads((runtime_root / "frontend-run-index.jsonl").read_text().splitlines()[-1])
        agent_persisted = json.loads(Path(agent_index["snapshot_path"]).read_text())
        persisted_tool = (agent_persisted.get("chat_agent") or {}).get("selected_tool", {}).get("name")
        if persisted_tool != "inspect_latest_run":
            raise AssertionError(f"persisted agent snapshot missing selected tool: {persisted_tool}")
        if agent_after["index_rows"] != agent_before["index_rows"] + 1:
            raise AssertionError("agent inspect did not persist exactly one frontend snapshot row")
        if agent_after["snapshot_files"] != agent_before["snapshot_files"] + 1:
            raise AssertionError("agent inspect did not create a distinct physical snapshot file")
        log(lines, "AGENT_INSPECT_TOOL=" + selected_tool)
        log(lines, "AGENT_INSPECT_LEDGER_COUNTS=" + json.dumps(agent_counts, sort_keys=True))
        log(lines, "AGENT_INSPECT_PERSISTED_SNAPSHOT=" + agent_index["snapshot_path"])
        _, agent_events = request_json(
            base + f"/api/events/recent?client_id={agent_client}&request_id={agent_request}"
        )
        if any(event.get("level") == "error" for event in agent_events["events"]):
            raise AssertionError("agent inspect event log contains an error")
        log(lines, "AGENT_INSPECT_EVENT_COUNT=" + str(len(agent_events["events"])))

        category_client = "fsv-category-client"
        category_request = "fsv-category-request"
        category_before = category_state(runtime_root)
        log(lines, "CATEGORY_BEFORE=" + json.dumps(category_before, sort_keys=True))
        _, category_pack = request_json(
            base + "/api/category/build",
            {
                "audience": "manual FSV investor category proof",
                "max_runs": 3,
                "client_id": category_client,
                "request_id": category_request,
            },
        )
        category_after = category_state(runtime_root)
        log(lines, "CATEGORY_AFTER=" + json.dumps(category_after, sort_keys=True))
        if category_after["pack_files"] != category_before["pack_files"] + 1:
            raise AssertionError("category build did not physically create exactly one pack file")
        pack_file = Path(category_pack["pack_file"]["path"])
        persisted_pack = json.loads(pack_file.read_text())
        if persisted_pack["pack_id"] != category_pack["pack_id"]:
            raise AssertionError("persisted product category pack id did not match HTTP response")
        if sha256_file(pack_file) != category_pack["pack_file"]["sha256"]:
            raise AssertionError("product category pack SHA-256 did not match direct file hash")
        if not persisted_pack.get("input_runs"):
            raise AssertionError("product category pack did not include real input runs")
        signed = persisted_pack["attestation"]["signed_attestation"]
        tamper = persisted_pack["attestation"]["tamper_test"]
        if signed.get("status") != "verified":
            raise AssertionError(f"signed attestation did not verify: {signed}")
        if tamper.get("status") != "tamper_detected":
            raise AssertionError(f"tamper test was not detected: {tamper}")
        if int(tamper.get("verify_command", {}).get("exit_code", 0)) == 0:
            raise AssertionError("tampered attestation manifest unexpectedly verified")
        for name, artifact in persisted_pack.get("artifacts", {}).items():
            artifact_path = Path(artifact["path"])
            if not artifact_path.exists():
                raise AssertionError(f"category artifact missing: {name} {artifact_path}")
            if artifact.get("sha256") and sha256_file(artifact_path) != artifact["sha256"]:
                raise AssertionError(f"category artifact hash mismatch: {name}")
        log(lines, "CATEGORY_PACK_FILE=" + str(pack_file))
        log(lines, "CATEGORY_PACK_ID=" + persisted_pack["pack_id"])
        log(lines, "CATEGORY_INPUT_RUNS=" + str(len(persisted_pack["input_runs"])))
        log(lines, "CATEGORY_ATTESTATION_STATUS=" + signed["status"])
        log(lines, "CATEGORY_TAMPER_STATUS=" + tamper["status"])
        _, category_events = request_json(
            base + f"/api/events/recent?client_id={category_client}&request_id={category_request}"
        )
        if any(event.get("level") == "error" for event in category_events["events"]):
            raise AssertionError("category build event log contains an error")
        log(lines, "CATEGORY_EVENT_COUNT=" + str(len(category_events["events"])))

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
            (
                "EDGE_CATEGORY_EMPTY_AUDIENCE",
                "/api/category/build",
                {"audience": "", "max_runs": 3},
                "GEMMAFRONTEND_CATEGORY_AUDIENCE_INVALID",
            ),
            (
                "EDGE_CATEGORY_ZERO_RUNS",
                "/api/category/build",
                {"audience": "manual FSV", "max_runs": 0},
                "GEMMAFRONTEND_CATEGORY_MAX_RUNS_INVALID",
            ),
            (
                "EDGE_CATEGORY_TOO_MANY_RUNS",
                "/api/category/build",
                {"audience": "manual FSV", "max_runs": 13},
                "GEMMAFRONTEND_CATEGORY_MAX_RUNS_INVALID",
            ),
        ]
        for name, path, payload, expected_code in edge_cases:
            edge_client = f"fsv-{name.lower()}-client"
            edge_request = f"fsv-{name.lower()}-request"
            payload = {**payload, "client_id": edge_client, "request_id": edge_request}
            before_edge = runtime_state(runtime_root)
            before_category_edge = category_state(runtime_root)
            log(lines, f"{name}_BEFORE=" + json.dumps(before_edge, sort_keys=True))
            log(lines, f"{name}_CATEGORY_BEFORE=" + json.dumps(before_category_edge, sort_keys=True))
            _, value = request_json(base + path, payload, expect_status=400)
            after_edge = runtime_state(runtime_root)
            after_category_edge = category_state(runtime_root)
            log(lines, f"{name}_AFTER=" + json.dumps(after_edge, sort_keys=True))
            log(lines, f"{name}_CATEGORY_AFTER=" + json.dumps(after_category_edge, sort_keys=True))
            if value.get("error_code") != expected_code:
                raise AssertionError(f"{name} expected {expected_code}, got {value}")
            if before_edge != after_edge:
                raise AssertionError(f"{name} mutated frontend source of truth")
            if before_category_edge != after_category_edge:
                raise AssertionError(f"{name} mutated product category source of truth")
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
