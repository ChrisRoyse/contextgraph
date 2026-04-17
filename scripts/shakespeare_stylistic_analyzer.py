#!/usr/bin/env python3
"""Shakespeare stylistic analyzer — Phase B of the Resurrection plan.

Reads `~/.contextgraph/training/shakespeare_sft_final_clean.jsonl`, analyzes
each Shakespeare passage (the assistant response) on meter, rhetorical devices,
register, vocabulary, sentence shape, semantic topic, and a 13-embedder style
profile vector from the Context Graph daemon. Emits one JSON record per line to
`~/.contextgraph/training/shakespeare_stylistic_tags.jsonl` exactly as locked in
`memory/decisions/coordinator--stylistic-tag-schema.md`.

Rules (from mission brief):
- NO fallbacks that mask failure. If the daemon is down at startup → exit 1.
- FAIL FAST on missing libs (pronouncing, spacy, en_core_web_sm) → exit 1.
- Per-record exceptions are logged + emitted with `analyzer_error`, do NOT
  crash the whole run.
- Idempotent: if output exists, verify count and resume or exit.
- Atomic: write .partial, rename on completion.
- Progress log every 250 records.
"""
from __future__ import annotations

# --- cupy block (must precede spacy import) --------------------------------
# cupy-cuda12x 14.0.1 raises AttributeError during module init which escapes
# spacy/compat.py's ImportError guard. We intercept the import so spacy
# cleanly falls back to CPU-only. See discoveries/...spacy-cupy-conflict.md
import sys as _sys


class _BlockCupy:
    def find_spec(self, name, path=None, target=None):
        if name == "cupy" or name.startswith("cupy."):
            raise ImportError("cupy disabled for CPU-only spacy")
        return None


_sys.meta_path.insert(0, _BlockCupy())
# ---------------------------------------------------------------------------

import argparse
import collections
import gc
import json
import os
import re
import socket
import sys
import threading
import time
import traceback
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterator

# PATCH P1-1 (S3 B-8/B-9): import shared resource guard (SIGTERM/SIGINT handlers,
# RSS cap, heartbeat). Resolved relative to this script's directory so `python3
# scripts/shakespeare_stylistic_analyzer.py` works from any cwd.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from shakespeare_resource_guard import managed_run  # noqa: E402

# ---- required third-party libraries (fail fast if missing) ----------------
_INSTALL_HINT = """
Missing required library. Install with:

    pip install pronouncing
    pip install https://github.com/explosion/spacy-models/releases/download/en_core_web_sm-3.8.0/en_core_web_sm-3.8.0-py3-none-any.whl
"""

try:
    import pronouncing
except ImportError as e:
    print(f"[analyzer] FATAL: pronouncing not installed: {e}{_INSTALL_HINT}",
          file=sys.stderr)
    sys.exit(1)

try:
    import spacy
except ImportError as e:
    print(f"[analyzer] FATAL: spacy not installed or import-broken: {e}{_INSTALL_HINT}",
          file=sys.stderr)
    sys.exit(1)

try:
    _NLP = spacy.load("en_core_web_sm")
except Exception as e:
    print(f"[analyzer] FATAL: spacy model 'en_core_web_sm' not installed: {e}{_INSTALL_HINT}",
          file=sys.stderr)
    sys.exit(1)

# ===========================================================================
# CONSTANTS
# ===========================================================================
ANALYZER_VERSION = "1.0"
ERROR_LOG_PATH = Path("/tmp/shake_analyzer_errors.log")

# Paths (absolute, expanduser-safe)
DEFAULT_INPUT = Path.home() / ".contextgraph" / "training" / "shakespeare_sft_final_clean.jsonl"
DEFAULT_OUTPUT = Path.home() / ".contextgraph" / "training" / "shakespeare_stylistic_tags.jsonl"

# Daemon
DEFAULT_DAEMON_HOST = "127.0.0.1"
DEFAULT_DAEMON_PORT = 3100
DEFAULT_CONSTELLATION_ID = "18ef2460-9ab6-44fc-8d7e-8d7513174ac8"  # Sonnets

# ------------- Archaic markers for vocabulary.archaic_density --------------
ARCHAIC_MARKERS = frozenset({
    "thee", "thou", "thy", "thine", "ye",
    "hast", "doth", "dost", "hath", "art",
    "wilt", "shalt", "wert", "canst", "didst", "shouldst", "couldst",
    "wouldst", "mayst", "mightst", "needst",
    "prithee", "forsooth", "hark", "wherefore", "methinks", "verily",
    "perchance", "alas", "betwixt", "anon", "unto", "ere", "oft",
    "whence", "hither", "thither", "whither", "yon", "yonder",
    "nay", "aye", "fie", "marry", "sooth", "pray",
    "twas", "tis", "twere", "twixt", "oer", "een", "neer", "tho", "thro",
    "'tis", "'twas", "'twere", "'twixt", "o'er", "e'en", "ne'er",
    "beseech", "beshrew", "zounds", "sirrah",
})

# ------------- Shakespeare neologisms (curated from OED / Crystal) ---------
# Each word is a known or strongly-attributed Shakespeare coinage.
SHAKESPEARE_NEOLOGISMS = frozenset({
    "addiction", "admirable", "assassination", "auspicious",
    "baseless", "barefaced", "bedazzled", "bedimmed", "belongings",
    "bloodstained", "bloody-minded", "besmirch", "bump",
    "castigate", "catlike", "champion", "cold-blooded", "compromise",
    "courtship", "critical", "dauntless", "dawn", "deafening",
    "dewdrop", "discontent", "disgraceful", "distasteful", "downstairs",
    "dwindle", "enthroned", "epileptic", "equivocal", "eventful",
    "excitement", "exposure", "eyeball", "eyesore", "fairyland",
    "fashionable", "fixture", "flawed", "fortune-teller", "frugal",
    "generous", "glow", "gnarled", "gossip", "green-eyed", "grovel",
    "gust", "hint", "hobnob", "hunchbacked", "hurried",
    "impartial", "inauspicious", "invulnerable", "jaded", "label",
    "laughable", "lonely", "luggage", "lustrous", "madcap", "majestic",
    "marketable", "metamorphize", "mimic", "misplaced", "monumental",
    "moonbeam", "mortifying", "multitudinous", "negotiate", "new-fangled",
    "obscene", "obsequiously", "ode", "olympian", "outbreak", "outdone",
    "outrageous", "overblown", "pageantry", "pander", "pedant",
    "pious", "premeditated", "priceless", "profitless", "promethean",
    "puking", "puppy-dog", "radiance", "rant", "rascal", "rival",
    "roadway", "sanctimonious", "savagery", "scuffle", "seamy", "shooting-star",
    "shudder", "silliness", "skim-milk", "soft-hearted", "stealthy",
    "submerged", "successful", "suspicious", "swagger", "tardiness",
    "time-honoured", "torture", "traditional", "tranquil", "undress",
    "unearthly", "uneducated", "unmitigated", "unreal", "unrivalled",
    "unsolicited", "vanish", "varied", "vulnerable", "watchdog",
    "well-behaved", "well-read", "whirligig", "worthless", "yelping",
    "zany",
})

# ------------- Classical allusions -----------------------------------------
# Case-insensitive substring match on these phrases.
CLASSICAL_ALLUSIONS = [
    "jove", "jupiter", "mars", "venus", "apollo", "diana",
    "hector", "achilles", "priam", "juno", "neptune", "ceres",
    "flora", "mercury", "cupid", "eros", "minerva", "proteus",
    "bacchus", "olympus", "parnassus", "styx", "elysium",
    "phoebus", "phaethon", "hecate", "saturn",
    "the muses", "fortune", "fates", "moirai",
]

# ------------- Register keyword classifier ---------------------------------
REGISTER_KEYWORDS = {
    "tragic":         ["death", "grave", "tomb", "woe", "tears", "tragedy",
                       "grief", "mourn", "lament", "perish", "bloody",
                       "slay", "sorrow", "weep", "dire"],
    "comic":          ["jest", "japes", "merry", "laugh", "fool", "quibble",
                       "prank", "wit", "folly", "sport", "mirth"],
    "philosophical":  ["know", "truth", "wisdom", "nature", "being", "mortal",
                       "exist", "cause", "reason", "soul", "mind", "question",
                       "seems", "essence"],
    "pleading":       ["beseech", "implore", "pray", "grant", "mercy", "pity",
                       "spare", "forgive", "entreat"],
    "commanding":     ["shall", "must", "command", "bid", "charge", "decree",
                       "obey", "hence", "begone", "hither"],
    "lyrical":        ["love", "beauty", "rose", "morn", "star", "fair",
                       "sweet", "dawn", "lark", "dove", "grace"],
    "bawdy":          ["flesh", "thigh", "lip", "bed", "maid", "wench",
                       "bawd", "stew", "knave"],
    "didactic":       ["teach", "learn", "know", "observe", "consider",
                       "mark", "heed", "behold", "take notice"],
    "dialectical":    ["yet", "however", "notwithstanding", "perchance", "or",
                       "either", "neither", "rather", "instead"],
}

SECONDARY_SUBTAGS = {
    "grief":         ["weep", "mourn", "grief", "tears", "sorrow", "lament"],
    "love":          ["love", "beloved", "fair", "sweet", "dear", "heart"],
    "rage":          ["rage", "wrath", "fury", "fire", "blood", "venge"],
    "jest":          ["jest", "laugh", "merry", "fool", "quibble"],
    "awe":           ["heaven", "divine", "majestic", "wondrous", "marvel"],
    "contemplation": ["seems", "methinks", "question", "ponder", "muse"],
    "command":       ["shall", "must", "command", "bid", "hence", "begone"],
}

# ------------- Antithesis pairs --------------------------------------------
OPPOSED_ADJECTIVES = {
    ("bold", "coward"), ("fair", "foul"), ("love", "hate"),
    ("life", "death"), ("light", "dark"), ("good", "evil"),
    ("heaven", "hell"), ("high", "low"), ("rise", "fall"),
    ("joy", "sorrow"), ("friend", "foe"), ("king", "beggar"),
    ("wisdom", "folly"), ("truth", "lie"), ("man", "woman"),
    ("sweet", "bitter"), ("day", "night"), ("young", "old"),
}
OPPOSED_LOOKUP = {}
for a, b in OPPOSED_ADJECTIVES:
    OPPOSED_LOOKUP.setdefault(a, set()).add(b)
    OPPOSED_LOOKUP.setdefault(b, set()).add(a)

ANTITHESIS_PATTERNS = [
    re.compile(r"\bnot\s+[\w\s,]{1,40}?\s+but\b", re.IGNORECASE),
    re.compile(r"\bbetter\s+[\w\s,]{1,40}?\s+than\b", re.IGNORECASE),
    re.compile(r"\bneither\s+[\w\s,]{1,40}?\s+nor\b", re.IGNORECASE),
    re.compile(r"\beither\s+[\w\s,]{1,40}?\s+or\b", re.IGNORECASE),
]

# ------------- Semantic topic keyword bank ---------------------------------
TOPIC_KEYWORDS = {
    "love":      ["love", "beloved", "heart", "affection", "passion", "fair",
                  "dear", "adore", "kiss"],
    "death":     ["death", "die", "grave", "tomb", "perish", "slain", "mortal",
                  "corpse", "coffin"],
    "fate":      ["fate", "destiny", "stars", "fortune", "doom", "ordained",
                  "providence"],
    "ambition":  ["ambition", "crown", "throne", "climb", "aspire", "glory",
                  "conquer", "rise"],
    "jealousy":  ["jealous", "envy", "suspicion", "betray", "cuckold", "rival"],
    "power":     ["power", "king", "rule", "sovereign", "command", "reign",
                  "subject", "decree"],
    "betrayal":  ["betray", "traitor", "false", "deceive", "treason",
                  "perfidy", "counterfeit"],
    "honor":     ["honor", "honour", "valor", "noble", "worthy", "virtue",
                  "reputation", "fame"],
    "nature":    ["nature", "tree", "flower", "wind", "sea", "earth", "sky",
                  "storm", "forest"],
    "time":      ["time", "hour", "day", "year", "age", "minute", "moment",
                  "ancient", "youth"],
}

# ------------- Abstract nouns for personification --------------------------
ABSTRACT_NOUNS = {
    "fate", "death", "honor", "honour", "time", "fortune", "nature",
    "love", "war", "peace", "justice", "mercy", "truth", "fame", "glory",
    "hope", "despair", "envy", "pride", "patience", "sorrow", "joy",
    "wrath", "fury", "night", "day", "heaven",
}

# ------------- Stop words for unique_word_count ---------------------------
STOP_WORDS = frozenset({
    "a", "an", "the", "and", "or", "but", "if", "then", "of", "to", "in",
    "on", "at", "by", "for", "with", "from", "as", "is", "am", "are", "was",
    "were", "be", "been", "being", "have", "has", "had", "do", "does", "did",
    "will", "would", "should", "could", "can", "may", "might", "must",
    "i", "you", "he", "she", "it", "we", "they", "me", "him", "her", "us",
    "them", "my", "your", "his", "their", "our", "this", "that", "these",
    "those", "what", "which", "who", "whom", "whose", "when", "where", "why",
    "how", "all", "any", "both", "each", "few", "more", "most", "other",
    "some", "such", "no", "nor", "not", "only", "own", "same", "so", "than",
    "too", "very", "s", "t",
})

# ------------- Inverted syntax subject pronouns ---------------------------
INVERSION_SUBJECTS = {"i", "thou", "he", "she", "we", "they", "ye", "you"}
INVERSION_VERBS = {
    "go", "come", "say", "see", "know", "think", "speak", "make",
    "hear", "stand", "sit", "fall", "rise", "walk", "run", "look",
    "live", "die", "love", "hate", "wilt", "shalt", "art", "dost",
    "doth", "hast", "hath", "say", "cry",
}

# ===========================================================================
# DAEMON CLIENT  (13-embedder constellation scoring)
# ===========================================================================


class DaemonError(RuntimeError):
    pass


class DaemonClient:
    """MCP stdio-over-TCP client. Mirrors DaemonScorer from
    shakespeare_chat_dashboard.py but trimmed to constellation-only use."""

    SOCKET_TIMEOUT_S = 30.0

    def __init__(self, host: str, port: int, constellation_id: str):
        self.host = host
        self.port = port
        self.constellation_id = constellation_id
        self._sock: socket.socket | None = None
        self._rid = 0
        self._lock = threading.Lock()

    # ---- transport ----
    def _reset(self) -> None:
        if self._sock is not None:
            try:
                self._sock.close()
            except Exception:
                pass
        self._sock = None

    def _connect(self) -> None:
        if self._sock is not None:
            return
        try:
            s = socket.create_connection((self.host, self.port),
                                         timeout=self.SOCKET_TIMEOUT_S)
        except (ConnectionRefusedError, OSError) as e:
            raise DaemonError(
                f"cannot connect to daemon {self.host}:{self.port}: "
                f"{type(e).__name__}: {e}"
            ) from e
        s.settimeout(self.SOCKET_TIMEOUT_S)
        self._sock = s
        try:
            self._raw_call("initialize", {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "shake-analyzer", "version": "1.0"},
            })
            self._sock.sendall((json.dumps({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {},
            }) + "\n").encode())
        except DaemonError:
            self._reset()
            raise

    def _raw_call(self, method: str, params: dict,
                  timeout: float | None = None) -> dict:
        if self._sock is None:
            raise DaemonError("socket not connected")
        self._rid += 1
        rid = self._rid
        tout = timeout or self.SOCKET_TIMEOUT_S
        try:
            self._sock.sendall((json.dumps({
                "jsonrpc": "2.0", "id": rid,
                "method": method, "params": params,
            }) + "\n").encode())
        except (BrokenPipeError, ConnectionResetError, OSError) as e:
            self._reset()
            raise DaemonError(
                f"send failed on {method}: {type(e).__name__}: {e}") from e
        buf = b""
        deadline = time.time() + tout
        while time.time() < deadline:
            try:
                chunk = self._sock.recv(65536)
            except socket.timeout as e:
                self._reset()
                raise DaemonError(f"recv timeout on {method} after {tout}s") from e
            except (ConnectionResetError, OSError) as e:
                self._reset()
                raise DaemonError(
                    f"recv failed on {method}: {type(e).__name__}: {e}") from e
            if not chunk:
                self._reset()
                raise DaemonError(f"daemon closed socket mid-{method}")
            buf += chunk
            while b"\n" in buf:
                line, _, rest = buf.partition(b"\n")
                buf = rest
                if not line.strip():
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError as e:
                    raise DaemonError(
                        f"{method} non-JSON line: {e} "
                        f"raw={line[:400]!r}") from e
                if obj.get("id") == rid:
                    return obj
        self._reset()
        raise DaemonError(f"{method} timed out after {tout}s")

    def _tool_call(self, name: str, args: dict,
                   timeout: float | None = None) -> dict:
        resp = self._raw_call("tools/call",
                              {"name": name, "arguments": args},
                              timeout)
        if "error" in resp:
            raise DaemonError(f"MCP error on {name}: {resp['error']}")
        result = resp.get("result")
        if not isinstance(result, dict):
            raise DaemonError(f"{name} missing result dict: {resp}")
        if result.get("isError"):
            raise DaemonError(f"MCP tool {name} isError=True: {result}")
        content = result.get("content")
        if not content or not isinstance(content, list) or \
                not content[0].get("text"):
            raise DaemonError(f"{name} missing content text: {result}")
        try:
            return json.loads(content[0]["text"])
        except json.JSONDecodeError as e:
            raise DaemonError(
                f"{name} content not JSON: {e} "
                f"raw={content[0]['text'][:400]!r}") from e

    # ---- public API ----
    def health_check(self) -> None:
        """Call initialize to verify daemon is actually listening+responding."""
        with self._lock:
            self._connect()

    def score_passage(self, text: str) -> list[float]:
        """Store → score → forget. Returns per_embedder_cosine (13 floats).

        Raises DaemonError on any problem. Always cleans up the temp
        fingerprint, even on failure.
        """
        with self._lock:
            self._connect()
            fp_id: str | None = None
            try:
                store = self._tool_call("store_memory", {
                    "content": text,
                    "sessionId": "analyzer",
                })
                fp_id = store.get("fingerprintId")
                if not fp_id:
                    raise DaemonError(
                        f"store_memory missing fingerprintId: {store}")
                score = self._tool_call("score_against_constellation", {
                    "constellationId": self.constellation_id,
                    "memoryId": fp_id,
                })
                per = score.get("per_embedder_cosine")
                if not isinstance(per, list) or len(per) != 13:
                    raise DaemonError(
                        f"expected 13-element per_embedder_cosine, got {score}")
                return [float(x) for x in per]
            finally:
                if fp_id:
                    try:
                        self._tool_call(
                            "forget_concept",
                            {"node_id": fp_id,
                             "reason": "analyzer cleanup"},
                            timeout=5.0)
                    except DaemonError as e:
                        # Don't raise — we still want the score if scoring
                        # succeeded. But log loudly: orphan guard memories
                        # must not accumulate.
                        print(f"[analyzer] WARN: forget_concept({fp_id}) "
                              f"failed: {e}", file=sys.stderr)

# ===========================================================================
# STYLISTIC ANALYSIS  (pure, no network)
# ===========================================================================


_WORD_RE = re.compile(r"[A-Za-z']+")
_SENT_SPLIT_RE = re.compile(r"[.!?]+(?:\s|$)")
_VOWEL_CLUSTER_RE = re.compile(r"[aeiouy]+", re.IGNORECASE)


def _tokens(text: str) -> list[str]:
    return [m.group(0).lower() for m in _WORD_RE.finditer(text)]


def _line_tokens(line: str) -> list[str]:
    return [m.group(0).lower() for m in _WORD_RE.finditer(line)]


def _lines_nonempty(text: str) -> list[str]:
    return [ln.rstrip() for ln in text.splitlines() if ln.strip()]


def _syllable_count(word: str) -> int:
    """Return CMU-dict syllable count if available, else vowel-cluster heuristic."""
    phones = pronouncing.phones_for_word(word.lower())
    if phones:
        return pronouncing.syllable_count(phones[0])
    # Fallback: vowel clusters (roughly)
    w = word.lower().rstrip("e")
    clusters = _VOWEL_CLUSTER_RE.findall(w)
    return max(1, len(clusters))


def _stress_pattern(word: str) -> str | None:
    """Return stress string (e.g. '010') if CMU has it, else None."""
    phones = pronouncing.phones_for_word(word.lower())
    if not phones:
        return None
    return pronouncing.stresses(phones[0])


def _line_syllables(line: str) -> int:
    return sum(_syllable_count(w) for w in _line_tokens(line))


def _line_iambic_match(line: str) -> tuple[bool, int]:
    """Return (is_iambic, syllable_count) for a single line.

    Iambic pentameter: 10 syllables with alternating unstressed-stressed
    pattern, tolerating 10±1 syllables and requiring >=60% of feet to
    match.
    """
    tokens = _line_tokens(line)
    if not tokens:
        return False, 0
    total_syll = 0
    stress_str = ""
    for w in tokens:
        total_syll += _syllable_count(w)
        stresses = _stress_pattern(w)
        if stresses:
            # Normalize: CMU uses 0/1/2 — treat 2 as stressed.
            stress_str += stresses.replace("2", "1")
        else:
            # Unknown word: fill with ? so we can still partially match.
            n = _syllable_count(w)
            stress_str += "?" * n
    if not (9 <= total_syll <= 11):
        return False, total_syll
    # Check alternating pattern: positions 1,3,5,7,9 should be stressed.
    n = len(stress_str)
    if n < 8:
        return False, total_syll
    matches = 0
    compared = 0
    for i, ch in enumerate(stress_str):
        if ch == "?":
            continue
        expected_stress = "1" if (i % 2 == 1) else "0"
        compared += 1
        if ch == expected_stress:
            matches += 1
    if compared == 0:
        return False, total_syll
    return (matches / compared) >= 0.60, total_syll


def _last_word_of_line(line: str) -> str | None:
    toks = _line_tokens(line)
    return toks[-1] if toks else None


def _rhyme_pair(a: str, b: str) -> bool:
    """True if words rhyme per CMU dict."""
    if not a or not b or a == b:
        return False
    rhymes_a = set(pronouncing.rhymes(a))
    return b in rhymes_a


def _rhyme_scheme(lines: list[str]) -> str | None:
    """Assign letters A, B, C... to rhyme classes; return e.g. 'abab cdcd ef ef gg'
    for sonnets. None if single line or no rhymes detected."""
    if len(lines) < 2:
        return None
    last_words = [_last_word_of_line(ln) or "" for ln in lines]
    # Build equivalence classes
    classes: list[str] = []  # letter per line
    next_letter_ord = ord("a")
    groups: list[set[str]] = []  # list of rhyme sets
    for w in last_words:
        if not w:
            classes.append("-")
            continue
        assigned = None
        for i, grp in enumerate(groups):
            if any(_rhyme_pair(w, other) for other in grp):
                assigned = i
                grp.add(w)
                break
        if assigned is None:
            groups.append({w})
            assigned = len(groups) - 1
        classes.append(chr(next_letter_ord + assigned) if assigned < 26
                       else "?")
    scheme = "".join(classes)
    # If every line is its own class and none rhyme, return None (blank verse)
    if len(set(scheme)) == len([c for c in scheme if c != "-"]):
        return None
    return scheme


def _analyze_meter(text: str) -> dict:
    lines = _lines_nonempty(text)
    if not lines:
        return {
            "form": "prose",
            "avg_syllables_per_line": 0.0,
            "stdev_syllables_per_line": 0.0,
            "iambic_pentameter_pct": 0.0,
            "rhyme_scheme": None,
            "ends_with_couplet": False,
        }
    syll_counts: list[int] = []
    iambic_hits = 0
    for ln in lines:
        is_iambic, syll = _line_iambic_match(ln)
        syll_counts.append(syll)
        if is_iambic:
            iambic_hits += 1
    avg_syll = sum(syll_counts) / len(syll_counts)
    if len(syll_counts) > 1:
        mean = avg_syll
        var = sum((c - mean) ** 2 for c in syll_counts) / len(syll_counts)
        stdev = var ** 0.5
    else:
        stdev = 0.0
    iambic_pct = iambic_hits / len(lines)

    # Rhyme scheme on final words
    scheme = _rhyme_scheme(lines) if len(lines) >= 2 else None

    # Couplet detection: last two lines rhyme
    ends_couplet = False
    if len(lines) >= 2:
        lw_last = _last_word_of_line(lines[-1])
        lw_prev = _last_word_of_line(lines[-2])
        if lw_last and lw_prev:
            ends_couplet = _rhyme_pair(lw_last, lw_prev)

    # Form classification
    form = _classify_form(lines, syll_counts, iambic_pct, scheme, ends_couplet)

    return {
        "form": form,
        "avg_syllables_per_line": round(avg_syll, 2),
        "stdev_syllables_per_line": round(stdev, 2),
        "iambic_pentameter_pct": round(iambic_pct, 3),
        "rhyme_scheme": scheme,
        "ends_with_couplet": ends_couplet,
    }


def _classify_form(lines: list[str], syll_counts: list[int],
                   iambic_pct: float, scheme: str | None,
                   ends_couplet: bool) -> str:
    """Decide: sonnet | blank_verse | couplet | prose | mixed."""
    n = len(lines)
    if n == 1:
        return "prose" if iambic_pct < 0.6 else "mixed"
    # pentameter line = 9-11 syllables
    pentameter_line = [9 <= c <= 11 for c in syll_counts]
    pent_pct = sum(pentameter_line) / n
    # Sonnet: 14 lines, mostly pentameter, ends with couplet, and
    # approximate sonnet rhyme scheme (ends_with_couplet is the strong marker).
    if n == 14 and pent_pct >= 0.8 and ends_couplet and iambic_pct >= 0.6:
        return "sonnet"
    # Couplet run: mostly pairs of rhyming lines and pentameter.
    if scheme:
        # Heuristic: majority of adjacent pairs rhyme with the partner
        pair_hits = 0
        pairs_total = n // 2
        for i in range(0, n - 1, 2):
            if scheme[i] == scheme[i + 1] and scheme[i] != "-":
                pair_hits += 1
        if pairs_total >= 2 and pair_hits / pairs_total >= 0.7 and pent_pct >= 0.6:
            return "couplet"
    # Blank verse: pentameter but mostly no rhyme scheme (or trivial)
    if pent_pct >= 0.7 and iambic_pct >= 0.5 and \
            (not scheme or len(set(scheme.replace("-", ""))) >= n * 0.8):
        return "blank_verse"
    # Prose: <70% pentameter
    if pent_pct < 0.7:
        return "prose"
    return "mixed"


# ---------------------------------------------------------------------------
# DEVICES
# ---------------------------------------------------------------------------


def _count_antithesis(text: str) -> int:
    count = 0
    for pat in ANTITHESIS_PATTERNS:
        count += len(pat.findall(text))
    # Opposed adjectives in same sentence
    for sent in _SENT_SPLIT_RE.split(text):
        toks = _line_tokens(sent)
        tok_set = set(toks)
        for w in tok_set:
            if w in OPPOSED_LOOKUP:
                if OPPOSED_LOOKUP[w] & tok_set:
                    count += 1
                    break  # count once per sentence
    return count


def _count_chiasmus(text: str) -> int:
    """ABBA pattern: find sentences whose 1st and last content words mirror
    across a comma (simple heuristic)."""
    count = 0
    for sent in _SENT_SPLIT_RE.split(text):
        if "," not in sent:
            continue
        parts = [p.strip() for p in sent.split(",") if p.strip()]
        if len(parts) != 2:
            continue
        a_toks = [w for w in _line_tokens(parts[0]) if w not in STOP_WORDS]
        b_toks = [w for w in _line_tokens(parts[1]) if w not in STOP_WORDS]
        if len(a_toks) < 2 or len(b_toks) < 2:
            continue
        # ABBA: a_first ≈ b_last AND a_last ≈ b_first
        if a_toks[0] == b_toks[-1] and a_toks[-1] == b_toks[0]:
            count += 1
    return count


def _count_anadiplosis(text: str) -> int:
    """Last word of clause N == first content word of clause N+1."""
    count = 0
    clauses = re.split(r"[;,.!?]\s+", text)
    for i in range(len(clauses) - 1):
        a = [w for w in _line_tokens(clauses[i]) if w not in STOP_WORDS]
        b = [w for w in _line_tokens(clauses[i + 1]) if w not in STOP_WORDS]
        if a and b and a[-1] == b[0]:
            count += 1
    return count


_POLYPTOTON_ROOTS = {
    "love":  {"love", "loves", "loved", "loving", "lover", "lovest"},
    "live":  {"live", "lives", "lived", "living", "liveth"},
    "die":   {"die", "dies", "died", "dying", "dieth"},
    "know":  {"know", "knows", "knew", "known", "knowing", "knowest"},
    "see":   {"see", "sees", "saw", "seen", "seeing", "seest"},
    "speak": {"speak", "speaks", "spoke", "spoken", "speaking", "speakest"},
    "give":  {"give", "gives", "gave", "given", "giving", "givest"},
    "sleep": {"sleep", "sleeps", "slept", "sleeping", "sleepest"},
    "fight": {"fight", "fights", "fought", "fighting", "fightest"},
    "kill":  {"kill", "kills", "killed", "killing", "killest"},
    "think": {"think", "thinks", "thought", "thinking", "thinkest"},
    "weep":  {"weep", "weeps", "wept", "weeping", "weepest"},
    "dream": {"dream", "dreams", "dreamt", "dreamed", "dreaming"},
}
_POLYPTOTON_WORD_TO_ROOT = {
    w: root for root, words in _POLYPTOTON_ROOTS.items() for w in words
}


def _count_polyptoton(text: str) -> int:
    count = 0
    for sent in _SENT_SPLIT_RE.split(text):
        hits: dict[str, set[str]] = {}
        for w in _line_tokens(sent):
            root = _POLYPTOTON_WORD_TO_ROOT.get(w)
            if root:
                hits.setdefault(root, set()).add(w)
        for root, forms in hits.items():
            if len(forms) >= 2:
                count += 1
    return count


def _count_epistrophe(text: str) -> int:
    """Repeated final word across 2+ adjacent clauses/sentences."""
    count = 0
    # Per sentence: break into clauses via ; or ,
    sents = _SENT_SPLIT_RE.split(text)
    prev_last = None
    prev_run = 0
    for s in sents:
        clauses = [c.strip() for c in re.split(r"[;,]", s) if c.strip()]
        for c in clauses:
            toks = [w for w in _line_tokens(c) if w not in STOP_WORDS]
            if not toks:
                continue
            last = toks[-1]
            if last == prev_last:
                prev_run += 1
                if prev_run == 1:
                    count += 1  # count the pair
            else:
                prev_run = 0
            prev_last = last
    return count


def _alliteration_per_line(text: str) -> float:
    lines = _lines_nonempty(text)
    if not lines:
        return 0.0
    total_hits = 0
    for ln in lines:
        toks = _line_tokens(ln)
        if len(toks) < 2:
            continue
        for i in range(len(toks) - 1):
            a, b = toks[i], toks[i + 1]
            if not a or not b:
                continue
            # Skip stop-word adjacencies
            if a in STOP_WORDS and b in STOP_WORDS:
                continue
            ca, cb = a[0], b[0]
            if ca == cb and ca.isalpha() and ca not in "aeiou":
                total_hits += 1
    return round(total_hits / len(lines), 2)


def _count_personification(text: str, doc) -> int:
    """Abstract noun followed by a verb implying agency."""
    count = 0
    tokens = list(doc)
    for i, tok in enumerate(tokens):
        if tok.text.lower() not in ABSTRACT_NOUNS:
            continue
        # Look for a verb within the next 3 tokens (skip aux)
        for j in range(i + 1, min(i + 4, len(tokens))):
            t = tokens[j]
            if t.pos_ == "VERB":
                count += 1
                break
            if t.pos_ in ("NOUN", "PROPN"):
                break
    return count


def _classical_allusions(text: str) -> list[str]:
    lower = text.lower()
    found: list[str] = []
    seen: set[str] = set()
    for name in CLASSICAL_ALLUSIONS:
        if name in lower and name not in seen:
            # Title-case single words, keep multi-word as-is-ish.
            display = name.title() if " " not in name else name.title()
            found.append(display)
            seen.add(name)
    return found


def _count_rhetorical_questions(text: str) -> int:
    count = 0
    for sent in re.split(r"(?<=[?!.])\s+", text):
        if "?" not in sent:
            continue
        low = sent.lower()
        if re.search(r"\b(wherefore|what|who|whence|why|how)\b", low):
            count += 1
    return count


def _analyze_devices(text: str, doc) -> dict:
    return {
        "antithesis":             _count_antithesis(text),
        "chiasmus":               _count_chiasmus(text),
        "anadiplosis":            _count_anadiplosis(text),
        "polyptoton":             _count_polyptoton(text),
        "epistrophe":             _count_epistrophe(text),
        "alliteration_per_line":  _alliteration_per_line(text),
        "personification":        _count_personification(text, doc),
        "classical_allusions":    _classical_allusions(text),
        "rhetorical_questions":   _count_rhetorical_questions(text),
    }


# ---------------------------------------------------------------------------
# REGISTER
# ---------------------------------------------------------------------------


def _register_classify(text: str) -> dict:
    low = text.lower()
    scores: dict[str, int] = {}
    for reg, kws in REGISTER_KEYWORDS.items():
        scores[reg] = sum(low.count(k) for k in kws)
    total = sum(scores.values())
    if total == 0:
        return {"primary": "philosophical", "secondary": None, "confidence": 0.0}
    primary = max(scores, key=lambda k: scores[k])
    conf = scores[primary] / total
    # Secondary sub-tag
    sec_scores = {
        sub: sum(low.count(k) for k in kws)
        for sub, kws in SECONDARY_SUBTAGS.items()
    }
    sec_total = sum(sec_scores.values())
    secondary = None
    if sec_total > 0:
        secondary = max(sec_scores, key=lambda k: sec_scores[k])
        if sec_scores[secondary] == 0:
            secondary = None
    return {
        "primary":    primary,
        "secondary":  secondary,
        "confidence": round(conf, 3),
    }


# ---------------------------------------------------------------------------
# VOCABULARY
# ---------------------------------------------------------------------------


_LATINATE_SUFFIXES = ("tion", "ment", "ence", "ance", "ity",
                      "ous", "eous", "ious", "able", "ible",
                      "sion", "ize", "ise", "ization", "isation")


def _vocabulary(text: str) -> dict:
    toks = _tokens(text)
    if not toks:
        return {
            "tier": "low",
            "archaic_density": 0.0,
            "latinate_density": 0.0,
            "anglo_saxon_density": 0.0,
            "unique_word_count": 0,
            "shakespeare_neologisms": [],
        }
    total = len(toks)
    archaic_hits = sum(1 for w in toks if w in ARCHAIC_MARKERS
                        or w.strip("'") in ARCHAIC_MARKERS)
    latinate_hits = sum(1 for w in toks
                        if any(w.endswith(suf) for suf in _LATINATE_SUFFIXES))
    archaic_density = archaic_hits / total
    latinate_density = latinate_hits / total
    # anglo_saxon = 1 - latinate (approximate; may double-count with archaic)
    anglo_density = max(0.0, 1.0 - latinate_density)
    neologisms = sorted({w for w in toks if w in SHAKESPEARE_NEOLOGISMS})
    uniq = {w for w in toks if w not in STOP_WORDS}
    if archaic_density > 0.15 and latinate_density > 0.10:
        tier = "high"
    elif archaic_density > 0.05:
        tier = "mid"
    else:
        tier = "low"
    return {
        "tier": tier,
        "archaic_density": round(archaic_density, 4),
        "latinate_density": round(latinate_density, 4),
        "anglo_saxon_density": round(anglo_density, 4),
        "unique_word_count": len(uniq),
        "shakespeare_neologisms": neologisms,
    }


# ---------------------------------------------------------------------------
# SENTENCE SHAPE
# ---------------------------------------------------------------------------


def _max_sbar_depth(doc) -> int:
    """Walk dependency trees; compute max depth of SBAR (subordinate) chains.

    Approach: for each token, count how many of its ancestors have dep in
    {ccomp, xcomp, advcl, relcl, acl, csubj}. The max such count is the
    subordination depth.

    SAFETY: we cap the upward walk at MAX_WALK tokens so a malformed parse
    (cycle or mutual self-head among sentence roots) can never hang us.
    """
    sub_deps = {"ccomp", "xcomp", "advcl", "relcl", "acl", "csubj",
                "csubjpass", "acl:relcl"}
    max_d = 0
    MAX_WALK = 40  # deeper than any realistic Shakespeare sentence
    for tok in doc:
        d = 0
        cur = tok
        steps = 0
        while steps < MAX_WALK:
            head = cur.head
            if head is None or head.i == cur.i:
                break
            if cur.dep_ in sub_deps:
                d += 1
            cur = head
            steps += 1
        if d > max_d:
            max_d = d
    return max_d


def _inverted_syntax_count(doc) -> int:
    """Count clauses where a finite verb appears before its subject.

    Heuristic via spaCy: find ROOT/conj verbs and check if their nsubj (or
    nsubjpass) token index is greater than the verb's own index.
    """
    count = 0
    for tok in doc:
        if tok.pos_ != "VERB" and tok.pos_ != "AUX":
            continue
        if tok.dep_ not in ("ROOT", "conj", "ccomp", "advcl", "relcl"):
            continue
        for child in tok.children:
            if child.dep_ in ("nsubj", "nsubjpass") and child.i > tok.i:
                count += 1
                break
    return count


def _ends_with_weight(doc) -> bool:
    """Last non-punct content token is a NOUN/PROPN/VERB (not pronoun/det)."""
    for tok in reversed(list(doc)):
        if tok.is_punct or tok.is_space:
            continue
        return tok.pos_ in ("NOUN", "PROPN", "VERB", "ADJ")
    return False


def _analyze_sentence_shape(text: str, doc) -> dict:
    sents = list(doc.sents)
    n_sent = max(1, len(sents))
    # Avg clauses per sentence: count commas + semicolons per sentence + 1
    total_clauses = 0
    for s in sents:
        comma_semicolon = sum(1 for t in s
                              if t.text in (",", ";"))
        total_clauses += comma_semicolon + 1
    avg_clauses = total_clauses / n_sent
    return {
        "sentence_count":            len(sents),
        "avg_clauses_per_sentence":  round(avg_clauses, 2),
        "max_subordination_depth":   _max_sbar_depth(doc),
        "inverted_syntax_count":     _inverted_syntax_count(doc),
        "ends_with_weight":          _ends_with_weight(doc),
    }


# ---------------------------------------------------------------------------
# SEMANTIC TOPIC
# ---------------------------------------------------------------------------


def _semantic_topic(text: str) -> str:
    low = text.lower()
    scores: dict[str, int] = {}
    for topic, kws in TOPIC_KEYWORDS.items():
        scores[topic] = sum(low.count(k) for k in kws)
    total = sum(scores.values())
    if total == 0:
        return "general"
    # find the max and check for ties
    max_score = max(scores.values())
    winners = [t for t, s in scores.items() if s == max_score]
    if len(winners) > 1:
        return "general"
    return winners[0]


# ===========================================================================
# RECORD ANALYSIS
# ===========================================================================


def _extract_passage(record: dict, idx: int) -> tuple[str, str]:
    """Return (memory_id, passage_text). Raises ValueError on bad shape.

    Schema: record["messages"] is a list of {role, content}. The Shakespeare
    passage is the LAST assistant message (usually messages[-1]). See
    discoveries/agent-04-coder-analyzer--messages-index-discrepancy.md.
    """
    messages = record.get("messages")
    if not isinstance(messages, list) or not messages:
        raise ValueError("missing or empty 'messages'")
    last = messages[-1]
    if not isinstance(last, dict) or last.get("role") != "assistant":
        raise ValueError(
            f"last message role is not 'assistant': {last.get('role')!r}")
    text = last.get("content")
    if not isinstance(text, str) or not text.strip():
        raise ValueError("assistant content is empty or not a string")
    meta = record.get("_meta") or {}
    mid = (meta.get("memory_id") or meta.get("fingerprintId")
           or meta.get("source_id") or f"sft-index-{idx}")
    return str(mid), text


def analyze_record(memory_id: str, text: str, source: str,
                   daemon: DaemonClient) -> dict:
    """Run all analyses. Returns a full tag record matching the schema."""
    doc = _NLP(text)
    char_count = len(text)
    word_count = len(_tokens(text))
    line_count = len(_lines_nonempty(text))

    meter      = _analyze_meter(text)
    devices    = _analyze_devices(text, doc)
    register   = _register_classify(text)
    vocabulary = _vocabulary(text)
    sentence   = _analyze_sentence_shape(text, doc)
    topic      = _semantic_topic(text)
    profile    = daemon.score_passage(text)  # may raise

    # PATCH P1-4 (S3 B-7): release spaCy Doc immediately after use so vocab/token
    # arrays don't linger through the daemon RPC call. Scoped `del` drops the
    # reference; periodic gc.collect() in the caller clears any cycles.
    del doc

    return {
        "memory_id": memory_id,
        "source": source,
        "text": text,
        "char_count": char_count,
        "word_count": word_count,
        "line_count": line_count,
        "meter": meter,
        "devices": devices,
        "register": register,
        "vocabulary": vocabulary,
        "sentence_shape": sentence,
        "semantic_topic": topic,
        "style_profile_vector": [round(float(x), 4) for x in profile],
        "analyzer_version": ANALYZER_VERSION,
        "analyzed_at": datetime.now(timezone.utc).isoformat(
            timespec="seconds").replace("+00:00", "Z"),
    }


def build_error_record(memory_id: str, text: str | None, source: str,
                       err_msg: str) -> dict:
    """Emit a minimal tag record when analysis failed, so the downstream
    pipeline can count/inspect broken records. analyzer_error is the ONLY
    required signal here."""
    return {
        "memory_id": memory_id,
        "source": source,
        "text": text if isinstance(text, str) else "",
        "char_count": len(text) if isinstance(text, str) else 0,
        "word_count": len(_tokens(text)) if isinstance(text, str) else 0,
        "line_count": len(_lines_nonempty(text)) if isinstance(text, str) else 0,
        "meter": None,
        "devices": None,
        "register": None,
        "vocabulary": None,
        "sentence_shape": None,
        "semantic_topic": None,
        "style_profile_vector": None,
        "analyzer_version": ANALYZER_VERSION,
        "analyzed_at": datetime.now(timezone.utc).isoformat(
            timespec="seconds").replace("+00:00", "Z"),
        "analyzer_error": err_msg,
    }


# ===========================================================================
# RESUME LOGIC
# ===========================================================================


def _count_lines(path: Path) -> int:
    if not path.exists():
        return 0
    with path.open("rb") as f:
        return sum(1 for _ in f)


# PATCH P1-3 (S3 B-2): rotate the error log if it has grown > 10 MB before a new run.
# WSL /tmp is tmpfs; unbounded growth across resume cycles pressures host RAM.
_ERROR_LOG_ROTATE_THRESHOLD_BYTES = 10 * 1024 * 1024  # 10 MB


def _rotate_error_log_if_large(path: Path) -> None:
    """If path exists and > 10MB, rename to path.old (overwriting prior .old)."""
    try:
        if not path.exists():
            return
        if path.stat().st_size <= _ERROR_LOG_ROTATE_THRESHOLD_BYTES:
            return
        rotated = path.with_suffix(path.suffix + ".old")
        if rotated.exists():
            rotated.unlink()
        os.replace(path, rotated)
        print(f"[analyzer] rotated error log {path} -> {rotated} "
              f"(> {_ERROR_LOG_ROTATE_THRESHOLD_BYTES // 1024 // 1024} MB)",
              file=sys.stderr)
    except OSError as e:
        # Rotation is best-effort; never abort the run on a rotate failure.
        print(f"[analyzer] WARN: error log rotation failed: {e}", file=sys.stderr)


def _maybe_resume(input_path: Path, output_path: Path) -> int:
    """Return the input index from which to resume.

    - If output doesn't exist: start at 0.
    - If output has == input record count: already complete.
    - If output has < input records: resume at that index.
    - If output has > input records: corrupt; abort.
    """
    n_in = _count_lines(input_path)
    n_out = _count_lines(output_path)
    if n_out == 0:
        return 0
    if n_out == n_in:
        print(f"[analyzer] output already complete ({n_out}/{n_in}). "
              f"No work to do.", file=sys.stderr)
        return -1  # sentinel: "done, exit 0"
    if n_out > n_in:
        print(f"[analyzer] FATAL: output has {n_out} records but input has "
              f"only {n_in}. Refusing to proceed. Delete {output_path} and "
              f"rerun to regenerate.", file=sys.stderr)
        sys.exit(1)
    print(f"[analyzer] resuming from index {n_out}/{n_in}", file=sys.stderr)
    return n_out


# ===========================================================================
# MAIN
# ===========================================================================


# PATCH P1-2 (S3 B-6/B-8): module-level holder so managed_run.on_shutdown can
# close the DaemonClient socket from the signal handler thread (main() creates
# the client as a local; signal handler has no access without this holder).
_DAEMON_HOLDER: dict[str, DaemonClient | None] = {"client": None}


def _shutdown_daemon_client() -> None:
    """Close the analyzer's daemon socket on shutdown (best-effort)."""
    client = _DAEMON_HOLDER.get("client")
    if client is not None:
        try:
            client._reset()
        except Exception:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Shakespeare stylistic analyzer (Phase B).")
    parser.add_argument("--input",  type=Path, default=DEFAULT_INPUT,
                        help="input JSONL (SFT clean file)")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT,
                        help="output JSONL (tag records)")
    parser.add_argument("--limit",  type=int, default=None,
                        help="process only the first N input records "
                             "(for smoke tests)")
    parser.add_argument("--daemon-host", default=DEFAULT_DAEMON_HOST)
    parser.add_argument("--daemon-port", type=int, default=DEFAULT_DAEMON_PORT)
    parser.add_argument("--constellation-id", default=DEFAULT_CONSTELLATION_ID)
    parser.add_argument("--progress-interval", type=int, default=250)
    args = parser.parse_args()

    input_path: Path = args.input.expanduser()
    output_path: Path = args.output.expanduser()
    if not input_path.exists():
        print(f"[analyzer] FATAL: input file missing: {input_path}",
              file=sys.stderr)
        return 1
    output_path.parent.mkdir(parents=True, exist_ok=True)

    source_label = input_path.stem  # e.g. "shakespeare_sft_final_clean"

    # ------------ resume logic ------------
    resume_at = _maybe_resume(input_path, output_path)
    if resume_at == -1:
        return 0

    # ------------ daemon startup check ------------
    daemon = DaemonClient(args.daemon_host, args.daemon_port,
                          args.constellation_id)
    # PATCH P1-2 (S3 B-6): publish to module holder so managed_run.on_shutdown
    # can close the socket cleanly from the signal handler.
    _DAEMON_HOLDER["client"] = daemon
    try:
        daemon.health_check()
    except DaemonError as e:
        print(f"[analyzer] FATAL: daemon unreachable at "
              f"{args.daemon_host}:{args.daemon_port}: {e}\n"
              f"Start the daemon before running this analyzer. NO FALLBACK.",
              file=sys.stderr)
        return 1

    # Smoke test: score one real passage to confirm constellation works
    try:
        probe = daemon.score_passage(
            "To be, or not to be, that is the question.")
        if not (isinstance(probe, list) and len(probe) == 13):
            print(f"[analyzer] FATAL: daemon probe returned unexpected shape: "
                  f"{probe}", file=sys.stderr)
            return 1
    except DaemonError as e:
        print(f"[analyzer] FATAL: daemon constellation probe failed: {e}",
              file=sys.stderr)
        return 1

    print(f"[analyzer] daemon OK at {args.daemon_host}:{args.daemon_port}; "
          f"probe returned 13-dim vector. Starting run.", file=sys.stderr)

    # ------------ stream input -> output ------------
    partial_path = output_path.with_suffix(output_path.suffix + ".partial")

    # In resume mode, first copy existing output to .partial so the atomic
    # rename at the end preserves resumed records.
    if resume_at > 0:
        import shutil
        shutil.copy2(output_path, partial_path)
        write_mode = "ab"
    else:
        write_mode = "wb"
        if partial_path.exists():
            partial_path.unlink()

    total_input = _count_lines(input_path)
    if args.limit is not None:
        total_input = min(total_input, args.limit)

    start = time.time()
    processed = 0
    errors = 0

    # PATCH P1-3 (S3 B-2): rotate error log before opening; use `with` block so the FD
    # is always released on exit, even under exception or SIGTERM.
    _rotate_error_log_if_large(ERROR_LOG_PATH)

    with ERROR_LOG_PATH.open("a", encoding="utf-8") as error_log, \
            input_path.open("r", encoding="utf-8") as fin, \
            partial_path.open(write_mode) as fout:
        error_log.write(f"\n=== analyzer run @ {datetime.now(timezone.utc).isoformat()} ===\n")
        error_log.flush()
        for idx, line in enumerate(fin):
            if idx < resume_at:
                continue
            if args.limit is not None and idx >= args.limit:
                break
            line = line.strip()
            if not line:
                continue
            # Parse record
            try:
                record = json.loads(line)
            except json.JSONDecodeError as e:
                errors += 1
                msg = f"record {idx}: JSON decode: {e}"
                error_log.write(msg + "\n")
                fout.write((json.dumps(
                    build_error_record(f"sft-index-{idx}", None,
                                       source_label,
                                       f"json_decode: {e}")
                ) + "\n").encode())
                fout.flush()
                continue
            # Extract passage
            try:
                mid, text = _extract_passage(record, idx)
            except ValueError as e:
                errors += 1
                msg = f"record {idx}: extract_passage: {e}"
                error_log.write(msg + "\n")
                fout.write((json.dumps(
                    build_error_record(f"sft-index-{idx}", None,
                                       source_label,
                                       f"extract: {e}")
                ) + "\n").encode())
                fout.flush()
                continue
            # Override source from meta if available
            rec_source = (record.get("_meta", {}).get("source")
                          or source_label)
            # Analyze
            try:
                out_rec = analyze_record(mid, text, rec_source, daemon)
                fout.write((json.dumps(out_rec, ensure_ascii=False)
                            + "\n").encode())
                fout.flush()
            except Exception as e:
                errors += 1
                tb = traceback.format_exc()
                error_log.write(f"record {idx} id={mid}: {e}\n{tb}\n")
                error_log.flush()
                fout.write((json.dumps(
                    build_error_record(mid, text, rec_source,
                                       f"{type(e).__name__}: {e}")
                ) + "\n").encode())
                fout.flush()
            processed += 1
            # PATCH P1-4 (S3 B-7): every 200 records, run gc.collect() to release
            # spaCy token arrays / Doc cycles that `del doc` alone can't reclaim.
            if processed % 200 == 0:
                gc.collect()
            if processed % args.progress_interval == 0:
                elapsed = time.time() - start
                avg_ms = (elapsed / processed) * 1000
                total_for_progress = total_input - resume_at
                pct = 100.0 * processed / max(1, total_for_progress)
                print(f"[analyzer] {processed}/{total_for_progress} "
                      f"({pct:.1f}%) — avg {avg_ms:.0f}ms/record "
                      f"— {errors} errors so far",
                      file=sys.stderr)

    # ------------ atomic rename ------------
    os.replace(partial_path, output_path)
    elapsed = time.time() - start
    print(f"[analyzer] done. Processed {processed} new records in "
          f"{elapsed:.1f}s. Errors: {errors}. "
          f"Output: {output_path} ({_count_lines(output_path)} total lines).",
          file=sys.stderr)
    if errors > 0:
        print(f"[analyzer] error log: {ERROR_LOG_PATH}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    # PATCH P1-1 (S3 B-8/B-9): wrap main() in managed_run so SIGTERM/SIGINT
    # install handlers that close the daemon socket, flush logs, and trip the
    # RSS cap instead of the kernel OOM-killer.
    with managed_run(
        script_name="phase-b-analyzer",
        rss_cap_gb=12,
        heartbeat_path=Path("/tmp/shake_analyzer_heartbeat.jsonl"),
        on_shutdown=_shutdown_daemon_client,
    ):
        sys.exit(main())
