#!/usr/bin/env python3
"""CLI entrypoint for SWE-bench Lite operational checks."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from swebench_lite_ops.cli import main


if __name__ == "__main__":
    sys.exit(main())
