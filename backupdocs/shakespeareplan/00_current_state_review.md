# 00 Current State Review

## Source Material Read

Local context reviewed:

- All SITREPs listed by the user from 2026-04-16 through 2026-04-24.
- `docs/arxivpaper/PAPER.md`, especially the DDA/TCT framing and Case 1 Shakespeare sections.
- `docs/V4_2_NOVEL_METHODS.md`.
- `docs/researchpaper/07_case_shakespeare.md`.
- `docs2/LORA_STYLE_TRANSFER_COMPLETE_REPORT.md`.
- Current scripts under `scripts/` and `scripts/v4_2/`.
- Current training/model artifacts under `~/.contextgraph/training/` and `~/.contextgraph/models/`.

## What Worked

### v2.1 Is The Stable Public Baseline

v2.1 is the most trustworthy model in the history so far because it had:

- Cleaned SFT data after the stage-direction echo bug was found.
- SFT plus DPO, not SFT-only.
- A working three-layer serving stack: LoRA, constrained decoder, constellation guard.
- Graceful degradation compared with v4.1's catastrophic archaic-token loops.

Its rigorous browser FSV was not perfect: 6/10 strict, 7/10 lenient. But its failures are understandable and non-catastrophic: code generation, some adversarial prompts, and non-reproduced Spanish behavior.

### The Architecture Is Correct In Shape

The successful design pattern is:

```text
prompt -> LoRA prior -> constrained decoder -> constellation guard -> accepted output
```

That maps well to TCT:

- Context Graph embedders are frozen.
- Style centroids are frozen.
- Training uses the same geometric target the guard uses.
- Runtime decisions are per-output and auditable.

### The Key Distinction Was Correct

The model must not roleplay as the historical person Shakespeare. It must be a modern knowledgeable assistant that writes in Shakespeare's style. This distinction fixed the "I know not Python" refusal behavior.

### E14 Is A Strategic Addition

E14 BGE-M3 Dense is now in the repo. Its proper role for Shakespeare is content preservation, multilingual equivalence, long-context matching, and cross-lingual tests. It should not be treated as a primary Shakespeare-style discriminator unless calibrated evidence says it helps.

## What Failed

### Dirty Raw Text Caused Echo And Memorization

Earlier data contained scene headers, stage directions, character names, and play-location text. The model learned those starts exactly. This was not a model problem. It was a training-data hygiene problem.

Any future training data must be hard-gated against:

- `SCENE`, `ACT`, `Enter`, `Exit`, `Exeunt`.
- all-caps speaker prefixes.
- bracketed stage directions.
- Project Gutenberg front matter.
- internal Context Graph engineering notes.

### DPO Was Often The Wrong Signal

The original contrastive miner pivoted on semantic differences instead of style differences. It often taught the model to prefer Shakespeare topics rather than Shakespeare style. Later v4.2 CAMD code improves this directionally, but sampled current CAMD records still contain dirty chosen/rejected text and unrelated implementation notes.

Conclusion: the DPO idea is right, but the pair source must be rebuilt with stronger gates.

### v3 Proved Raw Shakespeare Is Not Enough

v3 added more real Shakespeare text but regressed on modern-topic behavior. That proved the main lesson: raw source text teaches vocabulary and cadence, but not application of style to arbitrary modern topics.

Raw Shakespeare should be used mainly as:

- centroid reference material,
- style-analysis material,
- anti-memorization overlap source,
- small anchor set,
- not the dominant chat-response target.

### v4.1 Proved Harness FSV Is Not Product FSV

v4.1 scored well in direct model tests but looped in the dashboard with the constrained decoder and production system prompt. The direct harness omitted several failure-inducing production factors.

Conclusion: the publish gate must run through the browser/dashboard/inference stack.

### Single Style Score Is Not A Coherence Check

Archaic-token loops can score as "Shakespearean" if the guard only measures style similarity. The guard must also measure repetition, semantic progress, output structure, and content preservation.

## Current Workspace Red Flags

### v4.2 Data Artifacts Need Audit

Observed files:

- `~/.contextgraph/training/v4_2_camd_lmdb.jsonl`: 5,000 pairs.
- `~/.contextgraph/training/v4_2_camd_pairs_extended.jsonl.partial`: 49 pairs.
- `~/.contextgraph/models/shakespeare_lora_v4_2/dpo/final/`: trained adapter exists.

Sampled `v4_2_camd_pairs_extended.jsonl.partial` contains:

- scene headers and stage directions in chosen examples,
- tiny scene-header rejected examples,
- unrelated Context Graph implementation notes as rejected examples.

That makes the current v4.2 artifacts unsafe to publish without a full contamination audit and production-stack FSV.

### Existing Dashboard Still Assumes 13 Elements In Places

`scripts/shakespeare_chat_dashboard.py` checks for a 13-element `per_embedder_cosine`. E14 has landed in the Rust/MCP side, so Python consumers must be audited before any E14-aware guard is considered live.

## Strategic Conclusion

The next model should not be "v4.2 plus more". It should be a clean TCT run that treats data hygiene as Phase 0 and uses v2.1 as the stable starting adapter unless a clean-from-base ablation beats it.

