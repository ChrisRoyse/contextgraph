#!/usr/bin/env python3
"""Build style-contrastive DPO pairs for the Shakespeare resurrection mission.

For each Shakespeare SFT example:
  1. Read the original assistant reply (CHOSEN) + user prompt
  2. Look up the matching stylistic tag record (produced by Phase B analyzer)
  3. Generate up to 4 stylistically-inferior REJECTED variants
     (modern_paraphrase, archaic_pastiche, meter_modern, broken_archaic)
  4. Score CHOSEN + each REJECTED against the Sonnets constellation
  5. Keep pair iff:
       - style_score(chosen)   >= 0.45
       - style_score(rejected) <= 0.30
       - gap                   >= 0.20
  6. ALWAYS forget_concept(fingerprint) in try/finally

Daemon contract:
  - Requires context-graph daemon on 127.0.0.1:3100
  - Abort if > 1 %% of daemon calls fail
  - No fallback, no mock data

Output:
  ~/.contextgraph/training/shakespeare_dpo_style_pairs.jsonl
  (written atomically as .partial then renamed; sidecar state file allows resume)

Usage:
    python3 scripts/shakespeare_build_dpo_style_pairs.py \
        --sft-input  ~/.contextgraph/training/shakespeare_sft_final_clean.jsonl \
        --tags-input ~/.contextgraph/training/shakespeare_stylistic_tags.jsonl \
        --output     ~/.contextgraph/training/shakespeare_dpo_style_pairs.jsonl \
        [--target-count 30000] \
        [--limit-sft 100]   # for testing
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import re
import sys
import time
from collections import Counter
from pathlib import Path
from typing import Iterator

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _gemma_shakespeare_lib import Rpc   # noqa: E402
# PATCH P2-1 (S3 C-10/C-11): import shared resource guard (SIGTERM handler,
# RSS cap, heartbeat).
from shakespeare_resource_guard import managed_run  # noqa: E402


# =============================================================================
# Constants
# =============================================================================
SONNETS_CONSTELLATION_ID = "a8511ebf-80ad-4afb-a7d5-865b532ed4c5"  # Resurrection rebuild 2026-04-17: 161 sonnet-tagged passages

# Mirror of STYLE_EMBEDDER_WEIGHTS from scripts/shakespeare_chat_dashboard.py.
# These five embedders discriminate STYLE (not topic). Weights must sum to 1.0.
STYLE_EMBEDDER_WEIGHTS = {
    6:  0.15,   # E7  code/syntax pattern
    8:  0.15,   # E9  HDC structural robustness
    9:  0.15,   # E10 paraphrase (multimodal)
    11: 0.30,   # E12 ColBERT token-level (strongest)
    12: 0.25,   # E13 SPLADE term-expansion
}
assert abs(sum(STYLE_EMBEDDER_WEIGHTS.values()) - 1.0) < 1e-9

# Filter thresholds — pair is KEPT iff all three hold.
CHOSEN_MIN_STYLE_SCORE   = 0.45
REJECTED_MAX_STYLE_SCORE = 0.30
MIN_SCORE_GAP            = 0.20

# Abort run if > 1 %% of daemon calls fail.
DAEMON_FAIL_BUDGET = 0.01

# Logs + state.
REJECTED_LOG_PATH = Path("/tmp/shake_dpo_rejected_pairs.log")
COMPLETION_MARKER = "__COMPLETE__\n"


# =============================================================================
# Archaic <-> Modern substitution table
# =============================================================================
ARCHAIC_TO_MODERN: dict[str, str] = {
    # pronouns
    "thee": "you", "thou": "you", "thy": "your", "thine": "yours",
    "ye": "you",
    # verbs
    "hast": "have", "dost": "do", "doth": "does", "hath": "has",
    "art": "are", "wilt": "will", "shalt": "will",
    "wouldst": "would", "shouldst": "should", "canst": "can",
    "didst": "did", "mayst": "may", "mayest": "may",
    # markers / discourse
    "prithee": "please", "forsooth": "indeed", "hark": "listen",
    "wherefore": "why", "methinks": "I think", "verily": "truly",
    "perchance": "perhaps", "alas": "unfortunately",
    "betwixt": "between", "anon": "soon", "yon": "that",
    "yonder": "over there", "fie": "come on",
    "oft": "often", "tarry": "wait", "beseech": "beg",
    "naught": "nothing", "nought": "nothing",
    "whilst": "while", "unto": "to", "thereof": "of it",
    "hither": "here", "thither": "there", "whither": "where",
    "herein": "here", "wherein": "where",
    "hence": "from here", "thence": "from there", "whence": "from where",
    "morn": "morning", "ere": "before", "nay": "no", "aye": "yes",
    "mayhap": "maybe",
    "quill": "pen", "hearken": "listen", "mortal": "person",
    "immortal": "eternal", "beauteous": "beautiful",
    "wondrous": "amazing", "valiant": "brave",
    "treacherous": "sneaky", "villainous": "evil",
}

# Keys whose endings are "ungrammatical modern" after substitution — handled below.
ARCHAIC_CONTRACTIONS: dict[str, str] = {
    "'tis": "it's", "'twas": "it was", "'twere": "it were",
    "o'er": "over", "e'er": "ever", "ne'er": "never",
    "gi'": "give", "i'": "in",
}

# Inversion patterns ("thither go I" -> "I go there", "so did he" -> "he did").
INVERSION_PATTERNS: list[tuple[re.Pattern, str]] = [
    # Simple SOV / adv-verb-subj detection. We only un-invert common patterns;
    # aggressive parsing would need a real parser. This is a heuristic variant.
    (re.compile(r"\b(thither|hither|whither|yonder|anon)\s+(go|come|went|came|doth)\s+(I|thou|he|she|we|they|you)\b",
                re.IGNORECASE),
     lambda m: f"{m.group(3)} {m.group(2)} {m.group(1)}"),
]

# Modern filler phrases injected by the paraphrase variant.
MODERN_FILLERS = [
    "kind of", "really", "pretty much", "sort of", "basically",
    "you know", "like", "just", "honestly",
]


# 10-syllable modern templates (count: syllables marked; verified manually).
METER_MODERN_TEMPLATES: list[str] = [
    "I think about the way the world has changed,",
    "Perhaps the answer lies in what we see,",
    "The truth of life is something we must face,",
    "We walk the streets and search for meaning still,",
    "The lights of cities burn throughout the night,",
    "I feel the weight of choices that I made,",
    "The memories of childhood never fade,",
    "We build the things we hope will stand the test,",
    "The voice inside is telling me to wait,",
    "I wonder if the stars still know my name,",
    "The winter comes and stays a little while,",
    "A quiet thought runs slowly through my mind,",
    "We reach for what we cannot ever hold,",
    "The years have taught me more than I could ask,",
    "I look again and find I am the same,",
    "The questions linger long into the night,",
]

# Broken-archaic templates — grammatically wrong conjugations.
BROKEN_TEMPLATES: list[str] = [
    "Thou hast went to the store like yesterday.",
    "Doth not be sad, it is kind of okay.",
    "Prithee, could you maybe sort of help me out?",
    "Methinks like the weather is pretty bad today.",
    "Hark, basically the server is really down.",
    "Thou shalt tries to fix it, forsooth.",
    "Hast thou did that thing I asked about?",
    "Wherefore doth thou be sad about this?",
    "Verily, the code just ran kind of slow.",
    "Prithee be helping me with yon bug fix, basically.",
    "'Tis like, you know, kind of complicated.",
    "Hark, methinks like the app doth be crashing.",
    "Thou art goes to work every single morning.",
    "Forsooth, I just kind of ran the tests basically.",
    "Thy phone hath got really low battery now.",
    "Doth not thou wants the truth about it?",
]


# =============================================================================
# Tokenization + substitution helpers
# =============================================================================
# Case-preserving word substitution. We iterate word-by-word so that capitalized
# forms (Thou -> You) are preserved.
_WORD_RE = re.compile(r"\b[A-Za-z']+\b")


def _map_case(src: str, dst: str) -> str:
    if src.isupper():
        return dst.upper()
    if src[0:1].isupper():
        return dst[0:1].upper() + dst[1:]
    return dst


def _substitute_words(text: str, table: dict[str, str]) -> str:
    def _sub(m: re.Match) -> str:
        w = m.group(0)
        lw = w.lower()
        if lw in table:
            return _map_case(w, table[lw])
        return w
    return _WORD_RE.sub(_sub, text)


def _replace_contractions(text: str) -> str:
    for src, dst in ARCHAIC_CONTRACTIONS.items():
        # match as whole token, case-insensitive
        pattern = re.compile(r"(?<![A-Za-z'])" + re.escape(src) + r"(?![A-Za-z])",
                             re.IGNORECASE)
        text = pattern.sub(dst, text)
    return text


def _uninvert_syntax(text: str) -> str:
    for pat, repl in INVERSION_PATTERNS:
        text = pat.sub(repl, text)
    return text


def _inject_fillers(text: str, rng: random.Random) -> str:
    sentences = re.split(r"(?<=[.!?])\s+", text)
    out = []
    for s in sentences:
        if len(s.split()) >= 6 and rng.random() < 0.55:
            words = s.split()
            pos = rng.randint(1, len(words) - 1)
            words.insert(pos, rng.choice(MODERN_FILLERS))
            s = " ".join(words)
        out.append(s)
    return " ".join(out)


def _flatten_clauses(text: str) -> str:
    """Split long subordinate clauses into short sentences."""
    # Replace semicolons and comma-conjunction patterns with periods.
    text = re.sub(r";\s*", ". ", text)
    # Convert clause-heavy commas after long clauses into periods.
    text = re.sub(r",\s+(and|but|yet|though|whilst|while|for|so)\s+",
                  lambda m: ". " + m.group(1).capitalize() + " ",
                  text, flags=re.IGNORECASE)
    return text


def _count_archaic_markers(text: str) -> int:
    words = [w.lower().strip(".,;:!?\"'()—–-") for w in text.split()]
    markers = set(ARCHAIC_TO_MODERN.keys()) | set(ARCHAIC_CONTRACTIONS.keys())
    return sum(1 for w in words if w in markers)


def _archaic_density(text: str) -> float:
    words = [w for w in text.split() if w]
    if not words:
        return 0.0
    return _count_archaic_markers(text) / len(words)


# =============================================================================
# Variant generators
# =============================================================================
def make_modern_paraphrase(text: str, rng: random.Random) -> str | None:
    """Aggressive modern paraphrase. Returns None if the text has no archaic markers
    to remove (we don't want a no-op rejected that looks like the chosen)."""
    original_markers = _count_archaic_markers(text)
    if original_markers == 0:
        return None
    out = _replace_contractions(text)
    out = _substitute_words(out, ARCHAIC_TO_MODERN)
    out = _uninvert_syntax(out)
    out = _flatten_clauses(out)
    out = _inject_fillers(out, rng)
    # Normalize whitespace & drop double-punct from clause flattening.
    out = re.sub(r"\s+", " ", out).strip()
    out = re.sub(r"\.\s*\.", ".", out)
    # If substitution still left it archaic, force drop.
    if _archaic_density(out) > 0.02:
        out = _substitute_words(out, ARCHAIC_TO_MODERN)
    return out


def make_archaic_pastiche(text: str, rng: random.Random,
                          target_density: float = 0.075) -> str | None:
    """Pastiche: has *some* archaic vocab but flat syntax, no meter, no devices.
    Simulates an LLM that learned words-not-style. Drops ~50%% of archaic markers
    and expresses content in short, simple sentences."""
    # Rebuild: split into sentences, re-express each one flatly.
    sentences = [s for s in re.split(r"(?<=[.!?])\s+", text) if s.strip()]
    if not sentences:
        return None

    out_sents: list[str] = []
    for s in sentences:
        # Remove all line breaks; pastiche is prose, no meter.
        s = re.sub(r"\s+", " ", s).strip()
        if not s:
            continue
        # Chop long clauses.
        parts = re.split(r"[,;:]\s+", s)
        for p in parts:
            p = p.strip()
            if not p:
                continue
            # Ensure every sub-clause terminates with a period.
            if p[-1] not in ".!?":
                p += "."
            # Drop ~50%% of archaic markers by mapping them to modern.
            p = _partial_modernize(p, rng, drop_prob=0.5)
            # Break further if still long.
            if len(p.split()) > 14:
                mid = len(p.split()) // 2
                tokens = p.split()
                p = " ".join(tokens[:mid]) + ". " + " ".join(tokens[mid:])
                if p[-1] not in ".!?":
                    p += "."
            out_sents.append(p)

    out = " ".join(out_sents)
    out = re.sub(r"\s+", " ", out).strip()
    out = re.sub(r"\.\s*\.", ".", out)

    # Check density target; if too high, drop more archaic markers.
    d = _archaic_density(out)
    if d > target_density + 0.03:
        out = _partial_modernize(out, rng, drop_prob=0.6)
    # Must still have SOME archaic flavor to qualify as "pastiche".
    if _count_archaic_markers(out) == 0:
        return None
    return out


def _partial_modernize(text: str, rng: random.Random, drop_prob: float) -> str:
    """Replace each archaic marker with its modern equivalent with probability
    drop_prob. Preserves capitalization."""
    def _sub(m: re.Match) -> str:
        w = m.group(0)
        lw = w.lower()
        if lw in ARCHAIC_TO_MODERN and rng.random() < drop_prob:
            return _map_case(w, ARCHAIC_TO_MODERN[lw])
        return w
    out = _WORD_RE.sub(_sub, text)
    # Contractions: drop with same prob.
    for src, dst in ARCHAIC_CONTRACTIONS.items():
        pat = re.compile(r"(?<![A-Za-z'])" + re.escape(src) + r"(?![A-Za-z])",
                         re.IGNORECASE)
        def _csub(m: re.Match) -> str:
            return dst if rng.random() < drop_prob else m.group(0)
        out = pat.sub(_csub, out)
    return out


def make_meter_modern(text: str, meter_form: str, rng: random.Random) -> str | None:
    """Meter-correct modern variant. Only valid when the chosen is metrical verse
    (blank_verse / couplet / sonnet). For prose/mixed, return None."""
    if meter_form not in ("blank_verse", "couplet", "sonnet"):
        return None
    lines = [ln for ln in text.splitlines() if ln.strip()]
    # Target same line-count as original (up to 14, the sonnet cap).
    line_count = max(2, min(len(lines), 14))
    rng2 = random.Random(rng.randrange(1 << 30))
    templates = rng2.sample(METER_MODERN_TEMPLATES,
                            k=min(line_count, len(METER_MODERN_TEMPLATES)))
    # If we need more lines than templates, top up by re-shuffling.
    while len(templates) < line_count:
        templates.append(rng2.choice(METER_MODERN_TEMPLATES))
    out = "\n".join(templates[:line_count])
    # Sanity: meter-modern MUST have zero archaic markers (it's testing vocab-as-style).
    if _count_archaic_markers(out) > 0:
        return None
    return out


def make_broken_archaic(text: str, rng: random.Random) -> str | None:
    """Broken shakespeare: archaic markers misused, mixed with modern fillers.
    Sampled from BROKEN_TEMPLATES, optionally stitched with a modernized
    fragment from the original for length matching."""
    target_lines = max(1, min(len(text.splitlines()), 6))
    picks = [rng.choice(BROKEN_TEMPLATES) for _ in range(target_lines)]
    out = " ".join(picks)
    # Ensure at least one archaic marker is present (the templates have them).
    if _count_archaic_markers(out) == 0:
        return None
    return out


# =============================================================================
# SFT + tag loading
# =============================================================================
# PATCH P2-2 (S3 C-1): streaming generator replaces `load_sft_records` list
# materializer. Previously built a ~200 MB in-memory list of all 7,925
# records; now yields one record at a time and never retains any.
def iter_sft_records(path: Path, limit: int | None,
                     sft_start: int = 0, sft_end: int | None = None) -> Iterator[dict]:
    """Yield one cleaned SFT record at a time, optionally sliced by [sft_start, sft_end).

    Drops records with bad message structure; caps at `limit` records produced.
    Never holds more than one record in memory.

    sft_start / sft_end (zero-based line indices) enable partitioning the corpus
    across parallel workers: worker 0 gets 0..2000, worker 1 gets 2000..4000, etc.
    Filtering happens AFTER index match so each worker gets its contiguous slice.
    """
    produced = 0
    with path.open() as f:
        for i, line in enumerate(f):
            # Slice filter — only process records in [sft_start, sft_end)
            if i < sft_start:
                continue
            if sft_end is not None and i >= sft_end:
                break
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            msgs = obj.get("messages") or []
            if len(msgs) < 3:
                continue
            if msgs[0].get("role") != "system":
                continue
            if msgs[1].get("role") != "user":
                continue
            if msgs[-1].get("role") != "assistant":
                continue
            user = msgs[1].get("content") or ""
            chosen = msgs[-1].get("content") or ""
            system = msgs[0].get("content") or ""
            if not user.strip() or not chosen.strip():
                continue
            yield {
                "sft_index": i,
                "system": system,
                "prompt": user,
                "chosen": chosen,
                "_meta": obj.get("_meta") or {},
            }
            produced += 1
            if limit is not None and produced >= limit:
                return


def load_tag_index(path: Path | None) -> dict[str, dict]:
    """Load tags keyed by SHA-1 of the chosen text. Phase B writes records with
    `text` = the original Shakespeare passage. If the file is missing OR the
    CLI caller skipped it (None), we return an empty dict and use register
    defaults downstream — DPO still builds, just without register-aware
    variant customization.

    PATCH P2-4 (S3 C-2): only retain the three fields actually consumed by
    `build_pair_record` — `register.primary`, `meter.form`, `semantic_topic`.
    Full tag records (~2 KB each × 7,925) would occupy ~16 MB; this trimmed
    shape is ~200 bytes each → ~1.6 MB. No behavior change (callers read via
    the shaped keys only).
    """
    if not path or not path.exists():
        return {}
    idx: dict[str, dict] = {}
    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            text = rec.get("text") or ""
            if not text:
                continue
            key = hashlib.sha1(text.encode()).hexdigest()
            register_section = rec.get("register") or {}
            meter_section = rec.get("meter") or {}
            idx[key] = {
                "register": {"primary": register_section.get("primary")},
                "meter":    {"form":    meter_section.get("form")},
                "semantic_topic": rec.get("semantic_topic"),
            }
    return idx


def text_key(s: str) -> str:
    return hashlib.sha1(s.encode()).hexdigest()


# =============================================================================
# Daemon scoring wrapper — fail-fast with error budget
# =============================================================================
class DaemonStyleScorer:
    """Store → score_against_constellation → forget_concept. Tracks a global
    success/failure counter to enforce the 1%% error budget."""

    def __init__(self, rpc: Rpc):
        self.rpc = rpc
        self.calls_ok = 0
        self.calls_failed = 0
        # For audit: forget_concept count so caller can cross-check.
        self.forgets_ok = 0
        self.forgets_failed = 0
        # PATCH P2-5 (S3 C-3): budget-exceeded flag replaces `raise SystemExit`.
        # Main loop checks this flag and breaks cleanly so the final state
        # checkpoint + completion marker still run.
        self._budget_exceeded = False

    def score(self, text: str, tag: str) -> tuple[dict, str]:
        """Store text, score it, return (score_info, fingerprint_id).

        Raises RuntimeError on any daemon error (counted toward budget).
        Caller MUST call forget(fp_id) in a try/finally.
        """
        try:
            s = self.rpc.call("store_memory", {
                "content": text,
                "sessionId": "dpo-style-pairs",
                "importance": 0.3,
                "rationale": f"dpo-pair-{tag}"[:1024],
            }, timeout=60.0)
            fp = s.get("fingerprintId")
            if not fp:
                self.calls_failed += 1
                raise RuntimeError(f"store_memory returned no fingerprintId: {s}")
        except Exception as e:
            self.calls_failed += 1
            raise RuntimeError(f"store_memory failed ({tag}): {type(e).__name__}: {e}") from e

        try:
            r = self.rpc.call("score_against_constellation", {
                "constellationId": SONNETS_CONSTELLATION_ID,
                "memoryId": fp,
            }, timeout=60.0)
        except Exception as e:
            self.calls_failed += 1
            # still return fp so caller can forget it
            raise RuntimeError(
                f"score_against_constellation failed ({tag}): {type(e).__name__}: {e}"
            ) from e

        per = r.get("per_embedder_cosine")
        if not isinstance(per, list) or len(per) != 13:
            self.calls_failed += 1
            raise RuntimeError(
                f"score response malformed ({tag}): {r}"
            )

        style_score = 0.0
        for idx, w in STYLE_EMBEDDER_WEIGHTS.items():
            style_score += w * max(0.0, float(per[idx]))
        self.calls_ok += 1
        return {
            "style_score": round(style_score, 4),
            "combined_score": round(float(r.get("combined_score") or 0.0), 4),
            "per_embedder_cosine": [round(float(x), 4) for x in per],
            "fingerprint_id": fp,
        }, fp

    def forget(self, fp_id: str) -> None:
        try:
            self.rpc.call("forget_concept", {
                "node_id": fp_id,
                "reason": "dpo-style-pair cleanup",
            }, timeout=30.0)
            self.forgets_ok += 1
        except Exception as e:
            # forget failures are counted but never abort (we still log for audit).
            self.forgets_failed += 1
            print(f"[warn] forget_concept({fp_id}) failed: {type(e).__name__}: {e}",
                  file=sys.stderr)

    @property
    def total_calls(self) -> int:
        return self.calls_ok + self.calls_failed

    @property
    def failure_rate(self) -> float:
        t = self.total_calls
        return 0.0 if t == 0 else self.calls_failed / t


# =============================================================================
# Pair generation orchestration
# =============================================================================
VARIANT_KINDS = ("modern_paraphrase", "archaic_pastiche", "meter_modern", "broken_archaic")


def generate_variants(chosen: str, tag: dict | None, rng: random.Random) -> list[tuple[str, str]]:
    """Return a list of (variant_kind, text) tuples. Skips variants that return None."""
    out: list[tuple[str, str]] = []

    mp = make_modern_paraphrase(chosen, rng)
    if mp and mp.strip() and mp.strip() != chosen.strip():
        out.append(("modern_paraphrase", mp))

    ap = make_archaic_pastiche(chosen, rng)
    if ap and ap.strip() and ap.strip() != chosen.strip():
        out.append(("archaic_pastiche", ap))

    meter_form = ((tag or {}).get("meter") or {}).get("form") or "unknown"
    mm = make_meter_modern(chosen, meter_form, rng)
    if mm and mm.strip() and mm.strip() != chosen.strip():
        out.append(("meter_modern", mm))

    ba = make_broken_archaic(chosen, rng)
    if ba and ba.strip() and ba.strip() != chosen.strip():
        out.append(("broken_archaic", ba))

    return out


def build_pair_record(prompt: str, chosen: str, rejected: str,
                      variant_kind: str, chosen_info: dict,
                      rejected_info: dict, tag: dict | None) -> dict:
    return {
        "prompt": prompt,
        "chosen": chosen,
        "rejected": rejected,
        "_meta": {
            "variant_type": variant_kind,
            "chosen_style_score": chosen_info["style_score"],
            "rejected_style_score": rejected_info["style_score"],
            "score_gap": round(chosen_info["style_score"] - rejected_info["style_score"], 4),
            "chosen_combined": chosen_info["combined_score"],
            "rejected_combined": rejected_info["combined_score"],
            "tags": {
                "register": ((tag or {}).get("register") or {}).get("primary"),
                "meter_form": ((tag or {}).get("meter") or {}).get("form"),
                "semantic_topic": (tag or {}).get("semantic_topic"),
            },
        },
    }


# =============================================================================
# State management (resume support)
# =============================================================================
def load_state(state_path: Path) -> dict:
    if not state_path.exists():
        return {"processed_sft_indices": [], "pairs_written": 0,
                "rejected_pairs": 0, "last_checkpoint_ts": None}
    try:
        return json.loads(state_path.read_text())
    except Exception:
        return {"processed_sft_indices": [], "pairs_written": 0,
                "rejected_pairs": 0, "last_checkpoint_ts": None}


def save_state(state_path: Path, state: dict) -> None:
    tmp = state_path.with_suffix(state_path.suffix + ".tmp")
    tmp.write_text(json.dumps(state))
    os.replace(tmp, state_path)


def output_is_complete(output_path: Path) -> bool:
    """True iff the output file exists AND ends with COMPLETION_MARKER."""
    if not output_path.exists():
        return False
    try:
        size = output_path.stat().st_size
        with output_path.open("rb") as f:
            if size > 4096:
                f.seek(size - 4096)
            tail = f.read().decode("utf-8", errors="ignore")
        return COMPLETION_MARKER in tail
    except Exception:
        return False


# =============================================================================
# Main
# =============================================================================
def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Build style-contrastive DPO pairs")
    p.add_argument("--sft-input", type=str, required=True)
    p.add_argument("--tags-input", type=str, default=None,
                   help="Phase B stylistic tags JSONL (optional but recommended)")
    p.add_argument("--output", type=str, required=True)
    p.add_argument("--target-count", type=int, default=30000,
                   help="Stop once this many pairs have been written (0 = no cap).")
    p.add_argument("--sft-start", type=int, default=0,
                   help="Zero-based starting SFT line index (inclusive) for parallel worker partitioning")
    p.add_argument("--sft-end", type=int, default=None,
                   help="Zero-based ending SFT line index (exclusive). Combine with --sft-start for worker slices.")
    p.add_argument("--limit-sft", type=int, default=None,
                   help="Limit how many SFT records are processed (for testing).")
    p.add_argument("--seed", type=int, default=2026_04_17)
    return p.parse_args()


def main() -> int:
    args = parse_args()
    sft_path = Path(os.path.expanduser(args.sft_input)).resolve()
    out_path = Path(os.path.expanduser(args.output)).resolve()
    tags_path = Path(os.path.expanduser(args.tags_input)).resolve() if args.tags_input else None
    state_path = out_path.with_suffix(out_path.suffix + ".state")
    partial_path = out_path.with_suffix(out_path.suffix + ".partial")

    if not sft_path.is_file():
        print(f"[fatal] SFT input missing: {sft_path}", file=sys.stderr)
        return 1

    # Idempotency: completed run => exit 0 immediately.
    if output_is_complete(out_path):
        print(f"[done] output already complete: {out_path}")
        return 0

    out_path.parent.mkdir(parents=True, exist_ok=True)
    REJECTED_LOG_PATH.parent.mkdir(parents=True, exist_ok=True)

    # PATCH P2-2 (S3 C-1): stream SFT records instead of materializing the full
    # list. Size of JSONL file already logged for operator reference.
    print(f"[load] sft={sft_path} (limit={args.limit_sft}) — streaming")

    tags_idx = load_tag_index(tags_path)
    print(f"[load] tags={len(tags_idx)} records from {tags_path or '(none)'}")

    state = load_state(state_path)
    processed = set(state.get("processed_sft_indices", []))
    pairs_written = state.get("pairs_written", 0)
    rejected_pairs = state.get("rejected_pairs", 0)
    variant_counter: Counter = Counter()
    kept_style_gaps: list[float] = []

    # Open partial in append mode (so resume continues from last flush).
    partial_f = partial_path.open("a", encoding="utf-8")
    rejected_log_f = REJECTED_LOG_PATH.open("a", encoding="utf-8")

    rpc = Rpc()
    scorer = DaemonStyleScorer(rpc)
    rng_master = random.Random(args.seed)

    t_start = time.time()
    try:
        for rec in iter_sft_records(sft_path, args.limit_sft,
                                     sft_start=args.sft_start,
                                     sft_end=args.sft_end):
            # PATCH P2-5 (S3 C-3): honor the budget-exceeded flag between
            # records so state save + completion marker still execute.
            if scorer._budget_exceeded:
                print(f"[abort] daemon failure budget exceeded; stopping loop "
                      f"to persist checkpoint. ok={scorer.calls_ok} "
                      f"failed={scorer.calls_failed}", file=sys.stderr)
                break
            if rec["sft_index"] in processed:
                continue
            if args.target_count > 0 and pairs_written >= args.target_count:
                print(f"[done] target-count {args.target_count} reached")
                break

            chosen = rec["chosen"]
            prompt = rec["prompt"]
            tag = tags_idx.get(text_key(chosen))
            rng = random.Random(rng_master.randrange(1 << 30))

            # ---------- score CHOSEN once (reused for all variants) ----------
            chosen_info: dict | None = None
            chosen_fp: str | None = None
            try:
                chosen_info, chosen_fp = scorer.score(chosen, "chosen")
            except RuntimeError as e:
                print(f"[daemon] chosen score failed at sft_index={rec['sft_index']}: {e}",
                      file=sys.stderr)
                if chosen_fp:
                    scorer.forget(chosen_fp)
                # bookkeep sft_index so we don't spin on it, but also enforce budget
                processed.add(rec["sft_index"])
                _maybe_abort_on_budget(scorer)
                continue

            try:
                if chosen_info["style_score"] < CHOSEN_MIN_STYLE_SCORE:
                    rejected_log_f.write(json.dumps({
                        "reason": "chosen_below_threshold",
                        "sft_index": rec["sft_index"],
                        "chosen_style_score": chosen_info["style_score"],
                        "chosen_len": len(chosen),
                    }) + "\n")
                    rejected_pairs += 1
                    processed.add(rec["sft_index"])
                    continue

                # ---------- generate + score variants ----------
                variants = generate_variants(chosen, tag, rng)
                for kind, variant_text in variants:
                    if args.target_count > 0 and pairs_written >= args.target_count:
                        break
                    rejected_info: dict | None = None
                    rejected_fp: str | None = None
                    try:
                        rejected_info, rejected_fp = scorer.score(variant_text,
                                                                  f"rejected-{kind}")
                    except RuntimeError as e:
                        print(f"[daemon] rejected score failed "
                              f"sft_index={rec['sft_index']} kind={kind}: {e}",
                              file=sys.stderr)
                        if rejected_fp:
                            scorer.forget(rejected_fp)
                        _maybe_abort_on_budget(scorer)
                        continue

                    try:
                        gap = chosen_info["style_score"] - rejected_info["style_score"]
                        keep = (
                            chosen_info["style_score"] >= CHOSEN_MIN_STYLE_SCORE
                            and rejected_info["style_score"] <= REJECTED_MAX_STYLE_SCORE
                            and gap >= MIN_SCORE_GAP
                        )
                        if keep:
                            pair = build_pair_record(
                                prompt, chosen, variant_text, kind,
                                chosen_info, rejected_info, tag,
                            )
                            partial_f.write(json.dumps(pair) + "\n")
                            partial_f.flush()
                            pairs_written += 1
                            variant_counter[kind] += 1
                            kept_style_gaps.append(gap)
                        else:
                            rejected_pairs += 1
                            rejected_log_f.write(json.dumps({
                                "reason": "gap_or_threshold",
                                "sft_index": rec["sft_index"],
                                "variant_kind": kind,
                                "chosen_style_score": chosen_info["style_score"],
                                "rejected_style_score": rejected_info["style_score"],
                                "gap": round(gap, 4),
                            }) + "\n")
                    finally:
                        if rejected_fp:
                            scorer.forget(rejected_fp)

                processed.add(rec["sft_index"])
            finally:
                if chosen_fp:
                    scorer.forget(chosen_fp)

            # Periodic progress + state checkpoint.
            if pairs_written and pairs_written % 100 == 0:
                elapsed = time.time() - t_start
                rate = pairs_written / max(elapsed, 1e-9)
                avg_gap = sum(kept_style_gaps) / len(kept_style_gaps) if kept_style_gaps else 0.0
                print(f"[progress] pairs={pairs_written} rejected={rejected_pairs} "
                      f"rate={rate:.2f}/s avg_gap={avg_gap:.3f} "
                      f"variants={dict(variant_counter)} "
                      f"daemon_fail={scorer.failure_rate:.4%}",
                      file=sys.stderr)
                rejected_log_f.flush()
                state.update({
                    "processed_sft_indices": sorted(processed),
                    "pairs_written": pairs_written,
                    "rejected_pairs": rejected_pairs,
                    "last_checkpoint_ts": time.time(),
                })
                save_state(state_path, state)

    finally:
        # PATCH P2-5 (S3 C-3): ALWAYS save state, flush, and close handles, even
        # on exception or budget-exceeded break. Completion marker + atomic
        # rename only happens on clean completion (below).
        state.update({
            "processed_sft_indices": sorted(processed),
            "pairs_written": pairs_written,
            "rejected_pairs": rejected_pairs,
            "last_checkpoint_ts": time.time(),
        })
        try:
            save_state(state_path, state)
        except Exception as e:
            print(f"[warn] final state save failed: {e}", file=sys.stderr)
        try:
            rpc.close()
        except Exception:
            pass
        try:
            partial_f.close()
        except Exception:
            pass
        try:
            rejected_log_f.close()
        except Exception:
            pass

    # Append completion marker + atomic rename ONLY on clean completion
    # (i.e., budget not exceeded). On budget-exceeded, leave .partial behind
    # so the next run can resume from last state checkpoint.
    if not scorer._budget_exceeded:
        with partial_path.open("a", encoding="utf-8") as f:
            f.write(COMPLETION_MARKER)
        os.replace(partial_path, out_path)
    else:
        print(f"[abort] partial output preserved at {partial_path} for resume "
              f"(budget-exceeded; rerun to continue).", file=sys.stderr)

    elapsed = time.time() - t_start
    avg_gap = sum(kept_style_gaps) / len(kept_style_gaps) if kept_style_gaps else 0.0
    print("=" * 72)
    print(f"[done] pairs={pairs_written} rejected={rejected_pairs} "
          f"elapsed={elapsed:.1f}s rate={pairs_written/max(elapsed,1e-9):.2f}/s")
    print(f"[done] variants={dict(variant_counter)}")
    print(f"[done] avg_style_gap={avg_gap:.4f}")
    print(f"[done] daemon_calls ok={scorer.calls_ok} failed={scorer.calls_failed} "
          f"rate={scorer.failure_rate:.4%}")
    print(f"[done] forget_concept ok={scorer.forgets_ok} failed={scorer.forgets_failed}")
    print(f"[done] output={out_path}")
    print(f"[done] rejected_log={REJECTED_LOG_PATH} (appended)")
    print(f"[done] state={state_path}")
    return 0


def _maybe_abort_on_budget(scorer: DaemonStyleScorer) -> None:
    """Flag the scorer for graceful shutdown if daemon failure rate exceeds
    budget AND we've made at least 20 calls (avoid flagging on startup flap).

    PATCH P2-5 (S3 C-3): was `raise SystemExit(1)` — that bypassed the final
    checkpoint + atomic rename. Now sets `_budget_exceeded`; main loop checks
    the flag on every outer iteration and breaks cleanly so state.json and
    the completion marker stay consistent.
    """
    if scorer._budget_exceeded:
        return  # already flagged; don't re-log
    if scorer.total_calls < 20:
        return
    if scorer.failure_rate > DAEMON_FAIL_BUDGET:
        print(f"[fatal] daemon failure rate {scorer.failure_rate:.4%} "
              f"exceeds budget {DAEMON_FAIL_BUDGET:.2%} "
              f"(ok={scorer.calls_ok}, failed={scorer.calls_failed}). "
              f"Flagging for graceful shutdown after current record.",
              file=sys.stderr)
        scorer._budget_exceeded = True


if __name__ == "__main__":
    # PATCH P2-1 (S3 C-10/C-11): install SIGTERM/SIGINT + RSS cap + heartbeat.
    with managed_run(
        script_name="phase-c-dpo-builder",
        rss_cap_gb=14,
        heartbeat_path=Path("/tmp/shake_dpo_heartbeat.jsonl"),
    ):
        sys.exit(main())
