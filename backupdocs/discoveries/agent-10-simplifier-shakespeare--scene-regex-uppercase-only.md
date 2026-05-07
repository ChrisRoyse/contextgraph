---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:12:00Z
status: active
severity: high
tags: shakespeare, chunker, regex, corpus-portability
---

# Scene/act regex assumes UPPERCASE; Gutenberg corpus has 32% mixed-case scenes

## What Happened

`scripts/shakespeare_chunker.py:107-112`:

```python
ACT_RE   = re.compile(r"^ACT\s+([IVXLCDM]+)\.?\s*$")
SCENE_RE = re.compile(r"^SCENE\s+([IVXLCDM]+)\.?\s*(.*)$")
```

Uppercase-only. Verified against the actual corpus
(`/home/cabdru/contextgraph/shakespear/complete_works.txt`):

| Pattern              | Matches |
|----------------------|---------|
| `^SCENE\s+[IVX...]+` | 755     |
| `^Scene\s+[IVX...]+` | **365** |
| `^ACT\s+[IVX...]+`   | 284     |
| `^Act\s+[IVX...]+`   | 0       |

So **365 of 1,120 scene markers (32.6%) are silently dropped** because the
text uses mixed-case `Scene I. Rossillon. ...` (e.g., All's Well That Ends Well
opens at line 2935 with `Scene I.` not `SCENE I.`). Dropped scenes are merged
into whatever scene's `pending_scene_header` came before, polluting that scene's
`act`/`scene` metadata and inflating its content.

The user reports "953 scenes" produced — somewhere between the 755 uppercase
and 1,120 total, probably because some plays have BOTH cases mixed and
flush() pulled in the un-headered text under the wrong scene.

## Why It Matters

1. The current `_chunk_play` output silently mis-attributes scene boundaries for
   any play that uses mixed-case scene markers.
2. On a different edition of Shakespeare (e.g., Folger, Arden, MIT online), the
   chunker may produce nearly zero scenes — just one mega-chunk per play.
3. Even on the current corpus, `act`/`scene` metadata in stored memories is
   wrong for ~32% of scenes — downstream `compile_constellation` per session
   is unaffected (sessions are per-work), but any future per-act analysis is
   poisoned.

## Recommended Fix (Flag Only)

Make the regex case-insensitive at the pattern level:

```python
ACT_RE   = re.compile(r"^ACT\s+([IVXLCDM]+)\.?\s*$",       re.IGNORECASE)
SCENE_RE = re.compile(r"^SCENE\s+([IVXLCDM]+)\.?\s*(.*)$", re.IGNORECASE)
```

Then assert at the end of `parse_complete_works` that scene count >=
`expected_min` (e.g., 1000) and fail fast if a future corpus produces fewer.

Same applies to `SONNET_RE` if a future edition centers numbers differently.

## Related

- See also: `./memory/discoveries/agent-10-simplifier-shakespeare--title-bounds-fragile.md`
- Tags: corpus-portability, silent-loss, regex, chunker
