---
namespace: decisions
agent_id: coordinator
timestamp: 2026-04-17T16:05:00Z
status: active
tags: schema, stylistic-analysis, dpo, sft, resurrection
---

# Stylistic Tag Schema (LOCKED — all Phases B/C/D must conform)

The analyzer (Phase B) emits one record per Shakespeare passage. The DPO pair generator (Phase C) and SFT augmenter (Phase D) consume these records to match register-appropriate variants.

## Record Format (JSONL, one per line)

```json
{
  "memory_id": "<uuid>",
  "source": "shakespeare_sft_final_clean|shakespeare_compositions|...",
  "text": "<full Shakespeare passage>",
  "char_count": 347,
  "word_count": 62,
  "line_count": 14,

  "meter": {
    "form": "blank_verse|couplet|sonnet|prose|mixed",
    "avg_syllables_per_line": 10.1,
    "stdev_syllables_per_line": 0.4,
    "iambic_pentameter_pct": 0.85,
    "rhyme_scheme": "abab cdcd efef gg|aa bb cc|null",
    "ends_with_couplet": true
  },

  "devices": {
    "antithesis": 2,
    "chiasmus": 1,
    "anadiplosis": 0,
    "polyptoton": 1,
    "epistrophe": 0,
    "alliteration_per_line": 1.8,
    "personification": 3,
    "classical_allusions": ["Jove", "Fortune"],
    "rhetorical_questions": 1
  },

  "register": {
    "primary": "tragic|comic|philosophical|pleading|commanding|lyrical|bawdy|didactic|dialectical",
    "secondary": "grief|love|rage|jest|awe|contemplation|command|null",
    "confidence": 0.78
  },

  "vocabulary": {
    "tier": "high|mid|low",
    "archaic_density": 0.12,
    "latinate_density": 0.08,
    "anglo_saxon_density": 0.71,
    "unique_word_count": 54,
    "shakespeare_neologisms": ["bedimmed", "outstep"]
  },

  "sentence_shape": {
    "sentence_count": 4,
    "avg_clauses_per_sentence": 3.2,
    "max_subordination_depth": 4,
    "inverted_syntax_count": 6,
    "ends_with_weight": true
  },

  "semantic_topic": "love|death|fate|ambition|jealousy|power|betrayal|honor|nature|time|general",

  "style_profile_vector": [13 floats 0-1],  // per-embedder cosine to Sonnets centroid

  "analyzer_version": "1.0",
  "analyzed_at": "2026-04-17T..."
}
```

## Phase B → Phase C contract
- Phase C uses `register.primary` and `register.secondary` to generate register-appropriate REJECTED variants. A "tragic" passage gets "modern-therapy-speak" rejected; a "comic" passage gets "modern-casual-joke" rejected.
- Phase C uses `meter.form` to determine whether to emit meter-correct-modern variant (only for blank_verse / couplet / sonnet passages, skip for prose).
- Phase C uses `vocabulary.archaic_density` to calibrate: if chosen has density > 0.15, archaic-pastiche REJECTED should have density 0.05-0.10 (cognitively plausible pastiche, not obvious failure).

## Phase B → Phase D contract
- Phase D uses register distribution to BALANCE new augmentation: if existing SFT is 60% tragic / 20% comic / 20% other, new augmentation adds examples to rebalance toward 40/30/30.
- Phase D uses `semantic_topic` to ensure modern-topic compositions cover the full range (not just "love and death").

## Paths (immutable for this mission)
- Input: `~/.contextgraph/training/shakespeare_sft_final_clean.jsonl` (7,925 examples)
- Output (Phase B): `~/.contextgraph/training/shakespeare_stylistic_tags.jsonl`
- Output (Phase C): `~/.contextgraph/training/shakespeare_dpo_style_pairs.jsonl`
- Output (Phase D): `~/.contextgraph/training/shakespeare_sft_resurrection.jsonl`

## Error Handling
- Analyzer fails on a record: log full traceback to `/tmp/shake_analyzer_errors.log`, emit a tag record with `analyzer_error: "<msg>"` field, continue to next. Do NOT silently drop. User must be able to count errors.
- DPO pair generator: if scoring fails for a pair, log and SKIP that pair. If > 1% of pairs fail scoring, abort the whole run — the daemon is broken.
- SFT augmenter: never write an example that the Sonnets constellation scores below 0.45 (chosen must BE Shakespeare style).

## Dependencies
- Phase B: pronouncing (syllable+stress dict), spacy (parse tree), standard stdlib.
- Phase C: requires daemon up on 3100 for constellation scoring. Verify first via probe.
- Phase D: base Gemma-4-E4B via transformers (for paraphrase generation). Daemon must be DOWN while Gemma is loaded (VRAM).
