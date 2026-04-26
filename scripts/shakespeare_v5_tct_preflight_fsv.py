#!/usr/bin/env python3
"""V5 Shakespeare TCT preflight with Full State Verification.

This is a bounded pre-training gate. It does not train a model and it does not
generate synthetic training data. It audits existing Shakespeare artifacts,
copies only already-existing safe records into tiny smoke files, writes a SQLite
evidence database, then reopens the files/database to verify the physical state.

Source of truth for a run:
  docs/shakespeareplan/runs/<run_id>/fsv.sqlite
  docs/shakespeareplan/runs/<run_id>/v5_tct_smoke_sft.jsonl
  docs/shakespeareplan/runs/<run_id>/v5_tct_smoke_dpo.jsonl
  docs/shakespeareplan/runs/<run_id>/preflight_report.md

The edge-case fixtures are disposable evidence files under the same run
directory. They are never written to ~/.contextgraph/training.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sqlite3
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


HOME = Path.home()
REPO = Path(__file__).resolve().parents[1]
DEFAULT_RUNS_DIR = REPO / "docs" / "shakespeareplan" / "runs"

SOURCE_FILES = [
    ("signals_sft", HOME / ".contextgraph/training/shakespeare_sft_all_signals.jsonl", "sft"),
    ("broader_sft", HOME / ".contextgraph/training/shakespeare_sft_broader.jsonl", "sft"),
    ("final_clean_sft", HOME / ".contextgraph/training/shakespeare_sft_final_clean.jsonl", "sft"),
    ("v4_final_sft", HOME / ".contextgraph/training/shakespeare_v4_sft_final.jsonl", "sft"),
    ("style_tags", HOME / ".contextgraph/training/shakespeare_stylistic_tags.jsonl", "tags"),
    ("dpo_clean", HOME / ".contextgraph/training/shakespeare_dpo_clean.jsonl", "dpo"),
    ("v4_2_camd", HOME / ".contextgraph/training/v4_2_camd_lmdb.jsonl", "dpo"),
    ("dpo_style_smoke", HOME / ".contextgraph/training/shakespeare_dpo_style_pairs_smoke.jsonl", "dpo"),
]

MAX_ASSISTANT_CHARS = 2200
MAX_PROMPT_CHARS = 800
MAX_DPO_CHARS = 2200
MIN_ASSISTANT_CHARS = 30

ARCHAIC_RE = re.compile(
    r"\b(thee|thou|thy|thine|hast|dost|doth|hath|prithee|forsooth|"
    r"wherefore|methinks|verily|perchance|anon|betwixt|hither|thither|"
    r"whither|ere|oft|nay|aye|wilt|shalt|art)\b|(?:'tis|'twas|'twere|o'er)",
    re.IGNORECASE,
)

CONTAMINATION_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("act_scene_header", re.compile(r"\b(?:ACT|SCENE)\s+[IVXLC]+\b", re.IGNORECASE)),
    ("stage_enter_exit", re.compile(r"\b(?:Enter|Exit|Exeunt|Re-enter)\b", re.IGNORECASE)),
    ("bracket_stage_direction", re.compile(
        r"\[(?:_|Aside|Exit|Exeunt|Enter|Reads|Within|Kneeling|Drum|Flourish|Alarum|"
        r"Thunder|Music|Dies|To\s+[A-Z]).*?\]",
        re.IGNORECASE | re.DOTALL,
    )),
    ("speaker_label", re.compile(r"(?m)^\s*(?:\d+\s+)?[A-Z][A-Z0-9 ':-]{2,}\.[’'\"]?\s*$")),
    ("internal_project_note", re.compile(
        r"IMPL-PHASE-|SITREP|/home/cabdru/contextgraph|context-graph|Edge case:",
        re.IGNORECASE,
    )),
    ("gutenberg_boilerplate", re.compile(
        r"Project Gutenberg|End of the Project Gutenberg|START OF THE PROJECT",
        re.IGNORECASE,
    )),
]

KNOWN_CANONICAL_STARTS = [
    "shall i compare thee to a summer",
    "to be, or not to be",
    "my glass shall not persuade me",
    "or i shall live your epitaph",
    "let me not to the marriage of true minds",
    "when in disgrace with fortune",
    "full fathom five",
    "all the world's a stage",
    "tomorrow, and tomorrow",
    "now is the winter of our discontent",
    "once more unto the breach",
    "friends, romans, countrymen",
]


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def preview(text: str, n: int = 180) -> str:
    return re.sub(r"\s+", " ", text or "").strip()[:n]


def text_from_messageish(value: Any) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        parts = []
        for item in value:
            if isinstance(item, dict):
                content = item.get("content")
                if isinstance(content, str):
                    parts.append(content)
            elif isinstance(item, str):
                parts.append(item)
        return "\n".join(parts)
    if isinstance(value, dict):
        content = value.get("content")
        return content if isinstance(content, str) else ""
    return ""


def contamination_reasons(*texts: str) -> list[str]:
    joined = "\n".join(t for t in texts if t)
    reasons = []
    for name, pattern in CONTAMINATION_PATTERNS:
        if pattern.search(joined):
            reasons.append(name)
    return reasons


def archaic_count(text: str) -> int:
    return len(ARCHAIC_RE.findall(text or ""))


def normalize_for_overlap(text: str) -> str:
    text = (text or "").lower()
    text = text.replace("’", "'").replace("“", '"').replace("”", '"')
    text = re.sub(r"[^a-z0-9']+", " ", text)
    return re.sub(r"\s+", " ", text).strip()


def memorization_reasons(prompt: str, text: str) -> list[str]:
    reasons: list[str] = []
    norm_text = normalize_for_overlap(text)
    norm_prompt = normalize_for_overlap(prompt)

    for start in KNOWN_CANONICAL_STARTS:
        if norm_text.startswith(start):
            reasons.append("known_canonical_start")
            break

    continuation_markers = (
        "begin:",
        "continue this thought",
        "speak as shakespeare would",
        "in the manner of the bard",
    )
    if any(marker in (prompt or "").lower() for marker in continuation_markers):
        reasons.append("continuation_prompt")

    write_about_match = re.search(r"write about:\s*(.{20,140})", prompt or "", re.IGNORECASE | re.DOTALL)
    if write_about_match:
        requested = normalize_for_overlap(write_about_match.group(1))
        if len(requested) >= 20 and requested in norm_text:
            reasons.append("prompt_prefix_copied")

    # A generic Bard prompt plus a long verse passage is a memorization risk, not
    # a useful arbitrary-topic style-transfer pair.
    line_count = len([ln for ln in (text or "").splitlines() if ln.strip()])
    if line_count >= 10 and (
        "voice of william shakespeare" in (prompt or "").lower()
        or "compose a passage" in (prompt or "").lower()
    ):
        reasons.append("generic_prompt_long_canon_risk")

    return sorted(set(reasons))


def has_style_score_trace(meta: dict[str, Any]) -> bool:
    if not isinstance(meta, dict):
        return False
    direct = {"chosen_style_score", "rejected_style_score", "score_gap"}
    if direct.issubset(meta.keys()):
        return True
    # Future CAMD exports may use this shape. Require the gap as well as both
    # sides so a DPO row has inspectable reward evidence.
    extended = {"chosen_extended_combined", "rejected_extended_combined", "gap_extended_combined"}
    if extended.issubset(meta.keys()):
        return True
    return False


REAL_SIGNAL_PREFIXES = (
    "L1_",
    "L2_",
    "L4_",
    "L6_",
    "L7_",
    "L8_",
)


def is_real_shakespeare_sft_candidate(source_name: str, meta: dict[str, Any]) -> bool:
    """Whether a clean SFT row may be copied into a training smoke file.

    We audit all artifacts, but candidate rows are stricter: assistant targets
    must be real Shakespeare-derived writing, not generated bridge content,
    adversarial fixtures, or modern-topic compositions.
    """
    if not isinstance(meta, dict):
        return False
    prompt_source = str(meta.get("prompt_source") or "")
    work = str(meta.get("work") or "")
    if source_name == "signals_sft":
        return (
            prompt_source.startswith(REAL_SIGNAL_PREFIXES)
            and work not in {"", "General", "N/A"}
        )
    if source_name == "broader_sft":
        return (
            (
                prompt_source.startswith(REAL_SIGNAL_PREFIXES)
                or prompt_source in {"graph_entity", "intra_work"}
            )
            and work not in {"", "General", "N/A"}
        )
    if source_name == "final_clean_sft":
        return prompt_source in {"graph_entity", "intra_work"}
    return False


def is_dpo_training_candidate(source_name: str, rec: ExtractedRecord) -> bool:
    """Current DPO exports are audited, but not trusted as v5 training input.

    The rejected side in existing CAMD exports can be scene text, internal test
    text, or unrelated raw spans. Until DPO is rebuilt from clean derived
    signals, do not copy these rows into v5 smoke training files.
    """
    return False


@dataclass
class ExtractedRecord:
    kind: str
    prompt: str = ""
    chosen: str = ""
    rejected: str = ""
    system: str = ""
    meta: dict[str, Any] = field(default_factory=dict)
    raw: dict[str, Any] = field(default_factory=dict)


def extract_record(obj: dict[str, Any], declared_kind: str) -> ExtractedRecord:
    if declared_kind == "tags":
        text = text_from_messageish(obj.get("text"))
        return ExtractedRecord(
            kind="tags",
            chosen=text,
            meta={
                "memory_id": obj.get("memory_id"),
                "semantic_topic": obj.get("semantic_topic"),
                "register": (obj.get("register") or {}).get("primary"),
                "meter": (obj.get("meter") or {}).get("form"),
                "style_profile_len": len(obj.get("style_profile_vector") or []),
            },
            raw=obj,
        )

    messages = obj.get("messages")
    if isinstance(messages, list):
        system = ""
        user = ""
        assistant = ""
        for msg in messages:
            if not isinstance(msg, dict):
                continue
            role = msg.get("role")
            content = msg.get("content")
            if not isinstance(content, str):
                continue
            if role == "system" and not system:
                system = content
            elif role == "user" and not user:
                user = content
            elif role == "assistant":
                assistant = content
        return ExtractedRecord(
            kind="sft",
            system=system,
            prompt=user,
            chosen=assistant,
            meta=obj.get("_meta") or obj.get("meta") or {},
            raw=obj,
        )

    if "chosen" in obj and "rejected" in obj:
        return ExtractedRecord(
            kind="dpo",
            prompt=text_from_messageish(obj.get("prompt")),
            chosen=text_from_messageish(obj.get("chosen")),
            rejected=text_from_messageish(obj.get("rejected")),
            meta=obj.get("_meta") or obj.get("meta") or {},
            raw=obj,
        )

    return ExtractedRecord(kind=declared_kind, raw=obj)


def validate_record(rec: ExtractedRecord, source_kind: str) -> tuple[str, list[str]]:
    reasons: list[str] = []
    status = "PASS"

    if rec.kind == "sft":
        if not rec.system or not rec.prompt or not rec.chosen:
            reasons.append("missing_sft_messages")
        if len(rec.prompt) > MAX_PROMPT_CHARS:
            reasons.append("prompt_too_long")
        if len(rec.chosen) < MIN_ASSISTANT_CHARS:
            reasons.append("assistant_too_short")
        if len(rec.chosen) > MAX_ASSISTANT_CHARS:
            reasons.append("assistant_too_long")
        if archaic_count(rec.chosen) == 0:
            reasons.append("no_archaic_marker")
        reasons.extend(contamination_reasons(rec.prompt, rec.chosen))
    elif rec.kind == "dpo":
        if not rec.prompt or not rec.chosen or not rec.rejected:
            reasons.append("missing_dpo_fields")
        if not has_style_score_trace(rec.meta):
            reasons.append("missing_style_score_trace")
        if len(rec.prompt) > MAX_PROMPT_CHARS:
            reasons.append("prompt_too_long")
        if len(rec.chosen) > MAX_DPO_CHARS or len(rec.rejected) > MAX_DPO_CHARS:
            reasons.append("dpo_text_too_long")
        if archaic_count(rec.chosen) == 0:
            reasons.append("chosen_no_archaic_marker")
        reasons.extend(memorization_reasons(rec.prompt, rec.chosen))
        reasons.extend(memorization_reasons(rec.prompt, rec.rejected))
        reasons.extend(contamination_reasons(rec.prompt, rec.chosen, rec.rejected))
    elif rec.kind == "tags":
        if not rec.chosen:
            reasons.append("missing_tag_text")
        if rec.meta.get("style_profile_len") != 13:
            reasons.append("style_profile_not_13")
        if not rec.meta.get("semantic_topic"):
            reasons.append("missing_semantic_topic")
        if not rec.meta.get("register"):
            reasons.append("missing_register")
        if not rec.meta.get("meter"):
            reasons.append("missing_meter")
        reasons.extend(contamination_reasons(rec.chosen))
    else:
        reasons.append(f"unknown_record_kind:{source_kind}")

    if reasons:
        status = "FAIL"
    return status, sorted(set(reasons))


def init_db(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS artifacts (
            name TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            kind TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            line_count INTEGER NOT NULL,
            sha256 TEXT NOT NULL,
            checked_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_name TEXT NOT NULL,
            line_no INTEGER NOT NULL,
            record_kind TEXT NOT NULL,
            status TEXT NOT NULL,
            reasons_json TEXT NOT NULL,
            json_digest TEXT NOT NULL,
            prompt_preview TEXT NOT NULL,
            chosen_preview TEXT NOT NULL,
            rejected_preview TEXT NOT NULL,
            meta_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS checks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            status TEXT NOT NULL,
            details_json TEXT NOT NULL,
            checked_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS smoke_outputs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            output_kind TEXT NOT NULL,
            path TEXT NOT NULL,
            line_count INTEGER NOT NULL,
            sha256 TEXT NOT NULL,
            status TEXT NOT NULL,
            details_json TEXT NOT NULL,
            checked_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS edge_cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            phase TEXT NOT NULL,
            before_json TEXT NOT NULL,
            after_json TEXT NOT NULL,
            expected_status TEXT NOT NULL,
            observed_status TEXT NOT NULL,
            passed INTEGER NOT NULL,
            details_json TEXT NOT NULL,
            checked_at TEXT NOT NULL
        );
        """
    )
    conn.commit()
    return conn


def reset_run_tables(conn: sqlite3.Connection) -> None:
    for table in ("artifacts", "records", "checks", "smoke_outputs", "edge_cases"):
        conn.execute(f"DELETE FROM {table}")
    conn.commit()


def insert_check(conn: sqlite3.Connection, name: str, status: str, details: dict[str, Any]) -> None:
    conn.execute(
        "INSERT INTO checks(name,status,details_json,checked_at) VALUES(?,?,?,?)",
        (name, status, json.dumps(details, sort_keys=True), utc_now()),
    )
    conn.commit()


def line_count(path: Path) -> int:
    with path.open("rb") as f:
        return sum(1 for _ in f)


def audit_source_file(
    conn: sqlite3.Connection,
    source_name: str,
    path: Path,
    kind: str,
    sft_candidates: list[dict[str, Any]],
    dpo_candidates: list[dict[str, Any]],
    max_records: int,
) -> dict[str, Any]:
    if not path.exists():
        summary = {"exists": False, "path": str(path), "total": 0, "pass": 0, "fail": 0}
        insert_check(conn, f"source_exists:{source_name}", "FAIL", summary)
        return summary

    total_lines = line_count(path)
    artifact_sha = sha256_file(path)
    conn.execute(
        """
        INSERT OR REPLACE INTO artifacts
        (name,path,kind,size_bytes,line_count,sha256,checked_at)
        VALUES(?,?,?,?,?,?,?)
        """,
        (source_name, str(path), kind, path.stat().st_size, total_lines, artifact_sha, utc_now()),
    )
    conn.commit()

    summary = {
        "exists": True,
        "path": str(path),
        "kind": kind,
        "line_count": total_lines,
        "sha256": artifact_sha,
        "total": 0,
        "pass": 0,
        "fail": 0,
        "parse_error": 0,
        "completion_markers": 0,
        "top_reasons": {},
        "training_candidates": 0,
    }

    with path.open("r", encoding="utf-8") as f:
        for line_no, line in enumerate(f, 1):
            if max_records and summary["total"] >= max_records:
                break
            raw_line = line.rstrip("\n")
            if not raw_line.strip():
                continue
            if raw_line.strip() == "__COMPLETE__":
                summary["completion_markers"] += 1
                continue
            summary["total"] += 1
            try:
                obj = json.loads(raw_line)
                if not isinstance(obj, dict):
                    raise ValueError("json_root_not_object")
            except Exception as exc:
                summary["parse_error"] += 1
                summary["fail"] += 1
                reasons = [f"parse_error:{type(exc).__name__}"]
                conn.execute(
                    """
                    INSERT INTO records
                    (source_name,line_no,record_kind,status,reasons_json,json_digest,
                     prompt_preview,chosen_preview,rejected_preview,meta_json)
                    VALUES(?,?,?,?,?,?,?,?,?,?)
                    """,
                    (
                        source_name,
                        line_no,
                        kind,
                        "FAIL",
                        json.dumps(reasons),
                        sha256_text(raw_line),
                        "",
                        preview(raw_line),
                        "",
                        "{}",
                    ),
                )
                continue

            rec = extract_record(obj, kind)
            status, reasons = validate_record(rec, kind)
            if status == "PASS":
                summary["pass"] += 1
                if rec.kind == "sft":
                    if is_real_shakespeare_sft_candidate(source_name, rec.meta):
                        summary["training_candidates"] += 1
                        sft_candidates.append({
                            "messages": rec.raw["messages"],
                            "_meta": {
                                **(rec.meta if isinstance(rec.meta, dict) else {}),
                                "fsv_source": source_name,
                                "fsv_line_no": line_no,
                                "fsv_json_digest": sha256_text(raw_line),
                                "training_policy": "real_shakespeare_target_only",
                            },
                        })
                elif rec.kind == "dpo":
                    if is_dpo_training_candidate(source_name, rec):
                        summary["training_candidates"] += 1
                        dpo_candidates.append({
                            "prompt": rec.prompt,
                            "chosen": rec.chosen,
                            "rejected": rec.rejected,
                            "meta": {
                                **(rec.meta if isinstance(rec.meta, dict) else {}),
                                "fsv_source": source_name,
                                "fsv_line_no": line_no,
                                "fsv_json_digest": sha256_text(raw_line),
                                "training_policy": "real_shakespeare_dpo_only",
                            },
                        })
            else:
                summary["fail"] += 1
                for reason in reasons:
                    summary["top_reasons"][reason] = summary["top_reasons"].get(reason, 0) + 1

            conn.execute(
                """
                INSERT INTO records
                (source_name,line_no,record_kind,status,reasons_json,json_digest,
                 prompt_preview,chosen_preview,rejected_preview,meta_json)
                VALUES(?,?,?,?,?,?,?,?,?,?)
                """,
                (
                    source_name,
                    line_no,
                    rec.kind,
                    status,
                    json.dumps(reasons),
                    sha256_text(raw_line),
                    preview(rec.prompt),
                    preview(rec.chosen),
                    preview(rec.rejected),
                    json.dumps(rec.meta, sort_keys=True, ensure_ascii=False)[:4000],
                ),
            )

    conn.commit()
    source_status = "PASS" if summary["fail"] == 0 and summary["parse_error"] == 0 else "WARN"
    insert_check(conn, f"source_audit:{source_name}", source_status, summary)
    return summary


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    tmp = path.with_suffix(path.suffix + ".partial")
    with tmp.open("w", encoding="utf-8") as f:
        for rec in records:
            f.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    tmp.replace(path)


def record_selection_key(record: dict[str, Any], kind: str) -> str:
    if kind == "sft":
        rec = extract_record(record, "sft")
        chosen_digest = sha256_text(normalize_for_overlap(rec.chosen))
        return f"sft:{chosen_digest}"
    rec = extract_record(record, "dpo")
    return "dpo:" + sha256_text(
        normalize_for_overlap(rec.prompt)
        + "\n---\n"
        + normalize_for_overlap(rec.chosen)
        + "\n---\n"
        + normalize_for_overlap(rec.rejected)
    )


def select_unique_records(records: list[dict[str, Any]], count: int, kind: str) -> list[dict[str, Any]]:
    selected: list[dict[str, Any]] = []
    seen: set[str] = set()
    for record in records:
        key = record_selection_key(record, kind)
        if key in seen:
            continue
        seen.add(key)
        selected.append(record)
        if len(selected) >= count:
            break
    return selected


def unique_candidate_count(records: list[dict[str, Any]], kind: str) -> int:
    return len({record_selection_key(record, kind) for record in records})


def inspect_jsonl(path: Path, expected_count: int, kind: str) -> dict[str, Any]:
    details = {
        "path": str(path),
        "exists": path.exists(),
        "expected_count": expected_count,
        "line_count": 0,
        "parse_errors": 0,
        "schema_errors": 0,
        "sha256": "",
        "sample_digest": "",
    }
    if not path.exists():
        return details
    details["sha256"] = sha256_file(path)
    first_payload = ""
    with path.open("r", encoding="utf-8") as f:
        for line_no, line in enumerate(f, 1):
            stripped = line.strip()
            if not stripped:
                continue
            details["line_count"] += 1
            if not first_payload:
                first_payload = stripped
            try:
                obj = json.loads(stripped)
            except Exception:
                details["parse_errors"] += 1
                continue
            if kind == "sft":
                rec = extract_record(obj, "sft")
                status, _ = validate_record(rec, "sft")
                if status != "PASS":
                    details["schema_errors"] += 1
            elif kind == "dpo":
                rec = extract_record(obj, "dpo")
                status, _ = validate_record(rec, "dpo")
                if status != "PASS":
                    details["schema_errors"] += 1
    details["sample_digest"] = sha256_text(first_payload) if first_payload else ""
    return details


def build_smoke_outputs(
    conn: sqlite3.Connection,
    run_dir: Path,
    sft_candidates: list[dict[str, Any]],
    dpo_candidates: list[dict[str, Any]],
    sft_count: int,
    dpo_count: int,
) -> tuple[Path, Path]:
    smoke_sft = run_dir / "v5_tct_smoke_sft.jsonl"
    smoke_dpo = run_dir / "v5_tct_smoke_dpo.jsonl"

    selected_sft = select_unique_records(sft_candidates, sft_count, "sft")
    selected_dpo = select_unique_records(dpo_candidates, dpo_count, "dpo")

    write_jsonl(smoke_sft, selected_sft)
    write_jsonl(smoke_dpo, selected_dpo)

    for kind, path, expected, minimum in [
        ("sft", smoke_sft, len(selected_sft), min(1, sft_count)),
        ("dpo", smoke_dpo, len(selected_dpo), min(1, dpo_count)),
    ]:
        details = inspect_jsonl(path, expected, kind)
        status = "PASS"
        if not details["exists"]:
            status = "FAIL"
        if details["line_count"] != expected:
            status = "FAIL"
        if details["line_count"] < minimum:
            status = "FAIL"
        if details["parse_errors"] or details["schema_errors"]:
            status = "FAIL"
        conn.execute(
            """
            INSERT INTO smoke_outputs
            (output_kind,path,line_count,sha256,status,details_json,checked_at)
            VALUES(?,?,?,?,?,?,?)
            """,
            (
                kind,
                str(path),
                details["line_count"],
                details.get("sha256") or "",
                status,
                json.dumps(details, sort_keys=True),
                utc_now(),
            ),
        )
        insert_check(conn, f"smoke_output:{kind}", status, details)
    conn.commit()
    return smoke_sft, smoke_dpo


def table_count(conn: sqlite3.Connection, table: str, where: str = "", args: tuple[Any, ...] = ()) -> int:
    sql = f"SELECT COUNT(*) FROM {table}"
    if where:
        sql += " WHERE " + where
    return int(conn.execute(sql, args).fetchone()[0])


def audit_fixture_file(path: Path, kind: str) -> tuple[str, dict[str, Any]]:
    if not path.exists():
        return "FAIL_MISSING_FILE", {"exists": False}
    total = 0
    failures = 0
    reasons: list[str] = []
    with path.open("r", encoding="utf-8") as f:
        for line_no, line in enumerate(f, 1):
            stripped = line.strip()
            if not stripped:
                continue
            total += 1
            try:
                obj = json.loads(stripped)
                rec = extract_record(obj, kind)
                status, rec_reasons = validate_record(rec, kind)
            except Exception as exc:
                status = "FAIL"
                rec_reasons = [f"parse_error:{type(exc).__name__}"]
            if status != "PASS":
                failures += 1
                reasons.extend(rec_reasons)
    details = {
        "path": str(path),
        "size_bytes": path.stat().st_size,
        "total_records": total,
        "failures": failures,
        "reasons": sorted(set(reasons)),
        "sha256": sha256_file(path),
    }
    if total == 0:
        return "FAIL_NO_RECORDS", details
    if any(r.startswith("parse_error") for r in reasons):
        return "FAIL_PARSE", details
    if "assistant_too_long" in reasons or "dpo_text_too_long" in reasons:
        return "FAIL_MAX_LIMIT", details
    if any(r in reasons for r, _ in CONTAMINATION_PATTERNS):
        return "FAIL_CONTAMINATION", details
    if failures:
        return "FAIL_SCHEMA", details
    return "PASS", details


def run_edge_cases(conn: sqlite3.Connection, run_dir: Path) -> None:
    edge_dir = run_dir / "edge_cases"
    edge_dir.mkdir(parents=True, exist_ok=True)

    clean_system = "You are a modern assistant writing in Shakespearean style."
    valid_sft = {
        "messages": [
            {"role": "system", "content": clean_system},
            {"role": "user", "content": "Explain a small test."},
            {"role": "assistant", "content": "Prithee, this little proof doth show the gears within."},
        ]
    }
    cases = [
        {
            "name": "empty_input",
            "kind": "sft",
            "expected": "FAIL_NO_RECORDS",
            "content": "",
        },
        {
            "name": "invalid_json",
            "kind": "sft",
            "expected": "FAIL_PARSE",
            "content": '{"messages": [this is not valid json]\n',
        },
        {
            "name": "contaminated_stage_text",
            "kind": "sft",
            "expected": "FAIL_CONTAMINATION",
            "content": json.dumps({
                "messages": [
                    {"role": "system", "content": clean_system},
                    {"role": "user", "content": "Give me a clean response."},
                    {"role": "assistant", "content": "SCENE I. A room.\nEnter Hamlet.\nHAMLET.\nThou art here."},
                ]
            }) + "\n",
        },
        {
            "name": "maximum_limit_overflow",
            "kind": "sft",
            "expected": "FAIL_MAX_LIMIT",
            "content": json.dumps({
                **valid_sft,
                "messages": [
                    valid_sft["messages"][0],
                    valid_sft["messages"][1],
                    {
                        "role": "assistant",
                        "content": "Thou " + ("word " * (MAX_ASSISTANT_CHARS + 10)),
                    },
                ],
            }) + "\n",
        },
    ]

    for case in cases:
        path = edge_dir / f"{case['name']}.jsonl"
        before = {
            "path": str(path),
            "exists": path.exists(),
            "size_bytes": path.stat().st_size if path.exists() else 0,
            "edge_rows_before": table_count(conn, "edge_cases", "name=?", (case["name"],)),
        }
        print(f"EDGE BEFORE {case['name']}: {json.dumps(before, sort_keys=True)}")
        path.write_text(case["content"], encoding="utf-8")
        observed, details = audit_fixture_file(path, case["kind"])
        after = {
            "path": str(path),
            "exists": path.exists(),
            "size_bytes": path.stat().st_size,
            "observed_status": observed,
            "details": details,
            "edge_rows_after_write_before_insert": table_count(conn, "edge_cases", "name=?", (case["name"],)),
        }
        passed = int(observed == case["expected"])
        conn.execute(
            """
            INSERT INTO edge_cases
            (name,phase,before_json,after_json,expected_status,observed_status,
             passed,details_json,checked_at)
            VALUES(?,?,?,?,?,?,?,?,?)
            """,
            (
                case["name"],
                "manual_fixture_audit",
                json.dumps(before, sort_keys=True),
                json.dumps(after, sort_keys=True),
                case["expected"],
                observed,
                passed,
                json.dumps(details, sort_keys=True),
                utc_now(),
            ),
        )
        conn.commit()
        after_insert = {
            **after,
            "edge_rows_after_insert": table_count(conn, "edge_cases", "name=?", (case["name"],)),
            "passed": bool(passed),
        }
        print(f"EDGE AFTER  {case['name']}: {json.dumps(after_insert, sort_keys=True)}")

    failed_edges = table_count(conn, "edge_cases", "passed=0")
    insert_check(
        conn,
        "edge_case_audit",
        "PASS" if failed_edges == 0 else "FAIL",
        {
            "edge_cases": table_count(conn, "edge_cases"),
            "failed_edges": failed_edges,
        },
    )


def reopen_and_verify(db_path: Path, smoke_sft: Path, smoke_dpo: Path) -> dict[str, Any]:
    conn = sqlite3.connect(db_path)
    try:
        state = {
            "db_path": str(db_path),
            "artifacts": table_count(conn, "artifacts"),
            "records": table_count(conn, "records"),
            "records_pass": table_count(conn, "records", "status='PASS'"),
            "records_fail": table_count(conn, "records", "status='FAIL'"),
            "checks": table_count(conn, "checks"),
            "checks_fail": table_count(conn, "checks", "status='FAIL'"),
            "checks_warn": table_count(conn, "checks", "status='WARN'"),
            "smoke_outputs": table_count(conn, "smoke_outputs"),
            "edge_cases": table_count(conn, "edge_cases"),
            "edge_cases_failed": table_count(conn, "edge_cases", "passed=0"),
            "smoke_sft": inspect_jsonl(smoke_sft, line_count(smoke_sft) if smoke_sft.exists() else 0, "sft"),
            "smoke_dpo": inspect_jsonl(smoke_dpo, line_count(smoke_dpo) if smoke_dpo.exists() else 0, "dpo"),
        }
    finally:
        conn.close()
    return state


def write_report(
    run_dir: Path,
    db_path: Path,
    source_summaries: list[dict[str, Any]],
    final_state: dict[str, Any],
) -> Path:
    report = run_dir / "preflight_report.md"
    lines = [
        "# Shakespeare V5 TCT Preflight FSV",
        "",
        f"Run UTC: `{utc_now()}`",
        "",
        "## Source Of Truth",
        "",
        f"- SQLite evidence DB: `{db_path}`",
        f"- Smoke SFT JSONL: `{run_dir / 'v5_tct_smoke_sft.jsonl'}`",
        f"- Smoke DPO JSONL: `{run_dir / 'v5_tct_smoke_dpo.jsonl'}`",
        "",
        "## Source Artifact Audit",
        "",
        "| Source | Kind | Lines | Audited | Pass | Fail | Parse errors | Top reasons |",
        "|---|---|---:|---:|---:|---:|---:|---|",
    ]
    for s in source_summaries:
        top = ", ".join(f"{k}:{v}" for k, v in sorted(
            (s.get("top_reasons") or {}).items(),
            key=lambda kv: kv[1],
            reverse=True,
        )[:5])
        lines.append(
            f"| {s.get('path', '')} | {s.get('kind', '')} | {s.get('line_count', 0)} | "
            f"{s.get('total', 0)} | {s.get('pass', 0)} | {s.get('fail', 0)} | "
            f"{s.get('parse_error', 0)} | {top} |"
        )
    lines.extend([
        "",
        "## Reopened State Evidence",
        "",
        "```json",
        json.dumps(final_state, indent=2, sort_keys=True),
        "```",
        "",
        "## Decision",
        "",
    ])
    if final_state["checks_fail"] == 0 and final_state["edge_cases_failed"] == 0:
        if final_state.get("checks_warn", 0):
            lines.append(
                "PASS for smoke artifact construction, with WARN upstream source "
                "audits. Do not treat the audited source files as fully training-clean."
            )
        else:
            lines.append("PASS for pre-training smoke artifact construction.")
    else:
        lines.append("FAIL or WARN: inspect `checks` and `records` tables before training.")
    report.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return report


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--run-id", default=datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S"))
    ap.add_argument("--runs-dir", type=Path, default=DEFAULT_RUNS_DIR)
    ap.add_argument("--max-records", type=int, default=0,
                    help="Per-source record cap for quick debugging. 0 means full file.")
    ap.add_argument("--smoke-sft-count", type=int, default=32)
    ap.add_argument("--smoke-dpo-count", type=int, default=0)
    ap.add_argument("--skip-edge-cases", action="store_true")
    args = ap.parse_args()

    run_dir = args.runs_dir / args.run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    db_path = run_dir / "fsv.sqlite"
    conn = init_db(db_path)
    reset_run_tables(conn)

    print(f"SOURCE_OF_TRUTH_DB={db_path}")
    print(f"SOURCE_OF_TRUTH_RUN_DIR={run_dir}")

    sft_candidates: list[dict[str, Any]] = []
    dpo_candidates: list[dict[str, Any]] = []
    source_summaries = []
    for name, path, kind in SOURCE_FILES:
        print(f"AUDIT START {name}: {path}")
        summary = audit_source_file(
            conn,
            name,
            path,
            kind,
            sft_candidates,
            dpo_candidates,
            args.max_records,
        )
        source_summaries.append(summary)
        print(f"AUDIT DONE  {name}: {json.dumps(summary, sort_keys=True)[:1200]}")

    insert_check(conn, "safe_candidate_counts", "PASS" if sft_candidates else "FAIL", {
        "safe_sft_candidates": len(sft_candidates),
        "safe_dpo_candidates": len(dpo_candidates),
        "unique_safe_sft_candidates": unique_candidate_count(sft_candidates, "sft"),
        "unique_safe_dpo_candidates": unique_candidate_count(dpo_candidates, "dpo"),
        "requested_sft": args.smoke_sft_count,
        "requested_dpo": args.smoke_dpo_count,
    })

    smoke_sft, smoke_dpo = build_smoke_outputs(
        conn,
        run_dir,
        sft_candidates,
        dpo_candidates,
        args.smoke_sft_count,
        args.smoke_dpo_count,
    )

    if not args.skip_edge_cases:
        run_edge_cases(conn, run_dir)

    conn.close()

    final_state = reopen_and_verify(db_path, smoke_sft, smoke_dpo)
    report = write_report(run_dir, db_path, source_summaries, final_state)

    print("REOPENED_SOURCE_OF_TRUTH_STATE:")
    print(json.dumps(final_state, indent=2, sort_keys=True))
    print(f"REPORT={report}")

    if final_state["checks_fail"] or final_state["edge_cases_failed"]:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
