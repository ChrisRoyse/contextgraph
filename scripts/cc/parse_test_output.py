#!/usr/bin/env python3
"""Parse Bash tool output into ME-JEPA TestOutcomeEvent JSONL rows."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import time
from typing import Any, Iterable


CONTROL_RE = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")
PYTEST_RE = re.compile(r"^(?P<id>\S+::\S+)\s+(?P<outcome>PASSED|FAILED|SKIPPED|ERROR)\b")
CARGO_RE = re.compile(r"^test\s+(?P<id>\S+)\s+\.\.\.\s+(?P<outcome>ok|FAILED|ignored)\b")
UNITTEST_RE = re.compile(r"^(?P<id>[\w.]+)\s+\([^)]+\)\s+\.\.\.\s+(?P<outcome>ok|FAIL|ERROR|skipped)\b")
JEST_FILE_RE = re.compile(r"^(?P<outcome>PASS|FAIL)\s+(?P<id>\S+)")
JEST_TEST_RE = re.compile(r"^\s*(?:\u2713|\u2714|PASS)\s+(?P<id>.+?)(?:\s+\((?P<ms>\d+)\s*ms\))?$")
JEST_FAIL_TEST_RE = re.compile(r"^\s*(?:\u2715|\u2716|FAIL)\s+(?P<id>.+?)(?:\s+\((?P<ms>\d+)\s*ms\))?$")
SUMMARY_RE = re.compile(
    r"(?P<failed>\d+)\s+failed|(?P<passed>\d+)\s+passed|(?P<skipped>\d+)\s+skipped|(?P<errors>\d+)\s+errors?"
)


def clean(value: str, limit: int = 4096) -> str:
    text = CONTROL_RE.sub(" ", value)
    text = re.sub(r"\s+", " ", text).strip()
    encoded = text.encode("utf-8")
    if len(encoded) <= limit:
        return text
    return encoded[:limit].decode("utf-8", errors="ignore").rstrip() + "..."


def normalize_outcome(value: str) -> str:
    lowered = value.lower()
    if lowered in {"passed", "pass", "ok"}:
        return "pass"
    if lowered in {"failed", "fail", "error", "errors"}:
        return "fail"
    if lowered in {"skipped", "ignored"}:
        return "skipped"
    return "unknown"


def framework_for_command(command: str) -> str:
    lowered = command.lower()
    if "cargo test" in lowered:
        return "cargo_test"
    if "vitest" in lowered:
        return "vitest"
    if "jest" in lowered or "npm test" in lowered or "pnpm test" in lowered:
        return "jest"
    if "unittest" in lowered or "python -m unittest" in lowered:
        return "unittest"
    if "pytest" in lowered:
        return "pytest"
    return "unknown"


def parse_lines(text: str, framework: str) -> Iterable[dict[str, Any]]:
    for line_no, raw_line in enumerate(text.splitlines(), start=1):
        line = raw_line.strip()
        if not line:
            continue
        matches: list[tuple[str, re.Match[str] | None]] = []
        if framework in {"pytest", "unknown"}:
            matches.append(("pytest", PYTEST_RE.match(line)))
        if framework in {"cargo_test", "unknown"}:
            matches.append(("cargo_test", CARGO_RE.match(line)))
        if framework in {"unittest", "unknown"}:
            matches.append(("unittest", UNITTEST_RE.match(line)))
        if framework in {"jest", "vitest", "unknown"}:
            js_framework = "vitest" if framework == "vitest" else "jest"
            matches.append((js_framework, JEST_FILE_RE.match(line)))
            matches.append((js_framework, JEST_TEST_RE.match(line)))
            matches.append((js_framework, JEST_FAIL_TEST_RE.match(line)))
        for matched_framework, match in matches:
            if not match:
                continue
            outcome = normalize_outcome(match.group("outcome") if "outcome" in match.groupdict() else matched_framework)
            if match.re is JEST_TEST_RE:
                outcome = "pass"
            if match.re is JEST_FAIL_TEST_RE:
                outcome = "fail"
            duration_ms = None
            if "ms" in match.groupdict() and match.group("ms"):
                duration_ms = int(match.group("ms"))
            yield {
                "framework": matched_framework,
                "test_id": clean(match.group("id"), 512),
                "outcome": outcome,
                "duration_ms": duration_ms,
                "error_log": clean(raw_line, 4096) if outcome == "fail" else "",
                "line_no": line_no,
            }
            break


def parse_summary(text: str, framework: str) -> list[dict[str, Any]]:
    counts = {"failed": 0, "passed": 0, "skipped": 0, "errors": 0}
    for match in SUMMARY_RE.finditer(text):
        for key in counts:
            if match.group(key):
                counts[key] += int(match.group(key))
    if not any(counts.values()):
        return []
    events = []
    for key, count in counts.items():
        if count == 0:
            continue
        outcome = "fail" if key in {"failed", "errors"} else ("pass" if key == "passed" else "skipped")
        events.append(
            {
                "framework": framework,
                "test_id": f"{framework}::summary::{key}",
                "outcome": outcome,
                "duration_ms": None,
                "error_log": clean(text[-4096:], 4096) if outcome == "fail" else "",
                "line_no": 0,
                "summary_count": count,
            }
        )
    return events


def event_id_for(event: dict[str, Any]) -> str:
    payload = json.dumps(event, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()[:32]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--session-id", required=True)
    parser.add_argument("--tool-use-id", default="")
    parser.add_argument("--command", default="")
    args = parser.parse_args()
    text = sys.stdin.read()
    framework = framework_for_command(args.command)
    events = list(parse_lines(text, framework))
    if not events:
        events = parse_summary(text, framework)
    ts = int(time.time() * 1000)
    for idx, event in enumerate(events):
        event.update(
            {
                "ts": ts,
                "session_id": args.session_id,
                "tool_use_id": args.tool_use_id,
                "command": clean(args.command, 2048),
                "source": "claude_post_tool_use_bash",
                "sequence": idx,
            }
        )
        event["event_id"] = event_id_for(event)
        print(json.dumps(event, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
