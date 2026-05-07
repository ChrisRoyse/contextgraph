---
namespace: discoveries
agent_id: coordinator
timestamp: 2026-04-17T15:45:00Z
status: active
tags: constellation, guard, sonnets, threshold, validation
---

# Shakespeare Constellations (Pre-Existing in RocksDB, Validated)

## Why It Matters
44 per-play Shakespeare constellations (2,770 total members) are already compiled and stored in `CF_CONSTELLATIONS`. Each has per-embedder centroids, cosine spread stats, coherence. **We do NOT need to compile anything new.** The new guard scores candidate text against these existing constellations via `score_against_constellation`.

## Constellation Inventory (from `list_constellations`)
- **Primary anchor**: `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` — "Shakespeare — Sonnets" — 313 members, coherence 0.9036
- Secondary: `1989afc8-5941-4c9d-8d43-225d396b88f3` — "Shakespeare — Macbeth" — 86 members, coherence 0.9218
- Tertiary: `6dfe3898-f1a7-40bb-af25-1ad3b85bf553` — "Shakespeare — Romeo And Juliet" — 78 members, coherence 0.9309
- Full list has 44 play-level + 1 FSV test constellation. Total 45.

## `score_against_constellation` Response Schema (confirmed live)
```json
{
  "combined_score": 0.418,            // unweighted mean cosine across embedders with coverage>0
  "in_spread_p95": false,              // whether candidate is as central as p95 of E1 members
  "constellation_id": "<uuid>",
  "memory_id": "<candidate-fp-uuid>",
  "per_embedder_cosine": [13 floats]   // E1..E13 order
}
```
- `combined_score` is the primary discriminator.
- `in_spread_p95` is useful but too strict for short chat-length candidates (only 14-line sonnets would typically pass).

## Calibrated Thresholds (from live validation 2026-04-17)
| Text | Sonnets score | Macbeth score | Verdict |
|------|---------------|---------------|---------|
| Pure Shakespeare ("Hark! Wherefore art thou skeptical of neural nets? Thou shalt see...") | **0.418** | 0.343 | PASS |
| Mixed ("Thou shalt install Python using pip. Methinks most modern developers prefer venv...") | 0.323 | — | Borderline FAIL |
| Pure modern ("Neural networks are a type of machine learning model inspired by biological...") | 0.254 | 0.250 | FAIL |

**Chosen threshold**: `combined_score >= 0.35` against Sonnets constellation.
- Pure Shakespeare (0.418) → PASS with 0.068 margin
- Mixed (0.323) → FAIL (correct — mixed is not pure style)
- Modern (0.254) → FAIL with 0.096 margin

## Why Sonnets and Not Per-Play
- Largest corpus (313 members vs 86 for Macbeth) → tighter centroid, better spread statistics
- Pure verse without stage direction noise → the purest style signal
- Covers Shakespeare's whole career → most stylistically representative

## How to Apply
The new dashboard `DaemonScorer`:
1. `store_memory(text)` → fingerprintId
2. `score_against_constellation(SONNETS_ID, fingerprintId)` → combined_score + per_embedder_cosine
3. `forget_concept(fingerprintId)` (try/finally)
4. PASS iff `combined_score >= 0.35` AND vocab_density >= 0.02
5. No fallback. No bypass.

## Related
- See also: ./memory/decisions/coordinator--guard-architecture.md
- See also: ./memory/discoveries/coordinator--search-graph-schema-confirmed.md
- Superseded: the old `STYLE_CENTROID` / `STYLE_P5` hard-coded dict approach. Replaced with live constellation lookup.
