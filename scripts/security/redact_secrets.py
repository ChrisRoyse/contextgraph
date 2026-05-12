#!/usr/bin/env python3
"""Fail-closed redaction for hook/MCP text before agent-visible rendering."""

from __future__ import annotations

import re
import sys


PATTERNS = [
    re.compile(r"(?i)(api[_-]?key|token|secret|password)\s*[:=]\s*['\"]?([A-Za-z0-9_\-./+=]{12,})['\"]?"),
    re.compile(r"\b(sk-[A-Za-z0-9]{20,})\b"),
    re.compile(r"\b(ghp_[A-Za-z0-9]{20,})\b"),
    re.compile(r"\b(xox[baprs]-[A-Za-z0-9-]{20,})\b"),
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----", re.DOTALL),
]


def redact(text: str) -> str:
    out = text
    for pattern in PATTERNS:
        out = pattern.sub(lambda match: match.group(0).split(match.group(2))[0] + "[REDACTED]" if match.lastindex and match.lastindex >= 2 else "[REDACTED_SECRET]", out)
    return out


def main() -> int:
    sys.stdout.write(redact(sys.stdin.read()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
