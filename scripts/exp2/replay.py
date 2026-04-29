#!/usr/bin/env python3
"""Replay comparison entrypoint for EXP-2."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.fsv import compare_replay
from scripts.exp2.common import load_config, repo_path, write_json


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--reference", required=True)
    parser.add_argument("--replay", required=True)
    parser.add_argument("--output", default="artefacts/exp2/fsv_replay_log.json")
    args = parser.parse_args()
    cfg = load_config(args.config)
    report = compare_replay(repo_path(args.reference), repo_path(args.replay), float(cfg["run"]["replay_tolerance"]))
    write_json(args.output, report)


if __name__ == "__main__":
    main()
