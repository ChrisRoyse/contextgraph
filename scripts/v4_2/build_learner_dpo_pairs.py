#!/usr/bin/env python3
"""Build learner-conditioned DPO pairs from exported Learning-as-UTL events.

Input is produced by:

    context-graph-cli learning export-events-jsonl --storage <rocksdb> --out events.jsonl

The script only emits a pair when the same prompt/context group has at least
two real recorded assistant responses with different learner outcomes. It does
not synthesize rejected answers. If no valid contrast exists, it fails.
"""
from __future__ import annotations

import argparse
import json
import logging
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


LOG = logging.getLogger("build_learner_dpo_pairs")


@dataclass(frozen=True)
class EventRow:
    event_id: str
    query: str
    retrieved_context: str
    assistant_response: str
    task_id: str | None
    session_id: str | None
    utility_delta: float
    outcome_label: str
    feature_tensor: list[float]
    feature_summary: dict[str, Any]
    provenance_sha256: str

    @property
    def group_key(self) -> tuple[str | None, str, str]:
        return (self.task_id, self.query.strip(), self.retrieved_context.strip())


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--events", type=Path, required=True,
                   help="Learning event tensor JSONL from context-graph-cli.")
    p.add_argument("--output", type=Path, required=True,
                   help="Output TRL-compatible DPO JSONL.")
    p.add_argument("--min-gap", type=float, default=0.2,
                   help="Minimum chosen.utility_delta - rejected.utility_delta.")
    p.add_argument("--min-chosen-utility", type=float, default=0.05,
                   help="Minimum utility_delta for the chosen response.")
    p.add_argument("--max-rejected-utility", type=float, default=0.0,
                   help="Maximum utility_delta for the rejected response.")
    p.add_argument("--include-feature-tensors", action=argparse.BooleanOptionalAction,
                   default=True,
                   help="Include chosen/rejected feature tensors in pair metadata.")
    p.add_argument("--overwrite", action="store_true",
                   help="Replace an existing output file.")
    p.add_argument("--log-level", default="INFO",
                   choices=["DEBUG", "INFO", "WARNING", "ERROR"])
    return p.parse_args()


def load_events(path: Path) -> list[EventRow]:
    if not path.exists():
        raise FileNotFoundError(f"events file not found: {path}")

    rows: list[EventRow] = []
    with path.open("r", encoding="utf-8") as f:
        for line_no, line in enumerate(f, start=1):
            if not line.strip():
                continue
            obj = json.loads(line)
            if obj.get("record_kind") != "learning_event_tensor":
                raise ValueError(
                    f"{path}:{line_no}: expected record_kind=learning_event_tensor"
                )
            for key in ("event_id", "query", "retrieved_context",
                        "assistant_response", "outcome", "feature_tensor",
                        "feature_summary", "provenance_sha256"):
                if key not in obj:
                    raise ValueError(f"{path}:{line_no}: missing required field {key}")
            tensor = obj["feature_tensor"]
            schema = obj.get("feature_schema") or []
            if not isinstance(tensor, list) or not tensor:
                raise ValueError(f"{path}:{line_no}: feature_tensor must be non-empty")
            if len(schema) != len(tensor):
                raise ValueError(
                    f"{path}:{line_no}: feature_schema len {len(schema)} != "
                    f"feature_tensor len {len(tensor)}"
                )
            if not obj["assistant_response"].strip():
                raise ValueError(f"{path}:{line_no}: assistant_response is empty")
            outcome = obj["outcome"]
            rows.append(EventRow(
                event_id=str(obj["event_id"]),
                query=str(obj["query"]),
                retrieved_context=str(obj["retrieved_context"]),
                assistant_response=str(obj["assistant_response"]),
                task_id=obj.get("task_id"),
                session_id=obj.get("session_id"),
                utility_delta=float(outcome["utility_delta"]),
                outcome_label=str(outcome["label"]),
                feature_tensor=[float(v) for v in tensor],
                feature_summary=dict(obj["feature_summary"]),
                provenance_sha256=str(obj["provenance_sha256"]),
            ))

    if not rows:
        raise ValueError(f"events file contains zero usable rows: {path}")
    return rows


def prompt_for(row: EventRow) -> str:
    query = row.query.strip()
    context = row.retrieved_context.strip()
    if not query:
        raise ValueError(f"event {row.event_id} has empty query")
    if context:
        return f"{query}\n\nContext:\n{context}"
    return query


def build_pairs(
    rows: list[EventRow],
    min_gap: float,
    min_chosen_utility: float,
    max_rejected_utility: float,
    include_feature_tensors: bool,
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    groups: dict[tuple[str | None, str, str], list[EventRow]] = {}
    for row in rows:
        groups.setdefault(row.group_key, []).append(row)

    stats = {
        "events_seen": len(rows),
        "groups_seen": len(groups),
        "groups_too_small": 0,
        "groups_no_gap": 0,
        "groups_threshold_rejected": 0,
        "pairs_emitted": 0,
    }
    pairs: list[dict[str, Any]] = []

    for key, group in sorted(groups.items(), key=lambda item: repr(item[0])):
        if len(group) < 2:
            stats["groups_too_small"] += 1
            continue
        ordered = sorted(group, key=lambda row: row.utility_delta)
        rejected = ordered[0]
        chosen = ordered[-1]
        gap = chosen.utility_delta - rejected.utility_delta
        if gap < min_gap:
            stats["groups_no_gap"] += 1
            continue
        if (chosen.utility_delta < min_chosen_utility
                or rejected.utility_delta > max_rejected_utility):
            stats["groups_threshold_rejected"] += 1
            continue

        meta: dict[str, Any] = {
            "source": "learning_event_outcome",
            "chosen_event_id": chosen.event_id,
            "rejected_event_id": rejected.event_id,
            "task_id": chosen.task_id,
            "chosen_session_id": chosen.session_id,
            "rejected_session_id": rejected.session_id,
            "chosen_outcome_label": chosen.outcome_label,
            "rejected_outcome_label": rejected.outcome_label,
            "chosen_utility_delta": chosen.utility_delta,
            "rejected_utility_delta": rejected.utility_delta,
            "learner_reward_gap": gap,
            "combined_reward_components": {
                "constellation_style": None,
                "causal_validity": None,
                "learner_well_being": gap,
            },
            "chosen_feature_summary": chosen.feature_summary,
            "rejected_feature_summary": rejected.feature_summary,
            "chosen_provenance_sha256": chosen.provenance_sha256,
            "rejected_provenance_sha256": rejected.provenance_sha256,
            "group_key": repr(key),
            "min_gap": min_gap,
        }
        if include_feature_tensors:
            meta["chosen_feature_tensor"] = chosen.feature_tensor
            meta["rejected_feature_tensor"] = rejected.feature_tensor

        pairs.append({
            "prompt": prompt_for(chosen),
            "chosen": chosen.assistant_response,
            "rejected": rejected.assistant_response,
            "meta": meta,
        })
        stats["pairs_emitted"] += 1

    return pairs, stats


def validate_output_pair(pair: dict[str, Any], idx: int) -> None:
    for key in ("prompt", "chosen", "rejected", "meta"):
        if key not in pair:
            raise ValueError(f"pair {idx} missing {key}")
    if not pair["prompt"].strip() or not pair["chosen"].strip() or not pair["rejected"].strip():
        raise ValueError(f"pair {idx} has an empty DPO text field")
    if pair["chosen"] == pair["rejected"]:
        raise ValueError(f"pair {idx} chosen and rejected are identical")


def prepare_output(path: Path, overwrite: bool) -> Path:
    parent = path.parent
    if parent and not parent.exists():
        raise FileNotFoundError(f"output parent directory does not exist: {parent}")
    if path.exists() and not overwrite:
        raise FileExistsError(f"output already exists: {path}; pass --overwrite")
    tmp = path.with_suffix(path.suffix + ".tmp") if path.suffix else path.with_suffix(".tmp")
    if tmp.exists():
        tmp.unlink()
    return tmp


def write_pairs(path: Path, pairs: list[dict[str, Any]], overwrite: bool) -> int:
    tmp = prepare_output(path, overwrite)
    try:
        with tmp.open("w", encoding="utf-8") as f:
            for idx, pair in enumerate(pairs, start=1):
                validate_output_pair(pair, idx)
                f.write(json.dumps(pair, ensure_ascii=False, sort_keys=True))
                f.write("\n")
        tmp.replace(path)
    except Exception:
        if tmp.exists():
            tmp.unlink()
        raise
    return path.stat().st_size


def main() -> int:
    args = parse_args()
    logging.basicConfig(
        level=getattr(logging, args.log_level),
        format="%(levelname)s %(name)s: %(message)s",
    )
    LOG.info("reading events from %s", args.events)
    rows = load_events(args.events)
    LOG.info("loaded %d learning event rows", len(rows))

    pairs, stats = build_pairs(
        rows,
        min_gap=args.min_gap,
        min_chosen_utility=args.min_chosen_utility,
        max_rejected_utility=args.max_rejected_utility,
        include_feature_tensors=args.include_feature_tensors,
    )
    LOG.info("pair mining stats: %s", stats)
    if not pairs:
        raise RuntimeError(
            "no learner DPO pairs were emitted; record at least two real "
            "responses for the same query/context with different outcomes"
        )

    bytes_written = write_pairs(args.output, pairs, args.overwrite)
    print(json.dumps({
        "status": "ok",
        "source": str(args.events),
        "output": str(args.output),
        "pairs": len(pairs),
        "bytes_written": bytes_written,
        "stats": stats,
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        LOG.exception("build_learner_dpo_pairs failed")
        print(json.dumps({"status": "error", "error": str(exc)}, sort_keys=True), file=sys.stderr)
        sys.exit(1)
