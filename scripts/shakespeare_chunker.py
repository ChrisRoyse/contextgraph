#!/usr/bin/env python3
"""Parse the Project Gutenberg Complete Works of Shakespeare into
play-scene / sonnet / poem-chunk memories for ingestion.

Input : /home/cabdru/contextgraph/shakespear/complete_works.txt
Output: JSON-lines file, one memory per line. Fields:
    - title            (play/poem title, UPPER CASE)
    - slug             (kebab-case slug derived from title)
    - chunk_type       ("scene" | "sonnet" | "poem_chunk")
    - act              (Roman numeral, scenes only)
    - scene            (Roman numeral, scenes only)
    - sonnet_number    (1..154, sonnets only)
    - chunk_index      (integer, zero-based within the logical unit)
    - content          (raw text, trimmed)

Why these fields: they let downstream code group memories per play
(session_id or tag), extract act/scene metadata, and reconstruct the
original chunk order for a play — without any external mapping.

Hard rules:
- No mock content: every chunk contains real Gutenberg text.
- Fails fast on missing structural markers.
- Trims blank lines at chunk boundaries but preserves formatting inside.
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


# ----------------------------------------------------------------------------
# Title list from the TOC at the top of the Gutenberg file (lines ~50-85).
# Order matches the order plays appear in the file (TOC != body order).
# We use the TITLE SET to detect start markers; order is derived at runtime.
# ----------------------------------------------------------------------------
CANONICAL_TITLES = [
    # Longer poems (standalone)
    "THE SONNETS",
    "A LOVER'S COMPLAINT",
    "THE PASSIONATE PILGRIM",
    "THE PHOENIX AND THE TURTLE",
    "THE RAPE OF LUCRECE",
    "VENUS AND ADONIS",
    # Plays (alphabetical in TOC, but file order differs)
    "ALL'S WELL THAT ENDS WELL",
    "THE TRAGEDY OF ANTONY AND CLEOPATRA",
    "AS YOU LIKE IT",
    "THE COMEDY OF ERRORS",
    "THE TRAGEDY OF CORIOLANUS",
    "CYMBELINE",
    "THE TRAGEDY OF HAMLET, PRINCE OF DENMARK",
    "THE FIRST PART OF KING HENRY THE FOURTH",
    "THE SECOND PART OF KING HENRY THE FOURTH",
    "THE LIFE OF KING HENRY THE FIFTH",
    "THE FIRST PART OF HENRY THE SIXTH",
    "THE SECOND PART OF KING HENRY THE SIXTH",
    "THE THIRD PART OF KING HENRY THE SIXTH",
    "KING HENRY THE EIGHTH",
    "THE LIFE AND DEATH OF KING JOHN",
    "THE TRAGEDY OF JULIUS CAESAR",
    "THE TRAGEDY OF KING LEAR",
    "LOVE'S LABOUR'S LOST",
    "THE TRAGEDY OF MACBETH",
    "MEASURE FOR MEASURE",
    "THE MERCHANT OF VENICE",
    "THE MERRY WIVES OF WINDSOR",
    "A MIDSUMMER NIGHT'S DREAM",
    "MUCH ADO ABOUT NOTHING",
    "THE TRAGEDY OF OTHELLO, THE MOOR OF VENICE",
    "PERICLES, PRINCE OF TYRE",
    "KING RICHARD THE SECOND",
    "KING RICHARD THE THIRD",
    "THE TRAGEDY OF ROMEO AND JULIET",
    "THE TAMING OF THE SHREW",
    "THE TEMPEST",
    "THE LIFE OF TIMON OF ATHENS",
    "THE TRAGEDY OF TITUS ANDRONICUS",
    "TROILUS AND CRESSIDA",
    "TWELFTH NIGHT; OR, WHAT YOU WILL",
    "THE TWO GENTLEMEN OF VERONA",
    "THE TWO NOBLE KINSMEN",
    "THE WINTER'S TALE",
]

# Right-single-quote variant that appears in the Gutenberg file ("'" U+2019)
_APOSTROPHE_VARIANTS = [
    ("'", "\u2019"),
    ("\u2019", "'"),
]

POEM_TITLES = {
    "A LOVER'S COMPLAINT",
    "THE PASSIONATE PILGRIM",
    "THE PHOENIX AND THE TURTLE",
    "THE RAPE OF LUCRECE",
    "VENUS AND ADONIS",
}

SONNETS_TITLE = "THE SONNETS"

# Act/scene headers — matched only when line begins at column 0 (no leading space)
#   ACT I | ACT I. | ACT II. | ... possibly trailing whitespace
ACT_RE = re.compile(r"^ACT\s+([IVXLCDM]+)\.?\s*$", re.IGNORECASE)
# Scene headers inside plays follow patterns like:
#     SCENE I. Elsinore. A platform before the Castle
#     Scene I. Rossillon. A room in the Countess's palace.   ← mixed case also appears
# Case-insensitive to catch both UPPERCASE and Title-case forms in the same
# Gutenberg file (e.g. All's Well uses mixed-case).
SCENE_RE = re.compile(r"^SCENE\s+([IVXLCDM]+)\.?\s*(.*)$", re.IGNORECASE)
# Sonnet numeric headers are lines containing only a number, centered with spaces
SONNET_RE = re.compile(r"^\s+(\d{1,3})\s*$")
# Poem chunk size: ~50 lines keeps chunks coherent without fragmenting stanzas
POEM_CHUNK_LINES = 50
# Scene byte cap — guard against MAX_CONTENT_LENGTH (10_000 chars) in storage
MAX_CONTENT_CHARS = 9500


@dataclass
class Chunk:
    title: str
    slug: str
    chunk_type: str  # "scene" | "sonnet" | "poem_chunk"
    chunk_index: int
    content: str
    # Scenes-only:
    act: str | None = None
    scene: str | None = None
    # Sonnets-only:
    sonnet_number: int | None = None


def slugify(title: str) -> str:
    t = title.lower()
    for a, b in _APOSTROPHE_VARIANTS:
        t = t.replace(a, "")
    t = re.sub(r"[^a-z0-9]+", "-", t).strip("-")
    return t


def _normalize_title(line: str) -> str | None:
    """Return a canonical title if `line` matches one (handles curly-quote)."""
    stripped = line.strip()
    for title in CANONICAL_TITLES:
        variants = {title}
        for a, b in _APOSTROPHE_VARIANTS:
            variants.add(title.replace(a, b))
        if stripped in variants:
            return title
    return None


def _strip_gutenberg(lines: list[str]) -> list[str]:
    start_idx = None
    end_idx = None
    for i, line in enumerate(lines):
        if line.startswith("*** START OF"):
            start_idx = i + 1
        elif line.startswith("*** END OF"):
            end_idx = i
            break
    if start_idx is None or end_idx is None:
        raise ValueError("Project Gutenberg start/end markers not found")
    return lines[start_idx:end_idx]


def _collect_title_bounds(lines: list[str]) -> list[tuple[str, int]]:
    """Return list of (title, body_line_idx) in file order — first occurrence only.

    The TOC at the top of the file lists every title without body content,
    so we require the title to appear a SECOND time (in the body) to mark the
    start of that work. We skip TOC occurrences.
    """
    first_seen: dict[str, int] = {}
    body_start: list[tuple[str, int]] = []
    for i, line in enumerate(lines):
        title = _normalize_title(line)
        if title is None:
            continue
        if title not in first_seen:
            first_seen[title] = i
            continue
        body_start.append((title, i))
        # only the SECOND occurrence (body) matters; ignore further matches
        if title in {t for t, _ in body_start[:-1]}:
            body_start.pop()  # duplicate body match — keep the first body one
    # Preserve insertion order = file order
    return body_start


def _chunk_play(title: str, body: list[str]) -> list[Chunk]:
    """Split a play's body into scene-level chunks."""
    slug = slugify(title)
    chunks: list[Chunk] = []
    current_act: str | None = None
    pending_scene_header: tuple[str, str] | None = None  # (scene_num, scene_desc)
    buf: list[str] = []
    chunk_index = 0

    def flush():
        nonlocal buf, chunk_index, pending_scene_header
        if pending_scene_header is None:
            buf = []
            return
        content = "\n".join(buf).strip()
        if not content:
            buf = []
            pending_scene_header = None
            return
        # Cap content size — split into multiple chunks if too long
        for slice_start in range(0, len(content), MAX_CONTENT_CHARS):
            chunk = content[slice_start: slice_start + MAX_CONTENT_CHARS]
            chunks.append(Chunk(
                title=title,
                slug=slug,
                chunk_type="scene",
                chunk_index=chunk_index,
                content=chunk,
                act=current_act,
                scene=pending_scene_header[0],
            ))
            chunk_index += 1
        buf = []
        pending_scene_header = None

    for line in body:
        act_m = ACT_RE.match(line)
        if act_m:
            flush()
            current_act = act_m.group(1)
            continue
        scene_m = SCENE_RE.match(line)
        if scene_m:
            flush()
            pending_scene_header = (scene_m.group(1), scene_m.group(2).strip())
            # re-include the SCENE header line so the chunk carries its context
            buf.append(line.rstrip())
            continue
        if pending_scene_header is not None:
            buf.append(line.rstrip())
    flush()
    return chunks


def _chunk_sonnets(body: list[str]) -> list[Chunk]:
    """Split THE SONNETS into 154 individual sonnet memories."""
    chunks: list[Chunk] = []
    current: int | None = None
    buf: list[str] = []

    def flush():
        nonlocal buf, current
        if current is None:
            buf = []
            return
        content = "\n".join(buf).strip()
        if content:
            chunks.append(Chunk(
                title=SONNETS_TITLE,
                slug="sonnets",
                chunk_type="sonnet",
                chunk_index=current - 1,
                content=content,
                sonnet_number=current,
            ))
        buf = []
        current = None

    for line in body:
        # Sonnet numbers are centered standalone digits on their own line
        m = SONNET_RE.match(line)
        if m and line.rstrip().endswith(m.group(1)) and len(line.strip()) <= 4:
            flush()
            current = int(m.group(1))
            continue
        if current is not None:
            buf.append(line.rstrip())
    flush()
    return chunks


def _chunk_poem(title: str, body: list[str]) -> list[Chunk]:
    """Split a longer poem into POEM_CHUNK_LINES slices."""
    slug = slugify(title)
    non_blank = [ln for ln in body if ln.strip()]
    chunks: list[Chunk] = []
    for idx, start in enumerate(range(0, len(non_blank), POEM_CHUNK_LINES)):
        segment = "\n".join(non_blank[start: start + POEM_CHUNK_LINES]).strip()
        if not segment:
            continue
        chunks.append(Chunk(
            title=title,
            slug=slug,
            chunk_type="poem_chunk",
            chunk_index=idx,
            content=segment,
        ))
    return chunks


def parse_complete_works(path: Path) -> list[Chunk]:
    text = path.read_text(encoding="utf-8-sig")
    lines = text.splitlines()
    lines = _strip_gutenberg(lines)

    bounds = _collect_title_bounds(lines)
    if not bounds:
        raise ValueError("No canonical titles found in the body")

    # Append sentinel for the last work's end
    bounds_with_end: list[tuple[str, int, int]] = []
    for i, (title, start) in enumerate(bounds):
        end = bounds[i + 1][1] if i + 1 < len(bounds) else len(lines)
        bounds_with_end.append((title, start, end))

    all_chunks: list[Chunk] = []
    for title, start, end in bounds_with_end:
        body = lines[start + 1: end]  # skip the title header line itself
        if title == SONNETS_TITLE:
            all_chunks.extend(_chunk_sonnets(body))
        elif title in POEM_TITLES:
            all_chunks.extend(_chunk_poem(title, body))
        else:
            all_chunks.extend(_chunk_play(title, body))
    return all_chunks


def main() -> int:
    src = Path(sys.argv[1] if len(sys.argv) > 1 else
               "/home/cabdru/contextgraph/shakespear/complete_works.txt")
    dst = Path(sys.argv[2] if len(sys.argv) > 2 else
               "/tmp/shakespeare_chunks.jsonl")
    chunks = parse_complete_works(src)

    with dst.open("w") as f:
        for c in chunks:
            f.write(json.dumps(asdict(c)) + "\n")

    # Human-readable summary
    by_type: dict[str, int] = {}
    by_title: dict[str, int] = {}
    for c in chunks:
        by_type[c.chunk_type] = by_type.get(c.chunk_type, 0) + 1
        by_title[c.title] = by_title.get(c.title, 0) + 1

    print(f"Parsed {len(chunks)} chunks from {src}")
    print(f"Wrote {dst}")
    print(f"  by type: {by_type}")
    print(f"  works:   {len(by_title)}")
    for title, n in sorted(by_title.items(), key=lambda x: -x[1])[:10]:
        print(f"    {title:<55} {n}")
    print("  ...")
    for title, n in sorted(by_title.items(), key=lambda x: -x[1])[-5:]:
        print(f"    {title:<55} {n}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
