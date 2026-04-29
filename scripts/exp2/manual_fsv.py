#!/usr/bin/env python3
"""Manual happy-path and edge-case FSV through real MCP/RocksDB state."""

from __future__ import annotations

import argparse
import uuid
from pathlib import Path
import sys
from typing import Any, Callable

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import Exp2Error, load_config, output_dir, repo_path, setup_logging, utc_now, write_json
from scripts.exp2.mcp_client import McpClient


HAPPY_INPUTS = [
    "EXP2 manual FSV alpha: cache eviction pressure causes RocksDB read latency to increase, which triggers retry storms in downstream search workers.",
    "EXP2 manual FSV beta: a Rust async function validates input, stores a typed edge, and returns a UUID after the source-of-truth row is read back.",
    "EXP2 manual FSV gamma: multilingual retrieval compares English, Spanish, and French descriptions of the same memory graph update.",
]


def snapshot(client: McpClient) -> dict[str, Any]:
    status = client.call_tool("get_memetic_status", {})
    training = client.call_tool("count_training_records", {})
    return {
        "total_fingerprints": status["fingerprintCount"] if "fingerprintCount" in status else status.get("total_fingerprints"),
        "embedder_count": status["embedderCount"] if "embedderCount" in status else status.get("embedder_count"),
        "training_records": training.get("count"),
    }


def expect_error(action: Callable[[], Any]) -> str:
    try:
        action()
    except Exception as exc:
        return str(exc)
    raise Exp2Error("Expected operation to fail, but it succeeded")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    log = setup_logging(out)
    session_id = f"exp2-manual-fsv-{uuid.uuid4()}"
    storage = out / f"manual_fsv_rocksdb_{session_id}"
    report: dict[str, Any] = {"started_at": utc_now(), "source_of_truth": ["CF_FINGERPRINTS", "CF_TRAINING_RECORDS"], "tests": []}

    with McpClient(cfg["run"]["mcp_binary"], storage, cfg["run"]["models_path"], timeout=240) as client:
        before = snapshot(client)
        print("HAPPY BEFORE:", before)
        store = client.call_tool(
            "store_memories",
            {
                "sessionId": session_id,
                "operatorId": cfg["operator"],
                "items": [{"content": f"{text} Run id: {session_id}.", "rationale": "EXP-2 manual FSV", "importance": 0.5} for text in HAPPY_INPUTS],
            },
            timeout=1800,
        )
        if store.get("storedCount") != len(HAPPY_INPUTS) or store.get("embedderCount") != 14:
            raise Exp2Error(f"happy store_memories unexpected response: {store}")
        ids = [item["fingerprintId"] for item in store["items"]]
        fingerprints = [client.call_tool("get_memory_fingerprint", {"memoryId": mid, "includeVectorNorms": True}) for mid in ids]
        for fp in fingerprints:
            if fp.get("embeddersPresent") != 14:
                raise Exp2Error(f"manual FSV fingerprint missing embedders: {fp}")
        export = client.call_tool(
            "export_training_corpus",
            {
                "filter": "session",
                "filterId": session_id,
                "maxMemories": len(HAPPY_INPUTS),
                "includeEmbeddings": True,
                "includeSparseVectors": True,
                "includeTokenEmbeddings": True,
                "includeEdges": False,
                "includeCausalLabels": False,
                "clearExisting": True,
            },
            timeout=1800,
        )
        if export.get("exported") != len(HAPPY_INPUTS) or export.get("errors") != 0:
            raise Exp2Error(f"happy export_training_corpus unexpected response: {export}")
        records = [
            client.call_tool("get_training_record", {"memoryId": mid, "includeVectors": True, "includeTokenEmbeddings": True, "includeEdges": False})
            for mid in ids
        ]
        for rec in records:
            shape = rec.get("shape", {})
            expected = {"e1_len": 1024, "e7_len": 1536, "e14_len": 1024}
            for key, want in expected.items():
                if shape.get(key) != want:
                    raise Exp2Error(f"training record shape {key}={shape.get(key)}, expected {want}")
            if shape.get("e6_sparse_nnz", 0) <= 0 or shape.get("e12_token_count", 0) <= 0:
                raise Exp2Error(f"sparse/token record proof missing: {shape}")
        after = snapshot(client)
        print("HAPPY AFTER:", after)
        report["tests"].append({"name": "happy_path_store_export_readback", "before": before, "after": after, "memory_ids": ids, "export": export, "record_shapes": [r["shape"] for r in records]})

        edge_cases = [
            ("empty_batch", lambda: client.call_tool("store_memories", {"items": []})),
            ("whitespace_content", lambda: client.call_tool("store_memories", {"items": [{"content": "   "}]})),
            ("invalid_training_uuid", lambda: client.call_tool("get_training_record", {"memoryId": "not-a-uuid"})),
        ]
        for name, action in edge_cases:
            edge_before = snapshot(client)
            print(f"EDGE {name} BEFORE:", edge_before)
            error = expect_error(action)
            edge_after = snapshot(client)
            print(f"EDGE {name} AFTER:", edge_after)
            if edge_before != edge_after:
                raise Exp2Error(f"edge case {name} mutated source-of-truth state: before={edge_before}, after={edge_after}")
            report["tests"].append({"name": name, "before": edge_before, "after": edge_after, "expected_error": error})

    report["finished_at"] = utc_now()
    write_json(out / "manual_fsv_log.json", report)
    log.info("Manual FSV passed: %s", out / "manual_fsv_log.json")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 manual FSV failed: {exc}") from exc
