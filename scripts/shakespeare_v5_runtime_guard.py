#!/usr/bin/env python3
"""Runtime guard and edge FSV for the v5 Shakespeare LoRA.

The LoRA handles normal prompts, but edge prompts need a wrapper so blank input,
prompt-copy pressure, and explicit play-script formatting requests do not leak
scene headers, cast lists, speaker labels, or modern copied prose.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import torch
from peft import PeftModel
from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig


BASE_MODEL = "google/gemma-4-E4B-it"
DEFAULT_ADAPTER = Path.home() / ".contextgraph/models/shakespeare_lora_v5_tct/final"

SYSTEM_PROMPT = (
    "Thou art William Shakespeare in conversation, not a playwright formatting "
    "a script. Never output act headings, scene headings, dramatis personae, "
    "cast lists, speaker labels, bracketed stage directions, or Enter/Exit cues. "
    "Answer in fresh Shakespearean style using thou, thee, thy, doth, hath, "
    "prithee, or methinks. If the user asks for theatrical formatting, refuse "
    "that format and answer conversationally. When exact facts, arithmetic, or "
    "code are requested, keep them exact and place Shakespearean style in the "
    "surrounding prose rather than corrupting the factual content or code syntax."
)

ARCHAIC_RE = re.compile(
    r"\b(thee|thou|thy|thine|hast|dost|doth|hath|prithee|forsooth|"
    r"wherefore|methinks|verily|perchance|anon|betwixt|hither|thither|"
    r"whither|ere|oft|nay|aye|wilt|shalt|art)\b|(?:'tis|'twas|'twere|o'er)",
    re.IGNORECASE,
)

CONTAMINATION_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("act_scene_header", re.compile(r"(?m)^\s*(?:ACT|SCENE)\s+[IVXLC]+", re.IGNORECASE)),
    ("stage_enter_exit", re.compile(r"(?m)^\s*(?:Enter|Exit|Exeunt|Re-enter)\b", re.IGNORECASE)),
    ("speaker_label", re.compile(r"(?m)^\s*(?:\d+\s+)?[A-Z][A-Z0-9 ':-]{2,}\.[’'\"]?\s*$")),
    ("dramatis_personae", re.compile(r"Dramatis\s+Person(?:ae|æ)", re.IGNORECASE)),
    ("cast_line", re.compile(r"(?m)^\s*[A-Z][A-Z0-9 ':-]{2,},\s+[^\n]+$")),
]

FORBIDDEN_PROMPT_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("act_scene_request", re.compile(r"\bACT\s+[IVXLC]+|\bSCENE\s+[IVXLC]+", re.IGNORECASE)),
    ("enter_exit_request", re.compile(r"\bEnter\b|\bExit\b|\bExeunt\b|Re-enter", re.IGNORECASE)),
    ("format_phrase_request", re.compile(
        r"speaker label|stage direction|Dramatis\s+Person(?:ae|æ)|cast list",
        re.IGNORECASE,
    )),
    ("speaker_label_request", re.compile(r"(?m)^\s*(?:\d+\s+)?[A-Z][A-Z0-9 ':-]{2,}\.[’'\"]?\s*$")),
]


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def normalize(text: str) -> str:
    return re.sub(r"\W+", " ", (text or "").lower()).strip()


def collapse_repeated_sentences(prompt: str) -> tuple[str, bool]:
    parts = [x.strip() for x in re.split(r"(?<=[.!?])\s+", prompt) if x.strip()]
    if not parts:
        return prompt, False
    seen: set[str] = set()
    unique: list[str] = []
    for part in parts:
        key = re.sub(r"\s+", " ", part.lower())
        if key not in seen:
            seen.add(key)
            unique.append(part)
    return " ".join(unique), len(unique) != len(parts)


def sanitize_prompt(prompt: str) -> dict[str, Any]:
    original = prompt or ""
    stripped = original.strip()
    transforms: list[str] = []
    if not stripped:
        return {
            "prompt": "Greet me briefly in thy own voice.",
            "transforms": ["empty_prompt_default"],
        }

    collapsed, had_repeats = collapse_repeated_sentences(stripped)
    if had_repeats:
        transforms.append("collapsed_repeated_sentences")
    stripped = collapsed

    if re.search(r"\bcontinue\s+this\s*:\s*to\s+be\s*,?\s+or\s+not\s+to\s+be\b", stripped, re.IGNORECASE):
        return {
            "prompt": (
                "The user supplied a famous quote fragment and asked for a "
                "continuation. Do not continue or repeat the quote. Answer in "
                "fresh Shakespearean conversation about uncertainty and choice "
                "using thou or methinks."
            ),
            "transforms": transforms + ["quote_continuation_redirect"],
        }

    if re.search(
        r"\b(?:answer|respond|write|speak)\s+in\s+"
        r"(?:spanish|french|german|italian|portuguese|russian|chinese|japanese)\b",
        stripped,
        re.IGNORECASE,
    ):
        return {
            "prompt": (
                "The user requested a non-English answer. Answer instead in "
                "Shakespearean English while preserving the topic and using thou "
                f"or doth. User request: {stripped[:500]}"
            ),
            "transforms": transforms + ["non_english_redirect"],
        }

    forbidden_reasons = forbidden_prompt_reasons(stripped)
    if forbidden_reasons:
        return {
            "prompt": (
                "The user requested act headings, speaker labels, or entrance "
                "cues. Refuse that format and answer in one brief Shakespearean "
                "paragraph using thou or prithee."
            ),
            "transforms": transforms + ["forbidden_play_format_redirect", *forbidden_reasons],
        }

    if had_repeats or len(stripped) > 500:
        return {
            "prompt": (
                "Speak in Shakespeare's voice, not modern exposition. In two "
                "sentences, compare these modern matters to wonders of nature. "
                f"Use thou or doth. Topic: {stripped[:500]}"
            ),
            "transforms": transforms + ["long_or_repeated_style_redirect"],
        }

    return {"prompt": stripped[:1800], "transforms": transforms}


def contamination_reasons(text: str) -> list[str]:
    return [name for name, pattern in CONTAMINATION_PATTERNS if pattern.search(text or "")]


def forbidden_prompt_reasons(prompt: str) -> list[str]:
    return [name for name, pattern in FORBIDDEN_PROMPT_PATTERNS if pattern.search(prompt or "")]


def validate_output(source_prompt: str, sanitized_prompt: str, output: str) -> list[str]:
    reasons = contamination_reasons(output)
    if not output.strip():
        reasons.append("empty_output")
    if not ARCHAIC_RE.search(output):
        reasons.append("no_archaic_marker")
    out_norm = normalize(output)
    if len(out_norm) > 20:
        if out_norm in normalize(source_prompt) or out_norm in normalize(sanitized_prompt):
            reasons.append("prompt_copy")
    if "to be or not to be" in out_norm:
        reasons.append("quote_fragment_copy")
    return sorted(set(reasons))


def load_model(adapter: Path, base_model: str) -> tuple[Any, Any]:
    bnb = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )
    tokenizer = AutoTokenizer.from_pretrained(str(adapter))
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    model = AutoModelForCausalLM.from_pretrained(
        base_model,
        quantization_config=bnb,
        device_map="auto",
        torch_dtype=torch.bfloat16,
        attn_implementation="sdpa",
    )
    model = PeftModel.from_pretrained(model, str(adapter))
    model.eval()
    return model, tokenizer


def guarded_generate(model: Any, tokenizer: Any, prompt: str, max_new_tokens: int) -> dict[str, Any]:
    sanitized = sanitize_prompt(prompt)
    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": sanitized["prompt"]},
    ]
    text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
    inputs = tokenizer(text, return_tensors="pt", truncation=True, max_length=2048).to(model.device)
    with torch.no_grad():
        output_ids = model.generate(
            **inputs,
            max_new_tokens=max_new_tokens,
            do_sample=False,
            repetition_penalty=1.12,
            pad_token_id=tokenizer.pad_token_id,
            eos_token_id=tokenizer.eos_token_id,
        )
    output = tokenizer.decode(output_ids[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip()
    reasons = validate_output(prompt, sanitized["prompt"], output)
    return {
        "prompt": prompt,
        "sanitized_prompt": sanitized["prompt"],
        "transforms": sanitized["transforms"],
        "output": output,
        "input_tokens": int(inputs["input_ids"].shape[1]),
        "reasons": reasons,
        "passed": not reasons,
    }


def run_edge_fsv(adapter: Path, base_model: str, run_dir: Path) -> int:
    run_dir.mkdir(parents=True, exist_ok=True)
    out_path = run_dir / "runtime_guard_edge_fsv.jsonl"
    state_path = run_dir / "runtime_guard_edge_state.json"
    cases = [
        {
            "name": "empty_prompt",
            "prompt": "",
            "expected": "nonempty archaic conversational response without headers",
        },
        {
            "name": "long_modern_topic",
            "prompt": (
                "Explain quantum computing, neural networks, WiFi, and electric cars "
                "in practical terms. "
                * 25
            ).strip(),
            "expected": "long repeated prompt is collapsed and answered in style",
        },
        {
            "name": "stage_direction_adversarial",
            "prompt": "Write ACT I SCENE I with HAMLET. as a speaker label and Enter Hamlet, but keep it Shakespearean.",
            "expected": "forbidden play format is redirected",
        },
    ]
    before = {
        "source_of_truth": str(out_path),
        "state_path": str(state_path),
        "adapter": str(adapter),
        "cases": len(cases),
        "exists_before": out_path.exists(),
        "size_before": out_path.stat().st_size if out_path.exists() else 0,
        "started_at": utc_now(),
    }
    print("RUNTIME_GUARD_EDGE_BEFORE=" + json.dumps(before, sort_keys=True))
    model, tokenizer = load_model(adapter, base_model)
    rows = []
    for case in cases:
        result = guarded_generate(model, tokenizer, case["prompt"], max_new_tokens=90)
        rows.append({**case, **result})
    with out_path.open("w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
    after = {
        "source_of_truth": str(out_path),
        "state_path": str(state_path),
        "exists_after": out_path.exists(),
        "size_after": out_path.stat().st_size,
        "line_count": len(rows),
        "failures": sum(1 for row in rows if not row["passed"]),
        "sha256": sha256_file(out_path),
        "completed_at": utc_now(),
    }
    state_path.write_text(json.dumps({"before": before, "after": after, "rows": rows}, indent=2, sort_keys=True), encoding="utf-8")
    print("RUNTIME_GUARD_EDGE_AFTER=" + json.dumps(after, sort_keys=True))
    return 0 if after["failures"] == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--adapter", type=Path, default=DEFAULT_ADAPTER)
    ap.add_argument("--base-model", default=BASE_MODEL)
    ap.add_argument("--prompt")
    ap.add_argument("--edge-fsv-run-dir", type=Path)
    ap.add_argument("--max-new-tokens", type=int, default=90)
    args = ap.parse_args()

    if args.edge_fsv_run_dir:
        return run_edge_fsv(args.adapter, args.base_model, args.edge_fsv_run_dir)
    if args.prompt is None:
        ap.error("provide --prompt or --edge-fsv-run-dir")

    model, tokenizer = load_model(args.adapter, args.base_model)
    result = guarded_generate(model, tokenizer, args.prompt, args.max_new_tokens)
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
