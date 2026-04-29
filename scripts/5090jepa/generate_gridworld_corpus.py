#!/usr/bin/env python3
"""Generate the deterministic gridworld_5x5 Phase 0 corpus."""

import hashlib
import json
import random
import sys
from pathlib import Path

SEED = 0x5090
N_TRAJ = 1000
TRAJ_LEN = 8
GRID = 5
ACTIONS = ["up", "down", "left", "right", "noop"]


def step(x: int, y: int, action: str) -> tuple[int, int]:
    if action == "up":
        return x, max(y - 1, 0)
    if action == "down":
        return x, min(y + 1, GRID - 1)
    if action == "left":
        return max(x - 1, 0), y
    if action == "right":
        return min(x + 1, GRID - 1), y
    if action == "noop":
        return x, y
    raise ValueError(f"undeclared gridworld action: {action}")


def rows():
    rng = random.Random(SEED)
    for trajectory_index in range(N_TRAJ):
        trajectory_id = hashlib.blake2b(
            f"gridworld5x5:{trajectory_index}".encode("utf-8"),
            digest_size=8,
        ).hexdigest()
        x = rng.randrange(GRID)
        y = rng.randrange(GRID)
        for step_index in range(TRAJ_LEN):
            action = rng.choice(ACTIONS)
            next_x, next_y = step(x, y, action)
            yield {
                "trajectory_id": trajectory_id,
                "step": step_index,
                "state": {"x": x, "y": y},
                "action": {"move": action},
                "outcome": {"next_x": next_x, "next_y": next_y},
            }
            x, y = next_x, next_y


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(
            json.dumps(
                {
                    "operation": "generate_gridworld_corpus",
                    "status": "error",
                    "error_code": "ARGUMENT_VALIDATION",
                    "error_message": "expected exactly one output path argument",
                    "remediation": "run scripts/5090jepa/generate_gridworld_corpus.py configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl",
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 2

    out_path = Path(argv[1])
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", encoding="utf-8", newline="\n") as handle:
        for row in rows():
            handle.write(json.dumps(row, separators=(",", ":"), sort_keys=False))
            handle.write("\n")

    print(f"wrote {N_TRAJ * TRAJ_LEN} events to {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
