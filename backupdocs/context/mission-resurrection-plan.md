---
namespace: context
agent_id: coordinator
timestamp: 2026-04-17T16:00:00Z
status: active
tags: mission, resurrection, shakespeare, full-rebuild, 7-phase
---

# Mission: Shakespeare Resurrection (Full Pipeline Rebuild)

## Objective
Produce a model that makes the end user feel Shakespeare is ALIVE — not just archaic vocabulary, but meter, rhetorical devices, emotional register, and essence. This supersedes the previous "guard hardening" mission — we still need the hardened guard, but the deeper problem is the training data itself.

## 10 Style Dimensions We Must Capture
1. **Meter** — iambic pentameter for noble/emotional, prose for comic/low
2. **Rhetorical devices** — antithesis, chiasmus, anadiplosis, polyptoton, epistrophe
3. **Metaphor density** — layered comparisons, conceits, personification
4. **Register shifts** — tragic/comic/philosophical/pleading in same speech
5. **Wordplay** — puns, double meanings, quibbles
6. **Sentence shape** — complex multi-clause subordination with weighted endings
7. **Classical allusions** — Jove, Mars, the Muses, Fortune's wheel
8. **Closing couplets** — the "turn" at the end of a thought
9. **Dialectical voice** — self-questioning, considering both sides
10. **Shakespeare's vocabulary innovations** — ~1,700 coined words

## 7-Phase Plan
- **A** Hotfix + commit (DONE — dashboard guard-hardening + inference suppress-list fix)
- **B** Deep stylistic analysis — tag every Shakespeare passage on meter/devices/register/vocab/sentence-shape
- **C** Multi-axis DPO pair generation — 4 REJECTED variants per chosen, ≥30K clean-separation pairs
- **D** Rich SFT augmentation — 1,500+ examples covering modern topics, registers, forms, cross-lingual
- **E** SFT retrain (~4-5 hrs on RTX 5090)
- **F** DPO retrain (~5-6 hrs on RTX 5090)
- **G** Multi-dimensional validation with hardened guard

## Non-Negotiables
- NO workarounds, NO fallbacks. Every script fails fast with robust logs.
- NO mock data in tests. Real daemon, real corpus, real constellation scoring.
- NO tests that pass on broken state. If the pipeline is broken, tests MUST fail.
- Before SFT/DPO retrain: **GPU must be fully clear** (daemon down, inference down, VRAM free).
- Watch VRAM budget: RTX 5090 has 32.6GB total, SFT at rank-256 uses ~22.5GB → margin for embedders-off is mandatory.
- After training: **restart daemon first**, then start new inference server, then validate.

## Dependencies Between Phases
- B → C (analyzer output feeds DPO pair generator)
- B → D (analyzer output feeds SFT augmenter for register diversity)
- C + D → E (SFT retrain needs augmented data)
- E → F (DPO continues from SFT checkpoint)
- F → G (validation against final model)

## Constellation Anchors (already in RocksDB — DO NOT recompile)
- Primary: `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` — Sonnets (313 members, coherence 0.9036)
- Secondary sampling: 44 per-play constellations for register-aware scoring

## Data Locations
- Clean SFT: `~/.contextgraph/training/shakespeare_sft_final_clean.jsonl` (7,925 examples)
- Dirty DPO (WILL BE REPLACED): `~/.contextgraph/training/shakespeare_dpo_clean.jsonl` (11,724 pairs)
- New DPO output: `~/.contextgraph/training/shakespeare_dpo_style_pairs.jsonl` (TARGET ≥30,000 clean-separation pairs)
- New SFT augmented: `~/.contextgraph/training/shakespeare_sft_resurrection.jsonl`
- Stylistic analysis output: `~/.contextgraph/training/shakespeare_stylistic_tags.jsonl`

## Service State During Each Phase
| Phase | Daemon | Inference | GPU Use |
|-------|--------|-----------|---------|
| A (hotfix) | up | off | 20.7 GB |
| B (analysis) | up | off | 20.7 GB (CPU-bound analyzer) |
| C (DPO pairs, constellation scoring) | up | off | 20.7 GB |
| D (SFT augmentation, Gemma-base paraphrase) | DOWN | DOWN | 0 → Gemma loads ~11.5 GB |
| E (SFT retrain) | DOWN | DOWN | 0 → trainer ~22.5 GB |
| F (DPO retrain) | DOWN | DOWN | 0 → trainer ~22.5 GB |
| G (validation) | up | up (new model) | 20.7 + 11.5 GB |
