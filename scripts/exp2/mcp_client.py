"""Strict JSON-RPC MCP client used by EXP-2 extraction and FSV."""

from __future__ import annotations

import json
import os
import queue
import subprocess
import threading
import time
from pathlib import Path
from typing import Any

from .common import Exp2Error, repo_path


class McpClient:
    """Small JSON-RPC client for the Context Graph MCP server."""

    def __init__(
        self,
        binary_path: str | Path,
        storage_path: str | Path,
        models_path: str | Path,
        timeout: int = 180,
    ) -> None:
        self.binary_path = repo_path(binary_path)
        self.storage_path = repo_path(storage_path)
        self.models_path = repo_path(models_path)
        self.timeout = timeout
        self.proc: subprocess.Popen[bytes] | None = None
        self.request_id = 0
        self.responses: queue.Queue[dict[str, Any]] = queue.Queue()
        self.stderr_lines: list[str] = []

    def __enter__(self) -> "McpClient":
        self.start()
        return self

    def __exit__(self, *_: object) -> None:
        self.stop()

    def start(self) -> None:
        if not self.binary_path.exists():
            raise Exp2Error(f"MCP binary missing: {self.binary_path}")
        self.storage_path.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        env["CONTEXT_GRAPH_STORAGE_PATH"] = str(self.storage_path)
        env["CONTEXT_GRAPH_MODELS_PATH"] = str(self.models_path)
        env["CONTEXT_GRAPH_WARM_FIRST"] = "1"
        env["RUST_LOG"] = "error"
        self.proc = subprocess.Popen(
            [str(self.binary_path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=repo_path("."),
            env=env,
            bufsize=0,
        )
        threading.Thread(target=self._read_stdout, daemon=True).start()
        threading.Thread(target=self._read_stderr, daemon=True).start()
        resp = self.call_method(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "exp2-harness", "version": "1.0.0"},
            },
            timeout=360,
        )
        if "result" not in resp:
            raise Exp2Error(f"MCP initialize failed: {resp}")
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})

    def stop(self) -> None:
        if self.proc is None:
            return
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
            self.proc.terminate()
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()
        finally:
            self.proc = None

    def call_tool(
        self, name: str, arguments: dict[str, Any] | None = None, timeout: int | None = None
    ) -> dict[str, Any]:
        resp = self.call_method(
            "tools/call",
            {"name": name, "arguments": arguments or {}},
            timeout=timeout or self.timeout,
        )
        if "error" in resp:
            raise Exp2Error(f"MCP tool {name} JSON-RPC error: {resp['error']}")
        result = resp.get("result", {})
        if result.get("isError"):
            text = self._extract_text(result)
            raise Exp2Error(f"MCP tool {name} returned error: {text}")
        text = self._extract_text(result)
        if text is None:
            return result
        try:
            parsed = json.loads(text)
        except json.JSONDecodeError:
            return {"text": text}
        if isinstance(parsed, dict):
            return parsed
        return {"value": parsed}

    def call_method(
        self, method: str, params: dict[str, Any], timeout: int | None = None
    ) -> dict[str, Any]:
        if self.proc is None or self.proc.stdin is None:
            raise Exp2Error("MCP process is not running")
        self.request_id += 1
        rid = self.request_id
        self._send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        deadline = time.time() + (timeout or self.timeout)
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise Exp2Error(
                    "MCP server exited while waiting for response. "
                    f"stderr_tail={self.stderr_lines[-20:]}"
                )
            try:
                resp = self.responses.get(timeout=1)
            except queue.Empty:
                continue
            if resp.get("id") == rid:
                return resp
        raise Exp2Error(
            f"MCP method {method} timed out after {timeout or self.timeout}s. "
            f"stderr_tail={self.stderr_lines[-20:]}"
        )

    def _send(self, msg: dict[str, Any]) -> None:
        if self.proc is None or self.proc.stdin is None:
            raise Exp2Error("MCP process is not running")
        self.proc.stdin.write(json.dumps(msg).encode("utf-8") + b"\n")
        self.proc.stdin.flush()

    def _read_stdout(self) -> None:
        assert self.proc is not None and self.proc.stdout is not None
        for raw in self.proc.stdout:
            line = raw.decode("utf-8", errors="replace").strip()
            if not line:
                continue
            try:
                self.responses.put(json.loads(line))
            except json.JSONDecodeError:
                continue

    def _read_stderr(self) -> None:
        assert self.proc is not None and self.proc.stderr is not None
        for raw in self.proc.stderr:
            line = raw.decode("utf-8", errors="replace").strip()
            if line:
                self.stderr_lines.append(line)
                self.stderr_lines[:] = self.stderr_lines[-200:]

    @staticmethod
    def _extract_text(result: dict[str, Any]) -> str | None:
        content = result.get("content")
        if not isinstance(content, list) or not content:
            return None
        texts = [item.get("text", "") for item in content if item.get("type") == "text"]
        return "\n".join(texts) if texts else None
